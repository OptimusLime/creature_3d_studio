//! ConvolutionNode - Cellular automata rules for MarkovJunior.
//!
//! Implements cellular automata-style rules that operate based on neighbor counts.
//! Used for cave generation, Game of Life, and similar patterns.
//!
//! C# Reference: Convolution.cs (~191 lines)

use super::node::{ExecutionContext, Node};
use super::rng::MjRng;

/// Pre-defined 2D kernels for neighbor counting.
///
/// Kernels are 3x3 arrays where 1 means "count this neighbor" and 0 means "ignore".
/// The center cell (index 4) is always 0 because we don't count the cell itself.
pub mod kernels_2d {
    /// Von Neumann neighborhood - 4 orthogonal neighbors (N, S, E, W)
    /// ```text
    /// 0 1 0
    /// 1 0 1
    /// 0 1 0
    /// ```
    pub const VON_NEUMANN: [i32; 9] = [0, 1, 0, 1, 0, 1, 0, 1, 0];

    /// Moore neighborhood - 8 surrounding neighbors
    /// ```text
    /// 1 1 1
    /// 1 0 1
    /// 1 1 1
    /// ```
    pub const MOORE: [i32; 9] = [1, 1, 1, 1, 0, 1, 1, 1, 1];
}

/// Pre-defined 3D kernels for neighbor counting.
///
/// Kernels are 3x3x3 arrays (27 elements) laid out as z-slices.
pub mod kernels_3d {
    /// Von Neumann neighborhood - 6 orthogonal neighbors (±X, ±Y, ±Z)
    pub const VON_NEUMANN: [i32; 27] = [
        // z = -1
        0, 0, 0, 0, 1, 0, 0, 0, 0, // z = 0
        0, 1, 0, 1, 0, 1, 0, 1, 0, // z = +1
        0, 0, 0, 0, 1, 0, 0, 0, 0,
    ];

    /// No corners neighborhood - 18 neighbors (excludes 8 corner diagonals)
    pub const NO_CORNERS: [i32; 27] = [
        // z = -1
        0, 1, 0, 1, 1, 1, 0, 1, 0, // z = 0
        1, 1, 1, 1, 0, 1, 1, 1, 1, // z = +1
        0, 1, 0, 1, 1, 1, 0, 1, 0,
    ];
}

/// A single convolution rule.
///
/// Specifies when to transform a cell based on its current value
/// and the count of specific neighbor values.
#[derive(Debug, Clone)]
pub struct ConvolutionRule {
    /// Input value that this rule matches
    pub input: u8,
    /// Output value to write if rule matches
    pub output: u8,
    /// Which color values to count in neighbors (indices into grid.state)
    pub values: Vec<u8>,
    /// Allowed sum ranges - sums[i] = true if sum i is allowed
    /// Size 28 covers max possible sum (27 neighbors in 3D kernel)
    pub sums: Option<Vec<bool>>,
    /// Probability of applying this rule (0.0 to 1.0)
    pub p: f64,
}

impl ConvolutionRule {
    /// Create a simple rule without sum constraints.
    pub fn new(input: u8, output: u8) -> Self {
        Self {
            input,
            output,
            values: Vec::new(),
            sums: None,
            p: 1.0,
        }
    }

    /// Create a rule with sum constraints.
    ///
    /// `values` - which colors to count in neighbors
    /// `sums` - which sum values allow the rule to fire
    pub fn with_sums(input: u8, output: u8, values: Vec<u8>, sums: Vec<bool>) -> Self {
        Self {
            input,
            output,
            values,
            sums: Some(sums),
            p: 1.0,
        }
    }

    /// Set the probability for this rule.
    pub fn with_probability(mut self, p: f64) -> Self {
        self.p = p;
        self
    }

    /// Parse sum intervals like "5..8" or "3,5..7" into a bool array.
    ///
    /// Returns a Vec<bool> of size 28 where sums[i] = true if i is in the allowed set.
    ///
    /// C# Reference: Convolution.cs ConvolutionRule.Load() lines 152-164
    pub fn parse_sum_intervals(s: &str) -> Vec<bool> {
        let mut sums = vec![false; 28];

        for part in s.split(',') {
            let part = part.trim();
            if part.contains("..") {
                // Range like "5..8"
                let bounds: Vec<&str> = part.split("..").collect();
                if bounds.len() == 2 {
                    if let (Ok(min), Ok(max)) =
                        (bounds[0].parse::<usize>(), bounds[1].parse::<usize>())
                    {
                        for i in min..=max {
                            if i < 28 {
                                sums[i] = true;
                            }
                        }
                    }
                }
            } else {
                // Single value like "3"
                if let Ok(val) = part.parse::<usize>() {
                    if val < 28 {
                        sums[val] = true;
                    }
                }
            }
        }

        sums
    }
}

/// Convolution node for cellular automata rules.
///
/// Applies rules based on counting neighbors of specific colors.
/// Each step:
/// 1. Compute sumfield - for each cell, count neighbors of each color
/// 2. For each cell, check rules in order
/// 3. If a rule matches (input matches and sum is in allowed range), apply it
///
/// C# Reference: Convolution.cs ConvolutionNode
#[derive(Debug)]
pub struct ConvolutionNode {
    /// Rules to apply (checked in order)
    pub rules: Vec<ConvolutionRule>,
    /// Neighborhood kernel (determines which neighbors to count)
    pub kernel: Vec<i32>,
    /// Whether the grid wraps around (toroidal)
    pub periodic: bool,
    /// Current step counter
    pub counter: usize,
    /// Maximum steps (0 = unlimited)
    pub steps: usize,
    /// Sumfield - for each cell, count of each color in neighborhood
    /// sumfield[cell_index][color] = count
    sumfield: Vec<Vec<i32>>,
    /// Number of colors in the grid
    num_colors: usize,
    /// Whether this is a 2D grid (mz == 1)
    is_2d: bool,
}

impl ConvolutionNode {
    /// Create a new ConvolutionNode.
    ///
    /// # Arguments
    /// * `rules` - Rules to apply
    /// * `kernel` - Neighborhood kernel (9 elements for 2D, 27 for 3D)
    /// * `periodic` - Whether grid wraps around
    /// * `grid_size` - Total number of cells in grid
    /// * `num_colors` - Number of distinct colors (grid.c)
    /// * `is_2d` - Whether this is a 2D grid
    pub fn new(
        rules: Vec<ConvolutionRule>,
        kernel: Vec<i32>,
        periodic: bool,
        grid_size: usize,
        num_colors: usize,
        is_2d: bool,
    ) -> Self {
        // Initialize sumfield with zeros
        let sumfield = vec![vec![0i32; num_colors]; grid_size];

        Self {
            rules,
            kernel,
            periodic,
            counter: 0,
            steps: 0,
            sumfield,
            num_colors,
            is_2d,
        }
    }

    /// Create with a named 2D kernel.
    pub fn with_2d_kernel(
        rules: Vec<ConvolutionRule>,
        kernel_name: &str,
        periodic: bool,
        grid_size: usize,
        num_colors: usize,
    ) -> Option<Self> {
        let kernel = match kernel_name {
            "VonNeumann" => kernels_2d::VON_NEUMANN.to_vec(),
            "Moore" => kernels_2d::MOORE.to_vec(),
            _ => return None,
        };
        Some(Self::new(
            rules, kernel, periodic, grid_size, num_colors, true,
        ))
    }

    /// Create with a named 3D kernel.
    pub fn with_3d_kernel(
        rules: Vec<ConvolutionRule>,
        kernel_name: &str,
        periodic: bool,
        grid_size: usize,
        num_colors: usize,
    ) -> Option<Self> {
        let kernel = match kernel_name {
            "VonNeumann" => kernels_3d::VON_NEUMANN.to_vec(),
            "NoCorners" => kernels_3d::NO_CORNERS.to_vec(),
            _ => return None,
        };
        Some(Self::new(
            rules, kernel, periodic, grid_size, num_colors, false,
        ))
    }

    /// Set maximum steps.
    pub fn with_steps(mut self, steps: usize) -> Self {
        self.steps = steps;
        self
    }

    /// Compute sumfield for all cells.
    ///
    /// For each cell, count how many neighbors of each color are present.
    ///
    /// C# Reference: Convolution.cs Go() lines 58-106
    fn compute_sumfield(&mut self, ctx: &ExecutionContext) {
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;
        let mz = ctx.grid.mz;
        let state = &ctx.grid.state;

        // Clear sumfield
        for sums in &mut self.sumfield {
            sums.fill(0);
        }

        if self.is_2d {
            // 2D case: 3x3 kernel
            for y in 0..my {
                for x in 0..mx {
                    let cell_idx = x + y * mx;
                    let sums = &mut self.sumfield[cell_idx];

                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let kernel_idx = (dx + 1) as usize + (dy + 1) as usize * 3;
                            let kernel_val = self.kernel[kernel_idx];
                            if kernel_val == 0 {
                                continue;
                            }

                            let mut sx = x as i32 + dx;
                            let mut sy = y as i32 + dy;

                            if self.periodic {
                                if sx < 0 {
                                    sx += mx as i32;
                                } else if sx >= mx as i32 {
                                    sx -= mx as i32;
                                }
                                if sy < 0 {
                                    sy += my as i32;
                                } else if sy >= my as i32 {
                                    sy -= my as i32;
                                }
                            } else if sx < 0 || sy < 0 || sx >= mx as i32 || sy >= my as i32 {
                                continue;
                            }

                            let neighbor_idx = sx as usize + sy as usize * mx;
                            let neighbor_color = state[neighbor_idx] as usize;
                            if neighbor_color < self.num_colors {
                                sums[neighbor_color] += kernel_val;
                            }
                        }
                    }
                }
            }
        } else {
            // 3D case: 3x3x3 kernel
            for z in 0..mz {
                for y in 0..my {
                    for x in 0..mx {
                        let cell_idx = x + y * mx + z * mx * my;
                        let sums = &mut self.sumfield[cell_idx];

                        for dz in -1i32..=1 {
                            for dy in -1i32..=1 {
                                for dx in -1i32..=1 {
                                    let kernel_idx = (dx + 1) as usize
                                        + (dy + 1) as usize * 3
                                        + (dz + 1) as usize * 9;
                                    let kernel_val = self.kernel[kernel_idx];
                                    if kernel_val == 0 {
                                        continue;
                                    }

                                    let mut sx = x as i32 + dx;
                                    let mut sy = y as i32 + dy;
                                    let mut sz = z as i32 + dz;

                                    if self.periodic {
                                        if sx < 0 {
                                            sx += mx as i32;
                                        } else if sx >= mx as i32 {
                                            sx -= mx as i32;
                                        }
                                        if sy < 0 {
                                            sy += my as i32;
                                        } else if sy >= my as i32 {
                                            sy -= my as i32;
                                        }
                                        if sz < 0 {
                                            sz += mz as i32;
                                        } else if sz >= mz as i32 {
                                            sz -= mz as i32;
                                        }
                                    } else if sx < 0
                                        || sy < 0
                                        || sz < 0
                                        || sx >= mx as i32
                                        || sy >= my as i32
                                        || sz >= mz as i32
                                    {
                                        continue;
                                    }

                                    let neighbor_idx =
                                        sx as usize + sy as usize * mx + sz as usize * mx * my;
                                    let neighbor_color = state[neighbor_idx] as usize;
                                    if neighbor_color < self.num_colors {
                                        sums[neighbor_color] += kernel_val;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check if a rule matches at a given cell.
    fn rule_matches(&self, rule: &ConvolutionRule, cell_idx: usize, rng: &mut dyn MjRng) -> bool {
        // Check probability
        if rule.p < 1.0 && rng.next_double() >= rule.p {
            return false;
        }

        // Check sum constraint if present
        if let Some(ref allowed_sums) = rule.sums {
            let sums = &self.sumfield[cell_idx];

            // Sum up counts for the specified values
            let mut total = 0i32;
            for &val in &rule.values {
                if (val as usize) < sums.len() {
                    total += sums[val as usize];
                }
            }

            // Check if this sum is allowed
            let total_usize = total as usize;
            if total_usize >= allowed_sums.len() || !allowed_sums[total_usize] {
                return false;
            }
        }

        true
    }
}

impl Node for ConvolutionNode {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // Check step limit
        if self.steps > 0 && self.counter >= self.steps {
            return false;
        }

        // Compute neighbor sums for all cells
        self.compute_sumfield(ctx);

        // Apply rules
        let mut change = false;

        for i in 0..self.sumfield.len() {
            let input = ctx.grid.state[i];

            for rule in &self.rules {
                // Check if input matches
                if input != rule.input {
                    continue;
                }

                // Check if output would be different
                if rule.output == ctx.grid.state[i] {
                    continue;
                }

                // Check rule constraints
                if self.rule_matches(rule, i, ctx.random) {
                    ctx.grid.state[i] = rule.output;
                    change = true;
                    break; // Only first matching rule applies
                }
            }
        }

        self.counter += 1;
        change
    }

    fn reset(&mut self) {
        self.counter = 0;
        // Clear sumfield
        for sums in &mut self.sumfield {
            sums.fill(0);
        }
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
    fn test_convolution_rule_parse_sum_intervals() {
        // Single value
        let sums = ConvolutionRule::parse_sum_intervals("3");
        assert!(sums[3]);
        assert!(!sums[2]);
        assert!(!sums[4]);

        // Range
        let sums = ConvolutionRule::parse_sum_intervals("5..8");
        assert!(!sums[4]);
        assert!(sums[5]);
        assert!(sums[6]);
        assert!(sums[7]);
        assert!(sums[8]);
        assert!(!sums[9]);

        // Multiple parts
        let sums = ConvolutionRule::parse_sum_intervals("2,5..7");
        assert!(sums[2]);
        assert!(!sums[3]);
        assert!(!sums[4]);
        assert!(sums[5]);
        assert!(sums[6]);
        assert!(sums[7]);
        assert!(!sums[8]);
    }

    #[test]
    fn test_convolution_simple_rule() {
        // Grid with 2 colors: 0 (D) and 1 (A)
        let mut grid = MjGrid::with_values(3, 3, 1, "DA");

        // Set center cell to A (1), rest are D (0)
        grid.state[4] = 1; // center

        // Rule: A -> D (always, no sum constraint)
        let rule = ConvolutionRule::new(1, 0);
        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // Run one step
        let changed = node.go(&mut ctx);
        assert!(changed);

        // Center should now be D (0)
        assert_eq!(ctx.grid.state[4], 0);
    }

    #[test]
    fn test_convolution_sum_constraint() {
        // 3x3 grid, all D (0) except center is A (1)
        let mut grid = MjGrid::with_values(3, 3, 1, "DA");
        grid.state[4] = 1; // center is A

        // Rule: A -> D if neighbor count of A is 0
        // (center has 0 A neighbors, so this should fire)
        let sums = ConvolutionRule::parse_sum_intervals("0");
        let rule = ConvolutionRule::with_sums(1, 0, vec![1], sums);

        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        let changed = node.go(&mut ctx);
        assert!(changed);
        assert_eq!(ctx.grid.state[4], 0);
    }

    #[test]
    fn test_convolution_sum_constraint_not_met() {
        // 3x3 grid with center A, and one neighbor A
        let mut grid = MjGrid::with_values(3, 3, 1, "DA");
        grid.state[4] = 1; // center is A
        grid.state[1] = 1; // top neighbor is A

        // Rule: A -> D if neighbor count of A is 0
        // (center has 1 A neighbor, so this should NOT fire)
        let sums = ConvolutionRule::parse_sum_intervals("0");
        let rule = ConvolutionRule::with_sums(1, 0, vec![1], sums);

        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // Center should NOT change (sum constraint not met)
        let _changed = node.go(&mut ctx);
        assert_eq!(ctx.grid.state[4], 1); // still A
    }

    #[test]
    fn test_convolution_moore_neighbor_count() {
        // 3x3 grid, all A (1)
        let mut grid = MjGrid::with_values(3, 3, 1, "DA");
        for i in 0..9 {
            grid.state[i] = 1;
        }

        // Rule: A -> D if neighbor count of A is exactly 8 (all neighbors)
        // Only center has 8 neighbors
        let sums = ConvolutionRule::parse_sum_intervals("8");
        let rule = ConvolutionRule::with_sums(1, 0, vec![1], sums);

        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        let changed = node.go(&mut ctx);
        assert!(changed);

        // Only center should have changed
        assert_eq!(ctx.grid.state[4], 0); // center is now D
                                          // Corners have 3 neighbors, edges have 5 - they should remain A
        assert_eq!(ctx.grid.state[0], 1); // corner
        assert_eq!(ctx.grid.state[1], 1); // edge
    }

    #[test]
    fn test_convolution_von_neumann_kernel() {
        // 3x3 grid, all A (1)
        let mut grid = MjGrid::with_values(3, 3, 1, "DA");
        for i in 0..9 {
            grid.state[i] = 1;
        }

        // Rule: A -> D if VonNeumann neighbor count of A is exactly 4
        // Only center has 4 orthogonal neighbors
        let sums = ConvolutionRule::parse_sum_intervals("4");
        let rule = ConvolutionRule::with_sums(1, 0, vec![1], sums);

        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule],
            "VonNeumann",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        let changed = node.go(&mut ctx);
        assert!(changed);

        // Only center should have changed (4 VonNeumann neighbors)
        assert_eq!(ctx.grid.state[4], 0);
        // Edges have 2-3 VN neighbors, corners have 2
        assert_eq!(ctx.grid.state[0], 1); // corner (2 neighbors)
        assert_eq!(ctx.grid.state[1], 1); // edge (3 neighbors, but top edge has only 2)
    }

    #[test]
    fn test_convolution_periodic() {
        // 3x3 grid with A in top-left corner
        let mut grid = MjGrid::with_values(3, 3, 1, "DA");
        grid.state[0] = 1; // top-left is A

        // With periodic boundary, corner now wraps around
        // Rule: A -> D if neighbor count is exactly 0
        // With periodic, corner has 8 neighbors (all D), so sum is 0
        let sums = ConvolutionRule::parse_sum_intervals("0");
        let rule = ConvolutionRule::with_sums(1, 0, vec![1], sums);

        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule],
            "Moore",
            true, // periodic!
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        let changed = node.go(&mut ctx);
        assert!(changed);
        assert_eq!(ctx.grid.state[0], 0); // corner changed to D
    }

    #[test]
    fn test_convolution_step_limit() {
        let mut grid = MjGrid::with_values(3, 3, 1, "DA");
        grid.state[4] = 1;

        let rule = ConvolutionRule::new(1, 0);
        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap()
        .with_steps(1);

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // First step should work
        assert!(node.go(&mut ctx));

        // Second step should return false (step limit reached)
        ctx.grid.state[4] = 1; // reset
        assert!(!node.go(&mut ctx));
    }

    #[test]
    fn test_convolution_game_of_life_rules() {
        // Classic Game of Life:
        // - Live cell with 2-3 live neighbors survives
        // - Dead cell with exactly 3 live neighbors becomes alive
        // - All other live cells die

        // Use D=dead(0), A=alive(1)
        let grid = MjGrid::with_values(5, 5, 1, "DA");

        // Rule 1: A -> D if neighbors not in 2..3 (death rule)
        let death_sums = {
            let mut s = vec![true; 28];
            s[2] = false;
            s[3] = false;
            s
        };
        let death_rule = ConvolutionRule::with_sums(1, 0, vec![1], death_sums);

        // Rule 2: D -> A if neighbors exactly 3 (birth rule)
        let birth_sums = ConvolutionRule::parse_sum_intervals("3");
        let birth_rule = ConvolutionRule::with_sums(0, 1, vec![1], birth_sums);

        // Note: In real GoL, we need to apply both rules simultaneously.
        // This simple test just checks the rules can be constructed.
        let _node = ConvolutionNode::with_2d_kernel(
            vec![death_rule, birth_rule],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();
    }

    #[test]
    fn test_convolution_reset() {
        let mut grid = MjGrid::with_values(3, 3, 1, "DA");
        let rule = ConvolutionRule::new(1, 0);
        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap()
        .with_steps(2);

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // Run two steps
        ctx.grid.state[4] = 1;
        node.go(&mut ctx);
        ctx.grid.state[4] = 1;
        node.go(&mut ctx);

        // Should be at step limit
        assert_eq!(node.counter, 2);

        // Reset
        node.reset();
        assert_eq!(node.counter, 0);

        // Should be able to run again
        ctx.grid.state[4] = 1;
        assert!(node.go(&mut ctx));
    }

    #[test]
    fn test_convolution_3d_von_neumann() {
        // 3x3x3 grid, all A (1)
        let mut grid = MjGrid::with_values(3, 3, 3, "DA");
        for i in 0..27 {
            grid.state[i] = 1;
        }

        // Rule: A -> D if VonNeumann neighbor count is exactly 6
        // Only center has 6 orthogonal neighbors in 3D
        let sums = ConvolutionRule::parse_sum_intervals("6");
        let rule = ConvolutionRule::with_sums(1, 0, vec![1], sums);

        let mut node = ConvolutionNode::with_3d_kernel(
            vec![rule],
            "VonNeumann",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        let changed = node.go(&mut ctx);
        assert!(changed);

        // Center of 3x3x3 is at index 13 (1 + 1*3 + 1*9)
        assert_eq!(ctx.grid.state[13], 0); // center changed
    }

    #[test]
    fn test_convolution_neighbor_count_verification() {
        // Verify that the neighbor counting is correct by checking
        // that a cell with exactly N neighbors triggers a rule with sum=N

        // Create a known configuration:
        // D A D
        // A D A
        // D A D
        // Center (index 4) has 4 A neighbors (the corners are D)
        let mut grid = MjGrid::with_values(3, 3, 1, "DA");
        grid.state[1] = 1; // top
        grid.state[3] = 1; // left
        grid.state[5] = 1; // right
        grid.state[7] = 1; // bottom
        grid.state[4] = 0; // center is D

        // Rule: D -> A if exactly 4 A neighbors
        let sums = ConvolutionRule::parse_sum_intervals("4");
        let rule = ConvolutionRule::with_sums(0, 1, vec![1], sums);

        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // Run
        let changed = node.go(&mut ctx);
        assert!(
            changed,
            "Rule should fire for cell with exactly 4 A neighbors"
        );
        assert_eq!(ctx.grid.state[4], 1, "Center should change from D to A");

        // Verify corners didn't change (they have 2 neighbors each)
        assert_eq!(ctx.grid.state[0], 0, "Corner should remain D (2 neighbors)");
        assert_eq!(ctx.grid.state[2], 0, "Corner should remain D (2 neighbors)");
        assert_eq!(ctx.grid.state[6], 0, "Corner should remain D (2 neighbors)");
        assert_eq!(ctx.grid.state[8], 0, "Corner should remain D (2 neighbors)");
    }

    #[test]
    fn test_convolution_multiple_rules_order() {
        // Test that rules are applied in order and first matching rule wins

        let mut grid = MjGrid::with_values(3, 3, 1, "DAX");
        grid.state[4] = 0; // center is D

        // Rule 1: D -> A (always)
        let rule1 = ConvolutionRule::new(0, 1);
        // Rule 2: D -> X (always) - should never fire because rule1 matches first
        let rule2 = ConvolutionRule::new(0, 2);

        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule1, rule2],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        node.go(&mut ctx);

        // Center should be A (1), not X (2)
        assert_eq!(
            ctx.grid.state[4], 1,
            "First matching rule should win: expected A (1), got {}",
            ctx.grid.state[4]
        );
    }

    #[test]
    fn test_convolution_cave_like_rules() {
        // Test rules similar to Cave.xml:
        // - Rule 1: A -> D if 5-8 D neighbors
        // - Rule 2: D -> A if 6-8 A neighbors
        // This should create cave-like patterns

        let mut grid = MjGrid::with_values(8, 8, 1, "DA");

        // Initialize with random pattern
        let mut rng = StdRandom::from_u64_seed(42);
        for i in 0..grid.state.len() {
            grid.state[i] = if rng.next_bool() { 0 } else { 1 };
        }

        // Cave rules
        let rule1_sums = ConvolutionRule::parse_sum_intervals("5..8");
        let rule1 = ConvolutionRule::with_sums(1, 0, vec![0], rule1_sums); // A -> D if many D neighbors

        let rule2_sums = ConvolutionRule::parse_sum_intervals("6..8");
        let rule2 = ConvolutionRule::with_sums(0, 1, vec![1], rule2_sums); // D -> A if many A neighbors

        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule1, rule2],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap()
        .with_steps(10);

        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // Run multiple steps
        let mut total_changes = 0;
        while node.go(&mut ctx) {
            total_changes += 1;
        }

        // Should have made changes
        assert!(
            total_changes > 0,
            "Cave rules should cause changes over 10 steps"
        );

        // Grid should have both D and A values (not completely uniform)
        let d_count = ctx.grid.state.iter().filter(|&&v| v == 0).count();
        let a_count = ctx.grid.state.iter().filter(|&&v| v == 1).count();

        assert!(d_count > 0, "Should have some D cells remaining");
        assert!(a_count > 0, "Should have some A cells remaining");
    }

    #[test]
    fn test_convolution_sumfield_correctness() {
        // Directly test that sumfield computes correct values
        // by setting up a known pattern and verifying neighbor counts

        let mut grid = MjGrid::with_values(3, 3, 1, "DA");
        // Pattern:
        // A A A
        // A D A
        // A A A
        // All A except center
        for i in 0..9 {
            grid.state[i] = 1;
        }
        grid.state[4] = 0; // center is D

        let rule = ConvolutionRule::new(0, 1); // placeholder
        let mut node = ConvolutionNode::with_2d_kernel(
            vec![rule],
            "Moore",
            false,
            grid.state.len(),
            grid.c as usize,
        )
        .unwrap();

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = create_test_ctx(&mut grid, &mut rng);

        // Compute sumfield
        node.compute_sumfield(&mut ctx);

        // Verify center has 8 A neighbors
        let center_sums = &node.sumfield[4];
        assert_eq!(
            center_sums[1], 8,
            "Center should have 8 A neighbors, got {}",
            center_sums[1]
        );
        assert_eq!(
            center_sums[0], 0,
            "Center should have 0 D neighbors, got {}",
            center_sums[0]
        );

        // Verify corner (index 0) has 3 A neighbors (non-periodic)
        let corner_sums = &node.sumfield[0];
        assert_eq!(
            corner_sums[1], 2,
            "Top-left corner should have 2 A neighbors (excluding out-of-bounds), got {}",
            corner_sums[1]
        );

        // Edge (index 1) has 5 neighbors in Moore, but only 4 are in-bounds for top edge
        let edge_sums = &node.sumfield[1];
        // Top edge: neighbors are [0], [2], [3], [4], [5]
        // That's 5 neighbors: 4 are A (indices 0,2,3,5) and 1 is D (index 4)
        assert_eq!(
            edge_sums[1], 4,
            "Top edge should have 4 A neighbors, got {} (D neighbors: {})",
            edge_sums[1], edge_sums[0]
        );
    }
}
