# Markov Jr: Polar and Spherical Coordinate Extension

## Overview

This document describes extending Markov Jr to operate natively in polar (2D) and spherical (3D) coordinate systems. The goal is to generate procedural content on circular and spherical worlds without the distortion artifacts that come from projecting rectangular grids onto curved surfaces.

**Key insight**: By choosing a minimum radius (`r_min`) such that adjacent rings have nearly identical voxel counts, we achieve <1% distortion. At this distortion level, the neighbor relationships are effectively rectangular, and Markov Jr's pattern matching works with minimal modification.

---

## Part 1: The Distortion Problem

### Why Not Just Map a Rectangle to a Sphere?

The naive approach: generate terrain on a rectangular Markov Jr grid, then wrap it around a sphere.

Problems:
1. **Pole compression**: The top and bottom rows of the rectangle map to single points (poles). Massive distortion.
2. **Seam artifacts**: Left and right edges must match perfectly for θ wraparound.
3. **Non-uniform voxel size**: Voxels near poles are tiny; voxels at equator are large.
4. **Neighbor mismatch**: A voxel's "up" neighbor in the rectangle isn't its "up" neighbor on the sphere.

### The Solution: Native Polar/Spherical Grids

Instead of generating on a rectangle and mapping, we define the Markov Jr grid directly in polar/spherical coordinates with:
1. Fixed voxel arc-length (2D) or surface area (3D)
2. Variable θ subdivisions per radius/latitude
3. An unbuildable core region to avoid center singularity

---

## Part 2: 2D Polar Coordinates

### Coordinate System

A 2D polar voxel is addressed by `(r, θ)`:
- `r`: radius index (integer)
- `θ`: angular index (integer, but range depends on r)

### Fixed Arc-Length Voxels

We want every voxel to have the same arc length `a`.

At radius `r`, the circumference is `2πr`. To get arc length `a`:

```
θ_divisions(r) = floor(2πr / a)
```

A voxel at `(r, θ)` spans the angular range:
```
[θ / θ_divisions(r) × 2π, (θ+1) / θ_divisions(r) × 2π]
```

### The Distortion Metric

Moving from radius `r` to `r+1`, the ratio of θ divisions is:

```
ratio = θ_divisions(r+1) / θ_divisions(r) ≈ (r+1) / r
```

This ratio determines how many outer voxels correspond to one inner voxel:
- ratio = 1.0: exactly 1:1 mapping (no distortion)
- ratio = 1.5: some voxels map to 1 neighbor, some to 2
- ratio = 2.0: every voxel maps to 2 neighbors

**Distortion = ratio - 1**

| r_min | Max distortion at inner edge |
|-------|------------------------------|
| 10 | 10% |
| 50 | 2% |
| 100 | 1% |
| 256 | 0.39% |
| 512 | 0.20% |

### Recommended Configuration (2D)

```
r_min = 256
r_max = r_min + 255 = 511
Buildable depth = 256 levels (addressable with 1 byte)
Max distortion = 0.39%
```

At this distortion level, the vast majority of voxels have exactly 1 neighbor in each radial direction. The occasional 1:2 mapping can be handled as an edge case.

### 2D Voxel Counts

| r_min | r_max | Depth | Circumference at r_max | Total voxels |
|-------|-------|-------|------------------------|--------------|
| 256 | 511 | 256 | ~3,210 | ~1.5 million |
| 512 | 767 | 256 | ~4,820 | ~3.4 million |

Calculation for r_min=256:
```
Total = Σ θ_divisions(r) for r = 256 to 511
      = Σ 2πr for r = 256 to 511
      ≈ 2π × Σr
      ≈ 2π × (256 + 257 + ... + 511)
      ≈ 2π × 98,048
      ≈ 616,000 voxels per "wrap"
      
With depth 256: ~1.5 million total
```

### 2D Neighbor Relationships

```rust
fn neighbors_2d(r: u8, theta: u16) -> Neighbors2D {
    let r_actual = R_MIN + r as u32;
    let theta_divs = theta_divisions(r_actual);
    
    // Angular neighbors (always exactly 2)
    let theta_minus = (theta + theta_divs - 1) % theta_divs;
    let theta_plus = (theta + 1) % theta_divs;
    
    // Radial neighbors (almost always 1 each, occasionally 2)
    let inner = radial_neighbors_inner(r_actual, theta);
    let outer = radial_neighbors_outer(r_actual, theta);
    
    Neighbors2D {
        angular: [(r, theta_minus), (r, theta_plus)],
        inner,  // Vec with 1-2 elements
        outer,  // Vec with 1-2 elements
    }
}

fn radial_neighbors_inner(r: u32, theta: u16) -> Vec<(u8, u16)> {
    if r == R_MIN { return vec![]; }  // At inner boundary
    
    let current_divs = theta_divisions(r);
    let inner_divs = theta_divisions(r - 1);
    
    // Angular range of this voxel as fraction of circle
    let start = theta as f32 / current_divs as f32;
    let end = (theta + 1) as f32 / current_divs as f32;
    
    // Which inner voxels overlap this range?
    let first = (start * inner_divs as f32).floor() as u16;
    let last = ((end * inner_divs as f32).ceil() as u16).saturating_sub(1);
    
    (first..=last).map(|t| (r - 1, t % inner_divs)).collect()
}
```

At 0.39% distortion, `radial_neighbors_inner` returns exactly 1 voxel ~99.6% of the time.

### 2D Symmetries

In Cartesian 2D, patterns have 8 symmetries (D4 group): 4 rotations × 2 reflections.

In polar 2D, the dimensions are fundamentally different:
- **θ (angular)**: cyclic, wraps around
- **r (radial)**: linear, has direction (inward vs outward)

Valid polar symmetries (Klein four-group, 4 elements):

| Symmetry | Transform | Meaning |
|----------|-----------|---------|
| Identity | (r, θ) → (r, θ) | No change |
| θ-flip | (r, θ) → (r, -θ) | Mirror across radial line |
| r-flip | (r, θ) → (-r, θ) | Swap inner ↔ outer |
| Both | (r, θ) → (-r, -θ) | Both flips |

**90° rotation does not exist** in polar coordinates because θ and r are not interchangeable. You cannot rotate a pattern so that "inward" becomes "clockwise."

This is actually useful: radial direction is meaningful (surface vs depth), so patterns *should* distinguish between inward and outward.

### 2D Pattern Representation

Cartesian pattern (3×3):
```
[NW][N ][NE]
[W ][C ][E ]
[SW][S ][SE]
```

Polar pattern (3×3 equivalent):
```
     [outer]
[θ-1][center][θ+1]
     [inner]
```

Or with the occasional 1:2 mapping:
```
   [outer_a][outer_b]
[θ-1][center][θ+1]
      [inner]
```

For pattern matching, we handle the 1:2 case by:
1. Treating the 2 outer voxels as a single logical neighbor (majority vote or "any match")
2. Or defining separate rules for boundary cases (rare enough to be negligible)

---

## Part 3: 3D Spherical Coordinates

### Coordinate System

A 3D spherical voxel is addressed by `(r, θ, φ)`:
- `r`: radius index (integer, distance from center)
- `θ`: azimuth/longitude index (integer, around the equator)
- `φ`: elevation/latitude index (integer, pole to pole)

### Fixed Surface-Area Voxels

We want every voxel to have approximately the same surface area.

The surface area element on a sphere:
```
dA = r² × sin(φ) × dθ × dφ
```

The `sin(φ)` term causes voxels near poles to shrink. To compensate, we vary θ subdivisions by latitude:

```
θ_divisions(r, φ) = floor(2πr × sin(φ) / target_arc)
```

At the equator (φ = 90°, sin = 1): maximum θ divisions
At the poles (φ = 0° or 180°, sin = 0): θ divisions collapse to 1

### Polar Cap Exclusion

To avoid the pole singularity, we exclude polar caps:

```
φ_min = 10°  (north polar cap)
φ_max = 170° (south polar cap)
```

At φ = 10°, sin(10°) ≈ 0.17, so there are still ~17% as many θ divisions as the equator. This is the region of highest latitudinal distortion (~10%).

### Recommended Configuration (3D)

```
r_min = 256
r_max = r_min + 255 = 511
Buildable depth = 256 levels (1 byte)

φ_min = 10°
φ_max = 170°  
φ_divisions = 160 bands (1° per band, 1 byte addressable)

Max radial distortion = 0.39%
Max latitudinal distortion = ~10% at polar cap edges
```

### 3D Voxel Counts

For a spherical shell from r=256 to r=511, excluding 10° polar caps:

```
Surface area of shell ≈ 4π × r² × latitude_fraction

Total voxels ≈ Σ (4π × r² × 0.94) for r = 256 to 511
            ≈ 4π × 0.94 × Σ(r²)
            ≈ 4π × 0.94 × 39,180,166
            ≈ 460 million voxels
```

| r_min | r_max | Depth | Max distortion | Total voxels |
|-------|-------|-------|----------------|--------------|
| 256 | 511 | 256 | 0.39% radial, 10% at caps | ~460 million |
| 512 | 767 | 256 | 0.20% radial, 10% at caps | ~1.5 billion |

### 3D Addressing Scheme

```rust
struct SphericalCoord {
    r: u8,      // 0-255, actual radius = r_min + r
    phi: u8,    // 0-159, latitude band (10° to 170°)
    theta: u16, // 0 to θ_divisions(r, φ)-1, max ~3,210
}

impl SphericalCoord {
    fn theta_divisions(&self) -> u16 {
        let r_actual = R_MIN + self.r as u32;
        let phi_degrees = PHI_MIN + self.phi as f32;
        let phi_radians = phi_degrees.to_radians();
        
        let circumference = 2.0 * PI * r_actual as f32 * phi_radians.sin();
        (circumference / TARGET_ARC).round().max(1.0) as u16
    }
}
```

### 3D Neighbor Relationships

Each voxel has 6 neighbor directions:
- **θ±**: angular neighbors (always exactly 2)
- **φ±**: latitude neighbors (1-2 each due to varying θ divisions)
- **r±**: radial neighbors (1-2 each due to varying θ divisions)

```rust
fn neighbors_3d(r: u8, theta: u16, phi: u8) -> Neighbors3D {
    let coord = SphericalCoord { r, theta, phi };
    let theta_divs = coord.theta_divisions();
    
    Neighbors3D {
        // Angular (always exactly 2)
        theta_minus: (r, (theta + theta_divs - 1) % theta_divs, phi),
        theta_plus: (r, (theta + 1) % theta_divs, phi),
        
        // Latitude (1-2 each, depending on θ division ratio)
        phi_minus: latitude_neighbors(r, theta, phi, -1),
        phi_plus: latitude_neighbors(r, theta, phi, +1),
        
        // Radial (1-2 each, depending on θ division ratio)
        r_minus: radial_neighbors(r, theta, phi, -1),
        r_plus: radial_neighbors(r, theta, phi, +1),
    }
}
```

### 3D Symmetries

In Cartesian 3D, patterns have 48 symmetries (octahedral group): rotations and reflections of a cube.

In spherical 3D, the dimensions are:
- **θ (longitude)**: cyclic, wraps
- **φ (latitude)**: bounded, does not wrap, has direction (toward poles)
- **r (radius)**: bounded, does not wrap, has direction (inward/outward)

Valid spherical symmetries:

| Symmetry | Transform | Meaning |
|----------|-----------|---------|
| Identity | (r, θ, φ) → (r, θ, φ) | No change |
| θ-flip | (r, θ, φ) → (r, -θ, φ) | Mirror across meridian |
| φ-flip | (r, θ, φ) → (r, θ, -φ) | Mirror across equator |
| r-flip | (r, θ, φ) → (-r, θ, φ) | Swap inner ↔ outer |
| θ+φ flip | (r, θ, φ) → (r, -θ, -φ) | Both angular flips |
| θ+r flip | (r, θ, φ) → (-r, -θ, φ) | |
| φ+r flip | (r, θ, φ) → (-r, θ, -φ) | |
| All three | (r, θ, φ) → (-r, -θ, -φ) | All flips |

**8 symmetries total** (Z₂ × Z₂ × Z₂), compared to 48 in Cartesian.

Again, this is actually useful: "up" (outward), "north" (toward pole), and "east" (along rotation) are meaningfully different directions on a planet.

---

## Part 4: Implementation Plan

### Phase 1: 2D Polar Markov Jr

**Goal**: Prove the concept works in 2D before tackling 3D.

#### 1.1 Data Structures

```rust
/// 2D polar grid for Markov Jr
pub struct PolarMjGrid {
    pub r_min: u32,
    pub r_depth: u8,  // Number of radial levels (e.g., 256)
    pub target_arc: f32,
    
    // Storage: Vec of rings, each ring has variable length
    rings: Vec<Vec<u8>>,  // rings[r][theta] = cell value
}

impl PolarMjGrid {
    pub fn new(r_min: u32, r_depth: u8, target_arc: f32) -> Self {
        let mut rings = Vec::with_capacity(r_depth as usize);
        for dr in 0..r_depth {
            let r = r_min + dr as u32;
            let theta_count = Self::theta_divisions_for_r(r, target_arc);
            rings.push(vec![0u8; theta_count as usize]);
        }
        Self { r_min, r_depth, target_arc, rings }
    }
    
    pub fn theta_divisions(&self, r_index: u8) -> u16 {
        self.rings[r_index as usize].len() as u16
    }
    
    pub fn get(&self, r: u8, theta: u16) -> u8 {
        let ring = &self.rings[r as usize];
        ring[(theta as usize) % ring.len()]
    }
    
    pub fn set(&mut self, r: u8, theta: u16, value: u8) {
        let ring = &mut self.rings[r as usize];
        let idx = (theta as usize) % ring.len();
        ring[idx] = value;
    }
}
```

#### 1.2 Neighbor Function

```rust
#[derive(Debug)]
pub struct PolarNeighbors {
    pub theta_minus: (u8, u16),
    pub theta_plus: (u8, u16),
    pub r_minus: Vec<(u8, u16)>,  // 0-2 elements
    pub r_plus: Vec<(u8, u16)>,   // 0-2 elements
}

impl PolarMjGrid {
    pub fn neighbors(&self, r: u8, theta: u16) -> PolarNeighbors {
        let theta_divs = self.theta_divisions(r);
        
        PolarNeighbors {
            theta_minus: (r, (theta + theta_divs - 1) % theta_divs),
            theta_plus: (r, (theta + 1) % theta_divs),
            r_minus: self.radial_neighbors(r, theta, -1),
            r_plus: self.radial_neighbors(r, theta, 1),
        }
    }
    
    fn radial_neighbors(&self, r: u8, theta: u16, dr: i8) -> Vec<(u8, u16)> {
        let new_r = r as i16 + dr as i16;
        if new_r < 0 || new_r >= self.r_depth as i16 {
            return vec![];
        }
        let new_r = new_r as u8;
        
        let current_divs = self.theta_divisions(r) as f32;
        let new_divs = self.theta_divisions(new_r) as f32;
        
        // Angular range of current voxel
        let start = theta as f32 / current_divs;
        let end = (theta + 1) as f32 / current_divs;
        
        // Which voxels at new_r overlap?
        let first = (start * new_divs).floor() as u16;
        let last = ((end * new_divs).ceil() as u16).saturating_sub(1);
        
        (first..=last)
            .map(|t| (new_r, t % self.theta_divisions(new_r)))
            .collect()
    }
}
```

#### 1.3 Symmetry Transform

```rust
#[derive(Clone, Copy)]
pub enum PolarSymmetry {
    Identity,
    ThetaFlip,
    RFlip,
    BothFlip,
}

impl PolarSymmetry {
    pub fn all() -> [Self; 4] {
        [Self::Identity, Self::ThetaFlip, Self::RFlip, Self::BothFlip]
    }
    
    /// Transform a relative offset (dr, dtheta) by this symmetry
    pub fn transform(&self, dr: i8, dtheta: i8) -> (i8, i8) {
        match self {
            Self::Identity => (dr, dtheta),
            Self::ThetaFlip => (dr, -dtheta),
            Self::RFlip => (-dr, dtheta),
            Self::BothFlip => (-dr, -dtheta),
        }
    }
}
```

#### 1.4 Pattern Matching (Modified)

The core change: instead of assuming exactly 4 neighbors, we query the neighbor function and handle variable counts.

```rust
pub struct PolarPattern {
    // Center value
    pub center: u8,
    // Neighbor requirements: None = wildcard, Some(v) = must match v
    pub theta_minus: Option<u8>,
    pub theta_plus: Option<u8>,
    pub r_minus: Option<u8>,  // If multiple r_minus neighbors, all must match
    pub r_plus: Option<u8>,
}

impl PolarPattern {
    pub fn matches(&self, grid: &PolarMjGrid, r: u8, theta: u16) -> bool {
        if grid.get(r, theta) != self.center {
            return false;
        }
        
        let neighbors = grid.neighbors(r, theta);
        
        // Check theta neighbors (always exactly 1 each)
        if let Some(v) = self.theta_minus {
            if grid.get(neighbors.theta_minus.0, neighbors.theta_minus.1) != v {
                return false;
            }
        }
        if let Some(v) = self.theta_plus {
            if grid.get(neighbors.theta_plus.0, neighbors.theta_plus.1) != v {
                return false;
            }
        }
        
        // Check radial neighbors (1-2 each)
        if let Some(v) = self.r_minus {
            if !neighbors.r_minus.iter().all(|&(nr, nt)| grid.get(nr, nt) == v) {
                return false;
            }
        }
        if let Some(v) = self.r_plus {
            if !neighbors.r_plus.iter().all(|&(nr, nt)| grid.get(nr, nt) == v) {
                return false;
            }
        }
        
        true
    }
}
```

#### 1.5 Test Models (2D)

Models to verify the 2D implementation:

1. **Ring growth**: Start with a seed at one θ position, grow around the ring
   ```
   Rule: B next to W → W (spread along θ)
   Expected: Ring fills with W over time
   ```

2. **Radial gradient**: Different values at different r
   ```
   Rule: At r_max boundary, set to "surface" value
   Rule: Below surface, set to "underground" value
   Expected: Concentric rings of different materials
   ```

3. **Maze generation**: Polar maze with corridors
   ```
   Standard maze rules, adapted to polar neighbors
   Expected: Maze that wraps around θ, bounded by r
   ```

4. **Wave propagation**: Circular waves from a point
   ```
   Rule: Active cell activates neighbors
   Expected: Waves spread outward in concentric circles
   ```

### Phase 2: 3D Spherical Markov Jr

#### 2.1 Data Structures

```rust
pub struct SphericalMjGrid {
    pub r_min: u32,
    pub r_depth: u8,
    pub phi_min: u8,   // Degrees, e.g., 10
    pub phi_max: u8,   // Degrees, e.g., 170
    pub target_arc: f32,
    
    // Storage: shells[r][phi][theta]
    // But theta count varies by (r, phi), so:
    shells: Vec<Vec<Vec<u8>>>,  // shells[r][phi] = vec of theta values
}

impl SphericalMjGrid {
    pub fn theta_divisions(&self, r: u8, phi: u8) -> u16 {
        let r_actual = self.r_min + r as u32;
        let phi_deg = self.phi_min + phi;
        let phi_rad = (phi_deg as f32).to_radians();
        
        let circumference = 2.0 * PI * r_actual as f32 * phi_rad.sin();
        (circumference / self.target_arc).round().max(1.0) as u16
    }
    
    pub fn get(&self, r: u8, theta: u16, phi: u8) -> u8 {
        let band = &self.shells[r as usize][phi as usize];
        band[(theta as usize) % band.len()]
    }
}
```

#### 2.2 3D Neighbor Function

```rust
pub struct SphericalNeighbors {
    pub theta_minus: (u8, u16, u8),
    pub theta_plus: (u8, u16, u8),
    pub phi_minus: Vec<(u8, u16, u8)>,
    pub phi_plus: Vec<(u8, u16, u8)>,
    pub r_minus: Vec<(u8, u16, u8)>,
    pub r_plus: Vec<(u8, u16, u8)>,
}
```

The implementation follows the same overlap-calculation pattern as 2D, but applied to both φ and r transitions.

#### 2.3 Test Models (3D)

1. **Layered planet**: Different materials at different depths
   ```
   r = 0-64: core (hot)
   r = 64-192: mantle (rock)
   r = 192-255: crust (surface materials)
   ```

2. **Latitude biomes**: Different biomes at different φ
   ```
   φ near poles: ice
   φ near equator: tropical
   φ mid-latitudes: temperate
   ```

3. **Terrain generation**: Apply standard terrain rules on the surface shell
   ```
   Mountains, valleys, oceans generated at r_max
   Rules aware of latitude for biome variation
   ```

### Phase 3: Integration with Existing Codebase

#### 3.1 Files to Modify

| File | Changes |
|------|---------|
| `markov_junior/mod.rs` | Add `PolarMjGrid`, `SphericalMjGrid` |
| `markov_junior/field.rs` | Generalize field operations for polar grids |
| `markov_junior/rule.rs` | Add `PolarPattern`, symmetry transforms |
| `markov_junior/interpreter.rs` | Support polar grid in interpreter loop |
| `markov_junior/loader.rs` | New XML attributes for polar models |

#### 3.2 New Files

| File | Purpose |
|------|---------|
| `markov_junior/polar_grid.rs` | `PolarMjGrid` implementation |
| `markov_junior/spherical_grid.rs` | `SphericalMjGrid` implementation |
| `markov_junior/polar_symmetry.rs` | Polar/spherical symmetry groups |

#### 3.3 XML Model Format Extension

```xml
<!-- Existing Cartesian model -->
<model name="terrain" size="64 64 64">
  ...
</model>

<!-- New polar 2D model -->
<model name="ring_world" type="polar2d" r_min="256" r_depth="256" target_arc="1.0">
  ...
</model>

<!-- New spherical 3D model -->
<model name="planet" type="spherical" 
       r_min="256" r_depth="256" 
       phi_min="10" phi_max="170"
       target_arc="1.0">
  ...
</model>
```

---

## Part 5: Rendering Considerations

### The Question

Polar/spherical voxels are not cubes. How do we render them?

At low distortion (<1%), voxels are *nearly* cubes. Options:

### Option A: Render as Cubes Anyway

Transform polar coordinates to Cartesian for rendering:
```
x = r × cos(θ)
y = r × sin(θ)
```

Each voxel becomes a cube at that (x, y) position. At 0.39% distortion, the visual difference from "correct" polar wedges is imperceptible.

**Pros**: Reuse existing greedy meshing, no shader changes
**Cons**: Slight overlap/gaps at high zoom (likely invisible)

### Option B: Render as Polar Wedges

Each voxel is a wedge shape:
- Inner arc at r
- Outer arc at r+1
- Two radial edges at θ and θ+1

**Pros**: Geometrically correct
**Cons**: Cannot use greedy meshing (wedges don't tile into rectangles), more complex mesh generation

### Option C: Hybrid

- Use cube rendering for the bulk of voxels
- Use wedge rendering only at visible surfaces where distortion matters
- Or: render cubes but apply a vertex shader that "bends" them into wedges

### Recommendation

**Start with Option A** (render as cubes). At 0.39% distortion, the error is:
- At r=256: cube vs wedge differs by ~0.4% of voxel size
- At r=511: cube vs wedge differs by ~0.2% of voxel size

This is sub-pixel at any reasonable view distance. If visible artifacts appear, revisit with Option C.

### Greedy Meshing Implications

Greedy meshing assumes rectangular slices along cardinal axes. In polar:

- **θ direction**: Voxels are uniformly spaced along θ—greedy meshing works
- **r direction**: Voxels are uniformly spaced along r—greedy meshing works
- **Cross-ring boundaries**: Where θ_divisions changes, faces don't align perfectly

For 2D (or surface shell in 3D), greedy meshing along θ works fine. Radial greedy meshing is limited to runs within a single r-level.

At 0.39% distortion, the number of "boundary" faces where greedy meshing breaks is <1% of total faces. Acceptable.

---

## Part 6: Summary

### What We're Building

A native polar/spherical coordinate system for Markov Jr that:
1. Maintains uniform voxel size across the grid
2. Has predictable neighbor relationships (almost always 1:1)
3. Supports fewer but meaningful symmetries
4. Can generate content for circular (2D) and spherical (3D) worlds

### Key Parameters

| Parameter | 2D | 3D |
|-----------|-----|-----|
| r_min | 256 | 256 |
| r_max | 511 | 511 |
| Depth | 256 (1 byte) | 256 (1 byte) |
| φ range | N/A | 10°-170° |
| Max distortion | 0.39% | 0.39% radial, 10% at caps |
| Symmetries | 4 | 8 |
| Total voxels | ~1.5M | ~460M |

### Implementation Order

1. **2D polar grid** with neighbor lookup
2. **2D pattern matching** with 4 symmetries
3. **2D test models** (ring growth, maze, waves)
4. **3D spherical grid** with neighbor lookup
5. **3D pattern matching** with 8 symmetries
6. **3D test models** (layered planet, biomes)
7. **Rendering integration** (start with cube approximation)
8. **Performance optimization** (if needed)

### Open Questions

1. How to handle polar caps in 3D? (Unbuildable? Low-res? Different topology?)
2. Should we support non-uniform r spacing? (Denser near surface)
3. How to blend polar-generated content with existing Cartesian chunks?
4. LOD strategy for large spherical worlds?
