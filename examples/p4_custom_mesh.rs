//! Phase 4: Custom Mesh from Voxel Chunk
//!
//! Demonstrates VoxelMaterial with merged mesh from Lua script.
//! Uses forward rendering (no deferred pipeline).
//!
//! Run with: `cargo run --example p4_custom_mesh`
//!
//! Expected output: `screenshots/p4_custom_mesh.png`

use bevy::prelude::*;
use studio_core::VoxelWorldApp;

fn main() {
    VoxelWorldApp::new("Phase 4: Custom Mesh")
        .with_lua_script("assets/scripts/test_creature.lua")
        .with_deferred(false)
        .with_emissive_lights(false)
        .with_clear_color(Color::BLACK)
        .with_camera_angle(45.0, 30.0) // Auto-frame to voxel bounds
        .with_screenshot_timed("screenshots/p4_custom_mesh.png", 5, 15)
        .run();
}
