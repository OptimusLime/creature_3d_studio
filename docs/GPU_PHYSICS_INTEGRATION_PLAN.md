# GPU Physics Integration Plan

## Summary

Extend our existing GPU terrain collision system to support fragment-to-fragment collision, moving from "terrain-only collision" to "full world collision" while keeping terrain collision exactly as it works today.

**Key Insight:** Our terrain occupancy grid and gpu-physics-unity's spatial hash grid serve DIFFERENT purposes and should COEXIST:

| Grid | Purpose | Contents | When Used |
|------|---------|----------|-----------|
| **Terrain Occupancy** (existing) | "Is there terrain here?" | Bit-packed voxel data | Fragment-vs-terrain collision |
| **Spatial Hash Grid** (new) | "What fragments are near me?" | Fragment particle IDs | Fragment-vs-fragment collision |

We are NOT replacing our occupancy grid. We are ADDING a spatial hash grid alongside it.

---

## Current System (What We Have)

### Data Flow Today

```
┌─────────────────────────────────────────────────────────────────────┐
│                        CPU (Bevy World)                              │
│  ┌──────────────────┐    ┌──────────────────┐                       │
│  │ VoxelWorld       │    │ VoxelFragment    │                       │
│  │ (terrain voxels) │    │ (dynamic bodies) │                       │
│  └────────┬─────────┘    └────────┬─────────┘                       │
│           │                       │                                  │
│           ▼                       ▼                                  │
│  ┌──────────────────┐    ┌──────────────────┐                       │
│  │ WorldOccupancy   │    │ FragmentOccupancy│                       │
│  │ (CPU bit-packed) │    │ (per-body voxels)│                       │
│  └────────┬─────────┘    └────────┬─────────┘                       │
└───────────┼──────────────────────┼──────────────────────────────────┘
            │                      │
            │    GPU UPLOAD        │
            ▼                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│                           GPU                                        │
│  ┌──────────────────┐    ┌──────────────────┐                       │
│  │ chunk_textures   │    │ fragments[]      │                       │
│  │ (terrain occup.) │    │ fragment_occup[] │                       │
│  └────────┬─────────┘    └────────┬─────────┘                       │
│           │                       │                                  │
│           └───────────┬───────────┘                                  │
│                       ▼                                              │
│           ┌───────────────────────┐                                  │
│           │ voxel_collision.wgsl  │                                  │
│           │ (fragment vs terrain) │                                  │
│           └───────────┬───────────┘                                  │
│                       │                                              │
│                       ▼                                              │
│           ┌───────────────────────┐                                  │
│           │ contacts[] (output)   │                                  │
│           │ - position, normal    │                                  │
│           │ - penetration         │                                  │
│           └───────────┬───────────┘                                  │
└───────────────────────┼─────────────────────────────────────────────┘
                        │
                        │ GPU READBACK
                        ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        CPU (Physics Response)                        │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ gpu_kinematic_collision_system / gpu_fragment_collision_sys  │   │
│  │ - Reads contacts from GPU                                    │   │
│  │ - Applies position correction                                │   │
│  │ - Updates velocity                                           │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

### What Works Today

1. **Terrain occupancy on GPU** - `chunk_textures` texture array, O(1) lookup
2. **Fragment-vs-terrain collision** - Each fragment voxel checks terrain occupancy
3. **Contact generation** - GPU outputs contacts with position, normal, penetration
4. **CPU physics response** - Reads contacts, applies corrections

### What's Missing

1. **Fragment-vs-fragment collision** - Fragments pass through each other
2. **Physics integration on GPU** - Position/velocity updates happen on CPU
3. **Instanced rendering from GPU** - We read back positions for rendering

---

## Target System (What We're Building)

### Data Flow After Integration

```
┌─────────────────────────────────────────────────────────────────────┐
│                           GPU                                        │
│                                                                      │
│  TERRAIN OCCUPANCY (unchanged)     FRAGMENT PHYSICS STATE (new)     │
│  ┌──────────────────┐              ┌──────────────────────────────┐ │
│  │ chunk_textures   │              │ positions[]      (float3)    │ │
│  │ chunk_index[]    │              │ rotations[]      (float4)    │ │
│  └────────┬─────────┘              │ velocities[]     (float3)    │ │
│           │                        │ angular_vel[]    (float3)    │ │
│           │                        └──────────────┬───────────────┘ │
│           │                                       │                  │
│           │                        ┌──────────────▼───────────────┐ │
│           │                        │ STEP 1: Generate Particles   │ │
│           │                        │ (surface points of each body)│ │
│           │                        └──────────────┬───────────────┘ │
│           │                                       │                  │
│           │                        ┌──────────────▼───────────────┐ │
│           │                        │ STEP 2: Populate Spatial Hash│ │
│           │                        │ (fragment particles only)    │ │
│           │                        └──────────────┬───────────────┘ │
│           │                                       │                  │
│           │                        SPATIAL HASH GRID (new)          │
│           │                        ┌──────────────────────────────┐ │
│           │                        │ hash_grid[] (int4 per cell)  │ │
│           │                        │ stores: particle IDs         │ │
│           │                        └──────────────┬───────────────┘ │
│           │                                       │                  │
│           └─────────────┬─────────────────────────┘                  │
│                         │                                            │
│           ┌─────────────▼─────────────────────────────────────────┐ │
│           │ STEP 3: Collision Detection                           │ │
│           │ - For each fragment particle:                         │ │
│           │   - Check terrain occupancy (EXISTING CODE)           │ │
│           │   - Check spatial hash for other particles (NEW)      │ │
│           │   - Compute forces (spring + damping)                 │ │
│           └─────────────┬─────────────────────────────────────────┘ │
│                         │                                            │
│           ┌─────────────▼─────────────────────────────────────────┐ │
│           │ STEP 4: Aggregate Forces → Rigid Body Momenta         │ │
│           │ - Sum particle forces → linear force                  │ │
│           │ - Sum r × F → torque                                  │ │
│           └─────────────┬─────────────────────────────────────────┘ │
│                         │                                            │
│           ┌─────────────▼─────────────────────────────────────────┐ │
│           │ STEP 5: Integrate Position/Rotation                   │ │
│           │ - position += velocity * dt                           │ │
│           │ - rotation = integrate(angular_velocity)              │ │
│           └─────────────┬─────────────────────────────────────────┘ │
│                         │                                            │
│           ┌─────────────▼─────────────────────────────────────────┐ │
│           │ Instanced Rendering (reads positions/rotations)       │ │
│           └───────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### Key Differences from gpu-physics-unity

| Aspect | gpu-physics-unity | Our System |
|--------|-------------------|------------|
| Terrain collision | Ground plane only | Full voxel terrain via occupancy texture |
| Fragment collision | Spatial hash only | Spatial hash + terrain occupancy |
| Body representation | Uniform cubes | Arbitrary voxel shapes |
| Physics integration | GPU only | Currently CPU, moving to GPU |

---

## Phases

### Phase 1: Fragment-to-Fragment Detection in Existing Shader

**Outcome:** Two fragments collide with each other (not just terrain). Detection only - no physics response changes.

**Verification:**
```bash
cargo run --example p22_voxel_fragment
# Press SPACE twice to spawn two fragments above each other
# Console shows: "Fragment 0 contacts: 4 (terrain: 2, fragment: 2)"
# The "fragment: 2" part is NEW - proves fragment-fragment detection works
```

**Tasks:**

1. Add spatial hash grid buffer to `collision_prepare.rs`:
   - `hash_grid: Buffer` - int4 per cell, stores particle IDs
   - Grid covers same bounds as terrain chunks
   - Cell size = 1.0 (same as voxel size)

2. Add `clear_hash_grid` kernel to `voxel_collision.wgsl`:
   ```wgsl
   @compute @workgroup_size(64, 1, 1)
   fn clear_hash_grid(@builtin(global_invocation_id) id: vec3<u32>) {
       hash_grid[id.x] = vec4(-1, -1, -1, -1);
   }
   ```

3. Add `populate_hash_grid` kernel:
   ```wgsl
   @compute @workgroup_size(8, 8, 1)
   fn populate_hash_grid(...) {
       // Same fragment voxel iteration as main collision kernel
       // But instead of checking terrain, INSERT into hash_grid
       let cell_idx = world_to_grid(world_pos);
       atomicCompareExchange(&hash_grid[cell_idx].x, -1, particle_id, ...);
   }
   ```

4. Modify main collision kernel to ALSO check hash_grid:
   ```wgsl
   // Existing terrain check
   if is_terrain_occupied(voxel_pos) { ... }
   
   // NEW: Check spatial hash for other fragment particles
   for neighbor in check_27_neighbors(world_pos) {
       if neighbor.fragment_index != my_fragment_index {
           // Collision with different fragment!
           emit_contact(neighbor, ...);
       }
   }
   ```

5. Update dispatch order in `collision_node.rs`:
   - Dispatch `clear_hash_grid`
   - Dispatch `populate_hash_grid` (all fragments)
   - Dispatch `collision_detection` (all fragments)

6. Add contact type to `Contact` struct:
   ```wgsl
   struct Contact {
       position: vec3<f32>,
       penetration: f32,
       normal: vec3<f32>,
       fragment_index: u32,
       contact_type: u32,  // 0 = terrain, 1 = fragment
       other_fragment: u32, // for fragment contacts
   }
   ```

**Files Modified:**
- `assets/shaders/voxel_collision.wgsl`
- `crates/studio_core/src/deferred/collision_prepare.rs`
- `crates/studio_core/src/deferred/collision_node.rs`
- `crates/studio_core/src/voxel_collision_gpu.rs` (Contact struct)

---

### Phase 2: Fragment-to-Fragment Physics Response

**Outcome:** Fragments bounce off each other, not just terrain. Uses spring-damper model from gpu-physics-unity.

**Verification:**
```bash
cargo run --example p22_voxel_fragment  
# Spawn fragment above another
# Top fragment bounces off bottom fragment (not passing through)
# Both fragments eventually settle on terrain
```

**Tasks:**

1. Add collision parameters to uniforms:
   ```rust
   pub struct CollisionUniforms {
       // ... existing ...
       pub spring_k: f32,       // Repulsive spring constant
       pub damping_k: f32,      // Velocity damping
       pub friction_k: f32,     // Tangential friction
   }
   ```

2. Modify `gpu_fragment_terrain_collision_system` to handle fragment contacts:
   ```rust
   for contact in contacts_for_fragment(idx) {
       match contact.contact_type {
           ContactType::Terrain => {
               // Existing terrain response
               resolution += contact.normal * contact.penetration;
           }
           ContactType::Fragment => {
               // NEW: Spring-damper response
               let rel_vel = my_velocity - other_velocity;
               let spring_force = -spring_k * contact.penetration * contact.normal;
               let damping_force = damping_k * rel_vel;
               force += spring_force + damping_force;
           }
       }
   }
   ```

3. Apply forces to fragment velocity in existing system.

**Files Modified:**
- `crates/studio_core/src/voxel_fragment.rs`
- `crates/studio_core/src/voxel_collision_gpu.rs`

---

### Phase 3: GPU Physics Integration (Position/Velocity on GPU)

**Outcome:** Fragment positions and velocities live on GPU. No per-frame readback for physics - only for debug/special cases.

**Verification:**
```bash
cargo run --example p22_voxel_fragment
# Spawn 100 fragments
# FPS stays > 60 (no CPU bottleneck)
# Profiler shows: no GPU→CPU readback in frame
```

**Tasks:**

1. Add physics state buffers to `GpuPhysicsState`:
   ```rust
   pub struct GpuPhysicsState {
       pub positions: Buffer,         // float3 × N
       pub rotations: Buffer,         // float4 × N  
       pub velocities: Buffer,        // float3 × N
       pub angular_velocities: Buffer, // float3 × N
   }
   ```

2. Add integration kernel:
   ```wgsl
   @compute @workgroup_size(64, 1, 1)
   fn integrate(@builtin(global_invocation_id) id: vec3<u32>) {
       velocities[id.x].y -= gravity * dt;
       positions[id.x] += velocities[id.x] * dt;
       // Quaternion integration for rotation
   }
   ```

3. Modify collision detection to write forces instead of contacts:
   - Output: `particle_forces[]` buffer
   - Each particle gets accumulated force from terrain + fragment collisions

4. Add momenta aggregation kernel:
   ```wgsl
   @compute @workgroup_size(64, 1, 1)
   fn aggregate_momenta(@builtin(global_invocation_id) id: vec3<u32>) {
       let body_idx = id.x;
       var linear_force = vec3(0.0);
       var torque = vec3(0.0);
       
       for particle in body_particles(body_idx) {
           linear_force += particle_forces[particle];
           torque += cross(particle_offset, particle_forces[particle]);
       }
       
       velocities[body_idx] += linear_force / mass * dt;
       angular_velocities[body_idx] += torque * dt;
   }
   ```

5. Update dispatch order:
   - `clear_hash_grid`
   - `generate_particles` (transform to world space)
   - `populate_hash_grid`
   - `detect_collisions` (outputs forces, not contacts)
   - `aggregate_momenta`
   - `integrate`

**Files Modified:**
- `crates/studio_core/src/deferred/gpu_physics_state.rs` (new)
- `assets/shaders/physics_integrate.wgsl` (new)
- `assets/shaders/voxel_collision.wgsl` (modify to output forces)

---

### Phase 4: Instanced Rendering from GPU Buffers

**Outcome:** Fragment rendering reads transforms directly from GPU. Zero CPU involvement in render transform.

**Verification:**
```bash
cargo run --example p22_voxel_fragment
# Spawn 1000 fragments
# FPS > 30
# Profiler: 0 bytes GPU→CPU for rendering
```

**Tasks:**

1. Create instanced vertex shader:
   ```wgsl
   @vertex
   fn vertex_main(
       @builtin(instance_index) instance: u32,
       @location(0) position: vec3<f32>,
   ) -> VertexOutput {
       let body_pos = positions[instance];
       let body_rot = rotations[instance];
       let world_pos = body_pos + quat_rotate(body_rot, position);
       // ... projection
   }
   ```

2. Use `DrawMeshInstancedIndirect` for fragment rendering.

3. Remove CPU transform sync systems.

**Files Modified:**
- `assets/shaders/fragment_instanced.wgsl` (new)
- `crates/studio_core/src/voxel_fragment.rs` (remove sync)

---

## Why This Order?

**Phase 1 first** because:
- It validates the spatial hash grid works
- Uses our EXISTING shader infrastructure
- Minimal code change (add grid, modify one kernel)
- Immediate visible result: fragments collide!

**Phase 2 second** because:
- Builds directly on Phase 1's detection
- Still uses CPU physics (familiar code)
- Proves the collision model works before GPU-ifying

**Phase 3 third** because:
- Only NOW do we move physics to GPU
- We've already validated collision works
- Performance gain is measurable

**Phase 4 last** because:
- It's optimization, not functionality
- Requires Phase 3's GPU state

---

## Grid Comparison Summary

### Terrain Occupancy Grid (KEEP - unchanged)
- **Data:** Bit-packed voxel occupancy per chunk
- **Format:** 2D texture array, R32Uint, 32x32 per layer
- **Lookup:** `is_terrain_occupied(world_pos)` → bool
- **Purpose:** "Is there solid terrain at this position?"
- **Updates:** Only when terrain changes (rare)

### Spatial Hash Grid (NEW - for fragments)
- **Data:** Particle IDs in each cell
- **Format:** Buffer of int4, one per cell
- **Lookup:** `get_particles_in_cell(cell)` → up to 4 particle IDs
- **Purpose:** "What fragment particles are near this position?"
- **Updates:** Every frame (fragments move)

### Why Both?

Terrain is STATIC and DENSE. Bit-packing is perfect.
Fragments are DYNAMIC and SPARSE. Spatial hash is perfect.

Trying to use one structure for both would be:
- Wasteful (storing empty fragment space in terrain grid)
- Slow (rebuilding terrain grid every frame for fragment movement)
- Complex (mixing static/dynamic data)

---

## Success Criteria

| Phase | Criterion | How to Verify |
|-------|-----------|---------------|
| 1 | Fragments detect collision with each other | Console log shows "fragment contacts" |
| 2 | Fragments bounce off each other | Visual: no interpenetration |
| 3 | 100 fragments at 60fps | Profiler: no GPU→CPU sync |
| 4 | 1000 fragments at 30fps | Profiler: 0 bytes render readback |

---

## References

- `docs/research/gpu-physics-unity-analysis.md` - Detailed algorithm analysis
- `assets/shaders/voxel_collision.wgsl` - Our existing GPU collision shader
- `crates/studio_core/src/voxel_collision.rs` - Our occupancy data structures
