//! Phase 5 Screenshot Test: Emission affects brightness.
//!
//! This test verifies:
//! - Emission vertex attribute is read by shader
//! - Higher emission = brighter output
//! - Emission multiplier works (emission=1.0 triples brightness)
//!
//! Run with: `cargo run --example p5_emission`
//!
//! Expected output: `screenshots/p5_emission.png`
//! - 4 white voxels in a row
//! - Clear brightness gradient: leftmost darkest, rightmost brightest
//! - Brightest voxel should NOT clip to pure white (still visible shading)

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::render::view::Hdr;
use std::path::Path;
use studio_core::{build_chunk_mesh, load_creature_script, VoxelMaterial, VoxelMaterialPlugin};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p5_emission.png";
const CREATURE_SCRIPT: &str = "assets/scripts/test_emission.lua";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 5 Screenshot Test: Emission...");
    println!("Loading emission test script: {}", CREATURE_SCRIPT);
    println!(
        "Expected output: {} (4 white voxels with brightness gradient)",
        SCREENSHOT_PATH
    );

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 5: Emission".into(),
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
    // Load the emission test script
    let chunk = match load_creature_script(CREATURE_SCRIPT) {
        Ok(c) => {
            println!("Loaded emission test with {} voxels", c.count());
            c
        }
        Err(e) => {
            eprintln!("ERROR: Failed to load creature script: {:?}", e);
            std::process::exit(1);
        }
    };

    // Build mesh from chunk
    let mesh = build_chunk_mesh(&chunk);
    let vertex_count = mesh.count_vertices();
    let mesh_handle = meshes.add(mesh);

    // Create VoxelMaterial
    let material = materials.add(VoxelMaterial::default());

    // Spawn mesh entity
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::IDENTITY,
    ));

    println!(
        "Spawned 1 mesh entity with {} vertices",
        vertex_count
    );

    // Camera positioned to see the row of voxels from front
    // Voxels are at x=5,7,9,11 (centered at x=8), y=8, z=8
    // After chunk centering: x=-3,-1,1,3, y=0, z=0
    //
    // HDR enabled to prevent clipping on high-emission voxels.
    // Without HDR, emission boost pushes white (1,1,1) * 3.0 = (3,3,3) which clamps to (1,1,1).
    // With HDR, values > 1.0 are preserved and tonemapping compresses them properly.
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Transform::from_xyz(0.0, 2.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Directional light (same as previous phases)
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
