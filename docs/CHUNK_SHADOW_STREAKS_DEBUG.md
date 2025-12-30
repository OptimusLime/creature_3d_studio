# Chunk Streaming Vertical Shadow Streaks Debug Doc

## Problem Statement

In `p17_chunk_streaming` screenshot, there are massive vertical streak artifacts across the entire terrain. The shadows look like they're drawn with a broken comb - vertical lines everywhere.

## Observed Symptoms

- Vertical streak/striping patterns across all shadows
- Pattern appears consistent across chunks
- Streaks appear to align with... something (X or Z axis? chunk boundaries?)

## Scene Setup (p17_chunk_streaming)

- Camera: `Vec3::new(200.0, 100.0, 200.0)` looking at `Vec3::new(128.0, 0.0, 128.0)`
- World: 8x1x8 chunks of procedural terrain with varying heights
- Chunk streaming: load_radius=6, y_range=(-1, 1)
- Each chunk has glowing pillar at center

## Hypotheses

### Hypothesis 1: No explicit shadow caster - closest light selected

**Theory**: The chunk streaming example doesn't explicitly mark a shadow-casting light. The system picks the "closest to camera" which might be one of the glowing pillars deep in the scene, resulting in weird shadow angles.

**Test**: Check which light is being used for shadows. Add explicit shadow caster.

**Status**: PENDING

---

### Hypothesis 2: Shadow map not covering the visible scene

**Theory**: The shadow-casting light (one of the pillars) has a limited radius. Large portions of the scene are outside the shadow map frustum, causing default/garbage values.

**Test**: Check if the streaks correspond to areas outside shadow light radius.

**Status**: PENDING

---

### Hypothesis 3: Greedy mesh generating bad geometry

**Theory**: Greedy meshing across chunks might be creating very long thin quads that span many voxels. These might have interpolation issues with shadow UVs.

**Test**: Disable greedy meshing in chunk streaming config and see if streaks disappear.

**Status**: PENDING

---

### Hypothesis 4: World position precision issues

**Theory**: With chunks at large world coordinates (0-128+), world position floating point precision might be degrading, causing jittery shadow UV calculations.

**Test**: Check world position values in the G-buffer for chunks at extreme positions.

**Status**: PENDING

---

### Hypothesis 5: Shadow UV wrapping/clamping issues

**Theory**: For fragments outside the shadow map's valid region, the UV might wrap or sample garbage, creating streaks at regular intervals.

**Test**: Add bounds checking to shadow UV and return 1.0 (lit) for out-of-bounds.

**Status**: PENDING

---

### Hypothesis 6: Multiple lights interfering

**Theory**: The chunk streaming world has 64 glowing pillars (8x8 grid). If the shadow system is confused about which light to use, or if the light index is wrong, we get garbage.

**Test**: Verify point_lights.lights[0] matches the actual shadow-casting light.

**Status**: PENDING

---

### Hypothesis 7: Vertical streaks = Y-axis aligned artifacts

**Theory**: The streaks might be Y-axis aligned because the shadow projection is looking down (-Y) and the terrain is nearly flat. Small depth variations across the flat surface cause the streaks.

**Test**: Check if changing camera angle or light position affects streak direction.

**Status**: PENDING

---

## Test Log

### Test 1: [Date/Time]
**Action**: 
**Result**: 
**Conclusion**: 

---

## Key Differences from p9_island

| Aspect | p9_island | p17_chunk_streaming |
|--------|-----------|---------------------|
| Shadow light | Explicit at (-6, 10, -6) | Auto-selected (closest) |
| World size | Single small island | 8x8 chunks (128x128 voxels) |
| Light count | 1 crystal | 64 pillars |
| Camera distance | Close | Far (200 units out) |
| Greedy meshing | Yes | Yes |

## Files to Modify

- `examples/p17_chunk_streaming.rs` - Add explicit shadow caster
- `assets/shaders/deferred_lighting.wgsl` - Add debug modes
- `crates/studio_core/src/deferred/point_light_shadow.rs` - Shadow selection

## Resolution

[To be filled in when fixed]

---

## Cross-Chunk AO Bug Investigation (2024-12-30)

### Original Problem

Grid/cross artifacts visible on floors at chunk boundaries - bright lines where chunks meet.

### What We Implemented

Extended `ChunkBorders` to store diagonal neighbor data:

1. **Added `BorderEdge` struct** - 1D array of CHUNK_SIZE bools for edge neighbors
2. **Added 12 edge borders** to `ChunkBorders`:
   - XY edges: `edge_neg_x_neg_y`, `edge_neg_x_pos_y`, `edge_pos_x_neg_y`, `edge_pos_x_pos_y`
   - XZ edges: `edge_neg_x_neg_z`, `edge_neg_x_pos_z`, `edge_pos_x_neg_z`, `edge_pos_x_pos_z`
   - YZ edges: `edge_neg_y_neg_z`, `edge_neg_y_pos_z`, `edge_pos_y_neg_z`, `edge_pos_y_pos_z`
3. **Added 8 corner borders** (single bools) for 3-axis diagonal neighbors
4. **Added `extract_border_edge()` and `extract_border_corner()`** methods to `VoxelWorld`
5. **Rewrote `is_neighbor_solid()`** to:
   - Calculate target position from current position + offset
   - Detect which boundaries the TARGET crosses (not which boundary the source is at)
   - Handle 1-axis (face), 2-axis (edge), and 3-axis (corner) crossings
   - Use correct `target_x`, `target_y`, `target_z` for lookups

### Files Modified

- `crates/studio_core/src/voxel.rs`:
  - Added `BorderEdge` struct
  - Extended `ChunkBorders` with edge and corner fields
  - Rewrote `is_neighbor_solid()` completely
  - Added `extract_border_edge()` and `extract_border_corner()`
  - Added unit tests for diagonal neighbors

### Current Status: PARTIALLY FIXED

**Improved:**
- Vertical surfaces (walls, pillars) now have correct AO at chunk boundaries
- Simple floor-only scenes show NO grid artifacts (verified with test)
- All 83 unit tests pass

**STILL BROKEN:**
- Floor surfaces in complex scenes STILL show grid artifacts
- The grid pattern is most visible on flat horizontal surfaces
- The issue may be related to how AO offsets work for +Y faces specifically

### Remaining Investigation Needed

The AO calculation for +Y faces (top faces, like floors) uses offsets that all have `dy=1` (checking above). For example:
```rust
FaceDir::PosY => [
    [(0, 1, -1), (-1, 1, 0), (-1, 1, -1)],  // All have y+1
    ...
]
```

The bug might be:
1. Incorrect coordinate mapping when looking up edge/corner borders
2. Edge case in how `target_x/y/z` are calculated for floor-level voxels
3. Something specific to how face borders interact with edge borders at chunk boundaries

### Debug Commands

```bash
# Set DEBUG_MODE in deferred_lighting.wgsl:
# Mode 5 = AO only (shows the grid bug)
# Mode 0 = normal render

cargo run --example p18_cross_chunk_culling
cargo run --example p17_chunk_streaming
```
