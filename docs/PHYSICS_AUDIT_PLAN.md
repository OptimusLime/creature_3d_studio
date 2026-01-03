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

## Phase 0: Isolated Physics Test Harness

**Outcome:** A minimal example (`p24_physics_audit.rs`) with a flat ground plane (Y=0), no voxel terrain, that we use to verify physics in isolation.

**Verification:** Example compiles and shows a cube falling and landing on Y=0 plane.

### Tasks:
- [ ] Create `examples/p24_physics_audit.rs` - minimal setup with camera, no terrain
- [ ] Add a visual ground plane mesh at Y=0 (just for rendering, not collision)
- [ ] Spawn a single cube fragment at Y=10
- [ ] Ground collision = simple Y=0 plane check (exactly like `_collisionReactionWithGround`)
- [ ] Verify: cube falls due to gravity, stops at Y=0

---

## Phase 1: Fix Force Computation

**Outcome:** Force computation matches reference exactly.

**Verification:** Add debug logging, verify force magnitudes match expected values.

### Tasks:
- [ ] Fix gravity: `force.y -= 9.8` (subtracted, not added as negative)
- [ ] Fix spring force sign: `-springCoefficient * penetration * normal`
- [ ] Fix relative velocity: `ground_vel - my_vel` (zero minus my velocity)
- [ ] Add tangential force properly
- [ ] Match all constants from Section I

---

## Phase 2: Fix Integration

**Outcome:** Integration matches reference exactly.

**Verification:** Drop cube from height, verify it falls correctly, bounces, settles.

### Tasks:
- [ ] Apply friction BEFORE adding forces
- [ ] Use correct velocity update formula with mass
- [ ] Implement velocity threshold zeroing
- [ ] Fix quaternion integration with proper quat_concat

---

## Phase 3: Fix Force Aggregation

**Outcome:** Multiple contact points aggregate correctly.

**Verification:** Cube landing on edge creates torque, rotates as expected.

### Tasks:
- [ ] Sum particle/contact forces for linear force
- [ ] Compute torque using cross(offset, force)
- [ ] Use correct mass calculation

---

## Phase 4: Verify Ground Collision

**Outcome:** Ground collision in p24 matches gpu-physics-unity exactly.

**Verification:** Drop cube from Y=10, it bounces 2-3 times, settles at Y~0.5 (half cube above ground).

### Tasks:
- [ ] All Section A items pass
- [ ] All Section C items pass
- [ ] All Section E items pass
- [ ] All Section F items pass
- [ ] Visual verification: stable settling, no explosion, no jitter

---

## Phase 5: Implement Fragment-Fragment Collision

**Outcome:** Two fragments collide correctly.

**Verification:** Drop two cubes, they bounce off each other and settle.

### Tasks:
- [ ] All Section B items pass
- [ ] Spatial hash grid works (Section H)
- [ ] Verify: fragments collide, don't interpenetrate, settle

---

## Phase 6: Integrate Terrain Occupancy

**Outcome:** Replace Y=0 ground plane with voxel terrain.

**Verification:** p22_voxel_fragment works correctly.

### Tasks:
- [ ] Adapt ground collision to use terrain occupancy
- [ ] Each terrain contact = virtual stationary particle collision
- [ ] Verify: fragments land on terrain correctly

---

## Success Criteria

| Phase | Criterion | How to Verify |
|-------|-----------|---------------|
| 0 | Test harness runs | `cargo run --example p24_physics_audit` shows falling cube |
| 1 | Force audit complete | All Section A, B, C items checked |
| 2 | Integration audit complete | All Section E, F items checked |
| 3 | Aggregation audit complete | All Section D items checked |
| 4 | Ground collision works | Cube bounces and settles, no explosion |
| 5 | Fragment collision works | Two cubes collide correctly |
| 6 | Terrain integration works | p22 fragments land on voxel terrain correctly |

---

## What We Will NOT Do

- Guess at parameter values
- Mix physics approaches  
- Skip verification steps
- Proceed to next phase without current phase verified
- Make "optimizations" before correctness is proven
