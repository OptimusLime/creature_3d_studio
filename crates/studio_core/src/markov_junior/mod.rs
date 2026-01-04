//! MarkovJunior procedural generation system.
//!
//! A Rust port of the MarkovJunior probabilistic programming language for
//! procedural content generation using rewrite rules.
//!
//! This module provides:
//! - `MjGrid`: Core grid structure for storing voxel states
//! - `voxel_bridge`: Conversion from MjGrid to VoxelWorld for rendering
//!
//! ## Example
//!
//! ```ignore
//! use studio_core::markov_junior::{MjGrid, MjPalette};
//!
//! // Create a 5x5x1 grid
//! let mut grid = MjGrid::new(5, 5, 1);
//!
//! // Set some values (0 = empty, 1 = filled)
//! grid.set(2, 2, 0, 1); // center
//! grid.set(1, 2, 0, 1); // left
//! grid.set(3, 2, 0, 1); // right
//! grid.set(2, 1, 0, 1); // down
//! grid.set(2, 3, 0, 1); // up
//!
//! // Convert to VoxelWorld
//! let palette = MjPalette::default();
//! let world = grid.to_voxel_world(&palette);
//! ```

pub mod voxel_bridge;

pub use voxel_bridge::{to_voxel_world, MjPalette};

/// A 3D grid of voxel states for MarkovJunior.
///
/// The grid stores u8 values where:
/// - 0 typically represents empty/transparent
/// - 1+ represent different materials/colors
///
/// Indexing follows MarkovJunior convention: `index = x + y * mx + z * mx * my`
#[derive(Debug, Clone)]
pub struct MjGrid {
    /// Flat array of voxel states
    pub state: Vec<u8>,
    /// Width (X dimension)
    pub mx: usize,
    /// Height (Y dimension)  
    pub my: usize,
    /// Depth (Z dimension)
    pub mz: usize,
}

impl MjGrid {
    /// Create a new grid filled with zeros.
    pub fn new(mx: usize, my: usize, mz: usize) -> Self {
        Self {
            state: vec![0; mx * my * mz],
            mx,
            my,
            mz,
        }
    }

    /// Get the linear index for (x, y, z) coordinates.
    /// Returns None if out of bounds.
    #[inline]
    pub fn index(&self, x: usize, y: usize, z: usize) -> Option<usize> {
        if x < self.mx && y < self.my && z < self.mz {
            Some(x + y * self.mx + z * self.mx * self.my)
        } else {
            None
        }
    }

    /// Get the value at (x, y, z), or None if out of bounds.
    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<u8> {
        self.index(x, y, z).map(|i| self.state[i])
    }

    /// Set the value at (x, y, z). Returns false if out of bounds.
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, value: u8) -> bool {
        if let Some(i) = self.index(x, y, z) {
            self.state[i] = value;
            true
        } else {
            false
        }
    }

    /// Count voxels with non-zero values.
    pub fn count_nonzero(&self) -> usize {
        self.state.iter().filter(|&&v| v != 0).count()
    }

    /// Iterate over all non-zero voxels with their (x, y, z) coordinates.
    pub fn iter_nonzero(&self) -> impl Iterator<Item = (usize, usize, usize, u8)> + '_ {
        self.state.iter().enumerate().filter_map(|(i, &v)| {
            if v != 0 {
                let x = i % self.mx;
                let y = (i / self.mx) % self.my;
                let z = i / (self.mx * self.my);
                Some((x, y, z, v))
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_new() {
        let grid = MjGrid::new(5, 5, 1);
        assert_eq!(grid.mx, 5);
        assert_eq!(grid.my, 5);
        assert_eq!(grid.mz, 1);
        assert_eq!(grid.state.len(), 25);
        assert!(grid.state.iter().all(|&v| v == 0));
    }

    #[test]
    fn test_grid_set_get() {
        let mut grid = MjGrid::new(5, 5, 1);
        assert!(grid.set(2, 2, 0, 1));
        assert_eq!(grid.get(2, 2, 0), Some(1));
        assert_eq!(grid.get(0, 0, 0), Some(0));
        assert_eq!(grid.get(10, 0, 0), None); // out of bounds
    }

    #[test]
    fn test_grid_index() {
        let grid = MjGrid::new(3, 3, 2);
        // x + y * mx + z * mx * my
        assert_eq!(grid.index(0, 0, 0), Some(0));
        assert_eq!(grid.index(1, 0, 0), Some(1));
        assert_eq!(grid.index(0, 1, 0), Some(3)); // y=1 -> +mx
        assert_eq!(grid.index(0, 0, 1), Some(9)); // z=1 -> +mx*my
        assert_eq!(grid.index(3, 0, 0), None); // out of bounds
    }

    #[test]
    fn test_grid_count_nonzero() {
        let mut grid = MjGrid::new(5, 5, 1);
        assert_eq!(grid.count_nonzero(), 0);
        grid.set(2, 2, 0, 1);
        grid.set(1, 2, 0, 1);
        grid.set(3, 2, 0, 1);
        assert_eq!(grid.count_nonzero(), 3);
    }

    #[test]
    fn test_grid_iter_nonzero() {
        let mut grid = MjGrid::new(3, 3, 1);
        grid.set(1, 1, 0, 5);
        grid.set(2, 0, 0, 3);

        let nonzero: Vec<_> = grid.iter_nonzero().collect();
        assert_eq!(nonzero.len(), 2);
        assert!(nonzero.contains(&(1, 1, 0, 5)));
        assert!(nonzero.contains(&(2, 0, 0, 3)));
    }
}
