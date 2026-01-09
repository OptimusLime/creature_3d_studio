//! Search - A* search through state space for MarkovJunior.
//!
//! Implements search through possible rule applications to find a path
//! from the current state to a goal state satisfying observations.
//!
//! C# Reference: Search.cs (~295 lines)

use super::observation::Observation;
use super::rng::DotNetRandom;
#[cfg(test)]
use super::rng::StdRandom;
use super::MjRule;
use std::collections::{BinaryHeap, HashMap};

/// A search state node in the A* search tree.
#[derive(Clone)]
pub struct Board {
    /// Grid state at this node
    pub state: Vec<u8>,
    /// Index of parent board in database (-1 for root)
    pub parent_index: i32,
    /// Depth in search tree (number of steps from root)
    pub depth: i32,
    /// Backward estimate: steps from this state to reach goal
    pub backward_estimate: i32,
    /// Forward estimate: steps from start to reach this state
    pub forward_estimate: i32,
}

impl Board {
    /// Create a new board node.
    pub fn new(
        state: Vec<u8>,
        parent_index: i32,
        depth: i32,
        backward_estimate: i32,
        forward_estimate: i32,
    ) -> Self {
        Self {
            state,
            parent_index,
            depth,
            backward_estimate,
            forward_estimate,
        }
    }

    /// Compute ranking for priority queue.
    ///
    /// Lower rank = higher priority.
    /// Adds small random factor for tie-breaking.
    pub fn rank(&self, random: &mut dyn super::rng::MjRng, depth_coefficient: f64) -> f64 {
        let result = if depth_coefficient < 0.0 {
            1000.0 - self.depth as f64
        } else {
            (self.forward_estimate + self.backward_estimate) as f64
                + 2.0 * depth_coefficient * self.depth as f64
        };
        result + 0.0001 * random.next_double()
    }

    /// Extract trajectory from goal back to root.
    ///
    /// Returns boards from goal to root (needs to be reversed for forward order).
    pub fn trajectory(index: usize, database: &[Board]) -> Vec<Board> {
        let mut result = Vec::new();
        let mut current = &database[index];

        while current.parent_index >= 0 {
            result.push(current.clone());
            current = &database[current.parent_index as usize];
        }

        result
    }
}

/// Priority queue entry for A* search.
/// Uses Reverse ordering for min-heap behavior.
#[derive(Clone)]
struct PriorityEntry {
    index: usize,
    priority: f64,
}

impl PartialEq for PriorityEntry {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for PriorityEntry {}

impl PartialOrd for PriorityEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse ordering for min-heap (lower priority = higher in heap)
        other
            .priority
            .partial_cmp(&self.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Hash function for state comparison (matches C# StateComparer.GetHashCode)
fn state_hash(state: &[u8]) -> u64 {
    let mut result = 17u64;
    for &b in state {
        result = result.wrapping_mul(29).wrapping_add(b as u64);
    }
    result
}

/// Run A* search to find a path from present state to goal (future).
///
/// # Arguments
/// * `present` - Current grid state
/// * `future` - Future constraint waves (goal specification)
/// * `rules` - Available rewrite rules
/// * `mx`, `my`, `mz` - Grid dimensions
/// * `c` - Number of colors
/// * `all` - If true, use AllNode semantics (apply all non-overlapping matches)
/// * `limit` - Maximum states to explore (-1 for unlimited)
/// * `depth_coefficient` - Coefficient for depth in ranking
/// * `seed` - Random seed for tie-breaking (i32 to match C#'s System.Random)
///
/// # Returns
/// * `Some(trajectory)` - Sequence of states from start to goal (empty if already at goal)
/// * `None` - No path found (contradiction or limit reached)
pub fn run_search(
    present: &[u8],
    future: &[i32],
    rules: &[MjRule],
    mx: usize,
    my: usize,
    mz: usize,
    c: usize,
    all: bool,
    limit: i32,
    depth_coefficient: f64,
    seed: i32,
) -> Option<Vec<Vec<u8>>> {
    let grid_size = mx * my * mz;

    // Initialize potentials arrays
    let mut bpotentials: Vec<Vec<i32>> = vec![vec![-1; grid_size]; c];
    let mut fpotentials: Vec<Vec<i32>> = vec![vec![-1; grid_size]; c];

    // Compute backward potentials from future constraints
    Observation::compute_backward_potentials(&mut bpotentials, future, mx, my, mz, rules);
    let root_backward_estimate = Observation::backward_pointwise(&bpotentials, present);

    // Compute forward potentials from present state
    Observation::compute_forward_potentials(&mut fpotentials, present, mx, my, mz, rules);
    let root_forward_estimate = Observation::forward_pointwise(&fpotentials, future);

    // Check if problem is solvable
    if root_backward_estimate < 0 || root_forward_estimate < 0 {
        // Incorrect problem - no solution possible
        return None;
    }

    // Already at goal?
    if root_backward_estimate == 0 {
        return Some(Vec::new());
    }

    // Initialize search
    let root_board = Board::new(
        present.to_vec(),
        -1,
        0,
        root_backward_estimate,
        root_forward_estimate,
    );

    let mut database: Vec<Board> = vec![root_board.clone()];
    let mut visited: HashMap<u64, usize> = HashMap::new();
    visited.insert(state_hash(present), 0);

    let mut frontier: BinaryHeap<PriorityEntry> = BinaryHeap::new();
    // Use DotNetRandom to match C#'s System.Random behavior
    let mut random = DotNetRandom::from_seed(seed);

    frontier.push(PriorityEntry {
        index: 0,
        priority: root_board.rank(&mut random, depth_coefficient),
    });

    let mut record = root_backward_estimate + root_forward_estimate;

    while !frontier.is_empty() && (limit < 0 || (database.len() as i32) < limit) {
        let entry = frontier.pop().unwrap();
        let parent_index = entry.index;
        let parent_board = &database[parent_index];
        let parent_depth = parent_board.depth;
        let parent_state = parent_board.state.clone();

        // Generate child states
        let children = if all {
            all_child_states(&parent_state, mx, my, rules)
        } else {
            one_child_states(&parent_state, mx, my, rules)
        };

        for child_state in children {
            let hash = state_hash(&child_state);

            if let Some(&child_index) = visited.get(&hash) {
                // Already visited - check if we found a shorter path
                let old_board = &mut database[child_index];
                if parent_depth + 1 < old_board.depth {
                    old_board.depth = parent_depth + 1;
                    old_board.parent_index = parent_index as i32;

                    if old_board.backward_estimate >= 0 && old_board.forward_estimate >= 0 {
                        frontier.push(PriorityEntry {
                            index: child_index,
                            priority: old_board.rank(&mut random, depth_coefficient),
                        });
                    }
                }
            } else {
                // New state
                let child_backward_estimate =
                    Observation::backward_pointwise(&bpotentials, &child_state);

                // Recompute forward potentials for new state
                Observation::compute_forward_potentials(
                    &mut fpotentials,
                    &child_state,
                    mx,
                    my,
                    mz,
                    rules,
                );
                let child_forward_estimate = Observation::forward_pointwise(&fpotentials, future);

                if child_backward_estimate < 0 || child_forward_estimate < 0 {
                    continue; // Dead end
                }

                let child_board = Board::new(
                    child_state.clone(),
                    parent_index as i32,
                    parent_depth + 1,
                    child_backward_estimate,
                    child_forward_estimate,
                );

                database.push(child_board.clone());
                let child_index = database.len() - 1;
                visited.insert(hash, child_index);

                if child_forward_estimate == 0 {
                    // Found a solution!
                    let mut trajectory = Board::trajectory(child_index, &database);
                    trajectory.reverse();
                    return Some(trajectory.into_iter().map(|b| b.state).collect());
                } else {
                    if limit < 0 && child_backward_estimate + child_forward_estimate <= record {
                        record = child_backward_estimate + child_forward_estimate;
                    }

                    frontier.push(PriorityEntry {
                        index: child_index,
                        priority: child_board.rank(&mut random, depth_coefficient),
                    });
                }
            }
        }
    }

    // No solution found
    None
}

/// Generate all possible child states by applying one rule at one position (OneNode semantics).
fn one_child_states(state: &[u8], mx: usize, my: usize, rules: &[MjRule]) -> Vec<Vec<u8>> {
    let mut result = Vec::new();

    for rule in rules {
        for y in 0..my {
            for x in 0..mx {
                if matches_rule(rule, x, y, state, mx, my) {
                    result.push(apply_rule(rule, x, y, state, mx));
                }
            }
        }
    }

    result
}

/// Generate all possible child states by applying all non-overlapping matches (AllNode semantics).
fn all_child_states(state: &[u8], mx: usize, my: usize, rules: &[MjRule]) -> Vec<Vec<u8>> {
    // Find all matches
    let mut list: Vec<(usize, usize)> = Vec::new(); // (rule_index, position)
    let mut amounts = vec![0i32; state.len()];

    for i in 0..state.len() {
        let x = i % mx;
        let y = i / mx;

        for r in 0..rules.len() {
            let rule = &rules[r];
            if matches_rule(rule, x, y, state, mx, my) {
                list.push((r, i));
                // Mark cells covered by this match
                for dy in 0..rule.imy {
                    for dx in 0..rule.imx {
                        amounts[x + dx + (y + dy) * mx] += 1;
                    }
                }
            }
        }
    }

    if list.is_empty() {
        return Vec::new();
    }

    let mut mask = vec![true; list.len()];
    let mut solution: Vec<(usize, usize)> = Vec::new();
    let mut result: Vec<Vec<u8>> = Vec::new();

    enumerate_solutions(
        &mut result,
        &mut solution,
        &list,
        &mut amounts,
        &mut mask,
        state,
        mx,
        rules,
    );

    result
}

/// Recursively enumerate all maximal non-overlapping match combinations.
fn enumerate_solutions(
    children: &mut Vec<Vec<u8>>,
    solution: &mut Vec<(usize, usize)>,
    tiles: &[(usize, usize)],
    amounts: &mut [i32],
    mask: &mut [bool],
    state: &[u8],
    mx: usize,
    rules: &[MjRule],
) {
    // Find cell with maximum coverage
    let max_idx = max_positive_index(amounts);

    if max_idx < 0 {
        // No more cells to cover - we have a complete solution
        children.push(apply_solution(state, solution, mx, rules));
        return;
    }

    let max_x = (max_idx as usize) % mx;
    let max_y = (max_idx as usize) / mx;

    // Find all matches that cover this cell
    let mut cover: Vec<(usize, usize)> = Vec::new();
    for l in 0..tiles.len() {
        let (r, i) = tiles[l];
        let rule = &rules[r];
        let tile_x = i % mx;
        let tile_y = i / mx;

        if mask[l] && is_inside(max_x, max_y, rule, tile_x, tile_y) {
            cover.push((r, i));
        }
    }

    // Try each covering match
    for &(r, i) in &cover {
        solution.push((r, i));

        // Find intersecting matches
        let mut intersecting: Vec<usize> = Vec::new();
        let rule = &rules[r];
        let tile_x = i % mx;
        let tile_y = i / mx;

        for l in 0..tiles.len() {
            if mask[l] {
                let (r1, i1) = tiles[l];
                let rule1 = &rules[r1];
                let tile1_x = i1 % mx;
                let tile1_y = i1 / mx;

                if overlaps(rule, tile_x, tile_y, rule1, tile1_x, tile1_y) {
                    intersecting.push(l);
                }
            }
        }

        // Hide intersecting matches
        for &l in &intersecting {
            hide(l, false, tiles, amounts, mask, mx, rules);
        }

        enumerate_solutions(children, solution, tiles, amounts, mask, state, mx, rules);

        // Unhide intersecting matches
        for &l in &intersecting {
            hide(l, true, tiles, amounts, mask, mx, rules);
        }

        solution.pop();
    }
}

/// Find index of cell with maximum positive coverage amount.
fn max_positive_index(amounts: &[i32]) -> i32 {
    let mut max_val = 0i32;
    let mut max_idx = -1i32;

    for (i, &amt) in amounts.iter().enumerate() {
        if amt > max_val {
            max_val = amt;
            max_idx = i as i32;
        }
    }

    max_idx
}

/// Check if point (px, py) is inside rule bounds starting at (x, y).
fn is_inside(px: usize, py: usize, rule: &MjRule, x: usize, y: usize) -> bool {
    x <= px && px < x + rule.imx && y <= py && py < y + rule.imy
}

/// Check if two rule placements overlap.
fn overlaps(rule0: &MjRule, x0: usize, y0: usize, rule1: &MjRule, x1: usize, y1: usize) -> bool {
    for dy in 0..rule0.imy {
        for dx in 0..rule0.imx {
            if is_inside(x0 + dx, y0 + dy, rule1, x1, y1) {
                return true;
            }
        }
    }
    false
}

/// Hide or unhide a match for backtracking.
fn hide(
    l: usize,
    unhide: bool,
    tiles: &[(usize, usize)],
    amounts: &mut [i32],
    mask: &mut [bool],
    mx: usize,
    rules: &[MjRule],
) {
    mask[l] = unhide;
    let (r, i) = tiles[l];
    let rule = &rules[r];
    let x = i % mx;
    let y = i / mx;
    let incr = if unhide { 1 } else { -1 };

    for dy in 0..rule.imy {
        for dx in 0..rule.imx {
            amounts[x + dx + (y + dy) * mx] += incr;
        }
    }
}

/// Apply a solution (list of matches) to a state.
fn apply_solution(
    state: &[u8],
    solution: &[(usize, usize)],
    mx: usize,
    rules: &[MjRule],
) -> Vec<u8> {
    let mut result = state.to_vec();

    for &(r, i) in solution {
        let rule = &rules[r];
        let x = i % mx;
        let y = i / mx;
        apply_rule_in_place(rule, x, y, &mut result, mx);
    }

    result
}

/// Check if a rule matches at position (x, y) in state.
fn matches_rule(rule: &MjRule, x: usize, y: usize, state: &[u8], mx: usize, my: usize) -> bool {
    if x + rule.imx > mx || y + rule.imy > my {
        return false;
    }

    let mut dy = 0;
    let mut dx = 0;

    for di in 0..rule.input.len() {
        let wave = rule.input[di];
        let value = state[x + dx + (y + dy) * mx];

        if (wave & (1 << value)) == 0 {
            return false;
        }

        dx += 1;
        if dx == rule.imx {
            dx = 0;
            dy += 1;
        }
    }

    true
}

/// Apply a rule at position (x, y), returning new state.
fn apply_rule(rule: &MjRule, x: usize, y: usize, state: &[u8], mx: usize) -> Vec<u8> {
    let mut result = state.to_vec();
    apply_rule_in_place(rule, x, y, &mut result, mx);
    result
}

/// Apply a rule at position (x, y) in place.
fn apply_rule_in_place(rule: &MjRule, x: usize, y: usize, state: &mut [u8], mx: usize) {
    for dz in 0..rule.omz {
        for dy in 0..rule.omy {
            for dx in 0..rule.omx {
                let new_value = rule.output[dx + dy * rule.omx + dz * rule.omx * rule.omy];
                if new_value != 0xff {
                    state[x + dx + (y + dy) * mx] = new_value;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::MjGrid;

    #[test]
    fn test_board_rank() {
        let board = Board::new(vec![0, 0, 0], -1, 5, 10, 3);
        let mut rng = StdRandom::from_u64_seed(42);

        // With positive depth_coefficient
        let rank1 = board.rank(&mut rng, 1.0);
        // forward + backward + 2 * depth_coeff * depth = 3 + 10 + 2 * 1 * 5 = 23 + small random
        assert!(rank1 > 23.0 && rank1 < 24.0);

        // With negative depth_coefficient (prefer deeper)
        let mut rng2 = StdRandom::from_u64_seed(42);
        let rank2 = board.rank(&mut rng2, -1.0);
        // 1000 - depth = 1000 - 5 = 995 + small random
        assert!(rank2 > 994.0 && rank2 < 996.0);
    }

    #[test]
    fn test_matches_rule() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let rule = MjRule::parse("BW", "WB", &grid).unwrap();

        // State: BWxxx / xxxxx / ...
        let mut state = vec![0u8; 25];
        state[0] = 0; // B at (0,0)
        state[1] = 1; // W at (1,0)

        assert!(matches_rule(&rule, 0, 0, &state, 5, 5));
        assert!(!matches_rule(&rule, 1, 0, &state, 5, 5)); // WB doesn't match BW
    }

    #[test]
    fn test_apply_rule() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();

        let state = vec![0u8; 25]; // All B
        let new_state = apply_rule(&rule, 0, 0, &state, 5);

        assert_eq!(new_state[0], 1); // B -> W
        assert_eq!(new_state[1], 0); // unchanged
    }

    #[test]
    fn test_one_child_states() {
        let grid = MjGrid::with_values(3, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let rules = vec![rule];

        // State: BBB
        let state = vec![0u8, 0u8, 0u8];
        let children = one_child_states(&state, 3, 1, &rules);

        // Should have 3 children: WBB, BWB, BBW
        assert_eq!(children.len(), 3);
        assert!(children.contains(&vec![1, 0, 0]));
        assert!(children.contains(&vec![0, 1, 0]));
        assert!(children.contains(&vec![0, 0, 1]));
    }

    #[test]
    fn test_search_finds_solution_simple() {
        let grid = MjGrid::with_values(3, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let rules = vec![rule];

        // Start: BBB
        let present = vec![0u8, 0u8, 0u8];
        // Goal: all W (wave 0b10 = 2)
        let future = vec![2i32, 2i32, 2i32];

        let result = run_search(&present, &future, &rules, 3, 1, 1, 2, false, -1, 0.0, 42);

        assert!(result.is_some());
        let trajectory = result.unwrap();
        // Should find a path of 3 steps (B->W for each cell)
        assert_eq!(trajectory.len(), 3);
        // Final state should be all W
        assert_eq!(trajectory.last().unwrap(), &vec![1u8, 1u8, 1u8]);
    }

    #[test]
    fn test_search_already_at_goal() {
        let grid = MjGrid::with_values(3, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let rules = vec![rule];

        // Start: WWW (already at goal)
        let present = vec![1u8, 1u8, 1u8];
        // Goal: all W
        let future = vec![2i32, 2i32, 2i32];

        let result = run_search(&present, &future, &rules, 3, 1, 1, 2, false, -1, 0.0, 42);

        assert!(result.is_some());
        let trajectory = result.unwrap();
        assert!(trajectory.is_empty()); // Already at goal
    }

    #[test]
    fn test_search_no_solution() {
        let grid = MjGrid::with_values(3, 1, 1, "BWR");
        // Rule: B -> W (can only make W, not R)
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let rules = vec![rule];

        // Start: BBB
        let present = vec![0u8, 0u8, 0u8];
        // Goal: all R (wave 0b100 = 4) - impossible!
        let future = vec![4i32, 4i32, 4i32];

        let result = run_search(&present, &future, &rules, 3, 1, 1, 3, false, -1, 0.0, 42);

        assert!(result.is_none()); // No solution
    }

    #[test]
    fn test_search_with_limit() {
        let grid = MjGrid::with_values(5, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let rules = vec![rule];

        // Start: BBBBB
        let present = vec![0u8; 5];
        // Goal: all W
        let future = vec![2i32; 5];

        // Very low limit should fail
        let result = run_search(&present, &future, &rules, 5, 1, 1, 2, false, 2, 0.0, 42);
        assert!(result.is_none());
    }

    #[test]
    fn test_state_hash() {
        let state1 = vec![0u8, 1u8, 2u8];
        let state2 = vec![0u8, 1u8, 2u8];
        let state3 = vec![0u8, 1u8, 3u8];

        assert_eq!(state_hash(&state1), state_hash(&state2));
        assert_ne!(state_hash(&state1), state_hash(&state3));
    }

    #[test]
    fn test_all_child_states() {
        let grid = MjGrid::with_values(4, 1, 1, "BW");
        // Rule: BB -> WW
        let rule = MjRule::parse("BB", "WW", &grid).unwrap();
        let rules = vec![rule];

        // State: BBBB
        let state = vec![0u8; 4];
        let children = all_child_states(&state, 4, 1, &rules);

        // Possible non-overlapping combinations:
        // - Apply at (0,0) and (2,0) -> WWWW
        // - Apply at (0,0) only -> WWBx (partial)
        // - Apply at (1,0) only -> xWWx (partial)
        // etc.
        // The enumerate function finds all maximal combinations
        assert!(!children.is_empty());
        // At least one should be WWWW (both applied)
        assert!(children.contains(&vec![1, 1, 1, 1]));
    }
}
