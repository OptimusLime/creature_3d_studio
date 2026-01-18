//! 2D Voxel Buffer
//!
//! A simple 2D grid of material IDs for the map editor.

use bevy::prelude::*;

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
}
