//! Spherical/Polar coordinate grid for MarkovJunior.
//!
//! This module provides a spherical coordinate system that implements `MjGridOps`,
//! enabling MarkovJunior to run on polar/spherical grids with the same node implementations.
//!
//! ## Coordinate System
//!
//! - `r` - Radial index (0 to r_depth-1), actual radius = r_min + r
//! - `theta` - Azimuthal angle index (0 to theta_divisions-1), wraps around
//! - `phi` - Elevation angle index (0 to phi_divisions-1), 1 for 2D polar
//!
//! ## Flat Indexing
//!
//! `idx = theta + phi * theta_divisions + r * theta_divisions * phi_divisions`
//!
//! For 2D polar (phi_divisions=1): `idx = theta + r * theta_divisions`

use std::collections::HashMap;
use std::f32::consts::PI;

use super::grid_ops::MjGridOps;

/// Spherical/Polar grid for MarkovJunior.
///
/// Uses flat storage like Cartesian MjGrid, with coordinate conversion
/// between (r, theta, phi) and flat indices.
///
/// For 2D polar grids, set `phi_divisions = 1`.
#[derive(Debug, Clone)]
pub struct SphericalMjGrid {
    // === Flat storage (like Cartesian) ===
    /// Cell values as flat array
    pub state: Vec<u8>,
    /// Mask for tracking modifications (used by AllNode)
    pub mask: Vec<bool>,

    // === Dimensions ===
    /// Number of radial levels (rings/shells)
    pub r_depth: u16,
    /// Number of azimuthal divisions (around the axis)
    pub theta_divisions: u16,
    /// Number of elevation divisions (1 for 2D polar)
    pub phi_divisions: u16,

    // === Geometry ===
    /// Minimum radius (actual_radius = r_min + r_index)
    pub r_min: u32,
    /// Target arc length for cell sizing
    pub target_arc_length: f32,

    // === Value system (same as Cartesian) ===
    /// Number of distinct values/colors
    pub c: u8,
    /// Value index to character mapping
    pub characters: Vec<char>,
    /// Character to value index mapping
    pub values: HashMap<char, u8>,
    /// Character to wave bitmask mapping
    pub waves: HashMap<char, u32>,
}

impl SphericalMjGrid {
    /// Create a new 2D polar grid (phi_divisions = 1).
    ///
    /// # Arguments
    /// * `r_min` - Minimum radius (recommended: 256 for <1% distortion)
    /// * `r_depth` - Number of radial levels
    /// * `target_arc` - Target arc length for cell sizing
    /// * `values_str` - Character string defining the value alphabet (e.g., "BW")
    pub fn new_polar(r_min: u32, r_depth: u16, target_arc: f32, values_str: &str) -> Self {
        let theta_divisions = Self::calculate_theta_divisions(r_min, target_arc);
        Self::new(r_min, r_depth, theta_divisions, 1, target_arc, values_str)
    }

    /// Create a new 3D spherical grid.
    ///
    /// # Arguments
    /// * `r_min` - Minimum radius
    /// * `r_depth` - Number of radial levels (shells)
    /// * `theta_divisions` - Azimuthal divisions
    /// * `phi_divisions` - Elevation divisions
    /// * `target_arc` - Target arc length
    /// * `values_str` - Character string defining the value alphabet
    pub fn new_spherical(
        r_min: u32,
        r_depth: u16,
        theta_divisions: u16,
        phi_divisions: u16,
        target_arc: f32,
        values_str: &str,
    ) -> Self {
        Self::new(
            r_min,
            r_depth,
            theta_divisions,
            phi_divisions,
            target_arc,
            values_str,
        )
    }

    /// Internal constructor.
    fn new(
        r_min: u32,
        r_depth: u16,
        theta_divisions: u16,
        phi_divisions: u16,
        target_arc_length: f32,
        values_str: &str,
    ) -> Self {
        let total_cells = r_depth as usize * theta_divisions as usize * phi_divisions as usize;

        // Build value system from string
        let mut characters = Vec::new();
        let mut values = HashMap::new();
        let mut waves = HashMap::new();

        for (i, ch) in values_str.chars().enumerate() {
            characters.push(ch);
            values.insert(ch, i as u8);
            waves.insert(ch, 1u32 << i);
        }

        // Add wildcard
        let wildcard_wave = (1u32 << characters.len()) - 1;
        waves.insert('*', wildcard_wave);

        Self {
            state: vec![0u8; total_cells],
            mask: vec![false; total_cells],
            r_depth,
            theta_divisions,
            phi_divisions,
            r_min,
            target_arc_length,
            c: characters.len() as u8,
            characters,
            values,
            waves,
        }
    }

    /// Calculate theta divisions for a given radius and target arc length.
    ///
    /// Formula: theta_divisions = floor(2 * PI * r / target_arc)
    #[inline]
    pub fn calculate_theta_divisions(r: u32, target_arc: f32) -> u16 {
        let circumference = 2.0 * PI * r as f32;
        (circumference / target_arc).floor().max(6.0) as u16
    }

    /// Check if this is a 2D polar grid.
    #[inline]
    pub fn is_polar_2d(&self) -> bool {
        self.phi_divisions == 1
    }

    /// Get the actual radius for a given r index.
    #[inline]
    pub fn r_actual(&self, r: u16) -> u32 {
        self.r_min + r as u32
    }

    // === Coordinate Conversion ===

    /// Convert (r, theta, phi) to flat index.
    ///
    /// For 2D polar (phi=0): idx = theta + r * theta_divisions
    /// For 3D: idx = theta + phi * theta_divisions + r * theta_divisions * phi_divisions
    #[inline]
    pub fn coord_to_index(&self, r: u16, theta: u16, phi: u16) -> usize {
        let theta_wrapped = theta % self.theta_divisions;
        let phi_wrapped = phi % self.phi_divisions;
        theta_wrapped as usize
            + phi_wrapped as usize * self.theta_divisions as usize
            + r as usize * self.theta_divisions as usize * self.phi_divisions as usize
    }

    /// Convert flat index to (r, theta, phi).
    #[inline]
    pub fn index_to_coord(&self, idx: usize) -> (u16, u16, u16) {
        let theta_phi_size = self.theta_divisions as usize * self.phi_divisions as usize;
        let r = (idx / theta_phi_size) as u16;
        let remainder = idx % theta_phi_size;
        let phi = (remainder / self.theta_divisions as usize) as u16;
        let theta = (remainder % self.theta_divisions as usize) as u16;
        (r, theta, phi)
    }

    /// Get value at (r, theta, phi).
    #[inline]
    pub fn get(&self, r: u16, theta: u16, phi: u16) -> u8 {
        let idx = self.coord_to_index(r, theta, phi);
        self.state[idx]
    }

    /// Set value at (r, theta, phi).
    #[inline]
    pub fn set(&mut self, r: u16, theta: u16, phi: u16, value: u8) {
        let idx = self.coord_to_index(r, theta, phi);
        self.state[idx] = value;
    }

    /// Get neighbors at (r, theta, phi).
    ///
    /// Returns indices of neighboring cells. Theta wraps around, phi wraps
    /// for 3D. R neighbors are None at boundaries.
    pub fn neighbors(&self, r: u16, theta: u16, phi: u16) -> SphericalNeighbors {
        let theta_minus = (theta + self.theta_divisions - 1) % self.theta_divisions;
        let theta_plus = (theta + 1) % self.theta_divisions;

        let phi_minus = if self.phi_divisions > 1 {
            Some((phi + self.phi_divisions - 1) % self.phi_divisions)
        } else {
            None
        };
        let phi_plus = if self.phi_divisions > 1 {
            Some((phi + 1) % self.phi_divisions)
        } else {
            None
        };

        let r_minus = if r > 0 { Some(r - 1) } else { None };
        let r_plus = if r < self.r_depth - 1 {
            Some(r + 1)
        } else {
            None
        };

        SphericalNeighbors {
            theta_minus: self.coord_to_index(r, theta_minus, phi),
            theta_plus: self.coord_to_index(r, theta_plus, phi),
            phi_minus: phi_minus.map(|p| self.coord_to_index(r, theta, p)),
            phi_plus: phi_plus.map(|p| self.coord_to_index(r, theta, p)),
            r_minus: r_minus.map(|r| self.coord_to_index(r, theta, phi)),
            r_plus: r_plus.map(|r| self.coord_to_index(r, theta, phi)),
        }
    }

    /// Clear the grid (set all cells to 0) and reset mask.
    pub fn clear(&mut self) {
        self.state.fill(0);
        self.mask.fill(false);
    }

    /// Get combined wave bitmask for a string of characters.
    pub fn wave(&self, chars: &str) -> u32 {
        let mut result = 0u32;
        for ch in chars.chars() {
            if let Some(&w) = self.waves.get(&ch) {
                result |= w;
            }
        }
        result
    }

    /// Convert spherical coordinates to Cartesian (x, y, z).
    ///
    /// For 2D polar, z=0.
    pub fn to_cartesian(&self, r: u16, theta: u16, phi: u16) -> (f32, f32, f32) {
        let r_actual = self.r_actual(r) as f32;
        let theta_angle = (theta as f32 / self.theta_divisions as f32) * 2.0 * PI;

        if self.is_polar_2d() {
            // 2D polar: x = r * cos(theta), y = r * sin(theta), z = 0
            let x = r_actual * theta_angle.cos();
            let y = r_actual * theta_angle.sin();
            (x, y, 0.0)
        } else {
            // 3D spherical: standard spherical to Cartesian
            let phi_angle = (phi as f32 / self.phi_divisions as f32) * PI;
            let x = r_actual * phi_angle.sin() * theta_angle.cos();
            let y = r_actual * phi_angle.sin() * theta_angle.sin();
            let z = r_actual * phi_angle.cos();
            (x, y, z)
        }
    }
}

/// Neighbor indices for a spherical grid cell.
#[derive(Debug, Clone, Copy)]
pub struct SphericalNeighbors {
    /// Theta minus neighbor (always exists, wraps)
    pub theta_minus: usize,
    /// Theta plus neighbor (always exists, wraps)
    pub theta_plus: usize,
    /// Phi minus neighbor (None for 2D polar)
    pub phi_minus: Option<usize>,
    /// Phi plus neighbor (None for 2D polar)
    pub phi_plus: Option<usize>,
    /// R minus neighbor (None at r=0)
    pub r_minus: Option<usize>,
    /// R plus neighbor (None at r=r_depth-1)
    pub r_plus: Option<usize>,
}

// ============================================================================
// MjGridOps trait implementation for SphericalMjGrid
// ============================================================================

impl MjGridOps for SphericalMjGrid {
    fn len(&self) -> usize {
        self.state.len()
    }

    fn is_2d(&self) -> bool {
        self.phi_divisions == 1
    }

    fn get_state(&self, idx: usize) -> u8 {
        self.state[idx]
    }

    fn set_state(&mut self, idx: usize, value: u8) {
        self.state[idx] = value;
    }

    fn state(&self) -> &[u8] {
        &self.state
    }

    fn state_mut(&mut self) -> &mut [u8] {
        &mut self.state
    }

    fn num_values(&self) -> u8 {
        self.c
    }

    fn value_for_char(&self, ch: char) -> Option<u8> {
        self.values.get(&ch).copied()
    }

    fn char_for_value(&self, val: u8) -> Option<char> {
        self.characters.get(val as usize).copied()
    }

    fn wave_for_char(&self, ch: char) -> Option<u32> {
        self.waves.get(&ch).copied()
    }

    fn wave(&self, chars: &str) -> u32 {
        SphericalMjGrid::wave(self, chars)
    }

    fn get_mask(&self, idx: usize) -> bool {
        self.mask[idx]
    }

    fn set_mask(&mut self, idx: usize, value: bool) {
        self.mask[idx] = value;
    }

    fn clear_mask(&mut self) {
        self.mask.fill(false);
    }

    fn dimensions(&self) -> (usize, usize, usize) {
        // For spherical: (theta_divisions, phi_divisions, r_depth)
        // This matches the indexing order: theta varies fastest, then phi, then r
        (
            self.theta_divisions as usize,
            self.phi_divisions as usize,
            self.r_depth as usize,
        )
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_polar_2d() {
        let grid = SphericalMjGrid::new_polar(256, 64, 1.0, "BW");

        assert!(grid.is_polar_2d());
        assert_eq!(grid.phi_divisions, 1);
        assert_eq!(grid.r_depth, 64);
        assert_eq!(grid.c, 2);
        assert_eq!(grid.value_for_char('B'), Some(0));
        assert_eq!(grid.value_for_char('W'), Some(1));
    }

    #[test]
    fn test_new_spherical_3d() {
        let grid = SphericalMjGrid::new_spherical(100, 10, 32, 16, 1.0, "BWR");

        assert!(!grid.is_polar_2d());
        assert_eq!(grid.phi_divisions, 16);
        assert_eq!(grid.theta_divisions, 32);
        assert_eq!(grid.r_depth, 10);
        assert_eq!(grid.c, 3);
    }

    #[test]
    fn test_coord_conversion_2d() {
        let grid = SphericalMjGrid::new_polar(256, 8, 1.0, "BW");
        let theta_divs = grid.theta_divisions;

        // Test roundtrip for all cells
        for r in 0..8u16 {
            for theta in 0..theta_divs {
                let idx = grid.coord_to_index(r, theta, 0);
                let (r2, theta2, phi2) = grid.index_to_coord(idx);
                assert_eq!((r, theta, 0), (r2, theta2, phi2));
            }
        }
    }

    #[test]
    fn test_coord_conversion_3d() {
        let grid = SphericalMjGrid::new_spherical(100, 4, 8, 6, 1.0, "BW");

        // Test roundtrip for all cells
        for r in 0..4u16 {
            for phi in 0..6u16 {
                for theta in 0..8u16 {
                    let idx = grid.coord_to_index(r, theta, phi);
                    let (r2, theta2, phi2) = grid.index_to_coord(idx);
                    assert_eq!((r, theta, phi), (r2, theta2, phi2));
                }
            }
        }
    }

    #[test]
    fn test_get_set() {
        let mut grid = SphericalMjGrid::new_polar(256, 4, 1.0, "BWR");

        grid.set(2, 100, 0, 1); // Set to W
        assert_eq!(grid.get(2, 100, 0), 1);

        grid.set(2, 100, 0, 2); // Set to R
        assert_eq!(grid.get(2, 100, 0), 2);
    }

    #[test]
    fn test_theta_wraparound() {
        let grid = SphericalMjGrid::new_polar(256, 4, 1.0, "BW");
        let theta_divs = grid.theta_divisions;

        // Setting at theta_divs should wrap to 0
        let idx1 = grid.coord_to_index(0, 0, 0);
        let idx2 = grid.coord_to_index(0, theta_divs, 0); // wraps to 0
        assert_eq!(idx1, idx2);
    }

    #[test]
    fn test_neighbors_2d() {
        let grid = SphericalMjGrid::new_polar(256, 4, 1.0, "BW");
        let theta_divs = grid.theta_divisions;

        let neighbors = grid.neighbors(2, 5, 0);

        // Theta neighbors (wrap around)
        assert_eq!(neighbors.theta_minus, grid.coord_to_index(2, 4, 0));
        assert_eq!(neighbors.theta_plus, grid.coord_to_index(2, 6, 0));

        // Phi neighbors (None for 2D)
        assert!(neighbors.phi_minus.is_none());
        assert!(neighbors.phi_plus.is_none());

        // R neighbors
        assert_eq!(neighbors.r_minus, Some(grid.coord_to_index(1, 5, 0)));
        assert_eq!(neighbors.r_plus, Some(grid.coord_to_index(3, 5, 0)));

        // Edge cases
        let edge_neighbors = grid.neighbors(0, 0, 0);
        assert!(edge_neighbors.r_minus.is_none());
        assert_eq!(
            edge_neighbors.theta_minus,
            grid.coord_to_index(0, theta_divs - 1, 0)
        );
    }

    #[test]
    fn test_mjgridops_implementation() {
        let mut grid = SphericalMjGrid::new_polar(256, 4, 1.0, "BWR");

        // Test trait methods
        assert!(grid.is_2d());
        assert_eq!(grid.num_values(), 3);
        assert_eq!(grid.value_for_char('B'), Some(0));
        assert_eq!(grid.char_for_value(1), Some('W'));

        // Test wave
        assert_eq!(grid.wave("B"), 0b001);
        assert_eq!(grid.wave("W"), 0b010);
        assert_eq!(grid.wave("BW"), 0b011);

        // Test state access
        let idx = grid.coord_to_index(1, 50, 0);
        grid.set_state(idx, 2);
        assert_eq!(grid.get_state(idx), 2);

        // Test mask
        assert!(!grid.get_mask(idx));
        grid.set_mask(idx, true);
        assert!(grid.get_mask(idx));
        grid.clear_mask();
        assert!(!grid.get_mask(idx));
    }

    #[test]
    fn test_dimensions() {
        let grid = SphericalMjGrid::new_spherical(100, 10, 32, 16, 1.0, "BW");
        let (d0, d1, d2) = grid.dimensions();

        // dimensions() returns (theta_divisions, phi_divisions, r_depth)
        assert_eq!(d0, 32);
        assert_eq!(d1, 16);
        assert_eq!(d2, 10);
    }

    #[test]
    fn test_len() {
        let grid = SphericalMjGrid::new_spherical(100, 10, 32, 16, 1.0, "BW");
        assert_eq!(grid.len(), 10 * 32 * 16);

        let grid_2d = SphericalMjGrid::new_polar(256, 8, 1.0, "BW");
        assert_eq!(grid_2d.len(), 8 * grid_2d.theta_divisions as usize);
    }

    #[test]
    fn test_to_cartesian_2d() {
        let grid = SphericalMjGrid::new_polar(100, 4, 1.0, "BW");

        // At r=0, theta=0: should be at (r_min, 0, 0)
        let (x, y, z) = grid.to_cartesian(0, 0, 0);
        assert!((x - 100.0).abs() < 0.01);
        assert!(y.abs() < 0.01);
        assert_eq!(z, 0.0);
    }

    #[test]
    fn test_with_execution_context() {
        use crate::markov_junior::node::ExecutionContext;
        use crate::markov_junior::rng::StdRandom;

        // Create a spherical grid
        let mut grid = SphericalMjGrid::new_polar(256, 4, 1.0, "BWR");
        let mut rng = StdRandom::from_u64_seed(42);

        // Create ExecutionContext with SphericalMjGrid - this tests that our
        // generic ExecutionContext<'a, G: MjGridOps> works with SphericalMjGrid
        let mut ctx: ExecutionContext<'_, SphericalMjGrid> =
            ExecutionContext::new(&mut grid, &mut rng);

        // Test that we can use the context
        assert!(ctx.grid.is_2d());
        assert_eq!(ctx.grid.num_values(), 3);

        // Test state modification through trait
        let idx = ctx.grid.coord_to_index(1, 100, 0);
        ctx.grid.set_state(idx, 1);
        assert_eq!(ctx.grid.get_state(idx), 1);

        // Test change recording
        ctx.record_change(idx);
        assert_eq!(ctx.changes.len(), 1);
        assert_eq!(ctx.changes[0], idx);

        // Advance turn
        ctx.next_turn();
        assert_eq!(ctx.counter, 1);
    }
}
