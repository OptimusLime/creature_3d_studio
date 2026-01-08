//! ParallelNode - Apply all matches simultaneously using double-buffering.
//!
//! ParallelNode scans for matches and applies them all at once, reading from
//! the original state and writing to a buffer, then copying back. This allows
//! overlapping patterns to all read the same original state.
//!
//! C# Reference: ParallelNode.cs

use super::node::{ExecutionContext, Node};
use super::rule_node::RuleNodeData;
use super::MjRule;

/// A node that applies all matches simultaneously (double-buffered).
///
/// Unlike AllNode which applies sequentially and skips overlaps,
/// ParallelNode reads all matches from the original state and writes
/// to a buffer, then copies back. Overlapping writes go to the same
/// cell (last write wins, but all reads see original).
///
/// C# Reference: ParallelNode.cs
pub struct ParallelNode {
    /// Shared rule matching data
    pub data: RuleNodeData,
    /// Buffer for simultaneous writes
    newstate: Vec<u8>,
}

impl ParallelNode {
    /// Create a new ParallelNode with the given rules.
    pub fn new(rules: Vec<MjRule>, grid_size: usize) -> Self {
        Self {
            data: RuleNodeData::new(rules, grid_size),
            newstate: vec![0; grid_size],
        }
    }

    /// Create a ParallelNode with pre-configured RuleNodeData.
    pub fn with_data(data: RuleNodeData) -> Self {
        let grid_size = if !data.match_mask.is_empty() {
            data.match_mask[0].len()
        } else {
            0
        };
        Self {
            data,
            newstate: vec![0; grid_size],
        }
    }

    /// Apply a match directly during scanning (overrides RuleNode.Add behavior).
    ///
    /// In C#, ParallelNode overrides Add() to apply immediately to newstate
    /// based on rule.p probability.
    ///
    /// C# Reference: ParallelNode.Add() lines 16-34
    fn apply_match(
        &mut self,
        r: usize,
        x: i32,
        y: i32,
        z: i32,
        ctx: &mut ExecutionContext,
    ) -> bool {
        let rule = &self.data.rules[r];

        // Check probability
        if ctx.random.next_double() > rule.p {
            return false;
        }

        self.data.last[r] = true;
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;
        let mut changed = false;

        for dz in 0..rule.omz {
            for dy in 0..rule.omy {
                for dx in 0..rule.omx {
                    let out_idx = dx + dy * rule.omx + dz * rule.omx * rule.omy;
                    let new_value = rule.output[out_idx];

                    let sx = x + dx as i32;
                    let sy = y + dy as i32;
                    let sz = z + dz as i32;
                    let si = sx as usize + sy as usize * mx + sz as usize * mx * my;

                    if new_value != 0xff && new_value != ctx.grid.state[si] {
                        self.newstate[si] = new_value;
                        ctx.record_change(sx, sy, sz);
                        changed = true;
                    }
                }
            }
        }

        changed
    }

    /// Custom scan that applies matches directly (like C# Add override).
    fn scan_and_apply(&mut self, ctx: &mut ExecutionContext) {
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;
        let mz = ctx.grid.mz;

        self.data.match_count = 0;

        // Collect matches first to avoid borrow issues
        let mut pending_matches: Vec<(usize, i32, i32, i32)> = Vec::new();

        for (r, rule) in self.data.rules.iter().enumerate() {
            let mut z = rule.imz as i32 - 1;
            while z < mz as i32 {
                let mut y = rule.imy as i32 - 1;
                while y < my as i32 {
                    let mut x = rule.imx as i32 - 1;
                    while x < mx as i32 {
                        let grid_idx = x as usize + y as usize * mx + z as usize * mx * my;
                        let value = ctx.grid.state[grid_idx];

                        if (value as usize) < rule.ishifts.len() {
                            for &(shiftx, shifty, shiftz) in &rule.ishifts[value as usize] {
                                let sx = x - shiftx;
                                let sy = y - shifty;
                                let sz = z - shiftz;

                                // Check bounds for both input AND output pattern dimensions
                                let max_x = rule.imx.max(rule.omx) as i32;
                                let max_y = rule.imy.max(rule.omy) as i32;
                                let max_z = rule.imz.max(rule.omz) as i32;
                                if sx < 0
                                    || sy < 0
                                    || sz < 0
                                    || sx + max_x > mx as i32
                                    || sy + max_y > my as i32
                                    || sz + max_z > mz as i32
                                {
                                    continue;
                                }

                                if ctx.grid.matches(rule, sx, sy, sz) {
                                    pending_matches.push((r, sx, sy, sz));
                                }
                            }
                        }

                        x += rule.imx as i32;
                    }
                    y += rule.imy as i32;
                }
                z += rule.imz as i32;
            }
        }

        // Now apply all matches
        for (r, sx, sy, sz) in pending_matches {
            if self.apply_match(r, sx, sy, sz, ctx) {
                self.data.match_count += 1;
            }
        }
    }
}

impl Node for ParallelNode {
    /// Execute one step: scan for matches, apply all simultaneously.
    ///
    /// C# Reference: ParallelNode.Go() lines 36-50
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // Clear last flags
        for r in 0..self.data.last.len() {
            self.data.last[r] = false;
        }

        // Check step limit
        if self.data.steps > 0 && self.data.counter >= self.data.steps {
            return false;
        }

        // Ensure newstate buffer matches current grid size (grid can grow via map nodes)
        let grid_size = ctx.grid.state.len();
        if self.newstate.len() != grid_size {
            self.newstate.resize(grid_size, 0);
        }

        // Record changes start
        let changes_start = ctx.changes.len();

        // Scan and apply matches to newstate buffer
        self.scan_and_apply(ctx);

        // Copy changes from newstate to actual grid state
        // C# Reference: ParallelNode.Go() lines 40-45
        for n in changes_start..ctx.changes.len() {
            let (x, y, z) = ctx.changes[n];
            let i = x as usize + y as usize * ctx.grid.mx + z as usize * ctx.grid.mx * ctx.grid.my;
            ctx.grid.state[i] = self.newstate[i];
        }

        self.data.counter += 1;

        // Return true if any matches were applied
        self.data.match_count > 0
    }

    fn reset(&mut self) {
        self.data.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::rng::StdRandom;
    use crate::markov_junior::MjGrid;
    use rand::SeedableRng;

    #[test]
    fn test_parallel_node_applies_all() {
        // 5x1 grid "BBBBB" with rule B->W (p=1.0)
        // All 5 cells should become W simultaneously
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut node = ParallelNode::new(vec![rule], grid.state.len());

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        assert!(node.go(&mut ctx));

        // All should be W
        assert!(
            ctx.grid.state.iter().all(|&v| v == 1),
            "All 5 cells should be W. Got: {:?}",
            ctx.grid.state
        );
    }

    #[test]
    fn test_parallel_node_reads_original_state() {
        // Test that parallel node reads original state for all matches
        // 3x1 grid "BBB" with rule BW->WB
        // If we have BWB initially, both BW matches would see the same state
        let mut grid = MjGrid::with_values(3, 1, 1, "BW");
        // Set to BWB
        grid.state[0] = 0; // B
        grid.state[1] = 1; // W
        grid.state[2] = 0; // B

        // Rule BW -> WB (swap adjacent B and W)
        let rule = MjRule::parse("BW", "WB", &grid).unwrap();
        let mut node = ParallelNode::new(vec![rule], grid.state.len());

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Before: B W B (indices 0,1,2)
        // Match at position 0: BW -> WB (writes W to 0, B to 1)
        // No match at position 1 (WB doesn't match BW)
        assert!(node.go(&mut ctx));

        // Result should be W B B
        assert_eq!(ctx.grid.state[0], 1, "Position 0 should be W");
        assert_eq!(ctx.grid.state[1], 0, "Position 1 should be B");
        assert_eq!(ctx.grid.state[2], 0, "Position 2 should be B (unchanged)");
    }

    #[test]
    fn test_parallel_node_returns_false_when_no_matches() {
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        grid.state.fill(1); // All W

        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut node = ParallelNode::new(vec![rule], grid.state.len());

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        assert!(!node.go(&mut ctx));
    }
}
