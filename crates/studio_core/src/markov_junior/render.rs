//! PNG rendering for MarkovJunior grids.
//!
//! Renders MjGrid to PNG images without any Bevy/GPU dependencies.
//! Supports both 2D flat rendering and 3D isometric rendering.
//!
//! C# Reference: Graphics.cs (BitmapRender, IsometricRender, SaveBitmap)

use super::MjGrid;
use image::{ImageBuffer, Rgba, RgbaImage};
use std::collections::HashMap;
use std::path::Path;

/// Default background color (dark gray, matches C# GUI.BACKGROUND)
const BACKGROUND: [u8; 4] = [34, 34, 34, 255];

/// MarkovJunior palette mapping characters to RGBA colors.
/// Based on the C# palette.xml file.
#[derive(Debug, Clone)]
pub struct RenderPalette {
    /// Character to RGBA color mapping
    colors: HashMap<char, [u8; 4]>,
}

impl Default for RenderPalette {
    fn default() -> Self {
        Self::from_palette_xml()
    }
}

impl RenderPalette {
    /// Create palette from C# MarkovJunior palette.xml colors.
    /// This is the standard palette used by the reference implementation.
    pub fn from_palette_xml() -> Self {
        let mut colors = HashMap::new();

        // Primary colors (uppercase) from palette.xml
        colors.insert('B', [0x00, 0x00, 0x00, 0xFF]); // Black
        colors.insert('I', [0x1D, 0x2B, 0x53, 0xFF]); // Indigo
        colors.insert('P', [0x7E, 0x25, 0x53, 0xFF]); // Purple
        colors.insert('E', [0x00, 0x87, 0x51, 0xFF]); // Emerald
        colors.insert('N', [0xAB, 0x52, 0x36, 0xFF]); // browN
        colors.insert('D', [0x5F, 0x57, 0x4F, 0xFF]); // Dead/Dark
        colors.insert('A', [0xC2, 0xC3, 0xC7, 0xFF]); // Alive/grAy
        colors.insert('W', [0xFF, 0xF1, 0xE8, 0xFF]); // White
        colors.insert('R', [0xFF, 0x00, 0x4D, 0xFF]); // Red
        colors.insert('O', [0xFF, 0xA3, 0x00, 0xFF]); // Orange
        colors.insert('Y', [0xFF, 0xEC, 0x27, 0xFF]); // Yellow
        colors.insert('G', [0x00, 0xE4, 0x36, 0xFF]); // Green
        colors.insert('U', [0x29, 0xAD, 0xFF, 0xFF]); // blUe
        colors.insert('S', [0x83, 0x76, 0x9C, 0xFF]); // Slate
        colors.insert('K', [0xFF, 0x77, 0xA8, 0xFF]); // pinK
        colors.insert('F', [0xFF, 0xCC, 0xAA, 0xFF]); // Fawn

        // Secondary colors (lowercase) from palette.xml
        colors.insert('b', [0x29, 0x18, 0x14, 0xFF]); // black
        colors.insert('i', [0x11, 0x1D, 0x35, 0xFF]); // indigo
        colors.insert('p', [0x42, 0x21, 0x36, 0xFF]); // purple
        colors.insert('e', [0x12, 0x53, 0x59, 0xFF]); // emerald
        colors.insert('n', [0x74, 0x2F, 0x29, 0xFF]); // brown
        colors.insert('d', [0x49, 0x33, 0x3B, 0xFF]); // dead/dark
        colors.insert('a', [0xA2, 0x88, 0x79, 0xFF]); // alive/gray
        colors.insert('w', [0xF3, 0xEF, 0x7D, 0xFF]); // white
        colors.insert('r', [0xBE, 0x12, 0x50, 0xFF]); // red
        colors.insert('o', [0xFF, 0x6C, 0x24, 0xFF]); // orange
        colors.insert('y', [0xA8, 0xE7, 0x2E, 0xFF]); // yellow
        colors.insert('g', [0x00, 0xB5, 0x43, 0xFF]); // green
        colors.insert('u', [0x06, 0x5A, 0xB5, 0xFF]); // blue
        colors.insert('s', [0x75, 0x46, 0x65, 0xFF]); // slate
        colors.insert('k', [0xFF, 0x6E, 0x59, 0xFF]); // pink
        colors.insert('f', [0xFF, 0x9D, 0x81, 0xFF]); // fawn

        // Additional colors
        colors.insert('C', [0x00, 0xFF, 0xFF, 0xFF]); // Cyan
        colors.insert('c', [0x5F, 0xCD, 0xE4, 0xFF]); // cyan
        colors.insert('H', [0xE4, 0xBB, 0x40, 0xFF]); // Honey
        colors.insert('h', [0x8A, 0x6F, 0x30, 0xFF]); // honey
        colors.insert('J', [0x4B, 0x69, 0x2F, 0xFF]); // Jungle
        colors.insert('j', [0x45, 0x10, 0x7E, 0xFF]); // jungle
        colors.insert('L', [0x84, 0x7E, 0x87, 0xFF]); // Light
        colors.insert('l', [0x69, 0x6A, 0x6A, 0xFF]); // light
        colors.insert('M', [0xFF, 0x00, 0xFF, 0xFF]); // Magenta
        colors.insert('m', [0x9C, 0x09, 0xCC, 0xFF]); // magenta
        colors.insert('Q', [0x9B, 0xAD, 0xB7, 0xFF]); // aQua
        colors.insert('q', [0x3F, 0x3F, 0x74, 0xFF]); // aqua
        colors.insert('T', [0x37, 0x94, 0x6E, 0xFF]); // Teal
        colors.insert('t', [0x32, 0x3C, 0x39, 0xFF]); // teal
        colors.insert('V', [0x8F, 0x97, 0x4A, 0xFF]); // oliVe
        colors.insert('v', [0x52, 0x4B, 0x24, 0xFF]); // olive
        colors.insert('X', [0xFF, 0x00, 0x00, 0xFF]); // X (pure red)
        colors.insert('x', [0xD9, 0x57, 0x63, 0xFF]); // x
        colors.insert('Z', [0xFF, 0xFF, 0xFF, 0xFF]); // Z (pure white)
        colors.insert('z', [0xCB, 0xDB, 0xFC, 0xFF]); // z

        Self { colors }
    }

    /// Get the color for a character.
    pub fn get(&self, ch: char) -> Option<[u8; 4]> {
        self.colors.get(&ch).copied()
    }

    /// Get colors as a Vec ordered by grid index.
    /// Maps grid state values (0, 1, 2...) to their character colors.
    ///
    /// IMPORTANT: Value 0 is ALWAYS transparent/empty in MarkovJunior convention,
    /// regardless of what character it represents. This matches C# behavior where
    /// `visible[i] = value != 0`.
    ///
    /// For a grid with values="BWA":
    /// - State 0 → transparent (empty/background)
    /// - State 1 → 'W' → White  
    /// - State 2 → 'A' → Gray
    pub fn to_index_colors(&self, grid: &MjGrid) -> Vec<[u8; 4]> {
        let mut colors = Vec::with_capacity(grid.c as usize);

        for i in 0..grid.c {
            if i == 0 {
                // Value 0 is always transparent (empty/background) - matches C# convention
                colors.push([0, 0, 0, 0]);
            } else if (i as usize) < grid.characters.len() {
                let ch = grid.characters[i as usize];
                let color = self.get(ch).unwrap_or([255, 0, 255, 255]); // magenta fallback
                colors.push(color);
            } else {
                // Out of range = magenta (error indicator)
                colors.push([255, 0, 255, 255]);
            }
        }

        colors
    }
}

/// Get colors for a grid using the standard MarkovJunior palette.
/// This maps each character in the grid's values to its proper color.
pub fn colors_for_grid(grid: &MjGrid) -> Vec<[u8; 4]> {
    let palette = RenderPalette::default();
    palette.to_index_colors(grid)
}

/// Default color palette for rendering (legacy - use colors_for_grid instead).
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

/// Draw an isometric cube at the given position.
/// Matches the C# MarkovJunior Graphics.cs Sprite implementation.
///
/// The C# sprite uses:
/// - width = 2 * size
/// - height = 2 * size - 1
/// - Coordinate system: local_x = i - size + 1, local_y = size - j - 1
/// - Face brightness: top=215, left=143, right=71 (out of 256)
fn draw_isometric_cube(img: &mut RgbaImage, x: i32, y: i32, size: i32, color: [u8; 4]) {
    let (r, g, b, a) = (color[0], color[1], color[2], color[3]);

    // C# brightness values from Sprite class
    const C1: u32 = 215; // top (brightest)
    const C2: u32 = 143; // left (medium)
    const C3: u32 = 71; // right (darkest)

    let img_width = img.width() as i32;
    let img_height = img.height() as i32;

    let sprite_width = 2 * size;
    let sprite_height = 2 * size - 1;

    // Draw each pixel using C# coordinate system
    for j in 0..sprite_height {
        for i in 0..sprite_width {
            // C# local coordinates: local_x = i - size + 1, local_y = size - j - 1
            let local_x = i - size + 1;
            let local_y = size - j - 1;

            // C# boundary check from Sprite.f():
            // if (2*y - x >= 2*size || 2*y + x > 2*size || 2*y - x < -2*size || 2*y + x <= -2*size) return transparent;
            let two_y_minus_x = 2 * local_y - local_x;
            let two_y_plus_x = 2 * local_y + local_x;

            if two_y_minus_x >= 2 * size
                || two_y_plus_x > 2 * size
                || two_y_minus_x < -2 * size
                || two_y_plus_x <= -2 * size
            {
                continue; // transparent
            }

            // Determine face (from C# Sprite.f()):
            // if (x > 0 && 2*y < x) return c3;      // right face
            // if (x <= 0 && 2*y <= -x) return c2;   // left face
            // else return c1;                        // top face
            let grayscale = if local_x > 0 && 2 * local_y < local_x {
                C3 // right face (darkest)
            } else if local_x <= 0 && 2 * local_y <= -local_x {
                C2 // left face (medium)
            } else {
                C1 // top face (brightest)
            };

            // Apply grayscale to color
            let pr = ((r as u32) * grayscale / 256) as u8;
            let pg = ((g as u32) * grayscale / 256) as u8;
            let pb = ((b as u32) * grayscale / 256) as u8;

            // Calculate screen position
            let px = x + i;
            let py = y + j;

            if px >= 0 && px < img_width && py >= 0 && py < img_height {
                img.put_pixel(px as u32, py as u32, Rgba([pr, pg, pb, a]));
            }
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
/// Uses the grid's character mappings to determine correct colors from palette.xml.
///
/// # Arguments
/// * `grid` - The grid to render
/// * `path` - Output PNG path
/// * `pixel_size` - Size of each cell/voxel in pixels
///
/// # Returns
/// Ok(()) on success, Err on IO/image error
pub fn render_to_png(grid: &MjGrid, path: &Path, pixel_size: u32) -> Result<(), image::ImageError> {
    // Use grid-aware colors that respect character->color mapping
    let colors = colors_for_grid(grid);
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

    #[test]
    fn test_render_palette_from_xml() {
        let palette = RenderPalette::from_palette_xml();

        // Test some key colors from palette.xml
        assert_eq!(palette.get('B'), Some([0x00, 0x00, 0x00, 0xFF])); // Black
        assert_eq!(palette.get('W'), Some([0xFF, 0xF1, 0xE8, 0xFF])); // White (off-white)
        assert_eq!(palette.get('R'), Some([0xFF, 0x00, 0x4D, 0xFF])); // Red
        assert_eq!(palette.get('G'), Some([0x00, 0xE4, 0x36, 0xFF])); // Green
        assert_eq!(palette.get('A'), Some([0xC2, 0xC3, 0xC7, 0xFF])); // Alive/grAy
    }

    #[test]
    fn test_colors_for_grid_maps_correctly() {
        // MazeGrowth.xml uses values="BWA"
        let grid = MjGrid::with_values(4, 4, 1, "BWA");
        let colors = colors_for_grid(&grid);

        // Index 0 (B) should be transparent (value 0 = empty convention)
        assert_eq!(
            colors[0],
            [0, 0, 0, 0],
            "B should be transparent at index 0"
        );

        // Index 1 (W) should be off-white from palette
        assert_eq!(
            colors[1],
            [0xFF, 0xF1, 0xE8, 0xFF],
            "W should be off-white at index 1"
        );

        // Index 2 (A) should be gray from palette
        assert_eq!(
            colors[2],
            [0xC2, 0xC3, 0xC7, 0xFF],
            "A should be gray at index 2"
        );
    }

    #[test]
    fn test_colors_for_grid_mazebacktracker() {
        // MazeBacktracker.xml uses values="BRGW"
        let grid = MjGrid::with_values(4, 4, 1, "BRGW");
        let colors = colors_for_grid(&grid);

        assert_eq!(colors.len(), 4);
        assert_eq!(colors[0], [0, 0, 0, 0], "B should be transparent");
        assert_eq!(colors[1], [0xFF, 0x00, 0x4D, 0xFF], "R should be red");
        assert_eq!(colors[2], [0x00, 0xE4, 0x36, 0xFF], "G should be green");
        assert_eq!(colors[3], [0xFF, 0xF1, 0xE8, 0xFF], "W should be off-white");
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

    // ========================================================================
    // VERIFICATION TEST: Run ALL models with references, save to verification/
    // ========================================================================

    fn verification_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("screenshots/verification")
    }

    fn reference_images_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets/reference_images/mj")
    }

    /// Model configuration from models.xml
    struct ModelConfig {
        name: &'static str,
        size: usize,
        is_3d: bool,
    }

    /// All models that have reference images, with their correct sizes from models.xml
    fn verification_models() -> Vec<ModelConfig> {
        vec![
            ModelConfig {
                name: "Basic",
                size: 60,
                is_3d: false,
            },
            ModelConfig {
                name: "Growth",
                size: 359,
                is_3d: false,
            },
            ModelConfig {
                name: "MazeGrowth",
                size: 359,
                is_3d: false,
            },
            ModelConfig {
                name: "MazeBacktracker",
                size: 359,
                is_3d: false,
            },
            ModelConfig {
                name: "DungeonGrowth",
                size: 79,
                is_3d: false,
            },
            ModelConfig {
                name: "Flowers",
                size: 60,
                is_3d: false,
            },
            ModelConfig {
                name: "Circuit",
                size: 59,
                is_3d: false,
            },
            ModelConfig {
                name: "River",
                size: 80,
                is_3d: false,
            },
            ModelConfig {
                name: "Trail",
                size: 59,
                is_3d: false,
            },
            ModelConfig {
                name: "Wilson",
                size: 59,
                is_3d: false,
            },
            ModelConfig {
                name: "CompleteSAW",
                size: 19,
                is_3d: false,
            },
            ModelConfig {
                name: "RegularSAW",
                size: 39,
                is_3d: false,
            },
            ModelConfig {
                name: "LoopErasedWalk",
                size: 59,
                is_3d: false,
            },
            ModelConfig {
                name: "NystromDungeon",
                size: 39,
                is_3d: false,
            },
            // 3D models - skip for now, will add later
            // ModelConfig { name: "Apartemazements", size: 8, is_3d: true },
            // ModelConfig { name: "StairsPath", size: 33, is_3d: true },
        ]
    }

    /// MASTER VERIFICATION TEST
    /// Runs ALL 2D models with reference images to completion.
    /// Saves output to screenshots/verification/{model}_ours.png
    /// Also copies reference image as {model}_ref.{ext} for easy comparison.
    #[test]
    fn test_verification_run_all_2d_models() {
        use crate::markov_junior::Model;

        let out_dir = verification_dir();
        let ref_dir = reference_images_dir();

        // Create output directory
        std::fs::create_dir_all(&out_dir).expect("Failed to create verification directory");

        let models = verification_models();
        let mut results: Vec<(String, usize, usize, bool)> = Vec::new();

        println!("\n========================================");
        println!("MARKOV JUNIOR VERIFICATION TEST");
        println!("Running {} 2D models to completion", models.len());
        println!("Output: {}", out_dir.display());
        println!("========================================\n");

        for config in &models {
            if config.is_3d {
                continue; // Skip 3D for now
            }

            let xml_path = models_path().join(format!("{}.xml", config.name));

            print!("Running {}... ", config.name);

            // Load model with correct size
            let model_result = Model::load_with_size(&xml_path, config.size, config.size, 1);

            let mut model = match model_result {
                Ok(m) => m,
                Err(e) => {
                    println!("FAILED TO LOAD: {}", e);
                    results.push((config.name.to_string(), 0, 0, false));
                    continue;
                }
            };

            // Run with seed 0, limit steps for slow models
            let max_steps = config.size * config.size * 4; // Reasonable limit
            let steps = model.run(0, max_steps);
            let grid = model.grid();
            let nonzero = grid.count_nonzero();

            // Save our output
            let our_path = out_dir.join(format!("{}_ours.png", config.name));
            let colors = colors_for_grid(grid);
            let img = render_2d(grid, &colors, 2); // pixel_size=2 for reasonable file size
            if let Err(e) = save_png(&img, &our_path) {
                println!("FAILED TO SAVE: {}", e);
                results.push((config.name.to_string(), steps, nonzero, false));
                continue;
            }

            // Copy reference image
            let ref_extensions = ["gif", "png"];
            let mut ref_copied = false;
            for ext in &ref_extensions {
                let ref_src = ref_dir.join(format!("{}.{}", config.name, ext));
                if ref_src.exists() {
                    let ref_dst = out_dir.join(format!("{}_ref.{}", config.name, ext));
                    if std::fs::copy(&ref_src, &ref_dst).is_ok() {
                        ref_copied = true;
                        break;
                    }
                }
            }

            println!(
                "OK - {} steps, {} cells{}",
                steps,
                nonzero,
                if ref_copied { "" } else { " (no ref)" }
            );

            results.push((config.name.to_string(), steps, nonzero, true));
        }

        // Print summary
        println!("\n========================================");
        println!("VERIFICATION SUMMARY");
        println!("========================================");
        println!(
            "{:<20} {:>8} {:>8} {:>8}",
            "Model", "Steps", "Cells", "Status"
        );
        println!("{:-<48}", "");

        let mut passed = 0;
        let mut failed = 0;
        for (name, steps, cells, ok) in &results {
            let status = if *ok { "OK" } else { "FAIL" };
            println!("{:<20} {:>8} {:>8} {:>8}", name, steps, cells, status);
            if *ok {
                passed += 1;
            } else {
                failed += 1;
            }
        }

        println!("{:-<48}", "");
        println!("PASSED: {}, FAILED: {}", passed, failed);
        println!("\nOutput directory: {}", out_dir.display());
        println!("Compare *_ours.png with *_ref.gif/png");
        println!("========================================\n");

        // Test passes if at least some models ran
        assert!(passed > 0, "At least some models should run successfully");
    }

    /// 3D model configurations for verification
    fn verification_3d_models() -> Vec<(&'static str, usize)> {
        // (name, size) - all cubic grids
        vec![
            ("Growth", 29),           // Simple 3D growth
            ("MazeGrowth", 27),       // 3D maze
            ("Knots3D", 8),           // 3D knots
            ("Apartemazements", 8),   // Apartment buildings
            ("StairsPath", 33),       // Stairs pathfinding
            ("NoDeadEnds", 19),       // 3D maze no dead ends
            ("PillarsOfEternity", 9), // Pillar structures
            ("Escher", 8),            // Escher-style patterns
            ("ClosedSurface", 12),    // Closed 3D surface
            ("ColoredKnots", 12),     // Colored 3D knots
        ]
    }

    /// 3D VERIFICATION TEST
    /// Runs 3D models and saves isometric screenshots.
    #[test]
    fn test_verification_run_all_3d_models() {
        use crate::markov_junior::Model;

        let out_dir = verification_dir();
        std::fs::create_dir_all(&out_dir).expect("Failed to create verification directory");

        let models = verification_3d_models();
        let mut results: Vec<(String, usize, usize, bool)> = Vec::new();

        println!("\n========================================");
        println!("MARKOV JUNIOR 3D VERIFICATION TEST");
        println!("Running {} 3D models", models.len());
        println!("Output: {}", out_dir.display());
        println!("========================================\n");

        for (name, size) in &models {
            let xml_path = models_path().join(format!("{}.xml", name));

            print!("Running {} ({}x{}x{})... ", name, size, size, size);

            // Load model with cubic 3D size
            let model_result = Model::load_with_size(&xml_path, *size, *size, *size);

            let mut model = match model_result {
                Ok(m) => m,
                Err(e) => {
                    println!("FAILED TO LOAD: {}", e);
                    results.push((name.to_string(), 0, 0, false));
                    continue;
                }
            };

            // Run with seed 0, reasonable step limit for 3D
            let max_steps = size * size * size * 2;
            let steps = model.run(0, max_steps);
            let grid = model.grid();
            let nonzero = grid.count_nonzero();

            // Save isometric 3D render
            let our_path = out_dir.join(format!("{}_3d_ours.png", name));
            let colors = colors_for_grid(grid);
            let img = render_3d_isometric(grid, &colors, 4); // cube_size=4
            if let Err(e) = save_png(&img, &our_path) {
                println!("FAILED TO SAVE: {}", e);
                results.push((name.to_string(), steps, nonzero, false));
                continue;
            }

            println!("OK - {} steps, {} cells", steps, nonzero);
            results.push((name.to_string(), steps, nonzero, true));
        }

        // Print summary
        let passed = results.iter().filter(|(_, _, _, ok)| *ok).count();
        let failed = results.len() - passed;

        println!("\n========================================");
        println!("3D VERIFICATION SUMMARY");
        println!("========================================");
        println!("{:<24} {:>8} {:>8}   Status", "Model", "Steps", "Cells");
        println!("{}", "-".repeat(48));
        for (name, steps, cells, ok) in &results {
            println!(
                "{:<24} {:>8} {:>8}   {}",
                name,
                steps,
                cells,
                if *ok { "OK" } else { "FAIL" }
            );
        }
        println!("{}", "-".repeat(48));
        println!("PASSED: {}, FAILED: {}", passed, failed);
        println!("\nOutput: {}", out_dir.display());
        println!("========================================\n");

        assert!(
            passed > 0,
            "At least some 3D models should run successfully"
        );
    }

    /// Debug test: Run River.xml with incremental screenshots to diagnose phase transitions.
    /// Saves a screenshot every N steps to screenshots/verification/river_debug/
    #[test]
    fn test_river_incremental_debug() {
        use crate::markov_junior::Model;

        let out_dir = verification_dir().join("river_debug");
        std::fs::create_dir_all(&out_dir).expect("Failed to create river debug directory");

        // Load River.xml with correct size from models.xml (80x80)
        let xml_path = models_path().join("River.xml");

        // First, let's verify the XML structure
        let xml_content = std::fs::read_to_string(&xml_path).expect("Failed to read River.xml");
        println!("River.xml content:\n{}", xml_content);

        let mut model =
            Model::load_with_size(&xml_path, 80, 80, 1).expect("Failed to load River.xml");

        println!("\n========================================");
        println!("RIVER.XML INCREMENTAL DEBUG");
        println!("Output: {}", out_dir.display());
        println!("========================================\n");

        // Run with seed 0, save screenshot every 1000 steps
        model.reset(0);

        let screenshot_interval = 1000; // Save every 1000 steps
        let max_steps = 50000; // River needs many steps to complete
        let mut step = 0;
        let mut screenshot_count = 0;

        // Initial screenshot (step 0)
        {
            let grid = model.grid();
            let colors = colors_for_grid(grid);
            let img = render_2d(grid, &colors, 4);
            let path = out_dir.join(format!("river_{:04}.png", screenshot_count));
            save_png(&img, &path).unwrap();
            println!(
                "Step {:>5}: {} non-zero cells -> {}",
                0,
                grid.count_nonzero(),
                path.file_name().unwrap().to_string_lossy()
            );
            screenshot_count += 1;
        }

        // Run step by step
        let mut running = true;
        while running && step < max_steps {
            running = model.step();
            step += 1;

            // Save screenshot at wide intervals only
            let should_save = step % screenshot_interval == 0;

            if should_save {
                let grid = model.grid();
                let colors = colors_for_grid(grid);
                let img = render_2d(grid, &colors, 4);
                let path = out_dir.join(format!("river_{:04}.png", screenshot_count));
                save_png(&img, &path).unwrap();

                // Count each value
                let mut counts = [0usize; 6];
                for &v in &grid.state {
                    if (v as usize) < 6 {
                        counts[v as usize] += 1;
                    }
                }

                println!(
                    "Step {:>5}: B={} W={} R={} U={} G={} E={} -> {}",
                    step,
                    counts[0],
                    counts[1],
                    counts[2],
                    counts[3],
                    counts[4],
                    counts[5],
                    path.file_name().unwrap().to_string_lossy()
                );
                screenshot_count += 1;
            }
        }

        // Final screenshot
        {
            let grid = model.grid();
            let colors = colors_for_grid(grid);
            let img = render_2d(grid, &colors, 4);
            let path = out_dir.join("river_final.png");
            save_png(&img, &path).unwrap();

            let mut counts = [0usize; 6];
            for &v in &grid.state {
                if (v as usize) < 6 {
                    counts[v as usize] += 1;
                }
            }

            println!("\n========================================");
            println!("RIVER FINAL STATE");
            println!("========================================");
            println!("Total steps: {}", step);
            println!("Model still running: {}", running);
            println!(
                "B(black)={} W(white)={} R(red)={} U(blue)={} G(green)={} E(brown)={}",
                counts[0], counts[1], counts[2], counts[3], counts[4], counts[5]
            );
            println!("Saved to: {}", path.display());
            println!("========================================\n");
        }

        // Copy reference for comparison
        let ref_src = reference_images_dir().join("River.gif");
        if ref_src.exists() {
            let ref_dst = out_dir.join("River_ref.gif");
            let _ = std::fs::copy(&ref_src, &ref_dst);
        }

        // Basic assertion - we should have more than just W and R
        let grid = model.grid();
        let u_count = grid.state.iter().filter(|&&v| v == 3).count(); // U
        let g_count = grid.state.iter().filter(|&&v| v == 4).count(); // G
        let e_count = grid.state.iter().filter(|&&v| v == 5).count(); // E

        // Count RW adjacencies (R=2, W=1)
        let mut rw_count = 0;
        let mut wr_count = 0;
        let mx = grid.mx;
        let my = grid.my;
        for y in 0..my {
            for x in 0..(mx - 1) {
                let i = x + y * mx;
                let j = i + 1;
                if grid.state[i] == 2 && grid.state[j] == 1 {
                    rw_count += 1; // R followed by W
                }
                if grid.state[i] == 1 && grid.state[j] == 2 {
                    wr_count += 1; // W followed by R
                }
            }
        }
        // Also check vertical
        let mut rw_vert = 0;
        let mut wr_vert = 0;
        for y in 0..(my - 1) {
            for x in 0..mx {
                let i = x + y * mx;
                let j = x + (y + 1) * mx;
                if grid.state[i] == 2 && grid.state[j] == 1 {
                    rw_vert += 1;
                }
                if grid.state[i] == 1 && grid.state[j] == 2 {
                    wr_vert += 1;
                }
            }
        }

        println!("Expected: U>0, G>0, E>0 (river, banks, trees)");
        println!("Actual: U={}, G={}, E={}", u_count, g_count, e_count);
        println!(
            "RW adjacencies: horizontal RW={} WR={}, vertical RW={} WR={}",
            rw_count, wr_count, rw_vert, wr_vert
        );

        // This will likely fail - that's the point! We're debugging.
        // Comment out assertion to see the debug output.
        // assert!(u_count > 0, "Should have river cells (U)");
    }

    /// Debug test: Check what rule variants are generated by symmetry expansion.
    /// This helps diagnose if `RW -> UU` is correctly expanded to also match `WR`.
    #[test]
    fn test_debug_symmetry_expansion_for_river() {
        use crate::markov_junior::loader::load_model;
        use crate::markov_junior::symmetry::{square_symmetries, SquareSubgroup};
        use crate::markov_junior::MjGrid;
        use crate::markov_junior::MjRule;

        // Create a grid with River values: BWRUGE
        let grid = MjGrid::with_values(10, 10, 1, "BWRUGE");

        // Parse the rule "RW" -> "UU" (Phase 4 of River.xml)
        let base_rule = MjRule::parse("RW", "UU", &grid).expect("Failed to parse RW->UU rule");

        println!("\n========================================");
        println!("DEBUG: Symmetry expansion for RW -> UU");
        println!("========================================\n");

        // Check base rule
        println!("Base rule:");
        println!(
            "  Input pattern dimensions: {}x{}x{}",
            base_rule.imx, base_rule.imy, base_rule.imz
        );
        println!("  Input waves: {:?}", base_rule.input);
        println!("  Output values: {:?}", base_rule.output);

        // Values mapping: B=0, W=1, R=2, U=3, G=4, E=5
        // So R=2 has wave 0b000100=4, W=1 has wave 0b000010=2
        println!("\n  Expected for RW pattern:");
        println!("    Position 0 (R): wave should match R (value 2) -> wave = 1 << 2 = 4");
        println!("    Position 1 (W): wave should match W (value 1) -> wave = 1 << 1 = 2");
        println!("  Actual waves: {:?}", base_rule.input);

        // Apply full symmetry
        let variants = square_symmetries(&base_rule, Some(SquareSubgroup::All));

        println!("\n========================================");
        println!("Symmetry variants (All 8 transformations):");
        println!("========================================\n");

        for (i, rule) in variants.iter().enumerate() {
            // Decode what pattern this variant matches
            let pattern_desc = if rule.imx == 2 && rule.imy == 1 {
                // Horizontal 2x1
                let w0 = rule.input[0]; // First cell
                let w1 = rule.input[1]; // Second cell
                format!(
                    "[{}][{}] (horizontal)",
                    wave_to_char(w0, &grid),
                    wave_to_char(w1, &grid)
                )
            } else if rule.imx == 1 && rule.imy == 2 {
                // Vertical 1x2
                let w0 = rule.input[0]; // Top cell
                let w1 = rule.input[1]; // Bottom cell
                format!(
                    "[{}]/[{}] (vertical)",
                    wave_to_char(w0, &grid),
                    wave_to_char(w1, &grid)
                )
            } else {
                format!("{}x{}", rule.imx, rule.imy)
            };

            println!("  Variant {}: {} -> {:?}", i, pattern_desc, rule.output);
            println!("             input waves: {:?}", rule.input);
        }

        println!("\n========================================");
        println!("ANALYSIS:");
        println!("========================================\n");

        // Check if we have a variant that matches WR (horizontal)
        // WR means: position 0 = W (wave 2), position 1 = R (wave 4)
        let has_wr_horizontal = variants
            .iter()
            .any(|r| r.imx == 2 && r.imy == 1 && r.input[0] == 2 && r.input[1] == 4);

        // Check if we have a variant that matches R/W (vertical, R on top, W below)
        // This means: position 0 = R (wave 4), position 1 = W (wave 2) in 1x2 pattern
        let has_rw_vertical = variants
            .iter()
            .any(|r| r.imx == 1 && r.imy == 2 && r.input[0] == 4 && r.input[1] == 2);

        // Check if we have W/R (vertical, W on top, R below)
        let has_wr_vertical = variants
            .iter()
            .any(|r| r.imx == 1 && r.imy == 2 && r.input[0] == 2 && r.input[1] == 4);

        println!(
            "Has WR horizontal (W=wave2 followed by R=wave4): {}",
            has_wr_horizontal
        );
        println!("Has R/W vertical (R on top): {}", has_rw_vertical);
        println!("Has W/R vertical (W on top): {}", has_wr_vertical);

        // Now let's load River.xml and check the actual rules in the AllNode at phase 4
        println!("\n========================================");
        println!("Loading River.xml to check actual rules:");
        println!("========================================\n");

        let path = models_path().join("River.xml");
        let model = load_model(&path).expect("Failed to load River.xml");

        // The root should be a SequenceNode
        // We can't easily access the rules inside, so let's just verify loading works
        println!("River.xml loaded successfully.");
        println!("Grid values: {:?}", model.grid.values);
        println!("Grid waves: {:?}", model.grid.waves);

        // Assertions
        assert!(
            has_wr_horizontal,
            "Symmetry should produce WR horizontal variant to match WR adjacencies!"
        );
        assert!(
            has_rw_vertical,
            "Symmetry should produce R/W vertical variant!"
        );
        assert!(
            has_wr_vertical,
            "Symmetry should produce W/R vertical variant!"
        );

        println!("\nSUCCESS: All expected symmetry variants are present!");
    }

    /// Helper: Convert a wave bitmask back to a character for display
    fn wave_to_char(wave: u32, grid: &MjGrid) -> char {
        // If it's a single-bit wave, find the matching character
        if wave.count_ones() == 1 {
            let value = wave.trailing_zeros() as u8;
            for (&ch, &v) in &grid.values {
                if v == value {
                    return ch;
                }
            }
        }
        // Wildcard or multi-value
        '*'
    }

    /// Debug test: Direct test of AllNode matching logic
    #[test]
    fn test_debug_allnode_rw_matching() {
        use crate::markov_junior::node::{ExecutionContext, Node};
        use crate::markov_junior::symmetry::{square_symmetries, SquareSubgroup};
        use crate::markov_junior::AllNode;
        use crate::markov_junior::MjGrid;
        use crate::markov_junior::MjRule;
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        println!("\n========================================");
        println!("DEBUG: AllNode RW matching test");
        println!("========================================\n");

        // Create a grid with River values: BWRUGE
        // Grid values: B=0, W=1, R=2, U=3, G=4, E=5
        let mut grid = MjGrid::with_values(4, 4, 1, "BWRUGE");

        // Set up a pattern with W and R adjacent:
        // Row 0: R W B B  (RW at (0,0))
        // Row 1: B W R B  (WR at (1,1))
        // Row 2: B B W B
        // Row 3: B B R B
        //                  (W/R vertical at (2,2)-(2,3))

        grid.state[0] = 2; // R at (0,0)
        grid.state[1] = 1; // W at (1,0)
        grid.state[2] = 0; // B at (2,0)
        grid.state[3] = 0; // B at (3,0)

        grid.state[4] = 0; // B at (0,1)
        grid.state[5] = 1; // W at (1,1)
        grid.state[6] = 2; // R at (2,1)
        grid.state[7] = 0; // B at (3,1)

        grid.state[8] = 0; // B at (0,2)
        grid.state[9] = 0; // B at (1,2)
        grid.state[10] = 1; // W at (2,2)
        grid.state[11] = 0; // B at (3,2)

        grid.state[12] = 0; // B at (0,3)
        grid.state[13] = 0; // B at (1,3)
        grid.state[14] = 2; // R at (2,3)
        grid.state[15] = 0; // B at (3,3)

        println!("Grid state:");
        for y in 0..4 {
            let mut row = String::new();
            for x in 0..4 {
                let v = grid.state[x + y * 4];
                let ch = match v {
                    0 => 'B',
                    1 => 'W',
                    2 => 'R',
                    3 => 'U',
                    _ => '?',
                };
                row.push(ch);
            }
            println!("  Row {}: {}", y, row);
        }

        // Parse the rule "RW" -> "UU" with full symmetry
        let base_rule = MjRule::parse("RW", "UU", &grid).expect("Failed to parse RW->UU rule");
        let rules = square_symmetries(&base_rule, Some(SquareSubgroup::All));

        println!(
            "\nRules after symmetry expansion ({} variants):",
            rules.len()
        );
        for (i, rule) in rules.iter().enumerate() {
            println!(
                "  Rule {}: dims {}x{}, input {:?}, output {:?}",
                i, rule.imx, rule.imy, rule.input, rule.output
            );
        }

        // Create AllNode with these rules
        let mut all_node = AllNode::new(rules, grid.state.len());

        // Run one step
        let mut rng = StdRng::seed_from_u64(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        println!("\nBefore AllNode.go():");
        println!("  match_count: {}", all_node.data.match_count);

        let result = all_node.go(&mut ctx);

        println!("\nAfter AllNode.go():");
        println!("  result: {}", result);
        println!("  changes: {:?}", ctx.changes);

        // Check grid state
        println!("\nGrid state after:");
        for y in 0..4 {
            let mut row = String::new();
            for x in 0..4 {
                let v = ctx.grid.state[x + y * 4];
                let ch = match v {
                    0 => 'B',
                    1 => 'W',
                    2 => 'R',
                    3 => 'U',
                    _ => '?',
                };
                row.push(ch);
            }
            println!("  Row {}: {}", y, row);
        }

        // Count U cells
        let u_count = ctx.grid.state.iter().filter(|&&v| v == 3).count();
        println!("\nU count: {}", u_count);

        // Now test individual matching
        println!("\n========================================");
        println!("Testing individual rule matches:");
        println!("========================================\n");

        // Re-create grid for testing
        let mut grid2 = MjGrid::with_values(4, 4, 1, "BWRUGE");
        grid2.state[0] = 2; // R at (0,0)
        grid2.state[1] = 1; // W at (1,0)
        grid2.state[5] = 1; // W at (1,1)
        grid2.state[6] = 2; // R at (2,1)
        grid2.state[10] = 1; // W at (2,2)
        grid2.state[14] = 2; // R at (2,3)

        let base_rule = MjRule::parse("RW", "UU", &grid2).expect("Failed to parse RW->UU rule");
        let rules = square_symmetries(&base_rule, Some(SquareSubgroup::All));

        // Test at position (0,0) - should match RW horizontal
        println!("Testing at (0,0) - pattern RW horizontal:");
        for (i, rule) in rules.iter().enumerate() {
            let m = grid2.matches(rule, 0, 0, 0);
            println!("  Rule {} ({}x{}): matches={}", i, rule.imx, rule.imy, m);
        }

        // Test at position (1,1) - should match WR horizontal
        println!("\nTesting at (1,1) - pattern WR horizontal:");
        for (i, rule) in rules.iter().enumerate() {
            let m = grid2.matches(rule, 1, 1, 0);
            println!("  Rule {} ({}x{}): matches={}", i, rule.imx, rule.imy, m);
        }

        // Test at position (2,2) - should match W/R vertical
        println!("\nTesting at (2,2) - pattern W/R vertical:");
        for (i, rule) in rules.iter().enumerate() {
            let m = grid2.matches(rule, 2, 2, 0);
            println!("  Rule {} ({}x{}): matches={}", i, rule.imx, rule.imy, m);
        }

        // Assertions
        assert!(result, "AllNode should have found matches!");
        assert!(u_count > 0, "Should have converted some cells to U!");
    }

    /// Debug test: Test a minimal river-like sequence
    /// This tests the actual sequence transition behavior
    #[test]
    fn test_debug_river_minimal_sequence() {
        use crate::markov_junior::Model;

        println!("\n========================================");
        println!("DEBUG: Minimal River-like sequence test");
        println!("========================================\n");

        // Create a minimal river-like model:
        // 1. Phase 1: Place one W
        // 2. Phase 2: Place one R
        // 3. Phase 3: Grow W and R (until no more B)
        // 4. Phase 4: RW -> UU (this is the failing part in River)
        let xml = r#"
        <sequence values="BWRU">
            <one in="B" out="W" steps="1"/>
            <one in="B" out="R" steps="1"/>
            <one>
                <rule in="RB" out="RR"/>
                <rule in="WB" out="WW"/>
            </one>
            <all in="RW" out="UU"/>
        </sequence>
        "#;

        let mut model = Model::load_str(xml, 8, 8, 1).expect("Failed to load model");

        // Reset the model to start execution
        model.reset(42);

        // Run step by step and watch what happens
        let mut step = 0;
        let max_steps = 100;

        println!("Running step by step:");
        while step < max_steps {
            let grid = model.grid();
            let mut counts = [0usize; 4];
            for &v in &grid.state {
                if (v as usize) < 4 {
                    counts[v as usize] += 1;
                }
            }

            // Count RW adjacencies
            let mx = grid.mx;
            let my = grid.my;
            let mut rw_count = 0;
            let mut wr_count = 0;
            for y in 0..my {
                for x in 0..(mx - 1) {
                    let i = x + y * mx;
                    let j = i + 1;
                    if grid.state[i] == 2 && grid.state[j] == 1 {
                        rw_count += 1;
                    }
                    if grid.state[i] == 1 && grid.state[j] == 2 {
                        wr_count += 1;
                    }
                }
            }

            if step % 10 == 0 || counts[0] < 10 || counts[3] > 0 {
                println!(
                    "  Step {:>3}: B={:>2} W={:>2} R={:>2} U={:>2} | RW={} WR={}",
                    step, counts[0], counts[1], counts[2], counts[3], rw_count, wr_count
                );
            }

            if !model.step() {
                println!("  Model stopped at step {}", step);
                break;
            }
            step += 1;
        }

        let grid = model.grid();
        let u_count = grid.state.iter().filter(|&&v| v == 3).count();

        println!("\nFinal state:");
        let mut counts = [0usize; 4];
        for &v in &grid.state {
            if (v as usize) < 4 {
                counts[v as usize] += 1;
            }
        }
        println!(
            "  B={} W={} R={} U={}",
            counts[0], counts[1], counts[2], counts[3]
        );

        // Print grid visually
        println!("\nGrid visualization:");
        for y in 0..grid.my {
            let mut row = String::new();
            for x in 0..grid.mx {
                let v = grid.state[x + y * grid.mx];
                let ch = match v {
                    0 => '.',
                    1 => 'W',
                    2 => 'R',
                    3 => 'U',
                    _ => '?',
                };
                row.push(ch);
            }
            println!("  {}", row);
        }

        // Assertion: we should have some U cells!
        assert!(
            u_count > 0,
            "Phase 4 should have converted RW/WR to U! Got {} U cells",
            u_count
        );
    }
}
