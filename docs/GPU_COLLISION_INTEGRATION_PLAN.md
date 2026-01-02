# GPU Collision Integration Plan (Revised)

## Status: PHASES 1-3, 5 COMPLETE

Most critical deficiencies have been fixed. The system now produces correct collision results.
Remaining work: async readback (Phase 4), terrain change detection (Phase 6).

## Completed Fixes

1. ✅ **Dispatch size calculation** - Per-fragment dispatch with uniform updates
2. ✅ **Fragment occupancy upload** - Bit-packed occupancy buffer with shader sampling
3. ✅ **Entity mapping** - Contacts keyed by Entity, not query order
4. ✅ **Hash collision handling** - CPU uses linear probing matching shader

## Remaining Issues

1. ⏳ **Synchronous readback** - Works but blocks render thread (Phase 4)
2. ⏳ **Terrain changes** - Not detected, needs version tracking (Phase 6)

## Architecture (Corrected)

```
Main World (Update)                    Render World (Render)
─────────────────                      ─────────────────────
VoxelFragment                          ExtractedFragments
+ Entity ID        ──extract──►        + entity_to_index map
Transform                              + occupancy data per fragment
FragmentOccupancy                             │
                                              ▼
TerrainOccupancy   ──extract──►        GpuWorldOccupancy
+ change detection                     (chunk textures)
                                       + proper hash table
                                              │
                                              ▼
                                       voxel_collision.wgsl
                                       (fixed compute shader)
                                              │
                                              ▼
                                       Contact Buffer + Entity IDs
                                              │
                                       ──async map──►
                                              │
CollisionContacts  ◄──channel──        Staging Buffer
+ keyed by Entity                      (double-buffered)
      │
      ▼
gpu_fragment_collision_system
(applies ExternalForce by Entity)
```

---

## Phase 1: Fix Compute Dispatch and Fragment Indexing ✅ COMPLETE

**Goal:** Each fragment gets correct workgroups and maps back to correct Entity.

**Solution Implemented:**
- Per-fragment dispatch with uniform updates
- `CollisionUniforms` now includes `fragment_index` and `fragment_count`
- Shader reads `uniforms.fragment_index` instead of `workgroup_id.z`
- Entity mapping stored in `GpuCollisionResult.fragment_entities`

**Files Changed:**
- `collision_node.rs`: Per-fragment dispatch loop
- `voxel_collision_gpu.rs`: Added fields to `CollisionUniforms` and `GpuCollisionResult`
- `voxel_collision.wgsl`: Read fragment_index from uniforms

---

## Phase 2: Upload Fragment Occupancy Data ✅ COMPLETE

**Goal:** GPU knows which voxels in each fragment are actually occupied.

**Solution Implemented:**
- Added `fragment_occupancy_buffer` to `GpuCollisionPipeline`
- `GpuFragmentData` includes `occupancy_offset` and `occupancy_size`
- Shader has `is_fragment_voxel_occupied()` function using linear indexing
- Empty voxels skipped before terrain collision check

**Files Changed:**
- `voxel_collision_gpu.rs`: Added buffer, `new_with_occupancy()` constructor
- `collision_prepare.rs`: Builds occupancy buffer with per-fragment data
- `voxel_collision.wgsl`: Added binding and occupancy check function

---

## Phase 3: Fix Hash Table for Chunk Lookup ✅ COMPLETE

**Goal:** All terrain chunks are findable, no silent drops.

**Solution Implemented:**
- CPU now uses open addressing with linear probing (4 slots)
- Added `slot_to_coord: HashMap<u32, IVec3>` to track slot occupancy
- Matches shader's existing `lookup_chunk_layer()` probing logic

**Files Changed:**
- `voxel_collision_gpu.rs`: `update_chunk_index()` with probing, `slot_to_coord` field

---

## Phase 4: Implement Async Readback with Double Buffering

**Goal:** No render thread blocking, 1-frame latency acceptable.

**Current Bug:**
```rust
render_device.wgpu_device().poll(wgpu::PollType::wait()); // BLOCKS!
```

**Fix:**
- Double-buffer staging buffers (frame N writes, frame N-1 reads)
- Use async map with callback
- Channel results to main world
- Main world uses contacts from previous frame

**Tasks:**

1. Create double-buffered staging:
   ```rust
   pub struct CollisionStagingBuffers {
       buffers: [Buffer; 2],
       current_write: usize,
       pending_read: Option<usize>,
   }
   ```

2. In collision node:
   ```rust
   // Copy to current write buffer
   encoder.copy_buffer_to_buffer(&contact_buffer, &staging.buffers[staging.current_write]);
   
   // If previous buffer is mapped, read it
   if let Some(read_idx) = staging.pending_read {
       if staging.buffers[read_idx].is_mapped() {
           let data = staging.buffers[read_idx].get_mapped_range();
           channel.send(parse_contacts(&data));
           staging.buffers[read_idx].unmap();
           staging.pending_read = None;
       }
   }
   
   // Start async map of current write buffer for next frame
   staging.buffers[staging.current_write].map_async(...);
   staging.pending_read = Some(staging.current_write);
   staging.current_write = 1 - staging.current_write;
   ```

3. Main world receives via channel:
   ```rust
   fn receive_gpu_contacts(
       receiver: Res<ContactReceiver>,
       mut contacts: ResMut<GpuCollisionContacts>,
   ) {
       if let Ok(new_contacts) = receiver.try_recv() {
           contacts.set(new_contacts);
       }
   }
   ```

4. Remove all `poll(Wait)` calls

**Verification:**
```bash
cargo run --example p22_voxel_fragment --release
# Spawn 50 fragments
# Monitor frame times - should be consistent, no spikes
# GPU profiler shows no stalls
```

---

## Phase 5: Entity-Keyed Contact Application ✅ COMPLETE

**Goal:** Forces applied to correct entities regardless of query order.

**Solution Implemented:**
- `GpuCollisionResult` now includes `fragment_entities: Vec<Entity>`
- `collision_node.rs` builds entity map from `ExtractedFragments`
- `voxel_fragment.rs`: `gpu_fragment_terrain_collision_system` uses entity lookup

**Files Changed:**
- `voxel_collision_gpu.rs`: Added `fragment_entities` to `GpuCollisionResult`
- `collision_node.rs`: Builds and passes entity map
- `voxel_fragment.rs`: Entity-keyed contact application with HashMap lookup

---

## Phase 6: Terrain Change Detection

**Goal:** GPU sees terrain changes without full re-upload.

**Current Bug:**
```rust
if extracted.chunks.is_empty() {
    // Only extracts once, ignores future changes
}
```

**Fix:**
- Track terrain version/generation
- Re-extract only changed chunks
- Incremental GPU upload

**Tasks:**

1. Add change tracking to TerrainOccupancy:
   ```rust
   pub struct TerrainOccupancy {
       pub occupancy: WorldOccupancy,
       pub generation: u64,
       pub dirty_chunks: HashSet<IVec3>,
   }
   ```

2. Extraction compares generations:
   ```rust
   fn extract_terrain(
       terrain: Extract<Res<TerrainOccupancy>>,
       mut extracted: ResMut<ExtractedTerrainChunks>,
   ) {
       if terrain.generation != extracted.last_generation {
           for chunk in &terrain.dirty_chunks {
               extracted.dirty_chunks.insert(*chunk, terrain.get_chunk(*chunk));
           }
           extracted.last_generation = terrain.generation;
       }
   }
   ```

3. GPU upload only dirty chunks

**Verification:**
```bash
cargo run --example p22_voxel_fragment
# Spawn fragment on terrain
# Add voxel to terrain under fragment
# Fragment should collide with new voxel
```

---

## Phase 7: Performance Validation and Cleanup

**Goal:** GPU collision is faster than CPU for 50+ fragments.

**Tasks:**

1. Add benchmark comparing CPU vs GPU:
   ```rust
   // In p22 example, press 'B' for benchmark
   // Measures: 10, 25, 50, 100 fragments
   // Reports: avg frame time, collision time specifically
   ```

2. Profile and optimize:
   - Reduce buffer sizes if possible
   - Optimize shader occupancy check
   - Consider spatial partitioning for large fragment counts

3. Add fallback to CPU if:
   - GPU pipeline fails to create
   - Readback fails 3 frames in a row
   - Contact count exceeds buffer

4. Clean up logging:
   - Debug logs behind feature flag
   - Only log warnings/errors in release

5. Documentation:
   - Update GPU_COLLISION_INTEGRATION_PLAN.md with final architecture
   - Document configuration options
   - Add troubleshooting guide

**Verification:**
```bash
cargo run --example p22_voxel_fragment --release
# Press B for benchmark
# GPU mode: 50 fragments at 58+ FPS
# CPU mode: 50 fragments at ~30 FPS
# GPU is 2x faster minimum
```

---

## Success Metrics

| Metric | CPU Baseline | GPU Target | Current (Broken) |
|--------|--------------|------------|------------------|
| 10 fragments FPS | 60 | 60 | N/A |
| 50 fragments FPS | 30 | 58+ | Worse than CPU |
| 100 fragments FPS | 15 | 55+ | N/A |
| Collision correctness | 100% | 100% | ~0% (wrong dispatch) |
| Entity mapping | N/A | 100% | 0% (no mapping) |
| Frame stutter | None | None | Always (sync readback) |

---

## Files to Modify/Create

| File | Action | Phase |
|------|--------|-------|
| `collision_node.rs` | Rewrite dispatch logic | 1 |
| `collision_extract.rs` | Add entity mapping | 1, 5 |
| `collision_prepare.rs` | Add fragment occupancy buffer | 2 |
| `voxel_collision.wgsl` | Fix fragment_idx, add occupancy check | 1, 2 |
| `voxel_collision_gpu.rs` | Fix hash table | 3 |
| `collision_readback.rs` | Rewrite with double buffering | 4 |
| `voxel_fragment.rs` | Entity-keyed application | 5 |
| `plugin.rs` | Wire up new systems | All |

---

## Risk Mitigation

1. **GPU not faster**: Profile before/after, optimize shader
2. **Async complexity**: Start with sync, optimize later (but don't block)
3. **Entity mapping breaks**: Use generation IDs + entity, validate
4. **Hash table full**: Use larger table, or chunked lookup
