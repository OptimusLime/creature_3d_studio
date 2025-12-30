//! Phase 9: Floating Island Test Scene
//!
//! Demonstrates the deferred rendering pipeline with:
//! - Floating island with grass, dirt, stone layers
//! - Glowing crystals with point lights
//! - Shadow-casting light
//!
//! Run with: `cargo run --example p9_island`
//!
//! Expected output: `screenshots/p9_island.png`

use bevy::prelude::*;
use studio_core::VoxelWorldApp;

fn main() {
    VoxelWorldApp::new("Phase 9: Floating Island")
        .with_world_file("assets/worlds/island.voxworld")
        .with_shadow_light(Vec3::new(-6.0, 10.0, -6.0))
        .with_camera_angle(45.0, 30.0)
        .with_screenshot("screenshots/p9_island.png")
        .run();
}
