//! Phase 19: Dual Moon Shadow Test
//!
//! Demonstrates the dual moon shadow system with clear visual separation:
//! - Moon 1 (purple): Coming from the LEFT, casts shadows to the RIGHT
//! - Moon 2 (orange): Coming from the RIGHT, casts shadows to the LEFT
//!
//! The scene uses a simple pillar arrangement where you can clearly see
//! each pillar casting TWO distinct colored shadows in opposite directions.
//!
//! NO point lights are used - this is purely about moon lighting.
//!
//! Run with: `cargo run --example p19_dual_moon_shadows`
//!
//! Expected output: `screenshots/p19_dual_moon_shadows.png`

use bevy::prelude::*;
use studio_core::{MoonConfig, Voxel, VoxelWorld, VoxelWorldApp, WorldSource};

fn main() {
    VoxelWorldApp::new("Phase 19: Dual Moon Shadows")
        .with_world(WorldSource::Builder(Box::new(build_shadow_demo_world)))
        // No point light! We want to see the moons clearly
        .with_emissive_lights(false)
        .with_greedy_meshing(true) // Re-enabled: SSAO fixes the vertex AO interpolation bug
        .with_moon_config(MoonConfig {
            // Purple moon - coming from LEFT side (positive X)
            // High angle so shadows are clearly visible on ground
            moon1_direction: Vec3::new(0.7, -0.5, 0.0).normalize(),
            moon1_color: Vec3::new(0.6, 0.2, 1.0), // Bright purple
            moon1_intensity: 0.8,                  // Much brighter for visibility

            // Orange moon - coming from RIGHT side (negative X)
            // Similar high angle, opposite horizontal direction
            moon2_direction: Vec3::new(-0.7, -0.5, 0.0).normalize(),
            moon2_color: Vec3::new(1.0, 0.5, 0.1), // Bright orange
            moon2_intensity: 0.7,                  // Slightly dimmer so colors don't wash out

            // Shadow parameters
            shadow_size: 50.0,
            near: 0.1,
            far: 200.0,
            directional_shadow_softness: 0.3, // Moderate softness
            point_shadow_softness: 0.3,
        })
        .with_camera_position(
            Vec3::new(30.0, 25.0, 30.0), // High angle to see shadows on ground
            Vec3::new(8.0, 4.0, 8.0),
        )
        .with_clear_color(Color::srgb(0.02, 0.01, 0.03)) // Very dark background
        .with_screenshot("screenshots/p19_dual_moon_shadows.png")
        .run();
}

/// Build a simple demo world with pillars that cast clear shadows
fn build_shadow_demo_world(world: &mut VoxelWorld) {
    let gray = Voxel::solid(128, 128, 128);
    let white = Voxel::solid(200, 200, 200);

    // Large flat ground plane (16x16)
    // Gray floor so shadows show up well
    for x in 0..16 {
        for z in 0..16 {
            world.set_voxel(x, 0, z, gray);
        }
    }

    // Three pillars arranged in a row (perpendicular to moon directions)
    // This arrangement means each pillar casts TWO shadows - one from each moon

    // Pillar 1 (front) - white so we can see colored lighting on it
    for y in 1..6 {
        world.set_voxel(8, y, 4, white);
    }

    // Pillar 2 (middle)
    for y in 1..8 {
        world.set_voxel(8, y, 8, white);
    }

    // Pillar 3 (back)
    for y in 1..5 {
        world.set_voxel(8, y, 12, white);
    }

    // A small wall on the left side to catch orange light
    for z in 2..14 {
        for y in 1..4 {
            world.set_voxel(2, y, z, white);
        }
    }

    // A small wall on the right side to catch purple light
    for z in 2..14 {
        for y in 1..4 {
            world.set_voxel(14, y, z, white);
        }
    }
}
