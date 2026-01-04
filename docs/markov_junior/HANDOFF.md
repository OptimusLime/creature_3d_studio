# MarkovJunior Rust Port - Handoff Document

## Current State

**Branch:** `feature/markov-junior-rust`
**Phase:** Starting Phase 1.2 (Node Infrastructure)
**Tests:** 35 passing

## What's Been Done

### Phase 0: End-to-End Skeleton (COMPLETE)
- `MjGrid` struct with basic state storage
- `voxel_bridge.rs` converting MjGrid → VoxelWorld
- `examples/p25_markov_junior.rs` rendering hardcoded cross pattern

### Phase 1: Foundation Data Structures (COMPLETE)
- `MjGrid` with `state`, `mask`, `values`, `waves`, `characters`
- `wave()` method for bitmask computation
- `matches()` and `apply()` for rule pattern matching
- `MjRule` with `input`, `output`, `binput`, `ishifts`, `oshifts`
- `parse()` for pattern strings ("RB/WW" format)
- `z_rotated()`, `reflected()`, `same()` transformations
- `square_symmetries()` generating up to 8 rule variants
- Z-axis reversal in parsing matches C# exactly

## Key Files to Read

### Implementation
- `crates/studio_core/src/markov_junior/mod.rs` - MjGrid struct
- `crates/studio_core/src/markov_junior/rule.rs` - MjRule struct
- `crates/studio_core/src/markov_junior/symmetry.rs` - symmetry generation

### Documentation
- `docs/markov_junior/IMPLEMENTATION_PLAN.md` - Full phased plan with verification criteria
- `docs/markov_junior/DEVIATIONS.md` - **CRITICAL** - All differences from C# code
- `docs/markov_junior/ARCHITECTURE.md` - C# analysis notes

### C# Reference (in repo)
- `MarkovJunior/source/Node.cs` - Node base class
- `MarkovJunior/source/RuleNode.cs` - Pattern matching base
- `MarkovJunior/source/OneNode.cs` - Random single match
- `MarkovJunior/source/AllNode.cs` - All non-overlapping matches
- `MarkovJunior/source/ParallelNode.cs` - Simultaneous application

## Phase 1.2: Node Infrastructure

### Goal
Implement the node execution system that runs MarkovJunior models.

### Files to Create
```
crates/studio_core/src/markov_junior/
├── node.rs             # Node trait + SequenceNode, MarkovNode
├── rule_node.rs        # RuleNode base with match tracking
├── one_node.rs         # OneNode: pick random match, apply
├── all_node.rs         # AllNode: apply all non-overlapping
└── parallel_node.rs    # ParallelNode: apply all simultaneously
```

### Verification (from IMPLEMENTATION_PLAN.md)
Run `cargo test -p studio_core markov_junior::node` and see:
- `test_one_node_applies_single_match ... ok` (5x1 grid "BBBBB" with rule B→W, after 1 step exactly 1 cell is W)
- `test_all_node_fills_entire_grid ... ok` (5x1 grid "BBBBB" with rule B→W, after 1 step all 5 cells are W)
- `test_all_node_non_overlapping ... ok` (5x1 grid with rule BB→WW, after 1 step exactly 4 cells are W, 1 remains B)
- `test_markov_node_loops_until_done ... ok` (MarkovNode with B→W rule, runs until no matches, all cells become W)
- `test_sequence_node_runs_in_order ... ok` (SequenceNode with [B→R, R→W], final grid all W)

### Key C# Patterns to Follow

**Node trait (from Node.cs:11-15):**
```csharp
abstract class Node
{
    abstract public bool Go();  // Execute one step, return true if made progress
    virtual public void Reset() { }
}
```

**RuleNode match tracking (from RuleNode.cs):**
- `matches: List<(int, int, int, int)>` - (rule_index, x, y, z)
- `matchMask: bool[rules.Length][grid.state.Length]` - deduplication
- `last: int[]` - match count boundaries per step
- `counter: int` - step counter

**OneNode.Go() (from OneNode.cs:60-80):**
1. If first call or `matchCount == 0`, scan entire grid for matches
2. Pick random match (with optional temperature/field heuristics)
3. Apply rule at match position
4. Return true if applied, false if no matches

**AllNode.Go() (from AllNode.cs:35-70):**
1. Clear `grid.mask`
2. Scan for matches, skip if any cell in pattern already masked
3. For each valid match: apply rule, set mask for affected cells
4. Return true if any matches applied

## Critical Reminders

1. **Update DEVIATIONS.md** for ANY deviation from C# code
2. **Run tests frequently** - `cargo test -p studio_core markov_junior`
3. **The `mask` field exists** - use it in AllNode for conflict tracking
4. **ishifts/oshifts exist** - use them for incremental matching (or defer)

## Commands

```bash
# Run all markov_junior tests
cargo test -p studio_core markov_junior

# Run specific test
cargo test -p studio_core markov_junior::node::tests::test_one_node_applies_single_match

# Run example
cargo run --example p25_markov_junior

# Check what's changed
git diff HEAD~1
```

## Remaining Deviations to Address Later

From DEVIATIONS.md - NOT blocking Phase 1.2:
- `YRotated()` - needed for 3D (Phase 1.4)
- `CubeSymmetries()` - needed for 3D (Phase 1.4)
- Union type support - needed for XML loading (Phase 1.4)
- `transparent` field - for rendering
- `folder` field - for resource paths
