//! Overlap model WFC node.
//!
//! OverlapNode extracts NxN patterns from a sample image and generates
//! output that locally resembles the input sample.
//!
//! C# Reference: OverlapModel.cs (lines 1-181)

use super::wfc_node::{WfcNode, WfcState, DX, DY};
use crate::markov_junior::helper::{load_bitmap, ords};
use crate::markov_junior::node::{ExecutionContext, Node};
use crate::markov_junior::MjGrid;
use rand::prelude::*;
use std::collections::HashMap;
use std::path::Path;

// Re-export for use in tests
pub use super::wfc_node::WfcState as OverlapWfcState;

/// Overlap model WFC node.
///
/// Extracts NxN patterns from a sample image and uses them to generate
/// output that locally resembles the sample.
pub struct OverlapNode {
    /// Base WFC node with shared algorithms
    pub wfc: WfcNode,

    /// Extracted patterns: patterns[pattern_index] = NxN array of color indices
    pub patterns: Vec<Vec<u8>>,
}

impl OverlapNode {
    /// Create an OverlapNode from a sample image.
    ///
    /// # Arguments
    /// * `sample_path` - Path to the sample PNG image
    /// * `n` - Pattern size (typically 3)
    /// * `periodic_input` - Whether to treat sample as wrapping
    /// * `periodic` - Whether output wraps
    /// * `shannon` - Whether to use Shannon entropy
    /// * `tries` - Number of seed attempts
    /// * `symmetry` - Symmetry bitmask for pattern generation
    /// * `newgrid` - Output grid
    /// * `input_grid` - Input grid for initial constraints
    /// * `rules` - Map from input values to allowed output colors
    ///
    /// C# Reference: OverlapNode.Load() (lines 12-133)
    #[allow(clippy::too_many_arguments)]
    pub fn from_sample(
        sample_path: &Path,
        n: usize,
        periodic_input: bool,
        periodic: bool,
        shannon: bool,
        tries: usize,
        symmetry: &[bool],
        newgrid: MjGrid,
        input_grid: &MjGrid,
        rules: &[(u8, Vec<u8>)], // (input_value, allowed_output_values)
    ) -> Result<Self, String> {
        // Load sample image
        let (bitmap, smx, smy, _) =
            load_bitmap(sample_path).map_err(|e| format!("Failed to load sample image: {}", e))?;

        // Convert to color indices
        let (sample, c) = ords(&bitmap);

        if c > newgrid.c as usize {
            return Err(format!(
                "Sample has {} colors but grid only allows {}",
                c, newgrid.c
            ));
        }

        // Calculate number of possible patterns
        let w = power(c, n * n);

        // Extract patterns and count weights
        let mut weights_map: HashMap<i64, usize> = HashMap::new();
        let mut ordering: Vec<i64> = Vec::new();

        let ymax = if periodic_input {
            smy
        } else {
            smy.saturating_sub(n - 1)
        };
        let xmax = if periodic_input {
            smx
        } else {
            smx.saturating_sub(n - 1)
        };

        for y in 0..ymax {
            for x in 0..xmax {
                // Extract pattern at (x, y)
                let pattern = extract_pattern(&sample, smx, smy, x, y, n);

                // Generate symmetry variants
                let variants = pattern_symmetries(&pattern, n, symmetry);

                for p in variants {
                    let idx = pattern_index(&p, c);
                    if let Some(weight) = weights_map.get_mut(&idx) {
                        *weight += 1;
                    } else {
                        weights_map.insert(idx, 1);
                        ordering.push(idx);
                    }
                }
            }
        }

        let num_patterns = weights_map.len();
        if num_patterns == 0 {
            return Err("No patterns extracted from sample".to_string());
        }

        // Convert to pattern arrays and weights
        let mut patterns = Vec::with_capacity(num_patterns);
        let mut weights = Vec::with_capacity(num_patterns);

        for &idx in &ordering {
            patterns.push(pattern_from_index(idx, n, c));
            weights.push(weights_map[&idx] as f64);
        }

        // Build propagator: which patterns can be adjacent in each direction
        let propagator = build_overlap_propagator(&patterns, n);

        // Build map from input grid values to allowed patterns
        let map = build_pattern_map(input_grid, &newgrid, &patterns, rules);

        let mx = input_grid.mx;
        let my = input_grid.my;
        let mz = input_grid.mz;
        let wave_length = mx * my * mz;
        let num_directions = 4; // 2D only for overlap

        let wfc = WfcNode::new(
            wave_length,
            num_patterns,
            num_directions,
            propagator,
            weights,
            newgrid,
            map,
            n,
            periodic,
            shannon,
            tries,
            mx,
            my,
            mz,
        );

        Ok(Self { wfc, patterns })
    }

    /// Update the output grid state from the wave.
    ///
    /// Uses voting to determine the final color at each cell when
    /// multiple patterns could contribute.
    ///
    /// C# Reference: OverlapNode.UpdateState() (lines 136-178)
    pub fn update_state(&self, grid: &mut MjGrid) {
        let mx = grid.mx;
        let my = grid.my;
        let n = self.wfc.n;
        let num_colors = grid.c as usize;

        // Vote counting: votes[cell][color] = count
        let mut votes: Vec<Vec<i32>> = vec![vec![0; num_colors]; grid.state.len()];

        // Count votes from each cell's possible patterns
        for i in 0..self.wfc.wave.length {
            let x = i % mx;
            let y = i / mx;

            for p in 0..self.wfc.wave.p {
                if self.wfc.wave.get_data(i, p) {
                    let pattern = &self.patterns[p];

                    for dy in 0..n {
                        let mut ydy = y + dy;
                        if ydy >= my {
                            ydy -= my;
                        }

                        for dx in 0..n {
                            let mut xdx = x + dx;
                            if xdx >= mx {
                                xdx -= mx;
                            }

                            let value = pattern[dx + dy * n];
                            let cell = xdx + ydy * mx;
                            votes[cell][value as usize] += 1;
                        }
                    }
                }
            }
        }

        // Assign most-voted color to each cell (with random tie-breaking)
        let mut rng = rand::thread_rng();
        for (i, v) in votes.iter().enumerate() {
            let mut max_vote = -1.0;
            let mut argmax: u8 = 0xff;

            for (c, &vote) in v.iter().enumerate() {
                let value = vote as f64 + 0.1 * rng.gen::<f64>();
                if value > max_vote {
                    argmax = c as u8;
                    max_vote = value;
                }
            }

            grid.state[i] = argmax;
        }
    }
}

impl Node for OverlapNode {
    fn reset(&mut self) {
        self.wfc.reset();
    }

    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // Handle child node execution (Branch behavior)
        if self.wfc.child_index >= 0 {
            self.wfc.reset();
            return false;
        }

        if self.wfc.first_go {
            // First call - initialize
            if !self.wfc.initialize(ctx.grid, ctx.random) {
                return false;
            }
            // Swap grids
            std::mem::swap(&mut self.wfc.newgrid, ctx.grid);
            return true;
        }

        // Continue stepping
        if self.wfc.step() {
            // Still running - optionally update state for animation
            if ctx.gif {
                self.update_state(ctx.grid);
            }
            true
        } else {
            // Completed or failed
            // ctx.grid is already the newgrid (swapped on first_go)
            // Don't swap back - let parent sequence continue with newgrid
            if self.wfc.state == WfcState::Completed {
                self.update_state(ctx.grid);
            }
            false
        }
    }
}

// ============================================================================
// Helper functions for pattern extraction and manipulation
// ============================================================================

/// Calculate c^n (integer power).
fn power(c: usize, n: usize) -> i64 {
    let mut result: i64 = 1;
    for _ in 0..n {
        result *= c as i64;
    }
    result
}

/// Extract an NxN pattern from a sample at position (x, y).
fn extract_pattern(sample: &[u8], smx: usize, smy: usize, x: usize, y: usize, n: usize) -> Vec<u8> {
    let mut result = vec![0u8; n * n];
    for dy in 0..n {
        for dx in 0..n {
            let sx = (x + dx) % smx;
            let sy = (y + dy) % smy;
            result[dx + dy * n] = sample[sx + sy * smx];
        }
    }
    result
}

/// Convert a pattern to an index (base-C number).
///
/// C# Reference: Helper.Index(byte[], int) (lines 47-52)
fn pattern_index(p: &[u8], c: usize) -> i64 {
    let mut result: i64 = 0;
    let mut power: i64 = 1;
    for i in (0..p.len()).rev() {
        result += p[i] as i64 * power;
        power *= c as i64;
    }
    result
}

/// Convert an index back to a pattern.
///
/// C# Reference: OverlapNode.patternFromIndex (lines 50-66)
fn pattern_from_index(idx: i64, n: usize, c: usize) -> Vec<u8> {
    let mut residue = idx;
    let w = power(c, n * n);
    let mut p = w;

    let mut result = vec![0u8; n * n];
    for item in result.iter_mut() {
        p /= c as i64;
        let mut count = 0;
        while residue >= p {
            residue -= p;
            count += 1;
        }
        *item = count;
    }
    result
}

/// Generate symmetry variants of a pattern.
///
/// C# Reference: SymmetryHelper.SquareSymmetries with pattern-specific functions
fn pattern_symmetries(pattern: &[u8], n: usize, symmetry: &[bool]) -> Vec<Vec<u8>> {
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Generate all 8 square symmetry transforms
    let mut current = pattern.to_vec();

    for i in 0..8 {
        if i < symmetry.len() && symmetry[i] {
            // Check if we've seen this pattern
            let key: Vec<u8> = current.clone();
            if !seen.contains(&key) {
                seen.insert(key.clone());
                results.push(current.clone());
            }
        }

        // Transform for next iteration
        if i == 3 {
            // After 4 rotations, reflect
            current = reflect_pattern(&current, n);
        } else {
            // Rotate 90 degrees
            current = rotate_pattern(&current, n);
        }
    }

    if results.is_empty() {
        results.push(pattern.to_vec());
    }

    results
}

/// Rotate a pattern 90 degrees clockwise.
///
/// C# Reference: Helper.Rotated (line 89)
fn rotate_pattern(p: &[u8], n: usize) -> Vec<u8> {
    let mut result = vec![0u8; n * n];
    for y in 0..n {
        for x in 0..n {
            result[x + y * n] = p[(n - 1 - y) + x * n];
        }
    }
    result
}

/// Reflect a pattern horizontally.
///
/// C# Reference: Helper.Reflected (line 90)
fn reflect_pattern(p: &[u8], n: usize) -> Vec<u8> {
    let mut result = vec![0u8; n * n];
    for y in 0..n {
        for x in 0..n {
            result[x + y * n] = p[(n - 1 - x) + y * n];
        }
    }
    result
}

/// Build the propagator for overlap model.
///
/// Two patterns are compatible in a direction if their overlapping
/// cells match.
///
/// C# Reference: OverlapNode.agrees (lines 103-107) and propagator building (lines 110-121)
fn build_overlap_propagator(patterns: &[Vec<u8>], n: usize) -> Vec<Vec<Vec<usize>>> {
    let num_patterns = patterns.len();

    // 4 directions for 2D
    let mut propagator = vec![vec![Vec::new(); num_patterns]; 4];

    for d in 0..4 {
        let dx = DX[d];
        let dy = DY[d];

        for (t1, p1) in patterns.iter().enumerate() {
            for (t2, p2) in patterns.iter().enumerate() {
                if patterns_agree(p1, p2, dx, dy, n) {
                    propagator[d][t1].push(t2);
                }
            }
        }
    }

    propagator
}

/// Check if two patterns agree when offset by (dx, dy).
///
/// C# Reference: OverlapNode.agrees (lines 103-107)
fn patterns_agree(p1: &[u8], p2: &[u8], dx: i32, dy: i32, n: usize) -> bool {
    let n_i32 = n as i32;

    let xmin = if dx < 0 { 0 } else { dx };
    let xmax = if dx < 0 { dx + n_i32 } else { n_i32 };
    let ymin = if dy < 0 { 0 } else { dy };
    let ymax = if dy < 0 { dy + n_i32 } else { n_i32 };

    for y in ymin..ymax {
        for x in xmin..xmax {
            let idx1 = (x + y * n_i32) as usize;
            let idx2 = ((x - dx) + (y - dy) * n_i32) as usize;
            if p1[idx1] != p2[idx2] {
                return false;
            }
        }
    }

    true
}

/// Build map from input grid values to allowed patterns.
///
/// C# Reference: OverlapNode.Load() lines 123-131
fn build_pattern_map(
    input_grid: &MjGrid,
    output_grid: &MjGrid,
    patterns: &[Vec<u8>],
    rules: &[(u8, Vec<u8>)],
) -> Vec<Vec<bool>> {
    let num_patterns = patterns.len();
    let num_input_values = input_grid.c as usize;

    // Default: value 0 allows all patterns
    let mut map = vec![vec![true; num_patterns]; num_input_values];

    // Apply rules
    for (input_value, allowed_outputs) in rules {
        if (*input_value as usize) < map.len() {
            // Pattern is allowed if its first cell matches any allowed output
            for (p, pattern) in patterns.iter().enumerate() {
                let first_cell = pattern[0];
                map[*input_value as usize][p] = allowed_outputs.contains(&first_cell);
            }
        }
    }

    // If no rule for value 0, it allows all patterns (already default)

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power() {
        assert_eq!(power(2, 4), 16);
        assert_eq!(power(3, 3), 27);
        assert_eq!(power(10, 0), 1);
    }

    #[test]
    fn test_pattern_index_roundtrip() {
        let n = 2;
        let c = 3;

        // Test a specific pattern
        let pattern = vec![0, 1, 2, 0];
        let idx = pattern_index(&pattern, c);
        let restored = pattern_from_index(idx, n, c);
        assert_eq!(pattern, restored);
    }

    #[test]
    fn test_extract_pattern() {
        // 3x3 sample
        let sample = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];

        let pattern = extract_pattern(&sample, 3, 3, 0, 0, 2);
        // Should extract top-left 2x2
        assert_eq!(pattern, vec![0, 1, 3, 4]);

        let pattern2 = extract_pattern(&sample, 3, 3, 1, 1, 2);
        // Should extract center 2x2
        assert_eq!(pattern2, vec![4, 5, 7, 8]);
    }

    #[test]
    fn test_rotate_pattern() {
        // 2x2 pattern stored as row-major: [0,1,2,3] represents:
        // 0 1
        // 2 3
        let pattern = vec![0, 1, 2, 3];

        // C# rotation formula: result[x + y * N] = p[N - 1 - y + x * N]
        // For 90 degree clockwise rotation:
        // result[0] = p[1] = 1, result[1] = p[3] = 3
        // result[2] = p[0] = 0, result[3] = p[2] = 2
        // So rotated = [1, 3, 0, 2]
        let rotated = rotate_pattern(&pattern, 2);
        assert_eq!(rotated, vec![1, 3, 0, 2]);
    }

    #[test]
    fn test_reflect_pattern() {
        // 2x2 pattern:
        // 0 1
        // 2 3
        let pattern = vec![0, 1, 2, 3];

        // After horizontal reflection:
        // 1 0
        // 3 2
        let reflected = reflect_pattern(&pattern, 2);
        assert_eq!(reflected, vec![1, 0, 3, 2]);
    }

    #[test]
    fn test_patterns_agree() {
        // Pattern 1: 0 1 / 2 3
        let p1 = vec![0, 1, 2, 3];
        // Pattern 2: 1 0 / 3 2
        let p2 = vec![1, 0, 3, 2];

        // p1 offset by (1, 0) means p1's right column should match p2's left column
        // p1 right column: 1, 3
        // p2 left column: 1, 3
        // Should agree
        assert!(patterns_agree(&p1, &p2, 1, 0, 2));

        // p1 offset by (-1, 0) means p1's left column should match p2's right column
        // p1 left column: 0, 2
        // p2 right column: 0, 2
        // Should agree
        assert!(patterns_agree(&p1, &p2, -1, 0, 2));
    }

    #[test]
    fn test_pattern_symmetries_all() {
        let pattern = vec![0, 1, 2, 3];
        let symmetry = vec![true; 8]; // All symmetries

        let variants = pattern_symmetries(&pattern, 2, &symmetry);

        // Should generate up to 8 unique variants
        assert!(!variants.is_empty());
        assert!(variants.len() <= 8);

        // All should be unique
        let mut seen = std::collections::HashSet::new();
        for v in &variants {
            assert!(seen.insert(v.clone()));
        }
    }

    #[test]
    fn test_pattern_symmetries_none() {
        let pattern = vec![0, 1, 2, 3];
        let symmetry = vec![true, false, false, false, false, false, false, false];

        let variants = pattern_symmetries(&pattern, 2, &symmetry);

        // Should only generate the original pattern
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0], pattern);
    }

    #[test]
    fn test_build_overlap_propagator() {
        // Two patterns that should be compatible
        let patterns = vec![
            vec![0, 0, 0, 0], // All zeros
            vec![0, 1, 0, 1], // Checkerboard
        ];

        let propagator = build_overlap_propagator(&patterns, 2);

        // Should have 4 directions
        assert_eq!(propagator.len(), 4);

        // Pattern 0 (all zeros) is compatible with itself in all directions
        for d in 0..4 {
            assert!(propagator[d][0].contains(&0));
        }
    }
}
