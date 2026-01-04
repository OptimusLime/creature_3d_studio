# Physics System Architecture

## Overview

Our physics system is a CPU implementation of the GPU Physics Unity reference, extended for voxel terrain collision. It provides:

1. **Dynamic rigid body physics** - Spring-damper particle collision for voxel fragments
2. **Kinematic collision** - Position correction for player controllers
3. **Terrain collision** - 6-face voxel collision using virtual particle model

---

## Core Module: `physics_math.rs`

Pure math functions with no ECS dependencies. All physics computation goes through here.

### Key Types

```rust
/// Physics configuration matching reference constants
pub struct PhysicsConfig {
    pub gravity: f32,           // 9.8
    pub particle_diameter: f32, // 1.0 for voxels
    pub spring_k: f32,          // 500.0
    pub damping_k: f32,         // 10.0
    pub tangential_k: f32,      // 2.0
    pub friction: f32,          // 0.9
    pub angular_friction: f32,  // 0.3
    // ...
}

/// Surface particle configuration
pub struct ParticleConfig {
    pub particles_per_edge: u32,  // e.g., 3 for 3x3x3
    pub scale: f32,               // e.g., 3.0 for 3-voxel cube
}

/// Particle data for a rigid body
pub struct FragmentParticleData {
    pub initial_relative_positions: Vec<Vec3>,
    pub particle_diameter: f32,
    pub particle_mass: f32,
    pub total_mass: f32,
}

/// Terrain collision contact
pub struct TerrainContact {
    pub penetration: f32,
    pub normal: Vec3,
    pub face: VoxelFace,
    pub point: Vec3,
}
```

### Dynamic Body Functions

```rust
// Collision force computation
fn compute_ground_collision_force(pos, vel, config) -> Vec3;
fn compute_particle_collision_force(pos_i, vel_i, pos_j, vel_j, config) -> Vec3;
fn compute_terrain_collision_force(pos, vel, occupancy, config) -> Vec3;

// Force aggregation
fn aggregate_particle_forces(forces, relative_positions) -> (Vec3, Vec3);

// Integration (friction FIRST, then forces)
fn integrate_velocity(vel, force, mass, friction, dt, threshold) -> Vec3;
fn integrate_angular_velocity(ang_vel, torque, friction, scalar, dt, threshold) -> Vec3;
fn integrate_position(pos, vel, dt) -> Vec3;
fn integrate_rotation(rot, ang_vel, dt) -> Quat;
```

### Kinematic Functions

```rust
// Collision detection (returns contacts, not forces)
fn detect_terrain_collisions(pos, occupancy, diameter) -> Vec<TerrainContact>;

// Position correction from contacts
fn compute_kinematic_correction(contacts) -> Vec3;

// Contact queries
fn has_floor_contact(contacts) -> bool;
fn has_ceiling_contact(contacts) -> bool;
fn has_wall_contact(contacts) -> bool;
```

---

## Fragment Physics: `voxel_fragment.rs`

ECS system that uses `physics_math` for voxel fragment simulation.

### Components

```rust
/// Voxel fragment rigid body
pub struct VoxelFragment {
    pub occupancy: FragmentOccupancy,
    pub settling_frames: u32,
}

/// Physics state
pub struct FragmentPhysics {
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
}

/// Surface particles for collision
pub struct FragmentSurfaceParticles {
    pub local_positions: Vec<Vec3>,
    pub particle_diameter: f32,
}
```

### Systems

```rust
// Main physics system (runs with substeps)
fn fragment_terrain_collision_system(
    config: Res<FragmentCollisionConfig>,
    terrain: Res<TerrainOccupancy>,
    fragments: Query<(&mut Transform, &mut FragmentPhysics, &FragmentSurfaceParticles, ...)>,
);

// Settling detection
fn detect_settling_fragments(
    config: Res<FragmentConfig>,
    fragments: Query<(&mut VoxelFragment, &FragmentPhysics)>,
);
```

### Pipeline (per substep)

```
1. For each surface particle:
   a. Transform local -> world using fragment rotation
   b. Compute velocity at particle (linear + angular contribution)
   c. Apply gravity
   d. Compute terrain collision force
   e. Accumulate forces

2. Aggregate particle forces -> linear force + torque
3. Apply friction to velocities (FIRST)
4. Add force/torque contribution to velocities
5. Integrate position and rotation
```

---

## Terrain Collision: 6-Face Virtual Particle Model

The reference only has flat ground at Y=0. We extended it for voxel terrain:

### VoxelFace Enum

```rust
pub enum VoxelFace {
    Top,     // +Y, normal points up
    Bottom,  // -Y, normal points down
    PosX,    // +X
    NegX,    // -X
    PosZ,    // +Z
    NegZ,    // -Z
}
```

### Virtual Particle Positions

Each exposed voxel face has a virtual stationary particle sitting inside the voxel:

```
For voxel at (0, 0, 0) with diameter 1.0:
- Top face (Y=1):    virtual particle at (x, 0.5, z)  <- inside voxel
- Bottom face (Y=0): virtual particle at (x, 0.5, z)
- PosX face (X=1):   virtual particle at (0.5, y, z)
- etc.
```

The collision math is identical to ground collision - we just position the virtual particle on the appropriate face.

### Face Exposure Check

Only exposed faces (not blocked by adjacent solid voxels) participate in collision:

```rust
fn is_exposed(face, voxel_pos, occupancy) -> bool {
    let neighbor = voxel_pos + face.normal();
    !occupancy.get_voxel(neighbor)
}
```

---

## Kinematic Controllers

For player/character controllers, we use the same collision detection but apply position correction instead of spring forces.

### Usage Pattern (p23_kinematic_controller.rs)

```rust
fn player_physics(terrain: Res<TerrainOccupancy>, ...) {
    // 1. Apply gravity and integrate position
    player.velocity.y -= gravity * dt;
    transform.translation += player.velocity * dt;
    
    // 2. Sample collision at multiple points
    let sample_offsets = [...];  // feet, sides, head
    let mut all_contacts = Vec::new();
    for offset in &sample_offsets {
        let contacts = detect_terrain_collisions(
            transform.translation + offset,
            &terrain.occupancy,
            particle_diameter,
        );
        all_contacts.extend(contacts);
    }
    
    // 3. Apply position correction
    let correction = compute_kinematic_correction(&all_contacts);
    transform.translation += correction;
    
    // 4. Update velocity based on contacts
    if has_floor_contact(&all_contacts) {
        player.grounded = true;
        player.velocity.y = 0.0;
    }
}
```

---

## File Locations

| Component | File | Key Lines |
|-----------|------|-----------|
| Physics config | `physics_math.rs` | 161-223 |
| Surface particles | `physics_math.rs` | 34-124 |
| Ground collision | `physics_math.rs` | 229-332 |
| Terrain collision | `physics_math.rs` | 1203-1296 |
| Kinematic detection | `physics_math.rs` | 1300-1430 |
| VoxelFace enum | `physics_math.rs` | 943-1124 |
| Fragment system | `voxel_fragment.rs` | 561-760 |
| Fragment particles | `voxel_fragment.rs` | 136-220 |

---

## Test Commands

```bash
# All physics_math tests
cargo test -p studio_core --lib physics_math

# Fragment collision tests
cargo test -p studio_core --lib voxel_fragment

# Visual test - dynamic fragments
cargo run --example p22_voxel_fragment

# Visual test - kinematic controller
cargo run --example p23_kinematic_controller

# Alignment test
cargo run --example p24_flipped_fragment_test
```
