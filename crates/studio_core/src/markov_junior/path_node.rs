//! PathNode - Dijkstra pathfinding for dungeon generation.
//!
//! PathNode finds a path from start cells to finish cells through substrate cells,
//! then writes a specified value along the path. Used for dungeon/maze connectivity.
//!
//! C# Reference: Path.cs

use super::node::{ExecutionContext, Node};
use super::rng::{DotNetRandom, MjRng};
use std::collections::VecDeque;

/// A node that finds and draws paths between cells.
///
/// Uses BFS from finish positions to compute distances, then traces back
/// from a start position to draw the path.
///
/// C# Reference: Path.cs lines 8-26
#[derive(Debug, Clone)]
pub struct PathNode {
    /// Wave mask of start positions
    pub start: u32,
    /// Wave mask of finish positions (targets)
    pub finish: u32,
    /// Wave mask of cells that can be traversed
    pub substrate: u32,
    /// Value to write along the path
    pub value: u8,
    /// Prefer continuing in the same direction
    pub inertia: bool,
    /// Find longest path instead of shortest
    pub longest: bool,
    /// Allow diagonal moves in 2D (edge-connected)
    pub edges: bool,
    /// Allow 3D diagonal moves (vertex-connected)
    pub vertices: bool,
}

impl PathNode {
    /// Create a new PathNode with the given configuration.
    pub fn new(start: u32, finish: u32, substrate: u32, value: u8) -> Self {
        Self {
            start,
            finish,
            substrate,
            value,
            inertia: false,
            longest: false,
            edges: false,
            vertices: false,
        }
    }
}

impl Node for PathNode {
    /// Execute the pathfinding: find path from start to finish, draw it.
    ///
    /// Returns false if no valid path exists.
    ///
    /// C# Reference: Path.cs Go() lines 29-111
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;
        let mz = ctx.grid.mz;
        let grid_size = mx * my * mz;

        let mut frontier: VecDeque<(i32, i32, i32, i32)> = VecDeque::new();
        let mut start_positions: Vec<(i32, i32, i32)> = Vec::new();
        let mut generations = vec![-1i32; grid_size];

        // Find start and finish positions
        // C# Reference: Path.cs lines 36-48
        for z in 0..mz as i32 {
            for y in 0..my as i32 {
                for x in 0..mx as i32 {
                    let i = x as usize + y as usize * mx + z as usize * mx * my;
                    let s = ctx.grid.state[i];

                    if (self.start & (1 << s)) != 0 {
                        start_positions.push((x, y, z));
                    }
                    if (self.finish & (1 << s)) != 0 {
                        generations[i] = 0;
                        frontier.push_back((0, x, y, z));
                    }
                }
            }
        }

        // Return false if no start or finish positions
        if start_positions.is_empty() || frontier.is_empty() {
            return false;
        }

        // BFS from finish positions
        // C# Reference: Path.cs lines 52-67
        while let Some((t, x, y, z)) = frontier.pop_front() {
            for (dx, dy, dz) in directions(x, y, z, mx, my, mz, self.edges, self.vertices) {
                let nx = x + dx;
                let ny = y + dy;
                let nz = z + dz;
                let i = nx as usize + ny as usize * mx + nz as usize * mx * my;
                let v = ctx.grid.state[i];

                // Can traverse if substrate or start
                if generations[i] == -1
                    && ((self.substrate & (1 << v)) != 0 || (self.start & (1 << v)) != 0)
                {
                    // Only enqueue if substrate (not start)
                    if (self.substrate & (1 << v)) != 0 {
                        frontier.push_back((t + 1, nx, ny, nz));
                    }
                    generations[i] = t + 1;
                }
            }
        }

        // Check if any start position is reachable
        // C# Reference: Path.cs line 69
        let reachable: Vec<_> = start_positions
            .iter()
            .filter(|&&(x, y, z)| {
                let i = x as usize + y as usize * mx + z as usize * mx * my;
                generations[i] > 0
            })
            .copied()
            .collect();

        if reachable.is_empty() {
            return false;
        }

        // Create local RNG for this path
        // C# Reference: Path.cs line 71: Random localRandom = new(ip.random.Next());
        let seed = ctx.random.next_int();
        let mut local_random = DotNetRandom::from_seed(seed);

        // Find min/max generation start position
        // C# Reference: Path.cs lines 72-93
        // IMPORTANT: Must iterate in the SAME ORDER as start_positions (not reachable)
        // to match C# iteration order, but skip unreachable ones (g == -1)
        let mut min_gen = (mx * my * mz) as f64;
        let mut max_gen = -2.0f64;
        let mut argmin = (-1i32, -1i32, -1i32);
        let mut argmax = (-1i32, -1i32, -1i32);

        for &(px, py, pz) in &start_positions {
            let i = px as usize + py as usize * mx + pz as usize * mx * my;
            let g = generations[i];
            if g == -1 {
                continue;
            }
            let dg = g as f64;
            let noise = 0.1 * local_random.next_double();

            if dg + noise < min_gen {
                min_gen = dg + noise;
                argmin = (px, py, pz);
            }
            if dg + noise > max_gen {
                max_gen = dg + noise;
                argmax = (px, py, pz);
            }
        }

        // Select start based on longest flag
        let (mut penx, mut peny, mut penz) = if self.longest { argmax } else { argmin };

        // Get initial direction
        let (mut dirx, mut diry, mut dirz) = find_direction(
            penx,
            peny,
            penz,
            0,
            0,
            0,
            &generations,
            mx,
            my,
            mz,
            self.inertia,
            self.edges,
            self.vertices,
            &mut local_random,
        );

        // Move to first path cell
        penx += dirx;
        peny += diry;
        penz += dirz;

        // Trace path back to finish
        // C# Reference: Path.cs lines 101-110
        while generations[penx as usize + peny as usize * mx + penz as usize * mx * my] != 0 {
            let i = penx as usize + peny as usize * mx + penz as usize * mx * my;
            ctx.grid.state[i] = self.value;
            ctx.record_change(i);

            let (dx, dy, dz) = find_direction(
                penx,
                peny,
                penz,
                dirx,
                diry,
                dirz,
                &generations,
                mx,
                my,
                mz,
                self.inertia,
                self.edges,
                self.vertices,
                &mut local_random,
            );

            dirx = dx;
            diry = dy;
            dirz = dz;
            penx += dirx;
            peny += diry;
            penz += dirz;
        }

        true
    }

    fn reset(&mut self) {
        // PathNode has no state to reset
    }
}

/// Get possible move directions from a position.
///
/// Returns (dx, dy, dz) tuples for valid moves based on edges/vertices flags.
///
/// C# Reference: Path.cs Directions() lines 168-227
fn directions(
    x: i32,
    y: i32,
    z: i32,
    mx: usize,
    my: usize,
    mz: usize,
    edges: bool,
    vertices: bool,
) -> Vec<(i32, i32, i32)> {
    let mx = mx as i32;
    let my = my as i32;
    let mz = mz as i32;
    let mut result = Vec::new();

    if mz == 1 {
        // 2D case
        if x > 0 {
            result.push((-1, 0, 0));
        }
        if x < mx - 1 {
            result.push((1, 0, 0));
        }
        if y > 0 {
            result.push((0, -1, 0));
        }
        if y < my - 1 {
            result.push((0, 1, 0));
        }

        if edges {
            if x > 0 && y > 0 {
                result.push((-1, -1, 0));
            }
            if x > 0 && y < my - 1 {
                result.push((-1, 1, 0));
            }
            if x < mx - 1 && y > 0 {
                result.push((1, -1, 0));
            }
            if x < mx - 1 && y < my - 1 {
                result.push((1, 1, 0));
            }
        }
    } else {
        // 3D case
        if x > 0 {
            result.push((-1, 0, 0));
        }
        if x < mx - 1 {
            result.push((1, 0, 0));
        }
        if y > 0 {
            result.push((0, -1, 0));
        }
        if y < my - 1 {
            result.push((0, 1, 0));
        }
        if z > 0 {
            result.push((0, 0, -1));
        }
        if z < mz - 1 {
            result.push((0, 0, 1));
        }

        if edges {
            // XY diagonals
            if x > 0 && y > 0 {
                result.push((-1, -1, 0));
            }
            if x > 0 && y < my - 1 {
                result.push((-1, 1, 0));
            }
            if x < mx - 1 && y > 0 {
                result.push((1, -1, 0));
            }
            if x < mx - 1 && y < my - 1 {
                result.push((1, 1, 0));
            }

            // XZ diagonals
            if x > 0 && z > 0 {
                result.push((-1, 0, -1));
            }
            if x > 0 && z < mz - 1 {
                result.push((-1, 0, 1));
            }
            if x < mx - 1 && z > 0 {
                result.push((1, 0, -1));
            }
            if x < mx - 1 && z < mz - 1 {
                result.push((1, 0, 1));
            }

            // YZ diagonals
            if y > 0 && z > 0 {
                result.push((0, -1, -1));
            }
            if y > 0 && z < mz - 1 {
                result.push((0, -1, 1));
            }
            if y < my - 1 && z > 0 {
                result.push((0, 1, -1));
            }
            if y < my - 1 && z < mz - 1 {
                result.push((0, 1, 1));
            }
        }

        if vertices {
            // 3D corner diagonals
            if x > 0 && y > 0 && z > 0 {
                result.push((-1, -1, -1));
            }
            if x > 0 && y > 0 && z < mz - 1 {
                result.push((-1, -1, 1));
            }
            if x > 0 && y < my - 1 && z > 0 {
                result.push((-1, 1, -1));
            }
            if x > 0 && y < my - 1 && z < mz - 1 {
                result.push((-1, 1, 1));
            }
            if x < mx - 1 && y > 0 && z > 0 {
                result.push((1, -1, -1));
            }
            if x < mx - 1 && y > 0 && z < mz - 1 {
                result.push((1, -1, 1));
            }
            if x < mx - 1 && y < my - 1 && z > 0 {
                result.push((1, 1, -1));
            }
            if x < mx - 1 && y < my - 1 && z < mz - 1 {
                result.push((1, 1, 1));
            }
        }
    }

    result
}

/// Find the next direction to move along the path.
///
/// Traces back toward lower generation values (toward finish).
/// With inertia, prefers continuing in the same direction.
///
/// C# Reference: Path.cs Direction() lines 113-166
fn find_direction(
    x: i32,
    y: i32,
    z: i32,
    dx: i32,
    dy: i32,
    dz: i32,
    generations: &[i32],
    mx: usize,
    my: usize,
    mz: usize,
    inertia: bool,
    edges: bool,
    vertices: bool,
    random: &mut DotNetRandom,
) -> (i32, i32, i32) {
    let mx_i = mx as i32;
    let my_i = my as i32;
    let mz_i = mz as i32;
    let g = generations[x as usize + y as usize * mx + z as usize * mx * my];

    // Collect candidates that decrease generation by 1
    let mut candidates: Vec<(i32, i32, i32)> = Vec::new();

    let add_candidate = |candidates: &mut Vec<_>, ddx: i32, ddy: i32, ddz: i32| {
        let nx = x + ddx;
        let ny = y + ddy;
        let nz = z + ddz;
        if nx >= 0 && nx < mx_i && ny >= 0 && ny < my_i && nz >= 0 && nz < mz_i {
            let ni = nx as usize + ny as usize * mx + nz as usize * mx * my;
            if generations[ni] == g - 1 {
                candidates.push((ddx, ddy, ddz));
            }
        }
    };

    if !vertices && !edges {
        // Cardinal directions only
        // Check inertia first
        if dx != 0 || dy != 0 || dz != 0 {
            let cx = x + dx;
            let cy = y + dy;
            let cz = z + dz;
            if inertia && cx >= 0 && cy >= 0 && cz >= 0 && cx < mx_i && cy < my_i && cz < mz_i {
                let ci = cx as usize + cy as usize * mx + cz as usize * mx * my;
                if generations[ci] == g - 1 {
                    return (dx, dy, dz);
                }
            }
        }

        // Collect all valid cardinal moves
        if x > 0 {
            add_candidate(&mut candidates, -1, 0, 0);
        }
        if x < mx_i - 1 {
            add_candidate(&mut candidates, 1, 0, 0);
        }
        if y > 0 {
            add_candidate(&mut candidates, 0, -1, 0);
        }
        if y < my_i - 1 {
            add_candidate(&mut candidates, 0, 1, 0);
        }
        if z > 0 {
            add_candidate(&mut candidates, 0, 0, -1);
        }
        if z < mz_i - 1 {
            add_candidate(&mut candidates, 0, 0, 1);
        }

        // Pick random candidate using C#-compatible Random.Next(count)
        // C# Reference: candidates.Random(random) calls random.Next(candidates.Count)
        if candidates.is_empty() {
            return (0, 0, 0);
        }
        let idx = random.next_int_max(candidates.len() as i32) as usize;
        candidates[idx]
    } else {
        // With edges/vertices: collect all valid moves
        for (ddx, ddy, ddz) in directions(x, y, z, mx, my, mz, edges, vertices) {
            add_candidate(&mut candidates, ddx, ddy, ddz);
        }

        if candidates.is_empty() {
            return (0, 0, 0);
        }

        // With inertia, prefer direction with max cosine similarity
        if inertia && (dx != 0 || dy != 0 || dz != 0) {
            let mut max_scalar = -4.0f64;
            let mut result = candidates[0];

            for &(cdx, cdy, cdz) in &candidates {
                // C# uses random.NextDouble() for noise
                let noise = 0.1 * random.next_double();
                let dot = (cdx * dx + cdy * dy + cdz * dz) as f64;
                let len_c = ((cdx * cdx + cdy * cdy + cdz * cdz) as f64).sqrt();
                let len_d = ((dx * dx + dy * dy + dz * dz) as f64).sqrt();
                let cos = dot / (len_c * len_d);

                if cos + noise > max_scalar {
                    max_scalar = cos + noise;
                    result = (cdx, cdy, cdz);
                }
            }
            result
        } else {
            // C# Reference: candidates.Random(random) calls random.Next(candidates.Count)
            let idx = random.next_int_max(candidates.len() as i32) as usize;
            candidates[idx]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::rng::StdRandom;
    use crate::markov_junior::MjGrid;
    use rand::SeedableRng;

    #[test]
    fn test_path_node_simple() {
        // 5x5 grid
        // S at (0,0), F at (4,4), substrate=B everywhere else
        // B=0, S=1, F=2, P=3 (path)
        let mut grid = MjGrid::with_values(5, 5, 1, "BSFP");

        // Set start and finish
        grid.state[0] = 1; // S at (0,0)
        grid.state[4 + 4 * 5] = 2; // F at (4,4)

        // PathNode: start=S(bit 1), finish=F(bit 2), substrate=B(bit 0), write=P(3)
        let mut node = PathNode::new(0b0010, 0b0100, 0b0001, 3);

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Run pathfinding
        assert!(node.go(&mut ctx), "Should find a path");

        // Path should exist (P cells should be present)
        let p_count = ctx.grid.state.iter().filter(|&&v| v == 3).count();
        assert!(p_count > 0, "Should have drawn path cells");

        // Path length should be reasonable (Manhattan distance is 8)
        assert!(
            p_count >= 6 && p_count <= 10,
            "Path length should be close to Manhattan distance. Got: {}",
            p_count
        );
    }

    #[test]
    fn test_path_node_no_path() {
        // Grid with wall blocking all paths
        // B=0 (substrate), S=1, F=2, W=3 (wall)
        let mut grid = MjGrid::with_values(5, 5, 1, "BSFW");

        grid.state[0] = 1; // S at (0,0)
        grid.state[4 + 4 * 5] = 2; // F at (4,4)

        // Block with wall at x=2
        for y in 0..5 {
            grid.state[2 + y * 5] = 3; // W
        }

        // substrate=B (not W), so wall blocks path
        let mut node = PathNode::new(0b0010, 0b0100, 0b0001, 0);

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Should fail - no path
        assert!(!node.go(&mut ctx), "Should fail when path is blocked");
    }

    #[test]
    fn test_path_node_with_inertia() {
        // Test that inertia creates straighter paths
        let mut grid = MjGrid::with_values(10, 10, 1, "BSFP");

        grid.state[0] = 1; // S at (0,0)
        grid.state[9 + 9 * 10] = 2; // F at (9,9)

        let mut node = PathNode::new(0b0010, 0b0100, 0b0001, 3);
        node.inertia = true;

        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        assert!(node.go(&mut ctx), "Should find a path");

        let p_count = ctx.grid.state.iter().filter(|&&v| v == 3).count();
        assert!(p_count > 0, "Should have drawn path cells");
    }

    #[test]
    fn test_directions_2d() {
        // Test 2D cardinal directions
        let dirs = directions(2, 2, 0, 5, 5, 1, false, false);
        assert_eq!(dirs.len(), 4, "Should have 4 cardinal directions");

        // Test 2D with edges (diagonals)
        let dirs_edge = directions(2, 2, 0, 5, 5, 1, true, false);
        assert_eq!(dirs_edge.len(), 8, "Should have 8 directions with edges");

        // Test corner (fewer directions)
        let corner = directions(0, 0, 0, 5, 5, 1, false, false);
        assert_eq!(corner.len(), 2, "Corner should have 2 directions");
    }

    #[test]
    fn test_directions_3d() {
        let dirs = directions(1, 1, 1, 3, 3, 3, false, false);
        assert_eq!(dirs.len(), 6, "3D center should have 6 cardinal directions");

        let dirs_edge = directions(1, 1, 1, 3, 3, 3, true, false);
        assert_eq!(
            dirs_edge.len(),
            18,
            "3D with edges should have 18 directions"
        );

        let dirs_vert = directions(1, 1, 1, 3, 3, 3, true, true);
        assert_eq!(
            dirs_vert.len(),
            26,
            "3D with vertices should have 26 directions"
        );
    }
}
