# MarkovJunior C# Verification Phases

## Summary

Systematic verification of all 135 MarkovJunior models against the C# reference implementation. **100% cell-by-cell match is the only acceptable outcome** - anything less indicates a bug that must be fixed before proceeding.

## Context & Motivation

The Rust port compiles and produces visual output, but "looks right" is not verification. We need deterministic proof that every model produces **identical** output to C# given the same seed. This ensures:
1. Algorithm correctness
2. No hidden bugs masquerading as "slightly different random output"
3. Confidence to use in production

## Naming Conventions

- `verification/status.json` - Tracking file for all model results
- `MarkovJunior/verification/{Model}_seed{N}.json` - C# reference outputs
- `verification/rust/{Model}_seed{N}.json` - Rust outputs
- `scripts/batch_verify.py` - Primary verification tool

## Current Status

**Verified (8 models, 100% match):** Basic, River, Growth, Flowers, MazeGrowth, MazeBacktracker, Cave, Backtracker

**Failed (3 models):** ChainMaze (62.92%), BiasedGrowth (94.47%), DungeonGrowth (99.55%)

**Pending:** 121 models untested

---

## Phase 6.0: Full Model Census

**Outcome:** Every model has a verification result (pass/fail/skip with reason)

**Verification:** `python3 scripts/verification_status.py status` shows 0 pending models

### Tasks

1. [ ] Run batch verification on all 2D models (~100 models)
   - Command: `python3 scripts/batch_verify.py --all-2d`
   - Expected: ~1-2 hours runtime
   
2. [ ] Run batch verification on all 3D models (~35 models)
   - Command: `python3 scripts/batch_verify.py --all-3d`
   - Some may timeout or require special handling

3. [ ] Categorize results into buckets:
   - **VERIFIED (100%)**: No action needed
   - **CLOSE (>95%)**: Priority debug targets
   - **PARTIAL (50-95%)**: Secondary debug targets
   - **BROKEN (<50%)**: Likely missing feature
   - **SKIPPED**: Known unsupported (WFC tile, etc.)

4. [ ] Update `verification/status.json` with all results

5. [ ] Create summary report showing coverage

### Success Criteria
- All 135 models have a result entry
- Results categorized by accuracy bucket
- Clear list of models to fix, sorted by accuracy (highest first)

---

## Phase 6.1: Fix High-Accuracy Failures (>95%)

**Outcome:** All models with >95% match reach 100%

**Verification:** `python3 scripts/batch_verify.py MODEL` returns 100% for each

### Approach

Models >95% have **small, localized bugs**. Fix strategy:
1. Use bisection tool to find first divergence point
2. Compare step-by-step with C# debug output
3. Fix the specific bug
4. Verify 100% match

### Target Models (from current status)
- DungeonGrowth: 99.55% (28 cells differ)
- BiasedGrowth: 94.47% (796 cells differ)
- [Add more after Phase 6.0 completes]

### Tasks

1. [ ] Create incremental bisect tool for debugging
   - `scripts/bisect_model.py MODEL` - finds first divergent step
   
2. [ ] For each >95% model:
   - [ ] Run bisect to find divergence point
   - [ ] Form hypothesis about cause
   - [ ] Implement fix
   - [ ] Verify 100% match
   - [ ] Commit fix with test

### Success Criteria
- All models that were >95% now show 100%
- Each fix committed with explanation

---

## Phase 6.2: Fix Medium-Accuracy Failures (50-95%)

**Outcome:** All models with 50-95% match reach 100%

**Verification:** Same as Phase 6.1

### Approach

Models in this range likely have:
- Wrong node type behavior
- Missing symmetry handling
- Incorrect match selection

### Target Models (from current status)
- ChainMaze: 62.92%
- [Add more after Phase 6.0 completes]

### Tasks

1. [ ] For each 50-95% model:
   - [ ] Identify which node type is causing issues
   - [ ] Compare node implementation to C# reference
   - [ ] Fix systematic bug
   - [ ] Verify fix doesn't break other models
   - [ ] Verify 100% match

### Success Criteria
- All models that were 50-95% now show 100%
- Each fix committed with explanation

---

## Phase 6.3: Fix Low-Accuracy Failures (<50%)

**Outcome:** All models reach 100% or documented as unsupported

**Verification:** Same as above, or explicit skip entry with reason

### Approach

Models <50% likely have:
- Missing node types entirely
- Fundamental algorithm differences
- Unsupported features (WFC tiles, 3D overlap, etc.)

### Tasks

1. [ ] Categorize each <50% model:
   - Missing feature → add to SKIPPED with reason
   - Bug → fix and verify
   
2. [ ] Document unsupported features in `DEVIATIONS.md`

3. [ ] For fixable models, apply same process as Phase 6.2

### Success Criteria
- All models either 100% or explicitly skipped with documented reason
- No "unknown failure" states

---

## Phase 6.4: Multi-Seed Verification

**Outcome:** Verified models pass with multiple seeds

**Verification:** Each verified model passes with seeds 42, 123, 999

### Rationale

A single seed could mask bugs through coincidence. Testing multiple seeds proves:
- RNG integration is correct across different paths
- No seed-specific shortcuts in logic

### Tasks

1. [ ] Update batch_verify.py to support `--seeds 42,123,999`

2. [ ] Run multi-seed verification on all verified models

3. [ ] Fix any models that fail on different seeds

### Success Criteria
- All verified models pass with 3 different seeds
- Any seed-specific failures fixed

---

## Phase 6.5: Regression Test Suite

**Outcome:** Automated CI-ready verification suite

**Verification:** `cargo test -p studio_core verification_regression` passes

### Tasks

1. [ ] Create `#[test]` for each verified model that:
   - Loads model
   - Runs with DotNetRandom seed 42
   - Compares against committed reference JSON
   - Fails if not 100% match

2. [ ] Commit C# reference JSONs to repo (or generate on demand)

3. [ ] Add to CI pipeline

### Success Criteria
- Any code change that breaks verification fails CI
- Easy to add new models as they're verified

---

## Incremental Bisect Tool Design

For debugging divergences, we need a tool that:

1. Runs model step-by-step
2. Captures grid state after each step
3. Compares to C# step-by-step output
4. Reports first step where divergence occurs

### Usage
```bash
# Generate step-by-step C# output
cd MarkovJunior && dotnet run -- --model DungeonGrowth --seed 42 --dump-steps

# Run Rust bisect
python3 scripts/bisect_model.py DungeonGrowth --seed 42

# Output:
# Step 0: MATCH (6241 cells)
# Step 1: MATCH (6241 cells)
# ...
# Step 47: DIVERGE at (15, 23, 0) - C#=2 Rust=1
# First divergence at step 47
```

This tells us exactly where to look in the execution trace.

---

## Full Outcome Across All Phases

After completing all phases:
- **100% of models verified** against C# reference
- **Automated regression suite** prevents future breakage
- **Documented exceptions** for any unsupported features
- **Multi-seed verification** proves RNG correctness

---

## Directory Structure

```
verification/
  status.json              # Master tracking file
  rust/                    # Rust output JSONs
    Basic_seed42.json
    ...
    
MarkovJunior/
  verification/            # C# reference JSONs
    Basic_seed42.json
    ...
    
scripts/
  batch_verify.py          # Main verification driver
  verification_status.py   # Status reporting
  compare_grids.py         # Single model comparison
  bisect_model.py          # Step-by-step debugging (TODO)

docs/markov_junior/
  VERIFICATION_PHASES.md   # This file
  DEVIATIONS.md            # Documented differences/unsupported
```

---

## How to Review Progress

1. Run `python3 scripts/verification_status.py status -v` for current state
2. Check `verification/status.json` for detailed results
3. Each phase should be committed when complete with updated status

---

## Commands Reference

```bash
# Check overall status
python3 scripts/verification_status.py status -v

# List unverified models
python3 scripts/verification_status.py list-unverified

# Verify specific models
python3 scripts/batch_verify.py Basic River Growth

# Verify all 2D models
python3 scripts/batch_verify.py --all-2d

# Verify all models (slow)
python3 scripts/batch_verify.py --all

# Compare single model manually
python3 scripts/compare_grids.py MarkovJunior/verification/Basic_seed42.json verification/rust/Basic_seed42.json

# Generate C# reference
cd MarkovJunior && dotnet run -- --model Basic --seed 42 --dump-json
```
