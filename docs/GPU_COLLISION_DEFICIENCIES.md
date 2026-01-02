# GPU Collision System Deficiencies

This document catalogs the shortcuts and broken implementations in the GPU collision system.
**Last Updated:** After Phase 1-5 fixes

## Status Summary

| # | Deficiency | Severity | Status |
|---|------------|----------|--------|
| 1 | Sync readback blocks render | CRITICAL | **PENDING** (works but slow) |
| 2 | Fragment occupancy not uploaded | CRITICAL | **FIXED** |
| 3 | Dispatch size wrong | CRITICAL | **FIXED** |
| 4 | Negative coord handling | MEDIUM | Needs testing |
| 5 | Hash collision not handled | HIGH | **FIXED** |
| 6 | Contact overflow | LOW | Works (clamped) |
| 7 | No entity mapping | HIGH | **FIXED** |
| 8 | Terrain changes ignored | MEDIUM | **PENDING** |
| 9 | Overhead without fragments | LOW | Already handled |
| 10 | Arc<Mutex> fragile | MEDIUM | Works for now |

---

## Fixed Issues

### ~~2. Fragment Occupancy Data Not Uploaded to GPU~~ ✅ FIXED

**Fix:** Added `fragment_occupancy_buffer` to store bit-packed occupancy for all fragments.
- `collision_prepare.rs`: Builds occupancy buffer with offset/size per fragment
- `GpuFragmentData`: Now includes `occupancy_offset` and `occupancy_size`
- `voxel_collision.wgsl`: Added `is_fragment_voxel_occupied()` function that samples the buffer

### ~~3. Compute Dispatch Size is Wrong~~ ✅ FIXED

**Fix:** Changed to per-fragment dispatch with uniform updates.
- `collision_node.rs`: Now dispatches once per fragment
- `CollisionUniforms`: Added `fragment_index` and `fragment_count` fields
- `voxel_collision.wgsl`: Reads `fragment_idx` from `uniforms.fragment_index`

Each dispatch:
- workgroups_x = ceil(size.x / 8)
- workgroups_y = ceil(size.y / 8)
- workgroups_z = size.z

### ~~5. Hash Table Collision Resolution is Incomplete~~ ✅ FIXED

**Fix:** CPU now uses linear probing matching the shader.
- `voxel_collision_gpu.rs`: `update_chunk_index()` uses open addressing with 4 probe slots
- Added `slot_to_coord: HashMap<u32, IVec3>` to track slot occupancy
- Shader's `lookup_chunk_layer()` already did linear probing

### ~~7. No Entity Mapping for Readback~~ ✅ FIXED

**Fix:** Entity mapping now flows through the system.
- `GpuCollisionResult`: Added `fragment_entities: Vec<Entity>`
- `collision_node.rs`: Builds entity map from `ExtractedFragments`
- `voxel_fragment.rs`: Uses entity lookup instead of query order index

---

## Remaining Issues

### 1. Synchronous Readback Blocks the Render Thread

**Status:** PENDING (functional but slow)

**Current behavior:** Uses `poll(wgpu::PollType::wait())` which blocks.

**Planned fix (Phase 4):**
- Double-buffer staging buffers
- Use async `map_async` with callback
- Channel results to main world
- 1-frame latency acceptable

### 4. Shader Chunk Coordinate Handling for Negative Coords

**Status:** Needs testing

**Current behavior:** Uses bit shift which may work for WGSL arithmetic shift.
```wgsl
let chunk_coord = vec3<i32>(
    world_pos.x >> 5,  // divide by 32
    ...
);
```

**Note:** Should test with fragments at negative coordinates to verify.

### 8. Terrain Changes Not Detected

**Status:** PENDING

**Current behavior:** Terrain extracted once, never updated.

**Planned fix (Phase 6):**
- Track terrain version/generation
- Re-extract dirty chunks
- Incremental GPU upload

---

## Current Architecture

```
Main World (Update)                    Render World (Render)
─────────────────                      ─────────────────────
VoxelFragment                          ExtractedFragments
+ Entity ID        ──extract──►        + entity (for mapping)
Transform                              + occupancy_data
FragmentOccupancy                             │
                                              ▼
TerrainOccupancy   ──extract──►        GpuWorldOccupancy
(static for now)                       (chunk textures)
                                       + hash table with probing
                                              │
                                              ▼
                                       voxel_collision.wgsl
                                       (per-fragment dispatch)
                                       + reads fragment_index from uniform
                                       + samples fragment occupancy
                                              │
                                              ▼
                                       Contact Buffer
                                              │
                                       ──sync readback──
                                              │
GpuCollisionResult ◄───────────        Readback (blocking)
+ contacts                             + fragment_entities mapping
+ fragment_entities
      │
      ▼
gpu_fragment_collision_system
(entity-keyed application)
```

---

## Performance Notes

With synchronous readback, GPU collision is likely **slower** than CPU for small fragment counts. The benefit comes with many fragments (50+) where parallel voxel checking outweighs sync overhead.

**Benchmarking needed** to determine crossover point.

---

## Files Modified in Fixes

| File | Changes |
|------|---------|
| `collision_node.rs` | Per-fragment dispatch, entity mapping |
| `collision_prepare.rs` | Fragment occupancy buffer upload |
| `voxel_collision_gpu.rs` | Hash table probing, CollisionUniforms fields, entity vec |
| `voxel_collision.wgsl` | Fragment index from uniform, occupancy sampling |
| `voxel_fragment.rs` | Entity-keyed contact application |
