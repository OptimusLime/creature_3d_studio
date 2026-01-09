//! Phase 21: Day/Night Cycle Demo
//!
//! Demonstrates the day/night cycle system with:
//! - Independent dual-moon orbits
//! - Automatic position/color/intensity cycling
//! - Shadows tracking moon positions
//! - Emissive crystals with point lights
//! - Screenshot sequence capture
//!
//! Run with: `cargo run --example p21_day_night_cycle`
//!
//! Two modes:
//! 1. Default: Captures 8 screenshots at key cycle times, then exits
//! 2. With --interactive: Watch the cycle in real-time (no screenshots)
//!
//! Screenshots saved to: screenshots/day_night_cycle/

use bevy::prelude::*;
use studio_core::{
    DayNightCycle, MoonCycleConfig, ScreenshotSequence, Voxel, VoxelWorld, VoxelWorldApp,
    WorldSource,
};

fn main() {
    // Check for --interactive flag
    let interactive = std::env::args().any(|arg| arg == "--interactive");

    let mut app = VoxelWorldApp::new("Phase 21: Day/Night Cycle")
        .with_world(WorldSource::Builder(Box::new(build_dark_world)))
        .with_emissive_lights(true) // Enable point lights from crystals
        .with_greedy_meshing(true)
        .with_day_night_cycle(DayNightCycle {
            speed: 0.1, // 10 seconds per cycle (fast for demo)
            moon1_config: MoonCycleConfig::purple_moon(),
            moon2_config: MoonCycleConfig::orange_moon(),
            ..DayNightCycle::dark_world()
        })
        .with_camera_angle(45.0, 35.0)
        .with_zoom(0.7)
        .with_clear_color(Color::srgb(0.02, 0.01, 0.03)); // Very dark background

    // Add screenshot sequence unless in interactive mode
    if !interactive {
        app = app.with_screenshot_sequence(ScreenshotSequence::dark_world_highlights(
            "screenshots/day_night_cycle",
        ));
    }

    app.run();
}

/// Build a dark fantasy scene with terrain, pillars, altar, and glowing crystals.
/// Based on test_darkworld.lua but built programmatically.
fn build_dark_world(world: &mut VoxelWorld) {
    // Colors
    let obsidian = Voxel::solid(20, 15, 25); // Dark purple-black rock
    let dark_stone = Voxel::solid(35, 30, 40); // Slightly lighter purple stone
    let dark_metal = Voxel::solid(45, 40, 50); // For ruins

    // Emissive crystals (using Voxel::new for custom emission values)
    let purple_crystal = Voxel::new(120, 40, 220, 220);
    let orange_crystal = Voxel::emissive(255, 100, 20); // Full emission (255)
    let cyan_crystal = Voxel::new(50, 220, 255, 240);
    let pink_crystal = Voxel::new(255, 100, 150, 230);

    // Ground plane with slight height variation
    for x in 0..24 {
        for z in 0..24 {
            let height_noise = ((x as f32 * 0.3).sin() * (z as f32 * 0.4).cos() + 0.5) as i32;
            let base_y = height_noise.max(0);

            let color = if (x + z) % 7 == 0 {
                dark_stone
            } else {
                obsidian
            };

            for y in 0..=base_y {
                world.set_voxel(x, y, z, color);
            }
        }
    }

    // Raised platform/altar in center
    for x in 9..15 {
        for z in 9..15 {
            for y in 0..3 {
                world.set_voxel(x, y, z, dark_stone);
            }
        }
    }
    for x in 10..14 {
        for z in 10..14 {
            world.set_voxel(x, 3, z, obsidian);
        }
    }

    // Pillars at corners of altar
    let pillar_positions = [(9, 9), (14, 9), (9, 14), (14, 14)];
    for (i, (px, pz)) in pillar_positions.iter().enumerate() {
        let height = if i == 0 { 10 } else { 7 }; // One taller pillar
        for y in 0..height {
            world.set_voxel(*px, y, *pz, dark_metal);
        }
    }

    // Central altar orb (orange - matches one moon)
    world.set_voxel(11, 5, 11, orange_crystal);
    world.set_voxel(12, 5, 11, orange_crystal);
    world.set_voxel(11, 5, 12, orange_crystal);
    world.set_voxel(12, 5, 12, orange_crystal);
    world.set_voxel(11, 6, 12, orange_crystal);
    world.set_voxel(12, 6, 11, orange_crystal);

    // Purple crystal cluster (left side)
    world.set_voxel(3, 1, 6, purple_crystal);
    world.set_voxel(3, 2, 6, purple_crystal);
    world.set_voxel(3, 3, 6, purple_crystal);
    world.set_voxel(4, 1, 7, purple_crystal);
    world.set_voxel(4, 2, 7, purple_crystal);

    // Cyan crystal (front right)
    world.set_voxel(20, 1, 3, cyan_crystal);
    world.set_voxel(20, 2, 3, cyan_crystal);
    world.set_voxel(20, 3, 3, cyan_crystal);
    world.set_voxel(20, 4, 3, cyan_crystal);

    // Pink crystal (back)
    world.set_voxel(8, 1, 20, pink_crystal);
    world.set_voxel(8, 2, 20, pink_crystal);
    world.set_voxel(8, 3, 20, pink_crystal);

    // Floating rock with crystal
    for x in 16..19 {
        for z in 6..9 {
            world.set_voxel(x, 6, z, dark_stone);
            world.set_voxel(x, 7, z, dark_stone);
        }
    }
    world.set_voxel(17, 8, 7, orange_crystal);

    // Wall for shadow testing
    for z in 10..16 {
        for y in 1..4 {
            world.set_voxel(4, y, z, dark_stone);
        }
    }
}
