//! Phase 14: Face Culling Test
//!
//! Demonstrates face culling optimization with statistics.
//!
//! Run with: `cargo run --example p14_face_culling`
//!
//! Expected output: `screenshots/p14_face_culling.png`

use bevy::prelude::*;
use studio_core::{build_chunk_mesh, Voxel, VoxelChunk, VoxelWorldApp};

fn main() {
    println!("===========================================");
    println!("Phase 14: Face Culling Test");
    println!("===========================================\n");

    // Print statistics before rendering
    print_face_culling_stats();

    println!("\n===========================================");
    println!("Launching visual test...");
    println!("===========================================\n");

    VoxelWorldApp::new("Phase 14: Face Culling Test")
        .with_world_file("assets/worlds/mesh_test.voxworld")
        .with_deferred(true)
        .with_greedy_meshing(false) // Use face culling only
        .with_shadow_light(Vec3::new(15.0, 15.0, 15.0)) // Light outside the 8x8x8 cube
        .with_clear_color(Color::srgb(0.05, 0.05, 0.1))
        .with_camera_angle(45.0, 30.0) // Auto-frame to voxel bounds
        .with_screenshot_timed("screenshots/p14_face_culling.png", 10, 20)
        .run();
}

fn print_face_culling_stats() {
    println!("Face Culling Statistics");
    println!("-----------------------\n");

    // Test 1: Single voxel
    {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));
        let mesh = build_chunk_mesh(&chunk);
        let vertices = mesh.count_vertices();
        let max = 24;
        println!(
            "Single voxel:     {:4} vertices (max: {:4}, reduction: {:5.1}%)",
            vertices, max,
            (1.0 - vertices as f64 / max as f64) * 100.0
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
        let max = 8 * 24;
        println!(
            "2x2x2 cube:       {:4} vertices (max: {:4}, reduction: {:5.1}%)",
            vertices, max,
            (1.0 - vertices as f64 / max as f64) * 100.0
        );
    }

    // Test 3: 8x8x8 cube
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
        let max = 512 * 24;
        let expected = 6 * 64 * 4; // 6 faces * 64 surface quads * 4 verts
        println!(
            "8x8x8 cube:       {:4} vertices (max: {:5}, reduction: {:5.1}%)",
            vertices, max,
            (1.0 - vertices as f64 / max as f64) * 100.0
        );
        println!("                  Expected: {} vertices ({} surface faces)", expected, 6 * 64);
    }
}
