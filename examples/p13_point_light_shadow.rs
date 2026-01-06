//! Phase 13: Point Light Shadow Test
//!
//! Tests point light shadows with occluders:
//! - Flat ground plane with pillars
//! - Point light above center casting shadows
//!
//! Run with: `cargo run --example p13_point_light_shadow`
//!
//! Expected output: `screenshots/p13_point_light_shadow.png`

use bevy::prelude::*;
use studio_core::VoxelWorldApp;

fn main() {
    VoxelWorldApp::new("Phase 13: Point Light Shadow")
        .with_world_file("assets/worlds/shadow_test.voxworld")
        .with_shadow_light(Vec3::new(8.0, 6.0, 8.0))
        .with_camera_position(Vec3::new(20.0, 15.0, 20.0), Vec3::new(8.0, 0.0, 8.0))
        .with_screenshot("screenshots/p13_point_light_shadow.png")
        .run();
}
