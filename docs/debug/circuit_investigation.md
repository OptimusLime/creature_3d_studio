# Circuit Investigation

**Status:** RESOLVED
**Model:** Circuit.xml (2D)
**Match:** 100% (was 94.46%)
**Dimensions:** 59x59x1

## Resolution

Two fixes were required:

### Fix 1: WFC Completion Step Counting
WFCNode.Go() in C# returns `true` on the step when WFC completes, even if there are no children. Our Rust code returned `false` immediately. Fixed by always returning `true` on completion.

### Fix 2: Branch Child Retry Behavior
In C#, when a branch child (like a nested SequenceNode) fails:
1. The child sets `ip.current = parent` and resets itself
2. Main loop increments counter
3. Main loop calls `parent.Go()` which retries the same child (n unchanged)

Our Rust code was immediately advancing `n` when a branch child failed. Fixed by tracking `branch_child_was_active` and returning `true` (to trigger counter increment) when a branch child fails, allowing the retry to happen on the next call.

## Summary

Circuit is an **animation model** that runs forever without termination. It has `steps="1200"` limit in models.xml. The discrepancy was caused by step counting differences.

## Model Structure

```xml
<sequence values="BtEDGANYOWU" origin="True">
  <prl in="***/*B*/***" out="***/*E*/***"/>
  <all in="tEE" out="**t"/>
  <one file="Chip" legend="*tEADN" steps="2"/>
  <markov>
    <all>...</all>
    <path from="A" to="U" on="E" color="G" inertia="True"/>
    <all in="A" out="U"/>
    <one file="Chip" legend="*tEADN"/>
  </markov>
  <prl in="B" out="E"/>
  <all>...</all>
  <markov>...</markov>
  <all in="NG" out="NW"/>           <!-- 100% match up to here -->
  <all>                              <!-- DIVERGENCE STARTS HERE -->
    <rule in="ND" out="DN"/>         <!-- The "moving" rule -->
    <rule in="NWGG" out="DWYO"/>
    <rule in="YOG" out="GYO"/>
    <rule in="YO/*G" out="GY/*O"/>
    <rule in="YOAD" out="GGAN"/>
  </all>
</sequence>
```

## Current State

### What's Working
- All nodes before final `<all>` block: 100% match
- RNG synchronization: VERIFIED (identical shuffle sequences)
- Match counting: VERIFIED (identical match counts)
- Grid operations: VERIFIED (identical at corresponding iterations)

### What's Broken
- Step counting: 2-step offset
  - C# reaches final `<all>` at counter 819
  - Rust reaches final `<all>` at counter 817
- Both hit limit at counter 1199, but Rust runs 2 more iterations

### Observed Diffs
```
CircuitSimple (ND->DN only): 98.16% (64 differ)
  - Only N(6) and D(3) values differ
  - Values are swapped between C# and Rust

Full Circuit: 94.46% (193 differ)  
  - G(4), Y(7), O(8) values also differ
  - Same pattern: values shifted in position
```

## Hypotheses

### Hypothesis 1: Interpreter step() semantics differ
**Status:** INVESTIGATED - Partially fixed

C# main loop:
```csharp
while (current != null && (steps <= 0 || counter < steps)) {
    current.Go();
    counter++;  // ALWAYS increments
}
```

Rust (after fix):
```rust
while self.running && (max_steps == 0 || self.counter < max_steps) {
    self.step();  // Now always increments counter
}
```

The interpreter was fixed to always increment counter. But the 2-step offset persists.

### Hypothesis 2: Branch node `ip.current` tracking
**Status:** PARTIALLY INVESTIGATED

C# has complex `ip.current` tracking where:
1. When branch child succeeds, `ip.current = child`
2. Main loop calls `ip.current.Go()` directly (not parent)
3. When child fails, `ip.current = parent`
4. Counter increments between child failing and parent retrying

The `branch_child_active` retry mechanism was removed. Step counts may differ when branch children complete.

### Hypothesis 3: Steps limit off-by-one
**Status:** NOT YET TESTED

C# condition: `counter < steps` (exclusive upper bound)
Rust condition: `steps < limit` (same semantics?)

Need to verify exact boundary conditions.

### Hypothesis 4: Model-specific node interaction
**Status:** LIKELY

The 2-step difference happens somewhere in the model BEFORE the final `<all>` block. Could be:
- `<markov>` block completion counting
- `<one file="Chip">` WFC node step counting
- Nested sequence/markov interaction

## How to Prove the Issue

### Test 1: Run without step limit
If results match without step limit, confirms it's purely a step counting issue.

```bash
# Modify models.xml to remove steps limit for Circuit
# Then regenerate and compare
```

### Test 2: Add step logging to each node
Add counter logging at entry/exit of each major node's Go() method:
- Log `ip.counter` / `ctx.counter` at node entry
- Log return value
- Compare C# vs Rust traces

### Test 3: Binary search for step divergence
Run model for N steps (N < 1200) and compare:
- Find the exact step where counters diverge
- Identify which node is executing at that step

### Test 4: Check specific node types
Focus on nodes that might consume steps differently:
- `<one file="...">` WFC nodes (have internal steps)
- `<markov>` blocks (loop until no child succeeds)
- `<path>` nodes (multiple internal operations)

## Related Models

Models with similar step-limited animation behavior:
- Circuit: 94.46% (steps=1200)
- CircuitSimple: 98.16% (steps=1200)

Models that may have related issues:
- TilePath: 86.83% (has WFC)
- TileDungeon: 75.62% (has WFC)

## Commands

```bash
# Generate C# reference
cd MarkovJunior && dotnet run -- --model Circuit --seed 42 --dump-json

# Generate Rust output
MJ_MODELS=Circuit MJ_SEED=42 cargo test -p studio_core batch_generate_outputs -- --ignored --nocapture

# Compare outputs
python3 scripts/compare_grids.py MarkovJunior/verification/Circuit_seed42.json verification/rust/Circuit_seed42.json
```

## Files Modified

### Debug files created (can be removed)
- `MarkovJunior/models/CircuitSimple.xml` - Simplified model for testing
- `MarkovJunior/models/CircuitSimple2.xml` - Even simpler, 100% match

### Code changes made
1. `interpreter.rs`: Changed to always increment counter (matching C#)
2. `node.rs`: Removed `branch_child_active` mechanism, simplified SequenceNode

## Investigation Log

### WFC Completion Step Fix (Attempted)

**Hypothesis:** C# WFCNode.Go() returns `true` on the step when WFC completes (line 117: `return true;`), even if there are no children. Our Rust code returned `false` immediately when WFC completed with no children.

**Fix Applied:** Modified TileNode and OverlapNode to always return `true` on WFC completion, letting the next call handle the "no children" case.

**Result:** No change to Circuit. The fix was correct (matches C# behavior) but doesn't explain the 2-step offset.

### Remaining Hypotheses

1. **Something else in the model consumes different steps** - Need to trace step-by-step

2. **Path node step counting** - The `<path>` node in Circuit might have different behavior

3. **Markov block exit behavior** - When a Markov block's children all fail, how many steps does that consume?

## Next Steps

1. **Add detailed step logging** - Log at entry/exit of each node's Go() with counter value
2. **Binary search for divergence** - Run for N steps and find where counters first differ
3. **Check Path node** - The `<path from="A" to="U"...>` might have step counting differences
