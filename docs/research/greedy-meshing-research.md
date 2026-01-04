# Greedy Meshing Research

## Overview

Greedy meshing merges adjacent same-material faces into larger quads, reducing vertex count beyond what face culling achieves. While face culling eliminates hidden faces, greedy meshing optimizes visible surfaces.

## Performance Impact

| Scenario | Face Culling Only | + Greedy Meshing | Improvement |
|----------|-------------------|------------------|-------------|
| 8x8x8 solid cube | 384 quads | 6 quads | 64x |
| 16x16 flat surface | 256 quads | 1 quad | 256x |
| Noisy terrain | ~2200 quads | ~1670 quads | 1.3x |
| Detailed creature | ~1000 quads | ~800 quads | 1.25x |

**Key insight**: Greedy meshing is most effective for large uniform surfaces (floors, walls, ceilings) and less effective for detailed, multi-colored geometry.

## Bonsai's Approach

**Status**: Bonsai has greedy meshing code but it's **disabled** (`#if 0` in `world_chunk.cpp`).

**Active approach**: Fast bitwise face culling using 64-bit occupancy masks:
```c
// Bonsai's bitwise face culling
u64 RightFaces = (Bits) & ~(Bits>>1);  // Right neighbor empty
u64 LeftFaces  = (Bits) & ~(Bits<<1);  // Left neighbor empty
```

**Why disabled?**: Bonsai prioritizes frame-rate consistency over mesh optimization. The bitwise approach is faster to compute, even if it produces more vertices.

**Bonsai TODO note**: "Better greedy meshing? https://www.youtube.com/watch?v=4xs66m1Of4A"

## Mikola Lysenko's Algorithm

The standard greedy meshing algorithm from [0fps.net](https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/):

### Core Algorithm

```
For each of 6 face directions (+X, -X, +Y, -Y, +Z, -Z):
    For each slice perpendicular to that direction:
        1. Build 2D mask of visible faces with material IDs
        2. Greedy scan the mask:
           - For each unprocessed cell:
             a. Extend width (W) rightward while material matches
             b. Extend height (H) downward while entire W-wide row matches
             c. Emit quad of size W x H
             d. Clear processed cells from mask
```

### Pseudocode

```rust
fn greedy_mesh_slice(mask: &mut [[Option<MaterialKey>; SIZE]; SIZE]) -> Vec<Quad> {
    let mut quads = Vec::new();
    
    for v in 0..SIZE {
        let mut u = 0;
        while u < SIZE {
            if let Some(key) = mask[v][u] {
                // Compute width - extend right while material matches
                let mut w = 1;
                while u + w < SIZE && mask[v][u + w] == Some(key) {
                    w += 1;
                }
                
                // Compute height - extend down while entire row matches
                let mut h = 1;
                'height: while v + h < SIZE {
                    for k in 0..w {
                        if mask[v + h][u + k] != Some(key) {
                            break 'height;
                        }
                    }
                    h += 1;
                }
                
                // Emit quad
                quads.push(Quad { u, v, w, h, key });
                
                // Clear mask in merged region
                for dv in 0..h {
                    for du in 0..w {
                        mask[v + dv][u + du] = None;
                    }
                }
                
                u += w;
            } else {
                u += 1;
            }
        }
    }
    
    quads
}
```

### Complexity

- **Time**: O(n) where n = volume size
- **Space**: O(slice area) for the 2D mask
- **Mesh quality**: Within 8x of optimal (proven), conjectured within E/2 quads where E = perimeter edges

## The AO Problem

### Challenge

Our voxel mesh has **per-vertex ambient occlusion** (AO). Each of the 4 corners of a face can have different AO values based on neighboring voxels.

```
Face A corners: [1.0, 0.7, 0.7, 0.4]
Face B corners: [0.7, 1.0, 0.4, 0.7]
```

If we merge these faces, we'd need to somehow interpolate AO across the larger quad, but that's not possible with simple vertex attributes - the GPU interpolates linearly across the triangle, which would produce incorrect results.

### Why It Matters

Two adjacent faces typically have **different** AO values because:
1. They have different neighboring voxels
2. AO is computed per-vertex, not per-face
3. Shared edges have different corner neighbors

### Solutions Considered

| Solution | Pros | Cons |
|----------|------|------|
| 1. Ignore AO in merging | Maximum merging | Loses AO detail, visual artifacts |
| 2. Match exact AO | Preserves AO perfectly | Very limited merging |
| 3. Quantize AO (4 levels) | Good balance | Some AO precision loss |
| 4. Per-face average AO | Allows more merging | Loses smooth corner darkening |
| 5. AO texture atlas | Full merging + AO | Complex, memory overhead |

### Our Decision: **Option 3 - Quantize AO**

Quantize AO to 4 levels (0.0, 0.33, 0.66, 1.0) and include in merge key. This:
- Preserves most AO visual quality
- Allows significant merging for uniform surfaces
- Keeps implementation simple

## Implementation Plan

### Phase 1: Material-Only Greedy Meshing

First implementation ignores AO, merges based on color + emission only.

**Merge Key**:
```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FaceKey {
    color: [u8; 3],    // RGB
    emission: u8,
}

impl FaceKey {
    fn to_u32(&self) -> u32 {
        ((self.color[0] as u32) << 24) |
        ((self.color[1] as u32) << 16) |
        ((self.color[2] as u32) << 8) |
        (self.emission as u32)
    }
}
```

### Phase 2: AO-Aware Greedy Meshing

Add quantized AO to merge key:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FaceKeyWithAO {
    color: [u8; 3],
    emission: u8,
    ao_quantized: [u8; 4],  // 4 corners, 2 bits each (0-3)
}

fn quantize_ao(ao: f32) -> u8 {
    match ao {
        x if x < 0.25 => 0,
        x if x < 0.50 => 1,
        x if x < 0.75 => 2,
        _ => 3,
    }
}
```

### Data Structures

```rust
/// A merged quad from greedy meshing
struct GreedyQuad {
    /// Position in slice coordinates (u, v) and slice index
    u: usize,
    v: usize,
    slice: usize,
    
    /// Size of merged quad
    width: usize,
    height: usize,
    
    /// Face direction
    direction: FaceDir,
    
    /// Material key (color + emission + AO)
    key: FaceKey,
}

/// 2D mask for a single slice
type SliceMask = [[Option<FaceKey>; CHUNK_SIZE]; CHUNK_SIZE];
```

### Algorithm Outline

```rust
pub fn build_chunk_mesh_greedy(chunk: &VoxelChunk) -> Mesh {
    let mut all_quads: Vec<GreedyQuad> = Vec::new();
    
    // Process each of 6 face directions
    for direction in FaceDir::all() {
        let (slice_axis, u_axis, v_axis) = direction.axes();
        
        // Process each slice perpendicular to this direction
        for slice in 0..CHUNK_SIZE {
            // Build 2D mask of visible faces
            let mut mask = build_slice_mask(chunk, direction, slice);
            
            // Greedy merge the mask
            let quads = greedy_merge_slice(&mut mask, direction, slice);
            all_quads.extend(quads);
        }
    }
    
    // Convert quads to mesh vertices/indices
    build_mesh_from_quads(&all_quads, chunk)
}
```

### Vertex Generation for Merged Quads

For a merged quad of size W x H:
- 4 vertices (corners of the merged quad)
- 6 indices (2 triangles)
- AO values from original corner voxels (or averaged if AO-aware disabled)

```rust
fn emit_quad_vertices(
    quad: &GreedyQuad,
    chunk: &VoxelChunk,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 3]>,
    emissions: &mut Vec<f32>,
    aos: &mut Vec<f32>,
    indices: &mut Vec<u32>,
) {
    let base_idx = positions.len() as u32;
    
    // Calculate world positions for 4 corners
    let corners = quad.world_corners();
    
    for (i, corner) in corners.iter().enumerate() {
        positions.push(*corner);
        normals.push(quad.direction.normal());
        colors.push(quad.key.color_f32());
        emissions.push(quad.key.emission_f32());
        
        // AO for this corner - sample from original voxel
        aos.push(calculate_corner_ao(chunk, quad, i));
    }
    
    // Two triangles: 0-1-2, 0-2-3
    indices.extend_from_slice(&[
        base_idx, base_idx + 1, base_idx + 2,
        base_idx, base_idx + 2, base_idx + 3,
    ]);
}
```

## Test Strategy

### Unit Tests

1. **Single voxel**: 6 quads (no merging possible)
2. **2x1x1 same color**: 10 quads (vs 10 without greedy - adjacent faces merge)
3. **2x2x2 same color**: 6 quads (entire surfaces merge)
4. **2x2x2 checkerboard**: 24 quads (no merging - different colors)
5. **16x16x1 flat layer**: 2 quads (top + bottom fully merged)
6. **Mixed colors**: Verify boundaries don't merge

### Visual Test

Create `p15_greedy_mesh.rs`:
1. Create various shapes (solid cubes, flat surfaces)
2. Log quad count before/after greedy meshing
3. Verify no visual artifacts at merged edges

### Performance Benchmark

Compare mesh generation time and vertex counts:
- Face culling only vs greedy meshing
- Various chunk fill patterns

## Implementation Checklist

- [x] Add `FaceKey` struct for merge key
- [x] Add `GreedyQuad` struct for merged quads
- [x] Implement `build_slice_mask()` for each direction
- [x] Implement `greedy_merge_slice()` algorithm
- [x] Implement `emit_greedy_quad()` vertex generation
- [x] Handle AO correctly for merged quad corners
- [x] Add unit tests for quad counts (8 new tests)
- [x] Create visual test example (`p15_greedy_mesh.rs`)
- [x] Benchmark performance improvement

## Results

### Visual Test Results (`cargo run --example p15_greedy_mesh`)

```
Greedy Meshing Statistics
-------------------------

Shape                      Culled       Greedy    Reduction     Factor
----------------------------------------------------------------------
Single voxel                   24           24         0.0%       1.0x
2x2x2 uniform                  96           24        75.0%       4.0x
4x4x4 uniform                 384           24        93.8%      16.0x
8x8x8 uniform                1536           24        98.4%      64.0x
16x16 flat layer             2304           24        99.0%      96.0x
4x4x4 checkerboard            384          384         0.0%       1.0x
8x4x8 striped                1024           72        93.0%      14.2x
```

### Key Insights

- **Uniform surfaces**: Massive improvement (up to 96x for 16x16 flat surfaces)
- **Checkerboard patterns**: No improvement (different colors prevent merging)
- **Striped layers**: Partial improvement (layers merge within color boundaries)

### Unit Tests (8 new tests, all pass)

| Test | Expected | Description |
|------|----------|-------------|
| Single voxel | 24 vertices | No merging possible |
| 2x2x2 uniform | 24 vertices | 6 merged quads |
| 8x8x8 uniform | 24 vertices | 6 merged quads (64x improvement) |
| Checkerboard | 384 vertices | No merging (different colors) |
| Flat layer | 24 vertices | Top/bottom/sides all merge |
| Two colors | 40 vertices | Partial merge at boundaries |
| Attributes | All present | Color, emission, AO preserved |
| Comparison | 64x better | Greedy vs culling |

### Files Modified/Created

| File | Purpose |
|------|---------|
| `crates/studio_core/src/voxel_mesh.rs` | Greedy meshing implementation |
| `crates/studio_core/src/lib.rs` | Export `build_chunk_mesh_greedy` |
| `examples/p15_greedy_mesh.rs` | Visual test with statistics |
| `docs/research/greedy-meshing-research.md` | This documentation |

## References

1. [Mikola Lysenko - Meshing in a Minecraft Game](https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/)
2. [Mikola Lysenko - AO for Minecraft-like Worlds](https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/)
3. [JavaScript Reference Implementation](https://github.com/mikolalysenko/mikolalysenko.github.com/blob/gh-pages/MinecraftMeshes/js/greedy.js)
4. [Interactive Demo](https://mikolalysenko.github.com/MinecraftMeshes/index.html)
5. Bonsai `world_chunk.cpp` - disabled greedy meshing code
