//! Phase 6: Bloom Post-Processing
//!
//! Verifies Bloom works with VoxelMaterial.
//! High-emission voxels have visible glow halos.
//!
//! Run with: `cargo run --example p6_bloom`
//!
//! Expected output: `screenshots/p6_bloom.png`
//! - 4 white voxels with bloom halos on bright ones

use bevy::prelude::*;
use studio_core::VoxelWorldApp;

fn main() {
    VoxelWorldApp::new("Phase 6: Bloom")
        .with_lua_script("assets/scripts/test_emission.lua")
        .with_deferred(false)
        .with_bloom()
        .with_emissive_lights(false)
        .with_clear_color(Color::BLACK)
        .with_camera_angle(0.0, 20.0) // Front view, slight elevation
        .with_screenshot_timed("screenshots/p6_bloom.png", 5, 15)
        .run();
}
