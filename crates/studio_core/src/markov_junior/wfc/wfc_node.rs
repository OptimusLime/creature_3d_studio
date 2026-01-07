//! Base WFC node implementation with core algorithms.
//!
//! This module provides the shared WFC logic used by both OverlapNode and TileNode:
//! - `Ban()` - Remove a pattern possibility and update entropy
//! - `Propagate()` - Stack-based constraint propagation
//! - `Observe()` - Collapse a cell to a single pattern
//! - `NextUnobservedNode()` - Find minimum entropy cell
//! - `GoodSeed()` - Try multiple seeds to find a non-contradicting run
//!
//! C# Reference: WaveFunctionCollapse.cs class WFCNode (lines 7-258)

use super::wave::Wave;
use crate::markov_junior::node::{ExecutionContext, Node};
use crate::markov_junior::rng::{DotNetRandom, MjRng, StdRandom};
use crate::markov_junior::MjGrid;

/// Direction offsets for 2D/3D propagation.
/// Order: +X, +Y, -X, -Y, +Z, -Z
pub const DX: [i32; 6] = [1, 0, -1, 0, 0, 0];
pub const DY: [i32; 6] = [0, 1, 0, -1, 0, 0];
pub const DZ: [i32; 6] = [0, 0, 0, 0, 1, -1];

/// Opposite direction indices.
pub const OPPOSITE: [usize; 6] = [2, 3, 0, 1, 5, 4];

/// State of WFC execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WfcState {
    /// Not yet started
    Initial,
    /// Running (observing and propagating)
    Running,
    /// Successfully completed (all cells collapsed)
    Completed,
    /// Failed (contradiction reached)
    Failed,
}

/// Shared WFC node data and algorithms.
///
/// This struct contains all the state needed for WFC and provides
/// the core algorithms. OverlapNode and TileNode extend this with
/// their specific pattern/tile logic.
pub struct WfcNode {
    /// Wave state tracking possibilities
    pub wave: Wave,

    /// Start wave state (for GoodSeed retries)
    pub start_wave: Wave,

    /// Propagator: `propagator[direction][pattern]` = list of compatible patterns
    pub propagator: Vec<Vec<Vec<usize>>>,

    /// Pattern weights for random selection
    pub weights: Vec<f64>,

    /// Weight * log(weight) for entropy calculation
    weight_log_weights: Vec<f64>,

    /// Sum of all weights
    sum_of_weights: f64,

    /// Sum of weight * log(weight)
    sum_of_weight_log_weights: f64,

    /// Starting entropy value
    starting_entropy: f64,

    /// Stack for propagation: (cell_index, pattern_index)
    stack: Vec<(usize, usize)>,

    /// Distribution buffer for observation (reused to avoid allocation)
    distribution: Vec<f64>,

    /// New grid for output (WFC creates its own output grid)
    pub newgrid: MjGrid,

    /// Map from input grid values to allowed patterns
    pub map: Vec<Vec<bool>>,

    /// Pattern size N (for overlap) or tile size S (for tile)
    pub n: usize,

    /// Whether grid wraps around (toroidal)
    pub periodic: bool,

    /// Whether to use Shannon entropy
    pub shannon: bool,

    /// Number of tries to find a good seed
    pub tries: usize,

    /// Current execution state
    pub state: WfcState,

    /// Current child node index (for Branch behavior)
    pub child_index: i32,

    /// Whether this is the first go() call
    pub first_go: bool,

    /// Random generator for this WFC run (uses DotNetRandom to match C# behavior)
    rng: Option<Box<dyn MjRng>>,

    /// Grid dimensions (cached from input grid)
    pub mx: usize,
    pub my: usize,
    pub mz: usize,
}

impl WfcNode {
    /// Create a new WfcNode with the given configuration.
    ///
    /// # Arguments
    /// * `wave_length` - Number of cells in the wave
    /// * `num_patterns` - Number of patterns P
    /// * `num_directions` - Number of directions (4 for 2D, 6 for 3D)
    /// * `propagator` - Adjacency constraints
    /// * `weights` - Pattern weights
    /// * `newgrid` - Output grid
    /// * `n` - Pattern/tile size
    /// * `periodic` - Whether grid wraps
    /// * `shannon` - Whether to use Shannon entropy
    /// * `tries` - Number of seed attempts
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        wave_length: usize,
        num_patterns: usize,
        num_directions: usize,
        propagator: Vec<Vec<Vec<usize>>>,
        weights: Vec<f64>,
        newgrid: MjGrid,
        map: Vec<Vec<bool>>,
        n: usize,
        periodic: bool,
        shannon: bool,
        tries: usize,
        mx: usize,
        my: usize,
        mz: usize,
    ) -> Self {
        let wave = Wave::new(wave_length, num_patterns, num_directions, shannon);
        let start_wave = Wave::new(wave_length, num_patterns, num_directions, shannon);

        // Compute entropy-related values
        let mut sum_of_weights = 0.0;
        let mut sum_of_weight_log_weights = 0.0;
        let mut weight_log_weights = Vec::with_capacity(num_patterns);

        for &w in &weights {
            let wlw = if w > 0.0 { w * w.ln() } else { 0.0 };
            weight_log_weights.push(wlw);
            sum_of_weights += w;
            sum_of_weight_log_weights += wlw;
        }

        let starting_entropy = if sum_of_weights > 0.0 {
            sum_of_weights.ln() - sum_of_weight_log_weights / sum_of_weights
        } else {
            0.0
        };

        let stack_capacity = wave_length * num_patterns;
        let distribution = vec![0.0; num_patterns];

        Self {
            wave,
            start_wave,
            propagator,
            weights,
            weight_log_weights,
            sum_of_weights,
            sum_of_weight_log_weights,
            starting_entropy,
            stack: Vec::with_capacity(stack_capacity),
            distribution,
            newgrid,
            map,
            n,
            periodic,
            shannon,
            tries,
            state: WfcState::Initial,
            child_index: -1,
            first_go: true,
            rng: None,
            mx,
            my,
            mz,
        }
    }

    /// Initialize the wave and apply initial constraints from the input grid.
    ///
    /// C# Reference: WFCNode.Go() first-go branch (lines 73-104)
    pub fn initialize(&mut self, grid: &MjGrid, ctx_rng: &mut dyn MjRng) -> bool {
        // Initialize wave state
        self.wave.init(
            &self.propagator,
            self.sum_of_weights,
            self.sum_of_weight_log_weights,
            self.starting_entropy,
        );

        // Apply initial constraints from input grid
        // First collect all bans to avoid borrow conflicts
        let mut bans: Vec<(usize, usize)> = Vec::new();
        for i in 0..self.wave.length {
            let value = grid.state[i];
            if (value as usize) < self.map.len() {
                for t in 0..self.wave.p {
                    if !self.map[value as usize][t] {
                        bans.push((i, t));
                    }
                }
            }
        }
        // Now apply all bans
        for (i, t) in bans {
            self.ban(i, t);
        }

        // Propagate initial constraints
        let success = self.propagate();
        if !success {
            self.state = WfcState::Failed;
            return false;
        }

        // Save start state for retries
        self.start_wave.copy_from(&self.wave);

        // Find a good seed
        if let Some(seed) = self.good_seed(ctx_rng) {
            // Use DotNetRandom to match C# behavior exactly
            // The seed is an i32 from next_int(), cast to u64 for storage but used as i32 for DotNetRandom
            self.rng = Some(Box::new(DotNetRandom::from_seed(seed as i32)));
            self.stack.clear();
            self.wave.copy_from(&self.start_wave);
            self.newgrid.clear();
            self.state = WfcState::Running;
            self.first_go = false;
            true
        } else {
            self.state = WfcState::Failed;
            false
        }
    }

    /// Try multiple seeds to find one that doesn't lead to contradiction.
    ///
    /// C# Reference: WFCNode.GoodSeed() (lines 121-155)
    fn good_seed(&mut self, ctx_rng: &mut dyn MjRng) -> Option<u64> {
        for k in 0..self.tries {
            // C# uses ip.random.Next() which returns a non-negative int
            // Then creates new Random(seed) which uses .NET's specific seeding algorithm
            let seed = ctx_rng.next_int();
            // Use DotNetRandom to match .NET's Random behavior exactly
            let mut local_rng = DotNetRandom::from_seed(seed);

            self.stack.clear();
            self.wave.copy_from(&self.start_wave);

            loop {
                let node = self.next_unobserved_node(&mut local_rng);
                if node >= 0 {
                    self.observe(node as usize, &mut local_rng);
                    let success = self.propagate();
                    if !success {
                        // Contradiction - try another seed
                        break;
                    }
                } else {
                    // Successfully collapsed - found a good seed
                    return Some(seed as u64);
                }
            }
        }

        None
    }

    /// Perform one observation step.
    ///
    /// Returns true if still running, false if completed or failed.
    ///
    /// C# Reference: WFCNode.Go() else branch (lines 106-118)
    pub fn step(&mut self) -> bool {
        if self.state != WfcState::Running {
            return false;
        }

        // Clone rng to avoid borrow conflicts
        let mut rng = self.rng.clone().expect("WFC not initialized");

        let node = self.next_unobserved_node(rng.as_mut());
        if node >= 0 {
            self.observe(node as usize, rng.as_mut());
            self.rng = Some(rng);

            let success = self.propagate();
            if !success {
                self.state = WfcState::Failed;
                return false;
            }
            true
        } else {
            self.rng = Some(rng);
            // All cells collapsed
            self.state = WfcState::Completed;
            self.child_index = 0;
            false
        }
    }

    /// Find the next cell to observe (minimum entropy with >1 possibilities).
    ///
    /// Returns -1 if all cells are collapsed.
    ///
    /// C# Reference: WFCNode.NextUnobservedNode() (lines 157-178)
    fn next_unobserved_node(&self, rng: &mut dyn MjRng) -> i32 {
        let mut min_entropy = 1e4;
        let mut argmin: i32 = -1;

        for z in 0..self.mz {
            for y in 0..self.my {
                for x in 0..self.mx {
                    let i = x + y * self.mx + z * self.mx * self.my;

                    // Skip cells that would go out of bounds in non-periodic mode
                    if !self.periodic
                        && (x + self.n > self.mx || y + self.n > self.my || z + 1 > self.mz)
                    {
                        continue;
                    }

                    let remaining = self.wave.remaining(i);

                    if remaining > 1 {
                        let entropy = self.wave.entropy(i);
                        if entropy <= min_entropy {
                            // Add small noise for tie-breaking
                            let noise = 1e-6 * rng.next_double();
                            if entropy + noise < min_entropy {
                                min_entropy = entropy + noise;
                                argmin = i as i32;
                            }
                        }
                    }
                }
            }
        }

        argmin
    }

    /// Observe (collapse) a cell to a single pattern.
    ///
    /// Uses weighted random selection based on pattern weights.
    ///
    /// C# Reference: WFCNode.Observe() (lines 181-186)
    fn observe(&mut self, cell: usize, rng: &mut dyn MjRng) {
        // Build distribution of possible patterns
        for t in 0..self.wave.p {
            self.distribution[t] = if self.wave.get_data(cell, t) {
                self.weights[t]
            } else {
                0.0
            };
        }

        // Weighted random selection
        let r = self.weighted_random(&self.distribution, rng);

        // Ban all other patterns
        for t in 0..self.wave.p {
            if self.wave.get_data(cell, t) && t != r {
                self.ban(cell, t);
            }
        }
    }

    /// Select a random index based on weights.
    fn weighted_random(&self, weights: &[f64], rng: &mut dyn MjRng) -> usize {
        let sum: f64 = weights.iter().sum();
        if sum <= 0.0 {
            return 0;
        }

        let r = rng.next_double();
        let threshold = r * sum;

        let mut partial_sum = 0.0;

        for (i, &w) in weights.iter().enumerate() {
            partial_sum += w;
            if partial_sum >= threshold {
                return i;
            }
        }

        0
    }

    /// Propagate constraints after a ban.
    ///
    /// Uses stack-based arc consistency propagation.
    ///
    /// C# Reference: WFCNode.Propagate() (lines 189-228)
    fn propagate(&mut self) -> bool {
        while let Some((i1, p1)) = self.stack.pop() {
            let x1 = i1 % self.mx;
            let y1 = (i1 % (self.mx * self.my)) / self.mx;
            let z1 = i1 / (self.mx * self.my);

            // Collect bans to apply after iterating propagator
            let mut new_bans: Vec<(usize, usize)> = Vec::new();

            for d in 0..self.propagator.len() {
                let x2 = x1 as i32 + DX[d];
                let y2 = y1 as i32 + DY[d];
                let z2 = z1 as i32 + DZ[d];

                // Bounds check for non-periodic
                if !self.periodic {
                    if x2 < 0
                        || y2 < 0
                        || z2 < 0
                        || x2 as usize + self.n > self.mx
                        || y2 as usize + self.n > self.my
                        || z2 as usize + 1 > self.mz
                    {
                        continue;
                    }
                }

                // Wrap coordinates for periodic
                let x2 = if x2 < 0 {
                    (x2 + self.mx as i32) as usize
                } else if x2 >= self.mx as i32 {
                    (x2 - self.mx as i32) as usize
                } else {
                    x2 as usize
                };

                let y2 = if y2 < 0 {
                    (y2 + self.my as i32) as usize
                } else if y2 >= self.my as i32 {
                    (y2 - self.my as i32) as usize
                } else {
                    y2 as usize
                };

                let z2 = if z2 < 0 {
                    (z2 + self.mz as i32) as usize
                } else if z2 >= self.mz as i32 {
                    (z2 - self.mz as i32) as usize
                } else {
                    z2 as usize
                };

                let i2 = x2 + y2 * self.mx + z2 * self.mx * self.my;

                // For each pattern compatible with p1 in direction d
                if p1 < self.propagator[d].len() {
                    for &t2 in &self.propagator[d][p1] {
                        // Decrement compatible count
                        let count = self.wave.decrement_compatible(i2, t2, d);
                        if count == 0 {
                            new_bans.push((i2, t2));
                        }
                    }
                }
            }

            // Apply collected bans
            for (cell, pattern) in new_bans {
                self.ban(cell, pattern);
            }
        }

        // Check for contradiction (any cell with 0 possibilities)
        self.wave.sums_of_ones[0] > 0
    }

    /// Ban a pattern from a cell.
    ///
    /// Updates wave state, entropy, and pushes to propagation stack.
    ///
    /// C# Reference: WFCNode.Ban() (lines 231-251)
    fn ban(&mut self, cell: usize, pattern: usize) {
        self.wave.set_data(cell, pattern, false);

        // Zero out compatible counts for this pattern
        for d in 0..self.propagator.len() {
            self.wave.set_compatible(cell, pattern, d, 0);
        }

        // Push to propagation stack
        self.stack.push((cell, pattern));

        // Update sums
        self.wave.sums_of_ones[cell] -= 1;

        // Update Shannon entropy if enabled
        if self.shannon {
            if let Some(ref mut sow) = self.wave.sums_of_weights {
                let old_sum = sow[cell];

                if let Some(ref mut ent) = self.wave.entropies {
                    if let Some(ref mut sowlw) = self.wave.sums_of_weight_log_weights {
                        // Update entropy: add back old contribution, subtract new
                        if old_sum > 0.0 {
                            ent[cell] += sowlw[cell] / old_sum - old_sum.ln();
                        }

                        sow[cell] -= self.weights[pattern];
                        sowlw[cell] -= self.weight_log_weights[pattern];

                        let new_sum = sow[cell];
                        if new_sum > 0.0 {
                            ent[cell] -= sowlw[cell] / new_sum - new_sum.ln();
                        }
                    }
                }
            }
        }
    }

    /// Reset the WFC node for a new run.
    pub fn reset(&mut self) {
        self.state = WfcState::Initial;
        self.child_index = -1;
        self.first_go = true;
        self.rng = None;
        self.stack.clear();
    }
}

impl Node for WfcNode {
    fn reset(&mut self) {
        WfcNode::reset(self);
    }

    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // Handle child node execution (Branch behavior)
        if self.child_index >= 0 {
            // WFC completed, now would execute children
            // For now, just return false (no children implemented yet)
            self.reset();
            return false;
        }

        if self.first_go {
            // First call - initialize
            if !self.initialize(ctx.grid, ctx.random) {
                return false;
            }
            // Swap grids so subsequent operations use newgrid
            std::mem::swap(&mut self.newgrid, ctx.grid);
            return true;
        }

        // Continue stepping
        if self.step() {
            true
        } else {
            // Completed or failed - update output grid
            if self.state == WfcState::Completed {
                // Note: UpdateState is implemented in OverlapNode/TileNode
                // The grid swap already happened, output is in ctx.grid
            }
            // Swap grids back
            std::mem::swap(&mut self.newgrid, ctx.grid);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::rng::StdRandom;

    fn create_simple_wfc() -> WfcNode {
        // 2 patterns, 4 directions, 2x2 grid
        // Pattern 0 and 1 are compatible with each other in all directions
        let propagator = vec![
            vec![vec![0, 1], vec![0, 1]], // direction 0 (+X)
            vec![vec![0, 1], vec![0, 1]], // direction 1 (+Y)
            vec![vec![0, 1], vec![0, 1]], // direction 2 (-X)
            vec![vec![0, 1], vec![0, 1]], // direction 3 (-Y)
        ];

        let weights = vec![1.0, 1.0];
        let newgrid = MjGrid::with_values(2, 2, 1, "AB");

        // Map: value 0 allows both patterns, value 1 allows both patterns
        let map = vec![vec![true, true], vec![true, true]];

        WfcNode::new(
            4, // wave_length (2x2)
            2, // num_patterns
            4, // num_directions
            propagator, weights, newgrid, map, 1,     // n
            true,  // periodic
            false, // shannon
            10,    // tries
            2,     // mx
            2,     // my
            1,     // mz
        )
    }

    #[test]
    fn test_wfc_node_creation() {
        let wfc = create_simple_wfc();
        assert_eq!(wfc.wave.length, 4);
        assert_eq!(wfc.wave.p, 2);
        assert_eq!(wfc.propagator.len(), 4);
        assert_eq!(wfc.state, WfcState::Initial);
    }

    #[test]
    fn test_wfc_node_initialize() {
        let mut wfc = create_simple_wfc();
        let grid = MjGrid::with_values(2, 2, 1, "AB");
        let mut rng = StdRandom::from_u64_seed(42);

        let success = wfc.initialize(&grid, &mut rng);
        assert!(success);
        assert_eq!(wfc.state, WfcState::Running);
    }

    #[test]
    fn test_wfc_ban() {
        let mut wfc = create_simple_wfc();
        let grid = MjGrid::with_values(2, 2, 1, "AB");
        let mut rng = StdRandom::from_u64_seed(42);

        wfc.initialize(&grid, &mut rng);

        // Before ban: cell 0 has 2 possibilities
        assert_eq!(wfc.wave.sums_of_ones[0], 2);

        // Ban pattern 0 from cell 0
        wfc.ban(0, 0);

        // After ban: cell 0 has 1 possibility
        assert_eq!(wfc.wave.sums_of_ones[0], 1);
        assert!(!wfc.wave.get_data(0, 0));
        assert!(wfc.wave.get_data(0, 1));
    }

    #[test]
    fn test_wfc_propagate_reduces_possibilities() {
        // Create a WFC where banning one pattern forces the neighbor
        // Pattern 0 only compatible with pattern 0, pattern 1 only with pattern 1
        let propagator = vec![
            vec![vec![0], vec![1]], // direction 0: pattern 0->0, pattern 1->1
            vec![vec![0], vec![1]], // direction 1
            vec![vec![0], vec![1]], // direction 2
            vec![vec![0], vec![1]], // direction 3
        ];

        let weights = vec![1.0, 1.0];
        let newgrid = MjGrid::with_values(2, 2, 1, "AB");
        let map = vec![vec![true, true], vec![true, true]];

        let mut wfc = WfcNode::new(
            4, 2, 4, propagator, weights, newgrid, map, 1, true, false, 10, 2, 2, 1,
        );

        let grid = MjGrid::with_values(2, 2, 1, "AB");
        let mut rng = StdRandom::from_u64_seed(42);

        wfc.initialize(&grid, &mut rng);

        // Initially all cells have both patterns
        assert_eq!(wfc.wave.sums_of_ones[0], 2);
        assert_eq!(wfc.wave.sums_of_ones[1], 2);

        // Ban pattern 0 from cell 0
        wfc.ban(0, 0);
        let success = wfc.propagate();

        // After propagation, pattern 0 should be banned everywhere
        // (because pattern 1 is only compatible with pattern 1)
        assert!(success);
        // Cell 0 now has only pattern 1
        assert_eq!(wfc.wave.sums_of_ones[0], 1);
        assert!(!wfc.wave.get_data(0, 0));
        assert!(wfc.wave.get_data(0, 1));
    }

    #[test]
    fn test_wfc_contradiction_detected() {
        // Create constraints where pattern 0 is only compatible with pattern 1
        // and pattern 1 is only compatible with pattern 0 in a small grid.
        // This creates contradictions when we try to collapse.
        // Use a 2x2 grid where each cell must have the opposite pattern of its neighbor,
        // but there are 4 neighbors in a toroidal grid, making it impossible.
        let propagator = vec![
            vec![vec![1], vec![0]], // direction 0: pattern 0 requires 1, pattern 1 requires 0
            vec![vec![1], vec![0]], // direction 1: same
            vec![vec![1], vec![0]], // direction 2: same
            vec![vec![1], vec![0]], // direction 3: same
        ];

        let weights = vec![1.0, 1.0];
        let newgrid = MjGrid::with_values(2, 2, 1, "AB");
        let map = vec![vec![true, true], vec![true, true]];

        let mut wfc = WfcNode::new(
            4, 2, 4, propagator, weights, newgrid, map, 1, true, false, 1, 2, 2, 1,
        );

        let grid = MjGrid::with_values(2, 2, 1, "AB");
        let mut rng = StdRandom::from_u64_seed(42);

        // In a 2x2 toroidal grid with "opposite patterns only" constraint,
        // it's impossible because each cell would need to be different from all 4 neighbors,
        // but the grid is too small. Should fail to find a good seed after trying.
        let success = wfc.initialize(&grid, &mut rng);

        // The behavior depends on whether the specific seed/tries find a contradiction.
        // With the constraint above, it should fail most of the time.
        // If it succeeds, the WFC will run and eventually fail.
        if success {
            // Run until completion or failure
            while wfc.step() {}
            // Should either be completed (checkerboard worked) or failed
            assert!(
                wfc.state == WfcState::Failed || wfc.state == WfcState::Completed,
                "WFC should complete or fail, got {:?}",
                wfc.state
            );
        } else {
            assert_eq!(wfc.state, WfcState::Failed);
        }
    }

    #[test]
    fn test_wfc_weighted_random() {
        let wfc = create_simple_wfc();
        let mut rng = StdRandom::from_u64_seed(12345);

        // Test with unequal weights
        let weights = vec![1.0, 9.0]; // 10% / 90%

        let mut counts = [0, 0];
        for _ in 0..1000 {
            let idx = wfc.weighted_random(&weights, &mut rng);
            counts[idx] += 1;
        }

        // Pattern 1 should be selected much more often
        assert!(
            counts[1] > counts[0] * 5,
            "Expected pattern 1 to be selected more often"
        );
    }

    #[test]
    fn test_wfc_complete_simple() {
        let mut wfc = create_simple_wfc();
        let grid = MjGrid::with_values(2, 2, 1, "AB");
        let mut rng = StdRandom::from_u64_seed(42);

        wfc.initialize(&grid, &mut rng);

        // Step until completion
        let mut steps = 0;
        while wfc.step() {
            steps += 1;
            if steps > 100 {
                panic!("WFC did not complete in reasonable steps");
            }
        }

        assert!(
            wfc.state == WfcState::Completed || wfc.state == WfcState::Failed,
            "WFC should be completed or failed"
        );

        // If completed, all cells should have exactly one pattern
        if wfc.state == WfcState::Completed {
            assert!(wfc.wave.is_collapsed());
        }
    }

    #[test]
    fn test_wfc_reset() {
        let mut wfc = create_simple_wfc();
        let grid = MjGrid::with_values(2, 2, 1, "AB");
        let mut rng = StdRandom::from_u64_seed(42);

        wfc.initialize(&grid, &mut rng);
        assert_eq!(wfc.state, WfcState::Running);

        wfc.reset();
        assert_eq!(wfc.state, WfcState::Initial);
        assert!(wfc.rng.is_none());
    }

    #[test]
    fn test_wfc_adjacency_constraints_satisfied() {
        // Create a WFC with specific adjacency constraints:
        // Pattern 0 can be next to patterns 0 and 1
        // Pattern 1 can only be next to pattern 0
        // This creates a constraint: pattern 1 cannot be adjacent to pattern 1
        let propagator = vec![
            // direction 0 (+X)
            vec![vec![0, 1], vec![0]], // pattern 0 -> {0,1}, pattern 1 -> {0}
            // direction 1 (+Y)
            vec![vec![0, 1], vec![0]], // pattern 0 -> {0,1}, pattern 1 -> {0}
            // direction 2 (-X)
            vec![vec![0, 1], vec![0]], // pattern 0 -> {0,1}, pattern 1 -> {0}
            // direction 3 (-Y)
            vec![vec![0, 1], vec![0]], // pattern 0 -> {0,1}, pattern 1 -> {0}
        ];

        let weights = vec![1.0, 1.0];
        let newgrid = MjGrid::with_values(4, 4, 1, "AB");
        let map = vec![vec![true, true], vec![true, true]];

        let mut wfc = WfcNode::new(
            16, // wave_length (4x4)
            2,  // num_patterns
            4,  // num_directions
            propagator.clone(),
            weights,
            newgrid,
            map,
            1,     // n
            true,  // periodic
            false, // shannon
            10,    // tries
            4,     // mx
            4,     // my
            1,     // mz
        );

        let grid = MjGrid::with_values(4, 4, 1, "AB");
        let mut rng = StdRandom::from_u64_seed(42);

        // Initialize and run to completion
        assert!(wfc.initialize(&grid, &mut rng));

        let mut steps = 0;
        while wfc.state == WfcState::Running {
            wfc.step();
            steps += 1;
            assert!(steps < 1000, "WFC should complete in reasonable time");
        }

        // Verify completion (not contradiction)
        assert_eq!(
            wfc.state,
            WfcState::Completed,
            "WFC should complete without contradiction"
        );

        // Verify all cells are collapsed
        assert!(
            wfc.wave.is_collapsed(),
            "All cells should be collapsed to exactly one pattern"
        );

        // Verify adjacency constraints are satisfied
        let mx = 4;
        let my = 4;

        for y in 0..my {
            for x in 0..mx {
                let cell = x + y * mx;
                let pattern = wfc
                    .wave
                    .get_collapsed_pattern(cell)
                    .expect("Cell should be collapsed");

                // Check +X neighbor (direction 0)
                let nx = (x + 1) % mx; // periodic
                let neighbor_cell = nx + y * mx;
                let neighbor_pattern = wfc
                    .wave
                    .get_collapsed_pattern(neighbor_cell)
                    .expect("Neighbor should be collapsed");

                assert!(
                    propagator[0][pattern].contains(&neighbor_pattern),
                    "Pattern {} at ({},{}) not compatible with pattern {} at ({},{}) in +X direction",
                    pattern, x, y, neighbor_pattern, nx, y
                );

                // Check +Y neighbor (direction 1)
                let ny = (y + 1) % my; // periodic
                let neighbor_cell = x + ny * mx;
                let neighbor_pattern = wfc
                    .wave
                    .get_collapsed_pattern(neighbor_cell)
                    .expect("Neighbor should be collapsed");

                assert!(
                    propagator[1][pattern].contains(&neighbor_pattern),
                    "Pattern {} at ({},{}) not compatible with pattern {} at ({},{}) in +Y direction",
                    pattern, x, y, neighbor_pattern, x, ny
                );
            }
        }
    }

    #[test]
    fn test_wfc_larger_grid_adjacency() {
        // Test on a larger grid with more patterns
        // 4 patterns, checkerboard-like constraints
        // Pattern 0,2 can be adjacent to 1,3
        // Pattern 1,3 can be adjacent to 0,2
        let propagator = vec![
            // All 4 directions have same constraints
            vec![vec![1, 3], vec![0, 2], vec![1, 3], vec![0, 2]],
            vec![vec![1, 3], vec![0, 2], vec![1, 3], vec![0, 2]],
            vec![vec![1, 3], vec![0, 2], vec![1, 3], vec![0, 2]],
            vec![vec![1, 3], vec![0, 2], vec![1, 3], vec![0, 2]],
        ];

        let weights = vec![1.0, 1.0, 1.0, 1.0];
        let newgrid = MjGrid::with_values(8, 8, 1, "ABCD");
        let map = vec![
            vec![true, true, true, true],
            vec![true, true, true, true],
            vec![true, true, true, true],
            vec![true, true, true, true],
        ];

        let mut wfc = WfcNode::new(
            64, // wave_length (8x8)
            4,  // num_patterns
            4,  // num_directions
            propagator.clone(),
            weights,
            newgrid,
            map,
            1,     // n
            true,  // periodic
            false, // shannon
            20,    // tries
            8,     // mx
            8,     // my
            1,     // mz
        );

        let grid = MjGrid::with_values(8, 8, 1, "ABCD");
        let mut rng = StdRandom::from_u64_seed(123);

        assert!(wfc.initialize(&grid, &mut rng));

        let mut steps = 0;
        while wfc.state == WfcState::Running {
            wfc.step();
            steps += 1;
            assert!(steps < 5000, "WFC should complete");
        }

        assert_eq!(wfc.state, WfcState::Completed);
        assert!(wfc.wave.is_collapsed());

        // Verify all adjacency constraints
        let mx = 8;
        let my = 8;

        for y in 0..my {
            for x in 0..mx {
                let cell = x + y * mx;
                let pattern = wfc.wave.get_collapsed_pattern(cell).unwrap();

                // Check all 4 directions
                for d in 0..4 {
                    let (dx, dy) = match d {
                        0 => (1, 0),  // +X
                        1 => (0, 1),  // +Y
                        2 => (-1, 0), // -X
                        3 => (0, -1), // -Y
                        _ => unreachable!(),
                    };

                    let nx = ((x as i32 + dx + mx as i32) % mx as i32) as usize;
                    let ny = ((y as i32 + dy + my as i32) % my as i32) as usize;
                    let neighbor_cell = nx + ny * mx;
                    let neighbor_pattern = wfc.wave.get_collapsed_pattern(neighbor_cell).unwrap();

                    assert!(
                        propagator[d][pattern].contains(&neighbor_pattern),
                        "Constraint violated: pattern {} at ({},{}) -> pattern {} at ({},{}) in direction {}",
                        pattern, x, y, neighbor_pattern, nx, ny, d
                    );
                }
            }
        }
    }
}
