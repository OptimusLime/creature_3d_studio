//! Grid operations trait for abstracting over coordinate systems.
//!
//! This trait allows the MarkovJunior system to work with both Cartesian (x,y,z)
//! and Polar/Spherical (r,θ,φ) coordinate systems through a unified interface.
//!
//! The key insight is that most MJ logic operates on flat indices into the state
//! array, not on coordinates directly. This trait exposes that flat-index interface
//! while allowing each grid type to implement its own coordinate-to-index mapping.

/// Core operations that any MJ-compatible grid must support.
///
/// This trait abstracts over coordinate systems while preserving
/// the flat-index access pattern used throughout the codebase.
///
/// # Implementors
/// - `MjGrid` - Cartesian (x,y,z) coordinate system
/// - `SphericalMjGrid` - Polar/Spherical (r,θ,φ) coordinate system (future)
pub trait MjGridOps {
    // === Dimensions ===

    /// Total number of cells in the grid.
    fn len(&self) -> usize;

    /// Whether grid is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether grid is 2D (used for symmetry selection).
    ///
    /// For Cartesian: `mz == 1`
    /// For Spherical: `phi_divisions == 1`
    fn is_2d(&self) -> bool;

    // === State Access ===

    /// Get cell value at flat index.
    fn get_state(&self, idx: usize) -> u8;

    /// Set cell value at flat index.
    fn set_state(&mut self, idx: usize, value: u8);

    /// Get entire state as slice (for bulk operations).
    fn state(&self) -> &[u8];

    /// Get mutable state as slice.
    fn state_mut(&mut self) -> &mut [u8];

    // === Value System ===

    /// Number of distinct values/colors.
    fn num_values(&self) -> u8;

    /// Get index for character (e.g., 'B' -> 0).
    fn value_for_char(&self, ch: char) -> Option<u8>;

    /// Get character for index (e.g., 0 -> 'B').
    fn char_for_value(&self, val: u8) -> Option<char>;

    /// Get wave bitmask for character.
    fn wave_for_char(&self, ch: char) -> Option<u32>;

    /// Get combined wave for string (e.g., "BW" -> 0b11).
    fn wave(&self, chars: &str) -> u32;

    // === Mask (for AllNode non-overlap checking) ===

    /// Get mask value at flat index.
    fn get_mask(&self, idx: usize) -> bool;

    /// Set mask value at flat index.
    fn set_mask(&mut self, idx: usize, value: bool);

    /// Clear all mask values.
    fn clear_mask(&mut self);

    // === Coordinate System Info ===

    /// Dimension sizes as (d0, d1, d2).
    ///
    /// Interpretation varies by grid type:
    /// - Cartesian: (mx, my, mz)
    /// - Spherical: (theta_divs, phi_divs, r_depth)
    fn dimensions(&self) -> (usize, usize, usize);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::MjGrid;

    /// Test that the trait is object-safe (can be used as dyn MjGridOps)
    #[test]
    fn test_trait_is_object_safe() {
        // This test just needs to compile - if MjGridOps is not object-safe,
        // this function signature would fail to compile
        fn _takes_dyn_grid(_grid: &dyn MjGridOps) {}
        fn _takes_dyn_grid_mut(_grid: &mut dyn MjGridOps) {}
    }

    // ========================================================================
    // Tests for MjGridOps implementation on MjGrid
    // ========================================================================

    #[test]
    fn test_mjgrid_len_and_is_empty() {
        let grid = MjGrid::with_values(4, 4, 1, "BW");
        assert_eq!(grid.len(), 16);
        assert!(!grid.is_empty());

        let grid_3d = MjGrid::with_values(2, 3, 4, "BW");
        assert_eq!(grid_3d.len(), 24);
    }

    #[test]
    fn test_mjgrid_is_2d() {
        let grid_2d = MjGrid::with_values(4, 4, 1, "BW");
        assert!(grid_2d.is_2d());

        let grid_3d = MjGrid::with_values(4, 4, 2, "BW");
        assert!(!grid_3d.is_2d());
    }

    #[test]
    fn test_mjgrid_state_access() {
        let mut grid = MjGrid::with_values(3, 3, 1, "BW");

        // Initial state is all zeros
        assert_eq!(grid.get_state(0), 0);
        assert_eq!(grid.get_state(4), 0); // center

        // Set via trait method
        grid.set_state(4, 1);
        assert_eq!(grid.get_state(4), 1);

        // Verify state slice
        assert_eq!(grid.state().len(), 9);
        assert_eq!(grid.state()[4], 1);
    }

    #[test]
    fn test_mjgrid_state_mut() {
        let mut grid = MjGrid::with_values(2, 2, 1, "BW");

        // Modify via mutable slice
        grid.state_mut()[0] = 1;
        grid.state_mut()[3] = 1;

        assert_eq!(grid.get_state(0), 1);
        assert_eq!(grid.get_state(1), 0);
        assert_eq!(grid.get_state(2), 0);
        assert_eq!(grid.get_state(3), 1);
    }

    #[test]
    fn test_mjgrid_value_system() {
        let grid = MjGrid::with_values(3, 3, 1, "BRGW");

        // num_values
        assert_eq!(grid.num_values(), 4);

        // value_for_char
        assert_eq!(grid.value_for_char('B'), Some(0));
        assert_eq!(grid.value_for_char('R'), Some(1));
        assert_eq!(grid.value_for_char('G'), Some(2));
        assert_eq!(grid.value_for_char('W'), Some(3));
        assert_eq!(grid.value_for_char('X'), None);

        // char_for_value
        assert_eq!(grid.char_for_value(0), Some('B'));
        assert_eq!(grid.char_for_value(1), Some('R'));
        assert_eq!(grid.char_for_value(2), Some('G'));
        assert_eq!(grid.char_for_value(3), Some('W'));
        assert_eq!(grid.char_for_value(4), None);
    }

    #[test]
    fn test_mjgrid_wave_system() {
        let grid = MjGrid::with_values(3, 3, 1, "BW");

        // wave_for_char
        assert_eq!(grid.wave_for_char('B'), Some(0b01));
        assert_eq!(grid.wave_for_char('W'), Some(0b10));
        assert_eq!(grid.wave_for_char('*'), Some(0b11)); // wildcard

        // wave (combined)
        assert_eq!(grid.wave("B"), 0b01);
        assert_eq!(grid.wave("W"), 0b10);
        assert_eq!(grid.wave("BW"), 0b11);
        assert_eq!(grid.wave("WB"), 0b11); // order independent
    }

    #[test]
    fn test_mjgrid_mask_operations() {
        let mut grid = MjGrid::with_values(3, 3, 1, "BW");

        // Initial mask is all false
        assert!(!grid.get_mask(0));
        assert!(!grid.get_mask(4));

        // Set mask values
        grid.set_mask(0, true);
        grid.set_mask(4, true);
        assert!(grid.get_mask(0));
        assert!(grid.get_mask(4));
        assert!(!grid.get_mask(1));

        // Clear mask
        grid.clear_mask();
        assert!(!grid.get_mask(0));
        assert!(!grid.get_mask(4));
    }

    #[test]
    fn test_mjgrid_dimensions() {
        let grid_2d = MjGrid::with_values(5, 7, 1, "BW");
        assert_eq!(grid_2d.dimensions(), (5, 7, 1));

        let grid_3d = MjGrid::with_values(3, 4, 5, "BW");
        assert_eq!(grid_3d.dimensions(), (3, 4, 5));
    }

    #[test]
    fn test_mjgrid_via_trait_object() {
        // Test that we can use MjGrid through a trait object
        let mut grid = MjGrid::with_values(3, 3, 1, "BW");

        fn check_grid(grid: &dyn MjGridOps) {
            assert_eq!(grid.len(), 9);
            assert!(grid.is_2d());
            assert_eq!(grid.dimensions(), (3, 3, 1));
            assert_eq!(grid.num_values(), 2);
        }

        fn mutate_grid(grid: &mut dyn MjGridOps) {
            grid.set_state(4, 1);
            grid.set_mask(4, true);
        }

        check_grid(&grid);
        mutate_grid(&mut grid);

        assert_eq!(grid.get_state(4), 1);
        assert!(grid.get_mask(4));
    }

    #[test]
    fn test_mjgrid_trait_consistency_with_direct_access() {
        // Verify trait methods return same results as direct MjGrid methods
        let mut grid = MjGrid::with_values(4, 4, 1, "BRGW");

        // Set some values directly
        grid.set(1, 1, 0, 2); // G at (1,1)
        grid.set(2, 2, 0, 3); // W at (2,2)

        // Verify via trait methods - index = x + y * mx
        let idx_11 = 1 + 1 * 4; // 5
        let idx_22 = 2 + 2 * 4; // 10

        assert_eq!(grid.get_state(idx_11), 2);
        assert_eq!(grid.get_state(idx_22), 3);

        // Verify dimensions match
        assert_eq!(grid.dimensions(), (grid.mx, grid.my, grid.mz));

        // Verify len matches state length
        assert_eq!(grid.len(), grid.state.len());
    }
}
