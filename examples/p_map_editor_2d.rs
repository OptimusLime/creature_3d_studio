//! Map Editor 2D - Phase 1 Foundation
//!
//! A 2D voxel map editor with Lua scripting and hot reload.
//!
//! Run with: `cargo run --example p_map_editor_2d`
//!
//! Expected output: `screenshots/p_map_editor_2d.png`

use bevy::app::AppExit;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy_mod_imgui::prelude::{Condition, ImguiContext};
use std::path::Path;

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p_map_editor_2d.png";
const GRID_WIDTH: usize = 32;
const GRID_HEIGHT: usize = 32;
const CELL_SIZE: f32 = 16.0;
const CANVAS_SCALE: f32 = 10.0; // Scale up the tiny 32x32 texture for visibility

// ============================================================================
// Data Structures
// ============================================================================

/// A 2D voxel material with id, name, and color.
#[derive(Clone, Debug)]
struct Material {
    id: u32,
    name: String,
    color: [f32; 3], // RGB, 0.0-1.0
}

/// 2D voxel buffer storing material IDs.
#[derive(Resource)]
struct VoxelBuffer2D {
    width: usize,
    height: usize,
    data: Vec<u32>, // Material ID per cell, 0 = empty
}

impl VoxelBuffer2D {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0; width * height],
        }
    }

    fn set(&mut self, x: usize, y: usize, material_id: u32) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = material_id;
        }
    }

    fn get(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.data[y * self.width + x]
        } else {
            0
        }
    }
}

/// Collection of available materials.
#[derive(Resource)]
struct MaterialPalette {
    materials: Vec<Material>,
    selected_index: usize,
}

impl MaterialPalette {
    fn get_by_id(&self, id: u32) -> Option<&Material> {
        self.materials.iter().find(|m| m.id == id)
    }

    fn selected(&self) -> Option<&Material> {
        self.materials.get(self.selected_index)
    }
}

/// Handle to the render texture and its ImGui registration.
#[derive(Resource)]
struct CanvasTexture {
    handle: Handle<Image>,
    imgui_id: Option<bevy_mod_imgui::prelude::TextureId>,
}

/// Marker for the canvas sprite entity.
#[derive(Component)]
struct CanvasSprite;

#[derive(Resource)]
struct FrameCount(u32);

// ============================================================================
// Main
// ============================================================================

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Map Editor 2D...");
    println!("Expected output: {}", SCREENSHOT_PATH);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1024, 768).into(),
                title: "Map Editor 2D".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(bevy_mod_imgui::ImguiPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.15, 0.15, 0.15)))
        .insert_resource(FrameCount(0))
        .insert_resource(VoxelBuffer2D::new(GRID_WIDTH, GRID_HEIGHT))
        .insert_resource(create_default_palette())
        .insert_resource(CheckerboardState::default())
        .insert_resource(PlaybackState::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                generate_checkerboard,
                update_playback,
                update_canvas_texture,
                render_ui,
                capture_screenshot,
            )
                .chain(),
        )
        .run();

    if Path::new(SCREENSHOT_PATH).exists() {
        println!("SUCCESS: Screenshot saved to {}", SCREENSHOT_PATH);
    } else {
        println!("FAILED: Screenshot was not created at {}", SCREENSHOT_PATH);
        std::process::exit(1);
    }
}

fn create_default_palette() -> MaterialPalette {
    MaterialPalette {
        materials: vec![
            Material {
                id: 1,
                name: "stone".into(),
                color: [0.5, 0.5, 0.5],
            },
            Material {
                id: 2,
                name: "dirt".into(),
                color: [0.6, 0.4, 0.2],
            },
        ],
        selected_index: 0,
    }
}

// ============================================================================
// Systems
// ============================================================================

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);

    // Create the canvas texture
    let size = Extent3d {
        width: GRID_WIDTH as u32,
        height: GRID_HEIGHT as u32,
        depth_or_array_layers: 1,
    };

    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[30, 30, 30, 255], // Dark gray initially
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());

    let handle = images.add(image);

    // Spawn a sprite to display the canvas texture (scaled up for visibility)
    commands.spawn((
        Sprite {
            image: handle.clone(),
            custom_size: Some(Vec2::new(
                GRID_WIDTH as f32 * CANVAS_SCALE,
                GRID_HEIGHT as f32 * CANVAS_SCALE,
            )),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        CanvasSprite,
    ));

    commands.insert_resource(CanvasTexture {
        handle,
        imgui_id: None,
    });
}

/// Checkerboard generation state - tracks which materials to use.
#[derive(Resource)]
struct CheckerboardState {
    material_a: u32, // Used for even cells
    material_b: u32, // Used for odd cells
    needs_regenerate: bool,
}

impl Default for CheckerboardState {
    fn default() -> Self {
        Self {
            material_a: 1, // stone
            material_b: 2, // dirt
            needs_regenerate: true,
        }
    }
}

/// Playback state for step-by-step generation.
#[derive(Resource)]
struct PlaybackState {
    playing: bool,
    speed: f32,        // Cells per second (1-1000)
    step_index: usize, // Current cell index
    accumulator: f32,  // Time accumulator
    completed: bool,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            playing: false,
            speed: 100.0,
            step_index: 0,
            accumulator: 0.0,
            completed: false,
        }
    }
}

/// Step one cell in checkerboard generation.
fn step_checkerboard(
    buffer: &mut VoxelBuffer2D,
    checker_state: &CheckerboardState,
    playback: &mut PlaybackState,
) {
    let total_cells = buffer.width * buffer.height;
    if playback.step_index >= total_cells {
        playback.completed = true;
        return;
    }

    let x = playback.step_index % buffer.width;
    let y = playback.step_index / buffer.width;

    let mat_id = if (x + y) % 2 == 0 {
        checker_state.material_a
    } else {
        checker_state.material_b
    };
    buffer.set(x, y, mat_id);

    playback.step_index += 1;
    if playback.step_index >= total_cells {
        playback.completed = true;
    }
}

/// Update playback - advance generation based on speed.
fn update_playback(
    time: Res<Time>,
    mut buffer: ResMut<VoxelBuffer2D>,
    checker_state: Res<CheckerboardState>,
    mut playback: ResMut<PlaybackState>,
) {
    // Handle reset request (when materials change)
    if checker_state.needs_regenerate {
        return; // Will be handled by generate_checkerboard
    }

    if playback.completed || !playback.playing {
        return;
    }

    playback.accumulator += time.delta_secs() * playback.speed;

    while playback.accumulator >= 1.0 && !playback.completed {
        playback.accumulator -= 1.0;
        step_checkerboard(&mut buffer, &checker_state, &mut playback);
    }
}

/// Handle regeneration requests (when materials change).
fn generate_checkerboard(
    mut buffer: ResMut<VoxelBuffer2D>,
    mut state: ResMut<CheckerboardState>,
    mut playback: ResMut<PlaybackState>,
) {
    if !state.needs_regenerate {
        return;
    }
    state.needs_regenerate = false;

    // Clear buffer and reset playback
    for i in 0..buffer.data.len() {
        buffer.data[i] = 0;
    }
    playback.step_index = 0;
    playback.completed = false;
    playback.accumulator = 0.0;

    // Fill immediately (can be changed to step-by-step later)
    for y in 0..buffer.height {
        for x in 0..buffer.width {
            let mat_id = if (x + y) % 2 == 0 {
                state.material_a
            } else {
                state.material_b
            };
            buffer.set(x, y, mat_id);
        }
    }
    playback.step_index = buffer.width * buffer.height;
    playback.completed = true;
}

/// Update the canvas texture from the voxel buffer.
fn update_canvas_texture(
    buffer: Res<VoxelBuffer2D>,
    palette: Res<MaterialPalette>,
    canvas: Res<CanvasTexture>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(image) = images.get_mut(&canvas.handle) else {
        return;
    };

    for y in 0..buffer.height {
        for x in 0..buffer.width {
            let mat_id = buffer.get(x, y);
            let color = if mat_id == 0 {
                [30, 30, 30, 255] // Empty = dark gray
            } else if let Some(mat) = palette.get_by_id(mat_id) {
                [
                    (mat.color[0] * 255.0) as u8,
                    (mat.color[1] * 255.0) as u8,
                    (mat.color[2] * 255.0) as u8,
                    255,
                ]
            } else {
                [255, 0, 255, 255] // Unknown = magenta
            };

            let idx = (y * buffer.width + x) * 4;
            if let Some(ref mut data) = image.data {
                data[idx..idx + 4].copy_from_slice(&color);
            }
        }
    }
}

fn render_ui(
    mut context: NonSendMut<ImguiContext>,
    mut canvas: ResMut<CanvasTexture>,
    mut palette: ResMut<MaterialPalette>,
    mut checker_state: ResMut<CheckerboardState>,
    mut playback: ResMut<PlaybackState>,
    mut buffer: ResMut<VoxelBuffer2D>,
) {
    // Register texture with ImGui on first frame
    if canvas.imgui_id.is_none() {
        let id = context.register_bevy_texture(canvas.handle.clone());
        canvas.imgui_id = Some(id);
    }

    let ui = context.ui();

    // === Material Picker Panel (Left side) ===
    ui.window("Materials")
        .size([180.0, 300.0], Condition::FirstUseEver)
        .position([20.0, 20.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Select Material:");
            ui.separator();

            let mut new_selection = None;
            let num_materials = palette.materials.len();
            let current_selection = palette.selected_index;

            for i in 0..num_materials {
                let mat = &palette.materials[i];
                let is_selected = i == current_selection;
                let color = [mat.color[0], mat.color[1], mat.color[2], 1.0];

                // Color swatch button
                let _color_token = ui.push_style_color(imgui::StyleColor::Button, color);
                let _color_hover = ui.push_style_color(
                    imgui::StyleColor::ButtonHovered,
                    [color[0] * 1.2, color[1] * 1.2, color[2] * 1.2, 1.0],
                );

                let label = if is_selected {
                    format!("> {} <", mat.name)
                } else {
                    mat.name.clone()
                };

                if ui.button_with_size(&label, [150.0, 30.0]) {
                    new_selection = Some(i);
                }
            }

            if let Some(idx) = new_selection {
                palette.selected_index = idx;
                // Update checkerboard to use selected material as material_b
                if let Some(mat) = palette.materials.get(idx) {
                    checker_state.material_b = mat.id;
                    checker_state.needs_regenerate = true;
                }
            }

            ui.separator();
            if let Some(selected) = palette.selected() {
                ui.text(format!("Selected: {}", selected.name));
                ui.text(format!(
                    "Checker: {} + {}",
                    checker_state.material_a, checker_state.material_b
                ));
            }
        });

    // === Canvas window showing the checkerboard ===
    ui.window("Canvas")
        .size(
            [
                GRID_WIDTH as f32 * CELL_SIZE + 20.0,
                GRID_HEIGHT as f32 * CELL_SIZE + 40.0,
            ],
            Condition::FirstUseEver,
        )
        .position([220.0, 20.0], Condition::FirstUseEver)
        .build(|| {
            if let Some(tex_id) = canvas.imgui_id {
                imgui::Image::new(
                    tex_id,
                    [
                        GRID_WIDTH as f32 * CELL_SIZE,
                        GRID_HEIGHT as f32 * CELL_SIZE,
                    ],
                )
                .build(ui);
            } else {
                ui.text("Texture not registered");
            }
        });

    // === Playback Controls (Bottom) ===
    ui.window("Playback")
        .size([400.0, 120.0], Condition::FirstUseEver)
        .position(
            [220.0, GRID_HEIGHT as f32 * CELL_SIZE + 80.0],
            Condition::FirstUseEver,
        )
        .build(|| {
            // Play/Pause button
            let play_label = if playback.playing { "Pause" } else { "Play" };
            if ui.button(play_label) {
                playback.playing = !playback.playing;
            }

            ui.same_line();

            // Step button
            if ui.button("Step") && !playback.completed {
                step_checkerboard(&mut buffer, &checker_state, &mut playback);
            }

            ui.same_line();

            // Reset button
            if ui.button("Reset") {
                // Clear buffer
                for i in 0..buffer.data.len() {
                    buffer.data[i] = 0;
                }
                playback.step_index = 0;
                playback.completed = false;
                playback.accumulator = 0.0;
                playback.playing = false;
            }

            // Speed slider
            let mut speed = playback.speed;
            if ui.slider("Speed", 1.0f32, 1000.0f32, &mut speed) {
                playback.speed = speed;
            }
            ui.text(format!("{:.0} cells/sec", playback.speed));

            // Progress display
            let total_cells = buffer.width * buffer.height;
            let progress = playback.step_index as f32 / total_cells as f32 * 100.0;
            ui.text(format!(
                "Progress: {}/{} ({:.1}%)",
                playback.step_index, total_cells, progress
            ));

            if playback.completed {
                ui.text_colored([0.0, 1.0, 0.0, 1.0], "Generation complete!");
            }
        });
}

#[allow(deprecated)]
fn capture_screenshot(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;

    if frame_count.0 == 30 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }

    if frame_count.0 >= 45 {
        println!("Exiting after {} frames", frame_count.0);
        exit.write(AppExit::Success);
    }
}
