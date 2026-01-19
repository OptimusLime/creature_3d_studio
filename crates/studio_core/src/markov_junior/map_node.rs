//! MapNode - Grid transformation node.
//!
//! MapNode transforms the grid from one size to another by applying
//! mapping rules. It's used for scaling operations like upscaling
//! a low-res grid to high-res.
//!
//! C# Reference: Map.cs

use super::node::{ExecutionContext, MjNodeStructure, Node};
use super::rule::MjRule;
use super::MjGrid;
use serde_json::json;

/// Scale factor as numerator/denominator pair.
#[derive(Debug, Clone, Copy)]
pub struct ScaleFactor {
    pub numerator: i32,
    pub denominator: i32,
}

impl ScaleFactor {
    pub fn new(n: i32, d: i32) -> Self {
        Self {
            numerator: n,
            denominator: d,
        }
    }

    pub fn from_int(n: i32) -> Self {
        Self {
            numerator: n,
            denominator: 1,
        }
    }

    /// Parse a scale factor from string like "2" or "1/2"
    pub fn parse(s: &str) -> Option<Self> {
        if s.contains('/') {
            let parts: Vec<&str> = s.split('/').collect();
            if parts.len() != 2 {
                return None;
            }
            let n = parts[0].parse().ok()?;
            let d = parts[1].parse().ok()?;
            Some(Self::new(n, d))
        } else {
            let n = s.parse().ok()?;
            Some(Self::from_int(n))
        }
    }

    /// Apply scale factor to a dimension.
    pub fn apply(&self, dim: usize) -> usize {
        ((dim as i32) * self.numerator / self.denominator) as usize
    }
}

/// MapNode transforms the grid by applying mapping rules.
///
/// The transformation process:
/// 1. On first `Go()`: Clear newgrid, apply mapping rules to create scaled output
/// 2. On subsequent `Go()`: Execute child nodes on the newgrid
///
/// C# Reference: Map.cs
pub struct MapNode {
    /// Target grid (different size than input)
    pub newgrid: MjGrid,
    /// Rules that map from input grid patterns to output grid patterns
    pub rules: Vec<MjRule>,
    /// Scale factors (NX, NY, NZ) and divisors (DX, DY, DZ)
    pub scale_x: ScaleFactor,
    pub scale_y: ScaleFactor,
    pub scale_z: ScaleFactor,
    /// Child nodes to execute on the transformed grid
    pub children: Vec<Box<dyn Node>>,
    /// Current child index (-1 = need to apply mapping first)
    pub n: i32,
}

impl MapNode {
    /// Create a new MapNode with the given scale and rules.
    pub fn new(
        newgrid: MjGrid,
        rules: Vec<MjRule>,
        scale_x: ScaleFactor,
        scale_y: ScaleFactor,
        scale_z: ScaleFactor,
    ) -> Self {
        Self {
            newgrid,
            rules,
            scale_x,
            scale_y,
            scale_z,
            children: Vec::new(),
            n: -1,
        }
    }

    /// Add children to the MapNode.
    pub fn with_children(mut self, children: Vec<Box<dyn Node>>) -> Self {
        self.children = children;
        self
    }

    /// Check if a rule matches at position (x, y, z) in the source grid.
    ///
    /// C# Reference: Map.cs Matches() lines 59-76
    fn matches(
        rule: &MjRule,
        x: i32,
        y: i32,
        z: i32,
        state: &[u8],
        mx: usize,
        my: usize,
        mz: usize,
    ) -> bool {
        for dz in 0..rule.imz {
            for dy in 0..rule.imy {
                for dx in 0..rule.imx {
                    let mut sx = x as usize + dx;
                    let mut sy = y as usize + dy;
                    let mut sz = z as usize + dz;

                    // Wrap around (toroidal)
                    if sx >= mx {
                        sx -= mx;
                    }
                    if sy >= my {
                        sy -= my;
                    }
                    if sz >= mz {
                        sz -= mz;
                    }

                    let input_wave = rule.input[dx + dy * rule.imx + dz * rule.imx * rule.imy];
                    let state_value = state[sx + sy * mx + sz * mx * my];
                    if (input_wave & (1 << state_value)) == 0 {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Apply a rule's output at position (x, y, z) in the target grid.
    ///
    /// C# Reference: Map.cs Apply() lines 78-93
    fn apply(
        rule: &MjRule,
        x: i32,
        y: i32,
        z: i32,
        state: &mut [u8],
        mx: usize,
        my: usize,
        mz: usize,
    ) {
        for dz in 0..rule.omz {
            for dy in 0..rule.omy {
                for dx in 0..rule.omx {
                    let mut sx = x as usize + dx;
                    let mut sy = y as usize + dy;
                    let mut sz = z as usize + dz;

                    // Wrap around (toroidal)
                    if sx >= mx {
                        sx -= mx;
                    }
                    if sy >= my {
                        sy -= my;
                    }
                    if sz >= mz {
                        sz -= mz;
                    }

                    let output = rule.output[dx + dy * rule.omx + dz * rule.omx * rule.omy];
                    if output != 0xff {
                        state[sx + sy * mx + sz * mx * my] = output;
                    }
                }
            }
        }
    }
}

impl Node for MapNode {
    /// Reset the MapNode and all children.
    fn reset(&mut self) {
        self.n = -1;
        for child in &mut self.children {
            child.reset();
        }
    }

    /// Execute the MapNode.
    ///
    /// On first call (n == -1):
    /// 1. Clear the newgrid
    /// 2. For each position in source grid, apply matching rules to scaled position in newgrid
    /// 3. Replace context grid with newgrid
    ///
    /// On subsequent calls (n >= 0):
    /// Execute children on the newgrid
    ///
    /// C# Reference: Map.cs Go() lines 95-108
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // If we have children and n >= 0, execute them
        if self.n >= 0 {
            if self.children.is_empty() {
                return false;
            }

            // Execute current child
            while (self.n as usize) < self.children.len() {
                let child = &mut self.children[self.n as usize];
                if child.go(ctx) {
                    return true;
                }
                // Child completed, move to next
                self.n += 1;
                if (self.n as usize) < self.children.len() {
                    self.children[self.n as usize].reset();
                }
            }
            return false;
        }

        // First call: apply mapping transformation
        self.newgrid.clear();

        let src_mx = ctx.grid.mx;
        let src_my = ctx.grid.my;
        let src_mz = ctx.grid.mz;

        // Apply rules at each position
        for rule in &self.rules {
            for z in 0..src_mz as i32 {
                for y in 0..src_my as i32 {
                    for x in 0..src_mx as i32 {
                        if Self::matches(rule, x, y, z, &ctx.grid.state, src_mx, src_my, src_mz) {
                            // Calculate target position with scaling
                            let tx = x * self.scale_x.numerator / self.scale_x.denominator;
                            let ty = y * self.scale_y.numerator / self.scale_y.denominator;
                            let tz = z * self.scale_z.numerator / self.scale_z.denominator;

                            Self::apply(
                                rule,
                                tx,
                                ty,
                                tz,
                                &mut self.newgrid.state,
                                self.newgrid.mx,
                                self.newgrid.my,
                                self.newgrid.mz,
                            );
                        }
                    }
                }
            }
        }

        // Swap grids - the newgrid becomes the active grid
        std::mem::swap(&mut self.newgrid, ctx.grid);

        // Move to child execution phase
        self.n = 0;
        if !self.children.is_empty() {
            self.children[0].reset();
        }
        true
    }

    fn structure(&self) -> MjNodeStructure {
        MjNodeStructure::new("Map")
            .with_children(self.children.iter().map(|c| c.structure()).collect())
            .with_config(json!({
                "scale_x": format!("{}/{}", self.scale_x.numerator, self.scale_x.denominator),
                "scale_y": format!("{}/{}", self.scale_y.numerator, self.scale_y.denominator),
                "scale_z": format!("{}/{}", self.scale_z.numerator, self.scale_z.denominator),
                "rules_count": self.rules.len(),
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::rng::StdRandom;
    use rand::SeedableRng;

    #[test]
    fn test_scale_factor_parse_integer() {
        let sf = ScaleFactor::parse("2").unwrap();
        assert_eq!(sf.numerator, 2);
        assert_eq!(sf.denominator, 1);
        assert_eq!(sf.apply(10), 20);
    }

    #[test]
    fn test_scale_factor_parse_fraction() {
        let sf = ScaleFactor::parse("1/2").unwrap();
        assert_eq!(sf.numerator, 1);
        assert_eq!(sf.denominator, 2);
        assert_eq!(sf.apply(10), 5);
    }

    #[test]
    fn test_scale_factor_apply() {
        let sf = ScaleFactor::new(3, 2);
        assert_eq!(sf.apply(10), 15);
    }

    #[test]
    fn test_map_node_simple_2x_scale() {
        // Create a 2x2 source grid with values "BW"
        let src_grid = MjGrid::with_values(2, 2, 1, "BW");

        // Create a 4x4 target grid
        let newgrid = MjGrid::with_values(4, 4, 1, "BW");

        // Create a simple rule: B -> B (just copy)
        let rule = MjRule::parse("B", "B", &src_grid).unwrap();

        let scale = ScaleFactor::from_int(2);
        let mut node = MapNode::new(newgrid, vec![rule], scale, scale, ScaleFactor::from_int(1));

        // Set up source grid: single B at (0,0)
        let mut grid = src_grid.clone();
        grid.set(0, 0, 0, 0); // B

        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);
        let result = node.go(&mut ctx);

        assert!(result, "MapNode should return true on first go");

        // After mapping, the grid should be 4x4
        assert_eq!(ctx.grid.mx, 4);
        assert_eq!(ctx.grid.my, 4);
    }

    #[test]
    fn test_map_node_matches() {
        let grid = MjGrid::with_values(4, 4, 1, "BW");
        let rule = MjRule::parse("BW", "WB", &grid).unwrap();

        // State with BW at (0,0) and (1,0)
        let mut state = vec![0u8; 16];
        state[0] = 0; // B at (0,0)
        state[1] = 1; // W at (1,0)

        // Should match at (0,0)
        assert!(MapNode::matches(&rule, 0, 0, 0, &state, 4, 4, 1));
        // Should not match at (1,0) - would be WB pattern but next is B
        state[2] = 0; // B at (2,0)
        assert!(!MapNode::matches(&rule, 1, 0, 0, &state, 4, 4, 1));
    }
}
