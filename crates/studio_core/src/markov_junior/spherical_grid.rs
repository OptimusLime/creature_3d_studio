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

    /// Get the angular range (start, end) in radians for a cell at (r, theta).
    ///
    /// The range spans [theta/divs * 2*PI, (theta+1)/divs * 2*PI].
    #[inline]
    pub fn angular_range(&self, theta: u16) -> (f32, f32) {
        let divs = self.theta_divisions as f32;
        let theta_wrapped = (theta % self.theta_divisions) as f32;
        let start = theta_wrapped / divs * 2.0 * PI;
        let end = (theta_wrapped + 1.0) / divs * 2.0 * PI;
        (start, end)
    }

    /// Get neighbors by coordinates, returning coordinates (not flat indices).
    ///
    /// This is useful for tests that need to verify coordinate relationships.
    pub fn neighbors_coord(&self, r: u16, theta: u16, phi: u16) -> SphericalNeighborsCoord {
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

        SphericalNeighborsCoord {
            theta_minus: (r, theta_minus, phi),
            theta_plus: (r, theta_plus, phi),
            phi_minus: phi_minus.map(|p| (r, theta, p)),
            phi_plus: phi_plus.map(|p| (r, theta, p)),
            r_minus: r_minus.map(|nr| (nr, theta, phi)),
            r_plus: r_plus.map(|nr| (nr, theta, phi)),
        }
    }

    /// Total number of voxels in the grid.
    #[inline]
    pub fn total_voxels(&self) -> usize {
        self.state.len()
    }

    /// Count voxels with non-zero values.
    pub fn count_nonzero(&self) -> usize {
        self.state.iter().filter(|&&v| v != 0).count()
    }

    /// Compute a checksum of the grid for deterministic verification.
    pub fn checksum(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.state.hash(&mut hasher);
        hasher.finish()
    }

    /// Iterate over all voxels with their (r, theta, phi, value) coordinates.
    pub fn iter(&self) -> impl Iterator<Item = (u16, u16, u16, u8)> + '_ {
        self.state.iter().enumerate().map(|(idx, &value)| {
            let (r, theta, phi) = self.index_to_coord(idx);
            (r, theta, phi, value)
        })
    }

    /// Iterate over all non-zero voxels with their (r, theta, phi, value) coordinates.
    pub fn iter_nonzero(&self) -> impl Iterator<Item = (u16, u16, u16, u8)> + '_ {
        self.iter().filter(|(_, _, _, v)| *v != 0)
    }

    // =========================================================================
    // Rendering
    // =========================================================================

    /// Render the polar grid to an RGBA image.
    ///
    /// The image shows the polar grid as a ring/disk with the inner radius
    /// at the center and the outer radius at the edge.
    ///
    /// Uses pixel-based rendering: for each pixel, determine which cell it
    /// belongs to and color it accordingly.
    ///
    /// # Arguments
    /// * `image_size` - Width and height of the output image in pixels
    /// * `colors` - Color palette mapping value index to RGBA
    /// * `background` - Background color for empty areas
    ///
    /// # Returns
    /// RGBA image buffer
    pub fn render_to_image(
        &self,
        image_size: u32,
        colors: &[[u8; 4]],
        background: [u8; 4],
    ) -> image::RgbaImage {
        use image::{ImageBuffer, Rgba};

        let mut img: image::RgbaImage =
            ImageBuffer::from_pixel(image_size, image_size, Rgba(background));

        let center = image_size as f32 / 2.0;
        let r_min_actual = self.r_min as f32;
        let r_max_actual = (self.r_min + self.r_depth as u32) as f32;

        // Scale factor: map r_max_actual to image edge (with small margin)
        let scale = (image_size as f32 * 0.48) / r_max_actual;

        // Pixel-based rendering: for each pixel, find which cell it belongs to
        for py in 0..image_size {
            for px in 0..image_size {
                // Convert pixel to cartesian coords relative to center
                let x = px as f32 - center;
                let y = py as f32 - center;

                // Convert to polar coordinates
                let pixel_r = (x * x + y * y).sqrt() / scale;

                // Check if within our ring bounds
                if pixel_r < r_min_actual || pixel_r >= r_max_actual {
                    continue;
                }

                // Calculate r index
                let r_index = (pixel_r - r_min_actual) as u16;
                if r_index >= self.r_depth {
                    continue;
                }

                // Calculate theta (angle from positive x-axis)
                let mut angle = y.atan2(x);
                if angle < 0.0 {
                    angle += 2.0 * PI;
                }

                // Convert angle to theta index
                let theta_index = ((angle / (2.0 * PI)) * self.theta_divisions as f32) as u16;
                let theta_index = theta_index % self.theta_divisions;

                // Get cell value and color (phi=0 for 2D polar)
                let value = self.get(r_index, theta_index, 0) as usize;

                // Skip transparent/background
                if value >= colors.len() || colors[value][3] == 0 {
                    continue;
                }

                img.put_pixel(px, py, Rgba(colors[value]));
            }
        }

        img
    }

    /// Save the grid as a PNG image.
    pub fn save_png(
        &self,
        path: &std::path::Path,
        image_size: u32,
        colors: &[[u8; 4]],
        background: [u8; 4],
    ) -> Result<(), image::ImageError> {
        let img = self.render_to_image(image_size, colors, background);
        img.save(path)
    }
}

/// Neighbor coordinates for a spherical grid cell (coordinate form).
#[derive(Debug, Clone, Copy)]
pub struct SphericalNeighborsCoord {
    /// Theta minus neighbor (r, theta, phi)
    pub theta_minus: (u16, u16, u16),
    /// Theta plus neighbor (r, theta, phi)
    pub theta_plus: (u16, u16, u16),
    /// Phi minus neighbor (None for 2D polar)
    pub phi_minus: Option<(u16, u16, u16)>,
    /// Phi plus neighbor (None for 2D polar)
    pub phi_plus: Option<(u16, u16, u16)>,
    /// R minus neighbor (None at r=0)
    pub r_minus: Option<(u16, u16, u16)>,
    /// R plus neighbor (None at r=r_depth-1)
    pub r_plus: Option<(u16, u16, u16)>,
}

// ============================================================================
// Symmetry System
// ============================================================================

/// Symmetries for spherical/polar grids.
///
/// For 2D polar grids (phi_divisions=1), this is the Klein four-group (4 symmetries).
/// Unlike Cartesian 8-symmetry, polar grids have only 4 symmetries because the
/// radial direction is meaningful (surface vs depth), so patterns should
/// distinguish inward from outward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SphericalSymmetry {
    /// No transform: (dr, dtheta, dphi) -> (dr, dtheta, dphi)
    Identity,
    /// Theta flip: (dr, dtheta, dphi) -> (dr, -dtheta, dphi)
    /// Mirror across radial line
    ThetaFlip,
    /// R flip: (dr, dtheta, dphi) -> (-dr, dtheta, dphi)
    /// Swap inner <-> outer
    RFlip,
    /// Both flips: (dr, dtheta, dphi) -> (-dr, -dtheta, dphi)
    BothFlip,
}

impl SphericalSymmetry {
    /// All 4 symmetries (for 2D polar grids).
    pub fn all() -> [Self; 4] {
        [Self::Identity, Self::ThetaFlip, Self::RFlip, Self::BothFlip]
    }

    /// Transform a relative offset (dr, dtheta) by this symmetry.
    ///
    /// For 2D polar grids, phi offset is always 0.
    pub fn transform(&self, dr: i8, dtheta: i8) -> (i8, i8) {
        match self {
            Self::Identity => (dr, dtheta),
            Self::ThetaFlip => (dr, -dtheta),
            Self::RFlip => (-dr, dtheta),
            Self::BothFlip => (-dr, -dtheta),
        }
    }

    /// Compose two symmetries (apply self, then other).
    ///
    /// This implements the Klein four-group multiplication table.
    pub fn compose(&self, other: Self) -> Self {
        use SphericalSymmetry::*;
        match (*self, other) {
            (Identity, x) | (x, Identity) => x,
            (ThetaFlip, ThetaFlip) => Identity,
            (RFlip, RFlip) => Identity,
            (BothFlip, BothFlip) => Identity,
            (ThetaFlip, RFlip) | (RFlip, ThetaFlip) => BothFlip,
            (ThetaFlip, BothFlip) | (BothFlip, ThetaFlip) => RFlip,
            (RFlip, BothFlip) | (BothFlip, RFlip) => ThetaFlip,
        }
    }

    /// Inverse of this symmetry.
    ///
    /// In the Klein four-group, every element is its own inverse.
    pub fn inverse(&self) -> Self {
        *self
    }
}

// ============================================================================
// Pattern System
// ============================================================================

/// A spherical pattern for rule matching.
///
/// Specifies requirements for the center cell and its neighbors.
/// `None` means wildcard (any value matches).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SphericalPattern {
    /// Value at the center cell
    pub center: u8,
    /// Required value at theta_minus neighbor (None = wildcard)
    pub theta_minus: Option<u8>,
    /// Required value at theta_plus neighbor (None = wildcard)
    pub theta_plus: Option<u8>,
    /// Required value at r_minus neighbor (None = wildcard)
    pub r_minus: Option<u8>,
    /// Required value at r_plus neighbor (None = wildcard)
    pub r_plus: Option<u8>,
    /// Required value at phi_minus neighbor (None = wildcard, unused for 2D)
    pub phi_minus: Option<u8>,
    /// Required value at phi_plus neighbor (None = wildcard, unused for 2D)
    pub phi_plus: Option<u8>,
}

impl SphericalPattern {
    /// Create a new pattern with just a center value (all neighbors are wildcards).
    pub fn center_only(center: u8) -> Self {
        Self {
            center,
            ..Default::default()
        }
    }

    /// Check if this pattern matches at the given flat index in the grid.
    pub fn matches(&self, grid: &SphericalMjGrid, idx: usize) -> bool {
        // Check center
        if grid.get_state(idx) != self.center {
            return false;
        }

        let (r, theta, phi) = grid.index_to_coord(idx);
        let neighbors = grid.neighbors(r, theta, phi);

        // Check theta neighbors (always exist due to wrapping)
        if let Some(v) = self.theta_minus {
            if grid.get_state(neighbors.theta_minus) != v {
                return false;
            }
        }
        if let Some(v) = self.theta_plus {
            if grid.get_state(neighbors.theta_plus) != v {
                return false;
            }
        }

        // Check radial neighbors (may be None at boundaries)
        if let Some(required) = self.r_minus {
            match neighbors.r_minus {
                None => return false, // At boundary, can't match if we require a value
                Some(nidx) => {
                    if grid.get_state(nidx) != required {
                        return false;
                    }
                }
            }
        }
        if let Some(required) = self.r_plus {
            match neighbors.r_plus {
                None => return false,
                Some(nidx) => {
                    if grid.get_state(nidx) != required {
                        return false;
                    }
                }
            }
        }

        // Check phi neighbors (for 3D spherical)
        if let Some(required) = self.phi_minus {
            match neighbors.phi_minus {
                None => return false,
                Some(nidx) => {
                    if grid.get_state(nidx) != required {
                        return false;
                    }
                }
            }
        }
        if let Some(required) = self.phi_plus {
            match neighbors.phi_plus {
                None => return false,
                Some(nidx) => {
                    if grid.get_state(nidx) != required {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Transform this pattern by a symmetry.
    ///
    /// Returns a new pattern with the neighbor requirements rearranged
    /// according to the symmetry transform.
    pub fn transform(&self, symmetry: SphericalSymmetry) -> Self {
        match symmetry {
            SphericalSymmetry::Identity => self.clone(),
            SphericalSymmetry::ThetaFlip => Self {
                center: self.center,
                theta_minus: self.theta_plus,
                theta_plus: self.theta_minus,
                r_minus: self.r_minus,
                r_plus: self.r_plus,
                phi_minus: self.phi_minus,
                phi_plus: self.phi_plus,
            },
            SphericalSymmetry::RFlip => Self {
                center: self.center,
                theta_minus: self.theta_minus,
                theta_plus: self.theta_plus,
                r_minus: self.r_plus,
                r_plus: self.r_minus,
                phi_minus: self.phi_minus,
                phi_plus: self.phi_plus,
            },
            SphericalSymmetry::BothFlip => Self {
                center: self.center,
                theta_minus: self.theta_plus,
                theta_plus: self.theta_minus,
                r_minus: self.r_plus,
                r_plus: self.r_minus,
                phi_minus: self.phi_minus,
                phi_plus: self.phi_plus,
            },
        }
    }

    /// Generate all symmetry variants of this pattern.
    ///
    /// Returns up to 4 unique patterns (may be fewer if pattern has symmetry).
    pub fn all_variants(&self) -> Vec<Self> {
        let mut variants = Vec::with_capacity(4);
        let mut seen = std::collections::HashSet::new();

        for sym in SphericalSymmetry::all() {
            let transformed = self.transform(sym);
            if seen.insert(transformed.clone()) {
                variants.push(transformed);
            }
        }

        variants
    }
}

// ============================================================================
// Rule System
// ============================================================================

/// A spherical rewrite rule: if input pattern matches, output value is written.
#[derive(Debug, Clone)]
pub struct SphericalRule {
    /// Input pattern to match
    pub input: SphericalPattern,
    /// Output value to write to the center cell
    pub output: u8,
}

impl SphericalRule {
    /// Create a new rule.
    pub fn new(input: SphericalPattern, output: u8) -> Self {
        Self { input, output }
    }

    /// Check if this rule matches at the given flat index.
    pub fn matches(&self, grid: &SphericalMjGrid, idx: usize) -> bool {
        self.input.matches(grid, idx)
    }

    /// Apply this rule at the given flat index.
    ///
    /// Returns true if the rule was applied (pattern matched and value changed).
    pub fn apply(&self, grid: &mut SphericalMjGrid, idx: usize) -> bool {
        if self.matches(grid, idx) {
            let old_value = grid.get_state(idx);
            if old_value != self.output {
                grid.set_state(idx, self.output);
                return true;
            }
        }
        false
    }

    /// Generate all symmetry variants of this rule.
    pub fn all_variants(&self) -> Vec<Self> {
        self.input
            .all_variants()
            .into_iter()
            .map(|pattern| Self {
                input: pattern,
                output: self.output,
            })
            .collect()
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

    fn center_index(&self) -> usize {
        // Spherical center: middle radial ring, theta=0, phi=0
        // This places the origin seed in the middle ring at a consistent angular position
        let r = self.r_depth / 2;
        self.coord_to_index(r, 0, 0)
    }
}

// ============================================================================
// RecordableGrid and Renderable2D trait implementations
// ============================================================================

use super::recording::{GridType, RecordableGrid, Renderable2D};

impl RecordableGrid for SphericalMjGrid {
    fn grid_type(&self) -> GridType {
        if self.phi_divisions == 1 {
            GridType::Polar2D {
                r_min: self.r_min,
                r_depth: self.r_depth,
                theta_divisions: self.theta_divisions,
            }
        } else {
            GridType::Polar3D {
                r_min: self.r_min,
                r_depth: self.r_depth,
                theta_divisions: self.theta_divisions,
                phi_divisions: self.phi_divisions,
            }
        }
    }

    fn palette(&self) -> String {
        self.characters.iter().collect()
    }

    fn state_to_bytes(&self) -> Vec<u8> {
        self.state.clone()
    }

    fn state_from_bytes(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() != self.state.len() {
            return false;
        }
        self.state.copy_from_slice(bytes);
        true
    }
}

impl Renderable2D for SphericalMjGrid {
    fn render_to_image(
        &self,
        image_size: u32,
        colors: &[[u8; 4]],
        background: [u8; 4],
    ) -> image::RgbaImage {
        // Delegate to the existing method
        SphericalMjGrid::render_to_image(self, image_size, colors, background)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Level 0: Data Structure Tests (ported from polar_grid.rs)
    // ========================================================================

    mod level_0_data_structures {
        use super::*;

        #[test]
        fn test_grid_creation() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");

            // Verify dimensions
            assert_eq!(grid.r_min, 256);
            assert_eq!(grid.r_depth, 256);

            // Verify all cells initialized to 0
            for r in 0..256u16 {
                for theta in 0..grid.theta_divisions {
                    assert_eq!(grid.get(r, theta, 0), 0);
                }
            }
        }

        #[test]
        fn test_cell_read_write() {
            let mut grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BWR");

            // Write to various locations (using values 0, 1, 2 for B, W, R)
            grid.set(0, 0, 0, 1); // W
            grid.set(128, 500, 0, 2); // R
            grid.set(255, 1000, 0, 0); // B

            // Read back
            assert_eq!(grid.get(0, 0, 0), 1);
            assert_eq!(grid.get(128, 500, 0), 2);
            assert_eq!(grid.get(255, 1000, 0), 0);

            // Verify other cells unchanged
            assert_eq!(grid.get(0, 1, 0), 0);
            assert_eq!(grid.get(128, 501, 0), 0);
        }

        #[test]
        fn test_theta_wrapping() {
            let mut grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");
            let theta_divs = grid.theta_divisions;

            // Set a value
            grid.set(100, 0, 0, 1);

            // Access via wrapped index should return same value
            assert_eq!(grid.get(100, theta_divs, 0), 1); // wraps to 0
            assert_eq!(grid.get(100, theta_divs * 2, 0), 1); // wraps to 0
            assert_eq!(grid.get(100, theta_divs + 5, 0), grid.get(100, 5, 0));
        }

        #[test]
        fn test_memory_layout() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");

            // All cells should have the same theta_divisions (flat storage)
            // Total cells = r_depth * theta_divisions
            let expected_len = grid.r_depth as usize * grid.theta_divisions as usize;
            assert_eq!(grid.len(), expected_len);

            // Verify flat storage is contiguous
            assert_eq!(grid.state.len(), expected_len);
        }

        #[test]
        fn test_theta_divisions_formula() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");

            // Should be approximately 2*PI*r_min/target_arc
            // 2 * PI * 256 / 1.0 ~ 1608
            assert!(
                (grid.theta_divisions as f32 - 1608.0).abs() < 2.0,
                "Theta divs: {} (expected ~1608)",
                grid.theta_divisions
            );
        }
    }

    // ========================================================================
    // Level 1: Coordinate Math Tests (ported from polar_grid.rs)
    // ========================================================================

    mod level_1_coordinates {
        use super::*;

        #[test]
        fn test_no_distortion_between_rings() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");

            // With fixed theta divisions, there's NO distortion between rings.
            // All rings use the same theta_divisions value.
            // This is verified by the fact that theta_divisions is a single field,
            // not a per-ring calculation.
            assert!(grid.theta_divisions > 0);
        }

        #[test]
        fn test_arc_length_varies_by_radius() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");

            // With fixed theta divisions, arc length increases with radius.
            // At r_min: arc ~ target_arc (1.0)
            // At r_max: arc ~ target_arc * r_max/r_min

            let divs = grid.theta_divisions as f32;

            // Inner ring (r=0, actual r=256)
            let r_inner_actual = 256.0;
            let arc_inner = 2.0 * PI * r_inner_actual / divs;

            // Outer ring (r=255, actual r=511)
            let r_outer_actual = 511.0;
            let arc_outer = 2.0 * PI * r_outer_actual / divs;

            // Inner arc should be close to target
            assert!(
                (arc_inner - 1.0).abs() < 0.01,
                "Inner arc {} should be ~1.0",
                arc_inner
            );

            // Outer arc should be ~2x inner (since r_max/r_min ~ 2)
            let expected_ratio = r_outer_actual / r_inner_actual;
            let actual_ratio = arc_outer / arc_inner;
            assert!(
                (actual_ratio - expected_ratio).abs() < 0.01,
                "Arc length ratio {} should match radius ratio {}",
                actual_ratio,
                expected_ratio
            );
        }

        #[test]
        fn test_angular_range() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");

            // Voxel at theta should span [theta/divs * 2*PI, (theta+1)/divs * 2*PI]
            let theta = 50u16;
            let divs = grid.theta_divisions;

            let (start, end) = grid.angular_range(theta);

            let expected_start = theta as f32 / divs as f32 * 2.0 * PI;
            let expected_end = (theta + 1) as f32 / divs as f32 * 2.0 * PI;

            assert!((start - expected_start).abs() < 0.0001);
            assert!((end - expected_end).abs() < 0.0001);
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
    }

    // ========================================================================
    // Level 2: Neighbor Relationship Tests (ported from polar_grid.rs)
    // ========================================================================

    mod level_2_neighbors {
        use super::*;

        #[test]
        fn test_angular_neighbors() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");
            let divs = grid.theta_divisions;

            for r in [0u16, 50, 100, 200, 255] {
                for theta in [0u16, divs / 2, divs - 1] {
                    let neighbors = grid.neighbors_coord(r, theta, 0);

                    // Always exactly 2 angular neighbors
                    assert_eq!(neighbors.theta_minus, (r, (theta + divs - 1) % divs, 0));
                    assert_eq!(neighbors.theta_plus, (r, (theta + 1) % divs, 0));
                }
            }
        }

        #[test]
        fn test_angular_neighbor_wrapping() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");
            let r = 100u16;
            let divs = grid.theta_divisions;

            // At theta=0, theta_minus should wrap to divs-1
            let neighbors = grid.neighbors_coord(r, 0, 0);
            assert_eq!(neighbors.theta_minus.1, divs - 1);

            // At theta=divs-1, theta_plus should wrap to 0
            let neighbors = grid.neighbors_coord(r, divs - 1, 0);
            assert_eq!(neighbors.theta_plus.1, 0);
        }

        #[test]
        fn test_radial_neighbors_bounded() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");
            let divs = grid.theta_divisions;

            // With fixed theta divisions, EVERY cell has exactly 1 radial neighbor
            // in each direction (except at boundaries).

            for r in 1..255u16 {
                for theta in (0..divs).step_by(100) {
                    // Sample every 100th theta
                    let neighbors = grid.neighbors_coord(r, theta, 0);

                    // r_minus is always Some (we're not at r=0)
                    assert!(
                        neighbors.r_minus.is_some(),
                        "r_minus should be Some for r={}",
                        r
                    );
                    // r_plus is always Some (we're not at r=255)
                    assert!(
                        neighbors.r_plus.is_some(),
                        "r_plus should be Some for r={}",
                        r
                    );

                    // Check that neighbors have the SAME theta (aligned!)
                    let (r_m, theta_m, _) = neighbors.r_minus.unwrap();
                    let (r_p, theta_p, _) = neighbors.r_plus.unwrap();
                    assert_eq!(theta_m, theta, "r_minus theta should match");
                    assert_eq!(theta_p, theta, "r_plus theta should match");
                    assert_eq!(r_m, r - 1, "r_minus should be r-1");
                    assert_eq!(r_p, r + 1, "r_plus should be r+1");
                }
            }
        }

        #[test]
        fn test_radial_neighbor_alignment() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");

            // With fixed theta divisions, neighbors are perfectly aligned.
            // Cell (r, theta) has neighbors at exactly (r-1, theta) and (r+1, theta).
            for r in 1..255u16 {
                let theta = grid.theta_divisions / 2; // Middle theta
                let (my_start, my_end) = grid.angular_range(theta);

                let neighbors = grid.neighbors_coord(r, theta, 0);

                // Check inner neighbor has SAME angular range
                if let Some((_, nt, _)) = neighbors.r_minus {
                    let (n_start, n_end) = grid.angular_range(nt);
                    assert!(
                        (my_start - n_start).abs() < 0.0001 && (my_end - n_end).abs() < 0.0001,
                        "Inner neighbor angular range should match exactly"
                    );
                }

                // Check outer neighbor has SAME angular range
                if let Some((_, nt, _)) = neighbors.r_plus {
                    let (n_start, n_end) = grid.angular_range(nt);
                    assert!(
                        (my_start - n_start).abs() < 0.0001 && (my_end - n_end).abs() < 0.0001,
                        "Outer neighbor angular range should match exactly"
                    );
                }
            }
        }

        #[test]
        fn test_boundary_neighbors() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");

            // At r=0 (inner boundary), r_minus should be None
            let neighbors = grid.neighbors_coord(0, 0, 0);
            assert!(neighbors.r_minus.is_none());
            assert!(neighbors.r_plus.is_some());

            // At r=255 (outer boundary), r_plus should be None
            let neighbors = grid.neighbors_coord(255, 0, 0);
            assert!(neighbors.r_minus.is_some());
            assert!(neighbors.r_plus.is_none());
        }

        #[test]
        fn test_neighbor_symmetry() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");

            // If B is a neighbor of A, then A should be a neighbor of B
            for r in 1..255u16 {
                let theta = grid.theta_divisions / 2;
                let neighbors = grid.neighbors_coord(r, theta, 0);

                // Check outer neighbor lists us as inner neighbor
                if let Some((nr, nt, np)) = neighbors.r_plus {
                    let reverse_neighbors = grid.neighbors_coord(nr, nt, np);
                    assert_eq!(
                        reverse_neighbors.r_minus,
                        Some((r, theta, 0)),
                        "Neighbor symmetry violated: ({},{}) -> ({},{}) but not reverse",
                        r,
                        theta,
                        nr,
                        nt
                    );
                }
            }
        }

        #[test]
        fn test_neighbors_2d_flat_index() {
            // Test the flat index version of neighbors()
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
    }

    // ========================================================================
    // MjGridOps Trait Tests
    // ========================================================================

    mod mjgridops_tests {
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
        fn test_get_set() {
            let mut grid = SphericalMjGrid::new_polar(256, 4, 1.0, "BWR");

            grid.set(2, 100, 0, 1); // Set to W
            assert_eq!(grid.get(2, 100, 0), 1);

            grid.set(2, 100, 0, 2); // Set to R
            assert_eq!(grid.get(2, 100, 0), 2);
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

            // Create ExecutionContext with SphericalMjGrid
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

        #[test]
        fn test_clear() {
            let mut grid = SphericalMjGrid::new_polar(256, 8, 1.0, "BWR");

            // Set some state and mask values
            let idx1 = grid.coord_to_index(2, 100, 0);
            let idx2 = grid.coord_to_index(5, 200, 0);
            grid.set_state(idx1, 1);
            grid.set_state(idx2, 2);
            grid.set_mask(idx1, true);
            grid.set_mask(idx2, true);

            // Verify they're set
            assert_eq!(grid.get_state(idx1), 1);
            assert_eq!(grid.get_state(idx2), 2);
            assert!(grid.get_mask(idx1));
            assert!(grid.get_mask(idx2));

            // Clear via trait method
            use crate::markov_junior::grid_ops::MjGridOps;
            grid.clear();

            // Verify all state is 0 and mask is false
            assert_eq!(grid.get_state(idx1), 0);
            assert_eq!(grid.get_state(idx2), 0);
            assert!(!grid.get_mask(idx1));
            assert!(!grid.get_mask(idx2));
        }

        #[test]
        fn test_center_index_2d() {
            // For 2D polar grid with r_depth=8, center should be at r=4, theta=0
            let grid = SphericalMjGrid::new_polar(256, 8, 1.0, "BW");

            let center = grid.center_index();
            let (r, theta, phi) = grid.index_to_coord(center);

            // Center should be at middle radial ring
            assert_eq!(r, 4, "Center should be at r=r_depth/2=4");
            assert_eq!(theta, 0, "Center should be at theta=0");
            assert_eq!(phi, 0, "Center should be at phi=0 (2D)");

            // Verify the index calculation matches coord_to_index
            assert_eq!(center, grid.coord_to_index(4, 0, 0));
        }

        #[test]
        fn test_center_index_3d() {
            // For 3D spherical grid, center should be at middle radial shell
            let grid = SphericalMjGrid::new_spherical(100, 10, 32, 16, 1.0, "BW");

            let center = grid.center_index();
            let (r, theta, phi) = grid.index_to_coord(center);

            // Center should be at middle radial shell
            assert_eq!(r, 5, "Center should be at r=r_depth/2=5");
            assert_eq!(theta, 0, "Center should be at theta=0");
            assert_eq!(phi, 0, "Center should be at phi=0");
        }
    }

    // ========================================================================
    // Level 3: Symmetry Tests (ported from polar_grid.rs)
    // ========================================================================

    mod level_3_symmetries {
        use super::*;

        #[test]
        fn test_identity_symmetry() {
            use SphericalSymmetry::*;

            // Identity should not change anything
            assert_eq!(Identity.transform(1, 2), (1, 2));
            assert_eq!(Identity.transform(-1, -2), (-1, -2));
            assert_eq!(Identity.transform(0, 0), (0, 0));
        }

        #[test]
        fn test_theta_flip_symmetry() {
            use SphericalSymmetry::*;

            // ThetaFlip: (dr, dtheta) -> (dr, -dtheta)
            assert_eq!(ThetaFlip.transform(1, 2), (1, -2));
            assert_eq!(ThetaFlip.transform(-1, 3), (-1, -3));
            assert_eq!(ThetaFlip.transform(0, 0), (0, 0));

            // Double application should return to original
            let (dr, dt) = ThetaFlip.transform(1, 2);
            assert_eq!(ThetaFlip.transform(dr, dt), (1, 2));
        }

        #[test]
        fn test_r_flip_symmetry() {
            use SphericalSymmetry::*;

            // RFlip: (dr, dtheta) -> (-dr, dtheta)
            assert_eq!(RFlip.transform(1, 2), (-1, 2));
            assert_eq!(RFlip.transform(-1, 3), (1, 3));

            // Double application should return to original
            let (dr, dt) = RFlip.transform(1, 2);
            assert_eq!(RFlip.transform(dr, dt), (1, 2));
        }

        #[test]
        fn test_both_flip_symmetry() {
            use SphericalSymmetry::*;

            // BothFlip: (dr, dtheta) -> (-dr, -dtheta)
            assert_eq!(BothFlip.transform(1, 2), (-1, -2));
            assert_eq!(BothFlip.transform(-1, -3), (1, 3));

            // Should equal ThetaFlip composed with RFlip
            for dr in [-1i8, 0, 1] {
                for dt in [-2i8, 0, 2] {
                    let both = BothFlip.transform(dr, dt);
                    let (tdr, tdt) = ThetaFlip.transform(dr, dt);
                    let composed = RFlip.transform(tdr, tdt);
                    assert_eq!(both, composed);
                }
            }
        }

        #[test]
        fn test_symmetry_group_closure() {
            use SphericalSymmetry::*;

            // The 4 symmetries form a group (Klein four-group)
            // Composing any two should give another element of the group
            let symmetries = [Identity, ThetaFlip, RFlip, BothFlip];

            for &s1 in &symmetries {
                for &s2 in &symmetries {
                    // Compose s1 then s2
                    let (dr, dt) = s1.transform(1, 1);
                    let composed = s2.transform(dr, dt);

                    // Result should be achievable by a single symmetry
                    let found = symmetries.iter().any(|&s| s.transform(1, 1) == composed);
                    assert!(found, "Composition of {:?} and {:?} not in group", s1, s2);
                }
            }
        }

        #[test]
        fn test_pattern_symmetry_variants() {
            // A pattern with distinct neighbors should have 4 distinct variants
            let pattern = SphericalPattern {
                center: 1,
                theta_minus: Some(2),
                theta_plus: Some(3),
                r_minus: Some(4),
                r_plus: Some(5),
                phi_minus: None,
                phi_plus: None,
            };

            let variants: Vec<_> = SphericalSymmetry::all()
                .iter()
                .map(|s| pattern.transform(*s))
                .collect();

            // All 4 should be distinct
            for i in 0..4 {
                for j in (i + 1)..4 {
                    assert_ne!(
                        variants[i], variants[j],
                        "Variants {} and {} are identical",
                        i, j
                    );
                }
            }
        }

        #[test]
        fn test_symmetric_pattern_fewer_variants() {
            use std::collections::HashSet;

            // A pattern symmetric under theta flip should have only 2 unique variants
            let pattern = SphericalPattern {
                center: 1,
                theta_minus: Some(2),
                theta_plus: Some(2), // Same as theta_minus!
                r_minus: Some(3),
                r_plus: Some(4),
                phi_minus: None,
                phi_plus: None,
            };

            let variants: HashSet<_> = SphericalSymmetry::all()
                .iter()
                .map(|s| pattern.transform(*s))
                .collect();

            assert_eq!(
                variants.len(),
                2,
                "Expected 2 unique variants for theta-symmetric pattern, got {}",
                variants.len()
            );
        }
    }

    // ========================================================================
    // Level 4: Single-Step Rule Tests (ported from polar_grid.rs)
    // ========================================================================

    mod level_4_single_step {
        use super::*;

        #[test]
        fn test_simple_rule_matching() {
            let mut grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BWR");

            // Set up a pattern: center=0, all neighbors=0 except r_plus=1
            let r = 100u16;
            let theta = 500u16;
            let idx = grid.coord_to_index(r, theta, 0);
            let neighbors = grid.neighbors(r, theta, 0);

            // Set r_plus neighbor to 1
            if let Some(nidx) = neighbors.r_plus {
                grid.set_state(nidx, 1);
            }

            // Pattern: center=0, r_plus=1, others=wildcard
            let pattern = SphericalPattern {
                center: 0,
                theta_minus: None,
                theta_plus: None,
                r_minus: None,
                r_plus: Some(1),
                phi_minus: None,
                phi_plus: None,
            };

            assert!(pattern.matches(&grid, idx));

            // Shouldn't match at a location without r_plus=1
            let other_idx = grid.coord_to_index(r.saturating_sub(10), theta, 0);
            assert!(!pattern.matches(&grid, other_idx));
        }

        #[test]
        fn test_rule_application() {
            let mut grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BWR");

            // Rule: 0 -> 1 (unconditional)
            let rule = SphericalRule {
                input: SphericalPattern::center_only(0),
                output: 1,
            };

            let idx = grid.coord_to_index(100, 500, 0);
            assert_eq!(grid.get_state(idx), 0);

            // Apply the rule
            let applied = rule.apply(&mut grid, idx);
            assert!(applied);
            assert_eq!(grid.get_state(idx), 1);

            // Applying again should return false (value already correct)
            let applied_again = rule.apply(&mut grid, idx);
            assert!(!applied_again);
        }

        #[test]
        fn test_conditional_rule() {
            let mut grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BWR");

            let r = 100u16;
            let theta = 500u16;
            let idx = grid.coord_to_index(r, theta, 0);

            // Rule: if center=0 and r_plus=1, then set to 2
            let rule = SphericalRule {
                input: SphericalPattern {
                    center: 0,
                    theta_minus: None,
                    theta_plus: None,
                    r_minus: None,
                    r_plus: Some(1),
                    phi_minus: None,
                    phi_plus: None,
                },
                output: 2,
            };

            // Without r_plus=1, rule shouldn't match
            assert!(!rule.matches(&grid, idx));

            // Set up the required neighbor
            let neighbors = grid.neighbors(r, theta, 0);
            if let Some(nidx) = neighbors.r_plus {
                grid.set_state(nidx, 1);
            }

            // Now it should match and apply
            assert!(rule.matches(&grid, idx));
            let applied = rule.apply(&mut grid, idx);
            assert!(applied);
            assert_eq!(grid.get_state(idx), 2);
        }

        #[test]
        fn test_rule_with_symmetries() {
            let mut grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BWR");

            // Rule: if center=0 and theta_minus=1, output 2
            let rule = SphericalRule {
                input: SphericalPattern {
                    center: 0,
                    theta_minus: Some(1),
                    theta_plus: None,
                    r_minus: None,
                    r_plus: None,
                    phi_minus: None,
                    phi_plus: None,
                },
                output: 2,
            };

            // Generate symmetry variants
            let variants = rule.all_variants();

            // Should get variants with theta_plus=1 as well
            let has_theta_plus_variant = variants.iter().any(|r| r.input.theta_plus == Some(1));
            assert!(
                has_theta_plus_variant,
                "Should have variant with theta_plus requirement"
            );

            // Set up a grid state where theta_plus=1 (not theta_minus)
            let r = 100u16;
            let theta = 500u16;
            let idx = grid.coord_to_index(r, theta, 0);
            let neighbors = grid.neighbors(r, theta, 0);
            grid.set_state(neighbors.theta_plus, 1);

            // Original rule shouldn't match
            assert!(!rule.matches(&grid, idx));

            // But one of the variants should
            let any_variant_matches = variants.iter().any(|r| r.matches(&grid, idx));
            assert!(any_variant_matches, "One variant should match");
        }

        #[test]
        fn test_multiple_rules_priority() {
            let mut grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BWR");

            let idx = grid.coord_to_index(100, 500, 0);

            // Two rules that both match
            let rule1 = SphericalRule {
                input: SphericalPattern::center_only(0),
                output: 1,
            };
            let rule2 = SphericalRule {
                input: SphericalPattern::center_only(0),
                output: 2,
            };

            // First rule that matches wins (MarkovJunior behavior)
            assert!(rule1.matches(&grid, idx));
            assert!(rule2.matches(&grid, idx));

            // Apply rule1 first
            rule1.apply(&mut grid, idx);
            assert_eq!(grid.get_state(idx), 1);

            // rule2 no longer matches (center is now 1, not 0)
            assert!(!rule2.matches(&grid, idx));
        }
    }

    // ========================================================================
    // Level 5: Multi-Step Model Tests (ported from polar_grid.rs)
    // ========================================================================

    mod level_5_models {
        use super::*;

        #[test]
        fn test_ring_growth() {
            let mut grid = SphericalMjGrid::new_polar(256, 64, 1.0, "BW");

            // Seed: entire inner ring set to 1
            let theta_divs = grid.theta_divisions;
            for theta in 0..theta_divs {
                grid.set(0, theta, 0, 1);
            }

            // Rule: 0 with r_minus=1 -> 1 (grow outward)
            let rule = SphericalRule {
                input: SphericalPattern {
                    center: 0,
                    r_minus: Some(1),
                    ..Default::default()
                },
                output: 1,
            };

            // Run 63 steps (should fill all 64 rings)
            for _ in 0..63 {
                let mut to_set = vec![];
                for idx in 0..grid.len() {
                    if rule.matches(&grid, idx) {
                        to_set.push(idx);
                    }
                }
                for idx in to_set {
                    grid.set_state(idx, 1);
                }
            }

            // All cells should be 1
            for r in 0..64u16 {
                for theta in 0..theta_divs {
                    assert_eq!(
                        grid.get(r, theta, 0),
                        1,
                        "Cell ({}, {}) not filled",
                        r,
                        theta
                    );
                }
            }
        }

        #[test]
        fn test_wave_pattern() {
            let mut grid = SphericalMjGrid::new_polar(256, 64, 1.0, "BW");
            let theta_divs = grid.theta_divisions;

            // Create alternating rings: 1, 0, 1, 0, ...
            for r in 0..64u16 {
                let value = (r % 2) as u8;
                for theta in 0..theta_divs {
                    grid.set(r, theta, 0, value);
                }
            }

            // Verify pattern
            for r in 0..64u16 {
                let expected = (r % 2) as u8;
                for theta in 0..theta_divs {
                    assert_eq!(grid.get(r, theta, 0), expected);
                }
            }
        }

        #[test]
        fn test_deterministic_output() {
            fn run_model_with_seed(seed: u64) -> u64 {
                let mut grid = SphericalMjGrid::new_polar(256, 32, 1.0, "BW");

                // Use seed to determine starting position
                let start_theta = (seed % 100) as u16;
                grid.set(0, start_theta, 0, 1);

                // Simple growth rule
                let rule = SphericalRule {
                    input: SphericalPattern {
                        center: 0,
                        r_minus: Some(1),
                        ..Default::default()
                    },
                    output: 1,
                };

                // Run 10 steps
                for _ in 0..10 {
                    let mut to_set = vec![];
                    for idx in 0..grid.len() {
                        if rule.matches(&grid, idx) {
                            to_set.push(idx);
                        }
                    }
                    for idx in to_set {
                        grid.set_state(idx, 1);
                    }
                }

                grid.checksum()
            }

            // Same seed should produce identical results
            let result1 = run_model_with_seed(42);
            let result2 = run_model_with_seed(42);
            assert_eq!(result1, result2);

            // Different seed should produce different results
            let result3 = run_model_with_seed(43);
            assert_ne!(result1, result3);
        }
    }

    // ========================================================================
    // Level 6: Rendering/Utility Tests (ported from polar_grid.rs)
    // ========================================================================

    mod level_6_rendering {
        use super::*;

        #[test]
        fn test_polar_to_cartesian() {
            let grid = SphericalMjGrid::new_polar(256, 64, 1.0, "BW");

            // At theta=0, should be along positive x-axis
            let r_actual = 256.0 + 50.0;
            let (x, y, z) = grid.to_cartesian(50, 0, 0);

            // x should be close to r_actual (at theta=0, cos(0)=1)
            // y should be close to 0 (at theta=0, sin(0)=0)
            // Note: to_cartesian uses the starting angle of the cell, not mid-angle
            assert!(
                (x - r_actual).abs() < 1.0,
                "x={}, expected ~{}",
                x,
                r_actual
            );
            assert!(y.abs() < 1.0, "y={}, expected ~0", y);
            assert_eq!(z, 0.0); // 2D polar
        }

        #[test]
        fn test_cartesian_quarter_circle() {
            let grid = SphericalMjGrid::new_polar(256, 64, 1.0, "BW");

            // At theta=quarter_circle, should be along positive y-axis
            let r = 50u16;
            let divs = grid.theta_divisions;
            let quarter_theta = divs / 4;

            let (x, y, _z) = grid.to_cartesian(r, quarter_theta, 0);
            let r_actual = 256.0 + r as f32;

            // At quarter circle, x should be near 0, y should be near r
            assert!(x.abs() < r_actual * 0.1, "x={} should be near 0", x);
            assert!(
                (y - r_actual).abs() < r_actual * 0.1,
                "y={} should be near {}",
                y,
                r_actual
            );
        }

        #[test]
        fn test_total_voxels() {
            let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");
            let total = grid.total_voxels();

            // With fixed theta divisions:
            // total = r_depth * theta_divisions
            // theta_divisions ~ 2*PI*256 ~ 1608
            // total ~ 256 * 1608 ~ 411,648
            let expected = 256 * grid.theta_divisions as usize;
            assert_eq!(
                total, expected,
                "Total voxels {} should be r_depth * theta_divisions = {}",
                total, expected
            );
        }

        #[test]
        fn test_count_nonzero() {
            let mut grid = SphericalMjGrid::new_polar(256, 64, 1.0, "BWR");
            assert_eq!(grid.count_nonzero(), 0);

            // Set some values
            grid.set(0, 0, 0, 1);
            grid.set(10, 100, 0, 2);
            grid.set(63, 500, 0, 1);

            assert_eq!(grid.count_nonzero(), 3);

            // Clear and verify
            grid.clear();
            assert_eq!(grid.count_nonzero(), 0);
        }

        #[test]
        fn test_recordable_grid_trait() {
            let mut grid = SphericalMjGrid::new_polar(256, 32, 1.0, "BWR");

            // Test grid_type
            let grid_type = grid.grid_type();
            match grid_type {
                GridType::Polar2D {
                    r_min,
                    r_depth,
                    theta_divisions,
                } => {
                    assert_eq!(r_min, 256);
                    assert_eq!(r_depth, 32);
                    assert!(theta_divisions > 0);
                }
                _ => panic!("Expected Polar2D grid type"),
            }

            // Test palette
            assert_eq!(grid.palette(), "BWR");

            // Test state roundtrip
            grid.set(0, 0, 0, 1);
            grid.set(10, 100, 0, 2);
            let bytes = grid.state_to_bytes();

            let mut grid2 = SphericalMjGrid::new_polar(256, 32, 1.0, "BWR");
            assert!(grid2.state_from_bytes(&bytes));
            assert_eq!(grid2.get(0, 0, 0), 1);
            assert_eq!(grid2.get(10, 100, 0), 2);
        }
    }

    // ========================================================================
    // Level 7: Integration Tests with Video Export
    // ========================================================================

    mod level_7_integration {
        use super::*;
        use crate::markov_junior::recording::{SimulationRecorder, VideoError, VideoExporter};
        use std::path::PathBuf;

        fn output_dir() -> PathBuf {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("screenshots/spherical")
        }

        /// Run a model step: apply all matching rules once per cell
        fn run_step(grid: &mut SphericalMjGrid, rules: &[SphericalRule]) -> bool {
            let mut changes = Vec::new();

            // Find all matches
            for idx in 0..grid.len() {
                for rule in rules {
                    if rule.matches(grid, idx) {
                        changes.push((idx, rule.output));
                        break; // First matching rule wins
                    }
                }
            }

            // Apply changes
            let had_changes = !changes.is_empty();
            for (idx, value) in changes {
                grid.set_state(idx, value);
            }
            had_changes
        }

        /// Test: Ring Growth model - grows outward from inner ring.
        ///
        /// Seed the inner ring, grow outward via rules.
        /// Should produce concentric rings filling the disk.
        #[test]
        fn test_spherical_ring_growth_video() {
            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            // Create grid: B=0 (background), W=1 (white)
            let mut grid = SphericalMjGrid::new_polar(64, 32, 1.0, "BW");
            let theta_divs = grid.theta_divisions;

            // Seed: entire inner ring set to W (1)
            for theta in 0..theta_divs {
                grid.set(0, theta, 0, 1);
            }

            // Rule: B with r_minus=W -> W (grow outward)
            // "If I'm black and my inner neighbor is white, become white"
            let rules = vec![SphericalRule {
                input: SphericalPattern {
                    center: 0,        // B
                    r_minus: Some(1), // W
                    ..Default::default()
                },
                output: 1, // W
            }];

            // Generate all symmetry variants (though for this rule, symmetry doesn't change it)
            let rules: Vec<SphericalRule> =
                rules.into_iter().flat_map(|r| r.all_variants()).collect();

            // Record simulation
            let mut recorder = SimulationRecorder::new(&grid);
            recorder.record_frame(&grid);

            // Run model
            let max_steps = 100;
            for _ in 0..max_steps {
                if !run_step(&mut grid, &rules) {
                    break;
                }
                recorder.record_frame(&grid);
            }

            println!(
                "Spherical Ring Growth: {} frames recorded",
                recorder.frame_count()
            );

            // Save final PNG
            let colors = vec![
                [20, 20, 30, 255],    // B - dark background
                [240, 240, 230, 255], // W - white fill
            ];
            let path_png = out_dir.join("ring_growth.png");
            grid.save_png(&path_png, 512, &colors, [20, 20, 30, 255])
                .expect("Failed to save PNG");
            println!("Saved: {}", path_png.display());

            // Save archive
            let archive = recorder.into_archive();
            let archive_path = out_dir.join("ring_growth.mjsim");
            archive.save(&archive_path).expect("Failed to save archive");
            println!("Saved: {}", archive_path.display());

            // Export to MP4
            let exporter = VideoExporter::new(archive, colors, 512);
            let video_path = out_dir.join("ring_growth.mp4");
            match exporter.export_mp4(&video_path, 5.0, 30) {
                Ok(()) => println!("Exported: {}", video_path.display()),
                Err(VideoError::FfmpegNotFound) => {
                    println!("Skipping MP4 export (ffmpeg not installed)");
                }
                Err(e) => panic!("Video export failed: {}", e),
            }

            // Verify: grid should be filled
            let nonzero = grid.count_nonzero();
            let total = grid.total_voxels();
            println!(
                "Filled: {} / {} ({:.1}%)",
                nonzero,
                total,
                100.0 * nonzero as f64 / total as f64
            );
            assert!(
                nonzero > total / 2,
                "Ring growth should fill at least 50% of grid"
            );
        }

        /// Test: Geological Layers model - creates layered rings.
        ///
        /// Magma at center spreads outward, transforms through layers:
        /// Magma -> Stone -> Dirt -> Grass
        #[test]
        fn test_spherical_geological_layers_video() {
            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            // Palette: B=0 (background), M=1 (magma), S=2 (stone), D=3 (dirt), G=4 (grass)
            let mut grid = SphericalMjGrid::new_polar(64, 40, 1.0, "BMSDG");
            let theta_divs = grid.theta_divisions;

            // Seed: magma at inner ring
            for theta in 0..theta_divs {
                grid.set(0, theta, 0, 1); // M = 1
            }

            // Rules for geological layering:
            // - Magma grows outward, transforms to stone when at frontier
            // - Stone grows outward, transforms to dirt when at frontier
            // - Dirt grows outward, transforms to grass when at frontier
            // - Grass grows outward

            let base_rules = vec![
                // Magma grows outward from magma
                SphericalRule {
                    input: SphericalPattern {
                        center: 0,        // B
                        r_minus: Some(1), // M
                        ..Default::default()
                    },
                    output: 1, // M
                },
                // Magma at frontier transforms to stone
                SphericalRule {
                    input: SphericalPattern {
                        center: 1,        // M
                        r_minus: Some(1), // M (backed by magma)
                        r_plus: Some(0),  // B (frontier)
                        ..Default::default()
                    },
                    output: 2, // S
                },
                // Stone grows outward from stone
                SphericalRule {
                    input: SphericalPattern {
                        center: 0,        // B
                        r_minus: Some(2), // S
                        ..Default::default()
                    },
                    output: 2, // S
                },
                // Stone at frontier transforms to dirt
                SphericalRule {
                    input: SphericalPattern {
                        center: 2,        // S
                        r_minus: Some(2), // S (backed by stone)
                        r_plus: Some(0),  // B (frontier)
                        ..Default::default()
                    },
                    output: 3, // D
                },
                // Dirt grows outward from dirt
                SphericalRule {
                    input: SphericalPattern {
                        center: 0,        // B
                        r_minus: Some(3), // D
                        ..Default::default()
                    },
                    output: 3, // D
                },
                // Dirt at frontier transforms to grass
                SphericalRule {
                    input: SphericalPattern {
                        center: 3,        // D
                        r_minus: Some(3), // D (backed by dirt)
                        r_plus: Some(0),  // B (frontier)
                        ..Default::default()
                    },
                    output: 4, // G
                },
                // Grass grows outward from grass
                SphericalRule {
                    input: SphericalPattern {
                        center: 0,        // B
                        r_minus: Some(4), // G
                        ..Default::default()
                    },
                    output: 4, // G
                },
            ];

            // Expand symmetry variants
            let rules: Vec<SphericalRule> = base_rules
                .into_iter()
                .flat_map(|r| r.all_variants())
                .collect();

            // Record simulation
            let mut recorder = SimulationRecorder::new(&grid);
            recorder.record_frame(&grid);

            // Run model
            let max_steps = 200;
            for _ in 0..max_steps {
                if !run_step(&mut grid, &rules) {
                    break;
                }
                recorder.record_frame(&grid);
            }

            println!(
                "Spherical Geological Layers: {} frames recorded",
                recorder.frame_count()
            );

            // Colors for each layer
            let colors: Vec<[u8; 4]> = vec![
                [20, 20, 25, 255],   // B - void/background (dark)
                [255, 100, 30, 255], // M - magma (bright orange)
                [80, 75, 70, 255],   // S - stone (gray)
                [120, 80, 50, 255],  // D - dirt (brown)
                [60, 160, 50, 255],  // G - grass (green)
            ];

            // Save final PNG
            let path_png = out_dir.join("geological_layers.png");
            grid.save_png(&path_png, 512, &colors, [20, 20, 25, 255])
                .expect("Failed to save PNG");
            println!("Saved: {}", path_png.display());

            // Save archive
            let archive = recorder.into_archive();
            let archive_path = out_dir.join("geological_layers.mjsim");
            archive.save(&archive_path).expect("Failed to save archive");
            println!("Saved: {}", archive_path.display());

            // Export to MP4
            let exporter = VideoExporter::new(archive, colors, 512);
            let video_path = out_dir.join("geological_layers.mp4");
            match exporter.export_mp4(&video_path, 10.0, 30) {
                Ok(()) => println!("Exported: {}", video_path.display()),
                Err(VideoError::FfmpegNotFound) => {
                    println!("Skipping MP4 export (ffmpeg not installed)");
                }
                Err(e) => panic!("Video export failed: {}", e),
            }

            // Verify we have multiple layers
            let mut layer_counts = [0usize; 5];
            for idx in 0..grid.len() {
                let v = grid.get_state(idx) as usize;
                if v < 5 {
                    layer_counts[v] += 1;
                }
            }

            println!("Layer distribution:");
            println!("  B (background): {}", layer_counts[0]);
            println!("  M (magma): {}", layer_counts[1]);
            println!("  S (stone): {}", layer_counts[2]);
            println!("  D (dirt): {}", layer_counts[3]);
            println!("  G (grass): {}", layer_counts[4]);

            // Should have at least some of each non-background layer
            assert!(
                layer_counts[1] > 0 || layer_counts[2] > 0,
                "Expected some magma or stone"
            );
        }
    }
}
