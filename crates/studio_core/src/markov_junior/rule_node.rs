//! RuleNode - Base implementation for rule-based nodes.
//!
//! RuleNode provides match tracking, incremental pattern matching,
//! and the core Go() logic shared by OneNode, AllNode, and ParallelNode.
//!
//! C# Reference: RuleNode.cs

use super::field::Field;
use super::node::ExecutionContext;
use super::observation::Observation;
use super::search::run_search;
use super::MjRule;

/// A match: (rule_index, flat_grid_index) where rule can be applied.
///
/// Using flat indices rather than coordinates allows grid-agnostic match storage.
/// Callers convert to coordinates as needed for rule application.
pub type Match = (usize, usize);

/// Base data and logic for rule-based nodes.
///
/// This is NOT a trait - it's a struct that OneNode/AllNode/ParallelNode
/// compose with to share match-tracking logic.
///
/// C# Reference: RuleNode.cs lines 8-30
pub struct RuleNodeData {
    /// The rules this node can apply
    pub rules: Vec<MjRule>,
    /// Step counter for this node
    pub counter: usize,
    /// Maximum steps (0 = unlimited)
    pub steps: usize,
    /// List of valid matches: (rule_index, flat_grid_index)
    pub matches: Vec<Match>,
    /// Number of valid matches (matches may have stale entries beyond this)
    pub match_count: usize,
    /// Turn when we last computed matches (-1 = never)
    pub last_matched_turn: i32,
    /// Deduplication mask: match_mask[rule_idx][grid_idx] = already tracked
    pub match_mask: Vec<Vec<bool>>,
    /// Which rules were applied last step
    pub last: Vec<bool>,

    // --- Phase 1.5: Heuristic selection fields ---
    /// Distance potentials per color: potentials[color][grid_idx]
    /// Used for heuristic-guided rule selection.
    /// C# Reference: RuleNode.cs line 17
    pub potentials: Option<Vec<Vec<i32>>>,
    /// Field configurations per color (for recomputation)
    /// C# Reference: RuleNode.cs line 18
    pub fields: Option<Vec<Option<Field>>>,
    /// Temperature for randomized heuristic selection.
    /// 0 = greedy (always pick best), >0 = probabilistic selection.
    /// C# Reference: RuleNode.cs line 20
    pub temperature: f64,

    // --- Phase 1.7: Observation and search fields ---
    /// Observation constraints per color value.
    /// observations[value] defines what that value should become.
    /// C# Reference: RuleNode.cs line 19
    pub observations: Option<Vec<Option<Observation>>>,
    /// Future constraints: future[grid_idx] = wave mask of allowed values.
    /// Computed from observations at the start of execution.
    /// C# Reference: RuleNode.cs line 21
    pub future: Option<Vec<i32>>,
    /// Whether to use A* search mode.
    /// C# Reference: RuleNode.cs line 22
    pub search: bool,
    /// Maximum states to explore in search (-1 = unlimited).
    /// C# Reference: RuleNode.cs line 23
    pub limit: i32,
    /// Depth coefficient for search ranking.
    /// C# Reference: RuleNode.cs line 24
    pub depth_coefficient: f64,
    /// Pre-computed trajectory from search (for replay).
    /// C# Reference: RuleNode.cs line 25
    pub trajectory: Option<Vec<Vec<u8>>>,
    /// Current position in trajectory during replay.
    pub trajectory_index: usize,
    /// Whether future constraints have been computed from observations.
    /// C# Reference: RuleNode.cs line 22 (futureComputed)
    pub future_computed: bool,
}

impl RuleNodeData {
    /// Create a new RuleNodeData with the given rules and grid size.
    pub fn new(rules: Vec<MjRule>, grid_size: usize) -> Self {
        let num_rules = rules.len();
        Self {
            rules,
            counter: 0,
            steps: 0,
            matches: Vec::new(),
            match_count: 0,
            last_matched_turn: -1,
            match_mask: vec![vec![false; grid_size]; num_rules],
            last: vec![false; num_rules],
            potentials: None,
            fields: None,
            temperature: 0.0,
            observations: None,
            future: None,
            search: false,
            limit: -1,
            depth_coefficient: 0.0,
            trajectory: None,
            trajectory_index: 0,
            future_computed: false,
        }
    }

    /// Create RuleNodeData with field-based heuristics.
    ///
    /// `fields` is indexed by color value: fields[color] = Some(Field) or None.
    /// `num_colors` is the total number of colors (grid.c).
    pub fn with_fields(
        rules: Vec<MjRule>,
        grid_size: usize,
        fields: Vec<Option<Field>>,
        temperature: f64,
    ) -> Self {
        let num_rules = rules.len();
        let num_colors = fields.len();

        // Initialize potentials for each color
        let potentials = vec![vec![0i32; grid_size]; num_colors];

        Self {
            rules,
            counter: 0,
            steps: 0,
            matches: Vec::new(),
            match_count: 0,
            last_matched_turn: -1,
            match_mask: vec![vec![false; grid_size]; num_rules],
            last: vec![false; num_rules],
            potentials: Some(potentials),
            fields: Some(fields),
            temperature,
            observations: None,
            future: None,
            search: false,
            limit: -1,
            depth_coefficient: 0.0,
            trajectory: None,
            trajectory_index: 0,
            future_computed: false,
        }
    }

    /// Check if this node has heuristic fields configured.
    pub fn has_fields(&self) -> bool {
        self.fields.is_some()
    }

    /// Check if this node has observations configured.
    pub fn has_observations(&self) -> bool {
        self.observations.is_some()
    }

    /// Set observations for this node.
    ///
    /// When observations are set (without search mode), potentials are also
    /// initialized for backward potential computation.
    /// C# Reference: RuleNode.Load() lines 81-89
    pub fn set_observations(
        &mut self,
        observations: Vec<Option<Observation>>,
        grid_size: usize,
        num_colors: usize,
    ) {
        self.observations = Some(observations);
        self.future = Some(vec![0; grid_size]);

        // In non-search mode, initialize potentials for backward computation
        // C# Reference: "else potentials = AH.Array2D(grid.C, grid.state.Length, 0);"
        if !self.search && self.potentials.is_none() {
            self.potentials = Some(vec![vec![0i32; grid_size]; num_colors]);
        }
    }

    /// Configure search parameters.
    pub fn set_search(&mut self, search: bool, limit: i32, depth_coefficient: f64) {
        self.search = search;
        self.limit = limit;
        self.depth_coefficient = depth_coefficient;
    }

    /// Reset state for a new run.
    ///
    /// C# Reference: RuleNode.Reset() lines 105-112
    pub fn reset(&mut self) {
        self.last_matched_turn = -1;
        self.counter = 0;
        self.trajectory = None;
        self.trajectory_index = 0;
        self.future_computed = false;

        for r in 0..self.last.len() {
            self.last[r] = false;
        }
    }

    /// Clear the match mask (used when resetting matches).
    pub fn clear_match_mask(&mut self) {
        for mask in &mut self.match_mask {
            mask.fill(false);
        }
        self.match_count = 0;
    }

    /// Add a match if not already tracked.
    ///
    /// C# Reference: RuleNode.Add() lines 114-122
    ///
    /// # Arguments
    /// * `r` - Rule index
    /// * `idx` - Flat grid index where the rule can be applied
    pub fn add_match(&mut self, r: usize, idx: usize) {
        let mask = &mut self.match_mask[r];

        if !mask[idx] {
            mask[idx] = true;
            let m = (r, idx);
            if self.match_count < self.matches.len() {
                self.matches[self.match_count] = m;
            } else {
                self.matches.push(m);
            }
            self.match_count += 1;
        }
    }

    /// Scan the entire grid for matches (full scan).
    ///
    /// C# Reference: RuleNode.Go() lines 176-197 (the else branch)
    ///
    /// The C# code uses a stride optimization where it samples grid cells at
    /// Scan for all matches using C#'s strided scan with ishifts.
    ///
    /// C# scans the grid at intervals of rule.IMX/IMY/IMZ, then uses ishifts
    /// to find candidate rule positions. This ensures matches are found in
    /// the same ORDER as C# for shuffle compatibility.
    ///
    /// C# Reference: RuleNode.Go() lines 173-199 (the else branch)
    pub fn scan_all_matches(&mut self, ctx: &ExecutionContext) {
        self.match_count = 0;
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;
        let mz = ctx.grid.mz;

        for (r, rule) in self.rules.iter().enumerate() {
            let mask = &mut self.match_mask[r];

            // C# uses strided scan starting at (IMX-1, IMY-1, IMZ-1)
            // stepping by (IMX, IMY, IMZ)
            let mut z = (rule.imz - 1) as i32;
            while z < mz as i32 {
                let mut y = (rule.imy - 1) as i32;
                while y < my as i32 {
                    let mut x = (rule.imx - 1) as i32;
                    while x < mx as i32 {
                        // Get grid value at (x, y, z)
                        let grid_idx = x as usize + y as usize * mx + z as usize * mx * my;
                        let value = ctx.grid.state[grid_idx] as usize;

                        // Use ishifts to find candidate match positions
                        if value < rule.ishifts.len() {
                            for &(shiftx, shifty, shiftz) in &rule.ishifts[value] {
                                let sx = x - shiftx;
                                let sy = y - shifty;
                                let sz = z - shiftz;

                                // Bounds check - use max of input and output dimensions
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

                                let si = sx as usize + sy as usize * mx + sz as usize * mx * my;

                                // C# full scan (RuleNode.Go line 194) does NOT check mask.
                                // The mask is only used for deduplication in incremental scan.
                                // Full scan visits each position exactly once per rule.
                                if ctx.grid.matches(rule, sx, sy, sz) {
                                    mask[si] = true;
                                    let m = (r, si);
                                    if self.match_count < self.matches.len() {
                                        self.matches[self.match_count] = m;
                                    } else {
                                        self.matches.push(m);
                                    }
                                    self.match_count += 1;
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
    }

    /// Scan for matches incrementally based on recent changes.
    ///
    /// C# Reference: RuleNode.Go() lines 148-172 (the if lastMatchedTurn >= 0 branch)
    pub fn scan_incremental_matches(&mut self, ctx: &ExecutionContext) {
        let mx = ctx.grid.mx;
        let my = ctx.grid.my;
        let mz = ctx.grid.mz;

        // Get changes since last matched turn
        let start_idx = ctx
            .first
            .get(self.last_matched_turn as usize)
            .copied()
            .unwrap_or(0);

        for n in start_idx..ctx.changes.len() {
            let grid_idx = ctx.changes[n];
            let (x, y, z) = ctx.grid.index_to_coord(grid_idx);
            let value = ctx.grid.state[grid_idx];

            for (r, rule) in self.rules.iter().enumerate() {
                let mask = &mut self.match_mask[r];

                // Use ishifts to find candidate positions affected by this change
                if (value as usize) < rule.ishifts.len() {
                    for &(shiftx, shifty, shiftz) in &rule.ishifts[value as usize] {
                        let sx = x - shiftx;
                        let sy = y - shifty;
                        let sz = z - shiftz;

                        // Bounds check - use max of input and output dimensions
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

                        let si = sx as usize + sy as usize * mx + sz as usize * mx * my;

                        if !mask[si] && ctx.grid.matches(rule, sx, sy, sz) {
                            mask[si] = true;
                            let m = (r, si);
                            if self.match_count < self.matches.len() {
                                self.matches[self.match_count] = m;
                            } else {
                                self.matches.push(m);
                            }
                            self.match_count += 1;
                        }
                    }
                }
            }
        }
    }

    /// Common Go() logic - scan for matches and recompute fields.
    ///
    /// Returns false if steps limit reached or essential field computation fails.
    /// After calling, matches are populated and ready for node-specific logic.
    ///
    /// # Arguments
    /// * `ctx` - Execution context with grid and RNG
    /// * `is_all` - True if this is an AllNode (affects search behavior)
    ///
    /// C# Reference: RuleNode.Go() lines 124-217
    pub fn compute_matches(&mut self, ctx: &mut ExecutionContext, is_all: bool) -> bool {
        // Clear last flags
        for r in 0..self.last.len() {
            self.last[r] = false;
        }

        // Check step limit
        if self.steps > 0 && self.counter >= self.steps {
            return false;
        }

        // Handle observation initialization (one-time at start)
        // C# Reference: RuleNode.Go() lines 131-146
        if self.observations.is_some() && !self.future_computed {
            let (future, observations) = match (self.future.as_mut(), self.observations.as_ref()) {
                (Some(f), Some(o)) => (f, o),
                _ => return false,
            };

            // Compute future constraints and modify state
            if !Observation::compute_future_set_present(future, &mut ctx.grid.state, observations) {
                return false;
            }

            self.future_computed = true;

            // C# Reference: RuleNode.Go() lines 137-144
            if self.search {
                // Run A* search to find trajectory
                self.trajectory = None;
                let tries = if self.limit < 0 { 1 } else { 20 };
                for _ in 0..tries {
                    if self.trajectory.is_some() {
                        break;
                    }
                    // C# uses ip.random.Next() which returns int (i32)
                    let seed = ctx.random.next_int();
                    self.trajectory = run_search(
                        &ctx.grid.state,
                        future,
                        &self.rules,
                        ctx.grid.mx,
                        ctx.grid.my,
                        ctx.grid.mz,
                        ctx.grid.c as usize,
                        is_all,
                        self.limit,
                        self.depth_coefficient,
                        seed,
                    );
                }
                if self.trajectory.is_none() {
                    eprintln!("SEARCH RETURNED NULL");
                }
            } else {
                // In non-search mode with observations, compute backward potentials
                // These guide heuristic selection toward the goal state
                if let Some(ref mut potentials) = self.potentials {
                    let mx = ctx.grid.mx;
                    let my = ctx.grid.my;
                    let mz = ctx.grid.mz;
                    Observation::compute_backward_potentials(
                        potentials,
                        future,
                        mx,
                        my,
                        mz,
                        &self.rules,
                    );
                }
            }
        }

        // Scan for matches (incremental or full)
        if self.last_matched_turn >= 0 {
            self.scan_incremental_matches(ctx);
        } else {
            self.scan_all_matches(ctx);
        }

        // Recompute fields if configured
        // C# Reference: RuleNode.Go() lines 200-215
        if let (Some(ref mut potentials), Some(ref fields)) = (&mut self.potentials, &self.fields) {
            let mut any_success = false;
            let mut any_computation = false;

            for c in 0..fields.len() {
                if let Some(ref field) = fields[c] {
                    // Recompute if first step or field.recompute is true
                    if self.counter == 0 || field.recompute {
                        let success = field.compute(&mut potentials[c], ctx.grid);
                        if !success && field.essential {
                            return false;
                        }
                        any_success |= success;
                        any_computation = true;
                    }
                }
            }

            // If we computed any fields but none succeeded, fail
            if any_computation && !any_success {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::rng::StdRandom;
    use crate::markov_junior::MjGrid;
    use rand::SeedableRng;

    #[test]
    fn test_rule_node_data_scan_matches() {
        let grid = MjGrid::with_values(5, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut data = RuleNodeData::new(vec![rule], grid.state.len());

        let mut grid = grid;
        let mut rng = StdRandom::from_u64_seed(42);
        let ctx = ExecutionContext::new(&mut grid, &mut rng);

        data.scan_all_matches(&ctx);

        // All 5 cells are B, so we should have 5 matches
        assert_eq!(data.match_count, 5);
    }

    #[test]
    fn test_rule_node_data_add_match_deduplication() {
        let grid = MjGrid::with_values(5, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut data = RuleNodeData::new(vec![rule], grid.state.len());

        // Add same match twice (idx 2 corresponds to x=2, y=0, z=0 in a 5x1x1 grid)
        data.add_match(0, 2);
        data.add_match(0, 2);

        // Should only count once due to mask
        assert_eq!(data.match_count, 1);
    }

    #[test]
    fn test_rule_node_data_reset() {
        let grid = MjGrid::with_values(5, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut data = RuleNodeData::new(vec![rule], grid.state.len());

        data.counter = 10;
        data.last_matched_turn = 5;
        data.last[0] = true;

        data.reset();

        assert_eq!(data.counter, 0);
        assert_eq!(data.last_matched_turn, -1);
        assert!(!data.last[0]);
    }
}
