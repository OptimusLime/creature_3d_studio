# Bug: DungeonGrowth Counter Divergence

**Status:** RESOLVED
**Commit:** 825aef0

## Summary

DungeonGrowth was achieving 98.81% match (74 cells differ out of 6241). The root cause was that `last_matched_turn` was being set at the wrong point in the Rust `AllNode.go()` implementation, causing incremental match scans to cover different change ranges than C#.

## Root Cause

**C# AllNode.Go() lines 43-54:**
```csharp
if (!base.Go()) return false;     // Line 43 - computes matches
lastMatchedTurn = ip.counter;      // Line 44 - ALWAYS set after base.Go()
if (trajectory != null) { ... }    // Line 46-52
if (matchCount == 0) return false; // Line 54
```

**Rust AllNode.go() (BEFORE fix):**
```rust
if !self.data.compute_matches(ctx) { return false; }  // computes matches
if self.data.match_count == 0 { return false; }       // EARLY RETURN!
self.data.last_matched_turn = ctx.counter as i32;     // Only set if matches > 0
```

**The Bug:** When `match_count == 0`, Rust returned early **without setting `last_matched_turn`**, but C# always sets it after `base.Go()` regardless of match count.

## Fix

Move `last_matched_turn` assignment to happen before the `match_count == 0` check:

```rust
if !self.data.compute_matches(ctx) { return false; }

// Record this as the last matched turn BEFORE checking match_count
// C# sets lastMatchedTurn = ip.counter at line 44, before checking matchCount at line 54
self.data.last_matched_turn = ctx.counter as i32;

if self.data.match_count == 0 { return false; }
```

## Impact

This fix improved DungeonGrowth from 98.81% to 100% match, and improved overall verification from 112/144 (77.8%) to 133/156 (85.3%).

## Debugging Process

### 1. Systematic Model Simplification

Created `DungeonGrowthSimple.xml` - stripped down version of DungeonGrowth. Gradually added nodes back until divergence appeared, isolating it to the `<markov>` block containing `<all>`, `<path>`, and `<one>` nodes.

### 2. Added Targeted Logging

Added minimal logging to track:
- `INCR_SCAN: turn={}, start={}, end={}` - incremental scan ranges
- `ALL[...]: match_count={} matches=[...]` - match lists
- `PATH_START/PATH_SUCCESS` - path tracing

### 3. Evidence Collection

Compared logs side-by-side and found:
- Path cells identical (same 29 cells in same order)
- Shuffle permutation identical (`[56,18,83,0,53,61,...]`)
- Match sets identical (104 matches, verified by sorting)
- Match ORDER differs due to different scan ranges
- C# turn=141 vs Rust turn=125 at path #4

### 4. Hypothesis Testing

Traced `lastMatchedTurn` assignment in both implementations and found the ordering difference in AllNode.go().

## Remaining Failing Models (23)

After this fix, 23 models still fail verification:
- Island (639061 cells different)
- SubmergedKnots (34817)
- LostCity (20249)
- Surface models (EscherSurface, ClosedSurface, Surface)
- Chain models (ChainDungeon, ChainMaze, ChainDungeonMaze)
- Tile models (TilePath, TileDungeon)
- Hamiltonian models (HamiltonianPath, HamiltonianPaths)
- Knots2D, SelectLongKnots
- Circuit, Partitioning, Sewers
- SmartSAW, CompleteSAW, CompleteSAWSmart, FindLongCycle

These likely have different root causes to investigate.
