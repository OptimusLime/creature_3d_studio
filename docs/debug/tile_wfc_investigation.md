# 3D Tile WFC Propagator Investigation

## Problem Statement
3D tile WFC models (TestKnotsL1, SelectLongKnots, etc.) fail verification.
Starting verification: 108/140 models (77.1%).
Current status: 96.05% match on TestKnotsL1 (1066 cells differ out of 27000).

## Model Structure (TestKnotsL1)
```xml
<!-- Level 1: Just WFC with Knots3D tileset, no children -->
<wfc values="BW" tileset="Knots3D" tiles="Knots3D/3" size="10" d="3"/>
```

- Tileset: Knots3D (3x3x3 tiles: Empty, Line, Turn)
- fullSymmetry="True" in tileset XML
- Grid size: 10x10x10 wave cells -> 30x30x30 output

## Bugs Found and Fixed

### Bug 1: z_rotate_tile signature mismatch
**Location:** `tile_node.rs` line 856

**Problem:** `z_rotate_tile` took `tile: Vec<u8>` (ownership) while other rotation functions took `tile: &[u8]` (reference). This caused compilation errors when used with `tile_square_symmetries_with`.

**Fix:** Changed signature to `fn z_rotate_tile(tile: &[u8], s: usize, sz: usize) -> Vec<u8>`

### Bug 2: WFC RNG seeding consumed wrong amount
**Location:** `wfc_node.rs` line 253

**Problem:** 
- C# uses `ip.random.Next()` which returns an `int` (32-bit)
- Rust used `ctx_rng.next_u64()` which consumes two 32-bit values

**Evidence:** Different random sequences after first WFC seed lookup.

**Fix:** Changed to `ctx_rng.next_int()` to consume exactly one 32-bit value like C#.

### Bug 3: WFC local RNG used wrong algorithm
**Location:** `wfc_node.rs` line 235 and 257

**Problem:**
- C# uses `new Random(seed)` which uses .NET's specific seeding/generation algorithm
- Rust used `StdRandom::from_u64_seed(seed)` which uses a completely different algorithm

**Evidence:** Same seed but different random sequences.
```
C# noise[0]: 9.05e-7
Rust noise[0]: 7.48e-7  <- Wrong!
```

**Fix:** Changed to `DotNetRandom::from_seed(seed)` in both `good_seed()` (for trial runs) and `initialize()` (for actual execution).

### Bug 4: TileNode periodic default was wrong
**Location:** `loader.rs` line 758

**Problem:**
- C# defaults `periodic` to `false` for TileModel (TileModel.cs line 17)
- Rust defaulted to `true`

**Evidence:** 
```
C# NextUnobservedNode periodic=False, N=1
Rust NextUnobservedNode periodic=true  <- Wrong!
```

Periodic=true means cells wrap around edges, affecting which cells are considered valid and how propagation works.

**Fix:** Changed `.unwrap_or(true)` to `.unwrap_or(false)`

### Bug 5: TileNode N parameter was tile size instead of 1
**Location:** `tile_node.rs` line 192

**Problem:**
- C# TileModel inherits N=1 from WFCNode (line 11: `protected int P, N = 1;`)
- Rust passed `s` (tile size = 3) as the N parameter

The N parameter is used for boundary checks in NextUnobservedNode:
```
if (!periodic && (x + N > MX || y + N > MY || z + 1 > MZ)) continue;
```

With N=3 and MX=10, cells at x=8,9 are skipped (8+3=11 > 10).
With N=1 and MX=10, all cells 0-9 are valid (9+1=10 is NOT > 10).

**Evidence:**
```
C# NextUnobserved #0: 1000 rng calls  <- All 10x10x10 cells valid
Rust NextUnobserved #0: 640 rng calls <- Only 8x8x10 cells valid (wrong!)
```

**Fix:** Changed `WfcNode::new(..., s, ...)` to `WfcNode::new(..., 1, ...)` for TileNode.

### Bug 6: Step execution used different RNG than GoodSeed
**Location:** `wfc_node.rs` line 235

**Problem:**
After `good_seed()` finds a valid seed using `DotNetRandom`, the actual step-by-step execution was initialized with `StdRandom::from_u64_seed(seed)` instead of `DotNetRandom::from_seed(seed)`.

This caused the step execution to produce different patterns than the trial run that validated the seed.

**Evidence:** Wave state after WFC completion differed:
```
Rust wave cell 0: pattern 13
C# wave cell 0: pattern 0  <- Should match!
```

**Fix:** Changed `self.rng = Some(StdRandom::from_u64_seed(seed))` to `self.rng = Some(Box::new(DotNetRandom::from_seed(seed as i32)))`.

Also required changing the `rng` field type from `Option<StdRandom>` to `Option<Box<dyn MjRng>>`.

## Current Status

After all fixes:
- Propagator: 146 constraints for all 6 directions (matches C#)
- Propagator pairs: Identical between C# and Rust
- WFC observations: First 882 observations match exactly
- Wave state: First 20 cells match exactly
- **Output: 96.05% match (1066 cells differ)**

## Remaining Issue

The wave states match, but the output grids differ by ~4%.
- No ties detected in UpdateState (wave is fully collapsed)
- tiledata arrays match between C# and Rust
- Parameters (s=3, sz=3, overlap=0, overlapz=0) match

Differences occur at coordinates like (4,1,0), (7,1,0), (10,1,0)... which are all at `3n+1` positions (middle of tiles).

## Debug Output Still Active

**WARNING:** The following debug output is still enabled and should be removed before final commit:

### wfc_node.rs
- Lines 267-284: Observation logging during GoodSeed
- Lines 285-293: Contradiction/success logging
- Lines 359-376: SKIPPING cell debug
- Lines 437-447: Pattern selection logging
- Lines 461-477: weighted_random logging

### tile_node.rs  
- Lines 229-235: tiledata debug output
- Lines 275-300: TIE detection in UpdateState

### C# changes (MarkovJunior/source/)
- TileModel.cs: Debug output in UpdateState, propagator pair dump
- WaveFunctionCollapse.cs: NextUnobservedNode debug, Observe debug
- Helper.cs: weighted_random debug

## Key Commands

```bash
# Build
cargo build -p studio_core

# Run single test model with Rust
rm -f verification/rust/TestKnotsL1_seed42.json
MJ_MODELS=TestKnotsL1 MJ_SEED=42 cargo test -p studio_core verification::tests::batch_generate_outputs -- --ignored --nocapture

# Run single test model with C#
cd MarkovJunior && dotnet run -- --model TestKnotsL1 --seed 42 --dump-json

# Verify single model
python3 scripts/batch_verify.py TestKnotsL1 --regenerate

# Compare outputs
python3 scripts/compare_grids.py MarkovJunior/verification/TestKnotsL1_seed42.json verification/rust/TestKnotsL1_seed42.json
```

## Next Steps

1. Debug the UpdateState output coordinate calculation
2. Compare actual output values at specific coordinates
3. Check if there's a difference in how tile subcells are indexed (dx, dy, dz order)
4. Clean up debug output after finding root cause
