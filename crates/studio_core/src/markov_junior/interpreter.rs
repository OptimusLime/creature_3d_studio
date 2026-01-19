//! Interpreter for running MarkovJunior models.
//!
//! The interpreter coordinates model execution, managing:
//! - Grid state initialization and reset
//! - RNG for deterministic execution with seeds
//! - Step-by-step or full execution modes
//! - Change tracking for incremental matching
//!
//! C# Reference: Interpreter.cs (87 lines)

use super::node::{ExecutionContext, Node};
use super::rng::{MjRng, StdRandom};
use super::MjGrid;

/// Main interpreter for running MarkovJunior models.
///
/// # Example
///
/// ```ignore
/// use studio_core::markov_junior::{Interpreter, MjGrid, MarkovNode, OneNode};
///
/// let grid = MjGrid::with_values(10, 10, 1, "BW");
/// let node = Box::new(MarkovNode::new(vec![/* rules */]));
/// let mut interp = Interpreter::new(node, grid);
///
/// // Run to completion with seed 42
/// let steps = interp.run(42, 0); // 0 = no limit
///
/// // Or step through manually
/// interp.reset(42);
/// while interp.step() {
///     // Observe grid state each step
/// }
/// ```
///
/// C# Reference: Interpreter.cs lines 7-20
/// ```csharp
/// class Interpreter {
///     public Branch root, current;
///     public Grid grid;
///     Grid startgrid;
///     bool origin;
///     public Random random;
///     public List<(int, int, int)> changes;
///     public List<int> first;
///     public int counter;
/// }
/// ```
pub struct Interpreter {
    /// The root node of the model
    root: Box<dyn Node>,
    /// The grid being modified
    grid: MjGrid,
    /// Random number generator (deterministic with seed).
    /// Uses boxed MjRng trait to support both StdRandom and DotNetRandom.
    random: Box<dyn MjRng>,
    /// Whether to set origin (center cell = 1) on reset
    origin: bool,
    /// List of (x, y, z) positions that changed
    changes: Vec<(i32, i32, i32)>,
    /// Index into changes where each turn's changes start
    first: Vec<usize>,
    /// Current step counter
    counter: usize,
    /// Whether the model is still running (root.go() returned true last step)
    running: bool,
    /// Whether to update grid state incrementally during execution (for animation).
    /// When true, nodes like WFC will update the grid after each step.
    /// When false, updates only happen when nodes complete.
    animated: bool,
}

impl Interpreter {
    /// Create a new interpreter with a root node and grid.
    ///
    /// The interpreter starts in an uninitialized state. Call `reset(seed)` or
    /// `run(seed, max_steps)` to begin execution.
    pub fn new(root: Box<dyn Node>, grid: MjGrid) -> Self {
        Self {
            root,
            grid,
            random: Box::new(StdRandom::from_u64_seed(0)),
            origin: false,
            changes: Vec::new(),
            first: vec![0],
            counter: 0,
            running: false,
            animated: false,
        }
    }

    /// Create an interpreter with the origin flag set.
    ///
    /// When origin is true, `reset()` will set the center cell to value 1.
    /// This is used by growth models that start from a seed point.
    ///
    /// C# Reference: Interpreter.cs line 57
    /// ```csharp
    /// if (origin) grid.state[grid.MX / 2 + (grid.MY / 2) * grid.MX + (grid.MZ / 2) * grid.MX * grid.MY] = 1;
    /// ```
    pub fn with_origin(root: Box<dyn Node>, grid: MjGrid) -> Self {
        Self {
            root,
            grid,
            random: Box::new(StdRandom::from_u64_seed(0)),
            origin: true,
            changes: Vec::new(),
            first: vec![0],
            counter: 0,
            running: false,
            animated: false,
        }
    }

    /// Enable or disable animated mode.
    ///
    /// When animated is true, the grid state is updated incrementally during execution,
    /// allowing visualization of the generation process step-by-step.
    pub fn set_animated(&mut self, animated: bool) {
        self.animated = animated;
    }

    /// Check if animated mode is enabled.
    pub fn is_animated(&self) -> bool {
        self.animated
    }

    /// Reset the interpreter for a new run with the given seed.
    ///
    /// This clears the grid, resets the RNG, and prepares for execution.
    /// If origin is set, the center cell is set to value 1.
    ///
    /// C# Reference: Interpreter.cs lines 54-64
    pub fn reset(&mut self, seed: u64) {
        // Reset RNG with new seed
        self.random = Box::new(StdRandom::from_u64_seed(seed));

        // Clear grid state
        self.grid.clear();

        // Set origin if enabled
        if self.origin {
            let center = self.grid.mx / 2
                + (self.grid.my / 2) * self.grid.mx
                + (self.grid.mz / 2) * self.grid.mx * self.grid.my;
            self.grid.state[center] = 1;
        }

        // Clear change tracking
        self.changes.clear();
        self.first.clear();
        self.first.push(0);

        // Reset the root node
        self.root.reset();

        // Reset counter and mark as running
        self.counter = 0;
        self.running = true;
    }

    /// Reset the interpreter with a custom RNG.
    ///
    /// This allows using DotNetRandom for C# compatibility verification.
    ///
    /// # Example
    /// ```ignore
    /// use studio_core::markov_junior::{Interpreter, DotNetRandom};
    ///
    /// let mut interp = Interpreter::new(node, grid);
    /// interp.reset_with_rng(Box::new(DotNetRandom::from_seed(42)));
    /// ```
    pub fn reset_with_rng(&mut self, rng: Box<dyn MjRng>) {
        self.random = rng;

        // Clear grid state
        self.grid.clear();

        // Set origin if enabled
        if self.origin {
            let center = self.grid.mx / 2
                + (self.grid.my / 2) * self.grid.mx
                + (self.grid.mz / 2) * self.grid.mx * self.grid.my;
            self.grid.state[center] = 1;
        }

        // Clear change tracking
        self.changes.clear();
        self.first.clear();
        self.first.push(0);

        // Reset the root node
        self.root.reset();

        // Reset counter and mark as running
        self.counter = 0;
        self.running = true;
    }

    /// Execute a single step of the model.
    ///
    /// Returns `true` if the model is still running (current != null in C# terms).
    /// Returns `false` if the model is done.
    ///
    /// IMPORTANT: In C#, counter++ happens on EVERY iteration, not just when
    /// Go() returns true. The loop only stops when `current` becomes null
    /// (which happens when root fails and sets ip.current = ip.current.parent = null).
    ///
    /// C# Reference: Interpreter.cs lines 68-78
    /// ```csharp
    /// while (current != null && (steps <= 0 || counter < steps)) {
    ///     current.Go();
    ///     counter++;  // <-- ALWAYS increments
    ///     first.Add(changes.Count);
    /// }
    /// ```
    pub fn step(&mut self) -> bool {
        if !self.running {
            return false;
        }

        // Create execution context for this step
        // We need to temporarily take ownership of grid and random
        let mut ctx = ExecutionContext {
            grid: &mut self.grid,
            random: self.random.as_mut(),
            changes: std::mem::take(&mut self.changes),
            first: std::mem::take(&mut self.first),
            counter: self.counter,
            gif: self.animated, // Enable incremental updates when animated
        };

        // Execute one step
        let still_running = self.root.go(&mut ctx);

        // Restore state from context
        self.changes = ctx.changes;
        self.first = ctx.first;

        // C# always increments counter, regardless of Go() return value
        self.counter += 1;
        self.first.push(self.changes.len());

        if !still_running {
            // Root returned false, meaning ip.current would be set to null in C#
            self.running = false;
        }

        still_running
    }

    /// Run the model to completion or until max_steps.
    ///
    /// If `max_steps` is 0, runs until the model completes.
    /// Returns the number of steps executed.
    ///
    /// C# Reference: Interpreter.cs lines 52-79
    pub fn run(&mut self, seed: u64, max_steps: usize) -> usize {
        self.reset(seed);

        while self.running && (max_steps == 0 || self.counter < max_steps) {
            if !self.step() {
                break;
            }
        }

        self.counter
    }

    /// Get a reference to the current grid state.
    pub fn grid(&self) -> &MjGrid {
        &self.grid
    }

    /// Get the number of steps executed since the last reset.
    pub fn counter(&self) -> usize {
        self.counter
    }

    /// Check if the model is still running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get the list of all changes made during execution.
    pub fn changes(&self) -> &[(i32, i32, i32)] {
        &self.changes
    }

    /// Get the list of change indices marking the start of each step's changes.
    ///
    /// The `first` array contains indices into `changes`. The changes for step N
    /// are `changes[first[N]..first[N+1]]`.
    ///
    /// C# Reference: Interpreter.cs line 17: public List<int> first;
    pub fn first(&self) -> &[usize] {
        &self.first
    }

    /// Get the number of cells changed in the last step.
    ///
    /// Returns 0 if no steps have been executed or if the last step made no changes.
    pub fn last_step_change_count(&self) -> usize {
        if self.first.len() < 2 {
            return 0;
        }
        let last_idx = self.first.len() - 1;
        let prev_idx = last_idx - 1;
        self.first[last_idx].saturating_sub(self.first[prev_idx])
    }

    /// Get the positions of cells changed in the last step.
    ///
    /// Returns an empty slice if no steps have been executed or if the last step
    /// made no changes.
    pub fn last_step_changes(&self) -> &[(i32, i32, i32)] {
        if self.first.len() < 2 {
            return &[];
        }
        let last_idx = self.first.len() - 1;
        let prev_idx = last_idx - 1;
        let start = self.first[prev_idx];
        let end = self.first[last_idx];
        &self.changes[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::{MarkovNode, MjRule, OneNode};

    /// Test that step() returns false when the model is done.
    #[test]
    fn test_interpreter_step_returns_false_when_done() {
        // Create a 3x1 grid of B's with rule B -> W
        let grid = MjGrid::with_values(3, 1, 1, "BW");
        let grid_size = grid.state.len();
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let one_node = OneNode::new(vec![rule], grid_size);
        let markov = MarkovNode::new(vec![Box::new(one_node)]);

        let mut interp = Interpreter::new(Box::new(markov), grid);
        interp.reset(42);

        // Should take 3 steps to convert all B's to W's
        assert!(interp.step()); // 1 B -> W
        assert!(interp.step()); // 2 B -> W
        assert!(interp.step()); // 3 B -> W

        // Now done - no more B's to convert
        assert!(!interp.step());
        assert!(!interp.step()); // Still false

        // Verify grid is all W's
        assert!(interp.grid().state.iter().all(|&v| v == 1));
    }

    /// Test that run() stops after exactly max_steps.
    #[test]
    fn test_interpreter_run_with_max_steps() {
        // Create a 10x1 grid of B's
        let grid = MjGrid::with_values(10, 1, 1, "BW");
        let grid_size = grid.state.len();
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let one_node = OneNode::new(vec![rule], grid_size);
        let markov = MarkovNode::new(vec![Box::new(one_node)]);

        let mut interp = Interpreter::new(Box::new(markov), grid);

        // Run for exactly 5 steps
        let steps = interp.run(42, 5);
        assert_eq!(steps, 5);

        // Should have exactly 5 W cells
        let w_count = interp.grid().state.iter().filter(|&&v| v == 1).count();
        assert_eq!(w_count, 5);
    }

    /// Test that with_origin sets the center cell to 1 after reset.
    #[test]
    fn test_interpreter_origin_sets_center() {
        // Create a 5x5x1 grid (center = 2,2,0 = index 12)
        let grid = MjGrid::with_values(5, 5, 1, "BW");

        // Create a no-op node that always returns false
        struct NoOpNode;
        impl Node for NoOpNode {
            fn go(&mut self, _ctx: &mut ExecutionContext) -> bool {
                false
            }
            fn reset(&mut self) {}
        }

        let mut interp = Interpreter::with_origin(Box::new(NoOpNode), grid);
        interp.reset(42);

        // Center should be 1, all others 0
        let center_idx = 5 / 2 + (5 / 2) * 5; // = 2 + 2*5 = 12
        assert_eq!(interp.grid().state[center_idx], 1);
        assert_eq!(
            interp.grid().state.iter().filter(|&&v| v == 1).count(),
            1,
            "Only center should be 1"
        );
    }

    /// Test that reset() clears state and counter.
    #[test]
    fn test_interpreter_reset_clears_state() {
        // Create a 5x1 grid
        let grid = MjGrid::with_values(5, 1, 1, "BW");
        let grid_size = grid.state.len();
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let one_node = OneNode::new(vec![rule], grid_size);
        let markov = MarkovNode::new(vec![Box::new(one_node)]);

        let mut interp = Interpreter::new(Box::new(markov), grid);

        // Run partially
        interp.run(42, 3);
        assert_eq!(interp.counter(), 3);
        assert!(interp.grid().state.iter().any(|&v| v == 1));

        // Reset with new seed
        interp.reset(999);
        assert_eq!(interp.counter(), 0);
        assert!(
            interp.grid().state.iter().all(|&v| v == 0),
            "Grid should be cleared"
        );
        assert!(interp.is_running());
    }

    /// Placeholder test for C# cross-validation.
    /// This will be filled in once we have reference data from C# implementation.
    #[test]
    fn test_basic_model_matches_reference() {
        // TODO: Generate reference data from C# MarkovJunior:
        // cd MarkovJunior && dotnet run -- Basic 12345 --dump-state
        //
        // For now, just verify we can create and run a basic model
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let grid_size = grid.state.len();
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let one_node = OneNode::new(vec![rule], grid_size);
        let markov = MarkovNode::new(vec![Box::new(one_node)]);

        let mut interp = Interpreter::new(Box::new(markov), grid);
        let steps = interp.run(12345, 0);

        // Should complete in exactly 25 steps (5x5 grid)
        assert_eq!(steps, 25);
        assert!(interp.grid().state.iter().all(|&v| v == 1));
    }
}
