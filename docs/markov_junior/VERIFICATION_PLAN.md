# MarkovJunior Verification Plan - Cell-by-Cell Accuracy

## Problem Statement

The Rust port passes 306 tests and produces visible structure for 3D models. However, **visual inspection is insufficient** - output looks "close but not quite right." We need **100% cell-by-cell accuracy** verification against the C# reference implementation.

## Key Breakthrough: RNG Compatibility

We now have `.NET System.Random` compatibility via the `clr_random` crate:

```rust
use studio_core::markov_junior::rng::{MjRng, DotNetRandom, StdRandom};

// For C# verification - produces IDENTICAL sequences to .NET
let mut rng = DotNetRandom::from_seed(42);
assert_eq!(rng.next_int(), 1434747710); // Same as C# new Random(42).Next()

// For normal Rust use
let mut rng = StdRandom::from_seed(42);
```

This means **same seed = same random sequence = same output** (if our logic is correct).

---

## Verification Phases Overview

| Phase | Description | Outcome |
|-------|-------------|---------|
| **5.0** | RNG Integration | ExecutionContext uses MjRng trait |
| **5.1** | C# Grid Capture | Modify C# to dump grid state as JSON |
| **5.2** | Rust Grid Capture | Rust outputs matching JSON format |
| **5.3** | Comparison Tool | Cell-by-cell diff script |
| **5.4** | Simple Model Test | Verify Basic/River match 100% |
| **5.5** | 3D Model Batch | Run all 3D models, identify failures |
| **5.6** | Debug & Fix | Hypothesis-driven debugging |

---

## Phase 5.0: RNG Integration into ExecutionContext

**Status:** Design decision needed

**Problem:** Current `ExecutionContext` uses `StdRng` directly. We need to support `DotNetRandom` for verification without massive refactoring.

### Option A: Generic ExecutionContext (High Effort)
```rust
pub struct ExecutionContext<'a, R: MjRng> {
    pub grid: &'a mut MjGrid,
    pub random: &'a mut R,
    // ...
}
```
- Requires changing `Node` trait to be generic
- Touches every node implementation
- Clean but massive refactor

### Option B: Trait Object (Medium Effort)
```rust
pub struct ExecutionContext<'a> {
    pub grid: &'a mut MjGrid,
    pub random: &'a mut dyn MjRng,
    // ...
}
```
- No generics needed
- Small performance cost (vtable dispatch)
- Reasonable refactor

### Option C: Parallel Verification Harness (Low Effort) **RECOMMENDED**
- Keep production code using `StdRng`
- Create separate verification module that:
  1. Loads model XML
  2. Creates grid manually
  3. Runs execution loop using `DotNetRandom`
  4. Captures output for comparison

**Recommendation:** Start with Option C for fast iteration, consider Option B later if needed.

### Tasks
1. [ ] Create `verification.rs` module with standalone execution harness
2. [ ] Harness uses `DotNetRandom` directly (not through ExecutionContext)
3. [ ] Test on simple model to confirm RNG is being used correctly

---

## Phase 5.1: C# Grid State Capture

**Outcome:** C# outputs grid state as JSON after model completes

### Approach
Modify C# `Program.cs` to add `--dump-json` flag:

```csharp
// After model completes, dump grid state
if (args.Contains("--dump-json")) {
    var output = new {
        model = name,
        seed = seed,
        dimensions = new[] { grid.MX, grid.MY, grid.MZ },
        characters = grid.characters.Select(c => c.ToString()).ToArray(),
        state = grid.state.Select(b => (int)b).ToArray()
    };
    File.WriteAllText($"output/{name}_seed{seed}.json", 
        JsonSerializer.Serialize(output, new JsonSerializerOptions { WriteIndented = true }));
}
```

### Tasks
1. [ ] Add JSON output to C# Program.cs
2. [ ] Test: `cd MarkovJunior && dotnet run -- --model Basic --seed 42 --dump-json`
3. [ ] Verify JSON contains expected fields
4. [ ] Run for multiple seeds to confirm determinism

### Output Format
```json
{
  "model": "Basic",
  "seed": 42,
  "dimensions": [60, 60, 1],
  "characters": ["B", "W"],
  "state": [0, 0, 1, 1, 0, 1, ...]
}
```

---

## Phase 5.2: Rust Grid State Capture

**Outcome:** Rust outputs identical JSON format for comparison

### Approach
Create verification test that:
1. Loads model using existing `load_model()`
2. Runs with `DotNetRandom` seed
3. Outputs JSON matching C# format

```rust
// In verification.rs
pub fn capture_model_state(model_name: &str, seed: i32) -> ModelState {
    let (mut grid, root) = load_model_for_verification(model_name);
    let mut rng = DotNetRandom::from_seed(seed);
    
    // Run model to completion
    run_with_rng(&mut grid, root, &mut rng, 50000);
    
    ModelState {
        model: model_name.to_string(),
        seed,
        dimensions: [grid.mx, grid.my, grid.mz],
        characters: grid.characters.clone(),
        state: grid.state.clone(),
    }
}

#[test]
fn capture_basic_seed_42() {
    let state = capture_model_state("Basic", 42);
    let json = serde_json::to_string_pretty(&state).unwrap();
    std::fs::write("verification/rust/Basic_seed42.json", json).unwrap();
}
```

### Tasks
1. [ ] Create `verification.rs` module
2. [ ] Implement `load_model_for_verification()` that returns (grid, root_node)
3. [ ] Implement `run_with_rng()` that uses `DotNetRandom`
4. [ ] Create test that outputs JSON
5. [ ] Verify JSON format matches C#

---

## Phase 5.3: Comparison Tool

**Outcome:** Script that compares C# and Rust JSON outputs cell-by-cell

### Script: `scripts/compare_grids.py`

```python
#!/usr/bin/env python3
import json
import sys

def compare(csharp_path, rust_path):
    with open(csharp_path) as f:
        csharp = json.load(f)
    with open(rust_path) as f:
        rust = json.load(f)
    
    # Check dimensions
    if csharp["dimensions"] != rust["dimensions"]:
        print(f"DIMENSION MISMATCH: C#={csharp['dimensions']} Rust={rust['dimensions']}")
        return
    
    # Compare cell-by-cell
    total = len(csharp["state"])
    diffs = []
    for i, (c, r) in enumerate(zip(csharp["state"], rust["state"])):
        if c != r:
            mx, my, mz = csharp["dimensions"]
            x = i % mx
            y = (i // mx) % my
            z = i // (mx * my)
            diffs.append((x, y, z, c, r))
    
    # Report
    matching = total - len(diffs)
    print(f"Model: {csharp['model']}")
    print(f"Seed: {csharp['seed']}")
    print(f"Dimensions: {csharp['dimensions']}")
    print(f"Total cells: {total}")
    print(f"Matching: {matching} ({100*matching/total:.2f}%)")
    print(f"Different: {len(diffs)}")
    
    if diffs:
        print(f"\nFirst 20 differences:")
        for x, y, z, c, r in diffs[:20]:
            c_char = csharp["characters"][c] if c < len(csharp["characters"]) else "?"
            r_char = rust["characters"][r] if r < len(rust["characters"]) else "?"
            print(f"  ({x},{y},{z}): C#={c}({c_char}) Rust={r}({r_char})")

if __name__ == "__main__":
    compare(sys.argv[1], sys.argv[2])
```

### Tasks
1. [ ] Create `scripts/compare_grids.py`
2. [ ] Test on manually created test files
3. [ ] Add batch mode for comparing all models

---

## Phase 5.4: Simple Model Verification

**Outcome:** Basic and River models match C# 100%

### Why These Models?
- **Basic**: Simplest model (just `B -> W` rule)
- **River**: Simple 2D model with multiple rules

If these don't match 100%, we have fundamental issues.

### Tasks
1. [ ] Generate C# output: `Basic_seed42.json`, `River_seed42.json`
2. [ ] Generate Rust output for same models/seeds
3. [ ] Run comparison
4. [ ] If 100%: proceed to Phase 5.5
5. [ ] If <100%: debug before proceeding

### Expected Issues
- RNG call order differences
- Rule application order
- Match selection randomness

---

## Phase 5.5: 3D Model Batch Verification

**Outcome:** Accuracy report for all 3D models

### 3D Models to Test

| Model | Grid Size | Complexity |
|-------|-----------|------------|
| ParallelGrowth | 29x29x29 | Simple growth |
| ClosedSurface | 12x12x12 | Surface generation |
| ColoredKnots | 12x12x12 | Knot patterns |
| Apartemazements | 8x8x8 (WFC->40x40x40) | WFC + children |
| CarmaTower | 12x12x18 | Complex structure |

### Tasks
1. [ ] Generate C# JSON for each model (seed 0, 42, 12345)
2. [ ] Generate Rust JSON for each model
3. [ ] Run batch comparison
4. [ ] Categorize results:
   - **PERFECT**: 100% match
   - **HIGH**: >99% match
   - **PARTIAL**: <99% match
   - **FAILED**: Could not run

### Report Format
```
3D Model Verification Report
============================

PERFECT (100%):
  ParallelGrowth (seed=42): 24389/24389 cells match

HIGH (>99%):
  ClosedSurface (seed=42): 1726/1728 cells match (99.88%)
    First diff at (5,5,5): C#=2 Rust=1

PARTIAL (<99%):
  Apartemazements (seed=42): 58000/64000 cells match (90.63%)
    Pattern: All diffs in Z>5 region
    
FAILED:
  CarmaTower: Missing VOX file resources/vox/tower.vox
```

---

## Phase 5.6: Hypothesis-Driven Debugging

**Outcome:** All models reach 100% or have documented limitations

### Debug Process

For each non-100% model:

1. **Analyze diff pattern**
   - Are diffs random or clustered?
   - Do they follow coordinate patterns (edge, center, Z-layer)?
   - What values are wrong?

2. **Form hypothesis**
   - "RNG called extra time in rule matching"
   - "3D symmetry rotation order differs"
   - "WFC propagation visits cells in different order"

3. **Create minimal reproduction**
   ```rust
   #[test]
   fn test_hypothesis_rng_call_order() {
       // Minimal setup that reproduces the issue
   }
   ```

4. **Fix and verify**
   - Implement fix
   - Re-run comparison
   - Confirm improvement

5. **Iterate**

### Common Issue Categories

| Category | Symptom | Likely Cause |
|----------|---------|--------------|
| Random offset | All values shifted by N | RNG called N extra times |
| Edge errors | Only edge cells wrong | Boundary handling |
| Z-layer errors | Specific Z values wrong | 3D indexing bug |
| Pattern errors | Repeating wrong pattern | Symmetry/rotation |
| WFC errors | Random scattered diffs | Propagation order |

---

## Directory Structure

```
verification/
  csharp/
    Basic_seed42.json
    River_seed42.json
    ParallelGrowth_seed42.json
    ...
  rust/
    Basic_seed42.json
    River_seed42.json
    ParallelGrowth_seed42.json
    ...
  reports/
    comparison_report.txt
    
scripts/
  compare_grids.py          # Cell-by-cell comparison
  batch_compare.sh          # Run all comparisons
  
crates/studio_core/src/markov_junior/
  rng.rs                    # MjRng trait + implementations (DONE)
  verification.rs           # Verification harness (TODO)
```

---

## Success Criteria

### Phase 5.0 Complete When:
- [x] `MjRng` trait defined
- [x] `StdRandom` implementation works
- [x] `DotNetRandom` implementation matches C# sequences
- [ ] Decision made on integration approach

### Phase 5.1 Complete When:
- [ ] C# outputs JSON grid state
- [ ] JSON format documented
- [ ] Multiple seeds produce deterministic output

### Phase 5.2 Complete When:
- [ ] Rust outputs matching JSON format
- [ ] Verification harness uses `DotNetRandom`
- [ ] Can capture any loadable model

### Phase 5.3 Complete When:
- [ ] Comparison script works
- [ ] Reports accuracy percentage
- [ ] Shows coordinate-level diffs

### Phase 5.4 Complete When:
- [ ] Basic model matches 100%
- [ ] River model matches 100%
- [ ] Or bugs identified and documented

### Phase 5.5 Complete When:
- [ ] All 3D models have comparison results
- [ ] Results categorized
- [ ] Patterns identified

### Phase 5.6 Complete When:
- [ ] Each failure category has hypothesis
- [ ] Fixes improve accuracy
- [ ] All models at 100% OR limitations documented

---

## Quick Start Commands

```bash
# Build C#
cd MarkovJunior && dotnet build

# Generate C# reference (after adding --dump-json)
dotnet run -- --model Basic --seed 42 --dump-json

# Run Rust verification test
cargo test -p studio_core verification::capture_basic -- --nocapture

# Compare outputs
python scripts/compare_grids.py verification/csharp/Basic_seed42.json verification/rust/Basic_seed42.json

# Batch compare all
./scripts/batch_compare.sh
```
