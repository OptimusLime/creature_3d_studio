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
**Analysis:** `docs/research/gpu-physics-unity-analysis.md`

---

## Phase 0: Isolated Physics Test Harness

**Outcome:** A minimal example (`p24_physics_audit.rs`) with a flat ground plane (Y=0), no voxel terrain, that we use to verify physics in isolation.

**Verification:** Example compiles and shows a cube falling and landing on Y=0 plane.

### Tasks:
- [ ] Create `examples/p24_physics_audit.rs` - minimal setup with camera, no terrain
- [ ] Add a visual ground plane mesh at Y=0 (just for rendering, not collision)
- [ ] Spawn a single cube fragment at Y=10
- [ ] Ground collision = simple Y=0 plane check (not occupancy)
- [ ] Verify: cube falls due to gravity, stops at Y=0

---

## Phase 1: Audit - Force Computation

**Outcome:** Document exactly how gpu-physics-unity computes forces, line by line.

**Verification:** Markdown section with code snippets and our equivalent code side-by-side.

### Audit Items:
- [ ] **1.1 Gravity application** - Where/how is gravity added?
  - Reference: `ComputeParticleForces` line ~261: `force.y -= gravityCoefficient;`
  - Our code: ???
  
- [ ] **1.2 Spring force formula** - Exact formula and sign
  - Reference: `_collisionReaction` line ~287: `repulsiveForce = -springCoefficient * (particleDiameter - relativePositionMagnitude) * n`
  - Our code: ???
  - Key: `n` points FROM i TO j, force is NEGATIVE (pushes apart)
  
- [ ] **1.3 Damping force formula** - Exact formula
  - Reference: `_collisionReaction` line ~291: `dampingForce = dampingCoefficient * relativeVelocity`
  - Our code: ???
  - Key: `relativeVelocity = v[j] - v[i]` (other minus self)
  
- [ ] **1.4 Tangential force formula** - Exact formula
  - Reference: `_collisionReaction` lines ~296-297
  - Our code: ???
  
- [ ] **1.5 Ground collision** - How ground is handled
  - Reference: `_collisionReactionWithGround` - treats ground as virtual particle at Y=0
  - Our code: ???

---

## Phase 2: Audit - Integration

**Outcome:** Document exactly how gpu-physics-unity integrates velocity and position.

**Verification:** Markdown section with code snippets and our equivalent code side-by-side.

### Audit Items:
- [ ] **2.1 Velocity integration** - Exact formula
  - Reference: `ComputeMomenta` line ~349: `rigidBodyVelocities[id.x] += linearForceScalar * deltaTime * linearForce / cubeMass`
  - Our code: ???
  
- [ ] **2.2 Friction damping** - How velocity is damped
  - Reference: `ComputeMomenta` line ~346: `rigidBodyVelocities[id.x] /= 1.0 + deltaTime * frictionCoefficient`
  - Our code: ???
  
- [ ] **2.3 Position integration** - Exact formula
  - Reference: `ComputePosition` line ~389: `rigidBodyPositions[id.x] += rigidBodyVelocities[id.x] * deltaTime`
  - Our code: ???
  
- [ ] **2.4 Angular velocity integration** - Exact formula
  - Reference: `ComputeMomenta` lines ~352-355
  - Our code: ???
  
- [ ] **2.5 Rotation integration** - How quaternion is updated
  - Reference: `ComputePosition` lines ~391-397
  - Our code: ???

---

## Phase 3: Audit - Force Aggregation

**Outcome:** Document how particle forces become rigid body forces.

**Verification:** Markdown section with code snippets.

### Audit Items:
- [ ] **3.1 Linear force aggregation** - How particle forces sum to body force
  - Reference: `ComputeMomenta` line ~337: `linearForce += particleForces[p_id]`
  - Our code: ???
  
- [ ] **3.2 Torque computation** - How off-center forces create torque
  - Reference: `ComputeMomenta` line ~340: `angularForce += cross(relativePosition, particleForces[p_id])`
  - Our code: ???
  
- [ ] **3.3 Mass computation** - How mass is determined
  - Reference: `ComputeMomenta` line ~343: `cubeMass = particleMass * particlesPerRigidBody`
  - Our code: ???

---

## Phase 4: Implement Faithful Ground Collision

**Outcome:** Ground collision in p24 matches gpu-physics-unity exactly.

**Verification:** Drop cube from Y=10, it bounces and settles at Y=0. No explosion, no jitter, stable rest.

### Tasks:
- [ ] Implement ground collision as virtual stationary particle (like `_collisionReactionWithGround`)
- [ ] Use EXACT same formulas from Phase 1 audit
- [ ] Use EXACT same integration from Phase 2 audit
- [ ] Tune spring_k, damping_k to match reference defaults
- [ ] Verify: cube bounces 2-3 times, settles within 5 seconds

---

## Phase 5: Implement Faithful Fragment-Fragment Collision

**Outcome:** Two fragments collide with each other correctly.

**Verification:** Drop two cubes, they bounce off each other and settle.

### Tasks:
- [ ] Add second fragment in p24
- [ ] Implement spatial hash grid lookup (already exists)
- [ ] Use EXACT same collision force formulas
- [ ] Verify: fragments collide, don't interpenetrate, settle

---

## Phase 6: Integrate Terrain Occupancy

**Outcome:** Replace Y=0 ground plane with voxel terrain occupancy.

**Verification:** p22_voxel_fragment works correctly - fragments land on terrain, bounce, settle.

### Tasks:
- [ ] Replace ground plane check with terrain occupancy lookup
- [ ] Aggregate terrain contacts into single collision (max penetration, avg normal)
- [ ] Verify: fragments interact correctly with arbitrary terrain shapes

---

## Constants Reference

From gpu-physics-unity `GPUPhysics.cs`:
```csharp
public float gravityCoefficient = 9.8f;
public float particleDiameter = 1f;
public float springCoefficient = 500f;   // NOTE: Much lower than our 2000
public float dampingCoefficient = 10f;
public float tangentialCoefficient = 2f;
public float frictionCoefficient = 0.9f;
public float angularFrictionCoefficient = 0.3f;
```

---

## Success Criteria

| Phase | Criterion | How to Verify |
|-------|-----------|---------------|
| 0 | Test harness runs | `cargo run --example p24_physics_audit` shows falling cube |
| 1 | Force audit complete | All 1.x items checked, code snippets documented |
| 2 | Integration audit complete | All 2.x items checked, code snippets documented |
| 3 | Aggregation audit complete | All 3.x items checked, code snippets documented |
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
