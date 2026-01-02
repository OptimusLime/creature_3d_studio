# Unified Physics System Plan

## Status

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1 | COMPLETE | API extended with BodyKind, PhysicsBody, rotation fields |
| Phase 2 | COMPLETE | Dynamic body simulation with rotation and fragment collision |
| Phase 3 | COMPLETE | p22 uses VoxelPhysicsWorld (bridge mode with Rapier kinematic) |
| Phase 4 | COMPLETE | Removed terrain trimesh collider from p22 |
| Phase 5 | PENDING | GPU collision for both body types |
| Phase 6 | PENDING | Cleanup dead code |

**See `UNIFIED_PHYSICS_HANDOFF.md` for detailed current state and next steps.**

---

## Summary

Unify `VoxelPhysicsWorld` (p23) and `VoxelFragment` (p22) into a single physics system. Both examples will use the same `VoxelPhysicsWorld` API. GPU collision will work for both kinematic and dynamic bodies.

## Problem

We have two separate physics systems:

| System | File | Backend | Bodies | Collision |
|--------|------|---------|--------|-----------|
| p22 fragments | `voxel_fragment.rs` | Rapier | Dynamic (rotation) | Rapier trimesh + CPU/GPU occupancy |
| p23 controller | `voxel_physics_world.rs` | Custom | Kinematic (no rotation) | CPU occupancy only |

This is stupid. Two physics backends, two collision paths, duplicated code, hidden carve-outs.

## Target State

| System | Backend | Bodies | Collision |
|--------|---------|--------|-----------|
| All examples | `VoxelPhysicsWorld` | Kinematic OR Dynamic | CPU or GPU occupancy |

One physics system. One collision system. Shared code.

---

## Phase 1: Extend VoxelPhysicsWorld API (Facade Only)

**Outcome:** `VoxelPhysicsWorld` has API to support dynamic bodies with rotation. No behavior change yet - just the interface exists and compiles.

**Verification:** 
```bash
cargo test -p studio_core --lib
cargo run --example p23_kinematic_controller
# p23 works exactly as before
```

**Tasks:**

1. Add `BodyKind` enum to `voxel_physics_world.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Kinematic,  // No rotation, player-style movement
    Dynamic,    // Has rotation, fragment-style tumbling
}
```

2. Rename `KinematicBody` to `PhysicsBody`, add fields:
```rust
pub struct PhysicsBody {
    pub kind: BodyKind,
    pub position: Vec3,
    pub rotation: Quat,           // Identity for Kinematic
    pub velocity: Vec3,
    pub angular_velocity: Vec3,   // Zero for Kinematic
    pub half_extents: Vec3,
    pub grounded: bool,
    pub ground_normal: Vec3,
    // For Dynamic bodies with voxel occupancy:
    pub occupancy: Option<FragmentOccupancy>,
    // ... existing fields
}
```

3. Add builder methods:
```rust
impl PhysicsBody {
    pub fn kinematic(position: Vec3, half_extents: Vec3) -> Self { ... }
    pub fn dynamic(position: Vec3, occupancy: FragmentOccupancy) -> Self { ... }
}
```

4. Add transform getter:
```rust
impl VoxelPhysicsWorld {
    pub fn get_transform(&self, handle: BodyHandle) -> Option<(Vec3, Quat)> { ... }
}
```

5. Keep old `KinematicBody` as type alias for backwards compat:
```rust
pub type KinematicBody = PhysicsBody;
```

6. Verify p23 compiles and runs unchanged.

---

## Phase 2: Implement Dynamic Body Simulation

**Outcome:** `VoxelPhysicsWorld.step()` correctly simulates dynamic bodies - gravity, rotation, collision with terrain.

**Verification:**
```bash
cargo test -p studio_core --lib
# New tests pass:
# - test_dynamic_body_falls_and_lands
# - test_dynamic_body_rotation
```

**Tasks:**

1. In `step_fixed()`, branch on `body.kind`:
   - `Kinematic`: existing logic (no rotation)
   - `Dynamic`: new logic with rotation

2. For `Dynamic` bodies in `step_fixed()`:
```rust
// Apply gravity
body.velocity += gravity * dt;

// Apply angular velocity to rotation
if body.angular_velocity.length_squared() > 0.0001 {
    let angle = body.angular_velocity.length() * dt;
    let axis = body.angular_velocity.normalize();
    body.rotation = Quat::from_axis_angle(axis, angle) * body.rotation;
    body.rotation = body.rotation.normalize();
}

// Move and collide using fragment occupancy
if let Some(ref occupancy) = body.occupancy {
    let result = self.occupancy.check_fragment(occupancy, body.position, body.rotation);
    // Apply resolution, damping, torque from off-center contacts
}
```

3. Add collision response for dynamic bodies:
   - Resolution vector pushes body out
   - Off-center contacts apply torque
   - Floor contact dampens velocity

4. Add unit tests in `voxel_physics_world.rs`:
```rust
#[test]
fn test_dynamic_body_falls_and_lands() {
    // Create floor, add dynamic body above it, step, verify lands
}

#[test]
fn test_dynamic_body_rotation() {
    // Verify angular_velocity causes rotation over time
}
```

---

## Phase 3: p22 Uses VoxelPhysicsWorld (Bridge)

**Outcome:** p22 creates fragments via `VoxelPhysicsWorld`. Rapier still present for rendering transform sync. Both systems run in parallel temporarily.

**Verification:**
```bash
cargo run --example p22_voxel_fragment
# Press SPACE - fragment spawns
# Fragment falls and lands on terrain
# Collision works (from VoxelPhysicsWorld, not Rapier)
```

**Tasks:**

1. Add `VoxelPhysicsWorld` resource to p22:
```rust
.insert_resource(PhysicsWorldRes(VoxelPhysicsWorld::new(occupancy, config)))
```

2. When spawning fragment, add to both systems:
```rust
// Add to VoxelPhysicsWorld
let body_handle = physics.add_body(PhysicsBody::dynamic(position, occupancy));

// Still spawn Rapier entity for transform sync
commands.spawn(VoxelFragmentBundle { ... })
    .insert(PhysicsBodyHandle(body_handle));
```

3. Add sync system that copies VoxelPhysicsWorld state to Rapier:
```rust
fn sync_physics_to_rapier(
    physics: Res<PhysicsWorldRes>,
    mut query: Query<(&PhysicsBodyHandle, &mut Transform), With<VoxelFragment>>,
) {
    for (handle, mut transform) in query.iter_mut() {
        if let Some((pos, rot)) = physics.0.get_transform(handle.0) {
            transform.translation = pos;
            transform.rotation = rot;
        }
    }
}
```

4. Disable Rapier physics on fragments (just visual):
```rust
// Change from RigidBody::Dynamic to RigidBody::KinematicPositionBased
// So Rapier doesn't fight with VoxelPhysicsWorld
```

---

## Phase 4: Remove Rapier from Terrain Collision

**Outcome:** p22 uses only `VoxelPhysicsWorld` for collision. No Rapier trimesh for terrain.

**Verification:**
```bash
cargo run --example p22_voxel_fragment
# Same behavior as Phase 3
# RapierDebugRenderPlugin shows NO terrain collider wireframe
```

**Tasks:**

1. Remove terrain collider generation in p22:
```rust
// DELETE: let terrain_collider = generate_trimesh_collider(&terrain);
// DELETE: .insert(terrain_collider) on terrain entity
```

2. Remove Rapier collider from fragment spawning:
```rust
// Fragments no longer need Rapier Collider component
// They're just visual entities with Transform synced from VoxelPhysicsWorld
```

3. Optionally remove `bevy_rapier3d` dependency entirely from p22.

---

## Phase 5: GPU Collision for Both Body Types

**Outcome:** GPU collision works for both kinematic (p23) and dynamic (p22) bodies through same code path.

**Verification:**
```bash
cargo run --example p22_voxel_fragment
# Press G - GPU collision enabled
# Fragments still collide correctly

cargo run --example p23_kinematic_controller  
# Press G - GPU collision enabled (new feature)
# Player still collides correctly
```

**Tasks:**

1. The GPU shader already handles rotation - no shader changes needed.

2. Modify extraction to pull from `VoxelPhysicsWorld` instead of separate queries:
```rust
// collision_extract.rs
fn extract_bodies_system(
    physics: Extract<Option<Res<PhysicsWorldRes>>>,
    mut extracted: ResMut<ExtractedBodies>,
) {
    // Extract ALL bodies from VoxelPhysicsWorld
    // Both Kinematic and Dynamic go through same path
}
```

3. Add GPU collision toggle to p23:
```rust
fn toggle_gpu_collision(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut gpu_mode: ResMut<GpuCollisionMode>,
) {
    if keyboard.just_pressed(KeyCode::KeyG) {
        gpu_mode.enabled = !gpu_mode.enabled;
    }
}
```

4. For kinematic bodies, pass `Quat::IDENTITY` as rotation, `None` as occupancy (use AABB).

---

## Phase 6: Cleanup

**Outcome:** Single physics path. No dead code. Clean.

**Verification:**
```bash
cargo test -p studio_core --lib  # All pass
cargo build -p studio_core       # No warnings
cargo run --example p22_voxel_fragment  # Works
cargo run --example p23_kinematic_controller  # Works
```

**Tasks:**

1. Remove dead Rapier collision code from `voxel_fragment.rs`
2. Remove `fragment_terrain_collision_system` (replaced by VoxelPhysicsWorld)
3. Remove `gpu_fragment_terrain_collision_system` (replaced by unified system)
4. Update `VoxelFragmentPlugin` to not add duplicate systems
5. Update `GPU_COLLISION_DEFICIENCIES.md` to reflect unified system
6. Delete this plan doc or mark complete

---

## Files Modified Per Phase

| Phase | Files |
|-------|-------|
| 1 | `voxel_physics_world.rs` |
| 2 | `voxel_physics_world.rs` |
| 3 | `p22_voxel_fragment.rs` |
| 4 | `p22_voxel_fragment.rs` |
| 5 | `collision_extract.rs`, `p23_kinematic_controller.rs` |
| 6 | `voxel_fragment.rs`, `GPU_COLLISION_DEFICIENCIES.md` |

---

## Why GPU Collision Already Works for Both

The shader receives:
- `position: vec3<f32>` - body center
- `rotation: vec4<f32>` - quaternion
- `size: vec3<u32>` - AABB or occupancy bounds
- `occupancy_offset/size` - bit-packed voxel data

For kinematic bodies:
- `rotation = (0, 0, 0, 1)` (identity)
- `occupancy = None` (treat as solid AABB)

For dynamic bodies:
- `rotation = actual rotation`
- `occupancy = fragment voxel data`

Same shader code handles both. The dispatch in `collision_node.rs` just needs to extract from `VoxelPhysicsWorld` instead of having two separate extraction paths.

---

## Success Criteria

After all phases:

1. `VoxelPhysicsWorld` is THE physics system
2. Both p22 and p23 use identical physics API
3. GPU collision optional for both
4. No Rapier for voxel-terrain collision
5. No duplicate systems
6. All tests pass
