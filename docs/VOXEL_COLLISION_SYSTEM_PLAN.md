# Voxel Collision System Plan

## Summary

Replace trimesh-based physics colliders with a GPU-accelerated voxel occupancy collision system. This enables Minecraft-scale worlds without the performance death of 20,000+ triangle trimeshes.

## Context & Motivation

Current approach generates trimesh colliders from visual mesh:
- Checkerboard terrain (20x20x3) = 1760 triangles
- Trimesh collision is O(n) triangle tests
- Scales terribly with world size

New approach:
- Store voxel occupancy in GPU textures
- Collision = texture sample (O(1) per voxel)
- Handles arbitrary world size
- Enables real-time copy/paste overlap preview

## Architecture Overview

```
┌────────────────────────────────────────────────────────────────┐
│                         GPU LAYER                              │
│                                                                │
│   Chunk Texture Array          Fragment Textures               │
│   (one layer per chunk)        (one 3D tex per fragment)       │
│            │                           │                       │
│            └───────────┬───────────────┘                       │
│                        ▼                                       │
│            Collision Compute Shader                            │
│            - Fragment voxels × World occupancy                 │
│            - Output: collision points + normals                │
└────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌────────────────────────────────────────────────────────────────┐
│                       CPU LAYER                                │
│                                                                │
│   Rapier: Fragments as cuboid approximations                   │
│   - Broad-phase fragment-vs-fragment                           │
│   - GPU collision results → external forces                    │
│                                                                │
│   Kinematic Controller: Player/creatures on terrain            │
│   - Uses same occupancy queries                                │
└────────────────────────────────────────────────────────────────┘
```

## Multi-Chunk Solution

Fragments can span chunk boundaries. Solution:

1. **Texture2DArray** - Each layer = one 32x32x32 chunk occupancy
2. **Chunk Index Buffer** - Maps `IVec3 chunk_coord → layer_index`
3. **Shader lookup**:
   ```wgsl
   fn is_occupied(world_pos: vec3<i32>) -> bool {
       let chunk_coord = world_pos >> 5;  // divide by 32
       let local_pos = world_pos & 31;    // mod 32
       let layer = chunk_index[hash(chunk_coord)];
       if layer < 0 { return false; }  // chunk not loaded
       return sample_occupancy(chunk_array, layer, local_pos);
   }
   ```

## Naming Conventions

- `voxel_collision.rs` - CPU occupancy data structures
- `voxel_collision_gpu.rs` - GPU texture management and shaders
- `ChunkOccupancy` - Single chunk bit-packed occupancy
- `WorldOccupancy` - Multi-chunk manager (CPU)
- `GpuWorldOccupancy` - GPU texture array + index buffer
- `voxel_collision.wgsl` - Collision compute shader

---

## Phase 1: Benchmark Infrastructure + CPU Occupancy

**Outcome:** Shared benchmark plugin. ChunkOccupancy data structure with tests.

### Tasks

1. Create `crates/studio_core/src/benchmark.rs`:
   ```rust
   pub struct BenchmarkPlugin;
   pub struct BenchmarkConfig {
       pub report_interval_secs: f32,
       pub enabled: bool,
   }
   // Uses FrameTimeDiagnosticsPlugin, prints FPS every N seconds
   ```

2. Create `crates/studio_core/src/voxel_collision.rs`:
   ```rust
   /// Bit-packed occupancy for 32x32x32 chunk = 4096 bytes
   pub struct ChunkOccupancy {
       data: [u32; 1024],
   }
   
   impl ChunkOccupancy {
       pub fn new() -> Self;
       pub fn from_voxel_world(world: &VoxelWorld, chunk_min: IVec3) -> Self;
       pub fn get(&self, local_pos: UVec3) -> bool;
       pub fn set(&mut self, local_pos: UVec3, occupied: bool);
       pub fn as_bytes(&self) -> &[u8];  // For GPU upload
   }
   ```

3. Add to `lib.rs`, export `BenchmarkPlugin`, `ChunkOccupancy`

4. Tests:
   ```rust
   #[test] fn test_chunk_occupancy_roundtrip();
   #[test] fn test_chunk_occupancy_from_voxel_world();
   #[test] fn test_chunk_occupancy_bit_packing();
   ```

5. Update `p22_voxel_fragment.rs` to use `BenchmarkPlugin`

**Verification:**
```bash
cargo test -p studio_core occupancy -- --nocapture
cargo run --example p22_voxel_fragment --release 2>&1 | grep "FPS:"
```

---

## Phase 2: World Occupancy + CPU Collision Query

**Outcome:** Can query collision across multiple chunks on CPU. Benchmark shows baseline performance.

### Tasks

1. Add to `voxel_collision.rs`:
   ```rust
   pub struct WorldOccupancy {
       chunks: HashMap<IVec3, ChunkOccupancy>,
   }
   
   pub struct CollisionPoint {
       pub world_pos: Vec3,
       pub normal: Vec3,
       pub penetration: f32,
   }
   
   pub struct CollisionResult {
       pub contacts: Vec<CollisionPoint>,
   }
   
   impl WorldOccupancy {
       pub fn new() -> Self;
       pub fn load_chunk(&mut self, coord: IVec3, occupancy: ChunkOccupancy);
       pub fn unload_chunk(&mut self, coord: IVec3);
       pub fn get_voxel(&self, world_pos: IVec3) -> bool;
       pub fn check_aabb(&self, aabb: Aabb) -> CollisionResult;
       pub fn region_is_clear(&self, min: IVec3, max: IVec3) -> bool;
   }
   ```

2. Tests:
   ```rust
   #[test] fn test_world_occupancy_single_chunk();
   #[test] fn test_world_occupancy_cross_chunk_query();
   #[test] fn test_aabb_collision_single_voxel();
   #[test] fn test_aabb_collision_cross_chunk();
   #[test] fn test_region_is_clear();
   ```

3. Add benchmark test that measures CPU collision query time

**Verification:**
```bash
cargo test -p studio_core world_occupancy -- --nocapture
cargo test -p studio_core aabb_collision -- --nocapture
# Output shows query times
```

---

## Phase 3: Voxel Physics World (REFACTORED)

> **STATUS: REQUIRES REFACTOR** - Initial implementation was broken.
> See `docs/KINEMATIC_CONTROLLER_REFACTOR.md` for details.

**Outcome:** A proper `VoxelPhysicsWorld` with physics-engine-like API. Fixed timestep. No physics logic in examples.

### Phase 3.1: VoxelPhysicsWorld API

**File:** `crates/studio_core/src/voxel_physics_world.rs`

```rust
pub struct VoxelPhysicsWorld {
    occupancy: WorldOccupancy,
    bodies: Vec<KinematicBody>,
    config: PhysicsConfig,
    accumulator: f32,
}

pub struct PhysicsConfig {
    pub fixed_timestep: f32,      // 1/60
    pub gravity: Vec3,            // (0, -25, 0)
    pub max_steps_per_frame: u32, // 4
}

pub struct KinematicBody {
    pub position: Vec3,
    pub velocity: Vec3,
    pub half_extents: Vec3,
    pub grounded: bool,
    pub input_velocity: Vec3,  // From player input, separate from physics velocity
}

impl VoxelPhysicsWorld {
    pub fn new(occupancy: WorldOccupancy, config: PhysicsConfig) -> Self;
    pub fn step(&mut self, delta: f32);  // Uses fixed timestep internally
    pub fn add_body(&mut self, body: KinematicBody) -> BodyHandle;
    pub fn get_body(&self, handle: BodyHandle) -> Option<&KinematicBody>;
    pub fn set_body_input(&mut self, handle: BodyHandle, velocity: Vec3);
    pub fn jump(&mut self, handle: BodyHandle, speed: f32) -> bool;
}
```

**Verification:**
```bash
cargo test -p studio_core voxel_physics_world -- --nocapture
# Test: body at y=10 lands at y≈3.9 after 3 seconds of simulation
```

### Phase 3.2: Fixed Timestep Implementation

```rust
impl VoxelPhysicsWorld {
    pub fn step(&mut self, delta: f32) {
        self.accumulator += delta;
        let mut steps = 0;
        while self.accumulator >= self.config.fixed_timestep 
              && steps < self.config.max_steps_per_frame {
            self.step_fixed();
            self.accumulator -= self.config.fixed_timestep;
            steps += 1;
        }
    }
}
```

**Verification:**
```rust
#[test]
fn test_determinism_at_different_framerates() {
    // Simulate at 30fps (33ms steps) and 120fps (8ms steps)
    // Final position should be identical
}
```

### Phase 3.3: Example Uses API Only

`examples/p23_kinematic_controller.rs` should contain:
- NO gravity calculations
- NO velocity manipulation
- ONLY: `physics.set_body_input()`, `physics.jump()`, `physics.step()`

```rust
fn physics_step(time: Res<Time>, mut physics: ResMut<VoxelPhysicsWorld>) {
    physics.step(time.delta_secs());
}

fn sync_transforms(physics: Res<VoxelPhysicsWorld>, mut query: Query<(&Player, &mut Transform)>) {
    for (player, mut transform) in query.iter_mut() {
        if let Some(body) = physics.get_body(player.body_handle) {
            transform.translation = body.position;
        }
    }
}
```

**Verification:**
```bash
cargo run --example p23_kinematic_controller
# Player falls, lands on ground, can walk with WASD, can jump
# NO JITTER. NO FALLING THROUGH FLOOR.
```

### Phase 3.4: Comprehensive Tests

Required tests (must all pass before Phase 4):
1. `test_body_falls_and_lands`
2. `test_body_stops_at_wall`
3. `test_body_slides_along_wall`  
4. `test_body_jumps_when_grounded`
5. `test_body_cannot_jump_in_air`
6. `test_fixed_timestep_determinism`
7. `test_cross_chunk_collision`

---

### Files Changed (Phase 3 Refactor)

| File | Action |
|------|--------|
| `src/voxel_physics_world.rs` | NEW - Physics simulation with fixed timestep |
| `src/voxel_collision.rs` | MODIFY - Remove KinematicController (move to physics_world) |
| `src/lib.rs` | MODIFY - Export VoxelPhysicsWorld |
| `examples/p23_kinematic_controller.rs` | REWRITE - Use VoxelPhysicsWorld API |
   - Display FPS via BenchmarkPlugin

3. Tests:
   ```rust
   #[test] fn test_controller_stands_on_ground();
   #[test] fn test_controller_blocked_by_wall();
   #[test] fn test_controller_slides_along_slope();
   ```

**Verification:**
```bash
cargo run --example p23_kinematic_controller --release
# Controller walks on terrain, FPS displayed
# Can't walk through walls
```

---

## Phase 4: Fragment Occupancy + Rapier Integration

**Outcome:** Fragments use occupancy for terrain collision, Rapier cuboids for fragment-fragment. Benchmark compares to old trimesh approach.

### Tasks

1. Add to `voxel_collision.rs`:
   ```rust
   pub struct FragmentOccupancy {
       data: Vec<u32>,
       size: UVec3,
   }
   
   impl FragmentOccupancy {
       pub fn from_voxel_world(world: &VoxelWorld) -> Self;
       pub fn get(&self, local_pos: UVec3) -> bool;
       pub fn aabb_size(&self) -> Vec3;
       pub fn as_bytes(&self) -> &[u8];
   }
   
   impl WorldOccupancy {
       /// Check fragment against terrain
       pub fn check_fragment(
           &self,
           fragment: &FragmentOccupancy,
           position: Vec3,
           rotation: Quat,
       ) -> CollisionResult;
   }
   ```

2. Update `voxel_fragment.rs`:
   - Add `FragmentOccupancy` component
   - New system: `fragment_terrain_collision_system`
     - Uses `WorldOccupancy::check_fragment()`
     - Converts contacts to Rapier `ExternalForce`
   - Keep Rapier cuboid for fragment-vs-fragment broad phase

3. Update `p22_voxel_fragment.rs`:
   - Use new occupancy-based collision
   - Benchmark: spawn 5, 10, 20 fragments, measure FPS
   - Compare to old trimesh (can keep as flag)

4. Tests:
   ```rust
   #[test] fn test_fragment_occupancy_from_world();
   #[test] fn test_fragment_terrain_collision();
   #[test] fn test_fragment_stops_on_terrain();
   ```

**Verification:**
```bash
cargo run --example p22_voxel_fragment --release
# FPS with 10 fragments >= 30
# Fragments land on terrain correctly
# Press key to see benchmark comparison
```

---

## Phase 5: GPU Chunk Texture Upload

**Outcome:** Chunk occupancy data uploaded to GPU texture array. Shader can sample it.

### Tasks

1. Create `crates/studio_core/src/voxel_collision_gpu.rs`:
   ```rust
   pub struct GpuWorldOccupancy {
       chunk_texture_array: Handle<Image>,
       chunk_index_buffer: Buffer,
       loaded_chunks: HashMap<IVec3, u32>,
       free_layers: Vec<u32>,
       max_chunks: u32,
   }
   
   impl GpuWorldOccupancy {
       pub fn new(render_device: &RenderDevice, max_chunks: u32) -> Self;
       pub fn upload_chunk(&mut self, queue: &RenderQueue, coord: IVec3, data: &ChunkOccupancy);
       pub fn remove_chunk(&mut self, coord: IVec3);
       pub fn bind_group_layout() -> BindGroupLayout;
       pub fn bind_group(&self) -> BindGroup;
   }
   ```

2. Create `assets/shaders/test_occupancy.wgsl`:
   ```wgsl
   @group(0) @binding(0) var chunk_textures: texture_2d_array<u32>;
   @group(0) @binding(1) var<storage> chunk_index: array<i32>;
   
   fn is_occupied(world_pos: vec3<i32>) -> bool { ... }
   
   // Test: output 1.0 if position is occupied, 0.0 otherwise
   ```

3. Integration test that uploads chunks and verifies shader reads correctly

**Verification:**
```bash
cargo test -p studio_core gpu_occupancy -- --nocapture
# Test confirms GPU samples match CPU data
```

---

## Phase 6: GPU Collision Compute Shader

**Outcome:** Compute shader checks fragment vs world, outputs collision points. Benchmark shows GPU vs CPU speedup.

### Tasks

1. Create `assets/shaders/voxel_collision.wgsl`:
   ```wgsl
   struct CollisionOutput {
       count: atomic<u32>,
       contacts: array<Contact, 1024>,
   }
   
   @compute @workgroup_size(8, 8, 8)
   fn check_fragment_collision(@builtin(global_invocation_id) id: vec3<u32>) {
       let local_pos = vec3<i32>(id);
       if !fragment_occupied(local_pos) { return; }
       
       let world_pos = transform_point(local_pos, fragment_transform);
       if world_occupied(world_pos) {
           let normal = calculate_normal(world_pos);
           append_contact(world_pos, normal);
       }
   }
   ```

2. Create `GpuCollisionPipeline` in `voxel_collision_gpu.rs`:
   - Manages compute pipeline
   - Uploads fragment texture
   - Dispatches shader
   - Reads back collision results

3. Add GPU path to `fragment_terrain_collision_system`:
   - If GPU available, use `GpuCollisionPipeline`
   - Fall back to CPU if not

4. Benchmark: CPU vs GPU collision for various fragment sizes

**Verification:**
```bash
cargo run --example p22_voxel_fragment --release
# Press key to toggle CPU/GPU collision
# GPU should be faster for large fragments (500+ voxels)
```

---

## Phase 7: Copy/Paste Overlap Preview

**Outcome:** Real-time visualization of overlap when pasting a selection.

### Tasks

1. Add `assets/shaders/paste_preview.wgsl`:
   ```wgsl
   @compute @workgroup_size(8, 8, 8)
   fn check_paste_overlap(...) {
       if selection_occupied(local_pos) && world_occupied(target_pos) {
           output_overlap[local_pos] = 1u;
       }
   }
   ```

2. Add to `voxel_collision_gpu.rs`:
   ```rust
   impl GpuWorldOccupancy {
       pub fn check_paste_overlap(
           &self,
           selection: &FragmentOccupancy,
           target_position: IVec3,
       ) -> OverlapResult;
   }
   ```

3. Create `examples/p24_paste_preview.rs`:
   - Load terrain
   - Select region (creates FragmentOccupancy)
   - Move selection with mouse
   - Overlapping voxels render red, clear voxels render green
   - Real-time update as selection moves

**Verification:**
```bash
cargo run --example p24_paste_preview --release
# Drag selection around
# Red voxels appear where overlap exists
# Smooth real-time update
```

---

## Directory Structure

```
crates/studio_core/src/
├── benchmark.rs           # NEW: shared FPS reporting
├── voxel_collision.rs     # NEW: CPU occupancy + collision
├── voxel_collision_gpu.rs # NEW: GPU textures + compute
├── voxel_fragment.rs      # MODIFIED: use occupancy collision
├── voxel_physics.rs       # KEEP: legacy trimesh (for comparison)
├── lib.rs                 # MODIFIED: exports

assets/shaders/
├── test_occupancy.wgsl    # NEW: verification shader
├── voxel_collision.wgsl   # NEW: fragment collision compute
├── paste_preview.wgsl     # NEW: overlap visualization

examples/
├── p22_voxel_fragment.rs  # MODIFIED: use new system
├── p23_kinematic_controller.rs  # NEW: character controller
├── p24_paste_preview.rs   # NEW: copy/paste preview
```

---

## Phase Dependencies

```
Phase 1 (Benchmark + ChunkOccupancy)
    │
    ▼
Phase 2 (WorldOccupancy + CPU Query)
    │
    ├──────────────────┐
    ▼                  ▼
Phase 3              Phase 4
(Kinematic)          (Fragment + Rapier)
                       │
                       ▼
                   Phase 5 (GPU Upload)
                       │
                       ▼
                   Phase 6 (GPU Collision)
                       │
                       ▼
                   Phase 7 (Paste Preview)
```

Phase 3 and 4 can run in parallel after Phase 2.

---

## Success Metrics

| Metric | Target |
|--------|--------|
| FPS with 10 fragments (CPU) | >= 30 |
| FPS with 10 fragments (GPU) | >= 50 |
| Kinematic controller on terrain | Smooth movement, no clipping |
| Paste preview update | Real-time (<16ms) |
| CPU collision query (1000 voxels) | < 1ms |
| GPU collision query (1000 voxels) | < 0.1ms |

---

## How to Review

1. Run tests: `cargo test -p studio_core`
2. Run p22 example: `cargo run --example p22_voxel_fragment --release`
3. Verify FPS output every 2 seconds
4. Run p23 example: Walk on terrain with controller
5. Run p24 example: Drag selection, verify overlap preview
