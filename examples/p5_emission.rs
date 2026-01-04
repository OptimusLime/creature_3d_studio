//! Phase 5: Emission Test
//!
//! Verifies emission vertex attribute affects brightness.
//! Uses HDR rendering to preserve values > 1.0.
//!
//! Run with: `cargo run --example p5_emission`
//!
//! Expected output: `screenshots/p5_emission.png`
//! - 4 white voxels with brightness gradient

use bevy::prelude::*;
use studio_core::VoxelWorldApp;

fn main() {
    VoxelWorldApp::new("Phase 5: Emission")
        .with_lua_script("assets/scripts/test_emission.lua")
        .with_deferred(false)
        .with_hdr(true)
        .with_emissive_lights(false)
        .with_clear_color(Color::BLACK)
        .with_camera_angle(0.0, 20.0) // Front view, slight elevation
        .with_screenshot_timed("screenshots/p5_emission.png", 5, 15)
        .run();
}
