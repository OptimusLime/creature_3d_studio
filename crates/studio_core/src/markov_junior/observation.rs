//! Observation - Constraint propagation for MarkovJunior.
//!
//! Observations define future constraints on the grid state.
//! They are used to guide rule application toward a goal state.
//!
//! C# Reference: Observation.cs (~185 lines)

use super::MjGrid;
use super::MjRule;
use std::collections::VecDeque;

/// An observation constraint: cells of value `from` should become values in `to` wave.
#[derive(Debug, Clone)]
pub struct Observation {
    /// Source value (byte index, not wave)
    pub from: u8,
    /// Target wave mask (multiple allowed values)
    pub to: u32,
}

impl Observation {
    /// Create a new observation.
    ///
    /// # Arguments
    /// * `from` - Character representing the source value
    /// * `to` - String of characters representing allowed target values
    /// * `grid` - Grid with value/wave mappings
    pub fn new(from: char, to: &str, grid: &MjGrid) -> Option<Self> {
        let from_value = *grid.values.get(&from)?;
        let to_wave = grid.wave(to);
        Some(Self {
            from: from_value,
            to: to_wave,
        })
    }

    /// Compute future constraints from current state and observations.
    ///
    /// Sets `future[i]` to the wave mask of allowed values for cell i.
    /// Also modifies `state` by replacing observed values with their `from` value.
    ///
    /// Returns false if an observation references a value not present in the grid.
    ///
    /// C# Reference: Observation.ComputeFutureSetPresent()
    pub fn compute_future_set_present(
        future: &mut [i32],
        state: &mut [u8],
        observations: &[Option<Observation>],
    ) -> bool {
        // Track which observation indices have been seen
        let mut mask = vec![false; observations.len()];

        // Mark null observations as already seen
        for k in 0..observations.len() {
            if observations[k].is_none() {
                mask[k] = true;
            }
        }

        // Process each cell
        for i in 0..state.len() {
            let value = state[i] as usize;
            if value >= observations.len() {
                // Value out of range, treat as no observation
                future[i] = 1 << state[i];
                continue;
            }

            mask[value] = true;

            if let Some(ref obs) = observations[value] {
                future[i] = obs.to as i32;
                state[i] = obs.from;
            } else {
                future[i] = 1 << value;
            }
        }

        // Check all observed values were present
        for k in 0..mask.len() {
            if !mask[k] {
                // Observed value not present on the grid
                return false;
            }
        }

        true
    }

    /// Compute forward potentials from current state.
    ///
    /// Potentials represent the minimum steps to reach each cell from the current state.
    /// Cells with current value get potential 0, then BFS propagates through rules.
    ///
    /// C# Reference: Observation.ComputeForwardPotentials()
    pub fn compute_forward_potentials(
        potentials: &mut [Vec<i32>],
        state: &[u8],
        mx: usize,
        my: usize,
        mz: usize,
        rules: &[MjRule],
    ) {
        // Initialize all potentials to -1 (unreachable)
        for p in potentials.iter_mut() {
            p.fill(-1);
        }

        // Set potential 0 for cells matching current state
        for (i, &value) in state.iter().enumerate() {
            if (value as usize) < potentials.len() {
                potentials[value as usize][i] = 0;
            }
        }

        // Propagate potentials through rules (forward direction)
        Self::compute_potentials(potentials, mx, my, mz, rules, false);
    }

    /// Compute backward potentials from future constraints.
    ///
    /// Potentials represent minimum steps to reach the goal state.
    /// Cells allowed by future constraints get potential 0, then BFS propagates backward.
    ///
    /// C# Reference: Observation.ComputeBackwardPotentials()
    pub fn compute_backward_potentials(
        potentials: &mut [Vec<i32>],
        future: &[i32],
        mx: usize,
        my: usize,
        mz: usize,
        rules: &[MjRule],
    ) {
        // Initialize potentials based on future constraints
        for c in 0..potentials.len() {
            let potential = &mut potentials[c];
            for i in 0..future.len() {
                potential[i] = if (future[i] & (1 << c)) != 0 { 0 } else { -1 };
            }
        }

        // Propagate potentials through rules (backward direction)
        Self::compute_potentials(potentials, mx, my, mz, rules, true);
    }

    /// Core BFS potential propagation.
    ///
    /// When `backwards` is false: propagate from input to output (forward)
    /// When `backwards` is true: propagate from output to input (backward)
    fn compute_potentials(
        potentials: &mut [Vec<i32>],
        mx: usize,
        my: usize,
        mz: usize,
        rules: &[MjRule],
        backwards: bool,
    ) {
        let grid_size = mx * my * mz;

        // Queue of (color, x, y, z) to process
        let mut queue: VecDeque<(u8, i32, i32, i32)> = VecDeque::new();

        // Initialize queue with all cells that have potential 0
        for c in 0..potentials.len() {
            let potential = &potentials[c];
            for i in 0..potential.len() {
                if potential[i] == 0 {
                    let x = (i % mx) as i32;
                    let y = ((i % (mx * my)) / mx) as i32;
                    let z = (i / (mx * my)) as i32;
                    queue.push_back((c as u8, x, y, z));
                }
            }
        }

        // Match mask: matchMask[rule_idx][position] = already processed
        let mut match_mask: Vec<Vec<bool>> = vec![vec![false; grid_size]; rules.len()];

        while let Some((value, x, y, z)) = queue.pop_front() {
            let i = (x as usize) + (y as usize) * mx + (z as usize) * mx * my;
            let t = potentials[value as usize][i];

            for (r, rule) in rules.iter().enumerate() {
                let maskr = &mut match_mask[r];

                // Get shifts based on direction
                let shifts = if backwards {
                    &rule.oshifts
                } else {
                    &rule.ishifts
                };

                // Skip if this color has no shifts for this rule
                if (value as usize) >= shifts.len() {
                    continue;
                }

                for &(shiftx, shifty, shiftz) in &shifts[value as usize] {
                    let sx = x - shiftx;
                    let sy = y - shifty;
                    let sz = z - shiftz;

                    // Bounds check
                    if sx < 0
                        || sy < 0
                        || sz < 0
                        || (sx as usize) + rule.imx > mx
                        || (sy as usize) + rule.imy > my
                        || (sz as usize) + rule.imz > mz
                    {
                        continue;
                    }

                    let si = (sx as usize) + (sy as usize) * mx + (sz as usize) * mx * my;

                    if !maskr[si]
                        && Self::forward_matches(
                            rule,
                            sx as usize,
                            sy as usize,
                            sz as usize,
                            potentials,
                            t,
                            mx,
                            my,
                            backwards,
                        )
                    {
                        maskr[si] = true;
                        Self::apply_forward(
                            rule,
                            sx as usize,
                            sy as usize,
                            sz as usize,
                            potentials,
                            t,
                            mx,
                            my,
                            &mut queue,
                            backwards,
                        );
                    }
                }
            }
        }
    }

    /// Check if a rule matches at position for potential propagation.
    ///
    /// When backwards=false: check input pattern against potentials
    /// When backwards=true: check output pattern against potentials
    fn forward_matches(
        rule: &MjRule,
        x: usize,
        y: usize,
        z: usize,
        potentials: &[Vec<i32>],
        t: i32,
        mx: usize,
        my: usize,
        backwards: bool,
    ) -> bool {
        let a = if backwards {
            &rule.output
        } else {
            &rule.binput
        };

        let mut dz = 0usize;
        let mut dy = 0usize;
        let mut dx = 0usize;

        for di in 0..a.len() {
            let value = a[di];
            if value != 0xff {
                let idx = (x + dx) + (y + dy) * mx + (z + dz) * mx * my;
                let current = potentials[value as usize][idx];
                if current > t || current == -1 {
                    return false;
                }
            }

            dx += 1;
            if dx == rule.imx {
                dx = 0;
                dy += 1;
                if dy == rule.imy {
                    dy = 0;
                    dz += 1;
                }
            }
        }

        true
    }

    /// Apply rule output to potentials and enqueue new cells.
    ///
    /// When backwards=false: apply output pattern
    /// When backwards=true: apply input pattern (binput)
    fn apply_forward(
        rule: &MjRule,
        x: usize,
        y: usize,
        z: usize,
        potentials: &mut [Vec<i32>],
        t: i32,
        mx: usize,
        my: usize,
        queue: &mut VecDeque<(u8, i32, i32, i32)>,
        backwards: bool,
    ) {
        let a = if backwards {
            &rule.binput
        } else {
            &rule.output
        };

        for dz in 0..rule.imz {
            let zdz = z + dz;
            for dy in 0..rule.imy {
                let ydy = y + dy;
                for dx in 0..rule.imx {
                    let xdx = x + dx;
                    let idi = xdx + ydy * mx + zdz * mx * my;
                    let di = dx + dy * rule.imx + dz * rule.imx * rule.imy;
                    let o = a[di];

                    if o != 0xff && potentials[o as usize][idi] == -1 {
                        potentials[o as usize][idi] = t + 1;
                        queue.push_back((o, xdx as i32, ydy as i32, zdz as i32));
                    }
                }
            }
        }
    }

    /// Check if current state satisfies future constraints.
    ///
    /// Returns true if every cell's current value is allowed by the future wave.
    ///
    /// C# Reference: Observation.IsGoalReached()
    pub fn is_goal_reached(present: &[u8], future: &[i32]) -> bool {
        for i in 0..present.len() {
            let value_wave = 1 << present[i];
            if (value_wave & future[i]) == 0 {
                return false;
            }
        }
        true
    }

    /// Compute forward heuristic estimate (sum of minimum potentials to reach future).
    ///
    /// For each cell, find the minimum potential among allowed future values.
    /// Returns -1 if any cell has no reachable allowed value.
    ///
    /// C# Reference: Observation.ForwardPointwise()
    pub fn forward_pointwise(potentials: &[Vec<i32>], future: &[i32]) -> i32 {
        let mut sum = 0i32;

        for i in 0..future.len() {
            let mut f = future[i];
            let mut min = 1000i32;
            let mut argmin = -1i32;

            for c in 0..potentials.len() {
                let potential = potentials[c][i];
                if (f & 1) == 1 && potential >= 0 && potential < min {
                    min = potential;
                    argmin = c as i32;
                }
                f >>= 1;
            }

            if argmin < 0 {
                return -1;
            }
            sum += min;
        }

        sum
    }

    /// Compute backward heuristic estimate (sum of potentials from present state).
    ///
    /// Sum the potentials for each cell's current value.
    /// Returns -1 if any cell's value has unreachable potential.
    ///
    /// C# Reference: Observation.BackwardPointwise()
    pub fn backward_pointwise(potentials: &[Vec<i32>], present: &[u8]) -> i32 {
        let mut sum = 0i32;

        for i in 0..present.len() {
            let value = present[i] as usize;
            if value >= potentials.len() {
                return -1;
            }
            let potential = potentials[value][i];
            if potential < 0 {
                return -1;
            }
            sum += potential;
        }

        sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observation_new() {
        let grid = MjGrid::with_values(5, 5, 1, "BWR");
        let obs = Observation::new('B', "WR", &grid).unwrap();
        assert_eq!(obs.from, 0); // B is index 0
        assert_eq!(obs.to, 0b110); // W=0b010, R=0b100
    }

    #[test]
    fn test_observation_new_unknown_char() {
        let grid = MjGrid::with_values(5, 5, 1, "BWR");
        let obs = Observation::new('X', "WR", &grid);
        assert!(obs.is_none());
    }

    #[test]
    fn test_compute_future_set_present() {
        let grid = MjGrid::with_values(3, 3, 1, "BWR");
        // Create observations: B -> W (value 0 should become value 1)
        let obs_b = Observation::new('B', "W", &grid).unwrap();
        let observations: Vec<Option<Observation>> = vec![Some(obs_b), None, None];

        // State: all B's (value 0)
        let mut state = vec![0u8; 9];
        let mut future = vec![0i32; 9];

        let result =
            Observation::compute_future_set_present(&mut future, &mut state, &observations);
        assert!(result);

        // All cells should now want W (wave 0b010 = 2)
        for f in &future {
            assert_eq!(*f, 2);
        }

        // State should still be 0 (B) since from=B maps to B
        for s in &state {
            assert_eq!(*s, 0);
        }
    }

    #[test]
    fn test_is_goal_reached() {
        // Goal: cell 0 should be W (bit 1), cell 1 can be B or W (bits 0,1)
        let future = vec![0b010i32, 0b011i32];

        // Present: [W, B] = [1, 0]
        let present_ok = vec![1u8, 0u8];
        assert!(Observation::is_goal_reached(&present_ok, &future));

        // Present: [B, B] = [0, 0] - cell 0 is B but should be W
        let present_bad = vec![0u8, 0u8];
        assert!(!Observation::is_goal_reached(&present_bad, &future));
    }

    #[test]
    fn test_forward_pointwise() {
        // 2 colors, 4 cells
        let mut potentials = vec![vec![-1i32; 4]; 2];
        // Color 0 reachable at cells 0,1 with potential 1
        potentials[0][0] = 1;
        potentials[0][1] = 1;
        // Color 1 reachable at cells 2,3 with potential 2
        potentials[1][2] = 2;
        potentials[1][3] = 2;

        // Future: cell 0 wants color 0, cell 1 wants color 0, cell 2 wants color 1, cell 3 wants color 1
        let future = vec![0b01i32, 0b01i32, 0b10i32, 0b10i32];

        let estimate = Observation::forward_pointwise(&potentials, &future);
        assert_eq!(estimate, 1 + 1 + 2 + 2); // sum of minimum potentials
    }

    #[test]
    fn test_forward_pointwise_unreachable() {
        // Color 0 unreachable everywhere
        let potentials = vec![vec![-1i32; 4]; 2];
        // Future wants color 0 at cell 0
        let future = vec![0b01i32, 0b10i32, 0b10i32, 0b10i32];

        let estimate = Observation::forward_pointwise(&potentials, &future);
        assert_eq!(estimate, -1); // unreachable
    }

    #[test]
    fn test_backward_pointwise() {
        // 2 colors, 4 cells
        let mut potentials = vec![vec![0i32; 4]; 2];
        potentials[0][0] = 3;
        potentials[0][1] = 2;
        potentials[1][2] = 1;
        potentials[1][3] = 0;

        // Present state: [0, 0, 1, 1]
        let present = vec![0u8, 0u8, 1u8, 1u8];

        let estimate = Observation::backward_pointwise(&potentials, &present);
        assert_eq!(estimate, 3 + 2 + 1 + 0);
    }

    #[test]
    fn test_backward_pointwise_unreachable() {
        let mut potentials = vec![vec![0i32; 4]; 2];
        potentials[0][0] = -1; // unreachable

        let present = vec![0u8, 0u8, 1u8, 1u8];
        let estimate = Observation::backward_pointwise(&potentials, &present);
        assert_eq!(estimate, -1);
    }

    #[test]
    fn test_compute_backward_potentials_simple() {
        let grid = MjGrid::with_values(3, 1, 1, "BW");

        // Rule: B -> W (at any position)
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let rules = vec![rule];

        // Future: all cells should be W (wave = 0b10 = 2)
        let future = vec![2i32, 2i32, 2i32];

        let mut potentials = vec![vec![0i32; 3]; 2];
        Observation::compute_backward_potentials(&mut potentials, &future, 3, 1, 1, &rules);

        // W (color 1) should have potential 0 everywhere (it's the goal)
        assert_eq!(potentials[1][0], 0);
        assert_eq!(potentials[1][1], 0);
        assert_eq!(potentials[1][2], 0);

        // B (color 0) should have potential 1 everywhere (one step from W via rule B->W)
        assert_eq!(potentials[0][0], 1);
        assert_eq!(potentials[0][1], 1);
        assert_eq!(potentials[0][2], 1);
    }

    #[test]
    fn test_compute_forward_potentials_simple() {
        let grid = MjGrid::with_values(3, 1, 1, "BW");

        // Rule: B -> W
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let rules = vec![rule];

        // Current state: all B's
        let state = vec![0u8, 0u8, 0u8];

        let mut potentials = vec![vec![0i32; 3]; 2];
        Observation::compute_forward_potentials(&mut potentials, &state, 3, 1, 1, &rules);

        // B (color 0) should have potential 0 everywhere (current state)
        assert_eq!(potentials[0][0], 0);
        assert_eq!(potentials[0][1], 0);
        assert_eq!(potentials[0][2], 0);

        // W (color 1) should have potential 1 everywhere (one step from B via rule B->W)
        assert_eq!(potentials[1][0], 1);
        assert_eq!(potentials[1][1], 1);
        assert_eq!(potentials[1][2], 1);
    }
}
