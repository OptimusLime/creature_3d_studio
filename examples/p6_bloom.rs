//! Phase 6 Screenshot Test: Bloom Post-Processing.
//!
//! This test verifies:
//! - Bevy's built-in Bloom component works with our custom VoxelMaterial
//! - High-emission voxels have visible glow halos
//! - Lower emission voxels have less or no bloom
//! - Background remains black (bloom doesn't affect non-emissive areas)
//!
//! Implementation notes:
//! - Bevy's Bloom uses a COD-style mip-chain blur, similar to Bonsai's approach
//! - The bloom algorithm: extract bright pixels → downsample chain → upsample chain → composite
//! - This is the same technique described in Bonsai's bloom_downsample.fragmentshader
//!   and bloom_upsample.fragmentshader (13-tap downsample, 9-tap tent upsample)
//!
//! Run with: `cargo run --example p6_bloom`
//!
//! Expected output: `screenshots/p6_bloom.png`
//! - 4 white voxels in a row (same as p5)
//! - Highest emission voxel has visible glow halo extending beyond cube edges
//! - Lower emission voxels have progressively less bloom
//! - Bloom is soft/blurred, not sharp edges
//! - Background remains black

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::render::view::Hdr;
use std::path::Path;
use studio_core::{build_chunk_mesh, load_creature_script, VoxelMaterial, VoxelMaterialPlugin};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p6_bloom.png";
const CREATURE_SCRIPT: &str = "assets/scripts/test_emission.lua";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 6 Screenshot Test: Bloom...");
    println!("Loading emission test script: {}", CREATURE_SCRIPT);
    println!(
        "Expected output: {} (4 white voxels with bloom halos on bright ones)",
        SCREENSHOT_PATH
    );

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 6: Bloom".into(),
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
    // Load the emission test script (same as p5)
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

    println!("Spawned 1 mesh entity with {} vertices", vertex_count);

    // Camera with HDR + Bloom
    //
    // Bloom settings tuned for our dark fantasy aesthetic:
    // - intensity: 0.5 for noticeable but not overwhelming bloom
    // - low_frequency_boost: 0.7 to emphasize larger glow halos
    // - high_pass_frequency: 1.0 to only bloom the brightest areas
    // - prefilter threshold: 1.0 means only HDR values > 1.0 bloom
    // - composite_mode: Additive matches Bonsai's approach (adds 5% bloom)
    //
    // Bonsai reference (composite.fragmentshader:209):
    //   if (UseLightingBloom) { TotalLight += 0.05f*Bloom; }
    commands.spawn((
        Camera3d::default(),
        Hdr,
        // Use TonyMcMapface tonemapping (good for HDR bloom)
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(0.0, 2.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        Bloom {
            intensity: 0.3,
            low_frequency_boost: 0.7,
            low_frequency_boost_curvature: 0.95,
            high_pass_frequency: 1.0,
            prefilter: BloomPrefilter {
                // Only bloom pixels with brightness > 1.0 (HDR values)
                threshold: 1.0,
                threshold_softness: 0.5,
            },
            composite_mode: BloomCompositeMode::Additive,
            ..default()
        },
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
