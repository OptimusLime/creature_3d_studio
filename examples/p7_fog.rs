//! Phase 7: Distance Fog
//!
//! Verifies Bonsai-style additive fog in voxel.wgsl.
//! Voxels at different depths show fog gradient.
//!
//! Run with: `cargo run --example p7_fog`
//!
//! Expected output: `screenshots/p7_fog.png`
//! - 4 white voxels at varying depths with fog gradient
//! - Near voxel: full white, no fog
//! - Far voxel: heavily tinted purple from fog

use bevy::prelude::*;
use studio_core::{BloomConfig, VoxelWorldApp};

fn main() {
    VoxelWorldApp::new("Phase 7: Distance Fog")
        .with_world_file("assets/worlds/fog_test.voxworld")
        .with_deferred(false)
        .with_bloom_config(BloomConfig {
            intensity: 0.15,
            low_frequency_boost: 0.5,
            threshold: 1.0,
            threshold_softness: 0.5,
        })
        .with_emissive_lights(false)
        // Fog color - objects fade into the fog
        .with_clear_color(Color::srgb(0.102, 0.039, 0.180))
        // Position camera to see all 4 voxels at different depths
        .with_camera_position(
            Vec3::new(-8.0, 3.0, -5.0),
            Vec3::new(0.0, 0.0, 20.0),
        )
        .with_screenshot_timed("screenshots/p7_fog.png", 5, 15)
        .run();
}
