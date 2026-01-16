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

## 13. Phase 10: Spherical XML Loading

### 13.1 Goal

Enable loading polar/spherical models from XML, with the same syntax as Cartesian models but with polar-specific attributes.

### 13.2 XML Format

```xml
<!-- Polar model definition -->
<model name="PolarRings" type="polar">
  <!-- Grid definition -->
  <grid r_min="256" r_depth="64" target_arc="1.0" values="BWR"/>
  
  <!-- Rules work the same as Cartesian, but use polar symmetries -->
  <rule in="B" out="W" symmetry="polar"/>
  
  <!-- Patterns can specify polar neighbors -->
  <rule>
    <input>
      <pattern center="B" r_plus="W"/>
    </input>
    <output>R</output>
  </rule>
</model>
```

### 13.3 Implementation

Extend `loader.rs` to handle polar grids:

```rust
// In loader.rs
fn load_grid(elem: &Element) -> Result<Box<dyn MjGridOps>, LoadError> {
    if elem.has_attribute("r_min") || elem.has_attribute("r_depth") {
        // Polar/Spherical grid
        let r_min = elem.get_attr("r_min")?.parse()?;
        let r_depth = elem.get_attr("r_depth")?.parse()?;
        let target_arc = elem.get_attr("target_arc").unwrap_or("1.0").parse()?;
        let values = elem.get_attr("values")?;
        
        if elem.has_attribute("phi_divisions") {
            // 3D spherical
            let phi_divisions = elem.get_attr("phi_divisions")?.parse()?;
            Ok(Box::new(SphericalMjGrid::new_spherical(...)))
        } else {
            // 2D polar
            Ok(Box::new(SphericalMjGrid::new_polar(r_min, r_depth, target_arc, values)))
        }
    } else {
        // Cartesian grid (existing code)
        let mx = elem.get_attr("mx")?.parse()?;
        let my = elem.get_attr("my")?.parse()?;
        let mz = elem.get_attr("mz").unwrap_or("1").parse()?;
        Ok(Box::new(MjGrid::with_values(mx, my, mz, values)))
    }
}
```

### 13.4 Symmetry Loading

Extend symmetry parsing to handle polar symmetries:

```rust
fn parse_symmetry(s: &str, is_polar: bool) -> Vec<Box<dyn Symmetry>> {
    if is_polar {
        match s {
            "polar" | "all" => SphericalSymmetry::all_2d().to_vec(),
            "identity" => vec![SphericalSymmetry::Identity],
            "theta" => vec![SphericalSymmetry::Identity, SphericalSymmetry::ThetaFlip],
            "r" => vec![SphericalSymmetry::Identity, SphericalSymmetry::RFlip],
            _ => vec![SphericalSymmetry::Identity],
        }
    } else {
        // Existing Cartesian symmetry parsing
    }
}
```

### 13.5 Tests

- [ ] `test_load_polar_grid_from_xml`
- [ ] `test_load_polar_rule_from_xml`
- [ ] `test_load_polar_model_from_xml`
- [ ] `test_polar_symmetry_parsing`

### 13.6 Verification

```bash
cargo test -p studio_core markov_junior::loader::polar
```

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
| 7 | Test migration | `cargo test spherical_grid` | 42+ pass | PENDING |
| 8 | Symmetries | `cargo test spherical_grid::symmetry` | 7 pass | PENDING |
| 9 | Rules | `cargo test spherical_grid::rules` | 5 pass | PENDING |
| 10 | XML loading | `cargo test loader::polar` | 4 pass | PENDING |
| 11 | Rendering | `cargo test spherical_grid::render` | 4 pass | PENDING |
| 12 | Cleanup | `cargo test markov_junior` | 388+ pass | PENDING |

---

## 17. Timeline Estimate (Updated)

| Phase | Effort | Risk | Status |
|-------|--------|------|--------|
| Phase 0-5 | - | - | DONE |
| Phase 6 | - | - | DONE |
| Phase 7 | 2-3 hours | Low | DONE (45 tests) |
| Phase 8 | 1-2 hours | Low | DONE (included in Phase 7) |
| Phase 9 | 3-4 hours | Medium | DONE (included in Phase 7) |
| Phase 10 | 2-3 hours | Medium | PENDING (XML loading) |
| Phase 11 | 1-2 hours | Low | DONE |
| Phase 12 | 1 hour | Low | PENDING (manual approval) |

**Remaining**: ~3-4 hours (XML loading + deletion approval)

---

## 18. Definition of Done (Updated)

**Core Abstraction (COMPLETE)**:
- [x] All 388 MarkovJunior tests pass
- [x] Parity verification matches C# output
- [x] MjGridOps trait defined and implemented
- [x] ExecutionContext generic over grid type
- [x] Match/Changes use flat indices

**Polar/Spherical Unification (MOSTLY COMPLETE)**:
- [x] All 42+ polar tests pass with SphericalMjGrid (45 tests!)
- [x] SphericalSymmetry (4-group) implemented
- [x] SphericalPattern/SphericalRule implemented
- [ ] XML loading supports polar grids
- [x] Rendering works for polar grids (render_to_image, save_png)
- [x] RecordableGrid/Renderable2D traits implemented
- [ ] PolarMjGrid deleted (awaiting manual approval)
- [ ] Performance benchmarks show no regression >5%

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
