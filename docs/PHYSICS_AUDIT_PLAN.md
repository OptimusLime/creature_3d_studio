# Physics Audit Plan: Line-by-Line Match to gpu-physics-unity

## Summary

Our current spring-damper physics implementation is broken - fragments explode on contact. Rather than continue guessing at fixes, we will create a **line-by-line faithful reproduction** of gpu-physics-unity's physics, verify it works in isolation, then integrate our terrain occupancy.

## Context & Motivation

We attempted to implement Harada's spring-damper model but made several mistakes:
- Accumulated forces per-voxel instead of per-rigid-body
- Used wrong force magnitudes for direct Euler integration
- Mixed concepts from Rapier (which we removed) with our own integration
- Did not verify intermediate steps

The ONLY way out is to **faithfully match the reference** then layer in our terrain.

## Reference Implementation

**Source:** `gpu-physics-unity/Assets/Physics/GPUPhysicsComputeShader.compute`
**C# Setup:** `gpu-physics-unity/Assets/Physics/GPUPhysics.cs`
**Quaternion Math:** `gpu-physics-unity/Assets/Physics/Quaternion.cginc`

---

## Pseudo-Algorithm: gpu-physics-unity Physics Pipeline

This is the EXACT algorithm from the reference implementation. We MUST match this.

### Kernel Dispatch Order (per frame)

```
1. GenerateParticleValues     (per rigid body)
2. ClearGrid                  (per grid cell)
3. PopulateGrid               (per particle)
4. CollisionDetection         (per particle) -> outputs particleForces[]
5. ComputeMomenta             (per rigid body) -> updates velocities
6. ComputePositionAndRotation (per rigid body) -> updates positions/rotations
```

### Step 1: GenerateParticleValues (line 87-101)

For each rigid body:
```
for i in 0..particlesPerRigidBody:
    p_id = body_id * particlesPerRigidBody + i
    
    // Transform particle from local space to world space
    particleRelativePositions[p_id] = rotate(rigidBodyQuaternion, particleInitialRelativePositions[p_id])
    particlePositions[p_id] = rigidBodyPosition + particleRelativePositions[p_id]
    
    // Particle velocity = body linear velocity + angular contribution
    particleVelocities[p_id] = rigidBodyVelocity + cross(rigidBodyAngularVelocity, particleRelativePositions[p_id])
```

### Step 2: ClearGrid (line 121-128)

```
voxelCollisionGrid[cell_id] = int4(-1, -1, -1, -1)
```

### Step 3: PopulateGrid (line 140-158)

For each particle:
```
gridIndex = (particlePosition - gridStartPosition) / particleDiameter
if (gridIndex in bounds):
    // Try to insert into slot x, then y, then z, then w
    atomicCompareExchange(grid[gridIndex].x, -1, particle_id)
    // ... etc for y, z, w
```

### Step 4: CollisionDetection (line 290-330)

For each particle i:
```
force = float3(0, 0, 0)

// Check 27 neighboring cells for collisions with other particles
for each of 27 neighbors:
    for each particle j in cell:
        if j != i:
            force += _collisionReaction(j, i)

// Add gravity
force.y -= gravityCoefficient

// Add ground collision
force += _collisionReactionWithGround(i)

particleForces[i] = force
```

### Step 4a: _collisionReaction (line 174-216)

```python
def _collisionReaction(j, i):
    # Position of j relative to i (points FROM i TO j)
    relativePosition = particlePositions[j] - particlePositions[i]
    relativePositionMagnitude = length(relativePosition)
    
    if relativePositionMagnitude >= particleDiameter:
        return float3(0, 0, 0)  # No collision
    
    n = relativePosition / relativePositionMagnitude  # Normal FROM i TO j
    penetration = particleDiameter - relativePositionMagnitude
    
    # Repulsive spring force (Equation 10)
    # NEGATIVE because n points toward j, but we want force to push i AWAY from j
    repulsiveForce = -springCoefficient * penetration * n
    
    # Relative velocity (other minus self)
    relativeVelocity = particleVelocities[j] - particleVelocities[i]
    
    # Damping force (Equation 11)
    dampingForce = dampingCoefficient * relativeVelocity
    
    # Tangential force (Equation 12) - friction opposing sliding
    normalVelocity = dot(relativeVelocity, n) * n
    tangentialVelocity = relativeVelocity - normalVelocity
    tangentialForce = tangentialCoefficient * tangentialVelocity
    
    return repulsiveForce + dampingForce + tangentialForce
```

### Step 4b: _collisionReactionWithGround (line 218-251)

```python
def _collisionReactionWithGround(i):
    # Create virtual ground particle at Y = -particleDiameter * 0.5
    # This means ground SURFACE is at Y = 0
    groundParticlePosition = particlePositions[i]
    groundParticlePosition.y = -particleDiameter * 0.5
    
    # Same collision logic but with stationary ground particle
    relativePosition = groundParticlePosition - particlePositions[i]
    relativePositionMagnitude = length(relativePosition)
    
    if relativePositionMagnitude >= particleDiameter:
        return float3(0, 0, 0)
    
    n = relativePosition / relativePositionMagnitude
    penetration = particleDiameter - relativePositionMagnitude
    
    repulsiveForce = -springCoefficient * penetration * n
    
    # Ground velocity is ZERO
    relativeVelocity = float3(0, 0, 0) - particleVelocities[i]
    
    dampingForce = dampingCoefficient * relativeVelocity
    
    normalVelocity = dot(relativeVelocity, n) * n
    tangentialVelocity = relativeVelocity - normalVelocity
    tangentialForce = tangentialCoefficient * tangentialVelocity
    
    return repulsiveForce + dampingForce + tangentialForce
```

### Step 5: ComputeMomenta (line 348-382)

For each rigid body:
```python
linearForce = float3(0, 0, 0)
angularForce = float3(0, 0, 0)

for i in 0..particlesPerRigidBody:
    p_id = body_id * particlesPerRigidBody + i
    relativePosition = particleRelativePositions[p_id]
    
    linearForce += particleForces[p_id]
    angularForce += cross(relativePosition, particleForces[p_id])

cubeMass = particleMass * particlesPerRigidBody

# Velocity threshold (zero out tiny velocities)
threshold = 1e-6

# Apply friction BEFORE adding forces
rigidBodyVelocities[body_id] /= (1.0 + deltaTime * frictionCoefficient)
rigidBodyVelocities[body_id] += linearForceScalar * deltaTime * linearForce / cubeMass
if length(rigidBodyVelocities[body_id]) < threshold:
    rigidBodyVelocities[body_id] = float3(0, 0, 0)

# Angular (simplified - no inertia tensor in final version)
rigidBodyAngularVelocities[body_id] /= (1.0 + deltaTime * angularFrictionCoefficient)
rigidBodyAngularVelocities[body_id] += angularForceScalar * deltaTime * angularForce
if length(rigidBodyAngularVelocities[body_id]) < threshold:
    rigidBodyAngularVelocities[body_id] = float3(0, 0, 0)
```

### Step 6: ComputePositionAndRotation (line 397-432)

For each rigid body:
```python
# Position integration (simple Euler)
rigidBodyPositions[body_id] += rigidBodyVelocities[body_id] * deltaTime

# Rotation integration (quaternion derivative)
omega = float4(rigidBodyAngularVelocity.xyz, 0)
q = rigidBodyQuaternions[body_id]
rigidBodyQuaternions[body_id] = normalize(q + deltaTime * 0.5 * quat_concat(omega, q))
```

---

## Default Constants (from GPUPhysics.cs)

These are the EXACT values that MUST be used until physics is working:

| Constant | Value | Description |
|----------|-------|-------------|
| `gravityCoefficient` | 9.8 | Gravity force (subtracted from Y) |
| `particleDiameter` | 1.0 | Size of collision particle |
| `springCoefficient` | 500.0 | Spring stiffness (Hooke's law) |
| `dampingCoefficient` | 10.0 | Velocity damping |
| `tangentialCoefficient` | 2.0 | Friction/tangential damping |
| `frictionCoefficient` | 0.9 | Linear velocity friction |
| `angularFrictionCoefficient` | 0.3 | Angular velocity friction |
| `linearForceScalar` | 1.0 | Force multiplier |
| `angularForceScalar` | 1.0 | Torque multiplier |
| `particleMass` | cubeMass / particlesPerRigidBody | Per-particle mass |

---

## LINE-BY-LINE AUDIT CHECKLIST

**Instructions:** For each item, compare the reference code to our code. Mark as PASS/FAIL/TODO.

### Section A: Collision Detection - Ground

| # | Audit Item | Reference (gpu-physics-unity) | Our Code (voxel_fragment.rs) | Status |
|---|------------|-------------------------------|------------------------------|--------|
| A1 | Ground particle Y position | `groundParticlePosition.y = -particleDiameter * 0.5` (line 221) | ??? | [ ] TODO |
| A2 | Relative position direction | `groundParticlePosition - particlePositions[i]` (line 224) - points FROM particle TO ground | ??? | [ ] TODO |
| A3 | Collision condition | `relativePositionMagnitude < particleDiameter` (line 227) | ??? | [ ] TODO |
| A4 | Normal direction | `relativePosition / relativePositionMagnitude` - points FROM particle TO ground | ??? | [ ] TODO |
| A5 | Penetration calculation | `particleDiameter - relativePositionMagnitude` (line 227 implies) | ??? | [ ] TODO |
| A6 | Spring force sign | `-springCoefficient * penetration * n` (line 232) - NEGATIVE | ??? | [ ] TODO |
| A7 | Ground velocity | `float3(0, 0, 0)` (line 238) - stationary | ??? | [ ] TODO |
| A8 | Relative velocity direction | `ground_vel - particle_vel = -particle_vel` (line 238) | ??? | [ ] TODO |
| A9 | Damping force formula | `dampingCoefficient * relativeVelocity` (line 240) | ??? | [ ] TODO |
| A10 | Normal velocity projection | `dot(relativeVelocity, n) * n` (line 243) | ??? | [ ] TODO |
| A11 | Tangential velocity | `relativeVelocity - normalVelocity` (line 243) | ??? | [ ] TODO |
| A12 | Tangential force formula | `tangentialCoefficient * tangentialVelocity` (line 244) | ??? | [ ] TODO |
| A13 | Force sum | `repulsive + damping + tangential` (line 246) | ??? | [ ] TODO |

### Section B: Collision Detection - Particle-Particle

| # | Audit Item | Reference (gpu-physics-unity) | Our Code | Status |
|---|------------|-------------------------------|----------|--------|
| B1 | Relative position direction | `particlePositions[j] - particlePositions[i]` (line 178) - points FROM i TO j | ??? | [ ] TODO |
| B2 | Collision condition | `relativePositionMagnitude < particleDiameter` (line 181) | ??? | [ ] TODO |
| B3 | Normal direction | `relativePosition / relativePositionMagnitude` - points FROM i TO j | ??? | [ ] TODO |
| B4 | Penetration calculation | `particleDiameter - relativePositionMagnitude` (line 186) | ??? | [ ] TODO |
| B5 | Spring force sign | `-springCoefficient * penetration * n` (line 186) - NEGATIVE | ??? | [ ] TODO |
| B6 | Relative velocity direction | `particleVelocities[j] - particleVelocities[i]` (line 192) | ??? | [ ] TODO |
| B7 | Damping force formula | `dampingCoefficient * relativeVelocity` (line 207) | ??? | [ ] TODO |
| B8 | Tangential velocity | `relativeVelocity - dot(rel_vel, n) * n` (lines 210-211) | ??? | [ ] TODO |
| B9 | Tangential force formula | `tangentialCoefficient * tangentialVelocity` (line 211) | ??? | [ ] TODO |
| B10 | Force sum | `repulsive + damping + tangential` (line 213) | ??? | [ ] TODO |
| B11 | Self-collision skip | `j != i` check (lines 270, 274, 278, 282) | ??? | [ ] TODO |
| B12 | 27-cell neighbor check | Lines 296-324 check x-1,x,x+1 * y-1,y,y+1 * z-1,z,z+1 | ??? | [ ] TODO |

### Section C: Force Output & Gravity

| # | Audit Item | Reference (gpu-physics-unity) | Our Code | Status |
|---|------------|-------------------------------|----------|--------|
| C1 | Force accumulator init | `float3 force = float3(0,0,0)` (line 295) | ??? | [ ] TODO |
| C2 | Gravity application | `force.y -= gravityCoefficient` (line 326) - SUBTRACTED | ??? | [ ] TODO |
| C3 | Gravity value | `gravityCoefficient = 9.8` (GPUPhysics.cs line 39) | ??? | [ ] TODO |
| C4 | Ground collision added | `force += _collisionReactionWithGround(i)` (line 327) | ??? | [ ] TODO |
| C5 | Force output | `particleForces[i] = force` (line 329) | ??? | [ ] TODO |

### Section D: Momenta Computation (Particle Forces -> Rigid Body)

| # | Audit Item | Reference (gpu-physics-unity) | Our Code | Status |
|---|------------|-------------------------------|----------|--------|
| D1 | Linear force init | `float3 linearForce = float3(0,0,0)` (line 352) | ??? | [ ] TODO |
| D2 | Angular force init | `float3 angularForce = float3(0,0,0)` (line 353) | ??? | [ ] TODO |
| D3 | Linear force sum | `linearForce += particleForces[p_id]` (line 359) | ??? | [ ] TODO |
| D4 | Torque formula | `angularForce += cross(relativePosition, particleForces[p_id])` (line 360) | ??? | [ ] TODO |
| D5 | Relative position for torque | `particleRelativePositions[p_id]` (line 358) - local space offset | ??? | [ ] TODO |
| D6 | Mass calculation | `cubeMass = particleMass * particlesPerRigidBody` (line 364) | ??? | [ ] TODO |

### Section E: Velocity Integration

| # | Audit Item | Reference (gpu-physics-unity) | Our Code | Status |
|---|------------|-------------------------------|----------|--------|
| E1 | Friction FIRST | `vel /= (1.0 + dt * friction)` BEFORE adding forces (line 365) | ??? | [ ] TODO |
| E2 | Velocity threshold | `threshold = 1e-6` (line 362) | ??? | [ ] TODO |
| E3 | Linear velocity update | `vel += linearForceScalar * dt * linearForce / cubeMass` (line 366) | ??? | [ ] TODO |
| E4 | linearForceScalar value | `linearForceScalar = 1.0` (typically) | ??? | [ ] TODO |
| E5 | Velocity zeroing | If `length(vel) < threshold` then `vel = 0` (lines 367-369) | ??? | [ ] TODO |
| E6 | Angular friction FIRST | `angVel /= (1.0 + dt * angularFriction)` BEFORE adding (line 377) | ??? | [ ] TODO |
| E7 | Angular velocity update | `angVel += angularForceScalar * dt * angularForce` (line 378) | ??? | [ ] TODO |
| E8 | angularForceScalar value | `angularForceScalar = 1.0` (typically) | ??? | [ ] TODO |
| E9 | Angular velocity zeroing | If `length(angVel) < threshold` then `angVel = 0` (lines 379-381) | ??? | [ ] TODO |

### Section F: Position/Rotation Integration

| # | Audit Item | Reference (gpu-physics-unity) | Our Code | Status |
|---|------------|-------------------------------|----------|--------|
| F1 | Position update | `pos += vel * dt` (line 401) | ??? | [ ] TODO |
| F2 | Quaternion omega format | `omega = float4(angVel.xyz, 0)` (line 428) - xyz in xyz, 0 in w | ??? | [ ] TODO |
| F3 | Quaternion derivative | `q + dt * 0.5 * quat_concat(omega, q)` (line 431) | ??? | [ ] TODO |
| F4 | Quaternion normalization | `normalize(...)` after integration (line 431) | ??? | [ ] TODO |
| F5 | quat_concat formula | `float4(q1.w*q2.xyz + q2.w*q1.xyz + cross(q1.xyz, q2.xyz), q1.w*q2.w - dot(q1.xyz, q2.xyz))` (Quaternion.cginc line 44) | ??? | [ ] TODO |

### Section G: Particle Generation

| # | Audit Item | Reference (gpu-physics-unity) | Our Code | Status |
|---|------------|-------------------------------|----------|--------|
| G1 | Particle relative position | `rotate(bodyQuat, initialRelativePos)` (line 97) | ??? | [ ] TODO |
| G2 | Particle world position | `bodyPos + relativePos` (line 98) | ??? | [ ] TODO |
| G3 | Particle velocity | `bodyVel + cross(angVel, relativePos)` (line 99) | ??? | [ ] TODO |
| G4 | Quaternion rotation | `quat_mul(quaternion, vec)` (Quaternion.cginc line 47) | ??? | [ ] TODO |

### Section H: Spatial Hash Grid

| # | Audit Item | Reference (gpu-physics-unity) | Our Code | Status |
|---|------------|-------------------------------|----------|--------|
| H1 | Grid clear value | `-1` for all 4 slots (lines 124-127) | ??? | [ ] TODO |
| H2 | Grid index formula | `(pos - gridStart) / particleDiameter` then linearize (line 133) | ??? | [ ] TODO |
| H3 | 4 slots per cell | `int4` with atomic insert into x, y, z, w (lines 150-156) | ??? | [ ] TODO |
| H4 | Bounds checking | `gridIndex < gridMax && gridIndex > -1` (line 146) | ??? | [ ] TODO |

### Section I: Constants Match

| # | Constant | Reference Value | Our Value | Status |
|---|----------|-----------------|-----------|--------|
| I1 | gravityCoefficient | 9.8 | ??? | [ ] TODO |
| I2 | particleDiameter | 1.0 | ??? | [ ] TODO |
| I3 | springCoefficient | 500.0 | ??? | [ ] TODO |
| I4 | dampingCoefficient | 10.0 | ??? | [ ] TODO |
| I5 | tangentialCoefficient | 2.0 | ??? | [ ] TODO |
| I6 | frictionCoefficient | 0.9 | ??? | [ ] TODO |
| I7 | angularFrictionCoefficient | 0.3 | ??? | [ ] TODO |
| I8 | linearForceScalar | 1.0 | ??? | [ ] TODO |
| I9 | angularForceScalar | 1.0 | ??? | [ ] TODO |

---

## KEY DIFFERENCES: gpu-physics-unity vs Our System

### What gpu-physics-unity Does

1. **Particles represent rigid body surface** - Each rigid body has N particles on its surface
2. **Forces computed per particle** - Each particle checks for collisions independently
3. **Forces aggregated to rigid body** - Sum particle forces for linear, cross product for torque
4. **Single ground plane at Y=0** - Ground is a virtual stationary particle

### What We Do Differently (must adapt)

1. **Voxels represent rigid body** - Each voxel is like a particle
2. **Terrain is 3D occupancy, not Y=0 plane** - Must adapt ground collision
3. **Contact aggregation strategy** - We aggregate contacts, not particles

### Critical Adaptation Notes

For terrain collision, we need to:
1. Treat each terrain contact like a collision with a virtual stationary particle
2. The "particle diameter" = voxel size = 1.0
3. The terrain contact normal is the surface normal
4. The terrain contact position determines the "virtual ground particle" position

---

## PHASE 0: Create Isolated Test Harness (SCAFFOLDING ONLY)

**Outcome:** A minimal test file and example that compile. NO PHYSICS VERIFICATION.

**What Phase 0 IS:**
- A clean slate to implement and test physics in isolation
- Scaffolding with stub functions
- A place to add unit tests

**What Phase 0 IS NOT:**
- Working physics
- A demo that shows correct behavior
- Anything that "bounces" or "settles"

### Tasks:

- [ ] Create `tests/physics_audit_test.rs` with stub test functions (all `#[ignore]` initially)
- [ ] Create pure functions for physics math (no ECS, no Bevy) in a new module
- [ ] Create `examples/p24_physics_audit.rs` - visual harness (optional, for later visual debugging)

### Verification:

```bash
cargo test -p studio_core --lib physics  # Compiles, all tests ignored
cargo run --example p24_physics_audit    # Compiles and runs (cube may explode, that's fine)
```

### Test Stubs to Create:

```rust
// tests/physics_audit_test.rs

#[test]
#[ignore] // Until Phase 1 complete
fn test_ground_collision_force_direction() {
    // A1-A6: Verify spring force points UP when particle is below ground
    todo!()
}

#[test]
#[ignore] // Until Phase 1 complete  
fn test_ground_collision_force_magnitude() {
    // A5-A6: Verify force magnitude matches reference formula
    todo!()
}

#[test]
#[ignore] // Until Phase 1 complete
fn test_damping_force_opposes_velocity() {
    // A7-A9: Verify damping force opposes particle velocity
    todo!()
}

#[test]
#[ignore] // Until Phase 1 complete
fn test_tangential_force_opposes_sliding() {
    // A10-A12: Verify tangential force opposes sliding motion
    todo!()
}

#[test]
#[ignore] // Until Phase 2 complete
fn test_velocity_integration_with_friction() {
    // E1-E5: Verify friction applied BEFORE force, correct formula
    todo!()
}

#[test]
#[ignore] // Until Phase 2 complete
fn test_position_integration() {
    // F1: Verify pos += vel * dt
    todo!()
}

#[test]
#[ignore] // Until Phase 2 complete
fn test_quaternion_integration() {
    // F2-F5: Verify quaternion derivative formula
    todo!()
}

#[test]
#[ignore] // Until Phase 3 complete
fn test_particle_force_aggregation() {
    // D1-D6: Verify forces sum correctly, torque computed correctly
    todo!()
}

#[test]
#[ignore] // Until Phase 4 complete
fn test_cube_falls_and_settles() {
    // Full integration: cube dropped from height settles at ground
    todo!()
}

#[test]
#[ignore] // Until Phase 5 complete
fn test_two_cubes_collide() {
    // B1-B12: Two cubes collide and separate
    todo!()
}
```

---

## PHASE 1: Implement & Test Force Computation

**Outcome:** Pure functions for force computation that pass unit tests.

**Verification:** `cargo test -p studio_core --lib physics` - force tests PASS (remove `#[ignore]`)

### Tasks:

- [ ] Create `crates/studio_core/src/physics_math.rs` with pure functions
- [ ] Implement `compute_ground_collision_force(particle_pos, particle_vel, config) -> Vec3`
- [ ] Implement `compute_particle_collision_force(pos_i, vel_i, pos_j, vel_j, config) -> Vec3`
- [ ] Write tests for A1-A13 (ground collision)
- [ ] Write tests for B1-B10 (particle collision)
- [ ] Write tests for C1-C5 (gravity)
- [ ] All force computation tests pass

### Test Examples:

```rust
#[test]
fn test_ground_collision_force_direction() {
    let config = PhysicsConfig::default(); // Uses reference constants
    
    // Particle at Y=0.3 (below surface, penetrating)
    let particle_pos = Vec3::new(0.0, 0.3, 0.0);
    let particle_vel = Vec3::new(0.0, -5.0, 0.0); // Falling
    
    let force = compute_ground_collision_force(particle_pos, particle_vel, &config);
    
    // Force should point UP (positive Y)
    assert!(force.y > 0.0, "Ground collision force should point up, got {}", force.y);
    
    // Force should be significant (spring + damping)
    assert!(force.y > 100.0, "Force should be significant, got {}", force.y);
}

#[test]
fn test_ground_collision_no_force_when_above() {
    let config = PhysicsConfig::default();
    
    // Particle at Y=2.0 (well above ground)
    let particle_pos = Vec3::new(0.0, 2.0, 0.0);
    let particle_vel = Vec3::new(0.0, -5.0, 0.0);
    
    let force = compute_ground_collision_force(particle_pos, particle_vel, &config);
    
    // No collision = no force
    assert_eq!(force, Vec3::ZERO);
}

#[test]
fn test_spring_force_magnitude() {
    let config = PhysicsConfig::default(); // spring_k = 500
    
    // Particle at Y=0.3, penetration = 0.5 - 0.3 = 0.2 (since ground particle at -0.5)
    // Wait, let's think about this...
    // Ground particle Y = -0.5
    // Our particle Y = 0.3
    // Distance = 0.3 - (-0.5) = 0.8
    // Penetration = diameter(1.0) - distance(0.8) = 0.2
    // Spring force = -500 * 0.2 * normal
    // Normal points from particle toward ground = (0, -1, 0) normalized
    // So spring force = -500 * 0.2 * (0, -1, 0) = (0, 100, 0)
    
    let particle_pos = Vec3::new(0.0, 0.3, 0.0);
    let particle_vel = Vec3::ZERO; // No velocity = no damping
    
    let force = compute_ground_collision_force(particle_pos, particle_vel, &config);
    
    // Spring force should be approximately 100 (500 * 0.2)
    assert!((force.y - 100.0).abs() < 1.0, "Expected ~100, got {}", force.y);
}
```

---

## PHASE 2: Implement & Test Integration

**Outcome:** Pure functions for velocity/position integration that pass unit tests.

**Verification:** `cargo test -p studio_core --lib physics` - integration tests PASS

### Tasks:

- [ ] Implement `integrate_velocity(vel, force, mass, friction, dt) -> Vec3`
- [ ] Implement `integrate_position(pos, vel, dt) -> Vec3`
- [ ] Implement `integrate_rotation(quat, angular_vel, angular_friction, dt) -> Quat`
- [ ] Write tests for E1-E9 (velocity integration)
- [ ] Write tests for F1-F5 (position/rotation integration)
- [ ] All integration tests pass

### Test Examples:

```rust
#[test]
fn test_friction_applied_before_force() {
    // E1: Friction divides velocity BEFORE adding force
    let vel = Vec3::new(0.0, 10.0, 0.0);
    let force = Vec3::new(0.0, 100.0, 0.0);
    let mass = 1.0;
    let friction = 0.9;
    let dt = 1.0 / 60.0;
    
    let new_vel = integrate_velocity(vel, force, mass, friction, dt);
    
    // vel_after_friction = 10.0 / (1.0 + dt * 0.9) = 10.0 / 1.015 = 9.852
    // vel_after_force = 9.852 + 100.0 * dt = 9.852 + 1.667 = 11.519
    let expected = 10.0 / (1.0 + dt * friction) + force.y / mass * dt;
    assert!((new_vel.y - expected).abs() < 0.001, "Expected {}, got {}", expected, new_vel.y);
}

#[test]
fn test_velocity_zeroing_threshold() {
    // E2, E5: Tiny velocities should be zeroed
    let vel = Vec3::new(0.0, 0.0000001, 0.0); // Below 1e-6 threshold
    let force = Vec3::ZERO;
    let mass = 1.0;
    let friction = 0.9;
    let dt = 1.0 / 60.0;
    
    let new_vel = integrate_velocity(vel, force, mass, friction, dt);
    
    assert_eq!(new_vel, Vec3::ZERO, "Tiny velocity should be zeroed");
}

#[test]
fn test_position_integration() {
    // F1: pos += vel * dt
    let pos = Vec3::new(0.0, 10.0, 0.0);
    let vel = Vec3::new(0.0, -5.0, 0.0);
    let dt = 1.0 / 60.0;
    
    let new_pos = integrate_position(pos, vel, dt);
    
    let expected_y = 10.0 + (-5.0) * dt;
    assert!((new_pos.y - expected_y).abs() < 0.0001);
}
```

---

## PHASE 3: Implement & Test Force Aggregation

**Outcome:** Pure functions for aggregating particle forces to rigid body.

**Verification:** `cargo test -p studio_core --lib physics` - aggregation tests PASS

### Tasks:

- [ ] Implement `aggregate_particle_forces(particle_forces, particle_relative_positions) -> (Vec3, Vec3)` (linear, angular)
- [ ] Write tests for D1-D6 (force aggregation)
- [ ] All aggregation tests pass

### Test Examples:

```rust
#[test]
fn test_linear_force_aggregation() {
    // D3: Linear force = sum of all particle forces
    let particle_forces = vec![
        Vec3::new(0.0, 10.0, 0.0),
        Vec3::new(0.0, 20.0, 0.0),
        Vec3::new(0.0, 30.0, 0.0),
    ];
    let particle_relative_positions = vec![
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, -0.5),
        Vec3::new(0.0, 0.5, 0.0),
    ];
    
    let (linear, _angular) = aggregate_particle_forces(&particle_forces, &particle_relative_positions);
    
    assert_eq!(linear, Vec3::new(0.0, 60.0, 0.0));
}

#[test]
fn test_torque_from_off_center_force() {
    // D4: Torque = cross(relative_pos, force)
    // Force at +X offset should create torque around Z axis
    let particle_forces = vec![Vec3::new(0.0, 100.0, 0.0)]; // Upward force
    let particle_relative_positions = vec![Vec3::new(1.0, 0.0, 0.0)]; // At +X offset
    
    let (_linear, angular) = aggregate_particle_forces(&particle_forces, &particle_relative_positions);
    
    // cross((1,0,0), (0,100,0)) = (0*0-0*100, 0*0-1*0, 1*100-0*0) = (0, 0, 100)
    assert!((angular.z - 100.0).abs() < 0.001, "Expected Z torque ~100, got {}", angular.z);
}
```

---

## PHASE 4: Integration Test - Single Body Ground Collision

**Outcome:** A single rigid body dropped onto ground bounces and settles.

**Verification:** 
- `cargo test -p studio_core --lib test_cube_falls_and_settles` PASSES
- `cargo run --example p24_physics_audit` shows cube bouncing and settling (visual confirmation)

### Tasks:

- [ ] Wire up pure functions into a simulation loop
- [ ] Create `simulate_single_body(initial_pos, initial_vel, steps, dt) -> Vec<Vec3>` that returns position history
- [ ] Write integration test that verifies:
  - Cube starts at Y=10
  - Cube falls (Y decreases)
  - Cube bounces (Y increases after hitting ground)
  - Cube settles (Y stabilizes near 0.5 after N steps)
- [ ] Update example to use new physics
- [ ] Remove `#[ignore]` from `test_cube_falls_and_settles`

### Test Example:

```rust
#[test]
fn test_cube_falls_and_settles() {
    let initial_pos = Vec3::new(0.0, 10.0, 0.0);
    let initial_vel = Vec3::ZERO;
    let dt = 1.0 / 60.0;
    let steps = 600; // 10 seconds
    
    let positions = simulate_single_body(initial_pos, initial_vel, steps, dt);
    
    // Should fall initially
    assert!(positions[10].y < positions[0].y, "Should fall");
    
    // Should hit ground (Y should go near 0.5 at some point)
    let min_y = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    assert!(min_y < 1.0, "Should hit ground, min_y was {}", min_y);
    
    // Should settle near Y=0.5 (half particle diameter above ground)
    let final_y = positions.last().unwrap().y;
    assert!((final_y - 0.5).abs() < 0.1, "Should settle near Y=0.5, got {}", final_y);
    
    // Should not explode (Y should never go extremely high or negative)
    let max_y = positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!(max_y < 15.0, "Should not explode upward, max_y was {}", max_y);
    assert!(min_y > -1.0, "Should not go through ground, min_y was {}", min_y);
}
```

---

## PHASE 5: Implement & Test Particle-Particle Collision

**Outcome:** Two rigid bodies collide correctly.

**Verification:** `cargo test -p studio_core --lib test_two_cubes_collide` PASSES

### Tasks:

- [ ] Implement `compute_particle_collision_force` (already in Phase 1)
- [ ] Create `simulate_two_bodies(...)` 
- [ ] Write integration test that verifies:
  - Two cubes start separated
  - They collide
  - They separate (don't interpenetrate)
  - They settle
- [ ] Remove `#[ignore]` from `test_two_cubes_collide`

---

## PHASE 6: Terrain Occupancy Collision (Flat Voxel Floor)

**Outcome:** Physics works with a flat voxel terrain floor instead of Y=0 plane, verified by unit tests.

**Verification:** `cargo test -p studio_core --lib test_cube_on_voxel_floor` PASSES

### What This Phase IS:
- Replace `compute_ground_collision_force` with `compute_terrain_collision_force`
- Terrain is a FLAT voxel floor (single layer of voxels at Y=0)
- Use `WorldOccupancy` to check if voxel is solid
- Each occupied voxel the particle overlaps generates a collision force
- Forces aggregated exactly like particle-particle collision

### What This Phase IS NOT:
- Complex terrain shapes (stairs, overhangs, walls)
- The full p22 example integration
- GPU collision pipeline

### Tasks:

- [ ] Create `compute_terrain_collision_force(particle_pos, particle_vel, occupancy, config) -> Vec3`
- [ ] For each voxel the particle overlaps:
  - [ ] Check if voxel is occupied via `occupancy.get_voxel(ivec3)`
  - [ ] If occupied, compute collision force using same formula as ground collision
  - [ ] Terrain voxel center = virtual "ground particle" position
  - [ ] Sum all voxel collision forces
- [ ] Create test terrain: flat floor of voxels at Y=0 (10x10 grid)
- [ ] Write `test_cube_on_voxel_floor` - cube lands on voxel floor, settles
- [ ] Write `test_cube_misses_voxel_gap` - cube falls through gap in floor
- [ ] Write `test_terrain_collision_force_matches_ground` - single voxel collision force equals ground collision force
- [ ] All terrain collision tests pass

### Test Examples:

```rust
#[test]
fn test_terrain_collision_force_matches_ground() {
    // A single voxel at (0,0,0) should produce same force as Y=0 ground plane
    // when particle is directly above it
    let config = PhysicsConfig::default();
    
    // Create terrain with single voxel at origin
    let mut occupancy = WorldOccupancy::new();
    occupancy.set_voxel(IVec3::new(0, 0, 0), true);
    
    // Particle at Y=0.3, centered over voxel (same as ground test)
    let particle_pos = Vec3::new(0.5, 0.3, 0.5); // Center of voxel is (0.5, 0.5, 0.5)
    let particle_vel = Vec3::ZERO;
    
    let terrain_force = compute_terrain_collision_force(particle_pos, particle_vel, &occupancy, &config);
    let ground_force = compute_ground_collision_force(particle_pos, particle_vel, &config);
    
    // Forces should be very similar (not exact due to voxel center vs ground plane)
    assert!((terrain_force.y - ground_force.y).abs() < 50.0, 
        "Terrain force {} should be similar to ground force {}", terrain_force.y, ground_force.y);
    assert!(terrain_force.y > 0.0, "Should push up");
}

#[test]
fn test_cube_on_voxel_floor() {
    // Create 10x10 voxel floor at Y=0
    let mut occupancy = WorldOccupancy::new();
    for x in 0..10 {
        for z in 0..10 {
            occupancy.set_voxel(IVec3::new(x, 0, z), true);
        }
    }
    
    let initial_pos = Vec3::new(5.0, 10.0, 5.0); // Above center of floor
    let initial_vel = Vec3::ZERO;
    let dt = 1.0 / 60.0;
    let steps = 600;
    
    let positions = simulate_single_body_on_terrain(initial_pos, initial_vel, &occupancy, steps, dt);
    
    // Should fall
    assert!(positions[10].y < positions[0].y, "Should fall");
    
    // Should settle on floor (Y ~= 1.0 + 0.5 = 1.5, floor top is Y=1, half particle above)
    let final_y = positions.last().unwrap().y;
    assert!((final_y - 1.5).abs() < 0.2, "Should settle near Y=1.5, got {}", final_y);
    
    // Should not explode
    let max_y = positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!(max_y < 15.0, "Should not explode, max_y was {}", max_y);
}

#[test]
fn test_cube_falls_through_gap() {
    // Create floor with gap in center
    let mut occupancy = WorldOccupancy::new();
    for x in 0..10 {
        for z in 0..10 {
            // Gap at (4,5) x (4,5)
            if x < 4 || x > 5 || z < 4 || z > 5 {
                occupancy.set_voxel(IVec3::new(x, 0, z), true);
            }
        }
    }
    
    let initial_pos = Vec3::new(5.0, 10.0, 5.0); // Above the gap
    let initial_vel = Vec3::ZERO;
    let dt = 1.0 / 60.0;
    let steps = 600;
    
    let positions = simulate_single_body_on_terrain(initial_pos, initial_vel, &occupancy, steps, dt);
    
    // Should fall through gap (Y goes negative)
    let min_y = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    assert!(min_y < 0.0, "Should fall through gap, min_y was {}", min_y);
}
```

### Section J: Terrain Collision Checklist

| # | Audit Item | Expected Behavior | Status |
|---|------------|-------------------|--------|
| J1 | Voxel occupancy lookup | `occupancy.get_voxel(ivec3)` returns true for solid | [ ] TODO |
| J2 | Particle-voxel overlap detection | Check if particle sphere intersects voxel cube | [ ] TODO |
| J3 | Voxel center as collision point | Voxel at (x,y,z) has center at (x+0.5, y+0.5, z+0.5) | [ ] TODO |
| J4 | Collision normal from voxel | Normal points from voxel center toward particle | [ ] TODO |
| J5 | Penetration calculation | Same formula as ground: diameter - distance | [ ] TODO |
| J6 | Force formula matches ground | Same spring/damping/tangential as `_collisionReactionWithGround` | [ ] TODO |
| J7 | Multiple voxel force sum | If particle overlaps N voxels, sum N forces | [ ] TODO |
| J8 | Empty voxel = no force | No collision force from unoccupied voxels | [ ] TODO |

---

## PHASE 7: p22 Integration (Full Terrain + GPU Pipeline)

**Outcome:** The p22_voxel_fragment example works correctly with the new physics.

**Verification:** `cargo run --example p22_voxel_fragment` - fragments land on terrain, bounce, settle. Visual confirmation.

### What This Phase IS:
- Wire up the tested physics functions into the existing ECS systems
- Replace broken physics in `voxel_fragment.rs` with calls to new `physics_math.rs` functions
- Verify GPU collision pipeline feeds correct data to physics
- End-to-end visual verification

### What This Phase IS NOT:
- Writing new physics code (that was Phases 1-6)
- Fixing GPU collision detection (out of scope, assume it works)

### Tasks:

- [ ] Update `FragmentCollisionConfig` to use reference constants (Section I)
- [ ] Update `fragment_terrain_collision_system` to use `compute_terrain_collision_force`
- [ ] Update `gpu_fragment_physics_system` to use new integration functions
- [ ] Update `integrate_velocity` call with correct friction order (E1)
- [ ] Update quaternion integration to use correct formula (F2-F5)
- [ ] Test with p22 example:
  - [ ] Fragment spawns and falls
  - [ ] Fragment lands on terrain (no fall-through)
  - [ ] Fragment bounces (not explosively)
  - [ ] Fragment settles within ~5 seconds
  - [ ] No jitter at rest
- [ ] Test with multiple fragments:
  - [ ] Fragments collide with each other
  - [ ] Fragments don't interpenetrate
  - [ ] All fragments eventually settle

### Files to Modify:

| File | Changes |
|------|---------|
| `crates/studio_core/src/physics_math.rs` | Already done in Phases 1-6 |
| `crates/studio_core/src/voxel_fragment.rs` | Replace physics code with calls to `physics_math` |
| `crates/studio_core/src/lib.rs` | Export `physics_math` module |

### Verification Checklist:

| # | Visual Check | Expected | Status |
|---|--------------|----------|--------|
| V1 | Fragment falls | Y decreases over time | [ ] TODO |
| V2 | Fragment hits terrain | Stops falling, bounces | [ ] TODO |
| V3 | Bounce is reasonable | Goes up ~50% of drop height, not higher | [ ] TODO |
| V4 | Fragment settles | Comes to rest within 5 seconds | [ ] TODO |
| V5 | No jitter at rest | Fragment is visually still | [ ] TODO |
| V6 | No explosion | Fragment stays in scene bounds | [ ] TODO |
| V7 | Two fragments collide | They push apart, don't overlap | [ ] TODO |
| V8 | Fragments on slope | Roll/slide down, don't stick | [ ] TODO |

---

## Success Criteria

| Phase | Criterion | How to Verify |
|-------|-----------|---------------|
| 0 | Scaffolding compiles | `cargo test` compiles, `cargo run --example p24` runs |
| 1 | Force computation correct | `cargo test` - force tests pass |
| 2 | Integration correct | `cargo test` - integration tests pass |
| 3 | Aggregation correct | `cargo test` - aggregation tests pass |
| 4 | Single body works | `cargo test test_cube_falls_and_settles` passes |
| 5 | Two bodies work | `cargo test test_two_cubes_collide` passes |
| 6 | Terrain collision works | `cargo test test_cube_on_voxel_floor` passes |
| 7 | p22 integration works | Visual verification - fragments land and settle correctly |

---

## What We Will NOT Do

- Guess at parameter values
- Mix physics approaches  
- Skip verification steps
- Proceed to next phase without current phase verified
- Make "optimizations" before correctness is proven
- Write interactive examples as primary verification (TESTS FIRST)
- Skip unit tests and go straight to visual verification
