# Escher Investigation

**Status:** RESOLVED
**Models:** Escher.xml, OrientedEscher.xml, PeriodicEscher.xml, EscherSurface.xml
**Seed:** 42

## Summary

All 4 Escher models now pass at 100% match. Two bugs were fixed:
1. WFC `tries` default was wrong (10 vs 1000) - fixed in earlier session
2. `x_rotate_tile` formula was wrong in propagator building - fixed in this session

## Bug #1: WFC tries default (FIXED - previous session)

### Symptom
```
[FAIL] Escher: Dim mismatch: [40, 40, 40] vs [8, 8, 8]
[FAIL] OrientedEscher: Dim mismatch: [30, 30, 30] vs [6, 6, 6]
```

### Root Cause
`loader.rs` defaulted `tries` to 10, but C# defaults to **1000** (WaveFunctionCollapse.cs line 34).

### Fix
```rust
// loader.rs lines 683 and 768
// C# defaults to 1000 tries (WaveFunctionCollapse.cs line 34)
let tries = attrs.get("tries").and_then(|s| s.parse().ok()).unwrap_or(1000);
```

## Bug #2: x_rotate_tile formula (FIXED - this session)

### Symptom
After Bug #1 fix, propagator constraint counts differed:
- C#: 687 constraints per direction
- Rust: 671 constraints per direction

Pattern indices in propagator differed, causing WFC to select different patterns.

### Root Cause
The `x_rotate_tile` function in `tile_node.rs` had the wrong formula:

**Rust (WRONG):**
```rust
let src = x + (sz - 1 - z) * s + y * s * s;
```

**C# (CORRECT):**
```csharp
byte[] xRotate(byte[] p) => newtile((x, y, z) => p[x + z * S + (S - 1 - y) * S * S]);
```

The C# formula means: `result[x,y,z] = input[x, z, S-1-y]`

### Fix
```rust
/// C# Reference (TileModel.cs line 64):
///   byte[] xRotate(byte[] p) => newtile((x, y, z) => p[x + z * S + (S - 1 - y) * S * S]);
fn x_rotate_tile(tile: &[u8], s: usize, sz: usize) -> Vec<u8> {
    let mut result = vec![0u8; s * s * sz];
    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                let src = x + z * s + (s - 1 - y) * s * s;
                let dst = x + y * s + z * s * s;
                result[dst] = tile[src];
            }
        }
    }
    result
}
```

**Location:** `crates/studio_core/src/markov_junior/wfc/tile_node.rs` lines 818-834

## Final Results

```
[OK] Escher         (was 81.94%)
[OK] OrientedEscher (was dim mismatch)
[OK] PeriodicEscher (was 75.60%)
[OK] EscherSurface  (was 90.10%)
```

Overall verification: **125/132 models (94.7%)**

## Debug Methodology

1. Added identical debug logging to both C# and Rust tile loading
2. Confirmed tile generation was identical (same fingerprints)
3. Added propagator constraint count logging
4. Found constraint counts differed (687 vs 671)
5. Added detailed constraint logging for specific tile (Stairs)
6. Found pattern indices differed during SquareSymmetries in propagator building
7. Traced to `x_rotate_tile` function having wrong formula
8. Fixed formula to match C# exactly

## Key Files Modified

| File | Changes |
|------|---------|
| `crates/studio_core/src/markov_junior/wfc/tile_node.rs` | Fixed `x_rotate_tile` formula |

## Commands

```bash
# Verify all Escher models
python3 scripts/batch_verify.py Escher OrientedEscher PeriodicEscher EscherSurface --regenerate

# Full batch verify
python3 scripts/batch_verify.py --all --regenerate
```
