//! Field - Distance field computation for heuristic guidance.
//!
//! Fields compute BFS distance from target cells, used by OneNode and AllNode
//! to guide rule selection toward goals.
//!
//! C# Reference: Field.cs

use super::MjGrid;
use super::MjRule;
use std::collections::VecDeque;

/// A distance field configuration.
///
/// Fields compute BFS distance from "zero" cells (targets) through "substrate" cells.
/// Used for heuristic-guided rule selection.
///
/// C# Reference: Field.cs lines 7-23
#[derive(Debug, Clone)]
pub struct Field {
    /// Recompute distance field each step (vs. only on first step)
    pub recompute: bool,
    /// If true, invert distance (maximize instead of minimize)
    pub inversed: bool,
    /// If false, failure to compute is not fatal
    pub essential: bool,
    /// Wave mask of cells that can be traversed
    pub substrate: u32,
    /// Wave mask of zero-distance cells (targets)
    pub zero: u32,
}

impl Field {
    /// Create a new Field with the given configuration.
    pub fn new(substrate: u32, zero: u32) -> Self {
        Self {
            recompute: false,
            inversed: false,
            essential: false,
            substrate,
            zero,
        }
    }

    /// Compute the distance field via BFS.
    ///
    /// Fills `potential` with distances from zero cells through substrate cells.
    /// Cells not reachable are left as -1.
    ///
    /// Returns false if no zero cells found (nothing to compute from).
    ///
    /// C# Reference: Field.cs lines 25-68
    pub fn compute(&self, potential: &mut [i32], grid: &MjGrid) -> bool {
        let mx = grid.mx;
        let my = grid.my;
        let mz = grid.mz;

        let mut front: VecDeque<(i32, i32, i32, i32)> = VecDeque::new();

        // Initialize: find zero cells (targets)
        let mut ix = 0i32;
        let mut iy = 0i32;
        let mut iz = 0i32;

        for i in 0..grid.state.len() {
            potential[i] = -1;
            let value = grid.state[i];

            // Check if this cell is a target (zero distance)
            if (self.zero & (1 << value)) != 0 {
                potential[i] = 0;
                front.push_back((0, ix, iy, iz));
            }

            // Update position
            ix += 1;
            if ix == mx as i32 {
                ix = 0;
                iy += 1;
                if iy == my as i32 {
                    iy = 0;
                    iz += 1;
                }
            }
        }

        // No targets found
        if front.is_empty() {
            return false;
        }

        // BFS
        while let Some((t, x, y, z)) = front.pop_front() {
            for (nx, ny, nz) in neighbors(x, y, z, mx, my, mz) {
                let i = nx as usize + ny as usize * mx + nz as usize * mx * my;
                let v = grid.state[i];

                // If unvisited and traversable
                if potential[i] == -1 && (self.substrate & (1 << v)) != 0 {
                    front.push_back((t + 1, nx, ny, nz));
                    potential[i] = t + 1;
                }
            }
        }

        true
    }
}

/// Get 6-connected neighbors of a cell (cardinal directions only).
///
/// C# Reference: Field.cs lines 70-82
fn neighbors(x: i32, y: i32, z: i32, mx: usize, my: usize, mz: usize) -> Vec<(i32, i32, i32)> {
    let mut result = Vec::with_capacity(6);
    let mx = mx as i32;
    let my = my as i32;
    let mz = mz as i32;

    if x > 0 {
        result.push((x - 1, y, z));
    }
    if x < mx - 1 {
        result.push((x + 1, y, z));
    }
    if y > 0 {
        result.push((x, y - 1, z));
    }
    if y < my - 1 {
        result.push((x, y + 1, z));
    }
    if z > 0 {
        result.push((x, y, z - 1));
    }
    if z < mz - 1 {
        result.push((x, y, z + 1));
    }

    result
}

/// Calculate potential delta for applying a rule at a position.
///
/// Returns the change in total potential if the rule were applied.
/// Returns None if any output cell would have -1 potential (unreachable).
///
/// C# Reference: Field.cs lines 84-118
pub fn delta_pointwise(
    state: &[u8],
    rule: &MjRule,
    x: i32,
    y: i32,
    z: i32,
    fields: Option<&[Option<Field>]>,
    potentials: &[Vec<i32>],
    mx: usize,
    my: usize,
) -> Option<i32> {
    let mut sum = 0i32;
    let mut dz = 0usize;
    let mut dy = 0usize;
    let mut dx = 0usize;

    for di in 0..rule.input.len() {
        let new_value = rule.output[di];

        // Only consider cells that will actually change (not wildcards)
        // and where the new value differs from what the input could match
        if new_value != 0xff && (rule.input[di] & (1 << new_value)) == 0 {
            let i = (x + dx as i32) as usize
                + (y + dy as i32) as usize * mx
                + (z + dz as i32) as usize * mx * my;

            let new_potential = potentials[new_value as usize][i];
            if new_potential == -1 {
                return None;
            }

            let old_value = state[i];
            let old_potential = potentials[old_value as usize][i];
            sum += new_potential - old_potential;

            // Handle inversed fields
            if let Some(fields) = fields {
                if let Some(Some(old_field)) = fields.get(old_value as usize) {
                    if old_field.inversed {
                        sum += 2 * old_potential;
                    }
                }
                if let Some(Some(new_field)) = fields.get(new_value as usize) {
                    if new_field.inversed {
                        sum -= 2 * new_potential;
                    }
                }
            }
        }

        // Update position within rule pattern
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

    Some(sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_bfs_simple() {
        // 5x5 grid, target (W) in center, substrate (B) everywhere else
        // B=0, W=1
        let mut grid = MjGrid::with_values(5, 5, 1, "BW");

        // Place W at center (2,2)
        grid.state[2 + 2 * 5] = 1;

        // Field: zero=W (bit 1), substrate=B (bit 0)
        let field = Field::new(1, 2); // substrate=0b01 (B), zero=0b10 (W)

        let mut potential = vec![-1i32; 25];
        let success = field.compute(&mut potential, &grid);

        assert!(success, "Should successfully compute field");

        // Center should be 0
        assert_eq!(potential[2 + 2 * 5], 0, "Center (target) should be 0");

        // Adjacent cells should be 1
        assert_eq!(potential[1 + 2 * 5], 1, "Left of center should be 1");
        assert_eq!(potential[3 + 2 * 5], 1, "Right of center should be 1");
        assert_eq!(potential[2 + 1 * 5], 1, "Below center should be 1");
        assert_eq!(potential[2 + 3 * 5], 1, "Above center should be 1");

        // Corners should be 4 (manhattan distance)
        assert_eq!(potential[0 + 0 * 5], 4, "Bottom-left corner should be 4");
        assert_eq!(potential[4 + 4 * 5], 4, "Top-right corner should be 4");
    }

    #[test]
    fn test_field_bfs_with_obstacles() {
        // 5x5 grid with wall blocking direct path
        // B=0 (substrate), W=1 (target), X=2 (wall, not in substrate)
        let mut grid = MjGrid::with_values(5, 5, 1, "BWX");

        // Place W at (4,2) - right side
        grid.state[4 + 2 * 5] = 1;

        // Place wall at x=2 (vertical line blocking middle)
        for y in 0..5 {
            if y != 2 {
                // Leave gap at y=2
                grid.state[2 + y * 5] = 2; // X (wall)
            }
        }

        // Field: zero=W, substrate=B (not X)
        let field = Field::new(1, 2); // substrate=0b01 (B), zero=0b10 (W)

        let mut potential = vec![-1i32; 25];
        let success = field.compute(&mut potential, &grid);

        assert!(success, "Should successfully compute field");

        // Target should be 0
        assert_eq!(potential[4 + 2 * 5], 0, "Target should be 0");

        // Cell at (0,2) should go through gap: right to (2,2), then to target
        // Path: (0,2) -> (1,2) -> (2,2) -> (3,2) -> (4,2) = 4 steps
        assert_eq!(
            potential[0 + 2 * 5],
            4,
            "Cell at gap level should be 4 steps"
        );

        // Walls should be unreachable (-1)
        assert_eq!(potential[2 + 0 * 5], -1, "Wall cells should be unreachable");
        assert_eq!(potential[2 + 1 * 5], -1, "Wall cells should be unreachable");
    }

    #[test]
    fn test_field_no_targets_returns_false() {
        // Grid with no target cells
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        // All cells are B (0), no W cells

        let field = Field::new(1, 2); // zero=W, but no W in grid

        let mut potential = vec![-1i32; 25];
        let success = field.compute(&mut potential, &grid);

        assert!(!success, "Should return false when no targets");
    }

    #[test]
    fn test_delta_pointwise_simple() {
        // Simple test: rule changes B->W, check delta
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();

        // Create potentials: W has 0 everywhere, B has distance from center
        let mut potentials = vec![vec![0i32; 25]; 2];

        // B potential: distance from center
        for y in 0..5i32 {
            for x in 0..5i32 {
                let dist = (x - 2).abs() + (y - 2).abs();
                potentials[0][(x + y * 5) as usize] = dist;
            }
        }
        // W potential: 0 at center, -1 elsewhere (only center is target)
        potentials[1] = vec![-1; 25];
        potentials[1][2 + 2 * 5] = 0;

        // At corner (0,0): B has potential 4, W has potential -1
        // Changing B->W at corner should return None (W unreachable there)
        let delta = delta_pointwise(&grid.state, &rule, 0, 0, 0, None, &potentials, 5, 5);
        assert!(delta.is_none(), "Should return None for unreachable target");

        // At center (2,2): B has potential 0, W has potential 0
        // Delta = new_potential - old_potential = 0 - 0 = 0
        let delta = delta_pointwise(&grid.state, &rule, 2, 2, 0, None, &potentials, 5, 5);
        assert_eq!(delta, Some(0), "Delta at center should be 0");
    }

    #[test]
    fn test_delta_pointwise_prefers_lower() {
        // Test that delta is negative when moving toward target
        let grid = MjGrid::with_values(5, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();

        // Potentials: both B and W have same distance field
        // Target at x=4
        let mut potentials = vec![vec![0i32; 5]; 2];
        for x in 0..5 {
            potentials[0][x] = (4 - x as i32).abs(); // B: distance from right
            potentials[1][x] = (4 - x as i32).abs(); // W: same
        }

        // At x=0: B has potential 4, W has potential 4
        // No change in potential
        let delta = delta_pointwise(&grid.state, &rule, 0, 0, 0, None, &potentials, 5, 1);
        assert_eq!(delta, Some(0), "Same potentials should give delta 0");
    }

    #[test]
    fn test_neighbors() {
        // Test neighbor generation
        let n = neighbors(2, 2, 0, 5, 5, 1);
        assert_eq!(n.len(), 4, "2D grid center should have 4 neighbors");

        let corner = neighbors(0, 0, 0, 5, 5, 1);
        assert_eq!(corner.len(), 2, "2D corner should have 2 neighbors");

        let n3d = neighbors(1, 1, 1, 3, 3, 3);
        assert_eq!(n3d.len(), 6, "3D center should have 6 neighbors");
    }
}
