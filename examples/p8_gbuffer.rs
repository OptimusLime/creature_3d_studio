//! Phase 8: Deferred Rendering Pipeline
//!
//! Tests the deferred rendering G-Buffer pipeline.
//!
//! Run with: `cargo run --example p8_gbuffer`
//!
//! Expected output: `screenshots/p8_gbuffer.png`
//! - Scene rendered through deferred pipeline
//! - Deep purple background from fog color

use bevy::prelude::*;
use studio_core::VoxelWorldApp;

fn main() {
    VoxelWorldApp::new("Phase 8: Deferred Rendering Pipeline")
        .with_lua_script("assets/scripts/test_emission.lua")
        .with_deferred(true)
        .with_camera_angle(45.0, 30.0) // Auto-frame to voxel bounds
        .with_screenshot("screenshots/p8_gbuffer.png")
        .run();
}
