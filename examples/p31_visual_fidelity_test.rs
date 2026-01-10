//! Phase 31: Visual Fidelity Test Harness
//!
//! Automated screenshot capture from multiple camera angles for visual verification.
//! Used to verify visual improvements in the visual fidelity enhancement phases.
//!
//! Run with: `cargo run --example p31_visual_fidelity_test`
//!
//! Output: screenshots/visual_fidelity_test/
//!   - sky_up.png         - Looking straight up at the sky
//!   - sky_horizon.png    - Looking at the horizon (sky meets terrain)
//!   - building_front.png - Close view of generated building
//!   - building_aerial.png - Aerial/top-down view of building
//!   - terrain_distance.png - Looking at distant terrain
//!
//! The test auto-exits after capturing all screenshots.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::f32::consts::TAU;
use std::path::Path;
use studio_core::deferred::SkyDomeConfig;
use studio_core::markov_junior::render::RenderPalette;
use studio_core::markov_junior::Model;
use studio_core::{DeferredLightingConfig, Voxel, VoxelWorld, VoxelWorldApp, WorldSource};

const OUTPUT_DIR: &str = "screenshots/visual_fidelity_test";

fn main() {
    println!("==============================================");
    println!("  Phase 31: Visual Fidelity Test Harness");
    println!("==============================================");
    println!();
    println!("Output directory: {}", OUTPUT_DIR);
    println!();

    // Ensure output directory exists
    std::fs::create_dir_all(OUTPUT_DIR).expect("Failed to create output directory");

    let terrain = build_terrain();

    VoxelWorldApp::new("Phase 31: Visual Fidelity Test")
        .with_resolution(1280, 720)
        .with_world(WorldSource::World(terrain))
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_clear_color(Color::srgb(0.4, 0.6, 0.9))
        .with_shadow_light(Vec3::new(50.0, 80.0, 50.0))
        .with_camera_position(Vec3::new(0.0, 50.0, 0.0), Vec3::new(0.0, 0.0, 0.0))
        .with_resource(DeferredLightingConfig {
            fog_start: 100.0,
            fog_end: 500.0,
            ..Default::default()
        })
        .with_resource(MjPalette(
            RenderPalette::from_palette_xml().with_emission('R', 130),
        ))
        .with_resource(CaptureState::new())
        .with_update_systems(|app| {
            app.add_systems(Startup, setup_building);
            app.add_systems(Update, capture_sequence_system);
        })
        .run();
}

/// Build terrain with 40x40 center stage for the building
fn build_terrain() -> VoxelWorld {
    let mut terrain = VoxelWorld::new();

    // Large ground platform (80x80, 2 blocks thick)
    let ground_color = Voxel::solid(60, 90, 60);
    for x in -40..40 {
        for z in -40..40 {
            for y in 0..2 {
                terrain.set_voxel(x, y, z, ground_color);
            }
        }
    }

    // Stone border around center stage (42x42)
    let stone_color = Voxel::solid(100, 100, 110);
    for x in -21..21 {
        terrain.set_voxel(x, 2, -21, stone_color);
        terrain.set_voxel(x, 2, 20, stone_color);
    }
    for z in -21..21 {
        terrain.set_voxel(-21, 2, z, stone_color);
        terrain.set_voxel(20, 2, z, stone_color);
    }

    // Center stage platform (40x40) - matches grid size
    let platform_color = Voxel::solid(80, 80, 85);
    for x in -20..20 {
        for z in -20..20 {
            terrain.set_voxel(x, 2, z, platform_color);
        }
    }

    // Corner pillars with glowing crystals
    let pillar_color = Voxel::solid(150, 140, 130);
    let crystal_color = Voxel::emissive(100, 200, 255);
    let corners = [(-22, -22), (-22, 21), (21, -22), (21, 21)];
    for (cx, cz) in corners {
        for y in 2..12 {
            terrain.set_voxel(cx, y, cz, pillar_color);
        }
        terrain.set_voxel(cx, 12, cz, crystal_color);
    }

    terrain
}

// ============================================================================
// Resources
// ============================================================================

#[derive(Resource)]
struct MjPalette(RenderPalette);

/// Defines a camera position for a specific screenshot capture.
#[derive(Clone)]
struct CapturePosition {
    name: &'static str,
    position: Vec3,
    look_at: Vec3,
    /// Optional time of day override (0.0-1.0) for moon positioning
    time_of_day: Option<f32>,
    /// If true, look_at is computed from moon1 direction at time_of_day
    track_moon1: bool,
    /// If true, look_at is computed from moon2 direction at time_of_day
    track_moon2: bool,
}

/// Calculate moon direction using same orbital math as sky_dome_node.rs
/// Orbit keeps moons above horizon for visual impact
/// MUST MATCH sky_dome_node.rs MoonOrbit::calculate_direction exactly!
fn calculate_moon_direction(
    period: f32,
    phase_offset: f32,
    inclination: f32,
    min_altitude: f32,
    cycle_time: f32,
) -> Vec3 {
    let moon_time = (cycle_time / period + phase_offset).fract();
    let angle = moon_time * TAU;
    let incline_rad = inclination.to_radians();

    // Orbit in XZ plane (horizontal), with varying altitude
    let x = angle.cos();
    let z = angle.sin() * incline_rad.sin();

    // Altitude varies smoothly, always above min_altitude
    // MUST match sky_dome_node.rs: altitude_range = 0.8 - min_altitude
    let altitude_range = 0.8 - min_altitude;
    let raw_altitude = angle.sin(); // -1 to 1
    let y = min_altitude + (raw_altitude + 1.0) * 0.5 * altitude_range;

    Vec3::new(x, y, z).normalize()
}

/// Get purple moon direction at given time
/// MUST MATCH sky_dome_node.rs MoonOrbit::purple()
fn moon1_direction(time: f32) -> Vec3 {
    // period=1.0, phase_offset=0.0, inclination=25.0, min_altitude=0.15
    calculate_moon_direction(1.0, 0.0, 25.0, 0.15, time)
}

/// Get orange moon direction at given time
/// MUST MATCH sky_dome_node.rs MoonOrbit::orange()
fn moon2_direction(time: f32) -> Vec3 {
    // period=0.7, phase_offset=0.35, inclination=15.0, min_altitude=0.1
    calculate_moon_direction(0.7, 0.35, 15.0, 0.1, time)
}

/// State machine for the capture sequence.
#[derive(Resource)]
struct CaptureState {
    captures: Vec<CapturePosition>,
    current_index: usize,
    frames_waited: u32,
    wait_frames: u32,
    capture_pending: bool,
    complete: bool,
}

impl CaptureState {
    fn new() -> Self {
        // Define all the camera positions for our test captures
        // Numbered for clear ordering and reference
        let captures = vec![
            // 01: Purple moon - tracking camera looks directly at moon1
            CapturePosition {
                name: "01_purple_moon",
                position: Vec3::new(0.0, 5.0, 0.0),
                look_at: Vec3::ZERO,
                time_of_day: Some(0.15),
                track_moon1: true,
                track_moon2: false,
            },
            // 02: Orange moon - tracking camera looks directly at moon2
            CapturePosition {
                name: "02_orange_moon",
                position: Vec3::new(0.0, 5.0, 0.0),
                look_at: Vec3::ZERO,
                time_of_day: Some(0.0),
                track_moon1: false,
                track_moon2: true,
            },
            // 03: Dual moons - looking straight up at zenith
            CapturePosition {
                name: "03_dual_moons",
                position: Vec3::new(0.0, 5.0, 0.0),
                look_at: Vec3::new(0.0, 100.0, 0.0), // Look straight UP (point above camera)
                time_of_day: Some(0.25),
                track_moon1: false,
                track_moon2: false,
            },
            // 04: Sky gradient - look at horizon to see gradient
            CapturePosition {
                name: "04_sky_gradient",
                position: Vec3::new(0.0, 5.0, 0.0),
                look_at: Vec3::new(1.0, 0.2, 0.0), // Look toward horizon
                time_of_day: Some(0.5),
                track_moon1: false,
                track_moon2: false,
            },
            // 05: Building scene with sky
            CapturePosition {
                name: "05_building_scene",
                position: Vec3::new(40.0, 25.0, 40.0),
                look_at: Vec3::new(0.0, 20.0, 0.0),
                time_of_day: Some(0.15),
                track_moon1: false,
                track_moon2: false,
            },
            // 06: Terrain with moon in background
            CapturePosition {
                name: "06_terrain_moon",
                position: Vec3::new(0.0, 15.0, -50.0),
                look_at: Vec3::new(0.0, 30.0, 50.0),
                time_of_day: Some(0.12),
                track_moon1: false,
                track_moon2: false,
            },
        ];

        Self {
            captures,
            current_index: 0,
            frames_waited: 0,
            wait_frames: 15, // Wait 15 frames between captures for scene to settle
            capture_pending: false,
            complete: false,
        }
    }

    fn current_capture(&self) -> Option<&CapturePosition> {
        self.captures.get(self.current_index)
    }
}

// ============================================================================
// Systems
// ============================================================================

/// Setup system that generates a building with a fixed seed.
fn setup_building(world: &mut World) {
    // Load and run the Apartemazements model with fixed seed for reproducibility
    let xml_path = Path::new("MarkovJunior/models/Apartemazements.xml");

    if !xml_path.exists() {
        warn!("MarkovJunior model not found at {:?}", xml_path);
        warn!("Building will not be generated - testing sky/terrain only");
        return;
    }

    let mut model = match Model::load_with_size(xml_path, 8, 8, 8) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to load model: {}", e);
            return;
        }
    };

    // Run to completion with fixed seed
    model.reset(42);
    while model.step() {}

    // Get the palette
    let _palette = world
        .get_resource::<MjPalette>()
        .map(|p| p.0.clone())
        .unwrap_or_else(RenderPalette::from_palette_xml);

    // Convert to voxels and add to terrain
    // The grid is 40x40x40 after map expansion, offset to center on platform
    let grid = model.grid();

    // We need access to the VoxelWorld which is stored differently in VoxelWorldApp
    // For now, just log that we would add the building
    info!(
        "Building generated: {}x{}x{} grid, {} steps",
        grid.mx,
        grid.my,
        grid.mz,
        model.counter()
    );

    // Note: The building won't actually appear in the terrain for this initial version
    // because VoxelWorldApp doesn't give us direct access to modify the world after startup.
    // This is acceptable for Phase 0 - we're testing the screenshot harness, not the building.
    // The building integration will be improved in later phases.
}

/// Main capture sequence system.
fn capture_sequence_system(
    mut state: ResMut<CaptureState>,
    mut camera_query: Query<&mut Transform, With<Camera>>,
    mut sky_config: ResMut<SkyDomeConfig>,
    mut commands: Commands,
    mut exit: bevy::ecs::event::EventWriter<AppExit>,
) {
    if state.complete {
        return;
    }

    // Get current capture target
    let Some(capture) = state.current_capture().cloned() else {
        // All captures done
        println!();
        println!("==============================================");
        println!("  All captures complete!");
        println!("==============================================");
        println!("Output: {}/", OUTPUT_DIR);
        for cap in &state.captures {
            println!("  - {}.png", cap.name);
        }
        state.complete = true;
        exit.write(AppExit::Success);
        return;
    };

    // If we just started a new capture, position the camera and set time
    if state.frames_waited == 0 && !state.capture_pending {
        // Set time of day first (needed for moon direction calculation)
        let time = capture.time_of_day.unwrap_or(0.2);
        sky_config.time_of_day = time;

        // Compute look_at target
        let look_at = if capture.track_moon1 {
            let moon_dir = moon1_direction(time);
            println!("Moon1 direction at t={}: {:?}", time, moon_dir);
            capture.position + moon_dir * 100.0
        } else if capture.track_moon2 {
            let moon_dir = moon2_direction(time);
            println!("Moon2 direction at t={}: {:?}", time, moon_dir);
            capture.position + moon_dir * 100.0
        } else {
            capture.look_at
        };

        // Position camera for this capture
        for mut transform in camera_query.iter_mut() {
            transform.translation = capture.position;
            transform.look_at(look_at, Vec3::Y);
        }

        println!("Positioning camera for: {} (time={})", capture.name, time);
    }

    // Wait for scene to settle
    if state.frames_waited < state.wait_frames {
        state.frames_waited += 1;
        return;
    }

    // Take screenshot
    if !state.capture_pending {
        let path = format!("{}/{}.png", OUTPUT_DIR, capture.name);
        println!("Capturing: {}", path);

        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));

        state.capture_pending = true;
        return;
    }

    // Move to next capture (screenshot was initiated last frame)
    state.current_index += 1;
    state.frames_waited = 0;
    state.capture_pending = false;
}
