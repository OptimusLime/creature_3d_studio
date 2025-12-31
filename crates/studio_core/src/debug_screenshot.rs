//! Debug Screenshot System
//!
//! Provides multi-screenshot capture for systematic debugging of render pipelines.
//! Captures multiple screenshots per scene with different debug modes, organized
//! into folders for easy comparison.
//!
//! # Usage
//!
//! ```ignore
//! use studio_core::{DebugScreenshotConfig, DebugCapture};
//!
//! // Configure debug screenshots
//! let config = DebugScreenshotConfig::new("screenshots/gtao_test")
//!     .with_capture("render", DebugCapture::default())  // Normal render
//!     .with_capture("depth", DebugCapture::gtao_debug(11))  // Linear depth
//!     .with_capture("ao_only", DebugCapture::lighting_debug(5));  // AO only
//!
//! // Use with VoxelWorldApp
//! VoxelWorldApp::new("Test")
//!     .with_debug_screenshots(config)
//!     .run();
//! ```

use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;

/// Configuration for a single debug capture.
#[derive(Clone, Debug)]
pub struct DebugCapture {
    /// Name for the screenshot file (without extension)
    pub name: String,
    /// GTAO shader debug mode (passed via uniform)
    pub gtao_debug_mode: i32,
    /// Lighting shader debug mode (passed via uniform)  
    pub lighting_debug_mode: i32,
    /// Denoiser debug mode: 0=normal, 1=sum_weight, 2=edges_c, 3=blur_amount, 4=diff
    pub denoise_debug_mode: u32,
    /// Number of frames to wait before capture (for stabilization)
    pub wait_frames: u32,
}

impl Default for DebugCapture {
    fn default() -> Self {
        Self {
            name: "render".to_string(),
            gtao_debug_mode: 0,
            lighting_debug_mode: 0,
            denoise_debug_mode: 0,
            wait_frames: 5,
        }
    }
}

impl DebugCapture {
    /// Create a new debug capture with a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..default()
        }
    }

    /// Create a capture with GTAO debug mode.
    ///
    /// GTAO debug modes:
    /// - 0: Normal GTAO output
    /// - 10: NDC depth
    /// - 11: Linear viewspace depth
    /// - 12-15: Depth MIP levels 0-3
    /// - 20: View-space normal.z
    /// - 30: Screenspace radius
    /// - 40: Packed edges
    pub fn gtao_debug(mode: i32) -> Self {
        Self {
            name: format!("gtao_mode_{}", mode),
            gtao_debug_mode: mode,
            lighting_debug_mode: 0,
            denoise_debug_mode: 0,
            wait_frames: 5,
        }
    }

    /// Create a capture with denoiser debug mode.
    ///
    /// Denoiser debug modes:
    /// - 0: Normal denoised output
    /// - 1: Sum weight (normalized to [0,1] by /8)
    /// - 2: Min edges_c after symmetry
    /// - 3: Blur amount (normalized by /2)
    /// - 4: Difference from input (*10)
    pub fn denoise_debug(mode: u32) -> Self {
        Self {
            name: format!("denoise_mode_{}", mode),
            gtao_debug_mode: 0,
            lighting_debug_mode: 5, // Show AO only
            denoise_debug_mode: mode,
            wait_frames: 5,
        }
    }

    /// Create a capture with lighting debug mode.
    ///
    /// Lighting debug modes:
    /// - 0: Final lit scene
    /// - 1: G-buffer normals
    /// - 2: G-buffer depth
    /// - 3: Albedo only
    /// - 4: Shadow factor
    /// - 5: GTAO (ambient occlusion)
    /// - 6: Point lights only
    /// - 7: World position XZ
    pub fn lighting_debug(mode: i32) -> Self {
        Self {
            name: format!("lighting_mode_{}", mode),
            gtao_debug_mode: 0,
            lighting_debug_mode: mode,
            denoise_debug_mode: 0,
            wait_frames: 5,
        }
    }

    /// Set custom name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set wait frames before capture.
    pub fn with_wait_frames(mut self, frames: u32) -> Self {
        self.wait_frames = frames;
        self
    }
}

/// Configuration for debug screenshot session.
#[derive(Clone, Debug, Default)]
pub struct DebugScreenshotConfig {
    /// Output folder for screenshots
    pub output_folder: String,
    /// List of captures to perform in sequence
    pub captures: Vec<DebugCapture>,
    /// Base wait frames before first capture (for scene stabilization)
    pub base_wait_frames: u32,
}

impl DebugScreenshotConfig {
    /// Create a new debug screenshot config with output folder.
    pub fn new(output_folder: impl Into<String>) -> Self {
        Self {
            output_folder: output_folder.into(),
            captures: Vec::new(),
            base_wait_frames: 10,
        }
    }

    /// Add a capture to the sequence.
    pub fn with_capture(mut self, name: impl Into<String>, mut capture: DebugCapture) -> Self {
        capture.name = name.into();
        self.captures.push(capture);
        self
    }

    /// Add a default render capture (no debug modes).
    pub fn with_render(self) -> Self {
        self.with_capture("render", DebugCapture::default())
    }

    /// Add GTAO depth visualization.
    pub fn with_gtao_depth(self) -> Self {
        self.with_capture("gtao_depth", DebugCapture::gtao_debug(11))
    }

    /// Add GTAO normal visualization.
    pub fn with_gtao_normal(self) -> Self {
        self.with_capture("gtao_normal", DebugCapture::gtao_debug(20))
    }

    /// Add AO-only visualization (from lighting shader).
    pub fn with_ao_only(self) -> Self {
        self.with_capture("ao_only", DebugCapture::lighting_debug(5))
    }

    /// Add common GTAO debug set.
    pub fn with_gtao_debug_set(self) -> Self {
        self.with_render()
            .with_gtao_depth()
            .with_gtao_normal()
            .with_ao_only()
            .with_capture("gtao_edges", DebugCapture::gtao_debug(40))
            .with_capture("gtao_radius", DebugCapture::gtao_debug(30))
    }

    /// Set base wait frames.
    pub fn with_base_wait_frames(mut self, frames: u32) -> Self {
        self.base_wait_frames = frames;
        self
    }
}

/// Resource tracking debug screenshot state.
#[derive(Resource)]
pub struct DebugScreenshotState {
    pub config: DebugScreenshotConfig,
    pub current_capture_index: usize,
    pub frames_waited: u32,
    pub capture_pending: bool,
    pub complete: bool,
}

impl DebugScreenshotState {
    pub fn new(config: DebugScreenshotConfig) -> Self {
        Self {
            config,
            current_capture_index: 0,
            frames_waited: 0,
            capture_pending: false,
            complete: false,
        }
    }

    /// Get current capture config, if any.
    pub fn current_capture(&self) -> Option<&DebugCapture> {
        self.config.captures.get(self.current_capture_index)
    }

    /// Get the path for the current capture.
    pub fn current_path(&self) -> Option<String> {
        self.current_capture().map(|c| {
            format!("{}/{}.png", self.config.output_folder, c.name)
        })
    }

    /// Total frames to wait for current capture.
    pub fn current_wait_frames(&self) -> u32 {
        if self.current_capture_index == 0 {
            self.config.base_wait_frames
                + self.current_capture().map(|c| c.wait_frames).unwrap_or(0)
        } else {
            self.current_capture().map(|c| c.wait_frames).unwrap_or(5)
        }
    }
}

/// Resource for current debug mode (extracted to render world).
/// 
/// This resource controls shader debug modes at runtime without recompilation.
/// It is extracted to the render world and passed to shaders via uniforms.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct DebugModes {
    /// GTAO shader debug mode (0 = normal, 10+ = various debug visualizations)
    pub gtao_debug_mode: i32,
    /// Lighting shader debug mode (0 = normal, 1-7 = various debug visualizations)
    pub lighting_debug_mode: i32,
    /// Denoiser debug mode (0 = normal, 1-4 = debug visualizations)
    pub denoise_debug_mode: u32,
}

/// System to process debug screenshot captures.
#[allow(deprecated)]
pub fn debug_screenshot_system(
    mut commands: Commands,
    mut state: ResMut<DebugScreenshotState>,
    mut debug_modes: ResMut<DebugModes>,
    mut app_exit: bevy::prelude::EventWriter<bevy::app::AppExit>,
) {
    if state.complete {
        return;
    }

    // Get current capture config
    let Some(capture) = state.current_capture().cloned() else {
        // No more captures, we're done
        state.complete = true;
        println!("Debug screenshots complete!");
        app_exit.write(bevy::app::AppExit::Success);
        return;
    };

    // Update debug modes for current capture
    debug_modes.gtao_debug_mode = capture.gtao_debug_mode;
    debug_modes.lighting_debug_mode = capture.lighting_debug_mode;
    debug_modes.denoise_debug_mode = capture.denoise_debug_mode;

    // Wait for stabilization
    let wait_needed = state.current_wait_frames();
    if state.frames_waited < wait_needed {
        state.frames_waited += 1;
        return;
    }

    // Take screenshot if not pending
    if !state.capture_pending {
        let path = state.current_path().unwrap();
        
        // Ensure directory exists
        if let Some(parent) = Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        println!(
            "Capturing debug screenshot: {} (gtao={}, lighting={})",
            path, capture.gtao_debug_mode, capture.lighting_debug_mode
        );

        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        
        state.capture_pending = true;
    } else {
        // Screenshot was captured last frame, move to next
        state.capture_pending = false;
        state.frames_waited = 0;
        state.current_capture_index += 1;
    }
}

/// Plugin for debug screenshot functionality.
pub struct DebugScreenshotPlugin;

impl Plugin for DebugScreenshotPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugModes>()
            .add_systems(Update, debug_screenshot_system.run_if(resource_exists::<DebugScreenshotState>));
    }
}
