//! Phase 27: MarkovJunior ImGui Integration Test
//!
//! Tests the Phase 3.3 and 3.6 features:
//! - ImGui PNG save button
//! - Live 3D voxel rendering from Lua
//!
//! Run with: `cargo run --example p27_markov_imgui`

use bevy::prelude::*;
use studio_core::markov_junior::{MjPalette, Model};
use studio_core::{VoxelWorldApp, WorldSource};

fn main() {
    println!("Phase 27: MarkovJunior ImGui Integration Test");

    // Create a simple 3D growth model programmatically
    let xml = r#"<one values="BW" origin="True" in="B" out="W"/>"#;

    let size = 16;
    let mut model =
        Model::load_str(xml, size, size, size).expect("Failed to create 3D growth model");

    // Run the model
    let seed = 42;
    let max_steps = 1500;
    let steps = model.run(seed, max_steps);
    let grid = model.grid();

    let nonzero = grid.count_nonzero();
    println!("Generation complete: {} steps, {} voxels", steps, nonzero);

    // Test PNG rendering (Phase 3.3)
    use std::path::Path;
    use studio_core::markov_junior::render::render_to_png;

    let png_path = Path::new("screenshots/p27_markov_imgui_test.png");
    render_to_png(grid, png_path, 8).expect("Failed to render PNG");
    println!("PNG saved to: {}", png_path.display());

    // Convert to VoxelWorld using palette
    let palette = MjPalette::from_grid(grid);
    let world = grid.to_voxel_world(&palette);

    println!("VoxelWorld contains {} voxels", world.total_voxel_count());

    // Run with VoxelWorldApp
    VoxelWorldApp::new("Phase 27: MarkovJunior ImGui Test")
        .with_world(WorldSource::World(world))
        .with_resolution(1280, 720)
        .with_clear_color(Color::srgb(0.1, 0.1, 0.15))
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_camera_angle(45.0, 30.0)
        .with_zoom(0.6)
        .with_screenshot("screenshots/p27_markov_imgui.png")
        .run();
}
