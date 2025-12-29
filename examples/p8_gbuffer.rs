//! Phase 8 Screenshot Test: Deferred Rendering Pipeline.
//!
//! This test verifies:
//! - DeferredRenderingPlugin initializes with custom render graph nodes
//! - G-Buffer pass renders to MRT (color, normal, position)
//! - Lighting pass reads G-buffer and outputs to view target
//!
//! This is a FULL CUSTOM RENDER GRAPH implementation.
//!
//! Run with: `cargo run --example p8_gbuffer`
//!
//! Expected output: `screenshots/p8_gbuffer.png`
//! - Scene rendered through deferred pipeline
//! - Deep purple background from fog color
//! - Voxels rendered (initially just fog since G-buffer is cleared)

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    build_chunk_mesh, load_creature_script, DeferredCamera, DeferredRenderingPlugin,
    VoxelMaterial, VoxelMaterialPlugin,
};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p8_gbuffer.png";
const CREATURE_SCRIPT: &str = "assets/scripts/test_emission.lua";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 8 Screenshot Test: Deferred Rendering Pipeline...");
    println!("Loading test script: {}", CREATURE_SCRIPT);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 8: Deferred Rendering Pipeline".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        // Fog color as clear color (backup for non-deferred cameras)
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
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    // Load test script
    let chunk = match load_creature_script(CREATURE_SCRIPT) {
        Ok(c) => {
            println!("Loaded test scene with {} voxels", c.count());
            c
        }
        Err(e) => {
            eprintln!("ERROR: Failed to load creature script: {:?}", e);
            std::process::exit(1);
        }
    };

    let mesh = build_chunk_mesh(&chunk);
    let mesh_handle = meshes.add(mesh);
    let material = materials.add(VoxelMaterial::default());

    // Spawn voxels
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Camera with DeferredCamera marker - uses our custom render graph
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        DeferredCamera, // Mark this camera for our deferred rendering
    ));

    // Directional light (not used by our deferred lighting yet, but here for reference)
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: false, // Shadows not implemented in deferred yet
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    println!("Scene setup complete with DeferredCamera.");
    println!("Deferred pipeline nodes: GBufferPass -> LightingPass");
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
