//! Phase 18: Cross-Chunk Face Culling Test
//!
//! This example demonstrates cross-chunk face culling:
//! - Voxels at chunk boundaries have shared faces culled
//! - Compares vertex counts with and without cross-chunk culling
//! - Verifies no visual seams at chunk boundaries
//!
//! Run with: `cargo run --example p18_cross_chunk_culling`
//!
//! Expected output: `screenshots/p18_cross_chunk_culling.png`
//! - A solid wall spanning two chunks with NO visible seam
//! - Statistics showing vertex reduction from cross-chunk culling

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    build_world_meshes_cross_chunk_with_options, build_world_meshes_with_options,
    spawn_world_with_lights, CameraPreset, DeferredCamera, DeferredRenderingPlugin, Voxel,
    VoxelMaterialPlugin, VoxelWorld,
};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p18_cross_chunk_culling.png";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 18: Cross-Chunk Face Culling Test...");
    println!("===================================================\n");

    // First, demonstrate the vertex savings with statistics
    demonstrate_culling_stats();

    println!("\n--- Visual Test ---\n");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1024, 768).into(),
                title: "Phase 18: Cross-Chunk Face Culling".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .insert_resource(FrameCount(0))
        .add_systems(Startup, setup)
        .add_systems(Update, capture_and_exit)
        .run();

    if Path::new(SCREENSHOT_PATH).exists() {
        println!("\nSUCCESS: Screenshot saved to {}", SCREENSHOT_PATH);
    } else {
        println!("\nFAILED: Screenshot was not created at {}", SCREENSHOT_PATH);
        std::process::exit(1);
    }
}

/// Demonstrate vertex savings from cross-chunk culling with statistics.
fn demonstrate_culling_stats() {
    // Test 1: Two adjacent voxels across chunk boundary
    {
        let mut world = VoxelWorld::new();
        world.set_voxel(31, 16, 16, Voxel::solid(255, 0, 0)); // Edge of chunk 0
        world.set_voxel(32, 16, 16, Voxel::solid(0, 255, 0)); // Edge of chunk 1

        let meshes_without = build_world_meshes_with_options(&world, false);
        let meshes_with = build_world_meshes_cross_chunk_with_options(&world, false);

        let verts_without: usize = meshes_without
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();
        let verts_with: usize = meshes_with
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        println!("Test 1: Two Adjacent Voxels Across Chunk Boundary");
        println!("  Without cross-chunk culling: {} vertices", verts_without);
        println!("  With cross-chunk culling:    {} vertices", verts_with);
        println!(
            "  Reduction: {} vertices ({:.1}%)\n",
            verts_without - verts_with,
            (1.0 - verts_with as f32 / verts_without as f32) * 100.0
        );
    }

    // Test 2: 8x8 wall at chunk boundary (2 voxels thick)
    {
        let mut world = VoxelWorld::new();
        for y in 12..20 {
            for z in 12..20 {
                world.set_voxel(31, y, z, Voxel::solid(200, 100, 50));
                world.set_voxel(32, y, z, Voxel::solid(200, 100, 50));
            }
        }

        let meshes_without = build_world_meshes_with_options(&world, false);
        let meshes_with = build_world_meshes_cross_chunk_with_options(&world, false);

        let verts_without: usize = meshes_without
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();
        let verts_with: usize = meshes_with
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        println!("Test 2: 8x8 Wall at Chunk Boundary (2 voxels thick, 128 voxels total)");
        println!("  Without cross-chunk culling: {} vertices", verts_without);
        println!("  With cross-chunk culling:    {} vertices", verts_with);
        println!(
            "  Reduction: {} vertices ({:.1}%)\n",
            verts_without - verts_with,
            (1.0 - verts_with as f32 / verts_without as f32) * 100.0
        );
    }

    // Test 3: 4x4x4 cube spanning two chunks
    {
        let mut world = VoxelWorld::new();
        for x in 30..34 {
            for y in 14..18 {
                for z in 14..18 {
                    world.set_voxel(x, y, z, Voxel::solid(100, 150, 200));
                }
            }
        }

        let meshes_without = build_world_meshes_with_options(&world, false);
        let meshes_with = build_world_meshes_cross_chunk_with_options(&world, false);

        let verts_without: usize = meshes_without
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();
        let verts_with: usize = meshes_with
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        println!("Test 3: 4x4x4 Cube Spanning Two Chunks (64 voxels)");
        println!("  Without cross-chunk culling: {} vertices", verts_without);
        println!("  With cross-chunk culling:    {} vertices", verts_with);
        println!(
            "  Reduction: {} vertices ({:.1}%)\n",
            verts_without - verts_with,
            (1.0 - verts_with as f32 / verts_without as f32) * 100.0
        );
    }

    // Test 4: Same tests with greedy meshing
    {
        let mut world = VoxelWorld::new();
        for x in 30..34 {
            for y in 14..18 {
                for z in 14..18 {
                    world.set_voxel(x, y, z, Voxel::solid(100, 150, 200));
                }
            }
        }

        let meshes_without = build_world_meshes_with_options(&world, true);
        let meshes_with = build_world_meshes_cross_chunk_with_options(&world, true);

        let verts_without: usize = meshes_without
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();
        let verts_with: usize = meshes_with
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        println!("Test 4: 4x4x4 Cube WITH Greedy Meshing");
        println!("  Without cross-chunk culling: {} vertices", verts_without);
        println!("  With cross-chunk culling:    {} vertices", verts_with);
        println!(
            "  Reduction: {} vertices ({:.1}%)",
            verts_without - verts_with,
            if verts_without > 0 {
                (1.0 - verts_with as f32 / verts_without as f32) * 100.0
            } else {
                0.0
            }
        );
    }
}

#[derive(Resource)]
struct FrameCount(u32);

/// Create a test world with structures spanning chunk boundaries.
fn create_test_world() -> VoxelWorld {
    let mut world = VoxelWorld::new();

    // Large wall at X chunk boundary (demonstrating seamless culling)
    println!("Creating 16x8 wall spanning chunks at X=31/32...");
    for y in 4..20 {
        for z in 8..24 {
            world.set_voxel(31, y, z, Voxel::solid(180, 80, 60)); // Orange/red brick
            world.set_voxel(32, y, z, Voxel::solid(180, 80, 60));
        }
    }

    // Floor spanning chunks (both X and Z boundaries)
    println!("Creating floor spanning X and Z chunk boundaries...");
    for x in 24..40 {
        for z in 24..40 {
            world.set_voxel(x, 3, z, Voxel::solid(80, 80, 90)); // Gray stone
        }
    }

    // Glowing pillar at corner of 4 chunks
    println!("Creating glowing pillar at (32, 4, 32)...");
    for y in 4..12 {
        world.set_voxel(31, y, 31, Voxel::new(255, 200, 100, 200));
        world.set_voxel(32, y, 31, Voxel::new(255, 200, 100, 200));
        world.set_voxel(31, y, 32, Voxel::new(255, 200, 100, 200));
        world.set_voxel(32, y, 32, Voxel::new(255, 200, 100, 200));
    }

    // Bridge across Z chunk boundary
    println!("Creating bridge across Z chunk boundary...");
    for x in 16..28 {
        world.set_voxel(x, 8, 31, Voxel::solid(100, 140, 180)); // Blue-gray
        world.set_voxel(x, 8, 32, Voxel::solid(100, 140, 180));
    }

    world
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<studio_core::VoxelMaterial>>,
) {
    let world = create_test_world();

    println!(
        "World: {} chunks, {} voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );

    // Spawn with cross-chunk culling (default in WorldSpawnConfig)
    let result = spawn_world_with_lights(&mut commands, &mut meshes, &mut materials, &world);

    println!(
        "Spawned {} chunks + {} lights",
        result.chunk_entities.len(),
        result.light_entities.len()
    );

    // Camera to view the chunk boundary
    let camera = CameraPreset::isometric(Vec3::new(32.0, 10.0, 32.0), 50.0);
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_translation(camera.position).looking_at(camera.look_at, Vec3::Y),
        DeferredCamera,
    ));

    // Directional light
    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 15.0).looking_at(Vec3::new(32.0, 8.0, 32.0), Vec3::Y),
    ));

    println!("Scene setup complete.\n");
    println!("Look for:");
    println!("  - Seamless wall at X chunk boundary (no visible seam)");
    println!("  - Floor spanning multiple chunk boundaries");
    println!("  - Glowing pillar at the corner of 4 chunks");
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;

    if frame_count.0 == 15 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }

    if frame_count.0 >= 25 {
        exit.write(AppExit::Success);
    }
}
