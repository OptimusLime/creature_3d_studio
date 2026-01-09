# Apartemazements Bug Deep Dive

## Problem Statement

Apartemazements produces only ground tiles and a few cells instead of the expected 3D building with columns, windows, and structure.

**Expected (C# reference):** Full 3D building with roof maze, columns, windows, earth layers
**Actual (our output):** Ground layer (grass) + 3 cells (W, A, A) at top - no building

## Root Cause Identified

**The WFC tile color values don't map to grid character values correctly.**

### The Evidence

After WFC `update_state()`:
```
Grid values: {0: 509, 3: 2, 4: 1}
Grid characters: ['B', 'Y', 'D', 'A', 'W', 'P', 'R', 'F', 'U', 'E', 'N', 'C']
```

But the tiles only contain values:
```
Tile values used: {0, 1, 2, 3, 4}
```

### What This Means

The WFC newgrid has 12 characters: `BYDAWP RFUENC` (values 0-11).

The tiles loaded from VOX files contain raw ordinal indices 0-4, which happen to match:
- 0 = B (background)
- 1 = Y (yellow/earth marker)  
- 2 = D (down/column marker)
- 3 = A (air)
- 4 = W (wall/path)

But **the tiles should contain all 12 values** to produce the building structure. The values 5-11 (P, R, F, U, E, N, C) are never written by WFC.

### Why Children Fail

The WFC children expect to find specific values:
```xml
<prl in="B" out="C" comment="draw earth"/>     <!-- Needs B, writes C -->
<prl in="D B" out="* F"/>                       <!-- Needs D, writes F -->
<all comment="place windows">
  <rule in="FBBBF *AAA*" out="*RRR* *****"/>   <!-- Needs F, writes R -->
</all>
```

After WFC:
- Child 0 (`B→C`): Finds 509 B cells, converts to C ✓
- Child 1 (`C * * → B * *`): Finds C cells, converts some to B ✓
- Child 2 (`C C → E N`): Finds adjacent C cells, writes grass ✓
- **Child 6+ (columns, windows)**: Needs D, F, P values that DON'T EXIST → 0 matches

## The Bug Location

**File:** `crates/studio_core/src/markov_junior/wfc/tile_node.rs`
**Function:** `load_vox_tile()`

```rust
fn load_vox_tile(path: &Path, uniques: &mut Vec<i32>) -> Result<(Vec<u8>, usize), String> {
    // ...
    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                let v = voxels[src_idx];
                if v < 0 {
                    result[dst_idx] = 0;  // Empty → 0
                } else {
                    // BUG: Maps VOX palette index to sequential ordinal
                    // This does NOT correspond to grid character values!
                    let ord = if let Some(pos) = uniques.iter().position(|&u| u == v) {
                        pos
                    } else {
                        let pos = uniques.len();
                        uniques.push(v);
                        pos
                    };
                    result[dst_idx] = ord as u8;
                }
            }
        }
    }
}
```

The `uniques` array tracks unique VOX palette indices and assigns sequential ordinals (0, 1, 2, ...). But these ordinals have **no relationship** to the grid's character values.

## C# Reference

In C# `TileModel.cs`, tiles are loaded differently:

```csharp
// TileModel.cs line 77-95
for (int z = 0; z < SZ; z++)
    for (int y = 0; y < S; y++)
        for (int x = 0; x < S; x++)
        {
            int flatTileIndex = x + y * S + z * S * S;
            int vox = voxels[x + y * SX + z * SX * SY];
            
            // C# maps VOX palette to grid value via ords()
            // which creates a mapping based on unique colors
            tiledata[t][flatTileIndex] = (byte)ords[vox + 1];
        }
```

The key is `ords` - a mapping from VOX palette indices to grid ordinals that's built to be consistent with the grid's character ordering.

## How To Fix

### Option A: Build color-to-value mapping from tileset

1. When loading tiles, collect all unique VOX palette colors
2. Map these colors to grid values based on visual similarity or explicit mapping
3. The Paths tileset VOX files use specific palette colors that should map to BYDAWP RFUENC

### Option B: Use explicit color legend

Like PNG rule loading, require a `legend` that maps VOX palette to grid characters:
```xml
<wfc tileset="Paths" legend="BYDAWPRFUENC">
```

### Option C: Match C# ords() behavior

The C# `Helper.ords()` function creates a deterministic mapping. We need to replicate this exactly.

## Files to Study

| File | Purpose |
|------|---------|
| `MarkovJunior/source/TileModel.cs:77-95` | How C# loads tile voxels |
| `MarkovJunior/source/Helper.cs:ords()` | The color mapping function |
| `crates/studio_core/src/markov_junior/wfc/tile_node.rs:481-526` | Our broken tile loading |
| `crates/studio_core/src/markov_junior/helper.rs:load_vox()` | Our VOX loading |

## Test Requirements

A test MUST verify:

1. **Tile values match grid values**: After loading tiles, the values in `tiledata` should include all expected grid values (not just 0-4)

2. **Specific tile content**: Load the "Down" tile from Paths and verify it contains the expected D (down marker) value

3. **WFC output variety**: After WFC completes on a constrained grid, the output should contain more than 5 distinct values

## Reproduction

```rust
#[test]
fn test_tile_values_match_grid() {
    // Load Paths tileset with grid values "BYDAWP RFUENC"
    let grid = MjGrid::try_with_values(8, 8, 8, "BYDAWP RFUENC").unwrap();
    
    // Load TileNode
    let tile_node = TileNode::from_tileset(
        &tileset_path,
        "Paths",
        true, true, 10, 0, 0,
        grid.clone(),
        &grid,
        &[],
        false,
    ).unwrap();
    
    // Collect all values used in tiles
    let mut tile_values: HashSet<u8> = HashSet::new();
    for tile in &tile_node.tiledata {
        for &v in tile {
            tile_values.insert(v);
        }
    }
    
    // BUG: This currently fails!
    // Tiles only have {0,1,2,3,4} but should have values for
    // D (down markers), P (path), etc.
    assert!(tile_values.len() > 5, 
        "Tiles should use more than 5 values, got {:?}", tile_values);
    
    // Specific check: "Down" tile should contain D value (2)
    // and path-specific values
}
```

## Status

- [x] WFC children loading implemented
- [x] WFC children execution implemented  
- [x] **Tile color mapping - FIXED**
- [x] Test to catch tile color mapping issue

## Fix Applied

The fix was in `load_vox_tile()` at `tile_node.rs:497-529`. The bug was that empty voxels (value -1) were hardcoded to ordinal 0, but -1 was NOT added to the `uniques` list. This caused all non-empty VOX colors to get ordinals shifted by 1 from what C# produces.

The fix treats ALL voxel values (including -1) the same way: find or add to the `uniques` list. This matches the C# `Ords()` behavior where -1 is added to `uniques` when first encountered, becoming ordinal 0, and all other colors get subsequent ordinals.

After the fix, tiles have values {0, 1, 2, 3, 4, 5} instead of {0, 1, 2, 3, 4}, which correctly maps to grid values BYDAWP.
