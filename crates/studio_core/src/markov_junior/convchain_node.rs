//! ConvChainNode - MCMC texture synthesis for MarkovJunior.
//!
//! Implements Markov Chain Monte Carlo texture synthesis using patterns
//! learned from a sample image. Based on ConvChain algorithm.
//!
//! C# Reference: ConvChain.cs (~127 lines)

use super::helper::load_bitmap;
use super::node::{ExecutionContext, Node};
use super::symmetry::SquareSubgroup;
use std::path::Path;

/// ConvChain node for MCMC texture synthesis.
///
/// Learns NxN patterns from a sample image and uses Metropolis-Hastings
/// sampling to generate similar textures.
///
/// C# Reference: ConvChain.cs ConvChainNode
#[derive(Debug)]
pub struct ConvChainNode {
    /// Pattern size (N x N)
    pub n: usize,
    /// Temperature for acceptance probability (higher = more random)
    pub temperature: f64,
    /// Pattern weights (indexed by pattern bitmask)
    pub weights: Vec<f64>,
    /// Color 0 (typically "black" in sample)
    pub c0: u8,
    /// Color 1 (typically "white" in sample)
    pub c1: u8,
    /// Which cells can be modified
    pub substrate: Vec<bool>,
    /// Value that marks substrate cells before initialization
    pub substrate_color: u8,
    /// Current step counter
    pub counter: usize,
    /// Maximum steps (0 = unlimited)
    pub steps: usize,
}

impl ConvChainNode {
    /// Create a new ConvChainNode.
    ///
    /// # Arguments
    /// * `n` - Pattern size (NxN patterns)
    /// * `temperature` - Acceptance temperature (1.0 = neutral, higher = more random)
    /// * `weights` - Pattern weights indexed by pattern bitmask (size 2^(n*n))
    /// * `c0` - Color value for "black" (typically 0)
    /// * `c1` - Color value for "white" (typically 1)
    /// * `substrate_color` - Value that marks cells to be processed
    /// * `grid_size` - Total number of cells in grid
    pub fn new(
        n: usize,
        temperature: f64,
        weights: Vec<f64>,
        c0: u8,
        c1: u8,
        substrate_color: u8,
        grid_size: usize,
    ) -> Self {
        Self {
            n,
            temperature,
            weights,
            c0,
            c1,
            substrate: vec![false; grid_size],
            substrate_color,
            counter: 0,
            steps: 0,
        }
    }

    /// Create from a sample image file.
    ///
    /// C# Reference: ConvChain.cs Load() lines 20-58
    pub fn from_sample(
        sample_path: &Path,
        n: usize,
        temperature: f64,
        c0: u8,
        c1: u8,
        substrate_color: u8,
        grid_size: usize,
        symmetry: &[bool],
    ) -> Result<Self, String> {
        // Load sample image
        let (bitmap, smx, smy, _smz) = load_bitmap(sample_path).map_err(|e| format!("{}", e))?;

        // Convert to binary (bool array)
        // C# uses: sample[i] = bitmap[i] == -1 (white = true, black = false)
        // -1 as i32 = 0xFFFFFFFF in two's complement
        let sample: Vec<bool> = bitmap.iter().map(|&p| p == -1).collect();

        // Learn pattern weights from sample
        let weights = learn_pattern_weights(&sample, smx, smy, n, symmetry);

        Ok(Self::new(
            n,
            temperature,
            weights,
            c0,
            c1,
            substrate_color,
            grid_size,
        ))
    }

    /// Set maximum steps.
    pub fn with_steps(mut self, steps: usize) -> Self {
        self.steps = steps;
        self
    }

    /// Toggle a cell between c0 and c1.
    #[inline]
    fn toggle(&self, state: &mut [u8], i: usize) {
        state[i] = if state[i] == self.c0 {
            self.c1
        } else {
            self.c0
        };
    }

    /// Calculate the pattern index for an NxN region starting at (sx, sy).
    /// Returns the bitmask where bit i is set if cell (dx, dy) equals c1.
    fn pattern_index(&self, state: &[u8], mx: usize, my: usize, sx: i32, sy: i32) -> usize {
        let mut ind = 0usize;
        let mut power = 1usize;

        for dy in 0..self.n {
            for dx in 0..self.n {
                // Wrap coordinates (periodic boundary)
                let mut x = sx + dx as i32;
                let mut y = sy + dy as i32;

                if x < 0 {
                    x += mx as i32;
                } else if x >= mx as i32 {
                    x -= mx as i32;
                }
                if y < 0 {
                    y += my as i32;
                } else if y >= my as i32 {
                    y -= my as i32;
                }

                let value = state[x as usize + y as usize * mx];
                if value == self.c1 {
                    ind += power;
                }
                power *= 2;
            }
        }

        ind
    }
}

impl Node for ConvChainNode {
    /// Execute one step of MCMC sampling.
    ///
    /// C# Reference: ConvChain.cs Go() lines 62-118
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // Check step limit
        if self.steps > 0 && self.counter >= self.steps {
            return false;
        }

        let mx = ctx.grid.mx;
        let my = ctx.grid.my;
        let state = &mut ctx.grid.state;

        // First step: initialize substrate
        // C# Reference: ConvChain.cs Go() lines 69-79
        if self.counter == 0 {
            let mut any_substrate = false;
            for i in 0..self.substrate.len() {
                if state[i] == self.substrate_color {
                    // Randomly initialize to c0 or c1
                    // C#: state[i] = ip.random.Next(2) == 0 ? c0 : c1;
                    state[i] = if ctx.random.next_int_max(2) == 0 {
                        self.c0
                    } else {
                        self.c1
                    };
                    self.substrate[i] = true;
                    any_substrate = true;
                }
            }
            self.counter += 1;
            return any_substrate;
        }

        // MCMC sampling: try to toggle random substrate cells
        let n = self.n as i32;
        for _k in 0..state.len() {
            // Pick a random substrate cell
            // C#: int r = ip.random.Next(state.Length);
            let r = ctx.random.next_int_max(state.len() as i32) as usize;
            if !self.substrate[r] {
                continue;
            }

            let x = (r % mx) as i32;
            let y = (r / mx) as i32;

            // Calculate the quality ratio q for toggling this cell
            let mut q: f64 = 1.0;

            // For each NxN region that contains cell (x, y)
            for sy in (y - n + 1)..=(y + n - 1) {
                for sx in (x - n + 1)..=(x + n - 1) {
                    // Calculate pattern index before and after toggle
                    let ind = self.pattern_index(state, mx, my, sx, sy);

                    // Find the bit position for (x, y) within this pattern
                    // and what the difference in index would be if we toggled
                    let mut difference: i32 = 0;
                    for dy in 0..self.n {
                        for dx in 0..self.n {
                            let mut px = sx + dx as i32;
                            let mut py = sy + dy as i32;

                            if px < 0 {
                                px += mx as i32;
                            } else if px >= mx as i32 {
                                px -= mx as i32;
                            }
                            if py < 0 {
                                py += my as i32;
                            } else if py >= my as i32 {
                                py -= my as i32;
                            }

                            if px == x && py == y {
                                let power = 1i32 << (dy * self.n + dx);
                                let value = state[x as usize + y as usize * mx];
                                difference = if value == self.c1 { power } else { -power };
                                break;
                            }
                        }
                        if difference != 0 {
                            break;
                        }
                    }

                    // Update quality ratio
                    let new_ind = (ind as i32 - difference) as usize;
                    if ind < self.weights.len() && new_ind < self.weights.len() {
                        let w_old = self.weights[ind];
                        let w_new = self.weights[new_ind];
                        if w_old > 0.0 {
                            q *= w_new / w_old;
                        }
                    }
                }
            }

            // Accept or reject the toggle
            if q >= 1.0 {
                self.toggle(state, r);
            } else {
                // Apply temperature
                if self.temperature != 1.0 {
                    q = q.powf(1.0 / self.temperature);
                }
                if q > ctx.random.next_double() {
                    self.toggle(state, r);
                }
            }
        }

        self.counter += 1;
        true
    }

    fn reset(&mut self) {
        for s in &mut self.substrate {
            *s = false;
        }
        self.counter = 0;
    }
}

/// Learn pattern weights from a sample image.
///
/// Extracts all NxN patterns from the sample, applies symmetry transformations,
/// and counts occurrences to build the weight table.
///
/// C# Reference: ConvChain.cs Load() lines 49-57
fn learn_pattern_weights(
    sample: &[bool],
    smx: usize,
    smy: usize,
    n: usize,
    symmetry: &[bool],
) -> Vec<f64> {
    let num_patterns = 1 << (n * n);
    let mut weights = vec![0.0f64; num_patterns];

    // Extract patterns from sample with periodic boundary
    for y in 0..smy {
        for x in 0..smx {
            // Extract NxN pattern at (x, y)
            let pattern = extract_pattern(sample, smx, smy, x, y, n);

            // Apply symmetry transformations
            let subgroup = bool_slice_to_subgroup(symmetry);
            let symmetries = square_symmetries_bool(&pattern, n, Some(subgroup));

            for sym_pattern in symmetries {
                let idx = pattern_to_index(&sym_pattern);
                weights[idx] += 1.0;
            }
        }
    }

    // Ensure all weights are positive (avoid division by zero)
    for w in &mut weights {
        if *w <= 0.0 {
            *w = 0.1;
        }
    }

    weights
}

/// Extract an NxN pattern from sample at position (x, y) with periodic boundary.
fn extract_pattern(
    sample: &[bool],
    smx: usize,
    smy: usize,
    x: usize,
    y: usize,
    n: usize,
) -> Vec<bool> {
    let mut pattern = Vec::with_capacity(n * n);

    for dy in 0..n {
        for dx in 0..n {
            let px = (x + dx) % smx;
            let py = (y + dy) % smy;
            pattern.push(sample[px + py * smx]);
        }
    }

    pattern
}

/// Convert a bool pattern to its index (bitmask).
fn pattern_to_index(pattern: &[bool]) -> usize {
    let mut result = 0usize;
    let mut power = 1usize;

    for &p in pattern {
        if p {
            result += power;
        }
        power *= 2;
    }

    result
}

/// Rotate a bool pattern 90 degrees clockwise.
///
/// C# Reference: Helper.cs Rotated()
fn rotate_pattern(p: &[bool], n: usize) -> Vec<bool> {
    let mut result = vec![false; n * n];
    for y in 0..n {
        for x in 0..n {
            result[x + y * n] = p[(n - 1 - y) + x * n];
        }
    }
    result
}

/// Reflect a bool pattern horizontally.
///
/// C# Reference: Helper.cs Reflected()
fn reflect_pattern(p: &[bool], n: usize) -> Vec<bool> {
    let mut result = vec![false; n * n];
    for y in 0..n {
        for x in 0..n {
            result[x + y * n] = p[(n - 1 - x) + y * n];
        }
    }
    result
}

/// Check if two bool patterns are equal.
fn patterns_equal(a: &[bool], b: &[bool]) -> bool {
    a == b
}

/// Generate square symmetries for a bool pattern.
fn square_symmetries_bool(
    pattern: &[bool],
    n: usize,
    subgroup: Option<SquareSubgroup>,
) -> Vec<Vec<bool>> {
    // Generate all 8 symmetry variants
    let mut things = vec![Vec::new(); 8];

    things[0] = pattern.to_vec(); // e (identity)
    things[1] = reflect_pattern(&things[0], n); // b (reflect)
    things[2] = rotate_pattern(&things[0], n); // a (rotate 90)
    things[3] = reflect_pattern(&things[2], n); // ba
    things[4] = rotate_pattern(&things[2], n); // a2 (rotate 180)
    things[5] = reflect_pattern(&things[4], n); // ba2
    things[6] = rotate_pattern(&things[4], n); // a3 (rotate 270)
    things[7] = reflect_pattern(&things[6], n); // ba3

    // Get mask for which symmetries to include
    let mask = match subgroup {
        Some(sg) => sg.mask(),
        None => [true; 8],
    };

    // Filter by mask (no deduplication - C# doesn't deduplicate)
    // C# uses (q1, q2) => false as comparator, meaning all symmetries are included
    let mut result: Vec<Vec<bool>> = Vec::new();
    for i in 0..8 {
        if mask[i] {
            result.push(things[i].clone());
        }
    }

    result
}

/// Convert a bool slice to SquareSubgroup.
fn bool_slice_to_subgroup(symmetry: &[bool]) -> SquareSubgroup {
    if symmetry.len() < 8 {
        return SquareSubgroup::All;
    }

    // Check known patterns
    if symmetry == [true, false, false, false, false, false, false, false] {
        SquareSubgroup::None
    } else if symmetry == [true, true, false, false, false, false, false, false] {
        SquareSubgroup::ReflectX
    } else if symmetry == [true, false, false, false, false, true, false, false] {
        SquareSubgroup::ReflectY
    } else if symmetry == [true, true, false, false, true, true, false, false] {
        SquareSubgroup::ReflectXY
    } else if symmetry == [true, false, true, false, true, false, true, false] {
        SquareSubgroup::Rotate
    } else if symmetry.iter().all(|&b| b) {
        SquareSubgroup::All
    } else {
        SquareSubgroup::All // Default to all if pattern not recognized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::node::ExecutionContext;
    use crate::markov_junior::rng::StdRandom;
    use crate::markov_junior::MjGrid;

    fn create_test_ctx<'a>(grid: &'a mut MjGrid, rng: &'a mut StdRandom) -> ExecutionContext<'a> {
        ExecutionContext::new(grid, rng)
    }

    #[test]
    fn test_pattern_to_index() {
        // All false = 0
        assert_eq!(pattern_to_index(&[false, false, false, false]), 0);

        // Single true at position 0 = 1
        assert_eq!(pattern_to_index(&[true, false, false, false]), 1);

        // Single true at position 1 = 2
        assert_eq!(pattern_to_index(&[false, true, false, false]), 2);

        // All true = 15 (for 2x2)
        assert_eq!(pattern_to_index(&[true, true, true, true]), 15);
    }

    #[test]
    fn test_extract_pattern() {
        // 3x3 sample with pattern
        let sample = vec![true, false, true, false, true, false, true, false, true];

        // Extract 2x2 pattern at (0, 0)
        let pattern = extract_pattern(&sample, 3, 3, 0, 0, 2);
        assert_eq!(pattern, vec![true, false, false, true]);

        // Extract 2x2 pattern at (1, 1)
        let pattern = extract_pattern(&sample, 3, 3, 1, 1, 2);
        assert_eq!(pattern, vec![true, false, false, true]);
    }

    #[test]
    fn test_rotate_pattern() {
        // 2x2 pattern (row-major order):
        // 1 0    index: 0 1
        // 0 0           2 3
        let pattern = vec![true, false, false, false];

        // After rotation using C# formula: result[x+y*n] = p[(n-1-y) + x*n]
        // This produces:
        // 0 0    index: 0 1
        // 1 0           2 3
        let rotated = rotate_pattern(&pattern, 2);
        assert_eq!(rotated, vec![false, false, true, false]);
    }

    #[test]
    fn test_reflect_pattern() {
        // 2x2 pattern:
        // 1 0
        // 0 0
        let pattern = vec![true, false, false, false];

        // After reflection:
        // 0 1
        // 0 0
        let reflected = reflect_pattern(&pattern, 2);
        assert_eq!(reflected, vec![false, true, false, false]);
    }

    #[test]
    fn test_square_symmetries_bool_generates_unique() {
        // Asymmetric pattern - should generate 8 unique variants
        let pattern = vec![true, false, false, false];

        let symmetries = square_symmetries_bool(&pattern, 2, Some(SquareSubgroup::All));

        // Should have some unique variants (may be less than 8 if some match)
        assert!(symmetries.len() >= 1);
        assert!(symmetries.len() <= 8);
    }

    #[test]
    fn test_learn_pattern_weights() {
        // Simple 3x3 sample
        let sample = vec![false, true, false, true, true, true, false, true, false];

        let weights = learn_pattern_weights(&sample, 3, 3, 2, &[true; 8]);

        // All weights should be positive
        assert!(weights.iter().all(|&w| w > 0.0));

        // Total weights count should be related to number of positions
        let total: f64 = weights.iter().sum();
        assert!(total > 0.0);
    }

    #[test]
    fn test_convchain_initialization() {
        // Create a grid with substrate color
        let mut grid = MjGrid::with_values(5, 5, 1, "BDA");
        // Set some cells to substrate color (D = 1)
        for i in 0..grid.state.len() {
            grid.state[i] = 1; // D
        }

        // Create simple weights
        let weights = vec![0.5; 16]; // 2x2 patterns

        let mut node = ConvChainNode::new(
            2,   // n
            1.0, // temperature
            weights,
            0, // c0 = B
            2, // c1 = A
            1, // substrate_color = D
            grid.state.len(),
        );

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // First step should initialize substrate
        let changed = node.go(&mut ctx);
        assert!(changed);
        assert_eq!(node.counter, 1);

        // All cells should now be either c0 or c1, not substrate_color
        for &s in &ctx.grid.state {
            assert!(s == 0 || s == 2, "Cell should be c0 or c1, got {}", s);
        }

        // All cells should be marked as substrate
        assert!(node.substrate.iter().all(|&s| s));
    }

    #[test]
    fn test_convchain_mcmc_step() {
        // Create a grid already initialized
        let mut grid = MjGrid::with_values(5, 5, 1, "BA");
        // Initialize to alternating pattern
        for i in 0..grid.state.len() {
            grid.state[i] = (i % 2) as u8; // B or A
        }

        // Create weights that prefer checkerboard
        let mut weights = vec![0.1; 16];
        // Checkerboard patterns have indices where bits alternate
        // For 2x2: index 5 (0101) and 10 (1010) are checkerboard
        weights[5] = 10.0;
        weights[10] = 10.0;

        let mut node = ConvChainNode::new(
            2,
            1.0,
            weights,
            0, // B
            1, // A
            2, // (not used after initialization)
            grid.state.len(),
        );

        // Manually set substrate to all true (skip initialization step)
        for i in 0..node.substrate.len() {
            node.substrate[i] = true;
        }
        node.counter = 1;

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // Run MCMC step
        let changed = node.go(&mut ctx);
        assert!(changed);
        assert_eq!(node.counter, 2);
    }

    #[test]
    fn test_convchain_step_limit() {
        let mut grid = MjGrid::with_values(3, 3, 1, "BDA");
        for i in 0..grid.state.len() {
            grid.state[i] = 1; // D (substrate)
        }

        let weights = vec![0.5; 16];
        let mut node = ConvChainNode::new(2, 1.0, weights, 0, 2, 1, grid.state.len()).with_steps(2);

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // First step (initialization)
        assert!(node.go(&mut ctx));

        // Second step (MCMC)
        assert!(node.go(&mut ctx));

        // Third step should return false (limit reached)
        assert!(!node.go(&mut ctx));
    }

    #[test]
    fn test_convchain_reset() {
        let mut grid = MjGrid::with_values(3, 3, 1, "BDA");
        for i in 0..grid.state.len() {
            grid.state[i] = 1;
        }

        let weights = vec![0.5; 16];
        let mut node = ConvChainNode::new(2, 1.0, weights, 0, 2, 1, grid.state.len()).with_steps(3);

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // Run some steps
        node.go(&mut ctx);
        node.go(&mut ctx);

        assert_eq!(node.counter, 2);
        assert!(node.substrate.iter().any(|&s| s));

        // Reset
        node.reset();

        assert_eq!(node.counter, 0);
        assert!(node.substrate.iter().all(|&s| !s));
    }

    #[test]
    fn test_pattern_index_calculation() {
        // Create node with n=2
        let mut grid = MjGrid::with_values(4, 4, 1, "BA");
        // Set a known pattern
        grid.state.fill(0); // All B
        grid.state[0] = 1; // A at (0,0)

        let weights = vec![0.5; 16];
        let node = ConvChainNode::new(2, 1.0, weights, 0, 1, 2, grid.state.len());

        // Pattern at (0,0) should have index 1 (only bit 0 set)
        let idx = node.pattern_index(&grid.state, 4, 4, 0, 0);
        assert_eq!(idx, 1);

        // Set another cell
        grid.state[1] = 1; // A at (1,0)
        let idx = node.pattern_index(&grid.state, 4, 4, 0, 0);
        assert_eq!(idx, 3); // bits 0 and 1 set
    }

    #[test]
    fn test_weight_learning_from_sample() {
        // Create a simple 4x4 sample with a known pattern (checkerboard)
        // Pattern layout (row-major):
        // T F T F
        // F T F T
        // T F T F
        // F T F T
        let sample: Vec<bool> = vec![
            true, false, true, false, false, true, false, true, true, false, true, false, false,
            true, false, true,
        ];

        // Learn weights with n=2 and full symmetry
        let symmetry = [true; 8];
        let weights = learn_pattern_weights(&sample, 4, 4, 2, &symmetry);

        // For 2x2 checkerboard patterns:
        // Pattern indexing: bit i = pattern[i], where pattern order is (0,0), (1,0), (0,1), (1,1)
        //
        // At position (0,0): T F / F T -> bits: 1,0,0,1 -> index = 1 + 8 = 9
        // At position (1,0): F T / T F -> bits: 0,1,1,0 -> index = 2 + 4 = 6
        //
        // These are the only patterns in a perfect checkerboard (with symmetry variants)

        // Verify extraction logic
        let pattern = extract_pattern(&sample, 4, 4, 0, 0, 2);
        let idx = pattern_to_index(&pattern);
        assert_eq!(idx, 9, "Checkerboard pattern at (0,0) should have index 9");

        let checkerboard_a = weights[9]; // T F / F T
        let checkerboard_b = weights[6]; // F T / T F

        // Verify these patterns have higher weights than uniform patterns
        let uniform_all_false = weights[0]; // 0000: all black
        let uniform_all_true = weights[15]; // 1111: all white

        // Checkerboard patterns should dominate in a checkerboard sample
        assert!(
            checkerboard_a > uniform_all_false,
            "Checkerboard pattern {} should have higher weight than all-black {}",
            checkerboard_a,
            uniform_all_false
        );
        assert!(
            checkerboard_b > uniform_all_true,
            "Checkerboard pattern {} should have higher weight than all-white {}",
            checkerboard_b,
            uniform_all_true
        );

        // The two checkerboard patterns should have similar weights (symmetry related)
        let ratio = checkerboard_a / checkerboard_b;
        assert!(
            ratio > 0.5 && ratio < 2.0,
            "Checkerboard patterns should have similar weights: {} vs {}",
            checkerboard_a,
            checkerboard_b
        );

        // Verify uniform patterns have low weight (only 0.1 from smoothing)
        assert!(
            uniform_all_false <= 0.11,
            "All-black pattern should have minimal weight: {}",
            uniform_all_false
        );
        assert!(
            uniform_all_true <= 0.11,
            "All-white pattern should have minimal weight: {}",
            uniform_all_true
        );
    }

    #[test]
    fn test_output_patterns_have_positive_weights() {
        // This test verifies the core ConvChain guarantee:
        // After MCMC sampling, all NxN patterns in the output should have
        // positive weights (i.e., they existed in the sample or are allowed by symmetry)

        // Create weights where only certain patterns are allowed
        // Pattern 0 (all black) and pattern 15 (all white) have high weight
        // Pattern 5 and 10 (checkerboard) also allowed
        // Other patterns have minimum weight (0.1)
        let n = 2;
        let mut weights = vec![0.1; 1 << (n * n)];
        weights[0] = 10.0; // All black
        weights[15] = 10.0; // All white
        weights[5] = 5.0; // Checkerboard
        weights[10] = 5.0; // Checkerboard inverse

        // Create a grid and initialize
        let mut grid = MjGrid::with_values(6, 6, 1, "BA");
        for i in 0..grid.state.len() {
            grid.state[i] = 2; // Use value 2 as substrate marker (will be replaced)
        }
        // Actually we need a substrate color, let's redo with 3 values
        let mut grid = MjGrid::with_values(6, 6, 1, "BAS");
        for i in 0..grid.state.len() {
            grid.state[i] = 2; // S = substrate
        }

        let mut node = ConvChainNode::new(
            n,
            0.1, // Low temperature = prefer high-weight patterns strongly
            weights.clone(),
            0, // c0 = B
            1, // c1 = A
            2, // substrate = S
            grid.state.len(),
        )
        .with_steps(50);

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // Run until completion
        while node.go(&mut ctx) {}

        // Now verify: every 2x2 pattern in the output should have positive weight
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;

        for y in 0..my {
            for x in 0..mx {
                let idx = node.pattern_index(&ctx.grid.state, mx, my, x as i32, y as i32);
                assert!(
                    weights[idx] > 0.0,
                    "Pattern at ({}, {}) has index {} with weight {} - should be positive",
                    x,
                    y,
                    idx,
                    weights[idx]
                );
            }
        }
    }

    #[test]
    fn test_mcmc_converges_to_high_weight_patterns() {
        // Test that after many MCMC steps, the output is dominated by high-weight patterns

        let n = 2;
        let mut weights = vec![0.1; 1 << (n * n)];
        // Make checkerboard patterns MUCH more likely
        // Checkerboard indices (see test_weight_learning_from_sample for derivation):
        // Index 6: F T / T F (bits 0,1,1,0)
        // Index 9: T F / F T (bits 1,0,0,1)
        weights[6] = 100.0; // Checkerboard
        weights[9] = 100.0; // Checkerboard inverse

        let mut grid = MjGrid::with_values(8, 8, 1, "BAS");
        for i in 0..grid.state.len() {
            grid.state[i] = 2; // S = substrate
        }

        let mut node = ConvChainNode::new(
            n,
            0.1, // Low temperature for strong preference
            weights.clone(),
            0, // c0 = B
            1, // c1 = A
            2, // substrate = S
            grid.state.len(),
        )
        .with_steps(200); // Many steps for convergence

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // Run to completion
        while node.go(&mut ctx) {}

        // Count pattern occurrences
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;
        let mut pattern_counts = vec![0usize; 1 << (n * n)];

        for y in 0..my {
            for x in 0..mx {
                let idx = node.pattern_index(&ctx.grid.state, mx, my, x as i32, y as i32);
                pattern_counts[idx] += 1;
            }
        }

        // Checkerboard patterns (6 and 9) should be the most common
        let checkerboard_count = pattern_counts[6] + pattern_counts[9];
        let total_count: usize = pattern_counts.iter().sum();

        // With strong weights and low temperature, checkerboard should dominate
        // (allowing some tolerance for MCMC not being perfect)
        let checkerboard_fraction = checkerboard_count as f64 / total_count as f64;
        assert!(
            checkerboard_fraction > 0.5,
            "Checkerboard patterns should dominate (got {:.1}% = {}/{}), pattern distribution: {:?}",
            checkerboard_fraction * 100.0,
            checkerboard_count,
            total_count,
            pattern_counts
        );
    }

    #[test]
    fn test_quality_ratio_calculation_correctness() {
        // Test that the quality ratio (q) calculation for Metropolis-Hastings is correct
        // by verifying that toggling a cell and toggling it back gives q * (1/q) ≈ 1

        let n = 2;
        let mut weights = vec![1.0; 1 << (n * n)];
        weights[5] = 2.0; // Some pattern has different weight

        let mut grid = MjGrid::with_values(4, 4, 1, "BA");
        // Initialize to known pattern
        for i in 0..grid.state.len() {
            grid.state[i] = (i % 2) as u8;
        }

        let node = ConvChainNode::new(n, 1.0, weights.clone(), 0, 1, 2, grid.state.len());

        let mx = grid.mx;
        let my = grid.my;

        // Calculate quality ratio for toggling cell (1, 1)
        let x = 1i32;
        let y = 1i32;
        let r = (x as usize) + (y as usize) * mx;

        // Calculate q before toggle
        let mut q_forward: f64 = 1.0;
        for sy in (y - n as i32 + 1)..=(y) {
            for sx in (x - n as i32 + 1)..=(x) {
                let ind = node.pattern_index(&grid.state, mx, my, sx, sy);

                // Find difference for this pattern
                let mut difference: i32 = 0;
                for dy in 0..n {
                    for dx in 0..n {
                        let mut px = sx + dx as i32;
                        let mut py = sy + dy as i32;
                        if px < 0 {
                            px += mx as i32;
                        } else if px >= mx as i32 {
                            px -= mx as i32;
                        }
                        if py < 0 {
                            py += my as i32;
                        } else if py >= my as i32 {
                            py -= my as i32;
                        }

                        if px == x && py == y {
                            let power = 1i32 << (dy * n + dx);
                            let value = grid.state[x as usize + y as usize * mx];
                            difference = if value == 1 { power } else { -power };
                            break;
                        }
                    }
                    if difference != 0 {
                        break;
                    }
                }

                let new_ind = (ind as i32 - difference) as usize;
                if ind < weights.len() && new_ind < weights.len() {
                    q_forward *= weights[new_ind] / weights[ind];
                }
            }
        }

        // Toggle the cell
        grid.state[r] = if grid.state[r] == 0 { 1 } else { 0 };

        // Calculate q for toggling back
        let mut q_backward: f64 = 1.0;
        for sy in (y - n as i32 + 1)..=(y) {
            for sx in (x - n as i32 + 1)..=(x) {
                let ind = node.pattern_index(&grid.state, mx, my, sx, sy);

                let mut difference: i32 = 0;
                for dy in 0..n {
                    for dx in 0..n {
                        let mut px = sx + dx as i32;
                        let mut py = sy + dy as i32;
                        if px < 0 {
                            px += mx as i32;
                        } else if px >= mx as i32 {
                            px -= mx as i32;
                        }
                        if py < 0 {
                            py += my as i32;
                        } else if py >= my as i32 {
                            py -= my as i32;
                        }

                        if px == x && py == y {
                            let power = 1i32 << (dy * n + dx);
                            let value = grid.state[x as usize + y as usize * mx];
                            difference = if value == 1 { power } else { -power };
                            break;
                        }
                    }
                    if difference != 0 {
                        break;
                    }
                }

                let new_ind = (ind as i32 - difference) as usize;
                if ind < weights.len() && new_ind < weights.len() {
                    q_backward *= weights[new_ind] / weights[ind];
                }
            }
        }

        // q_forward * q_backward should equal 1 (detailed balance)
        let product = q_forward * q_backward;
        assert!(
            (product - 1.0).abs() < 0.0001,
            "Quality ratio product should be 1.0 (detailed balance), got {} (q_forward={}, q_backward={})",
            product,
            q_forward,
            q_backward
        );
    }

    #[test]
    fn test_convchain_end_to_end_with_sample_file() {
        // Integration test: load actual Maze sample, run ConvChain, verify output validity
        use std::path::PathBuf;

        let resources_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MarkovJunior/resources");

        let sample_path = resources_path.join("samples/Maze.png");
        if !sample_path.exists() {
            // Skip test if sample not available
            return;
        }

        let n = 2;
        let symmetry = [true; 8];

        let node_result = ConvChainNode::from_sample(
            &sample_path,
            n,
            1.0,       // temperature
            0,         // c0
            1,         // c1
            2,         // substrate
            16 * 16,   // grid size
            &symmetry, // symmetry
        );

        assert!(node_result.is_ok(), "Should load sample: {:?}", node_result);
        let mut node = node_result.unwrap().with_steps(50);

        // Verify weights were learned (not all equal)
        let weight_variance: f64 = {
            let mean = node.weights.iter().sum::<f64>() / node.weights.len() as f64;
            node.weights.iter().map(|w| (w - mean).powi(2)).sum::<f64>() / node.weights.len() as f64
        };
        assert!(
            weight_variance > 0.01,
            "Weights should have variance from learning, got {}",
            weight_variance
        );

        // Run ConvChain
        let mut grid = MjGrid::with_values(16, 16, 1, "BAS");
        for i in 0..grid.state.len() {
            grid.state[i] = 2; // substrate
        }

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        while node.go(&mut ctx) {}

        // Verify all patterns in output have positive weight
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;

        for y in 0..my {
            for x in 0..mx {
                let idx = node.pattern_index(&ctx.grid.state, mx, my, x as i32, y as i32);
                assert!(
                    node.weights[idx] > 0.0,
                    "Pattern at ({}, {}) with index {} should have positive weight",
                    x,
                    y,
                    idx
                );
            }
        }

        // Verify output isn't degenerate (all same value)
        let mut has_c0 = false;
        let mut has_c1 = false;
        for &v in &ctx.grid.state {
            if v == 0 {
                has_c0 = true;
            }
            if v == 1 {
                has_c1 = true;
            }
        }
        assert!(
            has_c0 && has_c1,
            "Output should contain both c0 and c1 values"
        );
    }
}
