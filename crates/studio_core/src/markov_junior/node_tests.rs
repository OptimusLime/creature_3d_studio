//! Phase 1.2 Verification Tests
//!
//! These are the official verification tests from IMPLEMENTATION_PLAN.md
//! that must pass to complete Phase 1.2.

#[cfg(test)]
mod phase_1_2_verification {
    use crate::markov_junior::{
        AllNode, ExecutionContext, MarkovNode, MjGrid, MjRule, Node, OneNode, SequenceNode,
    };
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    /// Phase 1.2 Test 1: OneNode applies exactly one match per step.
    ///
    /// Verification: 5x1 grid "BBBBB" with rule B→W, after 1 step exactly 1 cell is W.
    #[test]
    fn test_one_node_applies_single_match() {
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        // Grid starts all B (value 0)
        assert!(grid.state.iter().all(|&v| v == 0));

        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut node = OneNode::new(vec![rule], grid.state.len());

        let mut rng = StdRng::seed_from_u64(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Execute one step
        let made_progress = node.go(&mut ctx);
        assert!(made_progress, "OneNode should make progress");

        // Exactly 1 cell should be W (value 1)
        let w_count = ctx.grid.state.iter().filter(|&&v| v == 1).count();
        assert_eq!(
            w_count, 1,
            "Exactly 1 cell should be W after 1 step. Got: {:?}",
            ctx.grid.state
        );
    }

    /// Phase 1.2 Test 2: AllNode applies all 1x1 matches in one step.
    ///
    /// Verification: 5x1 grid "BBBBB" with rule B→W, after 1 step all 5 cells are W.
    #[test]
    fn test_all_node_fills_entire_grid() {
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        assert!(grid.state.iter().all(|&v| v == 0));

        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let mut node = AllNode::new(vec![rule], grid.state.len());

        let mut rng = StdRng::seed_from_u64(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Execute one step
        let made_progress = node.go(&mut ctx);
        assert!(made_progress, "AllNode should make progress");

        // All 5 cells should be W (value 1)
        assert!(
            ctx.grid.state.iter().all(|&v| v == 1),
            "All 5 cells should be W after 1 step. Got: {:?}",
            ctx.grid.state
        );
    }

    /// Phase 1.2 Test 3: AllNode respects non-overlapping constraint.
    ///
    /// Verification: 5x1 grid with rule BB→WW, after 1 step exactly 4 cells are W, 1 remains B.
    #[test]
    fn test_all_node_non_overlapping() {
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        assert!(grid.state.iter().all(|&v| v == 0));

        let rule = MjRule::parse("BB", "WW", &grid).unwrap();
        let mut node = AllNode::new(vec![rule], grid.state.len());

        let mut rng = StdRng::seed_from_u64(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Execute one step
        let made_progress = node.go(&mut ctx);
        assert!(made_progress, "AllNode should make progress");

        // For 5 cells with a 2-cell rule:
        // - Maximum non-overlapping matches: 2 (covering 4 cells)
        // - 1 cell must remain unchanged
        let w_count = ctx.grid.state.iter().filter(|&&v| v == 1).count();
        let b_count = ctx.grid.state.iter().filter(|&&v| v == 0).count();

        assert_eq!(
            w_count, 4,
            "Exactly 4 cells should be W (2 non-overlapping BB→WW matches). Got: {:?}",
            ctx.grid.state
        );
        assert_eq!(
            b_count, 1,
            "Exactly 1 cell should remain B. Got: {:?}",
            ctx.grid.state
        );
    }

    /// Phase 1.2 Test 4: MarkovNode loops until no child makes progress.
    ///
    /// Verification: MarkovNode with B→W rule, runs until no matches, all cells become W.
    #[test]
    fn test_markov_node_loops_until_done() {
        let mut grid = MjGrid::with_values(5, 1, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();

        // Create OneNode wrapped in MarkovNode
        let one_node = OneNode::new(vec![rule], grid.state.len());
        let mut markov_node = MarkovNode::new(vec![Box::new(one_node)]);

        let mut rng = StdRng::seed_from_u64(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // MarkovNode should loop until all B's are converted to W's
        let mut steps = 0;
        while markov_node.go(&mut ctx) {
            ctx.next_turn();
            steps += 1;
            if steps > 100 {
                panic!("MarkovNode should complete in <= 5 steps for 5 cells");
            }
        }

        // All cells should be W
        assert!(
            ctx.grid.state.iter().all(|&v| v == 1),
            "All cells should be W after MarkovNode completes. Got: {:?}",
            ctx.grid.state
        );

        // Should have taken exactly 5 steps (one per cell)
        assert_eq!(
            steps, 5,
            "Should take exactly 5 steps for 5 cells with OneNode"
        );
    }

    /// Phase 1.2 Test 5: SequenceNode runs children in order.
    ///
    /// Verification: SequenceNode with [B→R, R→W], final grid all W.
    #[test]
    fn test_sequence_node_runs_in_order() {
        let mut grid = MjGrid::with_values(5, 1, 1, "BRW");
        // Grid starts all B (value 0)
        assert!(grid.state.iter().all(|&v| v == 0));

        // Rule 1: B → R (all B's become R)
        let rule1 = MjRule::parse("B", "R", &grid).unwrap();
        // Rule 2: R → W (all R's become W)
        let rule2 = MjRule::parse("R", "W", &grid).unwrap();

        // Create two AllNodes for each rule
        let node1 = AllNode::new(vec![rule1], grid.state.len());
        let node2 = AllNode::new(vec![rule2], grid.state.len());

        // Wrap in SequenceNode
        let mut seq_node = SequenceNode::new(vec![Box::new(node1), Box::new(node2)]);

        let mut rng = StdRng::seed_from_u64(42);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // First go(): node1 (B→R) should run
        assert!(seq_node.go(&mut ctx), "First step should succeed");
        ctx.next_turn();

        // After first step: all should be R (value 1 for 'R' which is index 1 in "BRW")
        let r_value = ctx.grid.values.get(&'R').copied().unwrap();
        assert!(
            ctx.grid.state.iter().all(|&v| v == r_value),
            "After first step, all cells should be R. Got: {:?}",
            ctx.grid.state
        );

        // Second go(): node2 (R→W) should run
        assert!(seq_node.go(&mut ctx), "Second step should succeed");
        ctx.next_turn();

        // After second step: all should be W (value 2 for 'W' which is index 2 in "BRW")
        let w_value = ctx.grid.values.get(&'W').copied().unwrap();
        assert!(
            ctx.grid.state.iter().all(|&v| v == w_value),
            "After second step, all cells should be W. Got: {:?}",
            ctx.grid.state
        );

        // Third go(): both nodes exhausted, should return false
        assert!(
            !seq_node.go(&mut ctx),
            "Third step should return false (all nodes done)"
        );
    }
}
