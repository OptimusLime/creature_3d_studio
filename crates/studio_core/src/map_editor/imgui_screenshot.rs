//! Post-ImGui Screenshot Capture
//!
//! Captures screenshots that include ImGui panels by reading from the
//! intermediate texture that `bevy_mod_imgui` renders to.
//!
//! # How It Works
//!
//! 1. `bevy_mod_imgui` renders ImGui to an intermediate texture with `COPY_SRC` usage
//! 2. This plugin adds a render graph node that runs AFTER the ImGui blit node
//! 3. That node copies the intermediate texture to a staging buffer
//! 4. An async task reads back the buffer and saves to disk
//!
//! # Usage
//!
//! ```ignore
//! app.add_plugins(ImguiScreenshotPlugin)
//!    .insert_resource(ImguiScreenshotConfig::new("screenshot.png"));
//! ```

use bevy::app::AppExit;
use bevy::ecs::world::FromWorld;
use bevy::prelude::*;
use bevy::render::{
    render_graph::{Node, NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel},
    render_resource::{Buffer, BufferUsages},
    renderer::{RenderContext, RenderDevice},
    view::ExtractedWindows,
    Extract, Render, RenderApp, RenderSystems,
};
use bevy::tasks::AsyncComputeTaskPool;
use bevy_mod_imgui::prelude::{ImguiIntermediateTexture, ImguiNodeLabel};
use std::path::Path;
use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, Mutex,
};
use wgpu::{Extent3d, TextureFormat};

/// Configuration for ImGui-aware screenshot capture.
#[derive(Resource, Clone)]
pub struct ImguiScreenshotConfig {
    /// Path to save the screenshot.
    pub path: String,
    /// Frame number to capture on.
    pub capture_frame: u32,
}

impl ImguiScreenshotConfig {
    /// Create a new configuration.
    pub fn new(path: impl Into<String>, capture_frame: u32) -> Self {
        Self {
            path: path.into(),
            capture_frame,
        }
    }
}

/// Configuration for automatic app exit after N frames.
#[derive(Resource, Clone)]
pub struct AutoExitConfig {
    /// Frame number to exit on.
    pub exit_frame: u32,
}

impl AutoExitConfig {
    /// Create a new auto-exit configuration.
    pub fn new(exit_frame: u32) -> Self {
        Self { exit_frame }
    }
}

/// Plugin that provides ImGui-aware screenshot capture.
pub struct ImguiScreenshotPlugin;

impl Plugin for ImguiScreenshotPlugin {
    fn build(&self, app: &mut App) {
        // Main world resources and systems
        let (tx, rx) = channel::<(u32, u32, Vec<u8>, TextureFormat)>();
        app.insert_resource(ScreenshotReceiver(Arc::new(Mutex::new(rx))))
            .insert_resource(FrameCount(0))
            .add_systems(
                Update,
                (trigger_screenshot_system, receive_screenshot_system).chain(),
            );

        // Render world setup
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .insert_resource(ScreenshotSender(tx))
            .init_resource::<ImguiScreenshotState>()
            .add_systems(
                bevy::render::ExtractSchedule,
                extract_screenshot_config.ambiguous_with_all(),
            )
            .add_systems(
                Render,
                prepare_screenshot_resources.in_set(RenderSystems::Prepare),
            )
            .add_systems(
                Render,
                collect_imgui_screenshots_system.in_set(RenderSystems::Cleanup),
            );
    }

    fn finish(&self, app: &mut App) {
        // Add our screenshot node to run AFTER ImGui blit
        // This must be done in finish() because ImguiPlugin adds its nodes in finish()
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        use bevy::core_pipeline::core_2d::graph::Core2d;

        render_app
            .add_render_graph_node::<PostImguiScreenshotNode>(Core2d, PostImguiScreenshotLabel);
        // Run after the ImGui node so we capture after ImGui has rendered
        render_app.add_render_graph_edges(Core2d, (ImguiNodeLabel, PostImguiScreenshotLabel));
    }
}

/// Wrapper system to call collect_imgui_screenshots.
fn collect_imgui_screenshots_system(world: &mut World) {
    collect_imgui_screenshots(world);
}

/// Render label for our post-ImGui screenshot node.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct PostImguiScreenshotLabel;

/// Frame counter resource.
#[derive(Resource)]
struct FrameCount(u32);

/// Channel to receive completed screenshots in main world.
#[derive(Resource)]
struct ScreenshotReceiver(Arc<Mutex<Receiver<(u32, u32, Vec<u8>, TextureFormat)>>>);

/// Channel to send completed screenshots from render world.
#[derive(Resource, Clone)]
struct ScreenshotSender(Sender<(u32, u32, Vec<u8>, TextureFormat)>);

/// State for the screenshot capture in render world.
#[derive(Resource, Default)]
struct ImguiScreenshotState {
    /// Whether a capture is requested this frame.
    capture_requested: bool,
    /// Path to save the screenshot.
    save_path: Option<String>,
    /// Staging buffer for readback.
    staging_buffer: Option<Buffer>,
    /// Size of the capture.
    size: Option<Extent3d>,
    /// Format of the capture.
    format: Option<TextureFormat>,
}

/// Extract screenshot configuration from main world.
fn extract_screenshot_config(
    config: Extract<Option<Res<ImguiScreenshotConfig>>>,
    frame_count: Extract<Res<FrameCount>>,
    mut state: ResMut<ImguiScreenshotState>,
) {
    state.capture_requested = false;
    state.save_path = None;

    if let Some(config) = config.as_ref() {
        if frame_count.0 == config.capture_frame {
            state.capture_requested = true;
            state.save_path = Some(config.path.clone());
        }
    }
}

/// Prepare screenshot resources (staging buffer).
fn prepare_screenshot_resources(
    windows: Res<ExtractedWindows>,
    intermediate: Option<Res<ImguiIntermediateTexture>>,
    render_device: Res<RenderDevice>,
    mut state: ResMut<ImguiScreenshotState>,
) {
    if !state.capture_requested {
        return;
    }

    let Some(intermediate) = intermediate else {
        return;
    };

    let Some(primary) = windows.primary else {
        return;
    };
    let Some(window) = windows.windows.get(&primary) else {
        return;
    };

    let width = window.physical_width.min(intermediate.size.width);
    let height = window.physical_height.min(intermediate.size.height);
    let format = intermediate.format;

    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    // Calculate buffer size with row alignment (256 bytes)
    let pixel_size = format.block_copy_size(None).unwrap_or(4);
    let unpadded_row_bytes = width * pixel_size;
    let padded_row_bytes = align_to_256(unpadded_row_bytes);
    let buffer_size = padded_row_bytes as u64 * height as u64;

    let staging_buffer = render_device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("imgui-screenshot-staging-buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    state.staging_buffer = Some(staging_buffer);
    state.size = Some(size);
    state.format = Some(format);
}

/// Align to 256 bytes (wgpu requirement for buffer copies).
fn align_to_256(value: u32) -> u32 {
    (value + 255) & !255
}

/// The render graph node that captures the screenshot from the intermediate texture.
struct PostImguiScreenshotNode;

impl Node for PostImguiScreenshotNode {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let state = world.resource::<ImguiScreenshotState>();

        if !state.capture_requested {
            return Ok(());
        }

        let Some(ref staging_buffer) = state.staging_buffer else {
            return Ok(());
        };
        let Some(size) = state.size else {
            return Ok(());
        };
        let Some(format) = state.format else {
            return Ok(());
        };

        let Some(intermediate) = world.get_resource::<ImguiIntermediateTexture>() else {
            return Ok(());
        };

        let encoder = render_context.command_encoder();

        // Copy from intermediate texture to buffer
        let pixel_size = format.block_copy_size(None).unwrap_or(4);
        let padded_row_bytes = align_to_256(size.width * pixel_size);

        encoder.copy_texture_to_buffer(
            intermediate.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes),
                    rows_per_image: Some(size.height),
                },
            },
            size,
        );

        Ok(())
    }
}

impl FromWorld for PostImguiScreenshotNode {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

/// System to increment frame count and handle auto-exit.
#[allow(deprecated)]
fn trigger_screenshot_system(
    mut frame_count: ResMut<FrameCount>,
    screenshot_config: Option<Res<ImguiScreenshotConfig>>,
    exit_config: Option<Res<AutoExitConfig>>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;

    // Log when capturing screenshot
    if let Some(ref config) = screenshot_config {
        if frame_count.0 == config.capture_frame {
            println!(
                "Capturing post-ImGui screenshot at frame {} to {}...",
                frame_count.0, config.path
            );
        }
    }

    // Handle auto-exit (independent of screenshot)
    if let Some(ref config) = exit_config {
        if frame_count.0 >= config.exit_frame {
            println!("Exiting after {} frames", frame_count.0);
            exit.write(AppExit::Success);
        }
    }
}

/// System to receive and save completed screenshots.
fn receive_screenshot_system(
    receiver: Res<ScreenshotReceiver>,
    config: Option<Res<ImguiScreenshotConfig>>,
) {
    let Some(config) = config else {
        return;
    };

    let Ok(guard) = receiver.0.try_lock() else {
        return;
    };

    while let Ok((width, height, data, format)) = guard.try_recv() {
        save_screenshot(&config.path, width, height, &data, format);
    }
}

/// Save screenshot data to disk.
fn save_screenshot(path: &str, width: u32, height: u32, data: &[u8], format: TextureFormat) {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Convert to image based on format
    let img = match format {
        TextureFormat::Bgra8UnormSrgb | TextureFormat::Bgra8Unorm => {
            // BGRA -> RGBA conversion
            let mut rgba_data = data.to_vec();
            for chunk in rgba_data.chunks_exact_mut(4) {
                chunk.swap(0, 2); // Swap B and R
            }
            image::RgbaImage::from_raw(width, height, rgba_data)
        }
        TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm => {
            image::RgbaImage::from_raw(width, height, data.to_vec())
        }
        _ => {
            eprintln!("Unsupported texture format for screenshot: {:?}", format);
            return;
        }
    };

    match img {
        Some(img) => {
            // Convert to RGB (drop alpha) for PNG saving
            let rgb_img: image::RgbImage = image::DynamicImage::ImageRgba8(img).to_rgb8();
            match rgb_img.save(path) {
                Ok(_) => println!("SUCCESS: Screenshot saved to {}", path),
                Err(e) => eprintln!("Failed to save screenshot: {}", e),
            }
        }
        None => {
            eprintln!("Failed to create image from screenshot data");
        }
    }
}

// =============================================================================
// Async buffer readback
// =============================================================================

/// System in render world to trigger async buffer readback after rendering.
pub fn collect_imgui_screenshots(world: &mut World) {
    let state = world.resource::<ImguiScreenshotState>();

    if !state.capture_requested {
        return;
    }

    let Some(buffer) = state.staging_buffer.clone() else {
        return;
    };
    let Some(size) = state.size else {
        return;
    };
    let Some(format) = state.format else {
        return;
    };

    let sender = world.resource::<ScreenshotSender>().clone();
    let width = size.width;
    let height = size.height;
    let pixel_size = format.block_copy_size(None).unwrap_or(4);
    let unpadded_row_bytes = width * pixel_size;
    let padded_row_bytes = align_to_256(unpadded_row_bytes);

    let finish = async move {
        let (tx, rx) = async_channel::bounded(1);
        let buffer_slice = buffer.slice(..);

        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            if let Err(e) = result {
                eprintln!("Failed to map screenshot buffer: {}", e);
            }
            let _ = tx.try_send(());
        });

        rx.recv().await.ok();

        let data = buffer_slice.get_mapped_range();
        let mut result = Vec::with_capacity((width * height * pixel_size) as usize);

        // Remove row padding
        for row in 0..height {
            let start = (row * padded_row_bytes) as usize;
            let end = start + unpadded_row_bytes as usize;
            result.extend_from_slice(&data[start..end]);
        }

        drop(data);
        buffer.unmap();

        if sender.0.send((width, height, result, format)).is_err() {
            eprintln!("Failed to send screenshot data");
        }
    };

    AsyncComputeTaskPool::get().spawn(finish).detach();
}
