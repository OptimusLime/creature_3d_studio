# MarkovJunior Verification Status

## Summary

- **Total Models:** 157
- **Loaded Successfully:** 120 (76%)
- **Completed (DONE):** 76
- **Partial (still running):** 44
- **Failed to Load:** 37

## Load Failures by Category

### Missing 'from' attribute in `<observe>` (16 models)
Not implemented yet - observe nodes need 'from' attribute support.
- BishopParity
- CompleteSAW
- CompleteSAWSmart
- CrossCountry
- DiagonalPath
- EuclideanPath
- Island
- KnightPatrol
- MultiSokoban8
- MultiSokoban9
- RegularPath
- SequentialSokoban
- SnellLaw
- SokobanLevel1
- SokobanLevel2
- StormySnellLaw

### Unknown symmetry '(xy)' (5 models)
Need to implement (xy) symmetry subgroup.
- Apartemazements
- CarmaTower
- Partitioning
- PillarsOfEternity
- StairsPath

### Missing tileset files (12 models)
These need tileset resources that weren't copied.
- ClosedSurface
- ColoredKnots
- Escher
- EscherSurface
- Knots2D
- Knots3D
- OrientedEscher
- PeriodicEscher
- SelectLongKnots
- SubmergedKnots
- Surface
- TileDungeon
- TilePath

### Invalid union symbol (2 models)
- ModernHouse (symbol '.')
- SeaVilla (symbol '?')

### Image loading error (1 model)
- DualRetraction3D (file extension issue)

## 3D Models Status

Only 5 3D models successfully loaded and ran:

| Model | Size | Steps | Cells | Status | Notes |
|-------|------|-------|-------|--------|-------|
| Counting | 8x8x8 | 1 | 512 | DONE | Trivial |
| Hills | 40x40x12 | 3 | 0 | DONE | **SUSPICIOUS** - 0 cells |
| OddScale3D | 8x8x8 | 1176 | 26289 | DONE | |
| OpenCave3D | 40x40x40 | 69 | 10830 | DONE | |
| ParallelGrowth | 29x29x29 | 14 | 15 | DONE | **SUSPICIOUS** - only 15 cells in 24k grid |

### Suspicious 3D Results

**Hills (0 cells):** Uses convolution with NoCorners neighborhood. May have issue with 3D convolution.

**ParallelGrowth (15 cells):** Uses `<all>` with origin. Should grow to fill entire grid but only produced 15 cells. Likely issue with:
- 3D symmetry in AllNode
- Or origin placement in 3D

## Models Still Running (Partial)

44 models hit the step limit (200k) without completing. These may need:
- More steps
- Or have infinite loops
- Or bugs in our implementation

Notable partials:
- Basic (should complete quickly)
- BiasedGrowth variants
- Circuit
- Trail
- Wilson

## Completed Models (76)

These ran to completion and should be compared visually with C# reference output.

## Next Steps

1. **Investigate 3D issues:**
   - ParallelGrowth only 15 cells (should be ~24k)
   - Hills 0 cells

2. **Implement missing features:**
   - `<observe from="...">` attribute
   - `(xy)` symmetry subgroup

3. **Copy tileset resources** for WFC tile models

4. **Visual verification** of completed models against C# reference images
