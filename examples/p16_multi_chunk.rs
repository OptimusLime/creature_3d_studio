//! Phase 16: Multi-Chunk World Test
//!
//! Demonstrates rendering a world with multiple chunks.
//!
//! Run with: `cargo run --example p16_multi_chunk`
//!
//! Expected output: `screenshots/p16_multi_chunk.png`
//! - 4 chunks arranged in a 2x2 grid
//! - Different colored terrain per chunk with glowing crystals

use bevy::prelude::*;
use studio_core::VoxelWorldApp;

fn main() {
    VoxelWorldApp::new("Phase 16: Multi-Chunk World")
        .with_world_file("assets/worlds/multi_chunk_terrain.voxworld")
        .with_resolution(1024, 768)
        .with_deferred(true)
        .with_shadow_light(Vec3::new(32.0, 20.0, 32.0))
        .with_camera_angle(45.0, 35.0)
        .with_zoom(0.5) // Zoom in 50% more for closer detail
        .with_screenshot("screenshots/p16_multi_chunk.png")
        .run();
}
