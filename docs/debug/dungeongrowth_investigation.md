# DungeonGrowth Investigation

## Problem Statement
DungeonGrowth model achieves 98.29% match (107 cells differ out of 6241).
This is a 2D model (79x79x1) that uses a complex sequence of nodes including `path` with `longest="True"`.

## Model Structure
```xml
<sequence values="WRBUPY" origin="True" folder="DungeonGrowth">
  <union symbol="?" values="BR"/>
  <prl in="*****/*****/**W**/*****/*****" out="*****/*****/**B**/*****/*****"/>
  <one>
    <!-- 18 room rules -->
    <rule file="Room1" legend="*?WRBPU"/>
    ...
  </one>
  <one in="WUW/BBB" out="WRW/BBB"/>
  <all in="U" out="P"/>
  <markov>
    <all>
      <rule in="RY" out="UU"/>
      <rule in="UR" out="UU"/>
      <rule in="UY" out="UU"/>
      <rule in="BU" out="WU"/>
      <rule in="B*/*U" out="W*/*U"/>
    </all>
    <path from="R" to="Y" on="B" color="U" inertia="True" longest="True"/>
    <one in="Y" out="W"/>
    <one in="R" out="Y"/>
  </markov>
  <all>
    <rule in="U" out="P"/>
    <rule in="W" out="B"/>
  </all>
  <all in="BBB/BPB" out="***/*B*"/>
  <all>
    <rule in="BP" out="WP"/>
    <rule in="B*/*P" out="W*/*P"/>
  </all>
</sequence>
```

Key features:
- Uses `path` node with `longest="True"` and `inertia="True"`
- Complex markov block with nested all/path/one nodes
- Multiple rule files loaded from DungeonGrowth folder
- 6 colors: W(white), R(red), B(blue/black), U, P(purple), Y(yellow)

## Difference Analysis

```
First 20 differences:
  (72,36,0): C#=0(W) Rust=2(B)
  (71,37,0): C#=4(P) Rust=0(W)
  (72,37,0): C#=0(W) Rust=2(B)
  ...

Diff pattern analysis:
  Unique X values: 22 (range 19-77)
  Unique Y values: 22 (range 36-77)
  Unique Z values: 1 (range 0-0)
  C# values in diffs: {0, 2, 4}  (W, B, P)
  Rust values in diffs: {0, 2, 4}  (W, B, P)
  First diff at index: 2916
```

**Observations:**
1. Differences are clustered in the bottom-right quadrant (x: 19-77, y: 36-77)
2. First diff occurs at index 2916 (fairly late in the grid)
3. Color swaps involve W, B, P - which are manipulated in the post-path cleanup phase
4. The `path` node with `longest="True"` is a likely suspect since longest path is complex

## Hypotheses

### Hypothesis 1: Path node `longest` mode differs
The `longest="True"` parameter triggers a different algorithm (longest path vs shortest path).
This is complex and involves backtracking or search algorithms that may differ.

**Test:** Add debug logging to compare:
- Number of path attempts
- Path length found
- RNG calls during path finding

### Hypothesis 2: Path node `inertia` implementation differs
The `inertia="True"` parameter affects direction preferences during path search.

**Test:** Compare inertia direction calculations between C# and Rust.

### Hypothesis 3: Post-path rules execute differently
The rules after `path` depend on the path result. If the path differs slightly,
all subsequent rules will produce different results.

**Test:** Dump grid state immediately after `path` node completes in both C# and Rust.

### Hypothesis 4: RNG divergence in path search
Path search may consume RNG differently, causing downstream divergence.

**Test:** Compare RNG state before/after path node.

## Investigation Plan

### Phase 1: Isolate the divergence point
1. Add step-by-step grid checksum logging to both C# and Rust
2. Find the FIRST node where outputs diverge
3. This narrows down which node has the bug

### Phase 2: Deep-dive the divergent node
Once we know which node diverges:
1. Add detailed logging to that specific node
2. Compare intermediate values
3. Identify the root cause

### Phase 3: Fix and verify
1. Implement the fix
2. Verify 100% match
3. Run full batch verification to check for regressions

## Key Files

### Rust
- `crates/studio_core/src/markov_junior/path_node.rs` - Path node implementation
- `crates/studio_core/src/markov_junior/one_node.rs` - One node
- `crates/studio_core/src/markov_junior/all_node.rs` - All node
- `crates/studio_core/src/markov_junior/markov_node.rs` - Markov node

### C#
- `MarkovJunior/source/Search.cs` - Path node (Search class)
- `MarkovJunior/source/OneNode.cs` - One node
- `MarkovJunior/source/AllNode.cs` - All node (RuleNode handles both)

## Commands

```bash
# Run C# 
cd MarkovJunior && dotnet run -- --model DungeonGrowth --seed 42 --dump-json

# Run Rust
MJ_MODELS=DungeonGrowth MJ_SEED=42 cargo test -p studio_core verification::tests::batch_generate_outputs -- --ignored --nocapture

# Compare
python3 scripts/compare_grids.py MarkovJunior/verification/DungeonGrowth_seed42.json verification/rust/DungeonGrowth_seed42.json

# Verify
python3 scripts/batch_verify.py DungeonGrowth --regenerate
```

## Status
- [ ] Phase 1: Isolate divergence point
- [ ] Phase 2: Deep-dive divergent node  
- [ ] Phase 3: Fix and verify
