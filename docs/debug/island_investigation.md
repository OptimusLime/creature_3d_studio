# Island Investigation

**Status:** RESOLVED
**Model:** Island.xml (2D, 800x800)
**Initial Match:** 0.15% (639,061 cells differ out of 640,000)
**Final Match:** 100%
**Seed:** 42

## Summary

Island is a complex procedural terrain generator that creates islands with coastlines, rivers, mountains, forests, and beaches. It uses nearly every node type: `sequence`, `one`, `all`, `prl`, `convolution`, and critically `field` + `observe` for river pathfinding.

Two bugs were found and fixed:
1. **Step limit parsing** - `steps="-1"` was being parsed incorrectly
2. **SequenceNode n increment** - Active branch child completion was incorrectly advancing to next child

## Root Cause #1: Step Limit Bug (FIXED)

### Discovery

Using side-by-side debug logging in C# and Rust OneNode.Go():

**C# (634,065 Voronoi steps):**
```
[OneNode] matchCount=8722, rules=8, counter=0, steps=0
[OneNode] matchCount=8724, rules=8, counter=1, steps=0
...
[OneNode] matchCount=180, rules=8, counter=634061, steps=0
```

**Rust (47,277 Voronoi steps - stopped early!):**
```
[OneNode] matchCount=8722, rules=8, counter=0, steps=0
[OneNode] matchCount=8724, rules=8, counter=1, steps=0
...
[OneNode] matchCount=34251, rules=8, counter=47273, steps=0
```

### Root Cause

In `verification.rs` line 134:
```rust
"steps" => steps = val.parse().unwrap_or(50000),
```

When XML has `steps="-1"` (unlimited), parsing as `usize` fails for negative values, defaulting to 50000. Island's many OneNodes consumed ~50000 total interpreter steps.

### Fix

```rust
"steps" => {
    // steps="-1" means unlimited, use 0 which our run loop treats as unlimited
    let parsed: i64 = val.parse().unwrap_or(50000);
    steps = if parsed < 0 { 0 } else { parsed as usize };
}
```

### Result

- Island: 0.15% -> 77.55%

## Root Cause #2: SequenceNode n Increment Bug (FIXED)

### Discovery

Created layer test models IslandL11-L20 to isolate the remaining 22.45% divergence:

| Layer | Content | Status |
|-------|---------|--------|
| IslandL11 | ocean painting | 100% |
| IslandL12 | UI->UU | 100% |
| IslandL13 | coast marking | 100% |
| IslandL14 | shallow water | 100% |
| IslandL15 | deep ocean | 100% |
| IslandL16 | river source setup | 100% |
| **IslandL17** | **river sequence with field+observe** | **FAIL (99.87%)** |
| IslandL17a | just field | FAIL (99.99%) |

### Debug Logging

Added identical debug logging to C# OneNode.Go() and Rust one_node.go():

```csharp
// C#
Console.WriteLine($"[OneNode.Go] counter={counter} matchCount={matchCount} steps={steps} potentials={(potentials != null ? "yes" : "no")}");
```

```rust
// Rust
eprintln!("[OneNode.Go] counter={} matchCount={} steps={} potentials={}", ...);
```

**C# output (77 calls to field-guided OneNode):**
```
[OneNode.Go] counter=0 matchCount=92 steps=1 potentials=yes
[OneNode.Go] RandomMatch returned R=1 X=514 Y=432 Z=0
[OneNode.Go] Applied rule 1 at (514,432,0), counter now 1
[OneNode.Go] counter=0 matchCount=91 steps=1 potentials=yes   <- reset, retry
[OneNode.Go] RandomMatch returned R=2 X=515 Y=431 Z=0
...
```

**Rust output (1 call to field-guided OneNode):**
```
[OneNode.Go] counter=0 matchCount=92 steps=1 potentials=yes
[OneNode.Go] RandomMatch returned R=1 X=514 Y=432 Z=0
[OneNode.Go] Applied rule 1 at (514,432,0), counter now 1
<nothing more>
```

### Root Cause

In `node.rs`, SequenceNode's handling of active branch child completion:

**Before (WRONG):**
```rust
if let Some(active_idx) = self.active_branch_child {
    if self.nodes[active_idx].go(ctx) { return true; }
    self.active_branch_child = None;
    self.n += 1;  // <-- BUG: Advancing past the child
    return true;
}
```

**C# behavior:** When a sequence's branch child fails after being active, control returns to the parent. The parent's `n` was never incremented (because it returned via `return true` last time). On the next call, the for-loop starts at the same `n`, retrying the same (now reset) child.

**Rust behavior (buggy):** We were incrementing `n` when the active child failed, skipping to the next child instead of retrying.

### Fix

```rust
if let Some(active_idx) = self.active_branch_child {
    if self.nodes[active_idx].go(ctx) { return true; }
    // Child failed - it has already reset itself
    // IMPORTANT: Do NOT increment n! Parent should retry the same child.
    self.active_branch_child = None;
    // Don't increment n - we'll try the same child again, which has now been reset
    return true;
}
```

### Result

- Island: 77.55% -> 100%
- All 109 2D models: 100%

## Layer Test Results (Final)

| Layer | Content | Status |
|-------|---------|--------|
| IslandL1 | origin + prl | 100% |
| IslandL2 | L1 + first one | 100% |
| IslandL3 | L2 + backbone growth | 100% |
| IslandL4 | L3 + marker | 100% |
| IslandL5 | L4 + branch seeds | 100% |
| IslandL6 | L5 + branch growth | 100% |
| IslandL7 | L6 + all R->W | 100% |
| IslandL8 | L7 + low-prob prl | 100% |
| IslandL9 | L8 + Voronoi | 100% |
| IslandL10 | L9 + convolution | 100% |
| IslandL11-L20 | various | 100% |
| **Full Island** | **All content** | **100%** |

## Files Modified

- `crates/studio_core/src/markov_junior/verification.rs` - Fixed step limit parsing
- `crates/studio_core/src/markov_junior/node.rs` - Fixed SequenceNode n increment bug

## Commands

```bash
# Run full 2D verification
python3 scripts/batch_verify.py --all-2d --regenerate

# Test specific model
python3 scripts/batch_verify.py Island --regenerate
```

## Key Takeaway

The debugging methodology from HOW_WE_WORK.md was essential:
1. Create layer test models to isolate failure point (IslandL17a)
2. Add **identical** debug logging to C# and Rust at **same** points
3. Compare line-by-line to find divergence (77 calls vs 1 call)
4. Trace C# behavior to understand correct semantics
5. Fix Rust to match
