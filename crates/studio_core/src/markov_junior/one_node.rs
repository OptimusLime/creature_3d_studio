//! OneNode - Apply a single random match per step.
//!
//! OneNode scans for all valid rule matches, picks one at random,
//! and applies it. Returns false when no matches remain.
//!
//! Supports heuristic-guided selection when fields are configured:
//! - Uses Field.DeltaPointwise to score matches
//! - temperature=0: greedy (pick lowest delta)
//! - temperature>0: probabilistic weighted selection
//!
//! C# Reference: OneNode.cs

use super::field::delta_pointwise;
use super::node::{ExecutionContext, Node};
use super::observation::Observation;
use super::rule_node::RuleNodeData;
use super::MjRule;

/// A node that applies one random rule match per step.
///
/// C# Reference: OneNode.cs
pub struct OneNode {
    /// Shared rule matching data
    pub data: RuleNodeData,
}

impl OneNode {
    /// Create a new OneNode with the given rules.
    pub fn new(rules: Vec<MjRule>, grid_size: usize) -> Self {
        Self {
            data: RuleNodeData::new(rules, grid_size),
        }
    }

    /// Apply a rule at the given position, recording changes.
    ///
    /// C# Reference: OneNode.Apply() lines 27-49
    fn apply(&mut self, rule: &MjRule, x: i32, y: i32, z: i32, ctx: &mut ExecutionContext) {
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;

        for dz in 0..rule.omz {
            for dy in 0..rule.omy {
                for dx in 0..rule.omx {
                    let out_idx = dx + dy * rule.omx + dz * rule.omx * rule.omy;
                    let new_value = rule.output[out_idx];

                    if new_value != 0xff {
                        let sx = x + dx as i32;
                        let sy = y + dy as i32;
                        let sz = z + dz as i32;
                        let si = sx as usize + sy as usize * mx + sz as usize * mx * my;

                        let old_value = ctx.grid.state[si];
                        if new_value != old_value {
                            ctx.grid.state[si] = new_value;
                            ctx.record_change(sx, sy, sz);
                        }
                    }
                }
            }
        }
    }

    /// Pick a random valid match (no heuristics).
    ///
    /// Validates matches as we go (grid may have changed), removing invalid ones.
    /// Returns (rule_index, x, y, z) or None if no valid match.
    ///
    /// C# Reference: OneNode.RandomMatch() lines 122-138 (the else branch)
    fn random_match_simple(
        &mut self,
        ctx: &mut ExecutionContext,
    ) -> Option<(usize, i32, i32, i32)> {
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;

        while self.data.match_count > 0 {
            // Pick random match
            let match_index = ctx.random.next_usize_max(self.data.match_count);
            let (r, x, y, z) = self.data.matches[match_index];

            // Remove from tracking (swap with last)
            let i = x as usize + y as usize * mx + z as usize * mx * my;
            self.data.match_mask[r][i] = false;
            self.data.matches[match_index] = self.data.matches[self.data.match_count - 1];
            self.data.match_count -= 1;

            // Validate match still holds
            if ctx.grid.matches(&self.data.rules[r], x, y, z) {
                return Some((r, x, y, z));
            }
        }

        None
    }

    /// Pick a match using heuristic-guided selection.
    ///
    /// Uses Field.DeltaPointwise to score matches:
    /// - temperature=0: greedy (pick lowest delta, with small random tiebreaker)
    /// - temperature>0: probabilistic weighted by exp((h - firstH) / temperature)
    ///
    /// C# Reference: OneNode.RandomMatch() lines 77-120
    fn random_match_heuristic(
        &mut self,
        ctx: &mut ExecutionContext,
    ) -> Option<(usize, i32, i32, i32)> {
        // Check if goal reached (with observations)
        // C# Reference: OneNode.RandomMatch() lines 79-83
        if let (Some(ref observations), Some(ref future)) =
            (&self.data.observations, &self.data.future)
        {
            if Observation::is_goal_reached(&ctx.grid.state, future) {
                self.data.future_computed = false;
                return None;
            }
        }

        let mx = ctx.grid.mx;
        let my = ctx.grid.my;
        let temperature = self.data.temperature;

        // Get references to potentials and fields
        let potentials = self.data.potentials.as_ref()?;
        let fields = self.data.fields.as_ref();

        let mut max_key = -1000.0f64;
        let mut argmax: Option<usize> = None;

        let mut first_heuristic = 0.0f64;
        let mut first_heuristic_computed = false;

        let mut k = 0;
        while k < self.data.match_count {
            let (r, x, y, z) = self.data.matches[k];
            let i = x as usize + y as usize * mx + z as usize * mx * my;

            // Check if match still valid
            if !ctx.grid.matches(&self.data.rules[r], x, y, z) {
                // Remove invalid match (swap with last, decrement count)
                // C# Reference: lines 96-100: k-- after swap so loop re-checks swapped element
                // Our while loop with continue achieves the same - k stays same, re-checks swapped element
                self.data.match_mask[r][i] = false;
                self.data.matches[k] = self.data.matches[self.data.match_count - 1];
                self.data.match_count -= 1;
                continue;
            }

            // Calculate heuristic delta
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
                if !first_heuristic_computed {
                    first_heuristic = h as f64;
                    first_heuristic_computed = true;
                }

                let u: f64 = ctx.random.next_double();
                let h = h as f64;

                // C# Reference: lines 112-113
                // temperature > 0: key = u^exp((h - firstH) / temperature)
                // temperature = 0: key = -h + 0.001 * u (greedy with tiebreaker)
                let key = if temperature > 0.0 {
                    u.powf(((h - first_heuristic) / temperature).exp())
                } else {
                    -h + 0.001 * u
                };

                if key > max_key {
                    max_key = key;
                    argmax = Some(k);
                }
            }

            // Only increment k when we processed this match (didn't remove it)
            k += 1;
        }

        // Return the best match
        // C# Reference: In heuristic path, the winning match is NOT removed from the list
        // or match_mask. Only invalid matches are removed (in the loop above).
        if let Some(idx) = argmax {
            let (r, x, y, z) = self.data.matches[idx];
            Some((r, x, y, z))
        } else {
            None
        }
    }

    /// Pick a match (heuristic or random based on configuration).
    fn random_match(&mut self, ctx: &mut ExecutionContext) -> Option<(usize, i32, i32, i32)> {
        if self.data.potentials.is_some() {
            self.random_match_heuristic(ctx)
        } else {
            self.random_match_simple(ctx)
        }
    }
}

impl Node for OneNode {
    /// Execute one step: find matches, pick random one, apply it.
    ///
    /// C# Reference: OneNode.Go() lines 51-73
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // Compute matches (returns false if step limit reached)
        // OneNode passes is_all=false
        if !self.data.compute_matches(ctx, false) {
            return false;
        }

        // Record this as the last matched turn for incremental updates
        self.data.last_matched_turn = ctx.counter as i32;

        // C# Reference: OneNode.Go() lines 56-62 - trajectory replay
        if let Some(ref trajectory) = self.data.trajectory {
            if self.data.counter >= trajectory.len() {
                return false;
            }
            // Copy state from trajectory
            ctx.grid
                .state
                .copy_from_slice(&trajectory[self.data.counter]);
            self.data.counter += 1;
            return true;
        }

        // Pick and apply random match
        if let Some((r, x, y, z)) = self.random_match(ctx) {
            self.data.last[r] = true;

            // Clone the rule to avoid borrow issues
            let rule = self.data.rules[r].clone();
            self.apply(&rule, x, y, z, ctx);

            self.data.counter += 1;
            true
        } else {
            false
        }
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
    fn test_one_node_applies_single_match() {
        // 5x1 grid "BBBBB" with rule B->W
        // After 1 step, exactly 1 cell should be W
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut node = OneNode::new(vec![rule], grid.state.len());

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // One step
        assert!(node.go(&mut ctx));

        // Count W cells (value 1)
        let w_count = ctx.grid.state.iter().filter(|&&v| v == 1).count();
        assert_eq!(w_count, 1, "Exactly 1 cell should be W after 1 step");

        // B cells remaining
        let b_count = ctx.grid.state.iter().filter(|&&v| v == 0).count();
        assert_eq!(b_count, 4, "4 cells should still be B");
    }

    #[test]
    fn test_one_node_exhausts_matches() {
        let mut grid = MjGrid::with_values(3, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut node = OneNode::new(vec![rule], grid.state.len());

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Run until no more matches
        let mut steps = 0;
        while node.go(&mut ctx) {
            steps += 1;
            ctx.next_turn();
            if steps > 10 {
                panic!("Too many steps - should complete in 3");
            }
        }

        assert_eq!(steps, 3, "Should take exactly 3 steps for 3 cells");

        // All should be W now
        assert!(ctx.grid.state.iter().all(|&v| v == 1));
    }

    #[test]
    fn test_one_node_2x1_rule() {
        // Test that a 2-cell rule works correctly
        let mut grid = MjGrid::with_values(4, 1, 1, "BW");
        let rule = MjRule::parse("BB", "WW", &grid).unwrap();
        let mut node = OneNode::new(vec![rule], grid.state.len());

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // One step
        assert!(node.go(&mut ctx));

        // Should have changed 2 cells
        let w_count = ctx.grid.state.iter().filter(|&&v| v == 1).count();
        assert_eq!(w_count, 2, "Exactly 2 cells should be W after 1 step");
    }

    #[test]
    fn test_one_node_heuristic_selection() {
        // Test heuristic-guided selection prefers lower potential
        // Grid: 5x1, target W at right (x=4)
        // Rule: B->W
        // With fields, should prefer placing W closer to target (right side)
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        grid.state[4] = 1; // W at right

        let rule = MjRule::parse("B", "W", &grid).unwrap();

        // Create fields: W field with target at W positions
        let mut fields: Vec<Option<Field>> = vec![None; 2];
        fields[1] = Some(Field::new(1, 2)); // substrate=B, zero=W

        let mut node = OneNode {
            data: RuleNodeData::with_fields(vec![rule], grid.state.len(), fields, 0.0),
        };

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Run one step
        assert!(node.go(&mut ctx));

        // With temperature=0 (greedy), should pick position closest to target (x=3)
        // because delta_pointwise will give lower values for positions closer to W
        let w_count = ctx.grid.state.iter().filter(|&&v| v == 1).count();
        assert_eq!(w_count, 2, "Should have 2 W cells after 1 step");

        // The new W should be at x=3 (adjacent to target)
        assert_eq!(
            ctx.grid.state[3], 1,
            "Should place W at x=3 (closest to target)"
        );
    }

    #[test]
    fn test_one_node_heuristic_with_temperature() {
        // Test that temperature > 0 allows non-greedy selection
        // With high temperature, should be more random
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        grid.state[4] = 1; // W at right

        let rule = MjRule::parse("B", "W", &grid).unwrap();

        let mut fields: Vec<Option<Field>> = vec![None; 2];
        fields[1] = Some(Field::new(1, 2)); // substrate=B, zero=W

        // High temperature for probabilistic selection
        let mut node = OneNode {
            data: RuleNodeData::with_fields(vec![rule], grid.state.len(), fields, 100.0),
        };

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Run one step
        assert!(node.go(&mut ctx));

        // Should still place a W somewhere
        let w_count = ctx.grid.state.iter().filter(|&&v| v == 1).count();
        assert_eq!(w_count, 2, "Should have 2 W cells after 1 step");
    }
}
