# Grid Abstraction Implementation Plan

## Current Status

**Phases 0-6 COMPLETE** - Core grid abstraction is done.
**Phases 7-12 PENDING** - Polar/Spherical unification and full feature parity.

### Critical Issue: Duplicate Polar Implementations

We have TWO polar grid implementations that need to be unified:

| File | Lines | Has MjGridOps | Has Rules/Patterns | Has Symmetries | Tests |
|------|-------|---------------|-------------------|----------------|-------|
| `polar_grid.rs` | 2543 | NO | YES (PolarRule, PolarPattern) | YES (PolarSymmetry) | 42 |
| `spherical_grid.rs` | 572 | YES | NO | NO | 12 |

**The Problem**: `SphericalMjGrid` implements `MjGridOps` (so it can work with existing nodes), but it has no rule system. `PolarMjGrid` has a complete rule system but doesn't implement `MjGridOps`.

**The Solution**: `SphericalMjGrid` subsumes `PolarMjGrid`. A 2D polar grid is just `SphericalMjGrid` with `phi_divisions=1`. We need to:
1. Port all polar functionality (rules, patterns, symmetries) to work with `SphericalMjGrid`
2. Port all 42 polar tests to use `SphericalMjGrid`
3. Delete `PolarMjGrid` once verified

---

## 1. Research Summary

### 1.1 Test Infrastructure

**Total MarkovJunior Tests**: 388 tests across 29 modules

**Key Test Categories**:
| Category | Count | Purpose |
|----------|-------|---------|
| `loader` | 43 | XML parsing, model loading |
| `wfc` | 44 | Wave Function Collapse |
| `polar_grid` | 42 | Polar coordinate system (TO BE MIGRATED) |
| `render` | 31 | PNG/image rendering |
| `convchain_node` | 16 | MCMC texture synthesis |
| `convolution_node` | 15 | Cellular automata |
| `verification` | 11 | **C# parity checks** |
| `node_tests` | 8 | Core node behavior |
| `interpreter` | 5 | Execution loop |

### 1.2 Polar Grid Functionality to Migrate

From `polar_grid.rs`, the following must be ported to `spherical_grid.rs`:

```rust
// Types to port
pub struct PolarNeighbors { ... }      // -> SphericalNeighbors
pub enum PolarSymmetry { ... }         // -> SphericalSymmetry (4-group for 2D, larger for 3D)
pub struct PolarPattern { ... }        // -> SphericalPattern
pub struct PolarRule { ... }           // -> SphericalRule
pub struct PolarModel { ... }          // -> Use existing interpreter with SphericalMjGrid

// Traits implemented
impl RecordableGrid for PolarMjGrid    // -> impl for SphericalMjGrid
impl Renderable2D for PolarMjGrid      // -> impl for SphericalMjGrid
```

### 1.3 Test Categories to Migrate (42 tests)

| Level | Tests | Description |
|-------|-------|-------------|
| 0 | 5 | Data structures (grid creation, read/write) |
| 1 | 4 | Geometry (theta divisions, distortion) |
| 2 | 6 | Neighbors (angular, radial, boundary) |
| 3 | 7 | Symmetries (Klein four-group) |
| 4 | 5 | Single-step rules |
| 5 | 3 | Multi-step models |
| 6 | 4 | Rendering |
| 7 | 8 | Integration (full model execution) |

---

## 2. Implementation Phases

### Overview (Updated)

```
COMPLETED:
  Phase 0: Baseline established
  Phase 1: MjGridOps trait definition
  Phase 2: MjGrid trait implementation  
  Phase 3: ExecutionContext generic
  Phase 4: Match/Changes format migration
  Phase 5: Full integration verification
  Phase 6: SphericalMjGrid storage + MjGridOps

PENDING:
  Phase 7: Polar->Spherical Test Migration (port 42 tests)
  Phase 8: Spherical Symmetry System
  Phase 9: Spherical Pattern/Rule System
  Phase 10: Spherical XML Loading
  Phase 11: Spherical Rendering
  Phase 12: Delete PolarMjGrid, Performance Benchmarks
```

---

## 3-8. Phases 0-5: COMPLETE

See git history for implementation details:
- Phase 1-3: commit `8ef8cc3`
- Phase 4-5: commit `a6a78fc`
- Phase 6: commit `bfa96d6`

---

## 9. Phase 6: SphericalMjGrid Storage (COMPLETE)

Created `spherical_grid.rs` with:
- Flat storage (`Vec<u8>` state, `Vec<bool>` mask)
- `MjGridOps` trait implementation
- Coordinate conversion (`r,theta,phi` <-> flat index)
- Basic neighbor lookup
- 12 unit tests

**What's Missing**: Rules, patterns, symmetries, rendering, XML loading.

---

## 10. Phase 7: Polar Test Migration

### 10.1 Goal

Port all 42 polar tests from `PolarMjGrid` to `SphericalMjGrid`. Tests should pass with the new implementation before we add any new functionality.

### 10.2 Test Migration Strategy

For each test level, migrate tests in order:

**Level 0: Data Structures (5 tests)**
```rust
// Before (polar_grid.rs)
let grid = PolarMjGrid::new(256, 256, 1.0);

// After (spherical_grid.rs)
let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");
```

Tests to migrate:
- [ ] `test_grid_creation`
- [ ] `test_cell_read_write`
- [ ] `test_theta_wrapping`
- [ ] `test_memory_layout`
- [ ] `test_theta_divisions_formula`

**Level 1: Geometry (4 tests)**
- [ ] `test_no_distortion_between_rings`
- [ ] `test_arc_length_varies_by_radius`
- [ ] `test_angular_range`
- [ ] (already have `test_coord_conversion_2d`)

**Level 2: Neighbors (6 tests)**

Need to add `SphericalNeighbors` struct and `neighbors()` method:
```rust
pub struct SphericalNeighbors {
    pub theta_minus: Option<usize>,  // flat index
    pub theta_plus: Option<usize>,
    pub r_minus: Option<usize>,
    pub r_plus: Option<usize>,
    pub phi_minus: Option<usize>,    // None for 2D
    pub phi_plus: Option<usize>,     // None for 2D
}
```

Tests to migrate:
- [ ] `test_angular_neighbors`
- [ ] `test_angular_neighbor_wrapping`
- [ ] `test_radial_neighbors_bounded`
- [ ] `test_radial_neighbor_alignment`
- [ ] `test_boundary_neighbors`
- [ ] `test_neighbor_symmetry`

**Level 3: Symmetries (7 tests)** - See Phase 8

**Level 4: Single-Step Rules (5 tests)** - See Phase 9

**Level 5-7: Models, Rendering, Integration** - See Phases 9-11

### 10.3 Verification

```bash
# After each level migration
cargo test -p studio_core markov_junior::spherical_grid

# Target: 42+ tests in spherical_grid (12 existing + 30+ migrated)
```

---

## 11. Phase 8: Spherical Symmetry System

### 11.1 Goal

Port `PolarSymmetry` to work with `SphericalMjGrid`. For 2D polar (phi_divisions=1), this is the Klein four-group (4 symmetries). For 3D spherical, this may be larger.

### 11.2 Implementation

Add to `spherical_grid.rs`:

```rust
/// Symmetries for polar/spherical grids.
/// 
/// For 2D polar (phi_divisions=1): Klein four-group (4 symmetries)
/// - Identity
/// - ThetaFlip: (dr, dtheta) -> (dr, -dtheta)
/// - RFlip: (dr, dtheta) -> (-dr, dtheta)
/// - BothFlip: (dr, dtheta) -> (-dr, -dtheta)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SphericalSymmetry {
    Identity,
    ThetaFlip,
    RFlip,
    BothFlip,
    // Future: Add phi symmetries for 3D
}

impl SphericalSymmetry {
    /// All symmetries for 2D polar grids
    pub fn all_2d() -> &'static [SphericalSymmetry] {
        &[Self::Identity, Self::ThetaFlip, Self::RFlip, Self::BothFlip]
    }
    
    /// Transform a relative offset (dr, dtheta, dphi)
    pub fn transform(&self, dr: i8, dtheta: i8, dphi: i8) -> (i8, i8, i8) {
        match self {
            Self::Identity => (dr, dtheta, dphi),
            Self::ThetaFlip => (dr, -dtheta, dphi),
            Self::RFlip => (-dr, dtheta, dphi),
            Self::BothFlip => (-dr, -dtheta, dphi),
        }
    }
}
```

### 11.3 Tests to Migrate (7 tests)

- [ ] `test_identity_symmetry`
- [ ] `test_theta_flip_symmetry`
- [ ] `test_r_flip_symmetry`
- [ ] `test_both_flip_symmetry`
- [ ] `test_symmetry_group_closure`
- [ ] `test_pattern_symmetry_variants`
- [ ] `test_symmetric_pattern_fewer_variants`

### 11.4 Verification

```bash
cargo test -p studio_core markov_junior::spherical_grid::symmetry
```

---

## 12. Phase 9: Spherical Pattern/Rule System

### 12.1 Goal

Port `PolarPattern` and `PolarRule` to work with `SphericalMjGrid`. This enables MarkovJunior rules to run on polar grids.

### 12.2 Implementation

```rust
/// Pattern for matching cells in a spherical grid.
/// Uses flat indices internally, but pattern definition uses relative offsets.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SphericalPattern {
    /// Center cell value (required)
    pub center: u8,
    /// Neighbor requirements (None = wildcard)
    pub theta_minus: Option<u8>,
    pub theta_plus: Option<u8>,
    pub r_minus: Option<u8>,
    pub r_plus: Option<u8>,
    pub phi_minus: Option<u8>,  // For 3D
    pub phi_plus: Option<u8>,   // For 3D
}

impl SphericalPattern {
    /// Check if pattern matches at given flat index
    pub fn matches(&self, grid: &SphericalMjGrid, idx: usize) -> bool {
        // Check center
        if grid.get_state(idx) != self.center {
            return false;
        }
        // Check neighbors
        let neighbors = grid.neighbors_at(idx);
        // ... check each neighbor requirement
    }
    
    /// Generate all symmetry variants of this pattern
    pub fn symmetry_variants(&self) -> Vec<SphericalPattern> {
        SphericalSymmetry::all_2d()
            .iter()
            .map(|s| self.transform(*s))
            .collect()
    }
}

/// Rewrite rule for spherical grids
pub struct SphericalRule {
    pub input: SphericalPattern,
    pub output: u8,
}
```

### 12.3 Integration with Existing Nodes

The key insight: `SphericalMjGrid` implements `MjGridOps`, so it can already be used with `ExecutionContext`. But the existing `MjRule` uses Cartesian coordinates. We need either:

**Option A**: Create `SphericalRule` that works with `SphericalMjGrid` directly
**Option B**: Make `MjRule` generic over coordinate systems

For now, **Option A** is simpler and maintains separation.

### 12.4 Tests to Migrate (5 tests)

- [ ] `test_simple_rule_matching`
- [ ] `test_rule_application`
- [ ] `test_conditional_rule`
- [ ] `test_rule_with_symmetries`
- [ ] `test_multiple_rules_priority`

### 12.5 Verification

```bash
cargo test -p studio_core markov_junior::spherical_grid::rules
```

---

## 13. Phase 10: Generic Interpreter and XML Loading

### 13.1 Goal

Make `Interpreter` and `LoadedModel` generic over `MjGridOps` so they can work with
any grid type (Cartesian or Spherical). This is the prerequisite for XML loading of
spherical models.

### 13.2 Dependency Analysis

**Current grid-specific code in `Interpreter`:**

| Location | Code | Purpose | Solution |
|----------|------|---------|----------|
| Lines 143-146 | `self.grid.mx/my/mz` | Calculate center index for `origin` | Add `center_index()` to `MjGridOps` |
| Lines 181-184 | Same | Same | Same |
| Line 139, 177 | `self.grid.clear()` | Reset grid state | Add `clear()` to `MjGridOps` |
| Line 271 | Return `&MjGrid` | Expose grid to callers | Return `&G` where `G: MjGridOps` |

**Current grid-specific code in `LoadedModel`:**

| Location | Code | Purpose | Solution |
|----------|------|---------|----------|
| Line 115 | `pub grid: MjGrid` | Field type | Make generic `G: MjGridOps` |
| Line 123 | `(self.grid.mx, my, mz)` | Debug output | Use `dimensions()` |

**Call sites (non-test):** 13 total
- `model.rs`: 6 calls to `Interpreter::new/with_origin`
- `verification.rs`: 4 calls
- `lua_api.rs`: 2 calls
- `mod.rs`: 1 export

**Impact with default type parameter:** All existing code compiles unchanged because
`Interpreter<G: MjGridOps = MjGrid>` defaults to `MjGrid`.

### 13.3 Implementation - Phase 10a: Extend MjGridOps

Add two methods to `MjGridOps` trait in `grid_ops.rs`:

```rust
pub trait MjGridOps {
    // ... existing methods ...

    /// Clear the grid state (all cells to 0) and reset mask.
    fn clear(&mut self) {
        self.state_mut().fill(0);
        self.clear_mask();
    }

    /// Get the center cell index (for origin placement).
    /// 
    /// For Cartesian: mx/2 + (my/2)*mx + (mz/2)*mx*my
    /// For Spherical: r_depth/2 * theta_divisions (middle ring, theta=0)
    fn center_index(&self) -> usize;
}
```

Implement in `MjGrid`:
```rust
fn center_index(&self) -> usize {
    self.mx / 2 + (self.my / 2) * self.mx + (self.mz / 2) * self.mx * self.my
}
```

Implement in `SphericalMjGrid`:
```rust
fn center_index(&self) -> usize {
    // Middle radial ring, theta=0, phi=0
    let r = self.r_depth / 2;
    self.coord_to_index(r, 0, 0)
}
```

**Verification:**
```bash
cargo test -p studio_core markov_junior::grid_ops
cargo test -p studio_core markov_junior::spherical_grid::tests::mjgridops
```

### 13.4 Implementation - Phase 10b: Generic Interpreter

Modify `interpreter.rs`:

```rust
use super::grid_ops::MjGridOps;
use super::MjGrid;

/// Main interpreter for running MarkovJunior models.
pub struct Interpreter<G: MjGridOps = MjGrid> {
    root: Box<dyn Node>,
    grid: G,  // Was: MjGrid
    random: Box<dyn MjRng>,
    origin: bool,
    changes: Vec<usize>,
    first: Vec<usize>,
    counter: usize,
    running: bool,
    animated: bool,
}

impl<G: MjGridOps> Interpreter<G> {
    pub fn new(root: Box<dyn Node>, grid: G) -> Self { ... }
    pub fn with_origin(root: Box<dyn Node>, grid: G) -> Self { ... }
    
    pub fn reset(&mut self, seed: u64) {
        self.random = Box::new(StdRandom::from_u64_seed(seed));
        self.grid.clear();  // Use trait method
        
        if self.origin {
            let center = self.grid.center_index();  // Use trait method
            self.grid.set_state(center, 1);  // Use trait method
        }
        // ... rest unchanged
    }
    
    pub fn grid(&self) -> &G {  // Generic return type
        &self.grid
    }
}
```

**Key changes:**
1. Add generic parameter `G: MjGridOps = MjGrid`
2. Replace `self.grid.mx/my/mz` calculation with `self.grid.center_index()`
3. Replace `self.grid.clear()` with trait method
4. Replace `self.grid.state[center] = 1` with `self.grid.set_state(center, 1)`
5. Return `&G` instead of `&MjGrid`

**Verification:**
```bash
cargo test -p studio_core markov_junior::interpreter
cargo test -p studio_core markov_junior::verification  # All 11 parity tests
```

### 13.5 Implementation - Phase 10c: Generic LoadedModel

Modify `loader.rs`:

```rust
use super::grid_ops::MjGridOps;
use super::MjGrid;

/// Result of loading a model from XML.
pub struct LoadedModel<G: MjGridOps = MjGrid> {
    pub root: Box<dyn Node>,
    pub grid: G,
    pub origin: bool,
}

impl<G: MjGridOps> std::fmt::Debug for LoadedModel<G> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (d0, d1, d2) = self.grid.dimensions();
        f.debug_struct("LoadedModel")
            .field("grid_dimensions", &(d0, d1, d2))
            .field("origin", &self.origin)
            .finish()
    }
}
```

**Note:** The `load_model` functions still return `LoadedModel<MjGrid>` for now.
Phase 10d will add spherical loading functions.

**Verification:**
```bash
cargo test -p studio_core markov_junior::loader
```

### 13.6 Implementation - Phase 10d: Spherical Model Loading

Add new loading functions in `loader.rs`:

```rust
/// Load a spherical model from an XML string.
/// 
/// Detects spherical grid from `r_min` or `r_depth` attributes.
pub fn load_spherical_model_str(
    xml: &str,
    r_min: u32,
    r_depth: u16,
    target_arc: f32,
) -> Result<LoadedModel<SphericalMjGrid>, LoadError> {
    // ... implementation
}
```

**XML Format for Spherical Models:**
```xml
<!-- Spherical model uses r_min/r_depth instead of mx/my -->
<one values="BW" r_min="256" r_depth="64" target_arc="1.0">
  <rule in="B" out="W"/>
</one>

<!-- 3D spherical adds phi_divisions -->
<one values="BW" r_min="256" r_depth="64" theta_divisions="360" phi_divisions="180">
  <rule in="B" out="W"/>
</one>
```

**Detection logic:**
- Has `r_min` OR `r_depth` → Spherical grid
- Has `phi_divisions` > 1 → 3D spherical, else 2D polar

### 13.7 Tests

Phase 10a (MjGridOps):
- [x] `test_mjgrid_clear` (existing)
- [ ] `test_mjgrid_center_index`
- [ ] `test_spherical_center_index`

Phase 10b (Generic Interpreter):
- [ ] `test_interpreter_with_mjgrid` (existing tests should pass)
- [ ] `test_interpreter_with_spherical_grid`

Phase 10c (Generic LoadedModel):
- [ ] `test_loaded_model_debug_cartesian`
- [ ] `test_loaded_model_debug_spherical`

Phase 10d (Spherical Loading):
- [ ] `test_load_spherical_grid_from_xml`
- [ ] `test_detect_spherical_from_attributes`
- [ ] `test_load_spherical_model_runs`

### 13.8 Verification

```bash
# Phase 10a
cargo test -p studio_core markov_junior::grid_ops

# Phase 10b - must pass ALL existing tests
cargo test -p studio_core markov_junior::interpreter
cargo test -p studio_core markov_junior::verification

# Phase 10c
cargo test -p studio_core markov_junior::loader

# Phase 10d
cargo test -p studio_core markov_junior::loader::spherical

# Full regression
cargo test -p studio_core markov_junior
```

### 13.9 DETAILED ANALYSIS: Generic Node vs Separate Spherical Implementation

**Last Updated**: During Phase 10b investigation

This section provides a comprehensive analysis of two approaches for enabling
spherical grid support in MarkovJunior's node execution system.

---

#### 13.9.1 The Core Problem

The `Node` trait is tied to `MjGrid` through `ExecutionContext`:

```rust
// node.rs line 101 - uses default type parameter
pub trait Node {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool;  // = ExecutionContext<'_, MjGrid>
}

// ExecutionContext is already generic
pub struct ExecutionContext<'a, G: MjGridOps = MjGrid> {
    pub grid: &'a mut G,
    // ...
}
```

Making `Interpreter<G>` creates `ExecutionContext<'_, G>`, but `Node::go()` expects
`ExecutionContext<'_, MjGrid>` — type mismatch.

---

#### 13.9.2 Grid Access Inventory

**Total: ~163 grid accesses across all node implementations**

| Access Type | Count | In Trait? | Difficulty |
|-------------|-------|-----------|------------|
| `ctx.grid.state[i]` / `state()` | 90+ | YES | Easy |
| `ctx.grid.mx/my/mz` | 60+ | Via `dimensions()` | Easy |
| `ctx.grid.mask[i]` | 6 | Via `get/set_mask()` | Easy |
| `ctx.grid.index_to_coord(idx)` | 8 | NO | Medium |
| `ctx.grid.matches(rule, x,y,z)` | 10+ | NO | **Hard** |
| Index formula `x + y*mx + z*mx*my` | 30+ | Could add method | Medium |

**The "Hard" category is the blocker**: `matches()` takes Cartesian `(x,y,z)`
coordinates and checks a Cartesian `MjRule` pattern. This is fundamentally
coordinate-system-specific.

---

#### 13.9.3 Node-by-Node Analysis

| Node | Grid Accesses | Cartesian Logic | Generification Difficulty |
|------|---------------|-----------------|---------------------------|
| **SequenceNode** | 0 | None | **Already generic** |
| **MarkovNode** | 0 | None | **Already generic** |
| **OneNode** | 12 | `index_to_coord`, `matches`, index formula | Medium |
| **AllNode** | 12 | Same as OneNode + mask | Medium |
| **ParallelNode** | 12 | Same as OneNode | Medium |
| **PathNode** | 19+ | Cardinal directions (N/S/E/W/Up/Down), BFS | **Very Hard** |
| **ConvolutionNode** | 14 | 3x3/3x3x3 kernels, dx/dy/dz offsets | **Very Hard** |
| **ConvChainNode** | 11 | Pattern indexing, modular x/y | Hard |
| **MapNode** | 11 | Scale factors, 3D iteration | **Very Hard** |
| **RuleNodeData** | 18+ | ishifts mechanism, strided scanning | **Very Hard** |
| **OverlapNode** | 18 | 4-direction propagation | Hard |
| **TileNode** | 12 | Overlap calculation | Hard |
| **WfcNode** | 24+ | DX/DY/DZ directions, propagation | **Very Hard** |

**Key Finding**: About 40% of the node code (branch nodes, basic rule nodes) could
be generified with moderate effort. About 60% (WFC, convolution, path, map) is
deeply tied to Cartesian semantics.

---

#### 13.9.4 OPTION A: Generic Node<G>

**Approach**: Make `Node` generic over grid type.

```rust
pub trait Node<G: MjGridOps> {
    fn go(&mut self, ctx: &mut ExecutionContext<'_, G>) -> bool;
    fn reset(&mut self);
}

// All nodes become:
impl<G: MjGridOps> Node<G> for OneNode<G> { ... }

// Interpreter becomes:
pub struct Interpreter<G: MjGridOps = MjGrid> {
    root: Box<dyn Node<G>>,  // Now parameterized
    grid: G,
    // ...
}
```

**What Must Change:**

1. **Trait Definition** (1 file, ~10 lines)
   - Change `trait Node` to `trait Node<G: MjGridOps>`
   - Update `go()` signature

2. **Add Methods to MjGridOps** (1 file, ~50 lines)
   ```rust
   // grid_ops.rs additions
   fn coord_to_index(&self, c0: i32, c1: i32, c2: i32) -> Option<usize>;
   fn index_to_coord(&self, idx: usize) -> (i32, i32, i32);
   fn iter_indices(&self) -> impl Iterator<Item = usize>;
   fn neighbors(&self, idx: usize) -> impl Iterator<Item = (usize, u8)>;
   ```

3. **Node Implementations** (~12 files, ~400 lines of changes)
   
   **Easy nodes** (branch, one, all, parallel):
   - Replace `ctx.grid.mx/my/mz` with `let (d0, d1, d2) = ctx.grid.dimensions()`
   - Replace manual index formula with `ctx.grid.coord_to_index(x, y, z)`
   - Replace `ctx.grid.index_to_coord(idx)` with trait method
   - Keep using `ctx.grid.state()`/`state_mut()` (already in trait)

   **Medium nodes** (convchain, overlap, tile):
   - Same as above
   - Refactor neighbor iteration to use `ctx.grid.neighbors()`

   **Hard nodes** (path, convolution, map, wfc):
   - Require coordinate-system-specific logic
   - Two sub-options:
     a. Make them `Node<MjGrid>` only (not generic)
     b. Create spherical variants (SphericalPathNode, etc.)

4. **Rule System** (~2 files, ~200 lines)
   - Create `trait RuleOps<G>` for grid-type-specific rule matching
   - `MjRule` implements `RuleOps<MjGrid>`
   - `SphericalRule` implements `RuleOps<SphericalMjGrid>`

5. **Loader** (~1 file, ~100 lines)
   - Make `LoadedModel<G>` generic
   - Add `load_spherical_model()` function

**Effort Estimate:**
- Branch nodes: 1 hour
- OneNode/AllNode/ParallelNode: 3 hours
- Rule trait abstraction: 2 hours
- Easy WFC nodes: 2 hours
- Loader changes: 1 hour
- **Subtotal for "easy" generification: ~9 hours**

- ConvChain: 2 hours (coordinate wrapping)
- OverlapNode/TileNode: 3 hours each
- PathNode: 4 hours (spherical pathfinding is different)
- ConvolutionNode: 4 hours (spherical kernels)
- MapNode: 3 hours (spherical scaling?)
- WfcNode: 4 hours (spherical propagation)
- **Subtotal for "hard" generification: ~23 hours**

**Total for full Option A: ~32 hours**

**Risks:**
- Trait object handling gets complex: `Box<dyn Node<MjGrid>>` vs `Box<dyn Node<SphericalMjGrid>>`
- Some nodes may not make conceptual sense for spherical grids (MapNode scaling?)
- Performance regression possible from additional trait bounds

---

#### 13.9.5 OPTION B: Separate Spherical Implementation

**Approach**: Keep existing Cartesian infrastructure unchanged. Build parallel
spherical infrastructure.

**What Already Exists (from spherical_grid.rs):**

```rust
// Grid - DONE (50 tests passing)
pub struct SphericalMjGrid { ... }
impl MjGridOps for SphericalMjGrid { ... }

// Rules - DONE
pub struct SphericalPattern { center, theta_minus, theta_plus, r_minus, r_plus, phi_minus, phi_plus }
pub struct SphericalRule { input: SphericalPattern, output: u8 }

// Symmetries - DONE
pub enum SphericalSymmetry { Identity, ThetaFlip, RFlip, BothFlip }

// Simple execution - EXISTS (but very basic)
fn run_step(grid: &mut SphericalMjGrid, rules: &[SphericalRule]) -> bool { ... }
```

**What Must Be Built:**

1. **SphericalInterpreter** (~200 lines)
   ```rust
   pub struct SphericalInterpreter {
       root: Box<dyn SphericalNode>,
       grid: SphericalMjGrid,
       random: Box<dyn MjRng>,
       origin: bool,
       changes: Vec<usize>,
       // ...
   }
   ```

2. **SphericalNode Trait** (~50 lines)
   ```rust
   pub trait SphericalNode {
       fn go(&mut self, ctx: &mut SphericalExecutionContext) -> bool;
       fn reset(&mut self);
   }
   
   pub struct SphericalExecutionContext<'a> {
       pub grid: &'a mut SphericalMjGrid,
       pub random: &'a mut dyn MjRng,
       pub changes: Vec<usize>,
       // ...
   }
   ```

3. **Spherical Node Implementations**

   | Node | Effort | Notes |
   |------|--------|-------|
   | SphericalSequenceNode | 30 min | Direct port, no grid access |
   | SphericalMarkovNode | 30 min | Direct port, no grid access |
   | SphericalOneNode | 2 hours | Port with SphericalRule matching |
   | SphericalAllNode | 2 hours | Port with mask handling |
   | SphericalParallelNode | 2 hours | Port parallel application |
   | SphericalPathNode | 4 hours | **Different algorithm** - radial/angular pathfinding |
   | SphericalConvolutionNode | 4 hours | **Different kernels** - neighbor-based |
   | SphericalWfcNode | **8+ hours** | **Major rethink** - non-Cartesian propagation |

4. **Spherical XML Loader** (~300 lines)
   ```rust
   pub fn load_spherical_model(path: &Path) -> Result<SphericalLoadedModel, LoadError>
   pub fn load_spherical_model_str(xml: &str, ...) -> Result<SphericalLoadedModel, LoadError>
   ```

**Effort Estimate:**
- SphericalInterpreter: 2 hours
- SphericalNode trait + context: 1 hour
- Branch nodes (Sequence, Markov): 1 hour
- Basic rule nodes (One, All, Parallel): 6 hours
- Spherical XML loader: 4 hours
- **Subtotal for core functionality: ~14 hours**

- SphericalPathNode: 4 hours
- SphericalConvolutionNode: 4 hours
- SphericalWfcNode: 8+ hours (if needed)
- **Subtotal for advanced nodes: ~16 hours**

**Total for full Option B: ~30 hours** (similar to Option A)

**Advantages:**
- No risk of breaking existing Cartesian code
- Spherical nodes can be optimized for spherical topology
- Cleaner conceptual separation
- Can implement incrementally (core first, advanced later)

**Disadvantages:**
- Code duplication in branch nodes
- Two separate codepaths to maintain
- XML models need grid type indicator

---

#### 13.9.6 Comparison Summary

| Aspect | Option A: Generic Node<G> | Option B: Separate Implementation |
|--------|---------------------------|-----------------------------------|
| **Effort** | ~32 hours | ~30 hours |
| **Risk to existing code** | Medium | None |
| **Code duplication** | Low | Medium (branch nodes) |
| **Maintenance burden** | One codebase | Two codebases |
| **Conceptual clarity** | Mixed (some nodes non-generic) | Clear separation |
| **Performance** | Possible overhead from generics | Direct implementation |
| **Incremental delivery** | Hard (all-or-nothing for each node) | Easy (can ship core first) |
| **XML compatibility** | Same format, detect grid type | Need grid type indicator |

---

#### 13.9.7 Recommendation

**Short-term (next milestone)**: Use **Option B** for core functionality:
1. Build `SphericalInterpreter` + `SphericalNode` trait
2. Implement SphericalOneNode, SphericalAllNode, SphericalMarkovNode
3. Build spherical XML loader
4. Ship working spherical models with basic nodes

**Medium-term (future milestone)**: Evaluate whether to:
- Continue Option B with more spherical nodes
- Refactor to Option A now that spherical requirements are clearer
- Hybrid: Use Option A for simple nodes, Option B for complex nodes

**Rationale**: Option B has lower risk and allows incremental delivery. We can
ship working spherical models faster. If we later discover strong overlap between
Cartesian and Spherical logic, we can refactor to Option A.

---

#### 13.9.8 Work Completed (Phase 10a)

- [x] Added `clear()` default implementation to `MjGridOps` trait
- [x] Added `center_index()` to `MjGridOps` trait (required method)
- [x] Implemented `center_index()` for `MjGrid` (Cartesian)
- [x] Implemented `center_index()` for `SphericalMjGrid` (Spherical)
- [x] Added tests for both implementations
- [x] 14 grid_ops tests passing
- [x] 11 spherical mjgridops tests passing

---

## 14. Phase 11: Spherical Rendering

### 14.1 Goal

Implement `Renderable2D` for `SphericalMjGrid` to render polar grids to images.

### 14.2 Implementation

Port from `polar_grid.rs`:

```rust
impl Renderable2D for SphericalMjGrid {
    fn render_to_image(&self, palette: &[Color]) -> Image {
        // Convert polar to Cartesian for rendering
        let (cartesian, width, height) = self.to_cartesian_image();
        
        let mut image = Image::new(width, height);
        for (idx, &value) in cartesian.iter().enumerate() {
            let x = idx % width;
            let y = idx / width;
            image.set_pixel(x, y, palette[value as usize]);
        }
        image
    }
}
```

### 14.3 Tests to Migrate (4 tests) - DONE

- [x] `test_polar_to_cartesian`
- [x] `test_cartesian_quarter_circle`
- [x] `test_total_voxels`
- [x] `test_count_nonzero`
- [x] `test_recordable_grid_trait` (NEW)

### 14.4 Verification - DONE

```bash
cargo test -p studio_core markov_junior::spherical_grid::tests::level_6_rendering
# Result: 5 tests passed
```

### 14.5 MP4/Video Verification (PENDING)

The rendering pipeline is already complete in `recording/video.rs`:
- `VideoExporter::render_polar_2d()` renders frames from `GridType::Polar2D`
- `SphericalMjGrid` implements `RecordableGrid` returning `GridType::Polar2D`

**To verify rendering works end-to-end, regenerate all polar model MP4s:**

```bash
# Run integration tests that generate MP4s
cargo test -p studio_core markov_junior::spherical_grid::tests::level_7_integration --release -- --nocapture

# Or run individually:
cargo test test_spherical_ring_growth_video --release -- --nocapture
cargo test test_spherical_geological_layers_video --release -- --nocapture
```

**Tests to add to `spherical_grid.rs`:**

```rust
// Level 7: Integration tests with video export
mod level_7_integration {
    use super::*;
    use crate::markov_junior::recording::{SimulationRecorder, VideoExporter};

    fn output_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .parent().unwrap()
            .join("screenshots/spherical")
    }

    #[test]
    fn test_spherical_ring_growth_video() {
        // Port test_model_ring_growth from polar_grid.rs
        // 1. Create SphericalMjGrid
        // 2. Define rules using SphericalRule
        // 3. Run model, record frames with SimulationRecorder
        // 4. Export to screenshots/spherical/ring_growth.mp4
    }

    #[test]
    fn test_spherical_geological_layers_video() {
        // Port test_model_geological_layers from polar_grid.rs
        // 1. Create SphericalMjGrid with BMSDG palette
        // 2. Define geological layering rules
        // 3. Run model, record frames
        // 4. Export to screenshots/spherical/geological_layers.mp4
    }

    #[test]
    fn test_run_all_spherical_models() {
        // Master test that runs all polar models and generates summary
    }
}
```

**Output files to verify:**
```
screenshots/spherical/
├── ring_growth.mp4
├── ring_growth.mjsim
├── ring_growth.png
├── geological_layers.mp4
├── geological_layers.mjsim
├── geological_layers.png
├── angular_spread.mp4
├── wave_pattern.png
├── checkerboard.png
├── spiral.png
└── voronoi.png
```

**Visual verification checklist:**
- [ ] ring_growth.mp4 - Concentric rings filling outward from center
- [ ] geological_layers.mp4 - Layered rings: magma → stone → dirt → grass
- [ ] All PNGs render as polar/circular images (not rectangles)

---

## 15. Phase 12: Cleanup and Benchmarks

### 15.1 Delete PolarMjGrid

Once all 42 tests pass with `SphericalMjGrid`:

1. Verify no code depends on `PolarMjGrid`
2. Delete `polar_grid.rs`
3. Update module exports

```bash
# Find any remaining references
grep -r "PolarMjGrid\|polar_grid" crates/studio_core/src/

# Delete if clean
rm crates/studio_core/src/markov_junior/polar_grid.rs
```

### 15.2 Performance Benchmarks

Create benchmarks to verify no regression:

```rust
// benches/grid_bench.rs
#[bench]
fn bench_spherical_neighbor_lookup(b: &mut Bencher) {
    let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");
    b.iter(|| {
        for idx in 0..grid.len() {
            black_box(grid.neighbors_at(idx));
        }
    });
}

#[bench]
fn bench_spherical_pattern_match(b: &mut Bencher) {
    let grid = SphericalMjGrid::new_polar(256, 256, 1.0, "BW");
    let pattern = SphericalPattern::center_only(0);
    b.iter(|| {
        for idx in 0..grid.len() {
            black_box(pattern.matches(&grid, idx));
        }
    });
}
```

### 15.3 Verification

```bash
# All spherical tests (should be 42+)
cargo test -p studio_core markov_junior::spherical_grid

# No polar_grid tests (module deleted)
cargo test -p studio_core markov_junior::polar_grid
# Expected: 0 tests

# Benchmarks
cargo bench -p studio_core -- spherical
```

---

## 16. Verification Checkpoints Summary (Updated)

| Phase | Tests | Command | Expected | Status |
|-------|-------|---------|----------|--------|
| 0 | Baseline | `cargo test markov_junior` | 388 pass | DONE |
| 1 | Compile | `cargo build` | No errors | DONE |
| 2 | Trait impl | `cargo test grid_ops` | 11 pass | DONE |
| 3 | Context | `cargo test node` | 8 pass | DONE |
| 4 | Match format | `cargo test rule_node one_node all_node` | 13 pass | DONE |
| 5 | Full suite | `cargo test markov_junior` | 388 pass | DONE |
| 6 | Spherical storage | `cargo test spherical_grid` | 12 pass | DONE |
| 7 | Test migration | `cargo test spherical_grid` | 47 pass | DONE |
| 8 | Symmetries | `cargo test spherical_grid::symmetry` | 7 pass | DONE |
| 9 | Rules | `cargo test spherical_grid::rules` | 5 pass | DONE |
| 10a | MjGridOps extend | `cargo test grid_ops` | +2 pass | PENDING |
| 10b | Generic Interpreter | `cargo test interpreter verification` | 388 pass | PENDING |
| 10c | Generic LoadedModel | `cargo test loader` | 43 pass | PENDING |
| 10d | Spherical loading | `cargo test loader::spherical` | +4 pass | PENDING |
| 11 | Rendering | `cargo test spherical_grid::render` | 5 pass | DONE |
| 12 | Cleanup | `cargo test markov_junior` | 390+ pass | PENDING |

---

## 17. Timeline Estimate (Updated)

| Phase | Effort | Risk | Status |
|-------|--------|------|--------|
| Phase 0-5 | - | - | DONE |
| Phase 6 | - | - | DONE |
| Phase 7 | 2-3 hours | Low | DONE (47 tests) |
| Phase 8 | 1-2 hours | Low | DONE (included in Phase 7) |
| Phase 9 | 3-4 hours | Medium | DONE (included in Phase 7) |
| Phase 10a | 30 min | Low | **DONE** (clear(), center_index() added) |
| Phase 10b | ~32 hours | Medium | **ANALYZED** (see §13.9 for detailed breakdown) |
| Phase 10c | 30 min | Low | DEFERRED (depends on 10b approach) |
| Phase 10d | 4-14 hours | Medium | DEFERRED (depends on 10b approach) |
| Phase 11 | 1-2 hours | Low | DONE |
| Phase 12 | 1 hour | Low | PENDING (manual approval) |

**Remaining Work for Spherical XML Loading:**

Two approaches analyzed in §13.9:

| Approach | Effort | Risk | Recommended? |
|----------|--------|------|--------------|
| **Option A**: Generic `Node<G>` | ~32 hours | Medium | Long-term |
| **Option B**: Separate `SphericalNode` | ~30 hours | Low | **Short-term** |

**Recommendation**: Use Option B for initial delivery (lower risk, incremental).
Core spherical functionality (One/All/Markov nodes) can ship in ~14 hours.
Advanced nodes (WFC, Path, Convolution) can follow incrementally.

See §13.9 for detailed line-level analysis and node-by-node breakdown.

---

## 18. Definition of Done (Updated)

**Core Abstraction (COMPLETE)**:
- [x] All 388 MarkovJunior tests pass
- [x] Parity verification matches C# output
- [x] MjGridOps trait defined and implemented
- [x] ExecutionContext generic over grid type
- [x] Match/Changes use flat indices

**Polar/Spherical Unification (MOSTLY COMPLETE)**:
- [x] All 42+ polar tests pass with SphericalMjGrid (50 tests!)
- [x] SphericalSymmetry (4-group) implemented
- [x] SphericalPattern/SphericalRule implemented
- [x] `MjGridOps` extended with `clear()` and `center_index()`
- [x] Rendering works for polar grids (render_to_image, save_png)
- [x] RecordableGrid/Renderable2D traits implemented
- [ ] PolarMjGrid deleted (awaiting manual approval)
- [ ] Performance benchmarks show no regression >5%

**Spherical Node Execution (ANALYZED, NOT STARTED)**:
- [ ] Spherical node execution - Two approaches analyzed in §13.9:
  - Option A: Generic `Node<G>` (~32 hours, medium risk)
  - Option B: Separate `SphericalNode` (~30 hours, low risk) **← Recommended**
- [ ] XML loading supports spherical grids (depends on above)

**Final**:
- [ ] Documentation updated
- [ ] Code reviewed and merged

---

## 19. Commit History

| Commit | Phase | Description |
|--------|-------|-------------|
| 8ef8cc3 | 1-3 | MjGridOps trait, MjGrid impl, ExecutionContext generic |
| a6a78fc | 4-5 | Match/Changes format migration, full integration |
| bfa96d6 | 6 | SphericalMjGrid with MjGridOps |
| 32fae6c | - | Documentation update |
| e6d8481 | - | Complete rewrite of implementation plan |
| 8fde594 | 7-9 | Port 44 polar tests, symmetries, patterns, rules |
| 4433d71 | 11 | Rendering, RecordableGrid, Renderable2D traits |
