//! AllNode - Apply all non-overlapping matches in one step.
//!
//! AllNode scans for all valid rule matches, shuffles them, and applies
//! as many as possible without overlap. Uses grid.mask to track claimed cells.
//!
//! Supports heuristic-guided ordering when fields are configured:
//! - Sorts matches by delta_pointwise score instead of shuffling
//! - temperature=0: strict sorting by delta
//! - temperature>0: probabilistic weighted ordering
//!
//! C# Reference: AllNode.cs

use super::field::delta_pointwise;
use super::node::{ExecutionContext, Node};
use super::rng::shuffle_indices;
use super::rule_node::RuleNodeData;
use super::MjRule;

/// A node that applies all non-overlapping rule matches per step.
///
/// C# Reference: AllNode.cs
pub struct AllNode {
    /// Shared rule matching data
    pub data: RuleNodeData,
}

impl AllNode {
    /// Create a new AllNode with the given rules.
    pub fn new(rules: Vec<MjRule>, grid_size: usize) -> Self {
        Self {
            data: RuleNodeData::new(rules, grid_size),
        }
    }

    /// Compute heuristic-based ordering of matches.
    ///
    /// Returns indices into self.data.matches sorted by heuristic score.
    /// Uses delta_pointwise to compute scores, then sorts by:
    /// - temperature=0: strict descending by key (lower delta = higher priority)
    /// - temperature>0: probabilistic weighted by exp((h - firstH) / temperature)
    ///
    /// C# Reference: AllNode.Go() lines 57-86
    fn compute_heuristic_order(&self, ctx: &mut ExecutionContext) -> Vec<usize> {
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;
        let temperature = self.data.temperature;

        let potentials = self.data.potentials.as_ref().unwrap();
        let fields = self.data.fields.as_ref();

        let mut first_heuristic = 0.0f64;
        let mut first_heuristic_computed = false;

        // Calculate heuristic for each match
        let mut scored: Vec<(usize, f64)> = Vec::new();

        for m in 0..self.data.match_count {
            let (r, x, y, z) = self.data.matches[m];

            let heuristic = delta_pointwise(
                &ctx.grid.state,
                &self.data.rules[r],
                x,
                y,
                z,
                fields.map(|f| f.as_slice()),
                potentials,
                mx,
                my,
            );

            if let Some(h) = heuristic {
                let h = h as f64;
                if !first_heuristic_computed {
                    first_heuristic = h;
                    first_heuristic_computed = true;
                }

                let u: f64 = ctx.random.next_double();

                // Same formula as OneNode
                let key = if temperature > 0.0 {
                    u.powf(((h - first_heuristic) / temperature).exp())
                } else {
                    -h + 0.001 * u
                };

                scored.push((m, key));
            }
        }

        // Sort by key descending (higher key = higher priority)
        // C# uses OrderBy which is stable, Rust's sort_by is also stable
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return just the indices
        scored.into_iter().map(|(idx, _)| idx).collect()
    }

    /// Try to fit a rule at the given position.
    ///
    /// Checks if any output cell is already claimed (mask=true).
    /// If not, applies the rule and marks output cells as claimed.
    ///
    /// C# Reference: AllNode.Fit() lines 18-39
    fn fit(&mut self, r: usize, x: i32, y: i32, z: i32, ctx: &mut ExecutionContext) {
        let rule = &self.data.rules[r];
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;

        // First pass: check if any output cell is already claimed
        for dz in 0..rule.omz {
            for dy in 0..rule.omy {
                for dx in 0..rule.omx {
                    let out_idx = dx + dy * rule.omx + dz * rule.omx * rule.omy;
                    let value = rule.output[out_idx];

                    // Only check cells that will be written (not wildcards)
                    if value != 0xff {
                        let sx = x as usize + dx;
                        let sy = y as usize + dy;
                        let sz = z as usize + dz;
                        let si = sx + sy * mx + sz * mx * my;

                        if ctx.grid.mask[si] {
                            // Cell already claimed, cannot apply this match
                            return;
                        }
                    }
                }
            }
        }

        // Second pass: apply the rule and mark cells
        self.data.last[r] = true;

        for dz in 0..rule.omz {
            for dy in 0..rule.omy {
                for dx in 0..rule.omx {
                    let out_idx = dx + dy * rule.omx + dz * rule.omx * rule.omy;
                    let new_value = rule.output[out_idx];

                    if new_value != 0xff {
                        let sx = (x + dx as i32) as usize;
                        let sy = (y + dy as i32) as usize;
                        let sz = (z + dz as i32) as usize;
                        let si = sx + sy * mx + sz * mx * my;

                        // Mark as claimed
                        ctx.grid.mask[si] = true;
                        // Apply change
                        ctx.grid.state[si] = new_value;
                        ctx.record_change(sx as i32, sy as i32, sz as i32);
                    }
                }
            }
        }
    }
}

impl Node for AllNode {
    /// Execute one step: find all matches, apply non-overlapping ones.
    ///
    /// C# Reference: AllNode.Go() lines 41-107
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // Compute matches - AllNode passes is_all=true
        if !self.data.compute_matches(ctx, true) {
            return false;
        }

        // Record this as the last matched turn BEFORE checking match_count
        // C# sets lastMatchedTurn = ip.counter at line 44, before checking matchCount at line 54
        self.data.last_matched_turn = ctx.counter as i32;

        if self.data.match_count == 0 {
            return false;
        }

        let mx = ctx.grid.mx;
        let my = ctx.grid.my;

        // Get ordering of matches (heuristic or shuffled)
        // C# Reference: AllNode.Go() lines 57-97
        let ordered = if self.data.potentials.is_some() {
            self.compute_heuristic_order(ctx)
        } else {
            // Shuffle randomly using C#'s exact algorithm
            shuffle_indices(self.data.match_count, ctx.random)
        };

        // Apply matches in order, skipping overlaps
        let changes_start = ctx.changes.len();

        for k in ordered {
            let (r, x, y, z) = self.data.matches[k];

            // Clear mask entry for this match position
            let i = x as usize + y as usize * mx + z as usize * mx * my;
            self.data.match_mask[r][i] = false;

            // Try to fit (will skip if overlapping with already-applied matches)
            self.fit(r, x, y, z, ctx);
        }

        // Clear mask for all changed cells (prepare for next step)
        // C# Reference: AllNode.Go() lines 99-103
        for n in changes_start..ctx.changes.len() {
            let (x, y, z) = ctx.changes[n];
            let i = x as usize + y as usize * mx + z as usize * mx * my;
            ctx.grid.mask[i] = false;
        }

        self.data.counter += 1;
        self.data.match_count = 0;

        // Return true if we made any changes
        ctx.changes.len() > changes_start
    }

    fn reset(&mut self) {
        self.data.reset();
        self.data.clear_match_mask();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::field::Field;
    use crate::markov_junior::rng::StdRandom;
    use crate::markov_junior::rule_node::RuleNodeData;
    use crate::markov_junior::MjGrid;
    use rand::SeedableRng;

    #[test]
    fn test_all_node_fills_entire_grid() {
        // 5x1 grid "BBBBB" with rule B->W
        // After 1 step, all 5 cells should be W (no overlap since 1x1 rule)
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut node = AllNode::new(vec![rule], grid.state.len());

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // One step
        assert!(node.go(&mut ctx));

        // All should be W (value 1)
        assert!(
            ctx.grid.state.iter().all(|&v| v == 1),
            "All 5 cells should be W after 1 step. Got: {:?}",
            ctx.grid.state
        );
    }

    #[test]
    fn test_all_node_non_overlapping() {
        // 5x1 grid "BBBBB" with rule BB->WW (2-cell rule)
        // Possible matches at positions 0,1,2,3 (patterns at 0-1, 1-2, 2-3, 3-4)
        // But they overlap! So only non-overlapping subset can apply.
        // Best case: positions 0 and 2 (or 1 and 3), giving 4 W cells
        // One B will remain at position 4 (or 0)
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        let rule = MjRule::parse("BB", "WW", &grid).unwrap();
        let mut node = AllNode::new(vec![rule], grid.state.len());

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // One step
        assert!(node.go(&mut ctx));

        // Count W cells
        let w_count = ctx.grid.state.iter().filter(|&&v| v == 1).count();
        let b_count = ctx.grid.state.iter().filter(|&&v| v == 0).count();

        // Should have 4 W and 1 B (2 non-overlapping matches of size 2)
        assert_eq!(
            w_count, 4,
            "Should have exactly 4 W cells. Got: {:?}",
            ctx.grid.state
        );
        assert_eq!(
            b_count, 1,
            "Should have exactly 1 B cell remaining. Got: {:?}",
            ctx.grid.state
        );
    }

    #[test]
    fn test_all_node_returns_false_when_no_matches() {
        // Grid is all W, rule needs B
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        grid.state.fill(1); // All W

        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut node = AllNode::new(vec![rule], grid.state.len());

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Should return false immediately (no matches)
        assert!(!node.go(&mut ctx));
    }

    #[test]
    fn test_all_node_2d_non_overlapping() {
        // 4x4 grid with 2x2 rule
        // For a 2x2 rule on 4x4 grid, valid match positions are:
        // (0,0), (1,0), (2,0)
        // (0,1), (1,1), (2,1)
        // (0,2), (1,2), (2,2)
        // That's 9 possible matches, but they overlap.
        // Non-overlapping placements: we can fit at most 4 (at corners: (0,0), (2,0), (0,2), (2,2))
        // Due to shuffling, we may not get the optimal placement.
        // Let's just verify we get at least 2 matches (4 cells) worth applied.
        let mut grid = MjGrid::with_values(4, 4, 1, "BW");
        let rule = MjRule::parse("BB/BB", "WW/WW", &grid).unwrap();
        let mut node = AllNode::new(vec![rule], grid.state.len());

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // One step
        assert!(node.go(&mut ctx));

        // With random shuffling, we should get at least 2 non-overlapping matches (8 cells)
        // The maximum possible is 4 matches (16 cells) if we pick (0,0), (2,0), (0,2), (2,2)
        let w_count = ctx.grid.state.iter().filter(|&&v| v == 1).count();
        assert!(
            w_count >= 8,
            "Should have at least 8 W cells (2 matches). Got: {} from {:?}",
            w_count,
            ctx.grid.state
        );
        // Actually in most random orderings we should get 3-4 matches
        assert!(
            w_count >= 12,
            "Should typically have 12+ W cells (3+ matches). Got: {} from {:?}",
            w_count,
            ctx.grid.state
        );
    }

    #[test]
    fn test_all_node_heuristic_sorting() {
        // Test heuristic-guided ordering
        // Grid: 5x1, target W at right (x=4)
        // Rule: B->W
        // With fields, should prefer applying matches closer to target first
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        grid.state[4] = 1; // W at right

        let rule = MjRule::parse("B", "W", &grid).unwrap();

        // Create fields: W field with target at W positions
        let mut fields: Vec<Option<Field>> = vec![None; 2];
        fields[1] = Some(Field::new(1, 2)); // substrate=B, zero=W

        let mut node = AllNode {
            data: RuleNodeData::with_fields(vec![rule], grid.state.len(), fields, 0.0),
        };

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Run one step - should convert all B to W
        assert!(node.go(&mut ctx));

        // All B's should now be W
        let w_count = ctx.grid.state.iter().filter(|&&v| v == 1).count();
        assert_eq!(w_count, 5, "All cells should be W after 1 step");
    }
}
