# Plan: MarkovJunior Step-by-Step Execution Refactor

## Problem

The current `model.step()` implementation doesn't actually step one unit of work - it runs entire sub-computations to completion. This prevents animated visualization of the generation process.

### Root Cause

In C# MarkovJunior, the interpreter has a `current` pointer that tracks which node is actively executing:

```csharp
while (current != null && (steps <= 0 || counter < steps)) {
    current.Go();  // Go() can change `current` to a child node
    counter++;
}
```

When `Go()` returns, `current` may have changed to:
- A child branch (if the child needs to continue executing)
- The parent (if this node is done)
- `null` (if the root is done)

Our Rust implementation lacks this `current` tracking. Instead, each node's `go()` method recursively calls children and tries to complete internally, which means a single `step()` call can run hundreds of actual steps.

## Solution

Two options:

### Option A: Add `current` tracking (Major refactor)
- Add `current: Option<&mut dyn Node>` to Interpreter
- Modify `go()` to return which node should be `current` next
- Each `step()` calls `current.go()` once

**Pros:** Matches C# exactly
**Cons:** Requires significant changes to Node trait and all implementations, complex borrow checker issues with mutable references

### Option B: Limit work per `go()` call (Simpler)
- Add a "work budget" to ExecutionContext
- Each atomic operation (rule application, WFC observe) decrements budget
- When budget hits 0, return early with "needs more work" state
- `step()` sets budget to 1 for single-step mode

**Pros:** Less invasive, works with current architecture
**Cons:** Doesn't match C# exactly, may have edge cases

### Option C: Track execution state in nodes (Hybrid)
- Each Branch node tracks which child is "active"
- `go()` returns after one child makes progress
- Already partially implemented with `active_branch_child`

**This is what we attempted but it's not working correctly.**

## Immediate Fix (For p30)

1. **Reduce model size** - Use 8x8x8 instead of 16x16x16
2. **Use a simpler model** - Try a model that shows progress (not WFC-heavy)
3. **Debug why animated mode isn't working** - The `gif` flag should cause `update_state()` to be called

## Investigation

Let me trace what happens with `gif=true`:

1. `Interpreter::step()` creates context with `gif: self.animated`
2. Context passed to `root.go()`
3. For TileNode (WFC), line 369-371:
   ```rust
   if ctx.gif {
       self.update_state(ctx.grid);
   }
   ```

This should update the grid state after each WFC step. But WFC itself runs many internal iterations in `step()`.

## The Real Issue

Looking at `WfcNode::step()`:
```rust
pub fn step(&mut self) -> bool {
    // This runs observe + propagate, which can be many operations
}
```

And `TileNode::go()`:
```rust
if self.wfc.step() {
    if ctx.gif {
        self.update_state(ctx.grid);
    }
    true
}
```

So `wfc.step()` is ONE WFC collapse step, and `update_state` writes the current wave state to the grid. This should work.

BUT - the problem might be that the model structure has multiple layers:
- Sequence at top
- WFC in middle  
- Children of WFC (rules that run AFTER WFC completes)

When WFC is running, the grid is swapped with `newgrid`, so `model.grid()` returns the wrong grid!

## Fix Strategy

1. **Don't swap grids** - Instead, have WFC write to the main grid directly
2. **Or expose the "current" grid** - Add method to get whichever grid is active

## Phase 1: Quick Fix for p30

1. Reduce Apartemazements size to 8x8x8
2. Slow down steps_per_second to 5-10
3. Add debug logging to see what's happening

## Phase 2: Proper Fix

1. Modify TileNode/OverlapNode to NOT swap grids
2. Have them write directly to ctx.grid during animated mode
3. Or track which grid is "current" and expose that
