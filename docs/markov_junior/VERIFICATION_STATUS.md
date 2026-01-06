# MarkovJunior Verification Status

**Last Updated:** Phase 6.0 Census Complete

## Summary

| Category | Count | Percentage |
|----------|-------|------------|
| **VERIFIED (100%)** | 68 | 51.5% |
| **CLOSE (>95%)** | 12 | 9.1% |
| **PARTIAL (50-95%)** | 42 | 31.8% |
| **BROKEN (<50%)** | 5 | 3.8% |
| **LOADER ERRORS** | 5 | 3.8% |
| **Total** | 132 | 100% |

---

## VERIFIED (100%) - 68 Models

These models produce **identical output** to C# reference with seed 42.

| Model | Status |
|-------|--------|
| Backtracker | 100% |
| BacktrackerCycle | 100% |
| Basic | 100% |
| BasicBrickWall | 100% |
| BasicDungeonGrowth | 100% |
| BasicPartitioning | 100% |
| BlueNoise | 100% |
| Cave | 100% |
| CaveContour | 100% |
| CentralCrawlers | 100% |
| Counting | 100% |
| Coupling | 100% |
| Cycles | 100% |
| Digger | 100% |
| DualRetraction | 100% |
| Dwarves | 100% |
| Flowers | 100% |
| Forest | 100% |
| ForestFire | 100% |
| ForestFireCA | 100% |
| GameOfLife | 100% |
| Growth | 100% |
| GrowthCompetition | 100% |
| GrowthContraction | 100% |
| GrowthWalk | 100% |
| HamiltonianPaths | 100% |
| Hills | 100% |
| IrregularMazeGrowth | 100% |
| IrregularSAW | 100% |
| Laplace | 100% |
| LoopGrowth | 100% |
| LostCity | 100% |
| MarchingSquares | 100% |
| MazeBacktracker | 100% |
| MazeGrowth | 100% |
| MazeMap | 100% |
| MazeTrail | 100% |
| NestedGrowth | 100% |
| NoDeadEnds | 100% |
| Noise | 100% |
| NystromDungeon | 100% |
| OddScale | 100% |
| OddScale3D | 100% |
| OpenCave | 100% |
| OpenCave3D | 100% |
| OrganicMechanic | 100% |
| PaintCompetition | 100% |
| ParallelGrowth | 100% |
| ParallelMazeGrowth | 100% |
| Push | 100% |
| PutColoredLs | 100% |
| PutLs | 100% |
| RainbowGrowth | 100% |
| RegularSAW | 100% |
| RegularSAWRestart | 100% |
| River | 100% |
| SAWRestart | 100% |
| SelfAvoidingWalk | 100% |
| SmoothTrail | 100% |
| StochasticVoronoi | 100% |
| StrangeDungeon | 100% |
| StrangeGrowth | 100% |
| StrangeNoise | 100% |
| Tetris | 100% |
| Texture | 100% |
| Trail | 100% |
| Voronoi | 100% |
| WolfBasedApproach | 100% |

---

## CLOSE (>95%) - 12 Models - Priority Fixes

| Model | Match % | Cells Different | Likely Issue |
|-------|---------|-----------------|--------------|
| ConnectedCaves | 99.83% | 6 | Path RNG |
| BernoulliPercolation | 99.81% | 246 | Path RNG |
| DungeonGrowth | 99.55% | 28 | Path RNG |
| Percolation | 99.54% | 602 | Path RNG |
| SoftPath | 98.88% | 362 | Path RNG |
| CrawlersChase | 98.61% | 50 | Path RNG |
| DiagonalPath | 98.53% | 94 | Path RNG |
| CrossCountry | 97.59% | 154 | Path RNG |
| EuclideanPath | 97.45% | 163 | Path RNG |
| BishopParity | 96.94% | 110 | Unknown |
| BiasedGrowthContraction | 95.92% | 588 | Unknown |
| SelectLongKnots | 95.00% | 1350 | Unknown |

---

## PARTIAL (50-95%) - 42 Models

| Model | Match % | Cells Different |
|-------|---------|-----------------|
| BiasedMazeGrowth | 94.64% | 734 |
| BiasedGrowth | 94.47% | 796 |
| ColoredKnots | 94.39% | 2616 |
| BasicDijkstraFill | 94.39% | 202 |
| EscherSurface | 93.52% | 4150 |
| Knots3D | 92.81% | 4600 |
| Partitioning | 91.40% | 843 |
| GrowTo | 91.22% | 1264 |
| MultiHeadedWalkDungeon | 91.01% | 313 |
| StormySnellLaw | 90.57% | 1358 |
| SnellLaw | 90.34% | 618 |
| KnightPatrol | 89.69% | 371 |
| RegularPath | 88.77% | 701 |
| Knots2D | 88.56% | 412 |
| DenseSAW | 87.91% | 421 |
| TilePath | 86.70% | 1330 |
| SelectLargeCaves | 86.03% | 503 |
| MultiHeadedWalk | 85.59% | 1412 |
| Apartemazements | 84.73% | 9774 |
| Escher | 83.69% | 10439 |
| ClosedSurface | 82.55% | 8142 |
| PeriodicEscher | 80.69% | 12356 |
| BiasedVoronoi | 80.39% | 1255 |
| CentralSAW | 79.52% | 713 |
| Keys | 78.79% | 231 |
| OrientedEscher | 78.36% | 5843 |
| TileDungeon | 75.62% | 316 |
| Surface | 69.83% | 8145 |
| HamiltonianPath | 68.84% | 474 |
| MultiHeadedDungeon | 68.03% | 967 |
| ConstrainedCaves | 67.97% | 1153 |
| DijkstraDungeon | 65.75% | 548 |
| SmarterDigger | 64.44% | 569 |
| Circuit | 64.09% | 1250 |
| ChainDungeonMaze | 63.86% | 1301 |
| FireNoise | 63.40% | 32943 |
| ChainMaze | 62.92% | 1335 |
| SubmergedKnots | 60.74% | 43423 |
| FindLongCycle | 58.30% | 304 |
| CompleteSAW | 55.40% | 161 |
| CompleteSAWSmart | 54.44% | 241 |
| BasicDijkstraDungeon | 53.61% | 1670 |

---

## BROKEN (<50%) - 5 Models

| Model | Match % | Cells Different | Issue |
|-------|---------|-----------------|-------|
| SmartSAW | 45.43% | 197 | Unknown |
| ChainDungeon | 45.22% | 1972 | Unknown |
| Sewers | 32.38% | 1082 | Unknown |
| Island | 0.15% | 639061 | Massive divergence |
| PillarsOfEternity | 0.00% | N/A | Dimension mismatch |

---

## LOADER ERRORS - 5 Models

These models fail to load in Rust, not verification failures.

| Model | Error |
|-------|-------|
| CarmaTower | VOX file loading: extension not recognized |
| DualRetraction3D | VOX file loading: extension not recognized |
| ModernHouse | Union symbol conflict: '.' already defined |
| SeaVilla | Union symbol conflict: '?' already defined |
| StairsPath | Missing 'out' attribute in file-based rule |

---

## Analysis

### Root Causes Identified

1. **Path Node RNG Bug** (affects ~12 models >95%)
   - Rust uses `StdRng::seed_from_u64(ctx.random.next_u64())`
   - C# uses `new Random(ip.random.Next())` (int seed)
   - Fix: Use `DotNetRandom::from_seed(ctx.random.next_int())`

2. **Loader Bugs** (affects 5 models)
   - VOX file extension handling
   - Union symbol redefinition checking
   - File-based rule parsing

3. **Unknown Issues** (affects remaining models)
   - Need per-model investigation

### Priority Order

1. Fix Path Node RNG → should fix 12+ models
2. Fix loader bugs → enable 5 more models
3. Investigate remaining failures by accuracy (highest first)

---

## Commands

```bash
# Check current status
python3 scripts/verification_status.py status -v

# Show summary by bucket
python3 scripts/batch_verify.py --summary

# Verify specific model
python3 scripts/batch_verify.py ModelName

# Re-verify after fix
python3 scripts/batch_verify.py ModelName --regenerate
```
