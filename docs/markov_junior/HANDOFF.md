# MarkovJunior Rust Port - Handoff Document

## Current State

**Branch:** `feature/markov-junior-rust`
**Phase:** Phase 4.2 - Full Model Verification Complete, Bug Fixes Needed
**Tests:** 285 passing
**Models:** 120/157 loaded (76%), 76 completed, 44 partial

---

## IMMEDIATE NEXT STEPS

### Priority 1: Investigate 3D Growth Bug
**ParallelGrowth** produces only 15 cells instead of ~24,000 in a 29x29x29 grid.
- Model: `MarkovJunior/models/ParallelGrowth.xml`
- XML: `<all values="BW" origin="True" in="WB" out="*W"/>`
- Expected: Fill entire grid from origin
- Actual: Only 15 cells

**Likely causes:**
1. 3D symmetry not being applied to AllNode rules
2. Origin not placed correctly in 3D
3. Rule matching failing in Z dimension

**Debug approach:**
```bash
# Add debug test in render.rs
cargo test -p studio_core test_parallel_growth_debug -- --nocapture
```

### Priority 2: Implement `(xy)` Symmetry (5 models blocked)
Models: Apartemazements, CarmaTower, Partitioning, PillarsOfEternity, StairsPath

The `(xy)` symmetry subgroup is not implemented. Check `symmetry.rs` for existing subgroups and add `(xy)`.

### Priority 3: Implement `<observe from="...">` (16 models blocked)
Models using observe with `from` attribute fail to load. This is used for constraint-based generation.

---

## Full Verification Results

### Summary
| Category | Count | Percentage |
|----------|-------|------------|
| Total Models | 157 | 100% |
| Loaded Successfully | 120 | 76% |
| Completed (DONE) | 76 | 48% |
| Partial (hit step limit) | 44 | 28% |
| Failed to Load | 37 | 24% |

### Load Failures by Category

#### Missing `from` attribute in `<observe>` (16 models)
```
BishopParity, CompleteSAW, CompleteSAWSmart, CrossCountry, DiagonalPath,
EuclideanPath, Island, KnightPatrol, MultiSokoban8, MultiSokoban9,
RegularPath, SequentialSokoban, SnellLaw, SokobanLevel1, SokobanLevel2,
StormySnellLaw
```

#### Unknown symmetry `(xy)` (5 models)
```
Apartemazements, CarmaTower, Partitioning, PillarsOfEternity, StairsPath
```

#### Missing tileset files (12 models)
```
ClosedSurface, ColoredKnots, Escher, EscherSurface, Knots2D, Knots3D,
OrientedEscher, PeriodicEscher, SelectLongKnots, SubmergedKnots, Surface,
TileDungeon, TilePath
```

#### Invalid union symbol (2 models)
```
ModernHouse (symbol '.'), SeaVilla (symbol '?')
```

#### Image loading error (1 model)
```
DualRetraction3D (file extension issue)
```

### Suspicious 3D Results (INVESTIGATE)

| Model | Size | Steps | Cells | Issue |
|-------|------|-------|-------|-------|
| ParallelGrowth | 29x29x29 | 14 | 15 | Should be ~24k cells |
| Hills | 40x40x12 | 3 | 0 | Zero cells produced |

### 3D Models That Ran Successfully

| Model | Size | Steps | Cells | Status |
|-------|------|-------|-------|--------|
| Counting | 8x8x8 | 1 | 512 | DONE |
| OddScale3D | 8x8x8 | 1176 | 26289 | DONE |
| OpenCave3D | 40x40x40 | 69 | 10830 | DONE |

---

## Screenshots Location

All 120 model screenshots saved to:
```
screenshots/verification/all_models/
```

Filename format: `ModelName_done.png` or `ModelName_partial.png`

---

## Bug Fixed This Session

### Critical: `load_children_from_xml` Depth Tracking (commit b9b54cd)

**Problem:** When parsing nested XML elements, `read_element_content()` consumed the closing End event but the outer depth counter was never decremented. This caused sibling nodes after nested elements to be skipped.

**Impact:** River.xml, Circuit.xml, and other models with nested `<one>` or `<all>` elements were missing later phases.

**Fix:** Added `depth -= 1` after calling `read_element_content()` in `loader.rs:1941`

---

## Key Commands

```bash
# Run ALL 157 models, save screenshots
cargo test -p studio_core test_run_all_markov_models -- --nocapture

# Run specific 2D verification
cargo test -p studio_core test_verification_run_all_2d_models -- --nocapture

# Run specific 3D verification  
cargo test -p studio_core test_verification_run_all_3d_models -- --nocapture

# Run all MJ tests
cargo test -p studio_core markov_junior
```

---

## Key Files for Debugging

| File | Purpose |
|------|---------|
| `crates/studio_core/src/markov_junior/loader.rs` | XML parsing, symmetry handling |
| `crates/studio_core/src/markov_junior/symmetry.rs` | Symmetry subgroups (add `(xy)` here) |
| `crates/studio_core/src/markov_junior/all_node.rs` | AllNode - check 3D matching |
| `crates/studio_core/src/markov_junior/interpreter.rs` | Origin placement |
| `crates/studio_core/src/markov_junior/render.rs` | Verification tests |
| `docs/markov_junior/VERIFICATION_STATUS.md` | Detailed failure analysis |

---

## Completed Phases

| Phase | Description | Status |
|-------|-------------|--------|
| 1.0 | Core Algorithm | COMPLETE (237 tests) |
| 2.0 | Lua Integration | COMPLETE (25 tests) |
| 3.0-3.3 | PNG Rendering | COMPLETE |
| 3.6 | 3D VoxelWorld | COMPLETE |
| 4.0 | Verification Infrastructure | COMPLETE |
| 4.1 | 2D Model Testing | COMPLETE (13/14) |
| 4.2 | Full Model Scan | COMPLETE (120/157 loaded) |

---

## What's NOT Working

1. **3D Growth models** - ParallelGrowth only 15 cells, likely symmetry issue
2. **`(xy)` symmetry** - Not implemented, blocks 5 models
3. **`<observe from="...">`** - Not implemented, blocks 16 models
4. **Tileset loading** - Missing resource files, blocks 12 models
5. **44 models partial** - May need more steps or have bugs

---

## Critical Reminders

1. **285 tests must pass** before any commit
2. **Screenshots show `_done` vs `_partial`** - partial means hit step limit
3. **3D bugs are HIGH PRIORITY** - ParallelGrowth should fill grid
4. **Depth tracking bug was fixed** - see commit b9b54cd for pattern
5. **Run `test_run_all_markov_models`** after any fix to verify no regressions
