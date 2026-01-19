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
use serde::{Deserialize, Serialize};

/// Structure information for a Markov Jr. node, used for introspection.
///
/// This allows external tools (MCP, visualizers) to understand the
/// internal structure of a Markov Jr. model without executing it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MjNodeStructure {
    /// Node type: "Markov", "Sequence", "One", "All", "Path", etc.
    pub node_type: String,
    /// Children (for branch nodes)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<MjNodeStructure>,
    /// Rule strings (for One/All nodes)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rules: Vec<String>,
    /// Additional config as JSON
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub config: Option<serde_json::Value>,
}

impl MjNodeStructure {
    /// Create a new structure with just a node type.
    pub fn new(node_type: &str) -> Self {
        Self {
            node_type: node_type.to_string(),
            children: Vec::new(),
            rules: Vec::new(),
            config: None,
        }
    }

    /// Add children to this structure.
    pub fn with_children(mut self, children: Vec<MjNodeStructure>) -> Self {
        self.children = children;
        self
    }

    /// Add rules to this structure.
    pub fn with_rules(mut self, rules: Vec<String>) -> Self {
        self.rules = rules;
        self
    }

    /// Add config to this structure.
    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = Some(config);
        self
    }
}

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

    /// Return the structure of this node for introspection.
    ///
    /// This allows external tools (MCP, visualizers) to understand the
    /// internal structure of a Markov Jr. model without executing it.
    fn structure(&self) -> MjNodeStructure {
        MjNodeStructure::new("Unknown")
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
///
/// IMPORTANT: In C#, when a branch child succeeds, `ip.current` is set to that
/// child. The main loop then calls `current.Go()` directly on subsequent
/// iterations, bypassing the parent. This continues until the child fails.
/// We simulate this by tracking `active_branch_child` - if set, we delegate
/// directly to that child until it fails.
pub struct SequenceNode {
    /// Child nodes to execute in sequence
    pub nodes: Vec<Box<dyn Node>>,
    /// Current child index
    n: usize,
    /// Index of currently active branch child, if any.
    /// When a branch child succeeds, it becomes "active" and subsequent Go()
    /// calls delegate directly to it (simulating ip.current = child).
    active_branch_child: Option<usize>,
}

impl SequenceNode {
    /// Create a new sequence node with the given children.
    pub fn new(nodes: Vec<Box<dyn Node>>) -> Self {
        Self {
            nodes,
            n: 0,
            active_branch_child: None,
        }
    }
}

impl Node for SequenceNode {
    /// Execute the current child. If it returns false, advance to next child.
    /// Returns false when all children are exhausted.
    ///
    /// C# Reference: Branch.Go() lines 79-90
    /// ```csharp
    /// for (; n < nodes.Length; n++)
    /// {
    ///     Node node = nodes[n];
    ///     if (node is Branch branch) ip.current = branch;
    ///     if (node.Go()) return true;
    /// }
    /// ip.current = ip.current.parent;
    /// Reset();
    /// return false;
    /// ```
    ///
    /// IMPORTANT: In C#, when a branch child succeeds, `ip.current` is set to that
    /// child before Go() returns. The main loop then calls `ip.current.Go()` directly,
    /// which means the child keeps running until it fails. We simulate this by:
    /// 1. Tracking which branch child is "active"
    /// 2. Delegating Go() calls to the active child until it fails
    /// 3. When it fails, clearing active_branch_child and advancing n
    fn go(&mut self, ctx: &mut ExecutionContext<'_>) -> bool {
        // If we have an active branch child, delegate to it (simulates ip.current = child)
        if let Some(active_idx) = self.active_branch_child {
            if self.nodes[active_idx].go(ctx) {
                // Child still making progress
                return true;
            }
            // Child failed - in C#, this sets ip.current = parent and child.Reset() is called
            // The child's Reset() is already called by the child's Go() implementation when
            // it falls through to the end of its for loop.
            //
            // IMPORTANT: Do NOT increment n here! In C#, when the parent returned true last
            // time (with ip.current = child), the parent's for-loop was exited via return,
            // so n was never incremented. When we resume at the same n, the for-loop's
            // condition check will call the same child again (which has been reset).
            // The for-loop's n++ only happens on loop continuation (child returns false),
            // not after returning from the method.
            self.active_branch_child = None;
            // Don't increment n - we'll try the same child again, which has now been reset
            // Return true to allow counter increment before next call
            return true;
        }

        // Normal execution: try children from current n
        while self.n < self.nodes.len() {
            let is_branch = self.nodes[self.n].is_branch();

            if self.nodes[self.n].go(ctx) {
                // Child succeeded
                if is_branch {
                    // In C#: ip.current = branch (before Go() returns true)
                    // This means next iteration will call this child directly
                    self.active_branch_child = Some(self.n);
                }
                return true;
            }

            // Child failed immediately, advance to next
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
        self.active_branch_child = None;
    }

    fn is_branch(&self) -> bool {
        true
    }

    fn structure(&self) -> MjNodeStructure {
        MjNodeStructure::new("Sequence")
            .with_children(self.nodes.iter().map(|n| n.structure()).collect())
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
///
/// IMPORTANT: In C#, when a branch child succeeds, `ip.current` is set to that
/// child. The main loop then calls `current.Go()` directly on subsequent
/// iterations, bypassing the parent. This continues until the child fails.
/// We simulate this by tracking `active_branch_child` - if set, we delegate
/// directly to that child until it fails.
pub struct MarkovNode {
    /// Child nodes to execute
    pub nodes: Vec<Box<dyn Node>>,
    /// Current child index (reset to 0 on each Go call)
    n: usize,
    /// Index of currently active branch child, if any.
    /// When a branch child succeeds, it becomes "active" and subsequent Go()
    /// calls delegate directly to it (simulating ip.current = child).
    active_branch_child: Option<usize>,
}

impl MarkovNode {
    /// Create a new Markov node with the given children.
    pub fn new(nodes: Vec<Box<dyn Node>>) -> Self {
        Self {
            nodes,
            n: 0,
            active_branch_child: None,
        }
    }
}

impl Node for MarkovNode {
    /// Execute children starting from index 0. If any child succeeds, return true.
    /// Returns false only when no child can make progress.
    ///
    /// Key difference from SequenceNode: n is reset to 0 at the START of each call.
    ///
    /// IMPORTANT: In C#, when a branch child succeeds, `ip.current` is set to that
    /// child before Go() returns. The main loop then calls `ip.current.Go()` directly,
    /// which means the child keeps running until it fails. We simulate this by:
    /// 1. Tracking which branch child is "active"
    /// 2. Delegating Go() calls to the active child until it fails
    /// 3. When it fails, clearing active_branch_child and continuing normal execution
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // If we have an active branch child, delegate to it (simulates ip.current = child)
        if let Some(active_idx) = self.active_branch_child {
            if self.nodes[active_idx].go(ctx) {
                // Child still making progress
                return true;
            }
            // Child failed - in C#, this sets ip.current = parent and child.Reset() is called
            // The child's Reset() is already called by the child's Go() implementation
            // Clear active child.
            self.active_branch_child = None;
            // IMPORTANT: In C#, after child fails, ip.current = parent, then the main loop
            // increments counter and calls parent.Go() NEXT iteration. We need to return
            // here to allow that counter increment to happen. Return true to continue
            // execution (we're still "making progress" in the sense that we're transitioning
            // state), and next call will start fresh with n=0.
            return true;
        }

        // Normal execution: try children from n=0
        self.n = 0; // C#: n = 0; return base.Go();

        while self.n < self.nodes.len() {
            let is_branch = self.nodes[self.n].is_branch();

            if self.nodes[self.n].go(ctx) {
                // Child succeeded
                if is_branch {
                    // In C#: ip.current = branch (before Go() returns true)
                    // This means next iteration will call this child directly
                    self.active_branch_child = Some(self.n);
                }
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
        self.active_branch_child = None;
    }

    fn is_branch(&self) -> bool {
        true
    }

    fn structure(&self) -> MjNodeStructure {
        MjNodeStructure::new("Markov")
            .with_children(self.nodes.iter().map(|n| n.structure()).collect())
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

    #[test]
    fn test_mj_node_structure_basic() {
        // Test that MjNodeStructure serializes correctly
        let structure = MjNodeStructure::new("Sequence").with_children(vec![
            MjNodeStructure::new("One").with_rules(vec!["B=W".to_string()]),
            MjNodeStructure::new("All").with_rules(vec!["W=B".to_string(), "A=B".to_string()]),
        ]);

        // Serialize to JSON
        let json = serde_json::to_string(&structure).unwrap();
        assert!(json.contains("Sequence"));
        assert!(json.contains("One"));
        assert!(json.contains("B=W"));
        assert!(json.contains("All"));
    }

    #[test]
    fn test_sequence_node_structure() {
        // Sequence with CountdownNode should show structure
        let seq = SequenceNode::new(vec![
            Box::new(CountdownNode::new(2)),
            Box::new(CountdownNode::new(3)),
        ]);

        let structure = seq.structure();
        assert_eq!(structure.node_type, "Sequence");
        assert_eq!(structure.children.len(), 2);
    }

    #[test]
    fn test_markov_node_structure() {
        let markov = MarkovNode::new(vec![Box::new(CountdownNode::new(3))]);

        let structure = markov.structure();
        assert_eq!(structure.node_type, "Markov");
        assert_eq!(structure.children.len(), 1);
    }
}
