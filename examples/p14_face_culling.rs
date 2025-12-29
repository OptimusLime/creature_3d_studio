//! Phase 14: Face Culling Test
//!
//! This example demonstrates and verifies face culling optimization.
//! Creates various voxel shapes and logs the vertex reduction achieved.
//!
//! Run with: `cargo run --example p14_face_culling`
//!
//! Expected output:
//! - Renders a solid 8x8x8 cube with colored faces
//! - Logs show dramatic vertex reduction (87.5% for solid cube)
//! - Screenshot saved to `screenshots/p14_face_culling.png`

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    build_chunk_mesh, DeferredCamera, DeferredPointLight, DeferredRenderable,
    DeferredRenderingPlugin, Voxel, VoxelChunk, VoxelMaterial, VoxelMaterialPlugin,
};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p14_face_culling.png";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("===========================================");
    println!("Phase 14: Face Culling Test");
    println!("===========================================\n");

    // Run statistics tests before rendering
    print_face_culling_stats();

    println!("\n===========================================");
    println!("Launching visual test...");
    println!("===========================================\n");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 14: Face Culling Test".into(),
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

/// Print face culling statistics for various test shapes
fn print_face_culling_stats() {
    println!("Face Culling Statistics");
    println!("-----------------------\n");

    // Test 1: Single voxel (no culling possible)
    {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));
        let mesh = build_chunk_mesh(&chunk);
        let vertices = mesh.count_vertices();
        let theoretical_max = 1 * 6 * 4; // 1 voxel * 6 faces * 4 vertices
        println!(
            "Single voxel:     {:4} vertices (max: {:4}, reduction: {:5.1}%)",
            vertices,
            theoretical_max,
            (1.0 - vertices as f64 / theoretical_max as f64) * 100.0
        );
    }

    // Test 2: 2x2x2 cube
    {
        let mut chunk = VoxelChunk::new();
        for x in 8..10 {
            for y in 8..10 {
                for z in 8..10 {
                    chunk.set(x, y, z, Voxel::solid(255, 128, 0));
                }
            }
        }
        let mesh = build_chunk_mesh(&chunk);
        let vertices = mesh.count_vertices();
        let theoretical_max = 8 * 6 * 4; // 8 voxels * 6 faces * 4 vertices
        println!(
            "2x2x2 cube:       {:4} vertices (max: {:4}, reduction: {:5.1}%)",
            vertices,
            theoretical_max,
            (1.0 - vertices as f64 / theoretical_max as f64) * 100.0
        );
    }

    // Test 3: 4x4x4 cube
    {
        let mut chunk = VoxelChunk::new();
        for x in 8..12 {
            for y in 8..12 {
                for z in 8..12 {
                    chunk.set(x, y, z, Voxel::solid(0, 255, 128));
                }
            }
        }
        let mesh = build_chunk_mesh(&chunk);
        let vertices = mesh.count_vertices();
        let theoretical_max = 64 * 6 * 4; // 64 voxels * 6 faces * 4 vertices
        println!(
            "4x4x4 cube:       {:4} vertices (max: {:4}, reduction: {:5.1}%)",
            vertices,
            theoretical_max,
            (1.0 - vertices as f64 / theoretical_max as f64) * 100.0
        );
    }

    // Test 4: 8x8x8 cube (512 voxels)
    {
        let mut chunk = VoxelChunk::new();
        for x in 8..16 {
            for y in 8..16 {
                for z in 8..16 {
                    chunk.set(x, y, z, Voxel::solid(128, 0, 255));
                }
            }
        }
        let mesh = build_chunk_mesh(&chunk);
        let vertices = mesh.count_vertices();
        let theoretical_max = 512 * 6 * 4; // 512 voxels * 6 faces * 4 vertices
        let expected_faces = 6 * 64; // 6 sides * 8x8 surface faces
        let expected_vertices = expected_faces * 4;
        println!(
            "8x8x8 cube:       {:4} vertices (max: {:4}, reduction: {:5.1}%)",
            vertices,
            theoretical_max,
            (1.0 - vertices as f64 / theoretical_max as f64) * 100.0
        );
        println!(
            "                  Expected: {} vertices ({} surface faces)",
            expected_vertices, expected_faces
        );
    }

    // Test 5: 16x16x16 cube (4096 voxels)
    {
        let mut chunk = VoxelChunk::new();
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    chunk.set(x, y, z, Voxel::solid(255, 255, 0));
                }
            }
        }
        let mesh = build_chunk_mesh(&chunk);
        let vertices = mesh.count_vertices();
        let theoretical_max = 4096 * 6 * 4; // 4096 voxels * 6 faces * 4 vertices
        let expected_faces = 6 * 256; // 6 sides * 16x16 surface faces
        let expected_vertices = expected_faces * 4;
        println!(
            "16x16x16 cube:    {:4} vertices (max: {:5}, reduction: {:5.1}%)",
            vertices,
            theoretical_max,
            (1.0 - vertices as f64 / theoretical_max as f64) * 100.0
        );
        println!(
            "                  Expected: {} vertices ({} surface faces)",
            expected_vertices, expected_faces
        );
    }

    // Test 6: Hollow 8x8x8 shell (only surface voxels)
    {
        let mut chunk = VoxelChunk::new();
        for x in 8..16 {
            for y in 8..16 {
                for z in 8..16 {
                    // Only place voxels on the surface
                    let on_surface = x == 8 || x == 15 || y == 8 || y == 15 || z == 8 || z == 15;
                    if on_surface {
                        chunk.set(x, y, z, Voxel::solid(0, 255, 255));
                    }
                }
            }
        }
        let voxel_count = chunk.count();
        let mesh = build_chunk_mesh(&chunk);
        let vertices = mesh.count_vertices();
        let theoretical_max = voxel_count * 6 * 4;
        println!(
            "Hollow 8x8x8:     {:4} vertices (max: {:5}, reduction: {:5.1}%)",
            vertices,
            theoretical_max,
            (1.0 - vertices as f64 / theoretical_max as f64) * 100.0
        );
        println!("                  ({} surface voxels)", voxel_count);
    }
}

#[derive(Resource)]
struct FrameCount(u32);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    // Create a solid 8x8x8 cube with colored gradient
    let mut chunk = VoxelChunk::new();
    for x in 8..16 {
        for y in 8..16 {
            for z in 8..16 {
                // Color gradient based on position (avoid overflow)
                let r = ((x - 8) * 28 + 50) as u8;
                let g = ((y - 8) * 28 + 50) as u8;
                let b = ((z - 8) * 28 + 50) as u8;
                chunk.set(x, y, z, Voxel::solid(r, g, b));
            }
        }
    }

    let mesh = build_chunk_mesh(&chunk);

    // Log mesh statistics
    let vertices = mesh.count_vertices();
    let indices = mesh.indices().map(|i| i.len()).unwrap_or(0);
    let faces = vertices / 4;

    println!("Visual test mesh statistics:");
    println!("  Voxels:   512 (8x8x8 cube)");
    println!("  Vertices: {} (would be {} without culling)", vertices, 512 * 24);
    println!("  Indices:  {} (would be {} without culling)", indices, 512 * 36);
    println!("  Faces:    {} (would be {} without culling)", faces, 512 * 6);
    println!(
        "  Reduction: {:.1}%",
        (1.0 - vertices as f64 / (512.0 * 24.0)) * 100.0
    );

    let mesh_handle = meshes.add(mesh);
    let material = materials.add(VoxelMaterial::default());

    // Spawn the cube
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 0.0, 0.0),
        DeferredRenderable,
    ));

    // Camera - isometric view
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(12.0, 10.0, 12.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        DeferredCamera,
    ));

    // Point light for deferred rendering
    commands.spawn((
        DeferredPointLight {
            color: Color::srgb(1.0, 0.9, 0.8),
            intensity: 30.0,
            radius: 25.0,
        },
        Transform::from_xyz(8.0, 8.0, 8.0),
    ));

    println!("\nScene setup complete. Rendering...");
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;

    if frame_count.0 == 10 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }

    if frame_count.0 >= 20 {
        exit.write(AppExit::Success);
    }
}
