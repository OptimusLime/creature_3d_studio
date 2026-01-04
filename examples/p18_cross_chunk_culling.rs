//! Phase 18: Cross-Chunk Face Culling Test
//!
//! Demonstrates cross-chunk face culling with statistics.
//!
//! Run with: `cargo run --example p18_cross_chunk_culling`
//!
//! Expected output: `screenshots/p18_cross_chunk_culling.png`
//! - Seamless wall at chunk boundary (no visible seam)
//! - Floor spanning multiple chunk boundaries

use bevy::prelude::*;
use studio_core::{
    build_world_meshes_cross_chunk_with_options, build_world_meshes_with_options, Voxel,
    VoxelWorldApp, VoxelWorld,
};

fn main() {
    println!("Running Phase 18: Cross-Chunk Face Culling Test...");
    println!("===================================================\n");

    // Print statistics demonstrating vertex savings
    demonstrate_culling_stats();

    println!("\n--- Visual Test ---\n");

    VoxelWorldApp::new("Phase 18: Cross-Chunk Face Culling")
        .with_world_file("assets/worlds/cross_chunk_test.voxworld")
        .with_resolution(1024, 768)
        .with_deferred(true)
        .with_cross_chunk_culling(true)
        .with_shadow_light(Vec3::new(32.0, 15.0, 32.0))
        .with_clear_color(Color::srgb(0.05, 0.05, 0.1))
        .with_camera_angle(45.0, 30.0)
        .with_zoom(0.5) // Zoom in 50% for detail
        .with_screenshot("screenshots/p18_cross_chunk_culling.png")
        .run();
}

fn demonstrate_culling_stats() {
    // Test 1: Two adjacent voxels across chunk boundary
    {
        let mut world = VoxelWorld::new();
        world.set_voxel(31, 16, 16, Voxel::solid(255, 0, 0));
        world.set_voxel(32, 16, 16, Voxel::solid(0, 255, 0));

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

    // Test 2: 8x8 wall at chunk boundary
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

        println!("Test 2: 8x8 Wall at Chunk Boundary (128 voxels)");
        println!("  Without cross-chunk culling: {} vertices", verts_without);
        println!("  With cross-chunk culling:    {} vertices", verts_with);
        println!(
            "  Reduction: {} vertices ({:.1}%)\n",
            verts_without - verts_with,
            (1.0 - verts_with as f32 / verts_without as f32) * 100.0
        );
    }
}
