# Kinematic Controller Refactor

## Current State: COMPLETED

> **Refactor completed!** The `VoxelPhysicsWorld` API is now implemented and the
> example works correctly. Player lands on ground and can walk/jump.

The kinematic controller in Phase 3 is fundamentally broken:
- Player falls through ground
- Jitters when standing
- Physics logic scattered across example file
- No fixed timestep
- No proper API

## Root Causes

### 1. Physics Logic in Example (WRONG)
```rust
// p23_kinematic_controller.rs - THIS IS INSANE
fn player_movement(...) {
    // Gravity calculated HERE in the example
    if !player.controller.grounded {
        player.velocity.y -= config.gravity * delta;
    }
    // ...
}
```

The example should NOT contain physics logic. It should call an API.

### 2. No Fixed Timestep
Using `time.delta_secs()` which varies 16ms to 20ms+ causes instability.
Physics engines use fixed timestep (e.g., 1/60s) with accumulator pattern.

### 3. No Proper Module Structure
Current:
```
voxel_collision.rs  <- Contains BOTH occupancy AND controller (wrong)
```

Should be:
```
voxel_collision.rs      <- Occupancy data structures only
voxel_physics.rs        <- Already exists for Rapier, rename or restructure
kinematic_controller.rs <- NEW: Dedicated controller module
```

### 4. No Physics-Engine-Like API
Current: Manual velocity manipulation in example
Should be: `physics.step(delta)` style API

---

## Refactor Plan

### Phase 3.1: Create Proper VoxelPhysicsWorld

**Outcome:** A `VoxelPhysicsWorld` struct that owns simulation state and provides `.step()`.

**File:** `crates/studio_core/src/voxel_physics_world.rs`

```rust
pub struct VoxelPhysicsWorld {
    occupancy: WorldOccupancy,
    bodies: Vec<KinematicBody>,
    config: PhysicsConfig,
    accumulator: f32,
}

pub struct PhysicsConfig {
    pub fixed_timestep: f32,      // e.g., 1/60
    pub gravity: Vec3,
    pub max_steps_per_frame: u32,
}

pub struct KinematicBody {
    pub position: Vec3,
    pub velocity: Vec3,
    pub half_extents: Vec3,
    pub grounded: bool,
}

impl VoxelPhysicsWorld {
    /// Step physics by delta time using fixed timestep internally
    pub fn step(&mut self, delta: f32);
    
    /// Add a kinematic body, returns handle
    pub fn add_body(&mut self, body: KinematicBody) -> BodyHandle;
    
    /// Get body state
    pub fn get_body(&self, handle: BodyHandle) -> Option<&KinematicBody>;
    
    /// Apply input velocity to body (for player control)
    pub fn set_body_input_velocity(&mut self, handle: BodyHandle, velocity: Vec3);
    
    /// Request jump (will only work if grounded)
    pub fn jump(&mut self, handle: BodyHandle, speed: f32);
}
```

**Verification:**
```rust
#[test]
fn test_physics_world_body_lands_on_ground() {
    let mut world = VoxelPhysicsWorld::new(occupancy, config);
    let body = world.add_body(KinematicBody { position: Vec3::new(0.0, 10.0, 0.0), ... });
    
    // Simulate 3 seconds
    for _ in 0..180 {
        world.step(1.0 / 60.0);
    }
    
    let state = world.get_body(body).unwrap();
    assert!(state.grounded);
    assert!((state.position.y - 3.9).abs() < 0.1);
}
```

### Phase 3.2: Fixed Timestep with Accumulator

**Outcome:** Physics runs at fixed rate regardless of frame rate.

```rust
impl VoxelPhysicsWorld {
    pub fn step(&mut self, delta: f32) {
        self.accumulator += delta;
        
        let mut steps = 0;
        while self.accumulator >= self.config.fixed_timestep 
              && steps < self.config.max_steps_per_frame 
        {
            self.step_fixed(self.config.fixed_timestep);
            self.accumulator -= self.config.fixed_timestep;
            steps += 1;
        }
    }
    
    fn step_fixed(&mut self, dt: f32) {
        for body in &mut self.bodies {
            // Apply gravity
            if !body.grounded {
                body.velocity += self.config.gravity * dt;
            }
            
            // Move and collide
            self.move_body(body, dt);
        }
    }
}
```

**Verification:** 
- Run at 30fps and 120fps, body should land at same position

### Phase 3.3: Simplify Example to API Calls Only

**Outcome:** `p23_kinematic_controller.rs` contains NO physics logic.

```rust
fn setup(...) {
    // Create physics world
    let physics = VoxelPhysicsWorld::new(occupancy, PhysicsConfig::default());
    commands.insert_resource(PhysicsWorld(physics));
    
    // Add player body
    let player_body = physics.add_body(KinematicBody::player_sized(Vec3::new(0.0, 10.0, 0.0)));
    commands.spawn((Player { body: player_body }, ...));
}

fn player_input(...) {
    // Just set input velocity, no gravity/physics here
    physics.set_body_input_velocity(player.body, input_dir * speed);
    
    if jump_pressed {
        physics.jump(player.body, 10.0);
    }
}

fn physics_step(time: Res<Time>, mut physics: ResMut<PhysicsWorld>) {
    physics.0.step(time.delta_secs());
}

fn sync_transforms(physics: Res<PhysicsWorld>, mut query: Query<(&Player, &mut Transform)>) {
    for (player, mut transform) in query.iter_mut() {
        let body = physics.0.get_body(player.body).unwrap();
        transform.translation = body.position;
    }
}
```

**Verification:** Example has ZERO physics calculations. Only API calls.

### Phase 3.4: Comprehensive Tests

**Outcome:** Physics behavior verified by tests, not manual running.

Tests needed:
1. `test_body_falls_and_lands` - Gravity works, lands on ground
2. `test_body_stops_at_wall` - Horizontal collision works
3. `test_body_slides_along_wall` - Diagonal into wall slides
4. `test_body_jumps` - Jump when grounded works
5. `test_body_no_jump_in_air` - Can't jump when airborne
6. `test_fixed_timestep_determinism` - Same result at different frame rates
7. `test_body_cross_chunk_collision` - Works across chunk boundaries

---

## Priority

This refactor MUST happen before Phase 4 (Fragment integration).

Current Phase 3 is NOT COMPLETE. The verification "controller walks on terrain" FAILS.

---

## Files Created/Modified

| File | Status | Notes |
|------|--------|-------|
| `crates/studio_core/src/voxel_physics_world.rs` | CREATED | Physics simulation with fixed timestep |
| `crates/studio_core/src/lib.rs` | MODIFIED | Exports `VoxelPhysicsWorld`, `PhysicsConfig`, `KinematicBody`, `BodyHandle` |
| `examples/p23_kinematic_controller.rs` | REWRITTEN | Now uses API only - NO physics logic in example |

Note: `KinematicController` in `voxel_collision.rs` was NOT removed - it still works for
manual usage, but `VoxelPhysicsWorld` is the recommended API for Bevy integration.

---

## What Was Implemented

1. **`VoxelPhysicsWorld`** - Self-contained physics simulation
   - Owns `WorldOccupancy` for collision queries
   - Manages multiple `KinematicBody` instances
   - Fixed timestep with accumulator pattern
   - `step(delta)` runs 0+ fixed steps to catch up

2. **`PhysicsConfig`** - Configuration for physics
   - `fixed_timestep`: 1/60s default
   - `gravity`: Vec3 (default -25 on Y)
   - `max_steps_per_frame`: Prevents spiral of death

3. **`KinematicBody`** - Body state
   - `position`, `velocity`, `half_extents`
   - `grounded`, `ground_normal`
   - Internal: `input_velocity`, `jump_requested`

4. **Simple API**:
   - `add_body()` → `BodyHandle`
   - `set_body_input_velocity(handle, vel)`
   - `jump(handle, speed)`
   - `step(delta)` - ALL physics in one call
   - `get_body(handle)` → read position/grounded

---

## Tests Added (8 tests, all passing)

1. `test_body_falls_and_lands` - Gravity works
2. `test_body_stops_at_wall` - Horizontal collision
3. `test_body_slides_along_wall` - Sliding works
4. `test_body_jumps` - Jump when grounded
5. `test_body_no_jump_in_air` - Can't double jump
6. `test_fixed_timestep_determinism` - Same result at 30/60/120 fps
7. `test_body_cross_chunk_collision` - Works at chunk boundaries
8. `test_p23_exact_scenario` - Matches example terrain

---

## Lessons Learned

1. **Design API before implementation** - Should have defined `VoxelPhysicsWorld` interface first
2. **Physics needs fixed timestep** - Variable delta causes instability
3. **Examples should not contain logic** - They demonstrate APIs, not implement features
4. **Test the actual integration** - Unit tests passing != feature works
5. **One concern per module** - Occupancy data != physics simulation
