//! Node trait and branch nodes for MarkovJunior execution.
//!
//! This module defines the core execution model:
//! - `Node` trait with `go()` and `reset()` methods
//! - `SequenceNode` - runs children in order until all complete
//! - `MarkovNode` - loops children until none make progress
//!
//! C# Reference: Node.cs (lines 7-111)

use super::rng::MjRng;
use super::MjGrid;

/// Shared execution context passed to all nodes during execution.
///
/// Contains the grid, RNG, and change tracking for incremental matching.
pub struct ExecutionContext<'a> {
    /// The grid being modified
    pub grid: &'a mut MjGrid,
    /// Random number generator (deterministic with seed).
    /// Uses MjRng trait to support both StdRandom and DotNetRandom.
    pub random: &'a mut dyn MjRng,
    /// List of (x, y, z) positions that changed
    pub changes: Vec<(i32, i32, i32)>,
    /// Index into changes where each turn's changes start
    /// first[turn] = index of first change in that turn
    pub first: Vec<usize>,
    /// Current turn/step counter
    pub counter: usize,
    /// Whether to generate animation frames (update state after each step)
    pub gif: bool,
}

impl<'a> ExecutionContext<'a> {
    /// Create a new execution context.
    pub fn new(grid: &'a mut MjGrid, random: &'a mut dyn MjRng) -> Self {
        Self {
            grid,
            random,
            changes: Vec::new(),
            first: vec![0], // first[0] = 0, start of turn 0
            counter: 0,
            gif: false,
        }
    }

    /// Create a new execution context with gif mode enabled.
    pub fn with_gif(grid: &'a mut MjGrid, random: &'a mut dyn MjRng, gif: bool) -> Self {
        Self {
            grid,
            random,
            changes: Vec::new(),
            first: vec![0],
            counter: 0,
            gif,
        }
    }

    /// Record a change at the given position.
    #[inline]
    pub fn record_change(&mut self, x: i32, y: i32, z: i32) {
        self.changes.push((x, y, z));
    }

    /// Advance to the next turn, recording where this turn's changes end.
    pub fn next_turn(&mut self) {
        self.counter += 1;
        self.first.push(self.changes.len());
    }

    /// Get the changes from a specific turn.
    pub fn changes_from_turn(&self, turn: usize) -> &[(i32, i32, i32)] {
        let start = self.first.get(turn).copied().unwrap_or(0);
        let end = self
            .first
            .get(turn + 1)
            .copied()
            .unwrap_or(self.changes.len());
        &self.changes[start..end]
    }
}

/// The core Node trait for MarkovJunior execution.
///
/// C# Reference: Node.cs lines 7-12
/// ```csharp
/// abstract class Node
/// {
///     abstract public bool Go();
///     abstract public void Reset();
/// }
/// ```
pub trait Node {
    /// Execute one step of this node.
    ///
    /// Returns `true` if the node made progress (applied a rule, advanced state).
    /// Returns `false` if the node is done or cannot proceed.
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool;

    /// Reset this node to its initial state.
    fn reset(&mut self);

    /// Returns true if this node is a "branch" type (SequenceNode, MarkovNode).
    /// Branch nodes have special handling in parent branches for ip.current tracking.
    /// Default is false for non-branch nodes like OneNode, ParallelNode, etc.
    fn is_branch(&self) -> bool {
        false
    }
}

/// A sequence of nodes executed in order.
///
/// Runs each child node until it returns false (done), then moves to the next.
/// When all children complete, returns false.
///
/// C# Reference: Node.cs line 99
/// ```csharp
/// class SequenceNode : Branch { }
/// ```
/// Branch.Go() (lines 79-90) handles the sequential execution.
pub struct SequenceNode {
    /// Child nodes to execute in sequence
    pub nodes: Vec<Box<dyn Node>>,
    /// Current child index
    n: usize,
    /// Whether a branch child is currently "active" (was called and succeeded).
    /// In C#, when a branch child succeeds, ip.current is set to that branch,
    /// and subsequent main loop iterations call it directly. When it finally
    /// fails, ip.current is restored to the parent, and the parent re-tries
    /// that child. We simulate this by tracking whether a branch child is active.
    branch_child_active: bool,
}

impl SequenceNode {
    /// Create a new sequence node with the given children.
    pub fn new(nodes: Vec<Box<dyn Node>>) -> Self {
        Self {
            nodes,
            n: 0,
            branch_child_active: false,
        }
    }
}

impl Node for SequenceNode {
    /// Execute the current child. If it returns false, advance to next child.
    /// Returns false when all children are exhausted.
    ///
    /// C# Reference: Branch.Go() lines 79-90
    ///
    /// IMPORTANT: In C#, when a branch child succeeds, ip.current is set to that
    /// branch, causing subsequent main loop iterations to call the branch directly.
    /// When the branch finally fails, ip.current is restored to the parent, and
    /// the parent's NEXT call re-tries that same child (because n wasn't advanced).
    ///
    /// In Rust, we simulate this by tracking branch_child_active. When a branch
    /// child fails while active, we give it one more retry (calling it again
    /// immediately) before advancing n.
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        while self.n < self.nodes.len() {
            let result = self.nodes[self.n].go(ctx);

            if result {
                // Child succeeded - check if it's a branch type
                // In C#, branch children get ip.current set to them
                if self.nodes[self.n].is_branch() {
                    self.branch_child_active = true;
                }
                return true;
            }

            // Child failed
            if self.branch_child_active {
                // This was an active branch child that just failed.
                // In C#, this would set ip.current = parent, and the NEXT
                // main loop iteration would call us again with n unchanged.
                // We simulate the retry by calling the child again immediately.
                self.branch_child_active = false;

                // Retry the child once
                let retry_result = self.nodes[self.n].go(ctx);

                if retry_result {
                    // Retry succeeded, continue as normal
                    self.branch_child_active = self.nodes[self.n].is_branch();
                    return true;
                }
                // Retry also failed, fall through to advance n
            }

            // Non-branch child failed, or branch child failed on retry - advance
            self.n += 1;
        }
        // All children done, reset for next use
        self.reset();
        false
    }

    fn reset(&mut self) {
        for node in &mut self.nodes {
            node.reset();
        }
        self.n = 0;
        self.branch_child_active = false;
    }

    fn is_branch(&self) -> bool {
        true
    }
}

/// A Markov chain of nodes - loops until no child makes progress.
///
/// Unlike SequenceNode, MarkovNode restarts from the first child after each
/// successful step. It only returns false when NO child can make progress.
///
/// C# Reference: Node.cs lines 100-110
/// ```csharp
/// class MarkovNode : Branch
/// {
///     public override bool Go()
///     {
///         n = 0;  // Always restart from first child
///         return base.Go();
///     }
/// }
/// ```
pub struct MarkovNode {
    /// Child nodes to execute
    pub nodes: Vec<Box<dyn Node>>,
    /// Current child index (reset to 0 on each Go call)
    n: usize,
}

impl MarkovNode {
    /// Create a new Markov node with the given children.
    pub fn new(nodes: Vec<Box<dyn Node>>) -> Self {
        Self { nodes, n: 0 }
    }
}

impl Node for MarkovNode {
    /// Execute children starting from index 0. If any child succeeds, return true.
    /// Returns false only when no child can make progress.
    ///
    /// Key difference from SequenceNode: n is reset to 0 at the START of each call.
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        self.n = 0; // C#: n = 0; return base.Go();

        while self.n < self.nodes.len() {
            if self.nodes[self.n].go(ctx) {
                return true;
            }
            self.n += 1;
        }
        // No child made progress, reset
        self.reset();
        false
    }

    fn reset(&mut self) {
        for node in &mut self.nodes {
            node.reset();
        }
        self.n = 0;
    }

    fn is_branch(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::rng::StdRandom;

    /// A simple test node that counts down and returns true until reaching 0.
    struct CountdownNode {
        initial: usize,
        remaining: usize,
    }

    impl CountdownNode {
        fn new(count: usize) -> Self {
            Self {
                initial: count,
                remaining: count,
            }
        }
    }

    impl Node for CountdownNode {
        fn go(&mut self, _ctx: &mut ExecutionContext) -> bool {
            if self.remaining > 0 {
                self.remaining -= 1;
                true
            } else {
                false
            }
        }

        fn reset(&mut self) {
            self.remaining = self.initial;
        }
    }

    #[test]
    fn test_sequence_node_runs_in_order() {
        let mut grid = MjGrid::with_values(1, 1, 1, "BW");
        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        let mut seq = SequenceNode::new(vec![
            Box::new(CountdownNode::new(2)),
            Box::new(CountdownNode::new(3)),
        ]);

        // First node runs twice
        assert!(seq.go(&mut ctx));
        assert!(seq.go(&mut ctx));
        // First node done, second node starts
        assert!(seq.go(&mut ctx));
        assert!(seq.go(&mut ctx));
        assert!(seq.go(&mut ctx));
        // Both done
        assert!(!seq.go(&mut ctx));
    }

    #[test]
    fn test_markov_node_restarts_from_zero() {
        let mut grid = MjGrid::with_values(1, 1, 1, "BW");
        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Create a node that succeeds once then fails
        let mut markov = MarkovNode::new(vec![Box::new(CountdownNode::new(3))]);

        // Markov loops: each call restarts from child 0
        assert!(markov.go(&mut ctx)); // remaining: 3 -> 2
        assert!(markov.go(&mut ctx)); // remaining: 2 -> 1
        assert!(markov.go(&mut ctx)); // remaining: 1 -> 0
        assert!(!markov.go(&mut ctx)); // remaining: 0, returns false
    }

    #[test]
    fn test_execution_context_change_tracking() {
        let mut grid = MjGrid::with_values(5, 5, 1, "BW");
        let mut rng = StdRandom::from_u64_seed(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Turn 0
        ctx.record_change(0, 0, 0);
        ctx.record_change(1, 0, 0);
        ctx.next_turn();

        // Turn 1
        ctx.record_change(2, 0, 0);
        ctx.next_turn();

        assert_eq!(ctx.changes_from_turn(0), &[(0, 0, 0), (1, 0, 0)]);
        assert_eq!(ctx.changes_from_turn(1), &[(2, 0, 0)]);
        assert_eq!(ctx.counter, 2);
    }
}
