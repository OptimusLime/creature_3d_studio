# Unified Physics System - Handoff Document

## Current Status: Phases 1-4 COMPLETE

Phases 1-4 of the unification are done. `VoxelPhysicsWorld` now supports both kinematic (player) and dynamic (fragment) bodies. p22 uses the unified physics system.

**Remaining:** Phase 5 (GPU collision for both) and Phase 6 (cleanup).

## What Was Done

### Phase 1: Extended VoxelPhysicsWorld API (COMPLETE)

Added to `crates/studio_core/src/voxel_physics_world.rs`:

- `BodyKind` enum (`Kinematic`, `Dynamic`)
- `PhysicsBody` struct (renamed from `KinematicBody`) with new fields:
  - `kind: BodyKind`
  - `rotation: Quat`
  - `angular_velocity: Vec3`
  - `occupancy: Option<FragmentOccupancy>`
- `KinematicBody` type alias for backwards compat
- `PhysicsBody::dynamic(position, occupancy)` constructor
- `VoxelPhysicsWorld::get_transform(handle)` method
- Updated exports in `lib.rs`

### Phase 2: Dynamic Body Simulation (COMPLETE)

Added to `crates/studio_core/src/voxel_physics_world.rs`:

- `step_fixed()` now branches on `body.kind`
- `step_kinematic_body()` - extracted existing kinematic logic
- `step_dynamic_body()` - new dynamic simulation with:
  - Gravity application
  - Angular velocity to rotation
  - Fragment occupancy collision via `WorldOccupancy::check_fragment()`
  - Floor contact detection and damping
  - Torque from off-center contacts
  - Angular velocity clamping

New tests added:
- `test_dynamic_body_falls_and_lands`
- `test_dynamic_body_rotation`
- `test_dynamic_body_kind_is_dynamic`
- `test_kinematic_body_kind_is_kinematic`
- `test_get_transform`

### Phase 3: p22 Uses VoxelPhysicsWorld (COMPLETE)

Modified `examples/p22_voxel_fragment.rs`:

- Added `PhysicsWorldRes` resource wrapping `VoxelPhysicsWorld`
- Added `PhysicsBodyHandle` component to track body handles
- Added `UseUnifiedPhysics` resource (toggle with U key)
- When spawning fragments with unified physics:
  - Creates `PhysicsBody::dynamic()` in `VoxelPhysicsWorld`
  - Sets Rapier body to `RigidBody::KinematicPositionBased` (doesn't fight)
  - Stores `PhysicsBodyHandle` on entity
- Added `step_unified_physics` system
- Added `sync_unified_physics_to_transforms` system

### Phase 4: Remove Rapier Terrain Collider (COMPLETE)

Modified `examples/p22_voxel_fragment.rs`:

- Removed `generate_trimesh_collider()` call for terrain
- Terrain entity no longer has `RigidBody::Fixed` or `Collider`
- Fragment-terrain collision now 100% via `VoxelPhysicsWorld`

## What Remains

### Phase 5: GPU Collision for Both Body Types

**Goal:** GPU collision works for both kinematic (p23) and dynamic (p22) bodies through same code path.

**Current state:** GPU collision extracts from `VoxelFragment` components. Need to also support extraction from `VoxelPhysicsWorld` or keep current approach where `sync_unified_physics_to_transforms` updates `Transform` and GPU extraction still works.

**Potential approach:**
1. The current extraction (`collision_extract.rs`) queries `VoxelFragment` + `Transform`
2. Since `sync_unified_physics_to_transforms` updates `Transform` from `VoxelPhysicsWorld`, extraction should already work
3. May need to verify that `VoxelFragment.occupancy` matches `PhysicsBody.occupancy`
4. Add GPU toggle to p23 kinematic controller

**Files to modify:**
- `examples/p23_kinematic_controller.rs` - Add GPU collision toggle (G key)
- `crates/studio_core/src/deferred/collision_extract.rs` - May need updates for kinematic bodies

### Phase 6: Cleanup

**Goal:** Remove dead code, consolidate systems.

**Tasks:**
1. Review `voxel_fragment.rs`:
   - `fragment_terrain_collision_system` - Still used when unified physics disabled
   - `gpu_fragment_terrain_collision_system` - Still used when GPU mode enabled
   - Consider: Keep both for backwards compat? Or remove Rapier path entirely?
2. Update `GPU_COLLISION_DEFICIENCIES.md` to reflect unified system
3. Update `UNIFIED_PHYSICS_PLAN.md` to mark complete

## Key Files

| File | Purpose |
|------|---------|
| `crates/studio_core/src/voxel_physics_world.rs` | **THE** physics system. Has `PhysicsBody`, `BodyKind`, `VoxelPhysicsWorld` |
| `crates/studio_core/src/voxel_collision.rs` | Collision detection. `WorldOccupancy`, `FragmentOccupancy`, `check_aabb()`, `check_fragment()` |
| `crates/studio_core/src/voxel_fragment.rs` | Fragment components + OLD collision systems (may deprecate) |
| `examples/p22_voxel_fragment.rs` | Fragment demo - now uses unified physics |
| `examples/p23_kinematic_controller.rs` | Player controller - uses `VoxelPhysicsWorld` |
| `docs/HOW_WE_WORK.md` | Development process |
| `docs/UNIFIED_PHYSICS_PLAN.md` | The 6-phase plan |

## Commands

```bash
# Build
cargo build -p studio_core

# Test (199 tests should pass)
cargo test -p studio_core --lib

# Run p23 (kinematic controller - must always work)
cargo run --example p23_kinematic_controller

# Run p22 (fragments with unified physics)
cargo run --example p22_voxel_fragment
# Press SPACE to spawn fragment
# Press U to toggle unified physics on/off
# Press G to toggle GPU collision
# Press C to toggle CPU occupancy collision
```

## Architecture After Phases 1-4

```
p23 (kinematic controller):
  VoxelPhysicsWorld
    └── PhysicsBody (kind=Kinematic)
    └── WorldOccupancy.check_aabb()
    └── CPU collision

p22 (fragments):
  VoxelPhysicsWorld (when UseUnifiedPhysics=true)
    └── PhysicsBody (kind=Dynamic)
    └── WorldOccupancy.check_fragment()
    └── CPU collision
    └── Rapier for visual entity (KinematicPositionBased)
  
  OR old Rapier path (when UseUnifiedPhysics=false)
    └── Rapier RigidBody::Dynamic
    └── VoxelFragmentPlugin collision systems
```

## Test Count

- Before: 194 tests
- After: 199 tests (+5 for dynamic body tests)

All tests pass: `cargo test -p studio_core --lib`

## Notes for Next Session

1. **GPU collision should work already** for p22 because:
   - `VoxelFragment` component still exists on entities
   - `Transform` is synced from `VoxelPhysicsWorld`
   - GPU extraction queries `VoxelFragment` + `Transform`
   
2. **p23 doesn't have GPU collision yet** because:
   - No `VoxelFragment` component on player
   - Would need to add extraction path for kinematic bodies
   - Or add a placeholder `VoxelFragment` component

3. **Consider removing old Rapier path** in Phase 6:
   - Would simplify codebase significantly
   - But breaks backwards compat if anyone uses `UseUnifiedPhysics(false)`

4. **Missing: `VoxelPhysicsWorld::remove_body()`** method:
   - Currently when fragments are despawned, body stays in physics world
   - Not critical (bodies are just orphaned) but wasteful
   - Add `remove_body(handle)` method in cleanup phase
