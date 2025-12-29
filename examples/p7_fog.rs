//! Phase 7 Screenshot Test: Distance Fog.
//!
//! This test verifies:
//! - Bonsai-style additive fog implemented in voxel.wgsl
//! - Near voxels have full color (minimal fog)
//! - Far voxels fade toward purple fog color
//! - Fog is additive (lightens dark areas) not blend
//!
//! Implementation notes:
//! - Fog ported from Bonsai Lighting.fragmentshader:306-319
//! - Formula: FogContrib = clamp(dist/max, 0, 1)^2 * 1.2
//! - Additive: final = emissive_color + fog (not mix)
//! - FOG_MAX_DISTANCE = 50.0, FOG_COLOR = deep purple (#1a0a2e)
//!
//! Run with: `cargo run --example p7_fog`
//!
//! Expected output: `screenshots/p7_fog.png`
//! - 4 white voxels at varying depths (z = 0, -10, -25, -45)
//! - Near voxel (z=0): full white, no fog
//! - Far voxel (z=-45): heavily tinted purple from fog
//! - Clear depth gradient visible
//! - Background is fog color (#1a0a2e deep purple) - objects fade INTO the fog

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::render::view::Hdr;
use std::path::Path;
use studio_core::{build_chunk_mesh, load_creature_script, VoxelMaterial, VoxelMaterialPlugin};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p7_fog.png";
const CREATURE_SCRIPT: &str = "assets/scripts/test_fog.lua";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 7 Screenshot Test: Distance Fog...");
    println!("Loading fog test script: {}", CREATURE_SCRIPT);
    println!(
        "Expected output: {} (4 white voxels with distance fog gradient)",
        SCREENSHOT_PATH
    );

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 7: Distance Fog".into(),
                ..default()
            }),
            ..default()
        }))
        // Register our custom VoxelMaterial
        .add_plugins(VoxelMaterialPlugin)
        // Fog color as clear color - objects fade INTO the fog, not into black
        // #1a0a2e = RGB(26, 10, 46) = (0.102, 0.039, 0.180)
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
    // Load the fog test script (single voxel at center)
    let chunk = match load_creature_script(CREATURE_SCRIPT) {
        Ok(c) => {
            println!("Loaded fog test with {} voxels", c.count());
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

    // Spawn voxels in a diagonal line - offset in X so they don't overlap visually
    // FOG_MAX_DISTANCE in shader is 50.0
    // Positions: (x, y, z) - spread them out so each cube is clearly visible
    let positions = [
        (-3.0, 0.0, 2.0),   // Near - bottom left, closest
        (-1.0, 0.0, 10.0),  // Mid-near
        (1.0, 0.0, 25.0),   // Mid-far  
        (3.0, 0.0, 45.0),   // Far - top right, farthest (should be very foggy)
    ];
    
    for (i, (x, y, z)) in positions.iter().enumerate() {
        commands.spawn((
            Mesh3d(mesh_handle.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(*x, *y, *z),
        ));
        println!("Spawned voxel {} at ({}, {}, {}) - {} vertices", i, x, y, z, vertex_count);
    }

    println!("Spawned {} mesh entities total", positions.len());

    // Camera positioned close and angled to see all cubes clearly
    // Elevated and to the side so cubes don't overlap
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(-6.0, 4.0, -5.0).looking_at(Vec3::new(1.0, 0.0, 25.0), Vec3::Y),
        // Keep bloom for consistency, but low intensity since we're testing fog
        Bloom {
            intensity: 0.15,
            low_frequency_boost: 0.5,
            low_frequency_boost_curvature: 0.95,
            high_pass_frequency: 1.0,
            prefilter: BloomPrefilter {
                threshold: 1.0,
                threshold_softness: 0.5,
            },
            composite_mode: BloomCompositeMode::Additive,
            ..default()
        },
    ));

    // Directional light
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
