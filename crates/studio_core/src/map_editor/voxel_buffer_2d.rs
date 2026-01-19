//! 2D Voxel Buffer and VoxelGrid trait
//!
//! Provides the `VoxelGrid2D` trait for abstracting over different grid implementations,
//! and `VoxelBuffer2D` as the primary mutable buffer for the map editor.
//!
//! # VoxelGrid2D Trait
//!
//! The trait enables zero-copy rendering from different sources:
//! - `VoxelBuffer2D`: Direct access to material IDs
//! - `MjGridView`: Translation-on-read from Markov Jr. grids
//!
//! Renderers and MCP endpoints read from `&dyn VoxelGrid2D`, allowing them to work
//! with any grid implementation without copying data.

use bevy::prelude::*;

/// Trait for anything that provides 2D voxel/material data.
///
/// Implementations:
/// - `VoxelBuffer2D`: Direct access (for Lua generators)
/// - `MjGridView`: Translation-on-read (for Markov Jr. generators)
///
/// This trait enables zero-copy rendering: instead of copying MjGrid → SharedBuffer → VoxelBuffer2D,
/// we read directly from the source with translation happening on each `get()` call.
///
/// # Future: VoxelGrid3D
///
/// The same pattern extends to 3D in Phase 5:
/// ```ignore
/// pub trait VoxelGrid3D {
///     fn size(&self) -> (usize, usize, usize);
///     fn get(&self, x: usize, y: usize, z: usize) -> u32;
/// }
/// ```
pub trait VoxelGrid2D {
    /// Width of the grid in cells.
    fn width(&self) -> usize;

    /// Height of the grid in cells.
    fn height(&self) -> usize;

    /// Get the material ID at position (x, y).
    ///
    /// Returns 0 if coordinates are out of bounds.
    fn get(&self, x: usize, y: usize) -> u32;
}

/// A 2D grid of material IDs.
///
/// Each cell stores a `u32` material ID. ID 0 typically means "empty".
#[derive(Resource, Clone)]
pub struct VoxelBuffer2D {
    /// Width of the buffer in cells.
    pub width: usize,
    /// Height of the buffer in cells.
    pub height: usize,
    /// Flat array of material IDs (row-major order).
    pub data: Vec<u32>,
}

impl VoxelBuffer2D {
    /// Create a new buffer filled with zeros (empty).
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0; width * height],
        }
    }

    /// Set the material ID at position (x, y).
    ///
    /// Does nothing if coordinates are out of bounds.
    pub fn set(&mut self, x: usize, y: usize, material_id: u32) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = material_id;
        }
    }

    /// Get the material ID at position (x, y).
    ///
    /// Returns 0 if coordinates are out of bounds.
    pub fn get(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.data[y * self.width + x]
        } else {
            0
        }
    }

    /// Clear the buffer, setting all cells to zero.
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    /// Total number of cells in the buffer.
    pub fn cell_count(&self) -> usize {
        self.width * self.height
    }
}

impl VoxelGrid2D for VoxelBuffer2D {
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn get(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.data[y * self.width + x]
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buf = VoxelBuffer2D::new(4, 4);
        assert_eq!(buf.width, 4);
        assert_eq!(buf.height, 4);
        assert_eq!(buf.data.len(), 16);
        assert!(buf.data.iter().all(|&v| v == 0));
    }

    #[test]
    fn test_set_get() {
        let mut buf = VoxelBuffer2D::new(4, 4);
        buf.set(1, 2, 42);
        assert_eq!(buf.get(1, 2), 42);
        assert_eq!(buf.get(0, 0), 0);
    }

    #[test]
    fn test_out_of_bounds() {
        let mut buf = VoxelBuffer2D::new(4, 4);
        buf.set(10, 10, 99); // Should do nothing
        assert_eq!(buf.get(10, 10), 0);
    }

    #[test]
    fn test_voxel_grid_2d_trait() {
        let mut buf = VoxelBuffer2D::new(4, 4);
        buf.set(1, 2, 42);

        // Access via trait
        let grid: &dyn VoxelGrid2D = &buf;
        assert_eq!(grid.width(), 4);
        assert_eq!(grid.height(), 4);
        assert_eq!(grid.get(1, 2), 42);
        assert_eq!(grid.get(0, 0), 0);
        assert_eq!(grid.get(10, 10), 0); // out of bounds
    }
}
