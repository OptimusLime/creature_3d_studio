# ChainDungeonMaze Investigation

**Status:** RESOLVED
**Commit:** (pending)

## Summary

All three Chain* models (ChainMaze, ChainDungeon, ChainDungeonMaze) were failing verification due to a bug in ConvChain pattern weight calculation. The Rust implementation was deduplicating symmetric patterns, but C# does not.

## Root Cause

In `convchain_node.rs`, the `square_symmetries_bool` function was deduplicating patterns:

```rust
// BEFORE (incorrect)
for i in 0..8 {
    if mask[i] && !result.iter().any(|r| patterns_equal(r, &things[i])) {
        result.push(things[i].clone());
    }
}
```

But C# passes `(q1, q2) => false` as the equality comparator to `SquareSymmetries`, meaning duplicates are NOT removed:

```csharp
var symmetries = SymmetryHelper.SquareSymmetries(pattern, 
    q => Helper.Rotated(q, N), 
    q => Helper.Reflected(q, N), 
    (q1, q2) => false,  // Always returns false - no deduplication!
    symmetry);
```

This caused Rust weights to be exactly half of C# weights, leading to different MCMC acceptance probabilities.

## Fix

Remove deduplication in Rust:

```rust
// AFTER (correct)
for i in 0..8 {
    if mask[i] {
        result.push(things[i].clone());
    }
}
```

## Additional Fix

Also fixed RNG method mismatch in ConvChain MCMC loop:
- Changed `ctx.random.next_bool()` to `ctx.random.next_int_max(2) == 0` for initialization
- Changed `ctx.random.next_usize_max(state.len())` to `ctx.random.next_int_max(state.len() as i32) as usize` for position selection

These ensure the same RNG call sequence as C#.

## Verification Results

| Model | Before | After |
|-------|--------|-------|
| ChainMaze | 62.92% | 100% |
| ChainDungeon | 62.19% | 100% |
| ChainDungeonMaze | 73.22% | 100% |

## Debug Process

1. Created simplified model `ChainMazeSimple.xml` with just convchain
2. Tested with steps=1 (100% match) vs steps=2 (diverged)
3. Added logging to trace MCMC iterations - found q values differed
4. Added weight dump logging - found weights exactly half in Rust
5. Traced to `square_symmetries_bool` deduplication logic
6. Fixed and verified all Chain* models now pass
