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
//! - Glowing cyan/magenta crystals

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    build_chunk_mesh, load_creature_script, DeferredCamera, DeferredRenderable,
    DeferredRenderingPlugin, VoxelMaterial, VoxelMaterialPlugin,
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
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
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

    let mesh = build_chunk_mesh(&chunk);
    
    // Log mesh statistics
    if let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        println!("Mesh vertices: {}", positions.len());
    }
    if let Some(indices) = mesh.indices() {
        println!("Mesh indices: {}", indices.len());
    }
    
    let mesh_handle = meshes.add(mesh);
    let material = materials.add(VoxelMaterial::default());

    // Spawn island
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 0.0, 0.0),
        DeferredRenderable,
    ));

    // Camera positioned to see the island from an angle
    // Pulled back and elevated to see the whole scene
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(15.0, 12.0, 15.0).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
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
