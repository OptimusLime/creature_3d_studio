# Spherical MarkovJunior Implementation Audit

## Document Purpose

This document provides a comprehensive audit of all components that need spherical
counterparts for MarkovJunior to support spherical/polar coordinate grids. It serves
as the master checklist for the implementation effort.

---

## 1. Decision Context

### 1.1 Decision: Option B (Separate Spherical Implementation)

**Date**: January 2026

**Decision**: Implement a parallel spherical node system rather than making the
existing Node trait generic.

**Rationale**:
1. **Lower risk**: Existing Cartesian code remains untouched
2. **Incremental delivery**: Can ship core functionality first, advanced nodes later
3. **Conceptual clarity**: Spherical operations are fundamentally different for some nodes
4. **Similar effort**: Both options estimated at ~30 hours total

**Trade-offs accepted**:
- Some code duplication (especially in branch nodes)
- Two codepaths to maintain long-term
- XML models will need grid type indicator

### 1.2 What Already Exists

| Component | Status | Tests |
|-----------|--------|-------|
| `SphericalMjGrid` | Complete | 50 tests passing |
| `SphericalPattern` | Complete | Included in tests |
| `SphericalRule` | Complete | Included in tests |
| `SphericalSymmetry` (Klein 4-group) | Complete | Included in tests |
| `SphericalNeighbors` | Complete | Included in tests |
| Basic `run_step()` execution | Basic version exists | Integration tests |
| Rendering (`render_to_image`) | Complete | Level 6 tests |
| `RecordableGrid` trait impl | Complete | Level 6 tests |
| Video export | Complete | Level 7 tests |

### 1.3 Core Topology Differences

| Aspect | Cartesian | Spherical |
|--------|-----------|-----------|
| **Coordinates** | (x, y, z) | (r, θ, φ) |
| **Dimensions** | mx × my × mz | r_depth × θ_divisions × φ_divisions |
| **Index formula** | `x + y*mx + z*mx*my` | `θ + φ*θ_div + r*θ_div*φ_div` |
| **Neighbors (2D)** | 4: ±x, ±y | 4: ±θ, ±r |
| **Neighbors (3D)** | 6: ±x, ±y, ±z | 6: ±θ, ±φ, ±r |
| **Wrapping** | Optional per axis | θ always wraps, φ optional, r never |
| **Boundaries** | All edges optional | r_min (inner), r_max (outer) |
| **Cell shape** | Uniform cubes | Wedges varying by radius |
| **Pattern matching** | Rectangular blocks | Neighbor-based (star patterns) |

### 1.4 Known Tricky Areas

1. **WFC Propagation**: Cartesian uses 4/6 fixed directions (DX/DY/DZ). Spherical
   has variable neighbors at different radii.

2. **Path Finding**: Cartesian uses cardinal directions. Spherical paths curve
   around the angular dimension.

3. **Convolution Kernels**: 3×3 Cartesian kernels don't map directly to spherical
   where cells have different numbers of neighbors.

4. **Map Node Scaling**: Cartesian scaling (2× = double each dimension) has no
   clear spherical analog.

5. **Pattern Representation**: Cartesian patterns are rectangular grids. Spherical
   patterns are neighbor-relationship-based.

---

## 2. Node Type Audit

### 2.1 Audit Status Legend

| Symbol | Meaning |
|--------|---------|
| ⬜ | Not started |
| 🔍 | Under investigation |
| ✅ | Audit complete |
| ❌ | Determined not needed |

### 2.2 Master Audit Table

| Node Type | Cartesian File | Audit | Difficulty | Priority | Spherical Translation |
|-----------|----------------|-------|------------|----------|----------------------|
| **SequenceNode** | `node.rs` | ⬜ | Easy | P0 | Direct port (no grid access) |
| **MarkovNode** | `node.rs` | ⬜ | Easy | P0 | Direct port (no grid access) |
| **OneNode** | `one_node.rs` | ⬜ | Medium | P0 | Use SphericalRule matching |
| **AllNode** | `all_node.rs` | ⬜ | Medium | P0 | Use SphericalRule + mask |
| **ParallelNode** | `parallel_node.rs` | ⬜ | Medium | P1 | Parallel SphericalRule application |
| **PathNode** | `path_node.rs` | ⬜ | Hard | P2 | Different BFS for angular space |
| **ConvolutionNode** | `convolution_node.rs` | ⬜ | Hard | P2 | Neighbor-based kernels |
| **ConvChainNode** | `convchain_node.rs` | ⬜ | Hard | P3 | 2D-only, angular patterns |
| **MapNode** | `map_node.rs` | ⬜ | Very Hard | P3 | Unclear spherical analog |
| **WfcNode** | `wfc/wfc_node.rs` | ⬜ | Very Hard | P3 | Variable neighbor propagation |
| **OverlapNode** | `wfc/overlap_node.rs` | ⬜ | Very Hard | P3 | Spherical pattern overlap |
| **TileNode** | `wfc/tile_node.rs` | ⬜ | Very Hard | P3 | Spherical tile adjacency |

### 2.3 Supporting Systems Audit

| System | Cartesian File | Audit | Difficulty | Priority | Notes |
|--------|----------------|-------|------------|----------|-------|
| **Interpreter** | `interpreter.rs` | ⬜ | Medium | P0 | SphericalInterpreter |
| **ExecutionContext** | `node.rs` | ⬜ | Easy | P0 | SphericalExecutionContext |
| **RuleNodeData** | `rule_node.rs` | ⬜ | Medium | P1 | Spherical match scanning |
| **Field** | `field.rs` | ⬜ | Medium | P2 | Spherical BFS potentials |
| **Observation** | `observation.rs` | ⬜ | Medium | P2 | Goal checking |
| **XML Loader** | `loader.rs` | ⬜ | Medium | P1 | Spherical model loading |
| **Wave** | `wfc/wave.rs` | ⬜ | Hard | P3 | Variable neighbor counts |

---

## 3. Detailed Node Audits

### 3.1 SequenceNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`node.rs` lines 130-200):
```rust
pub struct SequenceNode {
    pub nodes: Vec<Box<dyn Node>>,
    n: usize,  // Current child index
    active_branch_child: Option<usize>,
}

impl Node for SequenceNode {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // No grid access - just delegates to children
    }
}
```

**Grid Accesses**: None

**Cartesian Tests**:
- `node.rs::test_sequence_node_runs_in_order`
- `node_tests.rs::test_sequence_node_runs_in_order`

**XML Models Using This**:
- 178 models use `sequence` as root
- Nearly all multi-step models use it

**Spherical Translation**:
- Direct port: `SphericalSequenceNode`
- Change `Box<dyn Node>` to `Box<dyn SphericalNode>`
- No algorithm changes needed

**Estimated Effort**: 30 minutes

---

### 3.2 MarkovNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`node.rs` lines 200-280):
```rust
pub struct MarkovNode {
    pub nodes: Vec<Box<dyn Node>>,
    n: usize,
    active_branch_child: Option<usize>,
}

impl Node for MarkovNode {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // No grid access - loops children until all fail
    }
}
```

**Grid Accesses**: None

**Cartesian Tests**:
- `node.rs::test_markov_node_restarts_from_zero`
- `node_tests.rs::test_markov_node_loops_until_done`

**XML Models Using This**:
- 87 models use `markov`
- Essential for iterative generation

**Spherical Translation**:
- Direct port: `SphericalMarkovNode`
- Change `Box<dyn Node>` to `Box<dyn SphericalNode>`
- No algorithm changes needed

**Estimated Effort**: 30 minutes

---

### 3.3 OneNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`one_node.rs` ~250 lines):
```rust
pub struct OneNode {
    pub data: RuleNodeData,  // Shared rule matching data
}

impl Node for OneNode {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // 1. Scan for matches (or use incremental)
        // 2. Pick random match
        // 3. Apply rule
    }
}
```

**Grid Accesses** (12 total):
- `ctx.grid.mx`, `ctx.grid.my` - dimensions
- `ctx.grid.state[i]` - read/write values
- `ctx.grid.index_to_coord(idx)` - convert flat index
- `ctx.grid.matches(rule, x, y, z)` - pattern matching
- Index formula: `sx + sy * mx + sz * mx * my`

**Cartesian Tests**:
- `one_node.rs::test_one_node_applies_single_match`
- `one_node.rs::test_one_node_exhausts_matches`
- `one_node.rs::test_one_node_2x1_rule`
- `one_node.rs::test_one_node_heuristic_selection`
- `one_node.rs::test_one_node_heuristic_with_temperature`

**XML Models Using This** (199 models):
- Simple: `Basic.xml`, `Growth.xml`, `Flowers.xml`
- Medium: `River.xml`, `DungeonGrowthSimple.xml`
- Complex: `Dwarves.xml`, `SokobanLevel1.xml`

**Spherical Translation**:
```rust
pub struct SphericalOneNode {
    rules: Vec<SphericalRule>,
    matches: Vec<(usize, usize)>,  // (rule_idx, flat_idx)
    match_count: usize,
    // ... similar to RuleNodeData
}

impl SphericalNode for SphericalOneNode {
    fn go(&mut self, ctx: &mut SphericalExecutionContext) -> bool {
        // 1. Scan all indices for SphericalRule matches
        // 2. Pick random match
        // 3. Apply: grid.set_state(idx, rule.output)
    }
}
```

Key changes:
- Use `SphericalRule::matches(grid, idx)` instead of Cartesian pattern matching
- Use `grid.len()` for iteration instead of triple nested loop
- Use `grid.index_to_coord(idx)` returning `(r, θ, φ)`

**Estimated Effort**: 2 hours

---

### 3.4 AllNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`all_node.rs` ~250 lines):
```rust
pub struct AllNode {
    pub data: RuleNodeData,
}

impl Node for AllNode {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // 1. Scan for ALL matches
        // 2. Filter to non-overlapping set
        // 3. Apply all simultaneously
    }
}
```

**Grid Accesses** (12 total):
- Same as OneNode plus:
- `ctx.grid.mask[i]` - overlap tracking

**Cartesian Tests**:
- `all_node.rs::test_all_node_fills_entire_grid`
- `all_node.rs::test_all_node_non_overlapping`
- `all_node.rs::test_all_node_returns_false_when_no_matches`
- `all_node.rs::test_all_node_2d_non_overlapping`
- `all_node.rs::test_all_node_heuristic_sorting`

**XML Models Using This** (175 models):
- Simple: `Basic.xml` (uses `all` for some rules)
- Medium: `Flowers.xml`, `River.xml`
- Complex: `MultiHeadedDungeon.xml`

**Spherical Translation**:
- Similar to SphericalOneNode
- Use `grid.mask[idx]` for overlap tracking (already in trait)
- SphericalRule only affects single cell, so overlap = same cell

**Estimated Effort**: 2 hours

---

### 3.5 ParallelNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`parallel_node.rs` ~220 lines):
```rust
pub struct ParallelNode {
    data: RuleNodeData,
    newstate: Vec<u8>,  // Buffer for simultaneous writes
}

impl Node for ParallelNode {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // 1. Scan grid for matches
        // 2. Compute new values into buffer
        // 3. Copy buffer back to grid
    }
}
```

**Grid Accesses** (12 total):
- Similar to OneNode
- Uses temporary `newstate` buffer for atomic updates

**Cartesian Tests**:
- `parallel_node.rs::test_parallel_node_applies_all`
- `parallel_node.rs::test_parallel_node_reads_original_state`
- `parallel_node.rs::test_parallel_node_returns_false_when_no_matches`

**XML Models Using This** (103 models):
- Simple: `GameOfLife.xml`
- Medium: `Cave.xml`, `OpenCave.xml`
- Complex: `Island.xml`, `Hills.xml`

**Spherical Translation**:
- Similar to SphericalOneNode
- Add `newstate: Vec<u8>` buffer
- All matches computed on original state, applied atomically

**Estimated Effort**: 2 hours

---

### 3.6 PathNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`path_node.rs` ~500 lines):
```rust
pub struct PathNode {
    from: u32,      // Wave mask for start cells
    to: u32,        // Wave mask for end cells
    on: u32,        // Wave mask for traversable cells
    colored: bool,  // Whether to color path
    value: u8,      // Color value for path
    inertia: bool,  // Prefer straight lines
}

impl Node for PathNode {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // 1. Find start cells
        // 2. BFS to find shortest path to end
        // 3. Trace back and color path
    }
}
```

**Grid Accesses** (19+ total):
- `ctx.grid.mx`, `my`, `mz` - dimensions
- `ctx.grid.state[i]` - read values
- Index formula for neighbors
- **Cardinal directions**: N/S/E/W/Up/Down (hardcoded)

**Critical Cartesian Code** (lines 236-350):
```rust
fn directions() -> [(i32, i32, i32); 6] {
    [(1,0,0), (-1,0,0), (0,1,0), (0,-1,0), (0,0,1), (0,0,-1)]
}
```

**Cartesian Tests**:
- `path_node.rs::test_path_node_simple`
- `path_node.rs::test_path_node_no_path`
- `path_node.rs::test_path_node_with_inertia`
- `path_node.rs::test_directions_2d`
- `path_node.rs::test_directions_3d`

**XML Models Using This** (24 models):
- Simple: `BasicDijkstraFill.xml`, `Percolation.xml`
- Medium: `DijkstraDungeon.xml`, `Lightning.xml`
- Complex: `SeaVilla.xml`, `ModernHouse.xml`

**Spherical Translation Challenges**:

1. **Direction concept**: Instead of N/S/E/W, use θ+/θ-/r+/r-/φ+/φ-
2. **Path shape**: Paths curve in angular dimension
3. **Inertia**: "Straight line" = constant θ (radial) or constant r (circular)
4. **Boundary handling**: Paths can wrap in θ but not in r

```rust
pub struct SphericalPathNode {
    from: u32,
    to: u32,
    on: u32,
    colored: bool,
    value: u8,
    inertia: bool,
    prefer_radial: bool,  // NEW: prefer radial vs angular paths
}

impl SphericalNode for SphericalPathNode {
    fn go(&mut self, ctx: &mut SphericalExecutionContext) -> bool {
        // BFS using grid.neighbors(idx) instead of hardcoded directions
    }
}
```

**Estimated Effort**: 4 hours

---

### 3.7 ConvolutionNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`convolution_node.rs` ~400 lines):
```rust
pub struct ConvolutionNode {
    rules: Vec<ConvolutionRule>,
    steps: Option<usize>,
    periodic: bool,
    sumfield: Vec<i32>,  // Cached neighbor sum
}

pub struct ConvolutionRule {
    input: u8,        // Cell value to match
    output: u8,       // Cell value to write
    sums: Vec<bool>,  // Allowed neighbor sums [0..9]
    values: u32,      // Wave mask for neighbor values
}

impl Node for ConvolutionNode {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // 1. Compute neighbor sums for each cell
        // 2. Apply rules based on cell value + sum
    }
}
```

**Grid Accesses** (14 total):
- `ctx.grid.mx`, `my`, `mz`
- `ctx.grid.state` read/write
- **3×3 kernel offsets**: dx, dy in range [-1, 1]

**Critical Cartesian Code** (kernel iteration):
```rust
for dy in -1..=1 {
    for dx in -1..=1 {
        if dx == 0 && dy == 0 { continue; }
        // Get neighbor at (x+dx, y+dy)
    }
}
```

**Cartesian Tests** (15 tests):
- `test_convolution_simple_rule`
- `test_convolution_sum_constraint`
- `test_convolution_moore_neighbor_count`
- `test_convolution_von_neumann_kernel`
- `test_convolution_game_of_life_rules`
- `test_convolution_3d_von_neumann`
- etc.

**XML Models Using This** (31 models):
- Simple: `Cave.xml`, `GameOfLife.xml`
- Medium: `Hills.xml`, `Island.xml`
- Complex: `CarmaTower.xml`

**Spherical Translation Challenges**:

1. **No fixed kernel**: Cells have different neighbor counts at different radii
2. **Neighbor access**: Use `grid.neighbors(idx)` returning variable count
3. **Sum calculation**: Sum over actual neighbors, not fixed 8/26

```rust
pub struct SphericalConvolutionNode {
    rules: Vec<SphericalConvolutionRule>,
    steps: Option<usize>,
}

pub struct SphericalConvolutionRule {
    input: u8,
    output: u8,
    min_count: u8,  // Min matching neighbors (instead of exact sums)
    max_count: u8,  // Max matching neighbors
    values: u32,    // Wave mask for neighbor values
}

impl SphericalNode for SphericalConvolutionNode {
    fn go(&mut self, ctx: &mut SphericalExecutionContext) -> bool {
        for idx in 0..ctx.grid.len() {
            let neighbors = ctx.grid.neighbors(idx);
            let count = neighbors.filter(|n| matches_value(n)).count();
            if count >= rule.min_count && count <= rule.max_count {
                // Apply rule
            }
        }
    }
}
```

**Estimated Effort**: 4 hours

---

### 3.8 ConvChainNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`convchain_node.rs` ~350 lines):

MCMC texture synthesis using learned N×N pattern weights.

**Grid Accesses** (11 total):
- `ctx.grid.mx`, `my` (2D only)
- `ctx.grid.state` read/write
- Pattern indexing with modular coordinates

**Cartesian Tests** (16 tests):
- `test_pattern_to_index`
- `test_learn_pattern_weights`
- `test_convchain_mcmc_step`
- etc.

**XML Models Using This** (4 models):
- `ChainMazeSimple.xml`, `ChainMaze.xml`
- `ChainDungeon.xml`, `ChainDungeonMaze.xml`

**Spherical Translation Challenges**:

1. **2D only**: ConvChain is inherently 2D (polar, not spherical 3D)
2. **Pattern indexing**: N×N patterns don't fit angular geometry
3. **Periodic boundaries**: θ wraps, r doesn't

**Estimated Effort**: 4 hours (if needed; low priority)

---

### 3.9 MapNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`map_node.rs` ~300 lines):

Scales grid and runs child nodes on scaled version.

**Grid Accesses** (11 total):
- Creates new grid with scaled dimensions
- Pattern matching at scale boundaries

**Cartesian Tests** (5 tests):
- `test_scale_factor_parse_integer`
- `test_map_node_simple_2x_scale`
- etc.

**XML Models Using This** (8 models):
- `MarchingSquares.xml`, `MazeMap.xml`, `OddScale.xml`
- `SeaVilla.xml`, `ModernHouse.xml`

**Spherical Translation Challenges**:

1. **What does "2× scale" mean for spherical?**
   - Double r_depth? (more shells)
   - Double θ_divisions? (finer angular resolution)
   - Both?
2. **Child execution**: Children would need SphericalNode interface

**Estimated Effort**: 4+ hours (conceptually unclear)

---

### 3.10 WfcNode (Wave Function Collapse)

**Status**: ⬜ Not audited

**Cartesian Implementation** (`wfc/wfc_node.rs` ~550 lines):

```rust
pub struct WfcNode {
    wave: Wave,           // Possibility tracking
    propagator: Vec<Vec<Vec<usize>>>,  // [direction][pattern] -> compatible patterns
    weights: Vec<f64>,    // Pattern weights
    mx, my, mz: usize,    // Output dimensions
    n: usize,             // Pattern size
    periodic: bool,
}
```

**Critical Cartesian Code**:
```rust
// Direction constants
pub const DX: [i32; 6] = [1, 0, -1, 0, 0, 0];
pub const DY: [i32; 6] = [0, 1, 0, -1, 0, 0];
pub const DZ: [i32; 6] = [0, 0, 0, 0, 1, -1];
pub const OPPOSITE: [usize; 6] = [2, 3, 0, 1, 5, 4];

fn propagate(&mut self) -> bool {
    while let Some((i1, p1)) = self.stack.pop() {
        for d in 0..6 {  // Fixed directions
            let x2 = x1 + DX[d];
            let y2 = y1 + DY[d];
            // ...
        }
    }
}
```

**Cartesian Tests** (10 tests):
- `test_wfc_node_creation`
- `test_wfc_propagate_reduces_possibilities`
- `test_wfc_contradiction_detected`
- `test_wfc_adjacency_constraints_satisfied`
- etc.

**XML Models Using This** (37 models):
- 2D: `Knots2D.xml`, `WaveFlowers.xml`, `Surface.xml`
- 3D: `Knots3D.xml`, `SeaVilla.xml`

**Spherical Translation Challenges**:

This is the most complex translation. Key issues:

1. **Variable neighbor count**: Inner rings have fewer θ neighbors than outer rings
2. **Direction semantics**: No fixed "4 directions" - use neighbor indices instead
3. **Propagator structure**: `propagator[direction][pattern]` assumes fixed directions
4. **Pattern overlap**: What does "overlap" mean in spherical coordinates?

**WFC in Spherical Coordinates - Conceptual Approach**:

Instead of direction-indexed propagators, use **neighbor-indexed** propagators:

```rust
pub struct SphericalWfcNode {
    wave: Wave,
    // For each pattern, list of (neighbor_type, compatible_patterns)
    // neighbor_type: 0=θ-, 1=θ+, 2=r-, 3=r+ (for 2D)
    propagator: Vec<Vec<(u8, Vec<usize>)>>,
    weights: Vec<f64>,
    grid_size: usize,
}

fn propagate(&mut self, grid: &SphericalMjGrid) -> bool {
    while let Some((i1, p1)) = self.stack.pop() {
        let neighbors = grid.neighbors_with_types(i1);
        for (neighbor_idx, neighbor_type) in neighbors {
            // Look up compatible patterns for this neighbor type
            for &t2 in &self.propagator[p1][neighbor_type] {
                // ... ban logic
            }
        }
    }
}
```

**Pattern Definition for Spherical**:

Instead of N×N pixel patterns, use **star patterns**:

```
    θ+
     |
r- --C-- r+
     |
    θ-
```

A "pattern" is (center_value, θ-_value, θ+_value, r-_value, r+_value).

**Estimated Effort**: 8+ hours

---

### 3.11 OverlapNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`wfc/overlap_node.rs` ~350 lines):

Extracts N×N patterns from sample image, builds compatibility from overlap.

**Spherical Translation**:

For spherical grids, "overlap" could mean:
- Adjacent patterns share their boundary cells
- Pattern at (r, θ) shares cells with pattern at (r, θ+1)

**Estimated Effort**: 4 hours (after WfcNode is done)

---

### 3.12 TileNode

**Status**: ⬜ Not audited

**Cartesian Implementation** (`wfc/tile_node.rs` ~400 lines):

Loads 3D tiles from .vox files, places them according to adjacency rules.

**Spherical Translation**:

- Tiles would need to be wedge-shaped
- Adjacency rules based on neighbor types
- Output grid calculation changes

**Estimated Effort**: 4 hours (after WfcNode is done)

---

## 4. Supporting Systems Audit

### 4.1 SphericalInterpreter

**Needed**: Yes, P0 priority

**Implementation**:
```rust
pub struct SphericalInterpreter {
    root: Box<dyn SphericalNode>,
    grid: SphericalMjGrid,
    random: Box<dyn MjRng>,
    origin: bool,
    changes: Vec<usize>,
    first: Vec<usize>,
    counter: usize,
    running: bool,
    animated: bool,
}

impl SphericalInterpreter {
    pub fn new(root: Box<dyn SphericalNode>, grid: SphericalMjGrid) -> Self;
    pub fn with_origin(root: Box<dyn SphericalNode>, grid: SphericalMjGrid) -> Self;
    pub fn reset(&mut self, seed: u64);
    pub fn step(&mut self) -> bool;
    pub fn run(&mut self, seed: u64, max_steps: usize) -> usize;
    pub fn grid(&self) -> &SphericalMjGrid;
}
```

**Estimated Effort**: 2 hours

---

### 4.2 SphericalExecutionContext

**Needed**: Yes, P0 priority

**Implementation**:
```rust
pub struct SphericalExecutionContext<'a> {
    pub grid: &'a mut SphericalMjGrid,
    pub random: &'a mut dyn MjRng,
    pub changes: Vec<usize>,
    pub first: Vec<usize>,
    pub counter: usize,
    pub gif: bool,
}
```

**Estimated Effort**: 30 minutes

---

### 4.3 SphericalRuleNodeData

**Needed**: Yes, P1 priority (for OneNode/AllNode)

**Implementation**:
```rust
pub struct SphericalRuleNodeData {
    pub rules: Vec<SphericalRule>,
    pub matches: Vec<(usize, usize)>,  // (rule_idx, flat_idx)
    pub match_count: usize,
    pub match_mask: Vec<Vec<bool>>,
    pub last_matched_turn: i32,
    // Field/observation support later
}

impl SphericalRuleNodeData {
    pub fn scan_all_matches(&mut self, ctx: &SphericalExecutionContext);
    pub fn scan_incremental_matches(&mut self, ctx: &SphericalExecutionContext);
}
```

**Estimated Effort**: 2 hours

---

### 4.4 SphericalField

**Needed**: Yes, P2 priority (for heuristic-guided nodes)

**Implementation**:
```rust
pub struct SphericalField {
    pub substrate: u32,   // Traversable cells
    pub zero: u32,        // Zero-potential cells
    pub recompute: bool,
}

impl SphericalField {
    pub fn compute(&self, potentials: &mut [i32], grid: &SphericalMjGrid) -> bool {
        // BFS using grid.neighbors() instead of direction vectors
    }
}
```

**Estimated Effort**: 2 hours

---

### 4.5 Spherical XML Loader

**Needed**: Yes, P1 priority

**Implementation**:
```rust
pub struct SphericalLoadedModel {
    pub root: Box<dyn SphericalNode>,
    pub grid: SphericalMjGrid,
    pub origin: bool,
}

pub fn load_spherical_model(path: &Path) -> Result<SphericalLoadedModel, LoadError>;
pub fn load_spherical_model_str(
    xml: &str,
    r_min: u32,
    r_depth: u16,
    target_arc: f32,
) -> Result<SphericalLoadedModel, LoadError>;
```

**Detection strategy**: Look for `r_min` or `r_depth` attributes in root element.

**Estimated Effort**: 4 hours

---

## 5. XML Model Analysis

### 5.1 Model Count by Node Type

| Node Type | Count | Percentage | Notes |
|-----------|-------|------------|-------|
| sequence | 216 | 84% | Nearly all models |
| one | 199 | 78% | Core rule application |
| all | 175 | 68% | Bulk rule application |
| prl | 103 | 40% | Parallel (cellular automata) |
| markov | 87 | 34% | Iterative loops |
| field | 38 | 15% | Heuristic guidance |
| wfc | 37 | 14% | Constraint-based |
| convolution | 31 | 12% | Neighbor-based CA |
| observe | 26 | 10% | Goal-directed |
| path | 24 | 9% | Pathfinding |
| map | 8 | 3% | Multi-scale |
| convchain | 4 | 2% | Texture synthesis |

### 5.2 Test Models by Priority

**P0 - Core functionality** (sequence, markov, one, all):
- `Basic.xml` - simplest model
- `Growth.xml` - origin + growth
- `Flowers.xml` - multiple rules
- `River.xml` - sequences

**P1 - Extended functionality** (parallel, field):
- `Cave.xml` - parallel rules
- `GameOfLife.xml` - cellular automata
- `BiasedGrowth.xml` - field-guided

**P2 - Complex features** (path, convolution):
- `Percolation.xml` - simple path
- `DijkstraDungeon.xml` - complex path
- `Hills.xml` - convolution

**P3 - Advanced features** (wfc, map, convchain):
- `Knots2D.xml` - WFC overlap
- `MazeMap.xml` - map scaling
- `ChainMaze.xml` - convchain

---

## 6. Wave Function Collapse Deep Dive

### 6.1 WFC Algorithm Overview

Wave Function Collapse is a constraint satisfaction algorithm:

1. **Initialize**: Each output cell can be any pattern (superposition)
2. **Observe**: Pick cell with minimum entropy, collapse to single pattern
3. **Propagate**: Update neighbors' possibilities based on constraints
4. **Repeat**: Until all cells collapsed or contradiction

### 6.2 Cartesian WFC Structure

**Patterns**: N×N pixel blocks extracted from sample image

**Compatibility**: `propagator[direction][pattern1]` = patterns that can be adjacent
to pattern1 in the given direction (0=+X, 1=+Y, 2=-X, 3=-Y)

**Example**: 3×3 patterns, 4 directions
```
Pattern A:      Pattern B:
[1,1,1]         [1,1,0]
[1,0,0]         [1,0,0]
[1,0,0]         [1,0,0]

A can be left of B because A's rightmost column matches B's leftmost column.
```

### 6.3 Why Cartesian WFC Doesn't Work for Spherical

1. **Fixed directions**: DX/DY/DZ assumes uniform grid
2. **Pattern shape**: N×N assumes rectangular cells
3. **Overlap checking**: Column/row matching assumes alignment
4. **Propagator indexing**: `propagator[direction]` assumes finite directions

### 6.4 Spherical WFC Approach

**Star Patterns**: Instead of N×N blocks, use center + neighbors:
```
Pattern = (center_value, θ-_value, θ+_value, r-_value, r+_value)

For 2D polar with 2 values (B/W):
- Pattern (0, 0, 0, 0, 0) = all black
- Pattern (1, 0, 0, 0, 0) = white center, black neighbors
- Pattern (1, 1, 0, 0, 0) = white center + θ- neighbor
- etc.
```

**Neighbor-Type Compatibility**:
```rust
// For each pattern, for each neighbor type, list of compatible patterns
struct SphericalPropagator {
    // propagator[pattern_idx] = [(neighbor_type, [compatible_patterns])]
    data: Vec<Vec<(NeighborType, Vec<usize>)>>,
}

enum NeighborType {
    ThetaMinus = 0,
    ThetaPlus = 1,
    RMinus = 2,
    RPlus = 3,
    PhiMinus = 4,  // 3D only
    PhiPlus = 5,   // 3D only
}
```

**Propagation Algorithm**:
```rust
fn propagate(&mut self, grid: &SphericalMjGrid) -> bool {
    while let Some((cell, banned_pattern)) = self.stack.pop() {
        // Get this cell's coordinates
        let (r, theta, phi) = grid.index_to_coord(cell);
        
        // Get actual neighbors (handles boundary correctly)
        let neighbors = grid.neighbors_with_indices(r, theta, phi);
        
        for (neighbor_cell, neighbor_type) in neighbors {
            // Find patterns that were compatible with banned_pattern
            // in this neighbor direction
            let opposite_type = opposite(neighbor_type);
            
            for &pattern in &self.wave.possible_patterns(neighbor_cell) {
                // Check if pattern required banned_pattern
                let compatible = &self.propagator[pattern][opposite_type];
                if compatible.contains(&banned_pattern) {
                    // Decrement compatible count
                    // If reaches 0, ban this pattern from neighbor
                }
            }
        }
    }
}
```

### 6.5 Spherical WFC Challenges

1. **Variable cell count per ring**: Inner rings have fewer cells
   - Solution: Each cell knows its neighbors, propagate to actual neighbors

2. **Pattern extraction from sample**: No "sample image" for spherical
   - Solution: Define patterns programmatically or from spherical sample grid

3. **Symmetry handling**: Cartesian has 8 symmetries (rotate/reflect)
   - Solution: Use SphericalSymmetry (Klein 4-group) for pattern variants

4. **Edge behavior**: θ wraps, r doesn't
   - Solution: `neighbors()` already handles this

### 6.6 Spherical WFC Estimated Scope

| Component | Effort | Notes |
|-----------|--------|-------|
| SphericalWave | 2 hours | Similar to Wave, different neighbor counting |
| SphericalPropagator | 2 hours | Neighbor-type indexed |
| SphericalWfcNode | 4 hours | Core algorithm adaptation |
| Pattern extraction | 2 hours | Star pattern definition |
| Testing | 2 hours | Verify constraint propagation |
| **Total** | **12 hours** | |

---

## 7. Implementation Roadmap

### 7.1 Phase 1: Core Infrastructure (P0)

**Estimated**: 6 hours

| Task | Effort | Dependencies |
|------|--------|--------------|
| SphericalNode trait | 30 min | None |
| SphericalExecutionContext | 30 min | SphericalNode |
| SphericalInterpreter | 2 hours | SphericalNode, Context |
| SphericalSequenceNode | 30 min | SphericalNode |
| SphericalMarkovNode | 30 min | SphericalNode |
| Basic integration test | 1 hour | All above |

### 7.2 Phase 2: Basic Nodes (P0)

**Estimated**: 8 hours

| Task | Effort | Dependencies |
|------|--------|--------------|
| SphericalRuleNodeData | 2 hours | SphericalExecutionContext |
| SphericalOneNode | 2 hours | SphericalRuleNodeData |
| SphericalAllNode | 2 hours | SphericalRuleNodeData |
| Tests for each | 2 hours | All above |

### 7.3 Phase 3: XML Loading (P1)

**Estimated**: 6 hours

| Task | Effort | Dependencies |
|------|--------|--------------|
| Spherical XML detection | 1 hour | None |
| SphericalLoadedModel | 1 hour | SphericalInterpreter |
| load_spherical_model | 3 hours | All nodes |
| Tests | 1 hour | All above |

### 7.4 Phase 4: Extended Nodes (P1-P2)

**Estimated**: 10 hours

| Task | Effort | Dependencies |
|------|--------|--------------|
| SphericalParallelNode | 2 hours | Phase 2 |
| SphericalField | 2 hours | Phase 2 |
| SphericalPathNode | 4 hours | SphericalField |
| Tests | 2 hours | All above |

### 7.5 Phase 5: Advanced Nodes (P3)

**Estimated**: 16 hours

| Task | Effort | Dependencies |
|------|--------|--------------|
| SphericalConvolutionNode | 4 hours | Phase 2 |
| SphericalWfcNode | 8 hours | Phase 2 |
| SphericalOverlapNode | 4 hours | SphericalWfcNode |
| Tests | Included | - |

### 7.6 Total Estimated Effort

| Phase | Hours | Priority |
|-------|-------|----------|
| Phase 1 | 6 | P0 |
| Phase 2 | 8 | P0 |
| Phase 3 | 6 | P1 |
| Phase 4 | 10 | P1-P2 |
| Phase 5 | 16 | P3 |
| **Total** | **46** | - |

---

## 8. Audit Checklist

Track audit progress here:

### 8.1 Node Audits

- [ ] SequenceNode
- [ ] MarkovNode
- [ ] OneNode
- [ ] AllNode
- [ ] ParallelNode
- [ ] PathNode
- [ ] ConvolutionNode
- [ ] ConvChainNode
- [ ] MapNode
- [ ] WfcNode
- [ ] OverlapNode
- [ ] TileNode

### 8.2 System Audits

- [ ] Interpreter
- [ ] ExecutionContext
- [ ] RuleNodeData
- [ ] Field
- [ ] Observation
- [ ] XML Loader
- [ ] Wave

### 8.3 Translation Verification

- [ ] Basic model running
- [ ] Ring growth model
- [ ] Multiple rules
- [ ] Parallel execution
- [ ] Path finding
- [ ] WFC generation

---

## Appendix A: File Reference

| Component | Cartesian File | Lines |
|-----------|----------------|-------|
| MjGrid | `mod.rs` | 1-600 |
| Node trait | `node.rs` | 1-100 |
| SequenceNode | `node.rs` | 130-200 |
| MarkovNode | `node.rs` | 200-280 |
| OneNode | `one_node.rs` | 1-280 |
| AllNode | `all_node.rs` | 1-250 |
| ParallelNode | `parallel_node.rs` | 1-220 |
| PathNode | `path_node.rs` | 1-500 |
| ConvolutionNode | `convolution_node.rs` | 1-400 |
| ConvChainNode | `convchain_node.rs` | 1-350 |
| MapNode | `map_node.rs` | 1-300 |
| WfcNode | `wfc/wfc_node.rs` | 1-550 |
| OverlapNode | `wfc/overlap_node.rs` | 1-350 |
| TileNode | `wfc/tile_node.rs` | 1-400 |
| Wave | `wfc/wave.rs` | 1-400 |
| Interpreter | `interpreter.rs` | 1-430 |
| RuleNodeData | `rule_node.rs` | 1-500 |
| Field | `field.rs` | 1-300 |
| Loader | `loader.rs` | 1-1000 |

---

## Appendix B: Test Reference

| Node | Test File | Test Count |
|------|-----------|------------|
| SequenceNode | `node.rs`, `node_tests.rs` | 2 |
| MarkovNode | `node.rs`, `node_tests.rs` | 2 |
| OneNode | `one_node.rs`, `node_tests.rs` | 6 |
| AllNode | `all_node.rs`, `node_tests.rs` | 6 |
| ParallelNode | `parallel_node.rs` | 3 |
| PathNode | `path_node.rs` | 5 |
| ConvolutionNode | `convolution_node.rs` | 15 |
| ConvChainNode | `convchain_node.rs` | 16 |
| MapNode | `map_node.rs` | 5 |
| WfcNode | `wfc/wfc_node.rs` | 10 |
| OverlapNode | `wfc/overlap_node.rs` | 9 |
| TileNode | `wfc/tile_node.rs` | 14 |
| Wave | `wfc/wave.rs` | 11 |
| Interpreter | `interpreter.rs` | 5 |
| Field | `field.rs` | 6 |
