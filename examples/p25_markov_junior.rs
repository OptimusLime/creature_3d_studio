//! Phase 25: MarkovJunior End-to-End Skeleton
//!
//! Demonstrates the MarkovJunior -> VoxelWorld pipeline with a hardcoded cross pattern.
//! This is Phase 0 of the MarkovJunior implementation - proving the pipeline works
//! before adding algorithm complexity.
//!
//! Run with: `cargo run --example p25_markov_junior`
//!
//! Expected output: `screenshots/p25_markov_junior.png`
//! - 5x5x1 cross pattern (center + 4 adjacent voxels)
//! - White voxels on black background
//! - Camera looking at the pattern from above-angle

use bevy::prelude::*;
use studio_core::markov_junior::{MjGrid, MjPalette};
use studio_core::{VoxelWorldApp, WorldSource};

fn main() {
    // Create a 5x5x1 MjGrid with a cross pattern
    let mut grid = MjGrid::new(5, 5, 1);

    // Set cross pattern: center + 4 adjacent (value 1 = white)
    grid.set(2, 2, 0, 1); // center
    grid.set(1, 2, 0, 1); // left
    grid.set(3, 2, 0, 1); // right
    grid.set(2, 1, 0, 1); // down
    grid.set(2, 3, 0, 1); // up

    println!("MarkovJunior Phase 0: End-to-End Skeleton");
    println!("Grid size: {}x{}x{}", grid.mx, grid.my, grid.mz);
    println!("Non-zero voxels: {}", grid.count_nonzero());

    // Convert to VoxelWorld using default palette
    let palette = MjPalette::default();
    let world = grid.to_voxel_world(&palette);

    println!(
        "Generated {} voxels in VoxelWorld",
        world.iter_chunks().map(|(_, c)| c.count()).sum::<usize>()
    );

    // Run with VoxelWorldApp
    VoxelWorldApp::new("Phase 25: MarkovJunior")
        .with_world(WorldSource::World(world))
        .with_resolution(800, 600)
        .with_clear_color(Color::BLACK)
        .with_deferred(false) // Simple forward rendering for this test
        .with_camera_angle(45.0, 35.0) // Slightly lower elevation
        .with_zoom(2.5) // Zoom out to see the full pattern clearly
        .with_screenshot("screenshots/p25_markov_junior.png")
        .run();
}
