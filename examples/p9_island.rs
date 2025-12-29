//! Phase 9.5: Floating Island Test Scene
//!
//! This example renders a more complex voxel scene to test the deferred pipeline:
//! - Floating island with grass, dirt, and stone layers
//! - Glowing crystals with high emission
//! - A small tree
//!
//! Run with: `cargo run --example p9_island`
//!
//! Expected output: `screenshots/p9_island.png`
//! - Floating island with multiple colors
//! - Purple fog background
//! - Glowing cyan/magenta crystals with point light shadows

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    ground_level_offset, load_creature_script, spawn_chunk_with_lights, CameraPreset,
    DeferredCamera, DeferredRenderingPlugin, VoxelMaterialPlugin,
};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p9_island.png";
const CREATURE_SCRIPT: &str = "assets/scripts/test_island.lua";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 9.5: Floating Island Test Scene...");
    println!("Loading test script: {}", CREATURE_SCRIPT);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 9.5: Floating Island".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        // Fog color as clear color
        .insert_resource(ClearColor(Color::srgb(0.102, 0.039, 0.180)))
        .insert_resource(FrameCount(0))
        .add_systems(Startup, setup)
        .add_systems(Update, capture_and_exit)
        .run();

    if Path::new(SCREENSHOT_PATH).exists() {
        println!("SUCCESS: Screenshot saved to {}", SCREENSHOT_PATH);
    } else {
        println!("FAILED: Screenshot was not created at {}", SCREENSHOT_PATH);
        std::process::exit(1);
    }
}

#[derive(Resource)]
struct FrameCount(u32);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<studio_core::VoxelMaterial>>,
) {
    // IMPORTANT: Spawn the shadow-casting light FIRST so it gets priority
    // (shadow system uses first lights in the list)
    // Island grass is at world Y=4-11, tree canopy up to Y=16
    // Place light well above at Y=20 to ensure it's above everything
    use studio_core::DeferredPointLight;
    commands.spawn((
        DeferredPointLight {
            color: Color::srgb(1.0, 0.95, 0.8), // Warm white
            intensity: 50.0,
            radius: 35.0,
        },
        Transform::from_xyz(0.0, 20.0, 0.0), // Directly above island center, high up
    ));
    println!("Added shadow-casting point light at (0, 20, 0)");

    // Load test script
    let chunk = match load_creature_script(CREATURE_SCRIPT) {
        Ok(c) => {
            println!("Loaded island scene with {} voxels", c.count());
            c
        }
        Err(e) => {
            eprintln!("ERROR: Failed to load creature script: {:?}", e);
            std::process::exit(1);
        }
    };

    // Use scene_utils to spawn chunk with lights at correct world position
    // ground_level_offset() places chunk Y=0 at world Y=0, putting the island at proper height
    let world_offset = ground_level_offset();
    let result = spawn_chunk_with_lights(
        &mut commands,
        &mut meshes,
        &mut materials,
        &chunk,
        world_offset,
    );

    println!(
        "Spawned mesh + {} point lights from {} emissive voxels",
        result.light_entities.len(),
        result.emissive_count
    );

    // Camera looking at the island - framed to see the whole scene
    let island_center = Vec3::new(0.0, 6.0, 0.0);
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(18.0, 16.0, 18.0).looking_at(island_center, Vec3::Y),
        DeferredCamera,
    ));

    // Directional light (used by forward pass, our deferred has hardcoded sun)
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));



    println!("Island scene setup complete.");
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;

    // Give render graph time to initialize
    if frame_count.0 == 15 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }

    if frame_count.0 >= 25 {
        println!("Exiting after {} frames", frame_count.0);
        exit.write(AppExit::Success);
    }
}
