# Island Investigation

**Status:** IN PROGRESS
**Model:** Island.xml (2D, 800x800)
**Initial Match:** 0.15% (639,061 cells differ out of 640,000)
**Current Match:** 77.55% (143,674 cells differ)  
**Seed:** 42

## Summary

Island is a complex procedural terrain generator that creates islands with coastlines, rivers, mountains, forests, and beaches. It uses nearly every node type: `sequence`, `one`, `all`, `prl`, `convolution`, and critically `field` + `observe` for river pathfinding.

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

When XML has `steps="-1"` (unlimited), parsing as `usize` fails for negative values, defaulting to 50000. Island's many OneNodes consumed ~50000 total interpreter steps:
- `<one steps="1"/>` = 1
- `<one steps="400"/>` = 400  
- `<one steps="20"/>` = 20
- `<one steps="2300"/>` = 2300
- Voronoi `<one/>` = 47,274
- **Total: ~50,000 (hit the limit)**

### Fix

```rust
"steps" => {
    // steps="-1" means unlimited, use 0 which our run loop treats as unlimited
    let parsed: i64 = val.parse().unwrap_or(50000);
    steps = if parsed < 0 { 0 } else { parsed as usize };
}
```

And in the run loop:
```rust
while interpreter.is_running() && (limit == 0 || steps < limit) {
```

### Result

- IslandL1-L10: ALL 100% match
- Full Island: 0.15% -> 77.55%

## Remaining Issue

Island is at 77.55% (143,674 cells differ). The issue is somewhere between L10 and the full model. Need to create more layer models to isolate.

## Layer Test Results

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
| ... | ... | TBD |
| Full Island | All content | 77.55% |

## Hypothesis for Remaining Issue

Looking at Island.xml beyond L10, the next sections are:
1. Ocean painting with symmetry `(x)`
2. More prl/all operations
3. **River pathfinding with `<field>` + `<observe>`** (likely culprit)
4. More convolution smoothing
5. Beach, forest, mountain generation

The `field`+`observe` pattern is used in passing models (BiasedGrowth, etc.) but Island uses it in a nested `<sequence>` context which may expose different bugs.

## Debugging Plan

### Phase 1: Isolate Failure Point (CURRENT)

Create IslandL11-L20 to find exactly which section fails:
- L11: L10 + ocean painting `all in="U/*" out="I/*" symmetry="(x)"`
- L12: L11 + `all in="UI" out="UU"`
- L13: L12 + coast marking
- L14: L13 + shallow water prl
- L15: L14 + deep ocean conversion
- L16: L15 + river source setup
- **L17: L16 + river sequence with field+observe** (suspect)
- Continue until failure found

### Phase 2: Fix the Bug

Once isolated, compare C# vs Rust execution at the failing section.

### Phase 3: Verify

After fix, verify all layers pass and full Island reaches 100%.

## Commands

```bash
# Generate C# reference
cd MarkovJunior && dotnet run -- --model Island --seed 42 --dump-json

# Generate Rust output  
MJ_MODELS=Island MJ_SEED=42 cargo test -p studio_core batch_generate_outputs -- --ignored --nocapture

# Compare outputs
python3 scripts/compare_grids.py MarkovJunior/verification/Island_seed42.json verification/rust/Island_seed42.json

# Test layer models
python3 scripts/batch_verify.py IslandL1 IslandL2 ... --regenerate
```

## Files Modified

- `crates/studio_core/src/markov_junior/verification.rs` - Fixed step limit parsing

## Related Models

Other models with `steps="-1"` that may have been affected:
- Island (0.15% -> 77.55%)
- Any model in models.xml with negative steps value
