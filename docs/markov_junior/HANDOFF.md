# MarkovJunior Rust Port - Handoff Document

## Current State

**Branch:** `feature/markov-junior-rust`
**Phase:** Phase 4.6 - Tileset Loading Fixed
**Tests:** 288 passing
**Models:** 152/157 loaded (97%)

---

## Session Summary (Phase 4.3-4.6)

### Bugs Fixed This Session

#### 1. 3D Cube Symmetry (CRITICAL - Phase 4.3)

**Problem:** `apply_symmetry()` in `loader.rs` was returning only the original rule for 3D models:
```rust
// BEFORE (broken)
fn apply_symmetry(rule: MjRule, symmetry: &[bool], is_2d: bool) -> Vec<MjRule> {
    if is_2d {
        square_symmetries(&rule, Some(subgroup))
    } else {
        vec![rule]  // BUG: Only 1 variant!
    }
}
```

**Fix:** Wired up `cube_symmetries()` to generate up to 48 rule variants for 3D:
```rust
// AFTER (fixed)
fn apply_symmetry(rule: MjRule, symmetry: &[bool], is_2d: bool) -> Vec<MjRule> {
    if is_2d {
        square_symmetries(&rule, Some(subgroup))
    } else {
        cube_symmetries(&rule, Some(&mask))  // Now generates all 3D variants!
    }
}
```

**Impact:**
- **ParallelGrowth**: Before: 15 cells. After: 24,389 cells (100% fill!)
- All 3D growth models now work correctly

#### 2. 3D Symmetry Subgroups (Phase 4.4)

**Problem:** `get_symmetry()` only supported `()` and `(xyz)` for 3D models.

**Fix:** Added all cube symmetry subgroups matching C# reference:
- `()` - Identity only (1 variant)
- `(x)` - Identity + x-reflection (2 variants)
- `(z)` - Identity + z-reflection (2 variants)
- `(xy)` - All 8 XY-plane symmetries (used by 5 models)
- `(xyz+)` - All 24 rotations (no reflections)
- `(xyz)` - All 48 symmetries

#### 3. Observe `from` Attribute Default (Phase 4.5)

**Problem:** `<observe>` element required the `from` attribute, but C# defaults it.

**Fix:** Made `from` optional with default to `value`:
```rust
let from_char = attrs
    .get("from")
    .and_then(|s| s.chars().next())
    .unwrap_or(value_char);  // Default to value if not specified
```

**Impact:** 16 models now load (BishopParity, CompleteSAW, Island, etc.)

#### 4. Tileset Path Loading (Phase 4.6)

**Problem:** Tileset loading looked for `{tileset_dir}/data.xml` but C# expects `{tilesets}/{name}.xml`.

**Fix:** Updated `LoadContext` and `load_tile_node()`:
```rust
// BEFORE (broken)
fn tileset_path(&self, name: &str) -> Option<PathBuf> {
    resources.join("tilesets").join(name)  // Returns directory, not file
}
let tileset_xml_path = tileset_path.join("data.xml");  // Wrong!

// AFTER (fixed)
fn tileset_xml_path(&self, name: &str) -> Option<PathBuf> {
    resources.join("tilesets").join(format!("{}.xml", name))
}
```

**Impact:** 15+ tileset models now load (Escher, Knots2D/3D, Surface, etc.)

#### 5. WFC Bounds Checks (Phase 4.6)

**Problem:** Index out of bounds errors in tile rendering and wave propagation.

**Fixes:**
- Added `sz_coord < output_mz` check in `tile_node.rs:230`
- Added bounds check in `wave.rs:set_compatible()`

---

## Current Verification Results

### Summary
| Category | Count | Percentage |
|----------|-------|------------|
| Total Models | 157 | 100% |
| Loaded Successfully | 152 | 97% |
| Failed to Load | 5 | 3% |

### Remaining Failures (5 models)

| Model | Error | Category |
|-------|-------|----------|
| CarmaTower | image extension issue | Resource |
| DualRetraction3D | image extension issue | Resource |
| ModernHouse | invalid union symbol `.` | Parser |
| SeaVilla | invalid union symbol `?` | Parser |
| StairsPath | missing `out` attribute | Parser |

---

## Key Commands

```bash
# Run ALL 157 models, save screenshots
cargo test -p studio_core test_run_all_markov_models -- --nocapture

# Run ParallelGrowth 3D symmetry verification test
cargo test -p studio_core test_parallel_growth_3d_symmetry_fix -- --nocapture

# Run all MJ tests (should be 288)
cargo test -p studio_core markov_junior
```

---

## Key Files Modified This Session

| File | Changes |
|------|---------|
| `crates/studio_core/src/markov_junior/loader.rs` | `apply_symmetry()` for 3D; `get_symmetry()` subgroups; `parse_observe_element()` from default; `tileset_xml_path()` |
| `crates/studio_core/src/markov_junior/wfc/tile_node.rs` | Added `output_mz` bounds check |
| `crates/studio_core/src/markov_junior/wfc/wave.rs` | Added bounds check in `set_compatible()` |
| `crates/studio_core/src/markov_junior/render.rs` | Added `test_parallel_growth_3d_symmetry_fix` |

---

## Completed Phases

| Phase | Description | Status |
|-------|-------------|--------|
| 1.0 | Core Algorithm | COMPLETE |
| 2.0 | Lua Integration | COMPLETE |
| 3.0-3.3 | PNG Rendering | COMPLETE |
| 3.6 | 3D VoxelWorld | COMPLETE |
| 4.0 | Verification Infrastructure | COMPLETE |
| 4.1 | 2D Model Testing | COMPLETE |
| 4.2 | Full Model Scan | COMPLETE |
| 4.3 | 3D Symmetry Fix | COMPLETE |
| 4.4 | 3D Symmetry Subgroups | COMPLETE |
| 4.5 | Observe `from` Default | COMPLETE |
| 4.6 | Tileset Loading | COMPLETE |

---

## What's Still Not Working (5 models)

1. **Image loading** - CarmaTower, DualRetraction3D need special file extension handling
2. **Union symbols** - ModernHouse (`.`), SeaVilla (`?`) use reserved characters
3. **Rule parsing** - StairsPath has a malformed `<rule>` element

These are edge cases in resource loading and XML parsing, not algorithm bugs.

---

## Critical Reminders

1. **288 tests must pass** before any commit
2. **152/157 models now load** - up from 120!
3. **ParallelGrowth is the canary** - if it produces <24k cells, 3D symmetry is broken
4. **Run `test_run_all_markov_models`** after any loader changes
