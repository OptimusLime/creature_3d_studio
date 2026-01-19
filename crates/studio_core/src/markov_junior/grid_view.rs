//! MjGridView - Zero-copy view into MjGrid with material ID translation.
//!
//! This module provides `MjGridView`, which implements `VoxelGrid2D` and translates
//! Markov Jr. grid values to material IDs on read. This enables zero-copy rendering
//! directly from MjGrid without intermediate buffer copies.
//!
//! # Translation
//!
//! MjGrid stores values as indices (0, 1, 2...) that map to characters ('B', 'W', 'R'...).
//! The map editor uses material IDs (u32). The translation is:
//!
//! ```text
//! MjGrid value (u8) → character (char) → material ID (u32)
//! ```
//!
//! This translation is pre-computed when creating the view, so `get()` is just
//! an array lookup.
//!
//! # Example
//!
//! ```ignore
//! use std::collections::HashMap;
//! use studio_core::markov_junior::{MjGrid, MjGridView};
//! use studio_core::map_editor::VoxelGrid2D;
//!
//! let grid = MjGrid::with_values(4, 4, 1, "BW");
//! // ... fill grid ...
//!
//! // Create character → material mapping
//! let mut char_to_material = HashMap::new();
//! char_to_material.insert('B', 1); // Black → material 1
//! char_to_material.insert('W', 2); // White → material 2
//!
//! let view = MjGridView::new(&grid, &char_to_material);
//!
//! // Now use as VoxelGrid2D
//! let material = view.get(0, 0); // Returns material ID, not grid value
//! ```

use super::MjGrid;
use crate::map_editor::VoxelGrid2D;
use std::collections::HashMap;

/// Zero-copy view into MjGrid that translates values to material IDs.
///
/// Implements `VoxelGrid2D` for use with the rendering system.
/// Translation happens on each `get()` call via a pre-computed lookup table.
pub struct MjGridView<'a> {
    /// Reference to the underlying MjGrid.
    grid: &'a MjGrid,
    /// Pre-computed mapping from grid values (0, 1, 2...) to material IDs.
    /// Index is the grid value, value is the material ID.
    value_to_material: Vec<u32>,
}

impl<'a> MjGridView<'a> {
    /// Create a new view with the given character-to-material mapping.
    ///
    /// # Arguments
    ///
    /// * `grid` - The MjGrid to view
    /// * `char_to_material` - Mapping from MJ characters ('B', 'W', etc.) to material IDs
    ///
    /// # Fallback Behavior
    ///
    /// If a character doesn't have a mapping in `char_to_material`, the material ID
    /// defaults to `index + 1` (so value 0 → material 1, value 1 → material 2, etc.).
    /// This maintains backward compatibility with code that doesn't set up explicit mappings.
    pub fn new(grid: &'a MjGrid, char_to_material: &HashMap<char, u32>) -> Self {
        // Pre-compute value→material mapping from grid.characters
        let value_to_material: Vec<u32> = grid
            .characters
            .iter()
            .enumerate()
            .map(|(i, &ch)| {
                // Look up material ID for this character
                // Fall back to index+1 if no mapping exists (legacy behavior)
                char_to_material.get(&ch).copied().unwrap_or(i as u32 + 1)
            })
            .collect();

        Self {
            grid,
            value_to_material,
        }
    }

    /// Create a view with default material mapping (value + 1).
    ///
    /// Useful for testing or when no explicit character mapping is needed.
    pub fn with_default_mapping(grid: &'a MjGrid) -> Self {
        Self::new(grid, &HashMap::new())
    }

    /// Get the underlying grid reference.
    pub fn grid(&self) -> &MjGrid {
        self.grid
    }
}

impl VoxelGrid2D for MjGridView<'_> {
    fn width(&self) -> usize {
        self.grid.mx
    }

    fn height(&self) -> usize {
        self.grid.my
    }

    fn get(&self, x: usize, y: usize) -> u32 {
        // Get grid value at (x, y, z=0) for 2D
        // Return 0 for out-of-bounds (matches VoxelBuffer2D behavior)
        match self.grid.get(x, y, 0) {
            Some(val) => {
                let val = val as usize;
                self.value_to_material.get(val).copied().unwrap_or(0)
            }
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mj_grid_view_basic() {
        let mut grid = MjGrid::with_values(4, 4, 1, "BW");
        // Set some values: B=0, W=1
        grid.set(0, 0, 0, 0); // B
        grid.set(1, 0, 0, 1); // W
        grid.set(2, 0, 0, 0); // B
        grid.set(3, 0, 0, 1); // W

        // Create mapping: B→10, W→20
        let mut mapping = HashMap::new();
        mapping.insert('B', 10u32);
        mapping.insert('W', 20u32);

        let view = MjGridView::new(&grid, &mapping);

        // Verify dimensions
        assert_eq!(view.width(), 4);
        assert_eq!(view.height(), 4);

        // Verify translation
        assert_eq!(view.get(0, 0), 10); // B → 10
        assert_eq!(view.get(1, 0), 20); // W → 20
        assert_eq!(view.get(2, 0), 10); // B → 10
        assert_eq!(view.get(3, 0), 20); // W → 20
    }

    #[test]
    fn test_mj_grid_view_default_mapping() {
        let grid = MjGrid::with_values(2, 2, 1, "BW");
        let view = MjGridView::with_default_mapping(&grid);

        // Default mapping: value 0 → material 1, value 1 → material 2
        // Grid is all zeros by default
        assert_eq!(view.get(0, 0), 1); // value 0 → material 1
    }

    #[test]
    fn test_mj_grid_view_out_of_bounds() {
        let grid = MjGrid::with_values(4, 4, 1, "BW");
        let view = MjGridView::with_default_mapping(&grid);

        // Out of bounds returns 0
        assert_eq!(view.get(10, 10), 0);
        assert_eq!(view.get(4, 0), 0);
        assert_eq!(view.get(0, 4), 0);
    }

    #[test]
    fn test_mj_grid_view_partial_mapping() {
        let mut grid = MjGrid::with_values(2, 2, 1, "BWR");
        grid.set(0, 0, 0, 0); // B
        grid.set(1, 0, 0, 1); // W
        grid.set(0, 1, 0, 2); // R

        // Only map B and W, R should fall back to index+1 = 3
        let mut mapping = HashMap::new();
        mapping.insert('B', 100u32);
        mapping.insert('W', 200u32);
        // No mapping for 'R'

        let view = MjGridView::new(&grid, &mapping);

        assert_eq!(view.get(0, 0), 100); // B → 100
        assert_eq!(view.get(1, 0), 200); // W → 200
        assert_eq!(view.get(0, 1), 3); // R → fallback (index 2 + 1)
    }

    #[test]
    fn test_mj_grid_view_trait_object() {
        let grid = MjGrid::with_values(4, 4, 1, "BW");
        let view = MjGridView::with_default_mapping(&grid);

        // Can use as trait object
        let grid_trait: &dyn VoxelGrid2D = &view;
        assert_eq!(grid_trait.width(), 4);
        assert_eq!(grid_trait.height(), 4);
    }
}
