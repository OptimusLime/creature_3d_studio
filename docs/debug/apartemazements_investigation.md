# Apartemazements Investigation

**Status:** RESOLVED
**Model:** Apartemazements.xml (3D)
**Match:** 100% (was 84.92%)
**Dimensions:** 40x40x40

## Model Structure

```xml
<sequence values="BWN" symmetry="(xy)">
  <prl in="B" out="W"/>
  <prl in="***/***/*** ***/*W*/*** ***/***/***" out="***/***/*** ***/*B*/*** ***/***/***"/>
  <prl in="B W" out="B N"/>
  <wfc values="BYDAWP RFUENC" tileset="Paths">
    <rule in="W" out="Empty"/>
    <rule in="N" out="Empty|Line|Up|Turn|X"/>

    <!-- Many child nodes inside WFC -->
    <prl in="B" out="C" comment="draw earth"/>
    <prl in="C * *" out="B * *" comment="remove extra earth"/>
    <prl in="C C" out="E N" comment="draw grass"/>
    <prl in="C" out="N"/>
    <prl in="Y" out="C" p="0.25" steps="1"/>
    <prl in="Y" out="B"/>
    <all comment="draw columns">...</all>
    <all comment="remove extra columns">...</all>
    <all comment="draw corner columns">...</all>
    <all comment="place windows">...</all>
    <prl comment="find h-uneven windows">...</prl>
    <all comment="mark h-uneven windows">...</all>
    <prl in="RFFR/BBBB" out="*RR*/****" comment="merge h-even windows"/>
    <!-- ... more children -->
  </wfc>
</sequence>
```

## Key Characteristics

1. **WFC with embedded children** - Apartemazements has `<prl>` and `<all>` nodes INSIDE the `<wfc>` tag
2. **3D Tile WFC** - Uses the "Paths" tileset with 3x3x3 tiles
3. **Rule mappings** - Maps initial grid values to tile possibilities:
   - `W` -> `Empty` only
   - `N` -> `Empty|Line|Up|Turn|X`

## Initial Diff Analysis

```
First 20 differences:
  (17,7,1): C#=0(B) Rust=9(E)
  (7,12,1): C#=0(B) Rust=9(E)
  ...
  
C# values in diffs: {0, 3, 4, 6, 11}  -> B, Y, D, W, C
Rust values in diffs: {0, 3, 4, 6, 9, 11} -> B, Y, D, W, E, C
First diff at index: 1897
```

Key observation: Rust produces `E` (value 9) in places where C# produces `B` (value 0) or `C` (value 11).

## Hypotheses

### Hypothesis 1: WFC children not executing correctly
WFC nodes can have children that execute AFTER WFC completes. The C# flow is:
1. WFC runs until completion
2. Sets `n = 0` to indicate completion
3. Next `Go()` call sees `n >= 0` and calls `base.Go()` to run children
4. Children operate on `newgrid` (which was swapped into `ip.grid`)

**Potential bugs:**
- Children not being called at all
- Children operating on wrong grid
- Children starting before WFC fully completes

### Hypothesis 2: Tile WFC propagation bug in 3D
The "Paths" tileset uses 3D tiles (3x3x3). Previous bugs found:
- y_rotate formula was wrong (fixed in tile_wfc_investigation.md)
- Propagator Z-direction constraints were swapped

Remaining possibilities:
- Neighbor constraints for vertical (top/bottom) direction
- fullSymmetry handling for 3D tiles

### Hypothesis 3: Rule mapping during WFC initialization
The `<rule in="W" out="Empty"/>` syntax maps initial colors to tile possibilities.
**Potential bugs:**
- Mapping applied incorrectly
- newgrid values not matching C#'s expectations

### Hypothesis 4: update_state() voting/assignment differs
When WFC completes, `update_state()` assigns colors to the output grid based on tile voting.
**Potential bugs:**
- Vote counting differs
- RNG for tie-breaking differs

## Debug Plan

### Step 1: Create simplified model
Strip Apartemazements down to minimal WFC-with-children:

```xml
<sequence values="BWN">
  <prl in="B" out="W"/>
  <prl in="B W" out="B N"/>
  <wfc values="BNA" tileset="Paths">
    <rule in="W" out="Empty"/>
    <rule in="N" out="Empty|Line"/>
    <!-- Add ONE child to test -->
    <prl in="A" out="B"/>
  </wfc>
</sequence>
```

### Step 2: Binary search complexity
1. Test WFC without children first
2. Add one child at a time
3. Find first child that causes divergence

### Step 3: Add targeted logging
- Log WFC completion state
- Log child execution count
- Log grid state before/after each child

### Step 4: Compare side-by-side
Run both C# and Rust with identical logging, compare outputs.

## Related Models

Other models using WFC with children (similar pattern):
- Partitioning (91.58% match) - simpler, good secondary test
- Knots2D (88.56%)
- SelectLongKnots (99.44%)
- EscherSurface (90.10%)
- ClosedSurface (84.88%)
- Surface (70.05%)

Models that work (WFC without complex children):
- TestKnotsL1, TestKnotsL2 (100%)
- ColoredKnots (100%)

## Commands

```bash
# Generate C# reference
cd MarkovJunior && dotnet run -- --model Apartemazements --seed 42 --dump-json

# Generate Rust output  
MJ_MODELS=Apartemazements MJ_SEED=42 cargo test -p studio_core batch_generate_outputs -- --ignored --nocapture

# Compare outputs
python3 scripts/compare_grids.py MarkovJunior/verification/Apartemazements_seed42.json verification/rust/Apartemazements_seed42.json

# Run simplified model
cd MarkovJunior && dotnet run -- --model ApartemazemeentsSimple --seed 42 --dump-json
```

## Progress Log

### Session Start
- Verified 117/146 models (80%)
- Apartemazements at 84.92% match
- Committed ConvChain fix (e788012)

### Bug Found and Fixed

**Root Cause:** Non-fullSymmetry tile propagator building was completely broken.

In C# TileModel.cs lines 212-254, for tilesets WITHOUT `fullSymmetry="True"`:

1. **left/right neighbors** (lines 214-236):
   - Parse rotation prefix with `tile()` function (e.g., "z Line" -> rotate Line by z)
   - Set 4 symmetry variants in direction 0 (+X)
   - zRotate tiles and set 4 more variants in direction 1 (+Y)
   - Uses yReflect/xReflect with swapped order for reflected versions

2. **top/bottom neighbors** (lines 239-253):
   - Parse rotation prefix with `tile()` function
   - Generate 4 square symmetries (zRotate + xReflect) for each tile
   - Set all pairs in direction 4 (+Z)

The Rust code was:
- NOT applying rotation prefixes
- NOT generating symmetry variants
- Using direction 1 for top/bottom (should be 4)
- Just doing simple tile name lookups

**Fix Location:** `crates/studio_core/src/markov_junior/wfc/tile_node.rs` in `build_tile_propagator()`

**Impact:**
- Apartemazements: 84.92% -> 100%
- Partitioning: 91.58% -> 98.93%
- Total verified: 117 -> 120 models

### Models Using Non-fullSymmetry Tilesets

The "Paths" tileset does NOT have `fullSymmetry="True"`, so it uses the non-fullSymmetry code path.
Other affected tilesets may include Partition, and any tileset without the fullSymmetry attribute.
