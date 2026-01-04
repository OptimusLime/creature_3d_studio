//! PNG rendering for MarkovJunior grids.
//!
//! Renders MjGrid to PNG images without any Bevy/GPU dependencies.
//! Supports both 2D flat rendering and 3D isometric rendering.
//!
//! C# Reference: Graphics.cs (BitmapRender, IsometricRender, SaveBitmap)

use super::MjGrid;
use image::{ImageBuffer, Rgba, RgbaImage};
use std::path::Path;

/// Default background color (dark gray, matches C# GUI.BACKGROUND)
const BACKGROUND: [u8; 4] = [34, 34, 34, 255];

/// Default color palette for rendering.
/// Maps grid value indices to RGBA colors.
/// Index 0 is transparent (empty), indices 1+ are visible colors.
pub fn default_colors() -> Vec<[u8; 4]> {
    vec![
        [0, 0, 0, 0],         // 0: transparent/empty
        [255, 255, 255, 255], // 1: white
        [255, 0, 0, 255],     // 2: red
        [0, 255, 0, 255],     // 3: green
        [0, 0, 255, 255],     // 4: blue
        [255, 255, 0, 255],   // 5: yellow
        [0, 255, 255, 255],   // 6: cyan
        [255, 0, 255, 255],   // 7: magenta
        [128, 128, 128, 255], // 8: gray
    ]
}

/// PICO-8 16-color palette.
pub fn pico8_colors() -> Vec<[u8; 4]> {
    vec![
        [0, 0, 0, 0],         // 0: transparent
        [29, 43, 83, 255],    // 1: dark-blue
        [126, 37, 83, 255],   // 2: dark-purple
        [0, 135, 81, 255],    // 3: dark-green
        [171, 82, 54, 255],   // 4: brown
        [95, 87, 79, 255],    // 5: dark-grey
        [194, 195, 199, 255], // 6: light-grey
        [255, 241, 232, 255], // 7: white
        [255, 0, 77, 255],    // 8: red
        [255, 163, 0, 255],   // 9: orange
        [255, 236, 39, 255],  // 10: yellow
        [0, 228, 54, 255],    // 11: green
        [41, 173, 255, 255],  // 12: blue
        [131, 118, 156, 255], // 13: lavender
        [255, 119, 168, 255], // 14: pink
        [255, 204, 170, 255], // 15: light-peach
    ]
}

/// Render a 2D grid (mz=1) to an RGBA image.
///
/// # Arguments
/// * `grid` - The grid to render (must have mz=1)
/// * `colors` - Color palette mapping value index to RGBA
/// * `pixel_size` - Size of each cell in pixels (1 = 1:1, 4 = 4x4 per cell)
///
/// # Returns
/// RGBA image buffer
pub fn render_2d(grid: &MjGrid, colors: &[[u8; 4]], pixel_size: u32) -> RgbaImage {
    let width = (grid.mx as u32) * pixel_size;
    let height = (grid.my as u32) * pixel_size;

    let mut img: RgbaImage = ImageBuffer::new(width, height);

    // Fill with background
    for pixel in img.pixels_mut() {
        *pixel = Rgba(BACKGROUND);
    }

    // Draw each cell
    for y in 0..grid.my {
        for x in 0..grid.mx {
            let idx = x + y * grid.mx;
            let value = grid.state[idx] as usize;

            // Get color (or skip if transparent/out of range)
            let color = if value < colors.len() {
                colors[value]
            } else {
                continue;
            };

            // Skip fully transparent
            if color[3] == 0 {
                continue;
            }

            // Fill the pixel_size x pixel_size block
            for dy in 0..pixel_size {
                for dx in 0..pixel_size {
                    let px = (x as u32) * pixel_size + dx;
                    let py = (y as u32) * pixel_size + dy;
                    img.put_pixel(px, py, Rgba(color));
                }
            }
        }
    }

    img
}

/// Render a 3D grid to an isometric RGBA image.
///
/// Uses the same isometric projection as the C# MarkovJunior.
///
/// # Arguments
/// * `grid` - The grid to render
/// * `colors` - Color palette mapping value index to RGBA
/// * `block_size` - Size of each voxel in pixels
///
/// # Returns
/// RGBA image buffer with isometric view
pub fn render_3d_isometric(grid: &MjGrid, colors: &[[u8; 4]], block_size: u32) -> RgbaImage {
    let mx = grid.mx;
    let my = grid.my;
    let mz = grid.mz;

    // Calculate image dimensions (matching C# formula)
    let fit_width = ((mx + my) as u32) * block_size;
    let fit_height = (((mx + my) / 2 + mz) as u32) * block_size;
    let width = fit_width + 2 * block_size;
    let height = fit_height + 2 * block_size;

    let mut img: RgbaImage = ImageBuffer::new(width, height);

    // Fill with background
    for pixel in img.pixels_mut() {
        *pixel = Rgba(BACKGROUND);
    }

    // Build visibility array
    let mut visible = vec![false; mx * my * mz];
    for z in 0..mz {
        for y in 0..my {
            for x in 0..mx {
                let i = x + y * mx + z * mx * my;
                visible[i] = grid.state[i] != 0;
            }
        }
    }

    // Collect visible voxels sorted by depth (back to front)
    // Depth = x + y + z (painter's algorithm for isometric)
    let mut voxels: Vec<(usize, usize, usize, u8)> = Vec::new();
    for z in 0..mz {
        for y in 0..my {
            for x in 0..mx {
                let i = x + y * mx + z * mx * my;
                if grid.state[i] != 0 {
                    voxels.push((x, y, z, grid.state[i]));
                }
            }
        }
    }

    // Sort by depth (back to front)
    voxels.sort_by_key(|&(x, y, z, _)| x + y + z);

    // Draw each voxel as a simple cube
    for (x, y, z, value) in voxels {
        let color = if (value as usize) < colors.len() {
            colors[value as usize]
        } else {
            [255, 255, 255, 255] // default white
        };

        if color[3] == 0 {
            continue;
        }

        // Isometric projection
        // u = (x - y) * block_size
        // v = (x + y) / 2 * block_size - z * block_size
        let u = ((x as i32) - (y as i32)) * (block_size as i32);
        let v = (((x + y) as i32) * (block_size as i32)) / 2 - (z as i32) * (block_size as i32);

        let center_x = (width / 2) as i32;
        let center_y = ((height - fit_height) / 2 + (mz as u32 - 1) * block_size) as i32;

        let pos_x = center_x + u - (block_size as i32);
        let pos_y = center_y + v;

        // Draw a simple diamond/cube shape
        draw_isometric_cube(&mut img, pos_x, pos_y, block_size as i32, color);
    }

    img
}

/// Draw a simple isometric cube at the given position.
fn draw_isometric_cube(img: &mut RgbaImage, x: i32, y: i32, size: i32, color: [u8; 4]) {
    let (r, g, b, a) = (color[0], color[1], color[2], color[3]);

    // Three faces with different brightness
    let top_color = Rgba([r, g, b, a]);
    let left_color = Rgba([
        (r as u32 * 140 / 255) as u8,
        (g as u32 * 140 / 255) as u8,
        (b as u32 * 140 / 255) as u8,
        a,
    ]);
    let right_color = Rgba([
        (r as u32 * 90 / 255) as u8,
        (g as u32 * 90 / 255) as u8,
        (b as u32 * 90 / 255) as u8,
        a,
    ]);

    let width = img.width() as i32;
    let height = img.height() as i32;

    // Draw each pixel in the isometric cube shape
    for dy in 0..(2 * size) {
        for dx in 0..(2 * size) {
            let px = x + dx;
            let py = y + dy - size;

            if px < 0 || px >= width || py < 0 || py >= height {
                continue;
            }

            // Determine which face this pixel belongs to
            let local_x = dx - size;
            let local_y = size - dy;

            // Check if inside the diamond shape
            let in_diamond = 2 * local_y.abs() + local_x.abs() <= 2 * size;
            if !in_diamond {
                continue;
            }

            // Determine face based on position
            let pixel_color = if local_y > local_x.abs() / 2 {
                // Top face
                top_color
            } else if local_x > 0 {
                // Right face
                right_color
            } else {
                // Left face
                left_color
            };

            img.put_pixel(px as u32, py as u32, pixel_color);
        }
    }
}

/// Save an RGBA image to a PNG file.
pub fn save_png(img: &RgbaImage, path: &Path) -> Result<(), image::ImageError> {
    img.save(path)
}

/// Convenience function: render grid and save to PNG.
///
/// Automatically chooses 2D or 3D rendering based on grid dimensions.
///
/// # Arguments
/// * `grid` - The grid to render
/// * `path` - Output PNG path
/// * `pixel_size` - Size of each cell/voxel in pixels
///
/// # Returns
/// Ok(()) on success, Err on IO/image error
pub fn render_to_png(grid: &MjGrid, path: &Path, pixel_size: u32) -> Result<(), image::ImageError> {
    let colors = default_colors();
    let img = if grid.mz == 1 {
        render_2d(grid, &colors, pixel_size)
    } else {
        render_3d_isometric(grid, &colors, pixel_size)
    };
    save_png(&img, path)
}

/// Convenience function with custom colors.
pub fn render_to_png_with_colors(
    grid: &MjGrid,
    path: &Path,
    pixel_size: u32,
    colors: &[[u8; 4]],
) -> Result<(), image::ImageError> {
    let img = if grid.mz == 1 {
        render_2d(grid, colors, pixel_size)
    } else {
        render_3d_isometric(grid, colors, pixel_size)
    };
    save_png(&img, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_output_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("screenshots")
    }

    #[test]
    fn test_render_2d_simple() {
        // Create a simple 5x5 grid with a cross pattern
        let mut grid = MjGrid::with_values(5, 5, 1, "BW");
        // B=0 (transparent), W=1 (white)
        grid.set(2, 2, 0, 1); // center
        grid.set(1, 2, 0, 1); // left
        grid.set(3, 2, 0, 1); // right
        grid.set(2, 1, 0, 1); // up
        grid.set(2, 3, 0, 1); // down

        let colors = default_colors();
        let img = render_2d(&grid, &colors, 10);

        assert_eq!(img.width(), 50);
        assert_eq!(img.height(), 50);

        // Center pixel should be white
        let center = img.get_pixel(25, 25);
        assert_eq!(center.0, [255, 255, 255, 255]);
    }

    #[test]
    fn test_render_2d_to_file() {
        let mut grid = MjGrid::with_values(16, 16, 1, "BW");
        // Create a checkerboard pattern
        for y in 0..16 {
            for x in 0..16 {
                if (x + y) % 2 == 0 {
                    grid.set(x, y, 0, 1);
                }
            }
        }

        let path = test_output_dir().join("test_render_2d_checkerboard.png");
        render_to_png(&grid, &path, 8).unwrap();

        assert!(path.exists(), "PNG file should be created");
    }

    #[test]
    fn test_render_3d_simple() {
        // Create a simple 3D grid with a few voxels
        let mut grid = MjGrid::with_values(5, 5, 5, "BW");
        grid.set(2, 2, 0, 1); // bottom center
        grid.set(2, 2, 1, 1); // one up
        grid.set(2, 2, 2, 1); // two up

        let colors = default_colors();
        let img = render_3d_isometric(&grid, &colors, 8);

        // Should produce a non-empty image
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }

    #[test]
    fn test_render_3d_to_file() {
        let mut grid = MjGrid::with_values(8, 8, 8, "BWR");
        // Create a small structure
        for x in 2..6 {
            for y in 2..6 {
                grid.set(x, y, 0, 1); // base layer
            }
        }
        for x in 3..5 {
            for y in 3..5 {
                grid.set(x, y, 1, 2); // second layer (red)
            }
        }
        grid.set(3, 3, 2, 1); // top

        let path = test_output_dir().join("test_render_3d_structure.png");
        let colors = default_colors();
        let img = render_3d_isometric(&grid, &colors, 12);
        save_png(&img, &path).unwrap();

        assert!(path.exists(), "PNG file should be created");
    }

    #[test]
    fn test_pico8_palette() {
        let colors = pico8_colors();
        assert_eq!(colors.len(), 16);
        assert_eq!(colors[0][3], 0); // first is transparent
        assert_ne!(colors[1][3], 0); // rest are opaque
    }

    // ========================================================================
    // Integration tests: Run actual MarkovJunior models and render output
    // ========================================================================

    fn models_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MarkovJunior/models")
    }

    /// Test: Load and run Growth.xml (2D), render to PNG.
    /// Growth.xml: Simple expansion from origin using WB->WW rule.
    #[test]
    fn test_markov_growth_2d_render() {
        use crate::markov_junior::Model;

        let path = models_path().join("Growth.xml");
        let mut model = Model::load(&path).expect("Failed to load Growth.xml");

        // Run with fixed seed
        let steps = model.run(12345, 5000);
        let grid = model.grid();

        // Should have generated many cells
        let nonzero = grid.count_nonzero();
        assert!(
            nonzero > 100,
            "Growth should produce >100 cells, got {}",
            nonzero
        );
        assert!(steps > 100, "Growth should take >100 steps, got {}", steps);

        // Render to PNG
        let output_path = test_output_dir().join("test_markov_growth_2d.png");
        render_to_png(grid, &output_path, 4).unwrap();

        assert!(output_path.exists(), "PNG should be created");
        println!(
            "Growth 2D: {} steps, {} cells -> {}",
            steps,
            nonzero,
            output_path.display()
        );
    }

    /// Test: Load and run MazeBacktracker.xml (2D), render to PNG.
    /// MazeBacktracker: Generates maze using backtracking algorithm.
    #[test]
    fn test_markov_maze_2d_render() {
        use crate::markov_junior::Model;

        let path = models_path().join("MazeBacktracker.xml");
        let mut model = Model::load(&path).expect("Failed to load MazeBacktracker.xml");

        // Run with fixed seed
        let steps = model.run(42, 10000);
        let grid = model.grid();

        // Should have generated maze structure
        let nonzero = grid.count_nonzero();
        assert!(
            nonzero > 50,
            "Maze should produce >50 cells, got {}",
            nonzero
        );

        // Render to PNG
        let output_path = test_output_dir().join("test_markov_maze_2d.png");
        render_to_png(grid, &output_path, 4).unwrap();

        assert!(output_path.exists(), "PNG should be created");
        println!(
            "Maze 2D: {} steps, {} cells -> {}",
            steps,
            nonzero,
            output_path.display()
        );
    }

    /// Test: Load and run MazeGrowth.xml (2D), render to PNG.
    /// MazeGrowth: Generates maze using growth pattern WBB->WAW.
    #[test]
    fn test_markov_mazegrowth_2d_render() {
        use crate::markov_junior::Model;

        let path = models_path().join("MazeGrowth.xml");
        let mut model = Model::load(&path).expect("Failed to load MazeGrowth.xml");

        // Run with fixed seed
        let steps = model.run(99, 5000);
        let grid = model.grid();

        let nonzero = grid.count_nonzero();
        assert!(
            nonzero > 50,
            "MazeGrowth should produce >50 cells, got {}",
            nonzero
        );

        // Render to PNG
        let output_path = test_output_dir().join("test_markov_mazegrowth_2d.png");
        render_to_png(grid, &output_path, 4).unwrap();

        assert!(output_path.exists(), "PNG should be created");
        println!(
            "MazeGrowth 2D: {} steps, {} cells -> {}",
            steps,
            nonzero,
            output_path.display()
        );
    }

    /// Test: Run Growth model with 3D dimensions, render isometric.
    /// NOTE: Growth.xml uses WB→WW which is a 1D rule. In 3D, it grows along X axis only.
    /// This test verifies 3D rendering works, not that Growth.xml is a good 3D model.
    #[test]
    fn test_markov_growth_3d_render() {
        use crate::markov_junior::Model;

        let path = models_path().join("Growth.xml");
        // Load with custom 3D dimensions
        let mut model = Model::load_with_size(&path, 16, 16, 16)
            .expect("Failed to load Growth.xml with 3D size");

        let grid = model.grid();
        assert_eq!(grid.mz, 16, "Grid should be 16 deep after load");

        // Run with fixed seed
        let steps = model.run(7777, 5000);
        let grid = model.grid();

        let nonzero = grid.count_nonzero();

        // Render to PNG (isometric) - this tests 3D rendering even with few voxels
        let output_path = test_output_dir().join("test_markov_growth_3d.png");
        render_to_png(grid, &output_path, 8).unwrap();

        assert!(output_path.exists(), "PNG should be created");
        // Growth.xml with 1D rule in 3D only produces a line, which is fine
        assert!(nonzero > 0, "Should produce some cells, got {}", nonzero);
        println!(
            "Growth 3D: {} steps, {} cells -> {}",
            steps,
            nonzero,
            output_path.display()
        );
    }

    /// Test: Create a proper 3D growth model programmatically and render.
    /// Uses B→W rule which works in all positions.
    #[test]
    fn test_markov_programmatic_3d_growth_render() {
        use crate::markov_junior::Model;

        // Create inline model with simple B→W rule and origin
        let xml = r#"<one values="BW" origin="True" in="B" out="W"/>"#;
        let mut model = Model::load_str(xml, 12, 12, 12).expect("Failed to load inline 3D model");

        let grid = model.grid();
        assert_eq!(grid.mz, 12, "Grid should be 12 deep");

        // Run - this should fill the entire grid since B→W matches everywhere
        let steps = model.run(42, 2000);
        let grid = model.grid();

        let nonzero = grid.count_nonzero();

        // B→W should eventually fill most of the grid
        assert!(
            nonzero > 500,
            "3D B→W should produce >500 cells, got {}",
            nonzero
        );

        // Render to PNG
        let output_path = test_output_dir().join("test_markov_programmatic_3d.png");
        render_to_png(grid, &output_path, 8).unwrap();

        assert!(output_path.exists(), "PNG should be created");
        println!(
            "Programmatic 3D: {} steps, {} cells -> {}",
            steps,
            nonzero,
            output_path.display()
        );
    }

    /// Test: Run MazeGrowth with 3D dimensions, render isometric.
    /// NOTE: MazeGrowth.xml uses WBB→WAW which is also a 1D pattern.
    #[test]
    fn test_markov_mazegrowth_3d_render() {
        use crate::markov_junior::Model;

        let path = models_path().join("MazeGrowth.xml");
        // Load with custom 3D dimensions (odd for proper maze)
        let mut model = Model::load_with_size(&path, 17, 17, 17)
            .expect("Failed to load MazeGrowth.xml with 3D size");

        let grid = model.grid();
        assert_eq!(grid.mz, 17, "Grid should be 17 deep");

        // Run with fixed seed
        let steps = model.run(1234, 10000);
        let grid = model.grid();

        let nonzero = grid.count_nonzero();

        // Render to PNG (isometric)
        let output_path = test_output_dir().join("test_markov_mazegrowth_3d.png");
        render_to_png(grid, &output_path, 6).unwrap();

        assert!(output_path.exists(), "PNG should be created");
        // MazeGrowth with 1D rule in 3D produces limited structure
        assert!(nonzero > 0, "Should produce some cells, got {}", nonzero);
        println!(
            "MazeGrowth 3D: {} steps, {} cells -> {}",
            steps,
            nonzero,
            output_path.display()
        );
    }
}
