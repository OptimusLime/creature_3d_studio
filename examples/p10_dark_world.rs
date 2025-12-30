//! Phase 10: Dark World Test Scene
//!
//! Dark fantasy scene with:
//! - Point lights from emissive voxels
//! - Shadow casting light
//!
//! Run with: `cargo run --example p10_dark_world`
//!
//! Expected output: `screenshots/p10_dark_world.png`

use bevy::prelude::*;
use studio_core::VoxelWorldApp;

fn main() {
    VoxelWorldApp::new("Phase 10: Dark World")
        .with_lua_script("assets/scripts/test_darkworld.lua")
        .with_resolution(1024, 768)
        .with_deferred(true)
        .with_shadow_light(Vec3::new(0.0, 5.0, 0.0))
        .with_clear_color(Color::srgb(0.02, 0.01, 0.03))
        .with_camera_angle(45.0, 35.0)
        .with_zoom(1.8) // Zoom out to see the full scene
        .with_screenshot("screenshots/p10_dark_world.png")
        .run();
}
