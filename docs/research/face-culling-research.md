# Face Culling Research

## Overview

Face culling eliminates internal faces that are never visible because they're occluded by adjacent solid voxels. This dramatically reduces vertex count for dense voxel meshes.

## Performance Impact

| Scenario | Without Culling | With Culling | Reduction |
|----------|-----------------|--------------|-----------|
| Single voxel | 6 faces | 6 faces | 0% |
| 2x2x2 cube (8 voxels) | 48 faces | 24 faces | 50% |
| Solid 16x16x16 chunk | 24,576 faces | ~1,536 faces | 94% |
| Hollow 16x16x16 shell | 24,576 faces | ~5,376 faces | 78% |

## Bonsai's Approach

Bonsai uses **bitwise operations on 64-bit occupancy masks** for face culling:

```c
// For each Y,Z slice, compute face visibility
u64 RightFaces = (Bits) & ~(Bits>>1);  // Right neighbor empty
u64 LeftFaces  = (Bits) & ~(Bits<<1);  // Left neighbor empty
u64 FrontFaces = Bits & (~yBits);      // Front neighbor empty
// ... 6 directions total
```

**Key Points**:
- Uses bit-packed occupancy (1 bit per voxel)
- Processes 64 voxels at once with bitwise operations
- Very fast, SIMD-friendly
- Does NOT do face merging (greedy meshing)

## Our Approach

We use a simpler per-voxel neighbor check since:
1. Our chunks are 32x32x32 (not 64x64x64)
2. We already have `is_neighbor_solid()` helper
3. Simplicity > raw speed for initial implementation
4. Can optimize later if needed

### Algorithm

```rust
for (x, y, z, voxel) in chunk.iter() {
    // Only add face if neighbor in that direction is empty
    if !chunk.is_neighbor_solid(x, y, z, 1, 0, 0) { add_face(+X); }
    if !chunk.is_neighbor_solid(x, y, z, -1, 0, 0) { add_face(-X); }
    if !chunk.is_neighbor_solid(x, y, z, 0, 1, 0) { add_face(+Y); }
    if !chunk.is_neighbor_solid(x, y, z, 0, -1, 0) { add_face(-Y); }
    if !chunk.is_neighbor_solid(x, y, z, 0, 0, 1) { add_face(+Z); }
    if !chunk.is_neighbor_solid(x, y, z, 0, 0, -1) { add_face(-Z); }
}
```

### Edge Behavior

Faces at chunk boundaries (x=0, y=0, z=0, x=31, y=31, z=31) are always rendered since:
- `is_neighbor_solid()` returns `false` for out-of-bounds coordinates
- This is correct - boundary faces are potentially visible

## Greedy Meshing (Future Work)

Greedy meshing merges adjacent same-material faces into larger quads:

| Scenario | Face Culling Only | + Greedy Meshing |
|----------|-------------------|------------------|
| 16x16 flat surface | 256 quads | 1-16 quads |
| Checkerboard pattern | 256 quads | 256 quads |

Greedy meshing is most effective for:
- Large flat surfaces
- Same-material regions
- Low complexity voxel art

Less effective for:
- Highly detailed models (many colors)
- Organic shapes
- Checkerboard patterns

**Decision**: Implement face culling first, add greedy meshing in Phase 14 if needed.

## Test Strategy

### Unit Tests

1. **Single isolated voxel**: Should produce 6 faces (24 vertices)
2. **Two adjacent voxels**: Should produce 10 faces (4 hidden faces culled)
3. **2x2x2 solid cube**: Should produce 24 faces (24 hidden faces culled)
4. **3x3x3 hollow shell**: All faces visible (no culling)
5. **L-shaped structure**: Verify correct internal face culling

### Integration Test

Create a visual test (`p14_face_culling.rs`) that:
1. Creates a solid 8x8x8 cube
2. Logs vertex count before and after culling
3. Expected reduction: 384 faces (8*8*8*6 / 8) vs 3072 faces (8*8*8*6)

### Verification Metrics

```
Before: voxels * 6 * 4 = vertices (all faces)
After: exposed_faces * 4 = vertices (only visible faces)

For solid 8x8x8:
- Total voxels: 512
- Before culling: 512 * 6 = 3072 faces, 12288 vertices
- After culling: 6 * 64 = 384 faces (6 sides * 8*8 surface), 1536 vertices
- Reduction: 87.5%
```

## Implementation Checklist

- [x] Verify `is_neighbor_solid()` works for boundary cases
- [x] Modify `add_cube_faces_with_ao()` to accept face visibility mask
- [x] Add neighbor checks in `build_chunk_mesh()`
- [x] Add unit tests for face count verification
- [x] Create visual test example (`p14_face_culling.rs`)
- [x] Log vertex/face counts for verification

## Results

### Unit Test Results (14 tests pass)

| Test | Result |
|------|--------|
| Single voxel → 24 vertices (6 faces) | PASS |
| Two adjacent voxels → 40 vertices (10 faces) | PASS |
| 2x2x2 cube → 96 vertices (24 faces) | PASS |
| 3x3x3 cube → 216 vertices (54 faces) | PASS |
| Cross shape → 88 vertices (22 faces) | PASS |
| Line of 5 → 88 vertices (22 faces) | PASS |

### Visual Test Results (`cargo run --example p14_face_culling`)

```
Face Culling Statistics
-----------------------

Single voxel:       24 vertices (max:   24, reduction:   0.0%)
2x2x2 cube:         96 vertices (max:  192, reduction:  50.0%)
4x4x4 cube:        384 vertices (max: 1536, reduction:  75.0%)
8x8x8 cube:       1536 vertices (max: 12288, reduction:  87.5%)
16x16x16 cube:    6144 vertices (max: 98304, reduction:  93.8%)
Hollow 8x8x8:     2400 vertices (max:  7104, reduction:  66.2%)
```

### Key Files Modified

| File | Changes |
|------|---------|
| `crates/studio_core/src/voxel_mesh.rs` | Added face_mask parameter, neighbor checks in build_chunk_mesh() |
| `examples/p14_face_culling.rs` | New visual test with statistics logging |
| `docs/research/face-culling-research.md` | This document |
