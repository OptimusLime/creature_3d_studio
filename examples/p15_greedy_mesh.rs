//! Phase 15: Greedy Meshing Test
//!
//! Demonstrates greedy meshing optimization with statistics.
//!
//! Run with: `cargo run --example p15_greedy_mesh`
//!
//! Expected output: `screenshots/p15_greedy_mesh.png`

use bevy::prelude::*;
use studio_core::{build_chunk_mesh, build_chunk_mesh_greedy, Voxel, VoxelChunk, VoxelWorldApp};

fn main() {
    println!("===========================================");
    println!("Phase 15: Greedy Meshing Test");
    println!("===========================================\n");

    // Print statistics before rendering
    print_greedy_mesh_stats();

    println!("\n===========================================");
    println!("Launching visual test...");
    println!("===========================================\n");

    VoxelWorldApp::new("Phase 15: Greedy Meshing Test")
        .with_world_file("assets/worlds/mesh_test.voxworld")
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_shadow_light(Vec3::new(15.0, 15.0, 15.0)) // Light outside the 8x8x8 cube
        .with_clear_color(Color::srgb(0.05, 0.05, 0.1))
        .with_camera_angle(45.0, 30.0) // Auto-frame to voxel bounds
        .with_screenshot_timed("screenshots/p15_greedy_mesh.png", 10, 20)
        .run();
}

fn print_greedy_mesh_stats() {
    println!("Greedy Meshing Statistics");
    println!("-------------------------\n");
    println!(
        "{:<20} {:>12} {:>12} {:>12} {:>10}",
        "Shape", "Culled", "Greedy", "Reduction", "Factor"
    );
    println!("{:-<70}", "");

    // Test 1: Single voxel
    {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));
        let culled = build_chunk_mesh(&chunk).count_vertices();
        let greedy = build_chunk_mesh_greedy(&chunk).count_vertices();
        let reduction = (1.0 - greedy as f64 / culled as f64) * 100.0;
        let factor = culled as f64 / greedy as f64;
        println!(
            "{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x",
            "Single voxel", culled, greedy, reduction, factor
        );
    }

    // Test 2: 8x8x8 uniform
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
        println!(
            "{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x",
            "8x8x8 uniform", culled, greedy, reduction, factor
        );
    }

    // Test 3: 16x16 flat layer
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
        println!(
            "{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x",
            "16x16 flat layer", culled, greedy, reduction, factor
        );
    }

    // Test 4: Checkerboard (no improvement)
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
        println!(
            "{:<20} {:>12} {:>12} {:>11.1}% {:>9.1}x",
            "4x4x4 checkerboard", culled, greedy, reduction, factor
        );
    }

    println!("{:-<70}", "");
    println!("\nKey insights:");
    println!("- Uniform surfaces: massive improvement (up to 64x for 8x8 faces)");
    println!("- Checkerboard: no improvement (colors prevent merging)");
}
