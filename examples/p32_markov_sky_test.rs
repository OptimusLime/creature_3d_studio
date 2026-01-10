//! Phase 32: MarkovJunior Sky Test Harness
//!
//! Automated screenshot capture for verifying the layered MarkovJunior sky system.
//! This is the test harness for all sky-related phases (clouds, moons, stars).
//!
//! Run with: `cargo run --example p32_markov_sky_test`
//!
//! Output: screenshots/markov_sky/frame_0001.png
//!
//! The test auto-exits after capturing the screenshot.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use studio_core::deferred::SkyDomeConfig;
use studio_core::{DeferredLightingConfig, Voxel, VoxelWorld, VoxelWorldApp, WorldSource};

const OUTPUT_DIR: &str = "screenshots/markov_sky";

fn main() {
    println!("==============================================");
    println!("  Phase 32: MarkovJunior Sky Test Harness");
    println!("==============================================");
    println!();
    println!("Output directory: {}", OUTPUT_DIR);
    println!();

    // Ensure output directory exists
    std::fs::create_dir_all(OUTPUT_DIR).expect("Failed to create output directory");

    let terrain = build_minimal_terrain();

    VoxelWorldApp::new("Phase 32: MarkovJunior Sky Test")
        .with_resolution(1280, 720)
        .with_world(WorldSource::World(terrain))
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_clear_color(Color::srgb(0.0, 0.0, 0.0)) // Black - sky should override
        .with_shadow_light(Vec3::new(50.0, 80.0, 50.0))
        // Camera looking at horizon to see gradient (horizon at bottom, zenith at top)
        .with_camera_position(Vec3::new(0.0, 5.0, 0.0), Vec3::new(100.0, 5.0, 0.0))
        .with_resource(DeferredLightingConfig {
            fog_start: 100.0,
            fog_end: 500.0,
            ..Default::default()
        })
        .with_resource(SkyDomeConfig {
            enabled: true,
            time_of_day: 0.15, // Night time, moons visible
            moons_enabled: true,
            ..Default::default()
        })
        .with_resource(CaptureState::new())
        .with_update_systems(|app| {
            app.add_systems(Update, capture_system);
        })
        .run();
}

/// Build minimal terrain - just a small platform for context
fn build_minimal_terrain() -> VoxelWorld {
    let mut terrain = VoxelWorld::new();

    // Small ground platform (20x20, 1 block thick)
    let ground_color = Voxel::solid(40, 50, 40);
    for x in -10..10 {
        for z in -10..10 {
            terrain.set_voxel(x, 0, z, ground_color);
        }
    }

    terrain
}

// ============================================================================
// Resources
// ============================================================================

/// State machine for screenshot capture.
#[derive(Resource)]
struct CaptureState {
    frames_waited: u32,
    wait_frames: u32,
    capture_pending: bool,
    frames_after_capture: u32,
    complete: bool,
}

impl CaptureState {
    fn new() -> Self {
        Self {
            frames_waited: 0,
            wait_frames: 10, // Wait 10 frames for scene to settle
            capture_pending: false,
            frames_after_capture: 0,
            complete: false,
        }
    }
}

// ============================================================================
// Systems
// ============================================================================

/// Capture system - waits for scene to settle, then captures screenshot.
fn capture_system(
    mut state: ResMut<CaptureState>,
    mut commands: Commands,
    mut exit: bevy::ecs::event::EventWriter<AppExit>,
) {
    if state.complete {
        return;
    }

    // Wait for scene to settle
    if state.frames_waited < state.wait_frames {
        state.frames_waited += 1;
        return;
    }

    // Take screenshot
    if !state.capture_pending {
        let path = format!("{}/frame_0001.png", OUTPUT_DIR);
        println!("Capturing: {}", path);

        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));

        state.capture_pending = true;
        return;
    }

    // Wait a few frames after capture for the screenshot to be written
    state.frames_after_capture += 1;
    if state.frames_after_capture < 5 {
        return;
    }

    // Screenshot should be saved now, exit
    println!();
    println!("==============================================");
    println!("  Capture complete!");
    println!("==============================================");
    println!("Output: {}/frame_0001.png", OUTPUT_DIR);
    state.complete = true;
    exit.write(AppExit::Success);
}
