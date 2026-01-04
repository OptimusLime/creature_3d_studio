# MarkovJunior Rust Port - Deviation Log

This document tracks ALL deviations from the original C# MarkovJunior implementation.
Each deviation must be reviewed and either:
1. Fixed to match C# exactly
2. Documented with justification for why it's acceptable

## Phase 1: Foundation Data Structures

### Grid.cs → mod.rs (MjGrid)

#### MISSING FIELDS

| C# Field | Rust Status | Impact | Priority |
|----------|-------------|--------|----------|
| `bool[] mask` | **IMPLEMENTED** | Used by AllNode for conflict tracking | DONE |
| `byte[] statebuffer` | **MISSING** | Double-buffer for State() method | LOW - State() is commented out in C# |
| `int transparent` | **MISSING** | Transparency mask for rendering | MEDIUM - needed for proper output |
| `string folder` | **MISSING** | Resource folder path | LOW - only for file loading |

#### MISSING METHODS

| C# Method | Rust Status | Impact |
|-----------|-------------|--------|
| `Grid.Load(XElement, ...)` | **MISSING** | XML loading - deferred to Phase 1.4 |
| `State()` | **MISSING** | Commented out in C# - skip |

#### TYPE DIFFERENCES

| Field | C# Type | Rust Type | Issue |
|-------|---------|-----------|-------|
| `waves` | `Dictionary<char, int>` | `HashMap<char, u32>` | C# uses `int` (32-bit signed), Rust uses `u32`. Should be fine but may cause issues with >31 colors |
| `MX/MY/MZ` | `int` | `usize` | Rust uses unsigned. C# allows negative which shouldn't happen but may cause subtle bugs |

#### BEHAVIORAL DIFFERENCES

1. **Grid.Wave() implementation differs slightly:**
   - C#: `sum += 1 << this.values[values[k]]` (uses addition)
   - Rust: `sum |= 1 << idx` (uses bitwise OR)
   - **Impact:** Functionally equivalent for non-overlapping bits, but C# would produce wrong results for duplicate chars while Rust handles correctly. This is actually a Rust IMPROVEMENT.

2. **Duplicate character handling in with_values():**
   - C#: Returns null with error message if duplicate found
   - Rust: **FIXED** - `try_with_values()` returns `Result<Self, GridError>`
   - **Impact:** Now matches C# behavior.

3. **Union types not implemented:**
   - C# Grid.Load() parses `<union>` elements to create composite wave types
   - Rust: Only `*` wildcard is added
   - **Impact:** Some models use custom unions. MUST ADD in Phase 1.4.

4. **matches() bounds checking:**
   - C#: No explicit bounds check - relies on caller
   - Rust: Added bounds checking that returns false
   - **Impact:** Rust is SAFER but may mask bugs that C# would crash on.

---

### Rule.cs → rule.rs (MjRule)

#### MISSING FIELDS

| C# Field | Rust Status | Impact | Priority |
|----------|-------------|--------|----------|
| `byte[] binput` | **IMPLEMENTED** | Compact input for fast comparison | DONE |
| `(int,int,int)[][] ishifts` | **IMPLEMENTED** | Precomputed positions per color | DONE |
| `(int,int,int)[][] oshifts` | **IMPLEMENTED** | Precomputed output positions | DONE |
| `bool original` | **MISSING** | Marks if rule is original vs symmetry variant | LOW - informational |

#### MISSING METHODS

| C# Method | Rust Status | Impact |
|-----------|-------------|--------|
| `Rule.Load(XElement, ...)` | **MISSING** | XML loading - deferred to Phase 1.4 |
| `Rule.LoadResource(...)` | **MISSING** | PNG/VOX loading - deferred to Phase 1.4 |
| `YRotated()` | **MISSING** | 3D rotation around Y axis | HIGH - needed for 3D models |
| `Symmetries(bool[], bool)` | **MISSING** | Wrapper method on Rule | LOW - can call symmetry module directly |

#### CONSTRUCTOR DIFFERENCES

1. **ishifts/oshifts:**
   - C# constructor builds precomputed lookup tables for which positions match each color
   - Rust: **IMPLEMENTED** - computed in `from_patterns()` and `parse()`
   - **Impact:** Now matches C# behavior.

2. **binput:**
   - C# computes `binput` which stores single-value inputs as bytes (0xff for wildcards)
   - Rust: **IMPLEMENTED** - computed in `from_patterns()` and `parse()`
   - **Impact:** Now matches C# behavior.

#### PATTERN PARSING DIFFERENCES

1. **Z-axis ordering:**
   - C#: `linesz = lines[MZ - 1 - z]` - Z layers are reversed
   - Rust: **FIXED** - Z layers now reversed to match C#
   - **Impact:** Now matches C# behavior.

2. **Helper.Split() not replicated:**
   - C# uses custom `Helper.Split(s, ' ', '/')` for nested splitting
   - Rust: Uses sequential split calls
   - **Impact:** Should be equivalent but needs verification with complex patterns.

---

### SymmetryHelper.cs → symmetry.rs

#### MISSING FUNCTIONALITY

| C# Feature | Rust Status | Impact | Priority |
|------------|-------------|--------|----------|
| `CubeSymmetries()` | **MISSING** | 48-element 3D symmetry group | HIGH - needed for 3D models |
| `cubeSubgroups` dictionary | **MISSING** | Predefined 3D subgroups | HIGH - needed for 3D |
| `GetSymmetry(bool d2, string, bool[])` | **MISSING** | Lookup by name with fallback | LOW - convenience |

#### IMPLEMENTATION DIFFERENCES

1. **Subgroup definitions may differ:**
   - Need to verify that `SquareSubgroup` masks match C# `squareSubgroups` exactly
   - C# `(x)(y)`: `[true, true, false, false, true, true, false, false]`
   - Rust `ReflectXY`: `[true, true, false, false, true, true, false, false]`
   - **Status:** MATCHES

2. **Generic vs concrete:**
   - C#: `SquareSymmetries<T>` is generic, takes function pointers
   - Rust: `square_symmetries` only works with `MjRule`
   - **Impact:** Less flexible but simpler. OK for now.

---

## Summary: Critical Issues Status

### Fixed in Phase 1 (Post-Audit)

1. **~~Add `mask: Vec<bool>` to MjGrid~~** - FIXED
2. **~~Add `ishifts`/`oshifts` to MjRule~~** - FIXED (also added `binput`)
3. **~~Fix Z-axis reversal in pattern parsing~~** - FIXED
4. **~~Add duplicate character detection~~** - FIXED (`try_with_values()` returns Result)

### Must Fix Before Phase 1.4 (XML Loading)

5. **Add union type support** - parse `<union>` elements into waves
6. **Implement `YRotated()`** - needed for 3D symmetries
7. **Implement `CubeSymmetries()`** - needed for 3D models

### Nice to Have (Can Defer)

8. Add `transparent` field for rendering
9. Add `folder` for resource paths
10. Add `original` flag for debugging

---

## Verification Needed

Before each phase is truly complete, run these checks:

### Cross-validation Test
```bash
# Generate C# reference output
cd MarkovJunior
dotnet run -- Basic 12345 --dump-state /tmp/basic_csharp.bin

# Run Rust with same seed
cargo test test_basic_matches_reference
```

### Pattern Parsing Test
Create test that parses "AB/CD EF/GH" and verifies exact byte positions match C# output.

---

## Change Log

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1 | Used u32 for waves instead of int | Rust idiom, functionally equivalent |
| 2026-01-04 | 1 | Used usize for dimensions | Rust idiom for array indexing |
| 2026-01-04 | 1 | Added bounds checking to matches() | Safety improvement |
| 2026-01-04 | 1 | Skipped CubeSymmetries | Deferred - 2D first |
| 2026-01-04 | 1 | **FIXED** Z reversal in pattern parsing | Was a bug, now matches C# |
| 2026-01-04 | 1 | **ADDED** mask field to MjGrid | Needed for AllNode |
| 2026-01-04 | 1 | **ADDED** ishifts/oshifts/binput to MjRule | Needed for incremental matching |
| 2026-01-04 | 1 | **ADDED** duplicate character detection | Matches C# error handling |
