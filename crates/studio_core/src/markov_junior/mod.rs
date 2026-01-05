//! MarkovJunior procedural generation system.
//!
//! A Rust port of the MarkovJunior probabilistic programming language for
//! procedural content generation using rewrite rules.
//!
//! This module provides:
//! - `MjGrid`: Core grid structure with wave/value mappings
//! - `MjRule`: Rewrite rules with pattern matching
//! - `symmetry`: Functions for generating symmetry variants
//! - `voxel_bridge`: Conversion from MjGrid to VoxelWorld for rendering
//!
//! ## Example
//!
//! ```ignore
//! use studio_core::markov_junior::{MjGrid, MjRule, MjPalette};
//!
//! // Create a grid with Black and White values
//! let mut grid = MjGrid::with_values(5, 5, 1, "BW");
//!
//! // Parse a rule: B -> W (black becomes white)
//! let rule = MjRule::parse("B", "W", &grid).unwrap();
//!
//! // Check if rule matches at position
//! if grid.matches(&rule, 0, 0, 0) {
//!     // Apply the rule...
//! }
//! ```

pub mod all_node;
pub mod convchain_node;
pub mod convolution_node;
pub mod field;
pub mod helper;
pub mod interpreter;
pub mod loader;
pub mod lua_api;
pub mod map_node;
pub mod model;
pub mod node;
#[cfg(test)]
mod node_tests;
pub mod observation;
pub mod one_node;
pub mod parallel_node;
pub mod path_node;
pub mod render;
pub mod rule;
pub mod rule_node;
pub mod search;
pub mod symmetry;
pub mod voxel_bridge;
pub mod wfc;

pub use all_node::AllNode;
pub use convchain_node::ConvChainNode;
pub use convolution_node::{ConvolutionNode, ConvolutionRule};
pub use field::{delta_pointwise, Field};
pub use interpreter::Interpreter;
pub use loader::{load_model, load_model_str, LoadError, LoadedModel};
pub use map_node::{MapNode, ScaleFactor};
pub use model::Model;
pub use node::{ExecutionContext, MarkovNode, Node, SequenceNode};
pub use observation::Observation;
pub use one_node::OneNode;
pub use parallel_node::ParallelNode;
pub use path_node::PathNode;
pub use rule::{MjRule, RuleParseError};
pub use rule_node::RuleNodeData;
pub use search::{run_search, Board};
pub use symmetry::{square_symmetries, SquareSubgroup};
pub use voxel_bridge::{to_voxel_world, MjPalette};
pub use wfc::{OverlapNode, TileNode, Wave, WfcNode, WfcState};

// PNG rendering (no GPU needed)
pub use render::{
    colors_for_grid, default_colors, pico8_colors, render_2d, render_3d_isometric, render_to_png,
    render_to_png_with_colors, save_png, RenderPalette,
};

// Lua API
pub use lua_api::{register_markov_junior_api, MjLuaVoxelWorld};

use std::collections::HashMap;
use std::fmt;

/// Error type for grid construction.
#[derive(Debug, Clone, PartialEq)]
pub enum GridError {
    /// Duplicate character found in values string
    DuplicateCharacter(char),
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GridError::DuplicateCharacter(c) => {
                write!(f, "duplicate character '{}' in values string", c)
            }
        }
    }
}

impl std::error::Error for GridError {}

/// A 3D grid of voxel states for MarkovJunior.
///
/// The grid stores u8 values and maintains mappings between:
/// - Characters (like 'B', 'W', 'R') and byte indices (0, 1, 2)
/// - Characters and wave bitmasks (1, 2, 4) for pattern matching
///
/// Indexing follows MarkovJunior convention: `index = x + y * mx + z * mx * my`
#[derive(Debug, Clone)]
pub struct MjGrid {
    /// Flat array of voxel states (byte indices, not wave bitmasks)
    pub state: Vec<u8>,
    /// Mask for tracking which cells have been modified/claimed (used by AllNode)
    pub mask: Vec<bool>,
    /// Width (X dimension)
    pub mx: usize,
    /// Height (Y dimension)
    pub my: usize,
    /// Depth (Z dimension)
    pub mz: usize,
    /// Number of distinct values/colors
    pub c: u8,
    /// Character array for each index (index -> char)
    pub characters: Vec<char>,
    /// Mapping from character to byte index
    pub values: HashMap<char, u8>,
    /// Mapping from character to wave bitmask
    pub waves: HashMap<char, u32>,
}

impl MjGrid {
    /// Create a new grid filled with zeros (no value/wave mappings).
    /// Use `with_values` for a grid with proper mappings.
    pub fn new(mx: usize, my: usize, mz: usize) -> Self {
        let size = mx * my * mz;
        Self {
            state: vec![0; size],
            mask: vec![false; size],
            mx,
            my,
            mz,
            c: 0,
            characters: Vec::new(),
            values: HashMap::new(),
            waves: HashMap::new(),
        }
    }

    /// Create a grid with value mappings from a string like "BWR".
    ///
    /// Each character becomes a value:
    /// - 'B' -> index 0, wave 0b001
    /// - 'W' -> index 1, wave 0b010
    /// - 'R' -> index 2, wave 0b100
    ///
    /// The wildcard '*' is automatically added with wave = (1 << c) - 1
    ///
    /// # Panics
    /// Panics if the values string contains duplicate characters.
    /// Use `try_with_values` for fallible construction.
    pub fn with_values(mx: usize, my: usize, mz: usize, values_str: &str) -> Self {
        Self::try_with_values(mx, my, mz, values_str).expect("duplicate character in values string")
    }

    /// Try to create a grid with value mappings, returning an error on duplicate characters.
    pub fn try_with_values(
        mx: usize,
        my: usize,
        mz: usize,
        values_str: &str,
    ) -> Result<Self, GridError> {
        let mut grid = Self::new(mx, my, mz);

        // Remove spaces from values string
        let values_str: String = values_str.chars().filter(|c| !c.is_whitespace()).collect();

        grid.c = values_str.len() as u8;
        grid.characters = values_str.chars().collect();

        for (i, ch) in values_str.chars().enumerate() {
            if grid.values.contains_key(&ch) {
                return Err(GridError::DuplicateCharacter(ch));
            }
            grid.values.insert(ch, i as u8);
            grid.waves.insert(ch, 1 << i);
        }

        // Add wildcard '*' that matches all values
        grid.waves.insert('*', (1u32 << grid.c) - 1);

        Ok(grid)
    }

    /// Get the wave bitmask for a string of characters.
    ///
    /// For example, with values "BW":
    /// - wave("B") = 1 (0b01)
    /// - wave("W") = 2 (0b10)
    /// - wave("BW") = 3 (0b11)
    pub fn wave(&self, chars: &str) -> u32 {
        let mut sum = 0u32;
        for ch in chars.chars() {
            if let Some(&idx) = self.values.get(&ch) {
                sum |= 1 << idx;
            }
        }
        sum
    }

    /// Check if a rule matches at the given position.
    ///
    /// Returns true if all input pattern cells match the grid state
    /// (using wave bitmasks to allow wildcards).
    pub fn matches(&self, rule: &MjRule, x: i32, y: i32, z: i32) -> bool {
        // Check bounds
        if x < 0 || y < 0 || z < 0 {
            return false;
        }
        let x = x as usize;
        let y = y as usize;
        let z = z as usize;

        if x + rule.imx > self.mx || y + rule.imy > self.my || z + rule.imz > self.mz {
            return false;
        }

        // Check each cell in the input pattern
        let mut dz = 0;
        let mut dy = 0;
        let mut dx = 0;

        for di in 0..rule.input.len() {
            let grid_idx = (x + dx) + (y + dy) * self.mx + (z + dz) * self.mx * self.my;
            let grid_value = self.state[grid_idx];
            let input_wave = rule.input[di];

            // Check if grid value is allowed by the input wave
            if (input_wave & (1 << grid_value)) == 0 {
                return false;
            }

            dx += 1;
            if dx == rule.imx {
                dx = 0;
                dy += 1;
                if dy == rule.imy {
                    dy = 0;
                    dz += 1;
                }
            }
        }

        true
    }

    /// Apply a rule at the given position (no bounds checking).
    ///
    /// Writes the output pattern to the grid state.
    /// Output value 0xff means "don't change" (wildcard).
    pub fn apply(&mut self, rule: &MjRule, x: usize, y: usize, z: usize) {
        let mut dz = 0;
        let mut dy = 0;
        let mut dx = 0;

        for di in 0..rule.output.len() {
            let out_value = rule.output[di];
            if out_value != 0xff {
                let grid_idx = (x + dx) + (y + dy) * self.mx + (z + dz) * self.mx * self.my;
                self.state[grid_idx] = out_value;
            }

            dx += 1;
            if dx == rule.omx {
                dx = 0;
                dy += 1;
                if dy == rule.omy {
                    dy = 0;
                    dz += 1;
                }
            }
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

    /// Clear the grid (set all cells to 0) and reset mask.
    pub fn clear(&mut self) {
        self.state.fill(0);
        self.mask.fill(false);
    }

    /// Clear just the mask (used before each AllNode step).
    pub fn clear_mask(&mut self) {
        self.mask.fill(false);
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
    fn test_grid_with_values() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        assert_eq!(grid.c, 2);
        assert_eq!(grid.values.get(&'B'), Some(&0));
        assert_eq!(grid.values.get(&'W'), Some(&1));
        assert_eq!(grid.waves.get(&'B'), Some(&1));
        assert_eq!(grid.waves.get(&'W'), Some(&2));
        assert_eq!(grid.waves.get(&'*'), Some(&3)); // wildcard
    }

    #[test]
    fn test_grid_duplicate_character_error() {
        let result = MjGrid::try_with_values(5, 5, 1, "BWB");
        assert!(matches!(result, Err(GridError::DuplicateCharacter('B'))));
    }

    #[test]
    fn test_grid_wave_bw() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        assert_eq!(grid.wave("B"), 1);
        assert_eq!(grid.wave("W"), 2);
        assert_eq!(grid.wave("BW"), 3);
        assert_eq!(grid.wave("WB"), 3); // order doesn't matter
    }

    #[test]
    fn test_grid_wave_multiple() {
        let grid = MjGrid::with_values(5, 5, 1, "BRGW");
        assert_eq!(grid.wave("B"), 0b0001);
        assert_eq!(grid.wave("R"), 0b0010);
        assert_eq!(grid.wave("G"), 0b0100);
        assert_eq!(grid.wave("W"), 0b1000);
        assert_eq!(grid.wave("BR"), 0b0011);
        assert_eq!(grid.wave("BRGW"), 0b1111);
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

    #[test]
    fn test_grid_matches_rule() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        // Grid is all B's (value 0) initially

        // Rule: B -> W (matches single B, outputs W)
        let rule = MjRule::parse("B", "W", &grid).unwrap();

        // Should match at (0,0,0) since grid is all B's
        assert!(grid.matches(&rule, 0, 0, 0));
        assert!(grid.matches(&rule, 4, 4, 0));

        // Out of bounds should not match
        assert!(!grid.matches(&rule, 5, 0, 0));
        assert!(!grid.matches(&rule, -1, 0, 0));
    }

    #[test]
    fn test_grid_matches_rule_pattern() {
        let mut grid = MjGrid::with_values(5, 5, 1, "BW");
        // Set a pattern: BWB in the first row
        grid.set(0, 0, 0, 0); // B
        grid.set(1, 0, 0, 1); // W
        grid.set(2, 0, 0, 0); // B

        // Rule that matches "BW"
        let rule = MjRule::parse("BW", "WW", &grid).unwrap();
        assert!(grid.matches(&rule, 0, 0, 0)); // matches BWx at (0,0)
        assert!(!grid.matches(&rule, 1, 0, 0)); // WB at (1,0) doesn't match

        // Rule with wildcard
        let rule_wild = MjRule::parse("*W", "WW", &grid).unwrap();
        assert!(grid.matches(&rule_wild, 0, 0, 0)); // *W matches BW
    }

    #[test]
    fn test_grid_apply_rule() {
        let mut grid = MjGrid::with_values(5, 5, 1, "BW");
        // Grid starts all B's (0)

        let rule = MjRule::parse("B", "W", &grid).unwrap();
        assert_eq!(grid.get(0, 0, 0), Some(0)); // B

        grid.apply(&rule, 0, 0, 0);
        assert_eq!(grid.get(0, 0, 0), Some(1)); // now W
    }

    #[test]
    fn test_grid_clear() {
        let mut grid = MjGrid::with_values(3, 3, 1, "BW");
        grid.set(1, 1, 0, 1);
        grid.set(2, 2, 0, 1);
        assert_eq!(grid.count_nonzero(), 2);

        grid.clear();
        assert_eq!(grid.count_nonzero(), 0);
    }
}
