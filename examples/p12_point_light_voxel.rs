//! Phase 12: Minimal Point Light Voxel Test
//!
//! Simple scene to test point lights:
//! - Small flat ground (7x7 gray voxels)
//! - One red emissive voxel above the ground
//!
//! Run with: `cargo run --example p12_point_light_voxel`
//!
//! Expected: Red glow on the ground from the point light

use bevy::prelude::*;
use studio_core::{Voxel, VoxelWorldApp};

fn main() {
    VoxelWorldApp::new("Phase 12: Point Light Voxel Test")
        .with_world_builder(|world| {
            // 7x7 ground at y=0
            let ground = Voxel::solid(128, 128, 128);
            for x in -3..=3 {
                for z in -3..=3 {
                    world.set_voxel(x, 0, z, ground);
                }
            }

            // Red emissive voxel at center, y=3
            world.set_voxel(0, 3, 0, Voxel::emissive(255, 50, 50));
        })
        .with_deferred(true)
        .with_shadow_light(Vec3::new(0.0, 5.0, 0.0))
        .with_clear_color(Color::srgb(0.02, 0.02, 0.02))
        .with_camera_position(Vec3::new(5.0, 8.0, 8.0), Vec3::new(0.0, 1.0, 0.0))
        .with_screenshot("screenshots/p12_point_light_voxel.png")
        .run();
}
