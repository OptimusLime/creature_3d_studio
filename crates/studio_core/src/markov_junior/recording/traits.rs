//! Traits for recordable grids and renderers.
//!
//! These traits allow generic recording and rendering of any MarkovJunior grid type.

use super::grid_type::GridType;
use image::RgbaImage;

/// Trait for any grid that can be recorded to a simulation archive.
///
/// Implement this trait for any grid type (MjGrid, PolarMjGrid, etc.)
/// to enable simulation recording and playback.
pub trait RecordableGrid {
    /// Get the grid type and dimensions.
    fn grid_type(&self) -> GridType;

    /// Get the palette/values string (e.g., "BWR" for black/white/red).
    fn palette(&self) -> String;

    /// Serialize the current grid state to bytes.
    ///
    /// The byte format should be consistent: one u8 per cell,
    /// in a deterministic order defined by the grid type.
    fn state_to_bytes(&self) -> Vec<u8>;

    /// Deserialize grid state from bytes.
    ///
    /// Returns true if successful, false if bytes don't match expected size.
    fn state_from_bytes(&mut self, bytes: &[u8]) -> bool;

    /// Number of bytes per frame (should match grid_type().bytes_per_frame()).
    fn bytes_per_frame(&self) -> usize {
        self.grid_type().bytes_per_frame()
    }
}

/// Trait for grids that can be rendered to a 2D image.
///
/// Implement this for Cartesian2D and Polar2D grids.
pub trait Renderable2D: RecordableGrid {
    /// Render the grid to an RGBA image.
    ///
    /// # Arguments
    /// * `image_size` - Width and height of the output image in pixels
    /// * `colors` - Color palette mapping value index to RGBA
    /// * `background` - Background color for empty areas or out-of-bounds
    fn render_to_image(
        &self,
        image_size: u32,
        colors: &[[u8; 4]],
        background: [u8; 4],
    ) -> RgbaImage;
}

/// Voxel data for 3D rendering.
///
/// Simple representation of colored voxels in 3D space.
#[derive(Debug, Clone)]
pub struct VoxelData {
    /// Dimensions (x, y, z)
    pub dimensions: (u32, u32, u32),
    /// Voxel values (flattened, index = x + y*mx + z*mx*my)
    pub values: Vec<u8>,
}

/// Trait for grids that can be rendered to 3D voxel data.
///
/// Implement this for Cartesian3D and Polar3D grids.
pub trait Renderable3D: RecordableGrid {
    /// Render the grid to voxel data.
    ///
    /// # Arguments
    /// * `colors` - Color palette mapping value index to RGBA
    fn render_to_voxels(&self, colors: &[[u8; 4]]) -> VoxelData;
}

/// Helper to generate default colors from a palette string.
///
/// Maps common characters to colors:
/// - 'B', 'X' -> Black (transparent)
/// - 'W' -> White
/// - 'R' -> Red
/// - 'G' -> Green
/// - 'M' -> Magenta
/// - 'Y' -> Yellow
/// - 'C' -> Cyan
/// - 'O' -> Orange
/// - Others -> grayscale based on index
pub fn default_colors_for_palette(palette: &str) -> Vec<[u8; 4]> {
    palette
        .chars()
        .enumerate()
        .map(|(i, c)| match c {
            'B' | 'X' => [0, 0, 0, 0],   // Black/transparent
            'W' => [255, 255, 255, 255], // White
            'R' => [255, 80, 80, 255],   // Red
            'G' => [80, 200, 80, 255],   // Green
            'M' => [255, 80, 255, 255],  // Magenta
            'Y' => [255, 200, 50, 255],  // Yellow
            'C' => [80, 200, 255, 255],  // Cyan
            'O' => [255, 140, 40, 255],  // Orange
            'P' => [180, 100, 255, 255], // Purple
            'N' => [100, 80, 60, 255],   // Brown
            _ => {
                // Grayscale based on index
                let v = ((i as u32 * 37 + 100) % 200 + 55) as u8;
                [v, v, v, 255]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_colors() {
        let colors = default_colors_for_palette("BWRGMYC");
        assert_eq!(colors.len(), 7);
        assert_eq!(colors[0], [0, 0, 0, 0]); // B = transparent
        assert_eq!(colors[1], [255, 255, 255, 255]); // W = white
        assert_eq!(colors[2], [255, 80, 80, 255]); // R = red
    }
}
