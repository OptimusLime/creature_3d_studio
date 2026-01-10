//! Generate a placeholder cloud texture for Phase 2 testing.
//!
//! Creates a simple checkerboard pattern with alpha to verify texture sampling works.
//!
//! Run: cargo run --example gen_placeholder_cloud

use std::path::Path;
use studio_core::markov_junior::{
    render::{render_2d, save_png},
    MjGrid,
};

fn main() {
    let size = 256usize;
    let checker_size = 32usize;

    // Create a 2D grid
    // Values: 0 = transparent, 1 = white cloud
    let mut grid = MjGrid::with_values(size, size, 1, "BW");

    for y in 0..size {
        for x in 0..size {
            let checker_x = (x / checker_size) % 2;
            let checker_y = (y / checker_size) % 2;

            let is_white = (checker_x + checker_y) % 2 == 0;

            if is_white {
                grid.set(x, y, 0, 1); // White
            }
            // else stays 0 (transparent)
        }
    }

    // Colors: 0=transparent, 1=white with alpha
    let colors: Vec<[u8; 4]> = vec![
        [0, 0, 0, 0],         // 0: transparent
        [255, 255, 255, 180], // 1: white with 70% opacity
    ];

    let img = render_2d(&grid, &colors, 1);

    let path = Path::new("assets/textures/clouds_placeholder.png");
    save_png(&img, path).expect("Failed to save image");

    println!("Created: {}", path.display());
    println!("Size: {}x{}", size, size);
    println!("Pattern: {}px checkerboard", checker_size);
}
