# Sewers Investigation

**Status:** RESOLVED
**Model:** Sewers.xml (2D)
**Initial Match:** 49.75% (804 cells differ out of 1600)
**Final Match:** 100%
**Dimensions:** 40x40x1

## Root Cause

Two bugs were found in `overlap_node.rs`:

### Bug 1: Wrong loop bounds for pattern extraction

**Location:** `overlap_node.rs:90-99`

**Problem:** Loop bounds used sample dimensions (`smx`, `smy`) instead of input grid dimensions (`input_grid.mx`, `input_grid.my`).

**C# Reference:** `OverlapModel.cs:71-72`
```csharp
int ymax = periodicInput ? grid.MY : grid.MY - N + 1;
int xmax = periodicInput ? grid.MX : grid.MX - N + 1;
```

C# iterates over the INPUT GRID dimensions, not the SAMPLE dimensions. With `periodicInput=true` (default), the loop bounds are `grid.MX` and `grid.MY`. Pattern extraction still uses sample dimensions with modulo wrapping.

**Impact:** For Sewers (40x40 grid, 16x20 sample):
- C#: loops 40x40 = 1600 iterations
- Rust (buggy): loops 16x20 = 320 iterations

This caused different pattern weights because C# counts patterns at positions wrapping around the sample multiple times.

**Fix:**
```rust
// Before (wrong)
let ymax = if periodic_input { smy } else { smy.saturating_sub(n - 1) };
let xmax = if periodic_input { smx } else { smx.saturating_sub(n - 1) };

// After (correct)
let ymax = if periodic_input { input_grid.my } else { input_grid.my.saturating_sub(n - 1) };
let xmax = if periodic_input { input_grid.mx } else { input_grid.mx.saturating_sub(n - 1) };
```

### Bug 2: Incorrect pattern deduplication in symmetry generation

**Location:** `overlap_node.rs:376-410` (pattern_symmetries function)

**Problem:** The `pattern_symmetries` function used a HashSet to deduplicate symmetric patterns, but C# does NOT deduplicate.

**C# Reference:** `OverlapModel.cs:76`
```csharp
var symmetries = SymmetryHelper.SquareSymmetries(pattern, ..., (q1, q2) => false, symmetry);
```

The `same` function is `(q1, q2) => false` - it NEVER considers two patterns equal. This means ALL 8 symmetry transforms are always returned, even if some are duplicates.

**Impact:** For patterns with rotational symmetry (like `[0,0,0,1,1,1,1,1,1]`):
- C#: generates 8 variants (counts each 8 times)
- Rust (buggy): generates 4 unique variants (counts each 4 times)

This caused weights to be half of what they should be (e.g., 10 instead of 20).

**Fix:**
```rust
// Before (wrong) - used deduplication
let mut seen = std::collections::HashSet::new();
for i in 0..8 {
    if i < symmetry.len() && symmetry[i] {
        if !seen.contains(&things[i]) {
            seen.insert(things[i].clone());
            results.push(things[i].clone());
        }
    }
}

// After (correct) - no deduplication
for i in 0..8 {
    if i < symmetry.len() && symmetry[i] {
        results.push(things[i].clone());
    }
}
```

## Verification

After fixes:
- SewersSimple1: 100% (was 39.25%)
- Sewers: 100% (was 49.75%)

Full batch verification:
- 126 -> 128 verified models (2 additional models fixed)
- Sewers and SewersSimple1 now at 100%

## Hypothesis-Driven Debugging Process

### Phase 1: Isolate WFC Base Pattern

**Test:** Created SewersSimple1 with just WFC, no children.

**Initial Result:** 39.25% match - divergence starts at position (0,0,0)

**Analysis:**
1. Added debug output to compare dimensions and loop bounds
2. Found loop bounds used wrong dimensions (sample vs grid)
3. Fixed loop bounds, improved to 45.44%
4. Compared pattern weights - found C# had 2x the weights
5. Added debug for symmetry variant counts
6. Found C# generates 8 variants, Rust generated 4 for symmetric patterns
7. Removed deduplication, achieved 100% match

**Lesson:** C#'s `same` function parameter `(q1, q2) => false` is intentional - it prevents deduplication for weight counting purposes.

## Files Modified

- `crates/studio_core/src/markov_junior/wfc/overlap_node.rs`
  - Lines 90-103: Fixed loop bounds to use input grid dimensions
  - Lines 376-410: Removed pattern deduplication in symmetry generation

## Related Models

Other overlap WFC models may have been affected by these bugs. The full batch verification shows 2 additional models were fixed.
