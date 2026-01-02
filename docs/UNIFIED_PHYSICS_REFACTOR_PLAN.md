# Unified Physics Refactor Plan

## Summary

Delete the custom `VoxelPhysicsWorld` physics engine and unify p22 (dynamic fragments) and p23 (kinematic controller) to use a single architecture: **GPU collision detection → Rapier physics integration**.

## Context & Motivation

We have two broken parallel paths:
1. **GPU path**: Works for p22 dynamic fragments when `UseUnifiedPhysics` is OFF
2. **CPU path**: `VoxelPhysicsWorld` with custom collision - broken for dynamic bodies, works for kinematic

The CPU path (`VoxelPhysicsWorld`) reimplements physics poorly when Rapier already handles this correctly. The terrain is too large for CPU collision detection, so GPU collision is mandatory. There is no reason to have custom physics integration when Rapier exists.

**Target architecture (one path):**
```
Entity with Collider + RigidBody
    + VoxelFragment (fragments) OR GpuCollisionAABB (characters)
              ↓
GPU collision detection (compute shader)
              ↓
GpuCollisionContacts (readback to CPU)
              ↓
Single system applies ExternalForce/ExternalImpulse
              ↓
Rapier integrates physics (handles both Dynamic and KinematicPositionBased)
```

## Naming Conventions

- `gpu_collision_response_system` - the unified system applying GPU results to Rapier
- `GpuCollisionAABB` - marker for AABB-based GPU collision (kinematic characters)
- `VoxelFragment` - marker for voxel-based GPU collision (dynamic fragments)

## Phases

### Phase 1: Delete Custom Physics Engine

**Outcome:** `VoxelPhysicsWorld` and all custom physics types removed from codebase.

**Verification:** `cargo build -p studio_core` compiles. Test count may decrease (custom physics tests deleted).

**Tasks:**
1. Delete `crates/studio_core/src/voxel_physics_world.rs` entirely
2. Remove `VoxelPhysicsWorld`, `PhysicsBody`, `KinematicBody`, `BodyHandle`, `PhysicsConfig`, `BodyKind` exports from `crates/studio_core/src/lib.rs`
3. Remove CPU-only collision helpers from `crates/studio_core/src/voxel_collision.rs`:
   - `KinematicController` struct and impl
   - `CollisionResult` (used by check_aabb for CPU)
   - `check_aabb` method on `WorldOccupancy` (keep `check_fragment` if GPU needs reference, or delete if not)
4. Keep: `WorldOccupancy`, `ChunkOccupancy`, `FragmentOccupancy`, `FragmentCollisionResult`, `GpuCollisionAABB` - these are used by GPU path
5. Update any remaining imports in lib.rs

### Phase 2: Refactor p23 to Rapier KinematicCharacterController

**Outcome:** p23 uses Rapier's `KinematicCharacterController` with GPU collision providing terrain interaction via `ExternalForce`.

**Verification:** `cargo run --example p23_kinematic_controller` - player spawns, falls, lands on terrain, WASD movement works, spacebar jumps.

**Tasks:**
1. Rewrite `examples/p23_kinematic_controller.rs`:
   - Remove all `VoxelPhysicsWorld` usage
   - Remove `PhysicsWorld` resource wrapper
   - Remove `PlayerBodyHandle` resource
   - Add Rapier components: `RigidBody::KinematicPositionBased`, `Collider::capsule()`, `KinematicCharacterController`
   - Keep `GpuCollisionAABB` component for GPU collision extraction
   - Player input sets `KinematicCharacterController.translation`
   - Remove custom `physics_step` and `sync_transforms` systems
2. Ensure `GpuCollisionMode` resource exists (default enabled)
3. GPU collision results apply `ExternalForce` to push player out of terrain

### Phase 3: Unify GPU Collision Response System

**Outcome:** Single system `gpu_collision_response_system` handles both `VoxelFragment` (dynamic) and `GpuCollisionAABB` (kinematic) entities.

**Verification:** Both p22 and p23 receive GPU collision forces. Add debug logging showing contact counts for both entity types.

**Tasks:**
1. Rename `gpu_fragment_terrain_collision_system` to `gpu_collision_response_system` in `crates/studio_core/src/voxel_fragment.rs`
2. Modify system to query both:
   - `Query<(Entity, &VoxelFragment, &mut Velocity, &mut ExternalForce), With<RigidBody>>`
   - `Query<(Entity, &GpuCollisionAABB, &mut KinematicCharacterController), With<RigidBody>>`
3. For dynamic bodies (`VoxelFragment`): apply `ExternalForce` as before
4. For kinematic bodies (`GpuCollisionAABB`): apply position correction via controller
5. Update `collision_extract.rs` to extract both `VoxelFragment` and `GpuCollisionAABB` entities (may already do this)
6. Export renamed system from `lib.rs`

### Phase 4: Clean Up p22

**Outcome:** p22 uses only the unified GPU → Rapier path. All broken toggles removed.

**Verification:** `cargo run --example p22_voxel_fragment` - press SPACE, fragment spawns, falls, lands on terrain, does not fall through.

**Tasks:**
1. Remove from `examples/p22_voxel_fragment.rs`:
   - `UseUnifiedPhysics` resource and toggle
   - `PhysicsWorldRes` resource  
   - `PhysicsBodyHandle` component
   - `step_unified_physics` system
   - `sync_unified_physics_to_transforms` system
   - `toggle_unified_physics` system
   - All imports of deleted types
2. Fragments spawn with only Rapier components + `VoxelFragment`
3. GPU collision enabled by default (no toggle needed, it's the only path)
4. Remove or simplify G key toggle (GPU collision is mandatory now)

### Phase 5: Final Verification & Cleanup

**Outcome:** Both examples work correctly, tests pass, no dead code.

**Verification:**
1. `cargo test -p studio_core` - all tests pass
2. `cargo run --example p22_voxel_fragment` - fragments land on terrain
3. `cargo run --example p23_kinematic_controller` - player walks on terrain
4. `cargo clippy -p studio_core` - no warnings about unused code

**Tasks:**
1. Run full test suite, fix any failures
2. Run both examples, verify behavior
3. Delete any orphaned test files for removed code
4. Update `docs/UNIFIED_PHYSICS_PLAN.md` to mark complete
5. Delete `docs/UNIFIED_PHYSICS_HANDOFF.md` (no longer relevant)

## Full Outcome

After all phases:
- One physics path: GPU collision → Rapier
- p22 dynamic fragments work
- p23 kinematic controller works  
- No custom physics engine code
- Clean, maintainable architecture

## Files Modified/Deleted

**Deleted:**
- `crates/studio_core/src/voxel_physics_world.rs`

**Modified:**
- `crates/studio_core/src/lib.rs` - remove exports
- `crates/studio_core/src/voxel_collision.rs` - remove CPU collision helpers
- `crates/studio_core/src/voxel_fragment.rs` - rename/generalize collision system
- `examples/p22_voxel_fragment.rs` - remove unified physics code
- `examples/p23_kinematic_controller.rs` - full rewrite to Rapier

## How to Review

1. Phase 1: Verify deleted files/code, check build passes
2. Phase 2: Run p23, verify player movement works
3. Phase 3: Check unified system handles both entity types
4. Phase 4: Run p22, verify fragments don't fall through
5. Phase 5: Run test suite, check for dead code warnings
