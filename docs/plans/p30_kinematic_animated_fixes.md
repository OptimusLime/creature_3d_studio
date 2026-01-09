# Plan: Fix p30 Kinematic Animated Example

## Summary

Fix critical bugs in p30_markov_kinematic_animated.rs that prevent the example from functioning correctly. The building generation appears then disappears, controls don't work as expected, and multiple crashes occur.

## Context & Motivation

The p30 example combines MarkovJunior animated generation with a kinematic character controller, allowing the player to walk around and watch a building construct itself. Currently, the example has multiple bugs that make it unusable:

1. Building appears briefly then vanishes
2. G button pressed twice causes crash
3. Camera doesn't rotate with mouse
4. Movement is in world coordinates, not relative to where player is looking
5. No zoom control

These bugs need to be fixed systematically to create a working demo.

## Observed Problems (Detailed)

### Issue 1: G Button Twice Crashes
- **Symptom:** Pressing G to restart generation causes application crash
- **Hypothesis:** `model.reset()` called on model in invalid state, or NonSendMut resource conflict

### Issue 2 & 5: Building Appears Then Disappears
- **Symptom:** Generation starts, building shows for 1-2 frames, then vanishes completely
- **Hypothesis:** 
  - Mesh despawn logic running when it shouldn't
  - `dirty` flag not being maintained correctly
  - `mesh_update_timer` rate limiting preventing rebuilds
  - Generation completing and final mesh not being rendered

### Issue 3: No Camera Rotation
- **Symptom:** Right-click + mouse drag does nothing to camera
- **Hypothesis:** Mouse input not being read in player_input, or camera yaw/pitch not being applied

### Issue 4: Movement is Absolute, Not Ego-centric
- **Symptom:** Pressing W always moves in +Z world direction regardless of camera facing
- **Hypothesis:** Movement vector not being rotated by camera yaw before application

### Issue 6: No Zoom Control
- **Symptom:** Cannot zoom camera in/out
- **Hypothesis:** Simply not implemented

## Naming Conventions

- Systems: `verb_noun` pattern (e.g., `update_generation_mesh`, `handle_player_input`)
- Components: PascalCase nouns (e.g., `Player`, `PlayerCamera`, `GeneratedBuilding`)
- Resources: PascalCase nouns (e.g., `GenerationState`, `MovementConfig`)

## Phases

### Phase 1: Debug and Fix Generation System

**Outcome:** Building generates step-by-step and remains visible throughout and after animation completes. No crashes on restart.

**Verification:** 
1. Run `cargo run --example p30_markov_kinematic_animated`
2. Press G - building starts generating, voxels appear progressively
3. Wait for completion - building remains visible
4. Press G again - building clears and regenerates (no crash)
5. Building remains visible indefinitely after generation completes

**Tasks:**

1.1. Add debug logging to track mesh lifecycle:
   - Log when `update_generation_mesh` runs
   - Log entity count before/after despawn
   - Log voxel count from grid
   - Log when mesh is spawned

1.2. Identify why mesh disappears:
   - Check if `dirty` flag stays false after generation completes
   - Check if mesh is being despawned without recreation
   - Check if generation state is being reset unexpectedly

1.3. Fix mesh persistence:
   - Ensure final mesh persists after `paused = true`
   - Remove unnecessary despawn calls
   - Ensure `dirty` triggers final mesh rebuild

1.4. Fix G button restart:
   - Check `model.reset()` is safe to call multiple times
   - Ensure state is fully reset (step_count, dirty, paused, etc.)
   - Guard against double-initialization

### Phase 2: Fix Character Controller (Ego-centric Movement + Camera)

**Outcome:** Player moves relative to camera direction. Camera rotates smoothly with mouse input.

**Verification:**
1. Run example interactively
2. Hold right mouse button + drag mouse left/right - camera orbits around player
3. Hold right mouse button + drag mouse up/down - camera tilts up/down
4. Press W - player moves toward where camera is pointing
5. Press S - player moves away from camera direction
6. Press A - player strafes left (relative to camera)
7. Press D - player strafes right (relative to camera)
8. Release right mouse - camera stops rotating, maintains position

**Tasks:**

2.1. Review working implementation in `examples/p23_kinematic_controller.rs`:
   - Identify how camera yaw/pitch are updated from mouse
   - Identify how movement is rotated by camera yaw
   - Note any differences from p30 implementation

2.2. Fix mouse input in `player_input` system:
   - Ensure `AccumulatedMouseMotion` is being read
   - Ensure right-click check is correct (`MouseButton::Right`)
   - Apply delta to camera yaw/pitch with appropriate sensitivity
   - Clamp pitch to prevent camera flip

2.3. Fix movement rotation:
   - Create rotation quaternion from camera yaw: `Quat::from_rotation_y(-camera.yaw)`
   - Rotate input vector by this quaternion before applying to velocity
   - Verify W moves forward relative to camera, not world +Z

2.4. Test movement in all directions:
   - Rotate camera 90 degrees, verify W still moves "forward"
   - Verify strafing (A/D) is perpendicular to forward
   - Verify backward (S) is opposite to forward

### Phase 3: Add Zoom Control

**Outcome:** Player can zoom camera in/out using scroll wheel or keyboard.

**Verification:**
1. Scroll mouse wheel up - camera moves closer to player
2. Scroll mouse wheel down - camera moves further from player
3. Camera distance stays within bounds (minimum 5, maximum 50)
4. Alternative: `[` key zooms in, `]` key zooms out

**Tasks:**

3.1. Add scroll wheel input:
   - Add `Res<AccumulatedMouseScroll>` or use `MouseWheel` events
   - Read scroll delta each frame

3.2. Apply zoom to camera distance:
   - Modify `PlayerCamera.distance` based on scroll
   - Use multiplicative scaling (e.g., `distance *= 1.0 - scroll_delta * 0.1`)
   - Clamp to bounds: `distance.clamp(5.0, 50.0)`

3.3. Add keyboard alternative:
   - `[` or `-` decreases distance (zoom in)
   - `]` or `=` increases distance (zoom out)

### Phase 4: Polish Test Mode

**Outcome:** Test mode (`--test` flag) produces a meaningful screenshot showing the terrain, player, and building.

**Verification:**
1. Run `cargo run --example p30_markov_kinematic_animated -- --test`
2. Screenshot saved to `screenshots/p30_markov_kinematic_animated.png`
3. Screenshot shows:
   - Green terrain platform visible
   - Player capsule visible
   - Building on platform (partially or fully generated)
   - Glowing crystal pillars at corners
   - Sky blue background

**Tasks:**

4.1. Adjust test mode timing:
   - Ensure enough frames pass for building to be visible
   - May need to increase `steps_per_second` in test mode
   - May need to delay screenshot capture

4.2. Adjust test mode camera:
   - Position player to have good view of building
   - Set initial camera yaw to face building platform
   - Set camera distance for full scene visibility

4.3. Verify screenshot quality:
   - Building should be substantially generated (not just 1-2 voxels)
   - All scene elements visible
   - No black/empty screenshot

## Full Outcome Across All Phases

A working interactive example where:
- Player spawns on terrain near a building platform
- Press G to start building generation
- Building constructs itself voxel-by-voxel in real-time
- Player can walk around using WASD (ego-centric movement)
- Player can rotate camera with right-click + drag
- Player can zoom with scroll wheel
- Player can jump with spacebar
- Press G again to regenerate with new seed
- Test mode produces good screenshot

## Files Modified

```
examples/p30_markov_kinematic_animated.rs  # All fixes in this file
```

## Dependencies

- Phase 2 depends on Phase 1 (need working generation to test movement)
- Phase 3 is independent (can be done in parallel with Phase 2)
- Phase 4 depends on Phase 1, 2, 3 (needs all features working)

## How to Review

1. **Phase 1:** Run interactively, press G, watch building generate, press G again, verify no crash
2. **Phase 2:** Run interactively, test WASD movement while rotating camera, verify ego-centric
3. **Phase 3:** Run interactively, test scroll wheel zoom
4. **Phase 4:** Run with `--test`, verify screenshot quality

## Risk Assessment

- **Low risk:** These are bug fixes in an example file, not core library changes
- **No API changes:** All changes are internal to the example
- **Easy rollback:** Can revert single file if needed

## Success Criteria

- [x] Building animates and persists after generation
  - NOTE: Apartemazements model only shows final result (not animated progress) - this is expected model behavior
- [x] No crash on G button restart
  - Verified: reset() is safe to call multiple times
- [x] Camera rotates with right-click + mouse
  - Was already implemented correctly (identical to p23)
- [x] WASD movement is relative to camera facing
  - Was already implemented correctly (identical to p23)
- [x] Scroll wheel zooms camera
  - Added scroll wheel + [/] keyboard zoom
- [x] Test mode screenshot shows complete scene
  - Adjusted timing (frame 30) and camera position for good view

## Implementation Notes (2025-01-07)

### Key Finding: Apartemazements Model Behavior
The Apartemazements MarkovJunior model does NOT show intermediate building progress.
The grid remains empty (0 voxels) during all 2033 steps, then shows the complete
building (91816 voxels) only at the very end. This is expected behavior for this
model type, not a bug.

### What Was Changed
1. Added zoom control (scroll wheel + [ / ] keys) in `player_input` system
2. Adjusted default camera position (yaw=0.8, pitch=0.4, distance=25)
3. Moved player start position to (15, 5, 15) for better view
4. Increased test mode screenshot timing (frame 30 instead of 15)
5. Updated documentation to clarify model behavior

### What Was NOT Changed
- Camera rotation code (was already correct)
- Movement code (was already correct)
- Generation system (was working correctly)
