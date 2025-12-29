//! Phase 4 Screenshot Test: Single merged mesh from voxel chunk.
//!
//! This test verifies:
//! - build_chunk_mesh() generates correct geometry
//! - Per-vertex colors render correctly
//! - Single mesh entity instead of per-voxel entities
//!
//! Run with: `cargo run --example p4_custom_mesh`
//!
//! Expected output: `screenshots/p4_custom_mesh.png` - identical to p3_lua_voxels.png
//! but with 1 mesh entity instead of 5 cube entities.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{build_chunk_mesh, load_creature_script, VoxelMaterial, VoxelMaterialPlugin};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p4_custom_mesh.png";
const CREATURE_SCRIPT: &str = "assets/scripts/test_creature.lua";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 4 Screenshot Test: Custom Mesh...");
    println!("Loading creature script: {}", CREATURE_SCRIPT);
    println!(
        "Expected output: {} (same as p3, but 1 mesh entity)",
        SCREENSHOT_PATH
    );

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 4: Custom Mesh".into(),
                ..default()
            }),
            ..default()
        }))
        // Register our custom VoxelMaterial
        .add_plugins(VoxelMaterialPlugin)
        // Black clear color (void)
        .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
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
    // Load the creature script
    let chunk = match load_creature_script(CREATURE_SCRIPT) {
        Ok(c) => {
            println!("Loaded creature with {} voxels", c.count());
            c
        }
        Err(e) => {
            eprintln!("ERROR: Failed to load creature script: {:?}", e);
            std::process::exit(1);
        }
    };

    // Build a single merged mesh from the chunk
    let mesh = build_chunk_mesh(&chunk);
    let vertex_count = mesh.count_vertices();
    let mesh_handle = meshes.add(mesh);

    // Create our custom VoxelMaterial
    let material = materials.add(VoxelMaterial::default());

    // Spawn SINGLE mesh entity (key difference from Phase 3)
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::IDENTITY,
    ));

    println!(
        "Spawned 1 mesh entity with {} vertices (5 voxels * 24 vertices/voxel = 120)",
        vertex_count
    );

    // Camera positioned to see the cross pattern from above-front-right
    // Same position as Phase 3 for comparison
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(5.0, 5.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Directional light (same as Phase 3)
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;

    if frame_count.0 == 5 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }

    if frame_count.0 >= 15 {
        println!("Exiting after {} frames", frame_count.0);
        exit.write(AppExit::Success);
    }
}
