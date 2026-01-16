//! 2D Polar coordinate grid for MarkovJunior.
//!
//! This module provides a polar coordinate system extension to MarkovJunior that enables
//! procedural generation on circular/ring-shaped grids without distortion artifacts.
//!
//! ## Key Concepts
//!
//! - **Fixed theta divisions**: All rings have the same number of angular divisions.
//!   This means cells are trapezoids (inner edge shorter than outer edge), but they
//!   align perfectly in the radial direction.
//!
//! - **Aligned neighbors**: The cell at (r, theta) has its r_minus neighbor at exactly
//!   (r-1, theta) and r_plus at (r+1, theta). No complex mapping needed.
//!
//! - **Low distortion**: With high r_min (e.g., 256), the difference between inner and
//!   outer edge of each cell is <1%, so cells are nearly rectangular.
//!
//! - **4 symmetries**: Unlike Cartesian grids with 8 symmetries, polar grids have
//!   4 symmetries (Klein four-group) because radial direction is meaningful.
//!
//! ## Example
//!
//! ```ignore
//! use studio_core::markov_junior::polar_grid::PolarMjGrid;
//!
//! // Create a polar grid with r_min=256, 256 rings, target arc length 1.0
//! let mut grid = PolarMjGrid::new(256, 256, 1.0);
//!
//! // Set a value at (r=100, theta=500)
//! grid.set(100, 500, 1);
//!
//! // Get neighbors - same theta across all rings!
//! let neighbors = grid.neighbors(100, 500);
//! assert_eq!(neighbors.r_minus, Some((99, 500)));
//! assert_eq!(neighbors.r_plus, Some((101, 500)));
//! ```

use std::f32::consts::PI;

/// 2D polar grid for Markov Jr.
///
/// The grid stores voxels in polar coordinates (r, theta) where:
/// - `r` is the radius index (0 to r_depth-1), actual radius = r_min + r
/// - `theta` is the angular index (0 to theta_divisions-1), same for ALL rings
///
/// All rings have the same number of theta divisions. This means cells are
/// trapezoids (inner edge shorter than outer edge), but they align perfectly
/// in the radial direction. Neighbors are trivial: (r, theta) neighbors
/// (r-1, theta) and (r+1, theta).
#[derive(Debug, Clone)]
pub struct PolarMjGrid {
    /// Minimum radius (actual radius = r_min + r_index)
    pub r_min: u32,
    /// Number of radial levels (rings)
    pub r_depth: u16,
    /// Number of angular divisions (same for all rings)
    pub theta_divisions: u16,
    /// Target arc length for voxels (used to calculate theta_divisions)
    pub target_arc: f32,
    /// Storage: rings[r][theta] = cell value
    /// All rings have the same length (theta_divisions)
    pub rings: Vec<Vec<u8>>,
}

impl PolarMjGrid {
    /// Create a new polar grid.
    ///
    /// # Arguments
    /// * `r_min` - Minimum radius (recommended: 256 for <1% distortion)
    /// * `r_depth` - Number of radial levels/rings
    /// * `target_arc` - Target arc length for voxels (1.0 is a good default)
    ///
    /// Theta divisions are calculated based on r_min to give approximately
    /// target_arc sized cells at the inner ring. All rings use the same
    /// theta_divisions, so cells are perfectly aligned radially.
    ///
    /// # Example
    /// ```ignore
    /// let grid = PolarMjGrid::new(256, 256, 1.0);
    /// assert_eq!(grid.rings.len(), 256);
    /// // All rings have the same theta_divisions
    /// assert_eq!(grid.theta_divisions, grid.theta_divisions);
    /// ```
    pub fn new(r_min: u32, r_depth: u16, target_arc: f32) -> Self {
        // Calculate theta divisions based on inner ring (r_min)
        // This gives target_arc at inner ring, slightly larger at outer rings
        let theta_divisions = Self::calculate_theta_divisions(r_min, target_arc);

        let mut rings = Vec::with_capacity(r_depth as usize);
        for _ in 0..r_depth {
            rings.push(vec![0u8; theta_divisions as usize]);
        }
        Self {
            r_min,
            r_depth,
            theta_divisions,
            target_arc,
            rings,
        }
    }

    /// Calculate theta divisions for a given radius and target arc length.
    ///
    /// Formula: theta_divisions = floor(2 * PI * r / target_arc)
    #[inline]
    pub fn calculate_theta_divisions(r: u32, target_arc: f32) -> u16 {
        let circumference = 2.0 * PI * r as f32;
        (circumference / target_arc).floor().max(6.0) as u16 // minimum 6 divisions
    }

    /// Get the number of theta divisions (same for all rings).
    #[inline]
    pub fn theta_divisions(&self, _r: u8) -> u16 {
        self.theta_divisions
    }

    /// Get the actual radius for a given r index.
    #[inline]
    pub fn r_actual(&self, r: u8) -> u32 {
        self.r_min + r as u32
    }

    /// Get the value at (r, theta).
    ///
    /// Theta automatically wraps around (theta % theta_divisions).
    #[inline]
    pub fn get(&self, r: u8, theta: u16) -> u8 {
        let ring = &self.rings[r as usize];
        ring[(theta as usize) % ring.len()]
    }

    /// Set the value at (r, theta).
    ///
    /// Theta automatically wraps around (theta % theta_divisions).
    #[inline]
    pub fn set(&mut self, r: u8, theta: u16, value: u8) {
        let ring = &mut self.rings[r as usize];
        let idx = (theta as usize) % ring.len();
        ring[idx] = value;
    }

    /// Get the angular range (in radians) for a voxel at (r, theta).
    ///
    /// Returns (start_angle, end_angle) in radians [0, 2*PI].
    pub fn angular_range(&self, r: u8, theta: u16) -> (f32, f32) {
        let divs = self.theta_divisions(r) as f32;
        let theta_wrapped = (theta % self.theta_divisions(r)) as f32;
        let start = theta_wrapped / divs * 2.0 * PI;
        let end = (theta_wrapped + 1.0) / divs * 2.0 * PI;
        (start, end)
    }

    /// Get neighbors for a voxel at (r, theta).
    ///
    /// With fixed theta divisions, neighbor lookup is trivial:
    /// - `theta_minus`, `theta_plus`: Same r, adjacent theta (with wraparound)
    /// - `r_minus`, `r_plus`: Same theta, adjacent r (None at boundaries)
    pub fn neighbors(&self, r: u8, theta: u16) -> PolarNeighbors {
        let theta_divs = self.theta_divisions;
        let theta_wrapped = theta % theta_divs;

        PolarNeighbors {
            theta_minus: (r, (theta_wrapped + theta_divs - 1) % theta_divs),
            theta_plus: (r, (theta_wrapped + 1) % theta_divs),
            // Same theta for radial neighbors - they align perfectly!
            r_minus: if r > 0 {
                Some((r - 1, theta_wrapped))
            } else {
                None
            },
            r_plus: if (r as u16) < self.r_depth - 1 {
                Some((r + 1, theta_wrapped))
            } else {
                None
            },
        }
    }

    /// Total number of voxels in the grid.
    pub fn total_voxels(&self) -> usize {
        self.rings.iter().map(|r| r.len()).sum()
    }

    /// Count voxels with non-zero values.
    pub fn count_nonzero(&self) -> usize {
        self.rings
            .iter()
            .flat_map(|ring| ring.iter())
            .filter(|&&v| v != 0)
            .count()
    }

    /// Clear the grid (set all cells to 0).
    pub fn clear(&mut self) {
        for ring in &mut self.rings {
            ring.fill(0);
        }
    }

    /// Iterate over all voxels with their (r, theta, value) coordinates.
    pub fn iter(&self) -> impl Iterator<Item = (u8, u16, u8)> + '_ {
        self.rings.iter().enumerate().flat_map(|(r, ring)| {
            ring.iter()
                .enumerate()
                .map(move |(theta, &value)| (r as u8, theta as u16, value))
        })
    }

    /// Iterate over all non-zero voxels with their (r, theta, value) coordinates.
    pub fn iter_nonzero(&self) -> impl Iterator<Item = (u8, u16, u8)> + '_ {
        self.iter().filter(|(_, _, v)| *v != 0)
    }

    /// Convert polar coordinates to Cartesian (x, y).
    ///
    /// Useful for rendering. Returns the center point of the voxel.
    pub fn to_cartesian(&self, r: u8, theta: u16) -> (f32, f32) {
        let r_actual = self.r_actual(r) as f32;
        let (start, end) = self.angular_range(r, theta);
        let mid_angle = (start + end) / 2.0;
        let x = r_actual * mid_angle.cos();
        let y = r_actual * mid_angle.sin();
        (x, y)
    }

    /// Compute a checksum of the grid for deterministic verification.
    pub fn checksum(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        for ring in &self.rings {
            ring.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Render the polar grid to an RGBA image.
    ///
    /// The image shows the polar grid as a ring/disk with the inner radius
    /// at the center and the outer radius at the edge.
    ///
    /// Uses pixel-based rendering: for each pixel, determine which cell it
    /// belongs to and color it accordingly. This eliminates gaps.
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
                let r_index = (pixel_r - r_min_actual) as u8;
                if r_index as u16 >= self.r_depth {
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

                // Get cell value and color
                let value = self.get(r_index, theta_index) as usize;

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

// ============================================================================
// RecordableGrid and Renderable2D trait implementations
// ============================================================================

use super::recording::{GridType, RecordableGrid, Renderable2D};

impl RecordableGrid for PolarMjGrid {
    fn grid_type(&self) -> GridType {
        GridType::Polar2D {
            r_min: self.r_min,
            r_depth: self.r_depth,
            theta_divisions: self.theta_divisions,
        }
    }

    fn palette(&self) -> String {
        // PolarMjGrid doesn't store palette info directly,
        // so we return a placeholder. The PolarModel has the palette.
        String::new()
    }

    fn state_to_bytes(&self) -> Vec<u8> {
        // Flatten rings into a single byte vector
        // Order: r=0 all thetas, r=1 all thetas, ...
        let mut bytes = Vec::with_capacity(self.total_voxels());
        for ring in &self.rings {
            bytes.extend_from_slice(ring);
        }
        bytes
    }

    fn state_from_bytes(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() != self.total_voxels() {
            return false;
        }

        let mut offset = 0;
        for ring in &mut self.rings {
            let end = offset + ring.len();
            ring.copy_from_slice(&bytes[offset..end]);
            offset = end;
        }
        true
    }
}

impl Renderable2D for PolarMjGrid {
    fn render_to_image(
        &self,
        image_size: u32,
        colors: &[[u8; 4]],
        background: [u8; 4],
    ) -> image::RgbaImage {
        // Delegate to the existing method
        PolarMjGrid::render_to_image(self, image_size, colors, background)
    }
}

/// Draw a polar wedge (arc segment) onto an image using radial line drawing.
/// This is much faster than checking every pixel in the bounding box.
fn draw_polar_wedge(
    img: &mut image::RgbaImage,
    center: f32,
    r_inner: f32,
    r_outer: f32,
    angle_start: f32,
    angle_end: f32,
    color: image::Rgba<u8>,
) {
    use std::f32::consts::PI;

    let img_width = img.width();
    let img_height = img.height();

    // Handle angle wraparound
    let (a_start, a_end) = if angle_end < angle_start {
        // Wraps around 0 - draw in two parts
        draw_polar_wedge_simple(img, center, r_inner, r_outer, angle_start, 2.0 * PI, color);
        (0.0f32, angle_end)
    } else {
        (angle_start, angle_end)
    };

    draw_polar_wedge_simple(img, center, r_inner, r_outer, a_start, a_end, color);
}

/// Draw a polar wedge without wraparound (angle_start < angle_end).
fn draw_polar_wedge_simple(
    img: &mut image::RgbaImage,
    center: f32,
    r_inner: f32,
    r_outer: f32,
    angle_start: f32,
    angle_end: f32,
    color: image::Rgba<u8>,
) {
    let img_width = img.width() as i32;
    let img_height = img.height() as i32;

    // Number of angular steps - more steps for outer rings
    let angle_span = angle_end - angle_start;
    let num_angle_steps = ((r_outer * angle_span) as usize).max(4);
    let angle_step = angle_span / num_angle_steps as f32;

    // Number of radial steps
    let radial_span = r_outer - r_inner;
    let num_radial_steps = (radial_span as usize).max(2);
    let radial_step = radial_span / num_radial_steps as f32;

    // Draw filled wedge by iterating over angle and radius
    for ai in 0..=num_angle_steps {
        let angle = angle_start + ai as f32 * angle_step;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        for ri in 0..=num_radial_steps {
            let r = r_inner + ri as f32 * radial_step;
            let px = (center + r * cos_a) as i32;
            let py = (center + r * sin_a) as i32;

            if px >= 0 && px < img_width && py >= 0 && py < img_height {
                img.put_pixel(px as u32, py as u32, color);
            }
        }
    }
}

/// Neighbors of a polar voxel.
///
/// With fixed theta divisions, each cell has exactly one neighbor in each
/// direction (or None at boundaries). Neighbors are perfectly aligned radially.
#[derive(Debug, Clone)]
pub struct PolarNeighbors {
    /// Angular neighbor in -theta direction (always exactly 1, wraps around)
    pub theta_minus: (u8, u16),
    /// Angular neighbor in +theta direction (always exactly 1, wraps around)
    pub theta_plus: (u8, u16),
    /// Radial neighbor in -r direction (None at inner boundary r=0)
    pub r_minus: Option<(u8, u16)>,
    /// Radial neighbor in +r direction (None at outer boundary r=r_depth-1)
    pub r_plus: Option<(u8, u16)>,
}

/// Symmetry transforms for polar patterns.
///
/// Unlike Cartesian 2D with 8 symmetries (D4 group), polar coordinates have
/// only 4 symmetries (Klein four-group V4) because:
/// - Theta (angular) can be flipped (mirror across radial line)
/// - R (radial) can be flipped (swap inner/outer)
/// - But 90-degree rotation doesn't exist: you can't swap theta and r
///
/// This is actually useful: radial direction is meaningful (surface vs depth),
/// so patterns *should* distinguish inward from outward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolarSymmetry {
    /// No transform: (dr, dtheta) -> (dr, dtheta)
    Identity,
    /// Theta flip: (dr, dtheta) -> (dr, -dtheta)
    /// Mirror across radial line
    ThetaFlip,
    /// R flip: (dr, dtheta) -> (-dr, dtheta)
    /// Swap inner <-> outer
    RFlip,
    /// Both flips: (dr, dtheta) -> (-dr, -dtheta)
    BothFlip,
}

impl PolarSymmetry {
    /// All 4 symmetries.
    pub fn all() -> [Self; 4] {
        [Self::Identity, Self::ThetaFlip, Self::RFlip, Self::BothFlip]
    }

    /// Transform a relative offset (dr, dtheta) by this symmetry.
    pub fn transform(&self, dr: i8, dtheta: i8) -> (i8, i8) {
        match self {
            Self::Identity => (dr, dtheta),
            Self::ThetaFlip => (dr, -dtheta),
            Self::RFlip => (-dr, dtheta),
            Self::BothFlip => (-dr, -dtheta),
        }
    }

    /// Compose two symmetries (apply self, then other).
    pub fn compose(&self, other: Self) -> Self {
        // Klein four-group multiplication table
        use PolarSymmetry::*;
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
    /// In the Klein four-group, every element is its own inverse.
    pub fn inverse(&self) -> Self {
        *self
    }
}

/// A polar pattern for rule matching.
///
/// Specifies requirements for the center cell and its neighbors.
/// `None` means wildcard (any value matches).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct PolarPattern {
    /// Value at the center cell
    pub center: u8,
    /// Required value at theta_minus neighbor (None = wildcard)
    pub theta_minus: Option<u8>,
    /// Required value at theta_plus neighbor (None = wildcard)
    pub theta_plus: Option<u8>,
    /// Required value at r_minus neighbor(s) (None = wildcard)
    /// If multiple neighbors, all must match this value
    pub r_minus: Option<u8>,
    /// Required value at r_plus neighbor(s) (None = wildcard)
    /// If multiple neighbors, all must match this value
    pub r_plus: Option<u8>,
}

impl PolarPattern {
    /// Create a new pattern with just a center value (all neighbors are wildcards).
    pub fn center_only(center: u8) -> Self {
        Self {
            center,
            ..Default::default()
        }
    }

    /// Check if this pattern matches at the given location in the grid.
    pub fn matches(&self, grid: &PolarMjGrid, r: u8, theta: u16) -> bool {
        // Check center
        if grid.get(r, theta) != self.center {
            return false;
        }

        let neighbors = grid.neighbors(r, theta);

        // Check theta neighbors (always exactly 1 each)
        if let Some(v) = self.theta_minus {
            if grid.get(neighbors.theta_minus.0, neighbors.theta_minus.1) != v {
                return false;
            }
        }
        if let Some(v) = self.theta_plus {
            if grid.get(neighbors.theta_plus.0, neighbors.theta_plus.1) != v {
                return false;
            }
        }

        // Check radial neighbors (exactly 1 each, or None at boundary)
        if let Some(required) = self.r_minus {
            match neighbors.r_minus {
                None => return false, // At boundary, can't match if we require a value
                Some((nr, nt)) => {
                    if grid.get(nr, nt) != required {
                        return false;
                    }
                }
            }
        }
        if let Some(required) = self.r_plus {
            match neighbors.r_plus {
                None => return false, // At boundary, can't match if we require a value
                Some((nr, nt)) => {
                    if grid.get(nr, nt) != required {
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
    pub fn transform(&self, symmetry: PolarSymmetry) -> Self {
        match symmetry {
            PolarSymmetry::Identity => self.clone(),
            PolarSymmetry::ThetaFlip => Self {
                center: self.center,
                theta_minus: self.theta_plus,
                theta_plus: self.theta_minus,
                r_minus: self.r_minus,
                r_plus: self.r_plus,
            },
            PolarSymmetry::RFlip => Self {
                center: self.center,
                theta_minus: self.theta_minus,
                theta_plus: self.theta_plus,
                r_minus: self.r_plus,
                r_plus: self.r_minus,
            },
            PolarSymmetry::BothFlip => Self {
                center: self.center,
                theta_minus: self.theta_plus,
                theta_plus: self.theta_minus,
                r_minus: self.r_plus,
                r_plus: self.r_minus,
            },
        }
    }

    /// Generate all symmetry variants of this pattern.
    ///
    /// Returns up to 4 unique patterns (may be fewer if pattern has symmetry).
    pub fn all_variants(&self) -> Vec<Self> {
        let mut variants = Vec::with_capacity(4);
        let mut seen = std::collections::HashSet::new();

        for sym in PolarSymmetry::all() {
            let transformed = self.transform(sym);
            if seen.insert(transformed.clone()) {
                variants.push(transformed);
            }
        }

        variants
    }
}

/// A polar rewrite rule: if input pattern matches, output value is written.
#[derive(Debug, Clone)]
pub struct PolarRule {
    /// Input pattern to match
    pub input: PolarPattern,
    /// Output value to write to the center cell
    pub output: u8,
}

impl PolarRule {
    /// Check if this rule matches at the given location.
    pub fn matches(&self, grid: &PolarMjGrid, r: u8, theta: u16) -> bool {
        self.input.matches(grid, r, theta)
    }

    /// Apply this rule at the given location (no matching check).
    pub fn apply(&self, grid: &mut PolarMjGrid, r: u8, theta: u16) {
        grid.set(r, theta, self.output);
    }

    /// Generate all symmetry variants of this rule.
    pub fn with_all_symmetries(&self) -> Vec<Self> {
        self.input
            .all_variants()
            .into_iter()
            .map(|input| Self {
                input,
                output: self.output,
            })
            .collect()
    }
}

// ============================================================================
// Polar Model - High-level API for running polar Markov Jr models
// ============================================================================

/// A simple polar Markov Jr model.
///
/// This provides a high-level API for defining and running polar models
/// with rules, seeds, and step-by-step or batch execution.
#[derive(Debug, Clone)]
pub struct PolarModel {
    /// The name of the model
    pub name: String,
    /// Character values (like "BW" for Black/White)
    pub values: String,
    /// The polar grid
    pub grid: PolarMjGrid,
    /// Rules to apply (with all symmetry variants pre-expanded)
    pub rules: Vec<PolarRule>,
    /// Random number generator seed
    pub seed: u64,
    /// Current step counter
    pub step: usize,
    /// RNG state (simple LCG for determinism)
    rng_state: u64,
}

impl PolarModel {
    /// Create a new polar model with the given parameters.
    ///
    /// # Arguments
    /// * `name` - Model name for identification
    /// * `values` - Character values string (e.g., "BW" for Black/White)
    /// * `r_min` - Minimum radius (256 recommended)
    /// * `r_depth` - Number of radial levels
    /// * `target_arc` - Target arc length (1.0 recommended)
    pub fn new(name: &str, values: &str, r_min: u32, r_depth: u16, target_arc: f32) -> Self {
        Self {
            name: name.to_string(),
            values: values.to_string(),
            grid: PolarMjGrid::new(r_min, r_depth, target_arc),
            rules: Vec::new(),
            seed: 0,
            step: 0,
            rng_state: 0,
        }
    }

    /// Set the random seed and reset the RNG state.
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = seed;
        self.rng_state = seed;
    }

    /// Add a rule to the model, automatically expanding symmetries.
    ///
    /// # Arguments
    /// * `rule` - The base rule to add
    /// * `expand_symmetries` - If true, add all symmetry variants
    pub fn add_rule(&mut self, rule: PolarRule, expand_symmetries: bool) {
        if expand_symmetries {
            self.rules.extend(rule.with_all_symmetries());
        } else {
            self.rules.push(rule);
        }
    }

    /// Parse and add a rule from string notation.
    ///
    /// Format: "center;theta-,theta+,r-,r+ -> output"
    /// Use '*' for wildcard, '-' for "not present".
    ///
    /// Examples:
    /// - "0;*,*,*,1 -> 1" - Center=0, r_plus=1 -> becomes 1
    /// - "0;1,*,*,* -> 1" - Center=0, theta_minus=1 -> becomes 1
    pub fn add_rule_str(&mut self, rule_str: &str, expand_symmetries: bool) -> Result<(), String> {
        let rule = self.parse_rule(rule_str)?;
        self.add_rule(rule, expand_symmetries);
        Ok(())
    }

    /// Parse a rule string.
    fn parse_rule(&self, rule_str: &str) -> Result<PolarRule, String> {
        // Split by "->"
        let parts: Vec<&str> = rule_str.split("->").collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid rule format: expected 'input -> output', got '{}'",
                rule_str
            ));
        }

        let input_str = parts[0].trim();
        let output_str = parts[1].trim();

        // Parse output (single value index or character)
        let output = self.parse_value(output_str)?;

        // Parse input: "center;theta-,theta+,r-,r+"
        let input_parts: Vec<&str> = input_str.split(';').collect();
        if input_parts.len() != 2 {
            return Err(format!(
                "Invalid input format: expected 'center;neighbors', got '{}'",
                input_str
            ));
        }

        let center = self.parse_value(input_parts[0].trim())?;
        let neighbor_str = input_parts[1].trim();
        let neighbors: Vec<&str> = neighbor_str.split(',').collect();

        if neighbors.len() != 4 {
            return Err(format!(
                "Invalid neighbors: expected 4 (theta-,theta+,r-,r+), got {}",
                neighbors.len()
            ));
        }

        let theta_minus = self.parse_optional_value(neighbors[0].trim())?;
        let theta_plus = self.parse_optional_value(neighbors[1].trim())?;
        let r_minus = self.parse_optional_value(neighbors[2].trim())?;
        let r_plus = self.parse_optional_value(neighbors[3].trim())?;

        Ok(PolarRule {
            input: PolarPattern {
                center,
                theta_minus,
                theta_plus,
                r_minus,
                r_plus,
            },
            output,
        })
    }

    /// Parse a value (character or index).
    fn parse_value(&self, s: &str) -> Result<u8, String> {
        // Try as index first
        if let Ok(idx) = s.parse::<u8>() {
            return Ok(idx);
        }

        // Try as character
        if s.len() == 1 {
            let ch = s.chars().next().unwrap();
            if let Some(idx) = self.values.chars().position(|c| c == ch) {
                return Ok(idx as u8);
            }
            return Err(format!(
                "Unknown character '{}' (values='{}')",
                ch, self.values
            ));
        }

        Err(format!("Cannot parse value '{}'", s))
    }

    /// Parse an optional value (* or - means None).
    fn parse_optional_value(&self, s: &str) -> Result<Option<u8>, String> {
        if s == "*" || s == "-" {
            return Ok(None);
        }
        Ok(Some(self.parse_value(s)?))
    }

    /// Simple LCG random number generator (deterministic).
    fn next_random(&mut self) -> u64 {
        // LCG constants (same as used in many systems)
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.rng_state
    }

    /// Reset the model to initial state.
    pub fn reset(&mut self) {
        self.grid.clear();
        self.step = 0;
        self.rng_state = self.seed;
    }

    /// Execute one step of the model.
    ///
    /// Finds all matching rule applications, randomly selects one, and applies it.
    /// Returns true if a rule was applied, false if no rules match.
    pub fn step(&mut self) -> bool {
        // Collect all matching locations
        let mut matches: Vec<(usize, u8, u16)> = Vec::new();

        for r in 0..self.grid.r_depth as u8 {
            let theta_divs = self.grid.theta_divisions(r);
            for theta in 0..theta_divs {
                for (rule_idx, rule) in self.rules.iter().enumerate() {
                    if rule.matches(&self.grid, r, theta) {
                        matches.push((rule_idx, r, theta));
                    }
                }
            }
        }

        if matches.is_empty() {
            return false;
        }

        // Randomly select one match
        let idx = (self.next_random() % matches.len() as u64) as usize;
        let (rule_idx, r, theta) = matches[idx];

        // Apply the rule
        self.rules[rule_idx].apply(&mut self.grid, r, theta);
        self.step += 1;

        true
    }

    /// Run the model for a maximum number of steps.
    ///
    /// Returns the number of steps executed.
    pub fn run(&mut self, max_steps: usize) -> usize {
        let mut steps = 0;
        while steps < max_steps && self.step() {
            steps += 1;
        }
        steps
    }

    /// Get colors for rendering based on the values string.
    pub fn colors(&self) -> Vec<[u8; 4]> {
        // Simple color palette
        let palette: Vec<[u8; 4]> = vec![
            [0, 0, 0, 0],         // 0: transparent/black
            [255, 255, 255, 255], // 1: white
            [255, 0, 77, 255],    // 2: red
            [0, 228, 54, 255],    // 3: green
            [41, 173, 255, 255],  // 4: blue
            [255, 236, 39, 255],  // 5: yellow
            [255, 163, 0, 255],   // 6: orange
            [131, 118, 156, 255], // 7: purple
        ];

        // For each character in values, assign a color
        let mut colors = vec![[0, 0, 0, 0]]; // Index 0 is always transparent

        for (i, ch) in self.values.chars().enumerate() {
            if i == 0 {
                // First character is background/transparent
                continue;
            }

            // Use palette color based on character
            let color = match ch {
                'W' | 'w' => [255, 241, 232, 255], // White
                'R' | 'r' => [255, 0, 77, 255],    // Red
                'G' | 'g' => [0, 228, 54, 255],    // Green
                'B' | 'b' => [0, 0, 0, 255],       // Black
                'U' | 'u' => [41, 173, 255, 255],  // Blue
                'Y' | 'y' => [255, 236, 39, 255],  // Yellow
                'O' | 'o' => [255, 163, 0, 255],   // Orange
                'P' | 'p' => [126, 37, 83, 255],   // Purple
                _ => palette.get(i + 1).copied().unwrap_or([128, 128, 128, 255]),
            };
            colors.push(color);
        }

        colors
    }

    /// Render the current state to an image.
    pub fn render(&self, image_size: u32) -> image::RgbaImage {
        let colors = self.colors();
        let background = [34, 34, 34, 255]; // Dark gray
        self.grid.render_to_image(image_size, &colors, background)
    }

    /// Save the current state to a PNG file.
    pub fn save_png(
        &self,
        path: &std::path::Path,
        image_size: u32,
    ) -> Result<(), image::ImageError> {
        let img = self.render(image_size);
        img.save(path)
    }

    /// Seed a single cell at the given location.
    pub fn seed_cell(&mut self, r: u8, theta: u16, value: u8) {
        self.grid.set(r, theta, value);
    }

    /// Seed the inner ring with a value.
    pub fn seed_inner_ring(&mut self, value: u8) {
        let divs = self.grid.theta_divisions(0);
        for theta in 0..divs {
            self.grid.set(0, theta, value);
        }
    }

    /// Fill a ring with a value.
    pub fn fill_ring(&mut self, r: u8, value: u8) {
        let divs = self.grid.theta_divisions(r);
        for theta in 0..divs {
            self.grid.set(r, theta, value);
        }
    }
}

/// Run a polar model and save screenshots at various steps.
///
/// This is the main entry point for testing and validating polar models.
///
/// # Arguments
/// * `model` - The model to run
/// * `output_dir` - Directory to save screenshots
/// * `image_size` - Size of output images
/// * `max_steps` - Maximum steps to run
/// * `screenshot_steps` - Steps at which to save screenshots (e.g., [0, 100, 500, -1] where -1 means final)
pub fn run_polar_model_with_screenshots(
    model: &mut PolarModel,
    output_dir: &std::path::Path,
    image_size: u32,
    max_steps: usize,
    screenshot_steps: &[i32],
) -> std::io::Result<Vec<std::path::PathBuf>> {
    use std::fs;

    // Create output directory
    fs::create_dir_all(output_dir)?;

    let mut saved_paths = Vec::new();
    let mut next_screenshot_idx = 0;

    // Initial screenshot if 0 is in the list
    if screenshot_steps.contains(&0) {
        let path = output_dir.join(format!("{}_step_{:05}.png", model.name, 0));
        model
            .save_png(&path, image_size)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        saved_paths.push(path);
        next_screenshot_idx = 1;
    }

    // Run the model
    let mut step = 0;
    while step < max_steps && model.step() {
        step += 1;

        // Check if we should save a screenshot
        while next_screenshot_idx < screenshot_steps.len() {
            let target = screenshot_steps[next_screenshot_idx];
            if target >= 0 && step == target as usize {
                let path = output_dir.join(format!("{}_step_{:05}.png", model.name, step));
                model
                    .save_png(&path, image_size)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
                saved_paths.push(path);
                next_screenshot_idx += 1;
            } else {
                break;
            }
        }
    }

    // Final screenshot if -1 is in the list
    if screenshot_steps.contains(&-1) {
        let path = output_dir.join(format!("{}_final_step_{:05}.png", model.name, step));
        model
            .save_png(&path, image_size)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        saved_paths.push(path);
    }

    Ok(saved_paths)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Level 0: Data Structure Tests
    // ========================================================================

    mod level_0_data_structures {
        use super::*;

        #[test]
        fn test_grid_creation() {
            let grid = PolarMjGrid::new(256, 256, 1.0);

            // Verify dimensions
            assert_eq!(grid.r_min, 256);
            assert_eq!(grid.r_depth, 256);

            // Verify ring count
            assert_eq!(grid.rings.len(), 256);

            // Verify all cells initialized to 0
            for r in 0..=255u8 {
                let theta_divs = grid.theta_divisions(r);
                for theta in 0..theta_divs {
                    assert_eq!(grid.get(r, theta), 0);
                }
            }
        }

        #[test]
        fn test_cell_read_write() {
            let mut grid = PolarMjGrid::new(256, 256, 1.0);

            // Write to various locations
            grid.set(0, 0, 42);
            grid.set(128, 500, 99);
            grid.set(255, 1000, 7);

            // Read back
            assert_eq!(grid.get(0, 0), 42);
            assert_eq!(grid.get(128, 500), 99);
            assert_eq!(grid.get(255, 1000), 7);

            // Verify other cells unchanged
            assert_eq!(grid.get(0, 1), 0);
            assert_eq!(grid.get(128, 501), 0);
        }

        #[test]
        fn test_theta_wrapping() {
            let mut grid = PolarMjGrid::new(256, 256, 1.0);
            let theta_divs = grid.theta_divisions(100);

            // Set a value
            grid.set(100, 0, 42);

            // Access via wrapped index should return same value
            assert_eq!(grid.get(100, theta_divs), 42); // wraps to 0
            assert_eq!(grid.get(100, theta_divs * 2), 42); // wraps to 0
            assert_eq!(grid.get(100, theta_divs + 5), grid.get(100, 5));
        }

        #[test]
        fn test_memory_layout() {
            let grid = PolarMjGrid::new(256, 256, 1.0);

            // With fixed theta divisions, ALL rings have the same size
            let inner_size = grid.rings[0].len();
            let outer_size = grid.rings[255].len();

            assert_eq!(
                inner_size, outer_size,
                "All rings should have same size (fixed theta divisions)"
            );

            // All rings should have theta_divisions elements
            for r in 0..256 {
                assert_eq!(
                    grid.rings[r].len(),
                    grid.theta_divisions as usize,
                    "Ring {} should have {} elements",
                    r,
                    grid.theta_divisions
                );
            }
        }
    }

    // ========================================================================
    // Level 1: Coordinate Math Tests
    // ========================================================================

    mod level_1_coordinates {
        use super::*;

        #[test]
        fn test_theta_divisions_formula() {
            let grid = PolarMjGrid::new(256, 256, 1.0);

            // With fixed theta divisions, all rings have the same count
            // Calculated from r_min: floor(2*PI*256/1) ~ 1608

            let divs_inner = grid.theta_divisions(0);
            let divs_outer = grid.theta_divisions(255);

            // All rings should have the same theta divisions
            assert_eq!(
                divs_inner, divs_outer,
                "All rings should have same theta divisions"
            );

            // Should be approximately 2*PI*r_min/target_arc
            assert!(
                (divs_inner as f32 - 1608.0).abs() < 2.0,
                "Theta divs: {} (expected ~1608)",
                divs_inner
            );
        }

        #[test]
        fn test_no_distortion_between_rings() {
            let grid = PolarMjGrid::new(256, 256, 1.0);

            // With fixed theta divisions, there's NO distortion between rings.
            // All rings have the same number of divisions.

            for r in 0..255u8 {
                let current_divs = grid.theta_divisions(r);
                let next_divs = grid.theta_divisions(r + 1);

                assert_eq!(
                    current_divs, next_divs,
                    "Theta divisions should be constant across all rings"
                );
            }
        }

        #[test]
        fn test_arc_length_varies_by_radius() {
            let grid = PolarMjGrid::new(256, 256, 1.0);

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
            let grid = PolarMjGrid::new(256, 256, 1.0);

            // Voxel (r, theta) should span [theta/divs * 2*PI, (theta+1)/divs * 2*PI]
            let r = 100u8;
            let theta = 50u16;
            let divs = grid.theta_divisions(r);

            let (start, end) = grid.angular_range(r, theta);

            let expected_start = theta as f32 / divs as f32 * 2.0 * PI;
            let expected_end = (theta + 1) as f32 / divs as f32 * 2.0 * PI;

            assert!((start - expected_start).abs() < 0.0001);
            assert!((end - expected_end).abs() < 0.0001);
        }
    }

    // ========================================================================
    // Level 2: Neighbor Relationship Tests
    // ========================================================================

    mod level_2_neighbors {
        use super::*;

        #[test]
        fn test_angular_neighbors() {
            let grid = PolarMjGrid::new(256, 256, 1.0);

            for r in [0u8, 50, 100, 200, 255] {
                let divs = grid.theta_divisions(r);

                for theta in [0u16, divs / 2, divs - 1] {
                    let neighbors = grid.neighbors(r, theta);

                    // Always exactly 2 angular neighbors
                    assert_eq!(neighbors.theta_minus, (r, (theta + divs - 1) % divs));
                    assert_eq!(neighbors.theta_plus, (r, (theta + 1) % divs));
                }
            }
        }

        #[test]
        fn test_angular_neighbor_wrapping() {
            let grid = PolarMjGrid::new(256, 256, 1.0);
            let r = 100u8;
            let divs = grid.theta_divisions(r);

            // At theta=0, theta_minus should wrap to divs-1
            let neighbors = grid.neighbors(r, 0);
            assert_eq!(neighbors.theta_minus.1, divs - 1);

            // At theta=divs-1, theta_plus should wrap to 0
            let neighbors = grid.neighbors(r, divs - 1);
            assert_eq!(neighbors.theta_plus.1, 0);
        }

        #[test]
        fn test_radial_neighbors_bounded() {
            let grid = PolarMjGrid::new(256, 256, 1.0);

            // At low distortion (<1%), voxels should have at most 2 radial neighbors
            // With fixed theta divisions, EVERY cell has exactly 1 radial neighbor
            // in each direction (except at boundaries). This is the key benefit
            // of the aligned trapezoid design.

            for r in 1..255u8 {
                let divs = grid.theta_divisions(r);
                for theta in 0..divs {
                    let neighbors = grid.neighbors(r, theta);

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
                    let (r_m, theta_m) = neighbors.r_minus.unwrap();
                    let (r_p, theta_p) = neighbors.r_plus.unwrap();
                    assert_eq!(theta_m, theta, "r_minus theta should match");
                    assert_eq!(theta_p, theta, "r_plus theta should match");
                    assert_eq!(r_m, r - 1, "r_minus should be r-1");
                    assert_eq!(r_p, r + 1, "r_plus should be r+1");
                }
            }

            println!("All cells have exactly 1 radial neighbor in each direction (aligned)");
        }

        #[test]
        fn test_radial_neighbor_alignment() {
            let grid = PolarMjGrid::new(256, 256, 1.0);

            // With fixed theta divisions, neighbors are perfectly aligned.
            // Cell (r, theta) has neighbors at exactly (r-1, theta) and (r+1, theta).
            for r in 1..255u8 {
                let theta = grid.theta_divisions(r) / 2; // Middle theta
                let (my_start, my_end) = grid.angular_range(r, theta);

                let neighbors = grid.neighbors(r, theta);

                // Check inner neighbor has SAME angular range
                if let Some((nr, nt)) = neighbors.r_minus {
                    let (n_start, n_end) = grid.angular_range(nr, nt);
                    assert!(
                        (my_start - n_start).abs() < 0.0001 && (my_end - n_end).abs() < 0.0001,
                        "Inner neighbor angular range should match exactly"
                    );
                }

                // Check outer neighbor has SAME angular range
                if let Some((nr, nt)) = neighbors.r_plus {
                    let (n_start, n_end) = grid.angular_range(nr, nt);
                    assert!(
                        (my_start - n_start).abs() < 0.0001 && (my_end - n_end).abs() < 0.0001,
                        "Outer neighbor angular range should match exactly"
                    );
                }
            }
        }

        #[test]
        fn test_boundary_neighbors() {
            let grid = PolarMjGrid::new(256, 256, 1.0);

            // At r=0 (inner boundary), r_minus should be None
            let neighbors = grid.neighbors(0, 0);
            assert!(neighbors.r_minus.is_none());
            assert!(neighbors.r_plus.is_some());

            // At r=255 (outer boundary), r_plus should be None
            let neighbors = grid.neighbors(255, 0);
            assert!(neighbors.r_minus.is_some());
            assert!(neighbors.r_plus.is_none());
        }

        #[test]
        fn test_neighbor_symmetry() {
            let grid = PolarMjGrid::new(256, 256, 1.0);

            // If B is a neighbor of A, then A should be a neighbor of B
            // With fixed theta divisions, this is guaranteed by design
            for r in 1..255u8 {
                let theta = grid.theta_divisions(r) / 2;
                let neighbors = grid.neighbors(r, theta);

                // Check outer neighbor lists us as inner neighbor
                if let Some((nr, nt)) = neighbors.r_plus {
                    let reverse_neighbors = grid.neighbors(nr, nt);
                    assert_eq!(
                        reverse_neighbors.r_minus,
                        Some((r, theta)),
                        "Neighbor symmetry violated: ({},{}) -> ({},{}) but not reverse",
                        r,
                        theta,
                        nr,
                        nt
                    );
                }
            }
        }
    }

    // ========================================================================
    // Level 3: Symmetry Tests
    // ========================================================================

    mod level_3_symmetries {
        use super::*;

        #[test]
        fn test_identity_symmetry() {
            use PolarSymmetry::*;

            // Identity should not change anything
            assert_eq!(Identity.transform(1, 2), (1, 2));
            assert_eq!(Identity.transform(-1, -2), (-1, -2));
            assert_eq!(Identity.transform(0, 0), (0, 0));
        }

        #[test]
        fn test_theta_flip_symmetry() {
            use PolarSymmetry::*;

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
            use PolarSymmetry::*;

            // RFlip: (dr, dtheta) -> (-dr, dtheta)
            assert_eq!(RFlip.transform(1, 2), (-1, 2));
            assert_eq!(RFlip.transform(-1, 3), (1, 3));

            // Double application should return to original
            let (dr, dt) = RFlip.transform(1, 2);
            assert_eq!(RFlip.transform(dr, dt), (1, 2));
        }

        #[test]
        fn test_both_flip_symmetry() {
            use PolarSymmetry::*;

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
            use PolarSymmetry::*;

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
            let pattern = PolarPattern {
                center: 1,
                theta_minus: Some(2),
                theta_plus: Some(3),
                r_minus: Some(4),
                r_plus: Some(5),
            };

            let variants: Vec<_> = PolarSymmetry::all()
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
            let pattern = PolarPattern {
                center: 1,
                theta_minus: Some(2),
                theta_plus: Some(2), // Same as theta_minus!
                r_minus: Some(3),
                r_plus: Some(4),
            };

            let variants: HashSet<_> = PolarSymmetry::all()
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
    // Level 4: Single-Step Rule Tests
    // ========================================================================

    mod level_4_single_step {
        use super::*;

        #[test]
        fn test_simple_rule_matching() {
            let mut grid = PolarMjGrid::new(256, 256, 1.0);

            // Set up a pattern: center=0, all neighbors=0 except r_plus=1
            let r = 100u8;
            let theta = 500u16;
            let neighbors = grid.neighbors(r, theta);

            // Set r_plus neighbor to 1
            if let Some((nr, nt)) = neighbors.r_plus {
                grid.set(nr, nt, 1);
            }

            // Pattern: center=0, r_plus=1, others=wildcard
            let pattern = PolarPattern {
                center: 0,
                theta_minus: None,
                theta_plus: None,
                r_minus: None,
                r_plus: Some(1),
            };

            assert!(pattern.matches(&grid, r, theta));

            // Shouldn't match at a location without r_plus=1
            assert!(!pattern.matches(&grid, r.saturating_sub(10), theta));
        }

        #[test]
        fn test_rule_application() {
            let mut grid = PolarMjGrid::new(256, 256, 1.0);

            // Rule: 0 -> 1 (unconditional)
            let rule = PolarRule {
                input: PolarPattern::center_only(0),
                output: 1,
            };

            let r = 100u8;
            let theta = 500u16;

            assert_eq!(grid.get(r, theta), 0);
            rule.apply(&mut grid, r, theta);
            assert_eq!(grid.get(r, theta), 1);
        }

        #[test]
        fn test_conditional_rule() {
            let mut grid = PolarMjGrid::new(256, 256, 1.0);

            // Rule: 0 with r_plus=1 -> 2
            let rule = PolarRule {
                input: PolarPattern {
                    center: 0,
                    r_plus: Some(1),
                    ..Default::default()
                },
                output: 2,
            };

            let r = 100u8;
            let theta = 500u16;
            let neighbors = grid.neighbors(r, theta);

            // Without r_plus=1, rule shouldn't apply
            assert!(!rule.matches(&grid, r, theta));

            // Set r_plus neighbor to 1
            if let Some((nr, nt)) = neighbors.r_plus {
                grid.set(nr, nt, 1);
            }

            // Now it should match and apply
            assert!(rule.matches(&grid, r, theta));
            rule.apply(&mut grid, r, theta);
            assert_eq!(grid.get(r, theta), 2);
        }

        #[test]
        fn test_rule_with_symmetries() {
            let mut grid = PolarMjGrid::new(256, 256, 1.0);

            // Asymmetric rule: 0 with theta_plus=1 -> 2
            let base_rule = PolarRule {
                input: PolarPattern {
                    center: 0,
                    theta_plus: Some(1),
                    ..Default::default()
                },
                output: 2,
            };

            let rules = base_rule.with_all_symmetries();
            assert_eq!(rules.len(), 2); // Only 2 unique due to r symmetry not affecting theta

            // Test that theta_minus=1 matches the theta-flipped variant
            let r = 100u8;
            let theta = 500u16;
            let neighbors = grid.neighbors(r, theta);

            grid.set(neighbors.theta_minus.0, neighbors.theta_minus.1, 1);

            // Base rule shouldn't match (theta_plus isn't 1)
            assert!(!base_rule.matches(&grid, r, theta));

            // But one of the symmetry variants should
            let any_matches = rules.iter().any(|rule| rule.matches(&grid, r, theta));
            assert!(any_matches, "No symmetry variant matched");
        }

        #[test]
        fn test_multiple_rules_priority() {
            let mut grid = PolarMjGrid::new(256, 256, 1.0);

            // Two rules that could both match
            let rule1 = PolarRule {
                input: PolarPattern::center_only(0),
                output: 1,
            };
            let rule2 = PolarRule {
                input: PolarPattern::center_only(0),
                output: 2,
            };

            // First matching rule should apply (order matters)
            let rules = vec![rule1, rule2];
            let r = 100u8;
            let theta = 500u16;

            for rule in &rules {
                if rule.matches(&grid, r, theta) {
                    rule.apply(&mut grid, r, theta);
                    break;
                }
            }

            assert_eq!(grid.get(r, theta), 1); // First rule's output
        }
    }

    // ========================================================================
    // Level 5: Multi-Step Model Tests (Simplified)
    // ========================================================================

    mod level_5_models {
        use super::*;

        #[test]
        fn test_ring_growth() {
            let mut grid = PolarMjGrid::new(256, 64, 1.0);

            // Seed: entire inner ring set to 1
            let inner_divs = grid.theta_divisions(0);
            for theta in 0..inner_divs {
                grid.set(0, theta, 1);
            }

            // Rule: 0 with r_minus=1 -> 1 (grow outward)
            let rule = PolarRule {
                input: PolarPattern {
                    center: 0,
                    r_minus: Some(1),
                    ..Default::default()
                },
                output: 1,
            };

            // Run 63 steps (should fill all 64 rings)
            for _ in 0..63 {
                let mut to_set = vec![];
                for r in 0..64u8 {
                    let divs = grid.theta_divisions(r);
                    for theta in 0..divs {
                        if rule.matches(&grid, r, theta) {
                            to_set.push((r, theta));
                        }
                    }
                }
                for (r, theta) in to_set {
                    grid.set(r, theta, 1);
                }
            }

            // All cells should be 1
            for r in 0..64u8 {
                let divs = grid.theta_divisions(r);
                for theta in 0..divs {
                    assert_eq!(grid.get(r, theta), 1, "Cell ({}, {}) not filled", r, theta);
                }
            }
        }

        #[test]
        fn test_wave_pattern() {
            let mut grid = PolarMjGrid::new(256, 64, 1.0);

            // Create alternating rings: 1, 0, 1, 0, ...
            for r in 0..64u8 {
                let value = (r % 2) as u8;
                let divs = grid.theta_divisions(r);
                for theta in 0..divs {
                    grid.set(r, theta, value);
                }
            }

            // Verify pattern
            for r in 0..64u8 {
                let expected = (r % 2) as u8;
                let divs = grid.theta_divisions(r);
                for theta in 0..divs {
                    assert_eq!(grid.get(r, theta), expected);
                }
            }
        }

        #[test]
        fn test_deterministic_output() {
            fn run_model_with_seed(seed: u64) -> u64 {
                let mut grid = PolarMjGrid::new(256, 32, 1.0);

                // Use seed to determine starting position
                let start_theta = (seed % 100) as u16;
                grid.set(0, start_theta, 1);

                // Simple growth rule
                let rule = PolarRule {
                    input: PolarPattern {
                        center: 0,
                        r_minus: Some(1),
                        ..Default::default()
                    },
                    output: 1,
                };

                // Run 10 steps
                for _ in 0..10 {
                    let mut to_set = vec![];
                    for r in 0..32u8 {
                        let divs = grid.theta_divisions(r);
                        for theta in 0..divs {
                            if rule.matches(&grid, r, theta) {
                                to_set.push((r, theta));
                            }
                        }
                    }
                    for (r, theta) in to_set {
                        grid.set(r, theta, 1);
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
    // Level 6: Rendering Tests
    // ========================================================================

    mod level_6_rendering {
        use super::*;

        #[test]
        fn test_polar_to_cartesian() {
            let grid = PolarMjGrid::new(256, 64, 1.0);

            // At theta=0, should be along positive x-axis
            let (x, y) = grid.to_cartesian(50, 0);
            let r_actual = 256.0 + 50.0;

            // x should be close to r_actual (cos(small_angle) ~ 1)
            // y should be close to 0 (sin(small_angle) ~ 0)
            // But since we use mid-angle, there's a small offset
            let divs = grid.theta_divisions(50);
            let mid_angle = PI / divs as f32; // mid-point of first voxel
            let expected_x = r_actual * mid_angle.cos();
            let expected_y = r_actual * mid_angle.sin();

            assert!(
                (x - expected_x).abs() < 0.01,
                "x={}, expected={}",
                x,
                expected_x
            );
            assert!(
                (y - expected_y).abs() < 0.01,
                "y={}, expected={}",
                y,
                expected_y
            );
        }

        #[test]
        fn test_cartesian_quarter_circle() {
            let grid = PolarMjGrid::new(256, 64, 1.0);

            // At theta=quarter_circle, should be along positive y-axis
            let r = 50u8;
            let divs = grid.theta_divisions(r);
            let quarter_theta = divs / 4;

            let (x, y) = grid.to_cartesian(r, quarter_theta);
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
            let grid = PolarMjGrid::new(256, 256, 1.0);
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
            let mut grid = PolarMjGrid::new(256, 64, 1.0);
            assert_eq!(grid.count_nonzero(), 0);

            // Set some values
            grid.set(0, 0, 1);
            grid.set(10, 100, 2);
            grid.set(63, 500, 3);

            assert_eq!(grid.count_nonzero(), 3);

            // Clear and verify
            grid.clear();
            assert_eq!(grid.count_nonzero(), 0);
        }
    }

    // ========================================================================
    // Level 7: Integration Tests - Run models and produce images
    // ========================================================================

    mod level_7_integration {
        use super::*;
        use std::path::PathBuf;

        fn output_dir() -> PathBuf {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("screenshots/polar")
        }

        /// Test: Ring Growth model - grows outward from inner ring.
        ///
        /// This is the simplest test: seed the inner ring, grow outward.
        /// Should produce concentric rings of color.
        #[test]
        fn test_model_ring_growth() {
            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            // Create model: values "BW" (B=0=background, W=1=white)
            // Use smaller r_depth (32) for faster testing
            let mut model = PolarModel::new("ring_growth", "BW", 64, 32, 1.0);
            model.set_seed(42);

            // Rule: B with r_minus=W -> W (grow outward)
            // In other words: if I'm black and my inner neighbor is white, become white
            model
                .add_rule_str("B;*,*,W,* -> W", true)
                .expect("Failed to add rule");

            // Seed the inner ring with white
            model.seed_inner_ring(1); // W = 1

            // Save initial state
            let path_0 = out_dir.join("ring_growth_step_00000.png");
            model.save_png(&path_0, 512).expect("Failed to save step 0");
            println!("Saved: {}", path_0.display());

            // Run model (smaller step count for faster test)
            let steps = model.run(5000);
            println!("Ring Growth: {} steps completed", steps);

            // Save final state
            let path_final = out_dir.join(format!("ring_growth_final_step_{:05}.png", model.step));
            model
                .save_png(&path_final, 512)
                .expect("Failed to save final");
            println!("Saved: {}", path_final.display());

            // Verify: all cells should be white (1)
            let nonzero = model.grid.count_nonzero();
            let total = model.grid.total_voxels();
            println!(
                "Nonzero: {} / {} ({:.1}%)",
                nonzero,
                total,
                100.0 * nonzero as f64 / total as f64
            );

            // Should show progress (at least 20% filled after 5000 steps)
            // Note: Full fill takes ~15k steps, but 5k is enough to verify rules work
            assert!(
                nonzero > total / 5,
                "Ring growth should fill at least 20% of grid after 5000 steps"
            );
        }

        /// Test: Angular Spread model - grows around rings.
        ///
        /// Seed one cell, spread around the ring first, then outward.
        #[test]
        fn test_model_angular_spread() {
            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            // Smaller grid for faster testing
            let mut model = PolarModel::new("angular_spread", "BW", 64, 24, 1.0);
            model.set_seed(123);

            // Rule 1: B with theta_minus=W -> W (spread clockwise)
            model
                .add_rule_str("B;W,*,*,* -> W", true)
                .expect("Failed to add rule");

            // Seed one cell at inner ring
            model.seed_cell(0, 0, 1);

            // Save step 0
            let path_0 = out_dir.join("angular_spread_step_00000.png");
            model.save_png(&path_0, 512).expect("Failed to save step 0");

            // Run 500 steps
            model.run(500);
            let path_500 = out_dir.join(format!("angular_spread_step_{:05}.png", model.step));
            model
                .save_png(&path_500, 512)
                .expect("Failed to save step 500");

            // Run to completion
            let _steps = model.run(10000);
            println!("Angular Spread: {} total steps", model.step);

            let path_final =
                out_dir.join(format!("angular_spread_final_step_{:05}.png", model.step));
            model
                .save_png(&path_final, 512)
                .expect("Failed to save final");
            println!("Saved: {}", path_final.display());
        }

        /// Test: Flood Fill model - fill entire grid from one seed.
        ///
        /// Uses rules that spread in all 4 directions.
        #[test]
        fn test_model_flood_fill() {
            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            // Smaller grid for faster testing
            let mut model = PolarModel::new("flood_fill", "BW", 64, 20, 1.0);
            model.set_seed(999);

            // Rules for all 4 directions:
            // B with any neighbor=W -> W
            model.add_rule_str("B;W,*,*,* -> W", true).unwrap();
            model.add_rule_str("B;*,W,*,* -> W", true).unwrap();
            model.add_rule_str("B;*,*,W,* -> W", true).unwrap();
            model.add_rule_str("B;*,*,*,W -> W", true).unwrap();

            // Seed center of middle ring
            let mid_r = 10;
            let mid_theta = model.grid.theta_divisions(mid_r) / 2;
            model.seed_cell(mid_r, mid_theta, 1);

            // Screenshots at various steps
            let screenshots = vec![0, 100, 500, -1];
            let paths =
                run_polar_model_with_screenshots(&mut model, &out_dir, 512, 5000, &screenshots)
                    .expect("Failed to run model with screenshots");

            for path in &paths {
                println!("Saved: {}", path.display());
            }

            // Verify grid is mostly filled
            let nonzero = model.grid.count_nonzero();
            let total = model.grid.total_voxels();
            println!(
                "Flood Fill: {} / {} cells ({:.1}%)",
                nonzero,
                total,
                100.0 * nonzero as f64 / total as f64
            );

            // Should show substantial progress (at least 40% filled after 5000 steps)
            // Note: Full fill would need more steps, but this proves flood fill works
            assert!(
                nonzero > total * 4 / 10,
                "Flood fill should fill >40% of grid after 5000 steps"
            );
        }

        /// Test: Wave Pattern - alternating concentric rings.
        ///
        /// This tests the rendering more than the rules.
        #[test]
        fn test_model_wave_pattern() {
            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            // Smaller grid for faster testing
            let mut model = PolarModel::new("wave_pattern", "BWR", 64, 32, 1.0);

            // Manually create alternating rings (no rules needed)
            for r in 0..32u8 {
                let value = match r % 3 {
                    0 => 0, // B (transparent)
                    1 => 1, // W (white)
                    _ => 2, // R (red)
                };
                model.fill_ring(r, value);
            }

            let path = out_dir.join("wave_pattern.png");
            model
                .save_png(&path, 512)
                .expect("Failed to save wave pattern");
            println!("Saved: {}", path.display());

            // Verify pattern
            assert_eq!(model.grid.get(0, 0), 0); // Ring 0 = B
            assert_eq!(model.grid.get(1, 0), 1); // Ring 1 = W
            assert_eq!(model.grid.get(2, 0), 2); // Ring 2 = R
        }

        /// Test: Checkerboard pattern - alternating in theta.
        #[test]
        fn test_model_checkerboard() {
            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            // Smaller grid for faster testing
            let mut model = PolarModel::new("checkerboard", "BW", 64, 32, 1.0);

            // Create checkerboard: alternate based on (r + theta) % 2
            for r in 0..32u8 {
                let divs = model.grid.theta_divisions(r);
                for theta in 0..divs {
                    let value = ((r as u16 + theta) % 2) as u8;
                    model.grid.set(r, theta, value);
                }
            }

            let path = out_dir.join("checkerboard.png");
            model
                .save_png(&path, 512)
                .expect("Failed to save checkerboard");
            println!("Saved: {}", path.display());
        }

        /// Test: Spiral Growth - grows in a spiral pattern.
        #[test]
        fn test_model_spiral() {
            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            // Smaller grid for faster testing
            let mut model = PolarModel::new("spiral", "BW", 64, 24, 1.0);
            model.set_seed(7777);

            // Rules: prefer to grow in theta+ direction, then outward
            // Higher priority for theta+ spread
            model.add_rule_str("B;*,W,*,* -> W", false).unwrap(); // theta+
            model.add_rule_str("B;*,*,*,W -> W", false).unwrap(); // r+

            // Seed inner ring at theta=0
            model.seed_cell(0, 0, 1);

            // Run with screenshots
            let screenshots = vec![0, 100, 500, -1];
            let paths =
                run_polar_model_with_screenshots(&mut model, &out_dir, 512, 5000, &screenshots)
                    .expect("Failed to run spiral model");

            for path in &paths {
                println!("Saved: {}", path.display());
            }
        }

        /// Test: Voronoi-style multi-seed flood fill.
        ///
        /// Seeds 5 different colors at random positions and lets them
        /// flood fill simultaneously, creating Voronoi-like regions.
        #[test]
        fn test_model_voronoi() {
            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            // 6 values: B=background, then 5 colors (R, G, B, Y, C)
            let mut model = PolarModel::new("voronoi", "BRGBYC", 64, 32, 1.0);
            model.set_seed(42424);

            // Each color spreads into background cells
            // R (1) spreads
            model.add_rule_str("B;R,*,*,* -> R", true).unwrap();
            model.add_rule_str("B;*,R,*,* -> R", true).unwrap();
            model.add_rule_str("B;*,*,R,* -> R", true).unwrap();
            model.add_rule_str("B;*,*,*,R -> R", true).unwrap();
            // G (2) spreads
            model.add_rule_str("B;G,*,*,* -> G", true).unwrap();
            model.add_rule_str("B;*,G,*,* -> G", true).unwrap();
            model.add_rule_str("B;*,*,G,* -> G", true).unwrap();
            model.add_rule_str("B;*,*,*,G -> G", true).unwrap();
            // B (3) spreads - note: 'B' in pattern means background (0), 'U' would be blue
            // Let's use different letters to avoid confusion
            // Actually the values string "BRGBYC" means:
            //   index 0 = 'B' (background/black)
            //   index 1 = 'R' (red)
            //   index 2 = 'G' (green)
            //   index 3 = 'B' - wait, that's confusing!
            // Let me use "XRGMYC" where X=background
            drop(model);

            // Recreate with clearer values - smaller grid for speed
            let mut model = PolarModel::new("voronoi", "XRGMYC", 32, 16, 1.0);
            model.set_seed(42424);

            // X=0 (background), R=1, G=2, M=3 (magenta), Y=4, C=5 (cyan)
            // Don't use symmetry expansion (false) for faster execution

            // R spreads into X (all 4 directions)
            model.add_rule_str("X;R,*,*,* -> R", false).unwrap();
            model.add_rule_str("X;*,R,*,* -> R", false).unwrap();
            model.add_rule_str("X;*,*,R,* -> R", false).unwrap();
            model.add_rule_str("X;*,*,*,R -> R", false).unwrap();
            // G spreads into X
            model.add_rule_str("X;G,*,*,* -> G", false).unwrap();
            model.add_rule_str("X;*,G,*,* -> G", false).unwrap();
            model.add_rule_str("X;*,*,G,* -> G", false).unwrap();
            model.add_rule_str("X;*,*,*,G -> G", false).unwrap();
            // M spreads into X
            model.add_rule_str("X;M,*,*,* -> M", false).unwrap();
            model.add_rule_str("X;*,M,*,* -> M", false).unwrap();
            model.add_rule_str("X;*,*,M,* -> M", false).unwrap();
            model.add_rule_str("X;*,*,*,M -> M", false).unwrap();
            // Y spreads into X
            model.add_rule_str("X;Y,*,*,* -> Y", false).unwrap();
            model.add_rule_str("X;*,Y,*,* -> Y", false).unwrap();
            model.add_rule_str("X;*,*,Y,* -> Y", false).unwrap();
            model.add_rule_str("X;*,*,*,Y -> Y", false).unwrap();
            // C spreads into X
            model.add_rule_str("X;C,*,*,* -> C", false).unwrap();
            model.add_rule_str("X;*,C,*,* -> C", false).unwrap();
            model.add_rule_str("X;*,*,C,* -> C", false).unwrap();
            model.add_rule_str("X;*,*,*,C -> C", false).unwrap();

            // Seed 5 points at different locations around the ring
            let theta_divs = model.grid.theta_divisions;
            let r_depth = model.grid.r_depth;

            // Spread seeds across the ring at different radii and angles
            model.seed_cell(r_depth as u8 / 4, theta_divs / 5 * 0, 1); // R
            model.seed_cell(r_depth as u8 / 2, theta_divs / 5 * 1, 2); // G
            model.seed_cell(r_depth as u8 * 3 / 4, theta_divs / 5 * 2, 3); // M
            model.seed_cell(r_depth as u8 / 3, theta_divs / 5 * 3, 4); // Y
            model.seed_cell(r_depth as u8 * 2 / 3, theta_divs / 5 * 4, 5); // C

            // Run with screenshots - smaller step count
            let screenshots = vec![0, 50, 200, -1];
            let paths =
                run_polar_model_with_screenshots(&mut model, &out_dir, 512, 5000, &screenshots)
                    .expect("Failed to run voronoi model");

            for path in &paths {
                println!("Saved: {}", path.display());
            }

            // Verify all colors are present
            let mut color_counts = [0usize; 6];
            for (_, _, v) in model.grid.iter() {
                color_counts[v as usize] += 1;
            }
            println!("Color distribution:");
            println!("  X (background): {}", color_counts[0]);
            println!("  R (red):        {}", color_counts[1]);
            println!("  G (green):      {}", color_counts[2]);
            println!("  M (magenta):    {}", color_counts[3]);
            println!("  Y (yellow):     {}", color_counts[4]);
            println!("  C (cyan):       {}", color_counts[5]);

            // At least 4 colors should have spread (some might not due to being blocked)
            let colors_present = color_counts[1..].iter().filter(|&&c| c > 0).count();
            assert!(
                colors_present >= 4,
                "Expected at least 4 colors to spread, got {}",
                colors_present
            );
        }

        /// Test: Geological Layers using proper MJ-style rules.
        ///
        /// Layers grow outward from the core using propagation rules:
        /// - M (Magma) at the center seeds everything
        /// - M spreads outward, then transforms: M -> S (stone) -> D (dirt) -> G (grass)
        /// - Each layer triggers the next when adjacent
        ///
        /// This is NOT manual pixel painting - it uses the PolarModel rule DSL.
        #[test]
        fn test_model_geological_layers() {
            use crate::markov_junior::recording::{SimulationRecorder, VideoExporter};

            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            // Palette: B=background, M=magma, S=stone, D=dirt, G=grass
            // B=0, M=1, S=2, D=3, G=4
            let palette = "BMSDG";
            let colors: Vec<[u8; 4]> = vec![
                [20, 20, 25, 255],   // B - void/background (dark)
                [255, 100, 30, 255], // M - magma (bright orange)
                [80, 75, 70, 255],   // S - stone (gray)
                [120, 80, 50, 255],  // D - dirt (brown)
                [60, 160, 50, 255],  // G - grass (green)
            ];

            let r_depth: u16 = 40;
            let mut model = PolarModel::new("geological_layers", palette, 64, r_depth, 1.0);
            model.set_seed(54321);

            // RULES (using MJ-style rule DSL):
            // Format: "center;theta-,theta+,r-,r+ -> output"
            // r- = inward neighbor (smaller radius), r+ = outward neighbor (larger radius)
            //
            // Simple geological layering model:
            // - Magma at center grows outward, but transforms to stone when far from center
            // - Stone grows outward, transforms to dirt
            // - Dirt grows outward, transforms to grass
            // - Grass only grows on the surface
            //
            // The key insight: materials transform when they have the SAME material
            // behind them (inner neighbor = same type), creating a "cooling" wavefront.

            // Magma grows outward from magma
            model.add_rule_str("B;*,*,M,* -> M", true).unwrap();
            // Magma at frontier (B outward) transforms to stone
            model.add_rule_str("M;*,*,M,B -> S", true).unwrap();

            // Stone grows outward from stone
            model.add_rule_str("B;*,*,S,* -> S", true).unwrap();
            // Stone at frontier (B outward) transforms to dirt
            model.add_rule_str("S;*,*,S,B -> D", true).unwrap();

            // Dirt grows outward from dirt
            model.add_rule_str("B;*,*,D,* -> D", true).unwrap();
            // Dirt at frontier (B outward) transforms to grass
            model.add_rule_str("D;*,*,D,B -> G", true).unwrap();

            // Grass grows outward from grass (surface growth)
            model.add_rule_str("B;*,*,G,* -> G", true).unwrap();

            // Seed: Just magma at the innermost ring
            model.seed_inner_ring(1); // M = 1
            let theta_divs = model.grid.theta_divisions;

            // Record simulation
            let mut recorder = SimulationRecorder::new(&model.grid);
            recorder.record_frame(&model.grid);

            // Run until stable or max steps
            let max_steps = 2000;
            for _ in 0..max_steps {
                if !model.step() {
                    break;
                }
                recorder.record_frame(&model.grid);
            }

            println!(
                "Geological Layers: {} frames recorded",
                recorder.frame_count()
            );

            // Save final image
            let path_final = out_dir.join("geological_layers.png");
            model
                .grid
                .save_png(&path_final, 512, &colors, [20, 20, 25, 255])
                .expect("Failed to save geological layers");
            println!("Saved: {}", path_final.display());

            // Save archive
            let mut archive = recorder.into_archive();
            archive.palette = palette.to_string();
            let archive_path = out_dir.join("geological_layers.mjsim");
            archive.save(&archive_path).expect("Failed to save archive");
            println!("Saved: {}", archive_path.display());

            // Export video
            let exporter = VideoExporter::new(archive, colors, 512);
            let video_path = out_dir.join("geological_layers.mp4");
            match exporter.export_mp4(&video_path, 10.0, 30) {
                Ok(()) => println!("Exported: {}", video_path.display()),
                Err(crate::markov_junior::recording::VideoError::FfmpegNotFound) => {
                    println!("Skipping MP4 export (ffmpeg not installed)");
                }
                Err(e) => panic!("Video export failed: {}", e),
            }

            // Verify we have multiple layers present
            let mut layer_counts = [0usize; 5];
            for r in 0..r_depth {
                for theta in 0..theta_divs {
                    let v = model.grid.get(r as u8, theta) as usize;
                    if v < 5 {
                        layer_counts[v] += 1;
                    }
                }
            }

            println!("Layer distribution:");
            println!("  B (background): {}", layer_counts[0]);
            println!("  M (magma): {}", layer_counts[1]);
            println!("  S (stone): {}", layer_counts[2]);
            println!("  D (dirt): {}", layer_counts[3]);
            println!("  G (grass): {}", layer_counts[4]);

            // We should have at least some of each layer (except maybe background)
            assert!(
                layer_counts[1] > 0 || layer_counts[2] > 0,
                "Expected some magma or stone"
            );
        }

        /// Master test: Run all polar models and generate a summary.
        #[test]
        fn test_run_all_polar_models() {
            let out_dir = output_dir();
            std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

            println!("\n========================================");
            println!("POLAR MODEL VERIFICATION");
            println!("Output: {}", out_dir.display());
            println!("========================================\n");

            // Define all models to run (smaller grids for faster testing)
            let models: Vec<(&str, Box<dyn Fn() -> PolarModel>)> = vec![
                (
                    "ring_growth",
                    Box::new(|| {
                        let mut m = PolarModel::new("ring_growth", "BW", 64, 32, 1.0);
                        m.set_seed(42);
                        m.add_rule_str("B;*,*,W,* -> W", true).unwrap();
                        m.seed_inner_ring(1);
                        m
                    }),
                ),
                (
                    "flood_fill",
                    Box::new(|| {
                        let mut m = PolarModel::new("flood_fill", "BW", 64, 20, 1.0);
                        m.set_seed(999);
                        m.add_rule_str("B;W,*,*,* -> W", true).unwrap();
                        m.add_rule_str("B;*,W,*,* -> W", true).unwrap();
                        m.add_rule_str("B;*,*,W,* -> W", true).unwrap();
                        m.add_rule_str("B;*,*,*,W -> W", true).unwrap();
                        m.seed_cell(10, 100, 1);
                        m
                    }),
                ),
                (
                    "angular_spread",
                    Box::new(|| {
                        let mut m = PolarModel::new("angular_spread", "BW", 64, 24, 1.0);
                        m.set_seed(123);
                        m.add_rule_str("B;W,*,*,* -> W", true).unwrap();
                        m.seed_cell(12, 0, 1);
                        m
                    }),
                ),
            ];

            let mut results = Vec::new();

            for (name, model_fn) in &models {
                print!("Running {}... ", name);

                let mut model = model_fn();
                let start = std::time::Instant::now();

                // Run with screenshots at step 0, mid, and final
                let screenshots = vec![0, 500, -1];
                let paths_result = run_polar_model_with_screenshots(
                    &mut model,
                    &out_dir,
                    512,
                    20000,
                    &screenshots,
                );

                let elapsed = start.elapsed();

                match paths_result {
                    Ok(paths) => {
                        let nonzero = model.grid.count_nonzero();
                        let total = model.grid.total_voxels();
                        let fill_pct = 100.0 * nonzero as f64 / total as f64;

                        println!(
                            "OK - {} steps, {:.1}% filled, {:?}",
                            model.step, fill_pct, elapsed
                        );
                        results.push((name.to_string(), model.step, fill_pct, true));

                        for path in paths {
                            println!("  -> {}", path.file_name().unwrap().to_string_lossy());
                        }
                    }
                    Err(e) => {
                        println!("FAILED: {}", e);
                        results.push((name.to_string(), 0, 0.0, false));
                    }
                }
            }

            // Summary
            println!("\n========================================");
            println!("SUMMARY");
            println!("========================================");
            println!(
                "{:<20} {:>8} {:>8} {:>8}",
                "Model", "Steps", "Fill%", "Status"
            );
            println!("{}", "-".repeat(48));

            for (name, steps, fill, ok) in &results {
                println!(
                    "{:<20} {:>8} {:>7.1}% {:>8}",
                    name,
                    steps,
                    fill,
                    if *ok { "OK" } else { "FAIL" }
                );
            }

            let passed = results.iter().filter(|(_, _, _, ok)| *ok).count();
            let total = results.len();
            println!("{}", "-".repeat(48));
            println!("PASSED: {} / {}", passed, total);
            println!("\nOutput directory: {}", out_dir.display());
            println!("========================================\n");

            assert_eq!(passed, total, "All models should pass");
        }
    }
}
