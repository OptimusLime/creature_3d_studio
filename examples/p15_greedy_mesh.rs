//! Phase 15: Greedy Meshing Test
//!
//! This example demonstrates and verifies greedy meshing optimization.
//! Compares face culling vs greedy meshing for various voxel shapes.
//!
//! Run with: `cargo run --example p15_greedy_mesh`
//!
//! Expected output:
//! - Renders a solid cube using greedy meshing (only 6 quads!)
//! - Logs show dramatic vertex reduction compared to face culling
//! - Screenshot saved to `screenshots/p15_greedy_mesh.png`

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    build_chunk_mesh, build_chunk_mesh_greedy, DeferredCamera, DeferredPointLight,
    DeferredRenderable, DeferredRenderingPlugin, Voxel, VoxelChunk, VoxelMaterial,
    VoxelMaterialPlugin,
};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p15_greedy_mesh.png";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("===========================================");
    println!("Phase 15: Greedy Meshing Test");
    println!("===========================================\n");

    // Run statistics tests before rendering
    print_greedy_mesh_stats();

    println!("\n===========================================");
    println!("Launching visual test...");
    println!("===========================================\n");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 15: Greedy Meshing Test".into(),
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

/// Print greedy meshing statistics for various test shapes
fn print_greedy_mesh_stats() {
    println!("Greedy Meshing Statistics");
    println!("-------------------------\n");
    println!("{:<20} {:>12} {:>12} {:>12} {:>10}", "Shape", "Culled", "Greedy", "Reduction", "Factor");
    println!("{:-<70}", "");

    // Test 1: Single voxel (no improvement)
    {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));
        
        let culled = build_chunk_mesh(&chunk).count_vertices();
        let greedy = build_chunk_mesh_greedy(&chunk).count_vertices();
        let reduction = (1.0 - greedy as f64 / culled as f64) * 100.0;
        let factor = culled as f64 / greedy as f64;
        
        println!("{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x", 
            "Single voxel", culled, greedy, reduction, factor);
    }

    // Test 2: 2x2x2 same color
    {
        let mut chunk = VoxelChunk::new();
        for x in 8..10 {
            for y in 8..10 {
                for z in 8..10 {
                    chunk.set(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }
        
        let culled = build_chunk_mesh(&chunk).count_vertices();
        let greedy = build_chunk_mesh_greedy(&chunk).count_vertices();
        let reduction = (1.0 - greedy as f64 / culled as f64) * 100.0;
        let factor = culled as f64 / greedy as f64;
        
        println!("{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x", 
            "2x2x2 uniform", culled, greedy, reduction, factor);
    }

    // Test 3: 4x4x4 same color
    {
        let mut chunk = VoxelChunk::new();
        for x in 8..12 {
            for y in 8..12 {
                for z in 8..12 {
                    chunk.set(x, y, z, Voxel::solid(0, 255, 0));
                }
            }
        }
        
        let culled = build_chunk_mesh(&chunk).count_vertices();
        let greedy = build_chunk_mesh_greedy(&chunk).count_vertices();
        let reduction = (1.0 - greedy as f64 / culled as f64) * 100.0;
        let factor = culled as f64 / greedy as f64;
        
        println!("{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x", 
            "4x4x4 uniform", culled, greedy, reduction, factor);
    }

    // Test 4: 8x8x8 same color
    {
        let mut chunk = VoxelChunk::new();
        for x in 8..16 {
            for y in 8..16 {
                for z in 8..16 {
                    chunk.set(x, y, z, Voxel::solid(0, 0, 255));
                }
            }
        }
        
        let culled = build_chunk_mesh(&chunk).count_vertices();
        let greedy = build_chunk_mesh_greedy(&chunk).count_vertices();
        let reduction = (1.0 - greedy as f64 / culled as f64) * 100.0;
        let factor = culled as f64 / greedy as f64;
        
        println!("{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x", 
            "8x8x8 uniform", culled, greedy, reduction, factor);
    }

    // Test 5: 16x16 flat layer
    {
        let mut chunk = VoxelChunk::new();
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 8, z, Voxel::solid(200, 200, 200));
            }
        }
        
        let culled = build_chunk_mesh(&chunk).count_vertices();
        let greedy = build_chunk_mesh_greedy(&chunk).count_vertices();
        let reduction = (1.0 - greedy as f64 / culled as f64) * 100.0;
        let factor = culled as f64 / greedy as f64;
        
        println!("{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x", 
            "16x16 flat layer", culled, greedy, reduction, factor);
    }

    // Test 6: Checkerboard (no improvement)
    {
        let mut chunk = VoxelChunk::new();
        let colors = [Voxel::solid(255, 0, 0), Voxel::solid(0, 255, 0)];
        for x in 8..12 {
            for y in 8..12 {
                for z in 8..12 {
                    chunk.set(x, y, z, colors[(x + y + z) % 2]);
                }
            }
        }
        
        let culled = build_chunk_mesh(&chunk).count_vertices();
        let greedy = build_chunk_mesh_greedy(&chunk).count_vertices();
        let reduction = (1.0 - greedy as f64 / culled as f64) * 100.0;
        let factor = culled as f64 / greedy as f64;
        
        println!("{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x", 
            "4x4x4 checkerboard", culled, greedy, reduction, factor);
    }

    // Test 7: Striped layers (partial improvement)
    {
        let mut chunk = VoxelChunk::new();
        for x in 8..16 {
            for y in 8..12 {
                for z in 8..16 {
                    // Different color per Y layer
                    let color = match y {
                        8 => Voxel::solid(255, 0, 0),
                        9 => Voxel::solid(0, 255, 0),
                        10 => Voxel::solid(0, 0, 255),
                        _ => Voxel::solid(255, 255, 0),
                    };
                    chunk.set(x, y, z, color);
                }
            }
        }
        
        let culled = build_chunk_mesh(&chunk).count_vertices();
        let greedy = build_chunk_mesh_greedy(&chunk).count_vertices();
        let reduction = (1.0 - greedy as f64 / culled as f64) * 100.0;
        let factor = culled as f64 / greedy as f64;
        
        println!("{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x", 
            "8x4x8 striped", culled, greedy, reduction, factor);
    }

    println!("{:-<70}", "");
    println!("\nKey insights:");
    println!("- Uniform surfaces: massive improvement (up to 64x for 8x8 faces)");
    println!("- Checkerboard: no improvement (colors prevent merging)");
    println!("- Striped: partial improvement (layers merge, boundaries don't)");
}

#[derive(Resource)]
struct FrameCount(u32);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    // Create a solid 8x8x8 cube - perfect case for greedy meshing
    let mut chunk = VoxelChunk::new();
    for x in 8..16 {
        for y in 8..16 {
            for z in 8..16 {
                // Single color to maximize greedy merging
                chunk.set(x, y, z, Voxel::solid(100, 150, 200));
            }
        }
    }

    // Build with greedy meshing
    let mesh = build_chunk_mesh_greedy(&chunk);

    // Log mesh statistics
    let vertices = mesh.count_vertices();
    let indices = mesh.indices().map(|i| i.len()).unwrap_or(0);
    let quads = vertices / 4;

    // Compare with face-culling only
    let culled_mesh = build_chunk_mesh(&chunk);
    let culled_vertices = culled_mesh.count_vertices();
    let culled_quads = culled_vertices / 4;

    println!("Visual test mesh statistics:");
    println!("  Voxels:   512 (8x8x8 cube, uniform color)");
    println!("  --- Face Culling Only ---");
    println!("  Vertices: {} ({} quads)", culled_vertices, culled_quads);
    println!("  --- Greedy Meshing ---");
    println!("  Vertices: {} ({} quads)", vertices, quads);
    println!("  Indices:  {}", indices);
    println!(
        "  Improvement: {:.1}x fewer vertices",
        culled_vertices as f64 / vertices as f64
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

    // Point light
    commands.spawn((
        DeferredPointLight {
            color: Color::srgb(1.0, 0.95, 0.9),
            intensity: 40.0,
            radius: 30.0,
        },
        Transform::from_xyz(10.0, 10.0, 10.0),
    ));

    // Camera - isometric view
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(12.0, 10.0, 12.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        DeferredCamera,
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
