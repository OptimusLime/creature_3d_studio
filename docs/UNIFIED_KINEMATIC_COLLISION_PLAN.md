# Unified Kinematic Collision Plan

## Summary

Fix p23 to use the unified GPU collision pipeline instead of the separate CPU-based `KinematicController`. Both dynamic fragments (p22) and kinematic characters (p23) must use the same path: GPU collision detection → Rapier physics integration.

## Context & Motivation

Currently we have two collision paths:
- **p22 (fragments)**: GPU collision → Rapier dynamics (CORRECT)
- **p23 (kinematic)**: CPU `KinematicController` with `WorldOccupancy::check_aabb()` (WRONG)

This violates the unified architecture. p23 should use `GpuCollisionAABB` component and have GPU collision results applied to a Rapier kinematic body.

## Current State (Updated)

### What exists:
1. `GpuCollisionAABB` component in `voxel_collision.rs` - marks entities for GPU collision
2. `collision_extract.rs` - extracts both `VoxelFragment` AND `GpuCollisionAABB` entities to render world
3. `gpu_fragment_terrain_collision_system` in `voxel_fragment.rs` - applies GPU collision results to fragments
4. `gpu_kinematic_collision_system` in `voxel_fragment.rs` - applies GPU collision results to kinematic bodies (ADDED)
5. `GpuCollisionContacts` resource - holds per-entity collision contacts from GPU

### Current Issue:
The GPU collision compute shader is NOT generating contacts for `GpuCollisionAABB` entities.
Debug logging shows:
- Entity extraction works: "Extracted 1 entities for GPU collision (0 fragments, 1 AABBs)"
- But contacts are 0: "gpu_kinematic_collision: 0 contacts, 1 entities in result"

The compute shader likely isn't processing AABB entities correctly, or the readback isn't working.
Need to debug the GPU collision node/shader.

### What's missing:
1. ~~System to apply GPU collision results to `GpuCollisionAABB` entities~~ DONE
2. Fix GPU collision shader to generate contacts for AABB entities
3. p23 rewrite to use Rapier + `GpuCollisionAABB` (blocked by #2)

## Naming Conventions

- `GpuCollisionAABB` - component marking entity for GPU AABB collision
- `gpu_kinematic_collision_system` - system applying GPU contacts to kinematic bodies
- `KinematicPlayer` - component for p23 player entity (distinct from fragment)

## Phases

### Phase 1: Extend GPU Collision System for Kinematic Bodies

**Outcome:** `GpuCollisionAABB` entities receive collision response from GPU pipeline.

**Verification:** 
1. Unit test: spawn entity with `GpuCollisionAABB` + `RigidBody::KinematicPositionBased`, verify GPU contacts are read
2. Integration test: entity with `GpuCollisionAABB` above terrain, verify it stops at terrain surface

**Tasks:**
1. Add `gpu_kinematic_collision_system` to `crates/studio_core/src/voxel_fragment.rs`:
   - Query entities with `GpuCollisionAABB`, `RigidBody`, `KinematicCharacterController`, `Transform`
   - Read contacts from `GpuCollisionContacts` resource for those entities
   - Apply collision response: adjust position based on penetration/normal
   
2. Register system in `VoxelFragmentPlugin` after `gpu_fragment_terrain_collision_system`

3. Export new system from `lib.rs`

4. Add unit test in `voxel_fragment.rs` verifying kinematic collision response

### Phase 2: Rewrite p23 to Use Unified Pipeline

**Outcome:** p23 uses Rapier kinematic body + `GpuCollisionAABB`, no CPU collision.

**Verification:**
1. `cargo run --example p23_kinematic_controller` - player falls and lands on terrain
2. Player can walk on terrain without falling through
3. Player can jump and land back on terrain
4. No imports from CPU `KinematicController` in p23

**Tasks:**
1. Rewrite `examples/p23_kinematic_controller.rs`:
   - Add `RapierPhysicsPlugin`
   - Player entity: `RigidBody::KinematicPositionBased` + `Collider::cuboid()` + `KinematicCharacterController` + `GpuCollisionAABB`
   - Remove `TerrainCollision` resource (CPU occupancy)
   - Remove all `KinematicController` usage
   - Player input system: compute desired velocity
   - Let `gpu_kinematic_collision_system` handle terrain collision

2. Verify player movement works correctly

3. Test edge cases: walking into walls, jumping, landing on raised platforms

### Phase 3: Clean Up CPU KinematicController

**Outcome:** CPU `KinematicController` either removed or clearly marked as fallback-only.

**Verification:** 
1. `grep -r "KinematicController" examples/` returns nothing (not used in examples)
2. If kept, docstring clearly states "CPU fallback, prefer GPU pipeline"

**Tasks:**
1. Evaluate if CPU `KinematicController` should be kept (useful for non-GPU scenarios?)
2. If removing: delete from `voxel_collision.rs`, remove export from `lib.rs`
3. If keeping: update docstring to warn about unified pipeline preference

## Full Outcome Across All Phases

- p22 and p23 both use: GPU collision detection → Rapier physics
- `GpuCollisionAABB` entities are handled by `gpu_kinematic_collision_system`
- `VoxelFragment` entities are handled by `gpu_fragment_terrain_collision_system`
- One unified collision pipeline, two entity types

## Directory Structure

No new files. Changes to:
```
crates/studio_core/src/
├── voxel_fragment.rs      # Add gpu_kinematic_collision_system
├── voxel_collision.rs     # Maybe remove/update KinematicController
├── lib.rs                 # Update exports
examples/
├── p23_kinematic_controller.rs  # Rewrite
```

## How to Review

1. Phase 1: Check `gpu_kinematic_collision_system` queries correct components, applies collision response correctly
2. Phase 2: Run p23, verify player collides with terrain via GPU pipeline
3. Phase 3: Verify no CPU collision path in examples
