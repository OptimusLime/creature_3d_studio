# GPU Physics Unity - Implementation Analysis

## Overview

This document analyzes the gpu-physics-unity project, a GPU-accelerated voxel physics solver based on Takahiro Harada's work from GPU Gems 3 (Chapter 29: "Real-Time Rigid Body Simulation on GPUs"). The implementation demonstrates how to simulate 64,000+ cubes with full rigid body physics entirely on the GPU.

**Source:** https://github.com/00jknight/GPU-Physics-Unity
**Reference:** http://www.00jknight.com/blog/gpu-accelerated-voxel-physics-solver

---

## Core Architecture

### Philosophy: Zero CPU-GPU Transfer at Runtime

The key design principle is that **no per-particle data transfers between GPU and CPU during simulation**. All physics state lives entirely on the GPU:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              GPU MEMORY                                  │
├─────────────────────────────────────────────────────────────────────────┤
│  Rigid Body State                                                        │
│  ├── positions[]         (float3 × N bodies)                            │
│  ├── quaternions[]       (float4 × N bodies)                            │
│  ├── velocities[]        (float3 × N bodies)                            │
│  └── angularVelocities[] (float3 × N bodies)                            │
├─────────────────────────────────────────────────────────────────────────┤
│  Particle State (particles are collision proxies on body surfaces)       │
│  ├── positions[]         (float3 × M particles)                         │
│  ├── velocities[]        (float3 × M particles)                         │
│  ├── forces[]            (float3 × M particles)                         │
│  └── relativePositions[] (float3 × M particles)                         │
├─────────────────────────────────────────────────────────────────────────┤
│  Spatial Hash Grid                                                       │
│  └── voxelCollisionGrid[] (int4 × grid cells) - stores particle IDs     │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ Dispatch compute shaders
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          COMPUTE PIPELINE                                │
│  1. GenerateParticleValues (per rigid body)                             │
│  2. ClearGrid              (per grid cell)                              │
│  3. PopulateGrid           (per particle)                               │
│  4. CollisionDetection     (per particle)                               │
│  5. ComputeMomenta         (per rigid body)                             │
│  6. ComputePositionAndRotation (per rigid body)                         │
│  7. SavePreviousPositionAndRotation (for interpolation)                 │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ DrawMeshInstancedIndirect
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          INSTANCED RENDERING                             │
│  Reads positions[] and quaternions[] directly from GPU buffers          │
│  No CPU involvement for rendering                                        │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Data Structures

### Rigid Body Representation

Each rigid body is a cube with particles on its surface that act as collision proxies:

```
┌───────────────┐
│               │  Rigid body (cube) has:
│  ●   ●   ●    │  - Position (center)
│               │  - Quaternion (rotation)
│  ●       ●    │  - Linear velocity
│               │  - Angular velocity
│  ●   ●   ●    │
└───────────────┘
       ●
    Particles (8-98 per body depending on particlesPerEdge)
```

**Key Insight:** Particles are NOT independent entities. They are fixed relative to their parent rigid body. They exist only to:
1. Detect collisions via spatial hashing
2. Accumulate forces that translate back to rigid body momenta

### Particle Distribution

Particles are placed only on the SURFACE of the cube (hollow interior):

```csharp
// From GPUPhysics.cs lines 249-258
for (int xIter = 0; xIter < particlesPerEdge; xIter++) {
    for (int yIter = 0; yIter < particlesPerEdge; yIter++) {
        for (int zIter = 0; zIter < particlesPerEdge; zIter++) {
            // Only add particles on edges (surface)
            if (xIter == 0 || xIter == (particlesPerEdge-1) || 
                yIter == 0 || yIter == (particlesPerEdge-1) || 
                zIter == 0 || zIter == (particlesPerEdge-1)) {
                particleInitialsSmall[initialRelativePositionIterator] = 
                    centeringOffset + new Vector3(xIter*particleDiameter, yIter*particleDiameter, zIter*particleDiameter);
            }
        }
    }
}
```

Formula for particles per body:
```
particlesPerBody = particlesPerEdge³ - (particlesPerEdge-2)³
```

| particlesPerEdge | Total particles | Surface particles |
|------------------|-----------------|-------------------|
| 2                | 8               | 8                 |
| 3                | 27              | 26                |
| 4                | 64              | 56                |
| 5                | 125             | 98                |

### Spatial Hash Grid

The grid stores up to 4 particle IDs per cell using `int4`:

```hlsl
// From GPUPhysicsComputeShader.compute line 118
RWStructuredBuffer<int4> voxelCollisionGrid;
```

Why 4 particles per cell?
- Trade-off between memory and collision accuracy
- Overflow is handled by ignoring additional particles (collisions may be missed)
- Uses `InterlockedCompareExchange` for thread-safe insertion

```hlsl
// Atomic insertion into grid cell (lines 149-156)
InterlockedCompareExchange(voxelCollisionGrid[gridIndex].x, -1, p_id, originalValue);
if (originalValue != -1)
    InterlockedCompareExchange(voxelCollisionGrid[gridIndex].y, -1, p_id, originalValue);
if (originalValue != -1)
    InterlockedCompareExchange(voxelCollisionGrid[gridIndex].z, -1, p_id, originalValue);
if (originalValue != -1)
    InterlockedCompareExchange(voxelCollisionGrid[gridIndex].w, -1, p_id, originalValue);
```

---

## Compute Shader Pipeline

### Kernel 1: GenerateParticleValues (Per Rigid Body)

**Purpose:** Transform particle positions from local space to world space based on current rigid body state.

**Input:** 
- Rigid body positions, quaternions, velocities, angular velocities
- Particle initial relative positions (constant, set once at init)

**Output:**
- Particle world positions
- Particle world velocities (linear + angular contribution)
- Particle relative positions (rotated by current quaternion)

```hlsl
// lines 87-101
[numthreads(RIGID_BODY_THREAD_COUNT,1,1)]
void GenerateParticleValues (uint3 id : SV_DispatchThreadID)
{
    float3 rigidBodyPosition = rigidBodyPositions[id.x];
    float4 rigidBodyQuaternion = rigidBodyQuaternions[id.x];
    float3 rigidBodyAngularVelocity = rigidBodyAngularVelocities[id.x];
    float3 rigidBodyVelocity = rigidBodyVelocities[id.x];

    for (int i = 0; i < particlesPerRigidBody; i++) {
        int p_id = id.x * particlesPerRigidBody + i;
        
        // Rotate particle's local offset by body's quaternion
        particleRelativePositions[p_id] = rotateVectorByQuaternion(rigidBodyQuaternion, 
                                                                    particleInitialRelativePositions[p_id]);
        
        // World position = body center + rotated offset
        particlePositions[p_id] = rigidBodyPosition + particleRelativePositions[p_id];
        
        // Particle velocity = body linear velocity + cross(angular_velocity, offset)
        particleVelocities[p_id] = rigidBodyVelocity + cross(rigidBodyAngularVelocity, 
                                                              particleRelativePositions[p_id]);
    }
}
```

**Key Math:** Particle velocity includes rotational component:
```
v_particle = v_body + ω × r
```
Where `r` is the particle's position relative to body center.

### Kernel 2: ClearGrid (Per Grid Cell)

**Purpose:** Reset all grid cells to empty (-1) before repopulation.

```hlsl
// lines 121-128
[numthreads(CLEAR_GRID_THREAD_COUNT,1,1)]
void ClearGrid (uint3 id : SV_DispatchThreadID)
{
    voxelCollisionGrid[id.x].r = -1;
    voxelCollisionGrid[id.x].g = -1;
    voxelCollisionGrid[id.x].b = -1;
    voxelCollisionGrid[id.x].a = -1;
}
```

**Note:** This step is identified as a potential optimization target - could be eliminated with double-buffering or frame counters.

### Kernel 3: PopulateGrid (Per Particle)

**Purpose:** Insert each particle into the spatial hash grid based on its world position.

```hlsl
// Grid index calculation (lines 132-138)
int _gridIndex(int p_id) {
    int3 gridLocation = (particlePositions[p_id] - gridStartPosition) / particleDiameter;
    return gridLocation.x + gridDimensions.x * gridLocation.y + 
           (gridDimensions.x * gridDimensions.y * gridLocation.z);
}

// Atomic insertion (lines 140-158)
[numthreads(PARTICLE_THREAD_COUNT,1,1)]
void PopulateGrid (uint3 id : SV_DispatchThreadID)
{
    int p_id = id.x;
    int gridIndex = _gridIndex(p_id);
    
    if (gridIndex < gridMax && gridIndex > -1)
    {
        int originalValue = 0;
        InterlockedCompareExchange(voxelCollisionGrid[gridIndex].x, -1, p_id, originalValue);
        // ... fallback to y, z, w if x is occupied
    }
}
```

### Kernel 4: CollisionDetection (Per Particle)

**Purpose:** For each particle, check neighboring grid cells for collisions and compute forces.

**Neighbor Search:** Each particle checks its own cell plus all 26 neighbors (3³ - 1):

```hlsl
// lines 291-329
[numthreads(PARTICLE_THREAD_COUNT,1,1)]
void CollisionDetection (uint3 id : SV_DispatchThreadID)
{
    int i = id.x;
    int3 i_gridLocation = _gridIndexThree(i);
    float3 force = float3(0,0,0);
    
    // Check all 27 neighboring cells (including own)
    for (dx = -1 to 1)
        for (dy = -1 to 1)
            for (dz = -1 to 1)
                force += _checkGridCell(i, i_gridLocation.x+dx, i_gridLocation.y+dy, i_gridLocation.z+dz);
    
    // Apply gravity
    force.y -= gravityCoefficient;
    
    // Ground collision
    force += _collisionReactionWithGround(i);
    
    particleForces[i] = force;
}
```

**Collision Response (Harada Model):**

Based on GPU Gems 3, the collision force has three components:

```hlsl
// lines 174-216
float3 _collisionReaction(int j_id, int i_id)
{
    float3 relativePosition = particlePositions[j_id] - particlePositions[i_id];
    float relativePositionMagnitude = length(relativePosition);

    if (relativePositionMagnitude < particleDiameter)
    {
        float3 n = relativePosition / relativePositionMagnitude;

        // 1. REPULSIVE FORCE (Spring - Equation 10)
        // Pushes particles apart when overlapping
        float3 repulsiveForce = -springCoefficient * (particleDiameter - relativePositionMagnitude) * n;
        
        // 2. DAMPING FORCE (Equation 11)
        // Dissipates energy based on relative velocity
        float3 relativeVelocity = particleVelocities[j_id] - particleVelocities[i_id];
        float3 dampingForce = dampingCoefficient * relativeVelocity;

        // 3. TANGENTIAL FORCE (Friction - Equation 12)
        // Resists sliding motion perpendicular to contact normal
        float3 tangentialVelocity = relativeVelocity - (dot(relativeVelocity, n) * n);
        float3 tangentialForce = tangentialCoefficient * tangentialVelocity;

        return repulsiveForce + dampingForce + tangentialForce;
    }
    return float3(0,0,0);
}
```

**Mathematical Model:**

```
F_repulsive = -k_spring * (d - |r|) * n̂     (Hooke's law)
F_damping = k_damp * (v_j - v_i)             (Velocity damping)
F_tangential = k_tan * v_tangent             (Friction)

Where:
  d = particle diameter
  r = relative position vector
  n̂ = normalized contact normal
  v_tangent = v_rel - (v_rel · n̂) * n̂
```

### Kernel 5: ComputeMomenta (Per Rigid Body)

**Purpose:** Aggregate particle forces into linear and angular momentum changes for the rigid body.

```hlsl
// lines 348-382
[numthreads(RIGID_BODY_THREAD_COUNT,1,1)]
void ComputeMomenta (uint3 id : SV_DispatchThreadID)
{
    float3 linearForce = float3(0,0,0);
    float3 angularForce = float3(0,0,0);

    for (int i = 0; i < particlesPerRigidBody; i++) 
    {
        int p_id = id.x * particlesPerRigidBody + i;
        float3 relativePosition = particleRelativePositions[p_id];
        
        // Sum all particle forces for linear momentum
        linearForce += particleForces[p_id];
        
        // Torque = r × F (cross product of offset and force)
        angularForce += cross(relativePosition, particleForces[p_id]);
    }
    
    float cubeMass = particleMass * particlesPerRigidBody;
    
    // Apply friction damping
    rigidBodyVelocities[id.x] /= 1.0 + deltaTime * frictionCoefficient;
    
    // Update linear velocity: v += (F/m) * dt
    rigidBodyVelocities[id.x] += linearForceScalar * deltaTime * linearForce / cubeMass;
    
    // Apply angular friction damping
    rigidBodyAngularVelocities[id.x] /= 1.0 + deltaTime * angularFrictionCoefficient;
    
    // Update angular velocity
    rigidBodyAngularVelocities[id.x] += angularForceScalar * deltaTime * angularForce;
}
```

**Key Physics:**

Linear momentum update:
```
v_new = v_old / (1 + dt * k_friction) + (dt * F_total) / m
```

Angular momentum update:
```
ω_new = ω_old / (1 + dt * k_angular_friction) + dt * τ_total
```

Where torque τ = Σ(r_i × F_i) for all particles.

**Note:** The implementation simplifies the inertia tensor handling. Full rigid body dynamics would use:
```
τ = I * α  →  α = I⁻¹ * τ
```
Where I is the rotated inertia tensor. The code has commented-out attempts at proper inertia handling.

### Kernel 6: ComputePositionAndRotation (Per Rigid Body)

**Purpose:** Integrate velocities to update positions and rotations.

```hlsl
// lines 397-432
[numthreads(RIGID_BODY_THREAD_COUNT,1,1)]
void ComputePositionAndRotation (uint3 id : SV_DispatchThreadID)
{
    // Position integration: x = x + v * dt
    rigidBodyPositions[id.x] = rigidBodyPositions[id.x] + rigidBodyVelocities[id.x] * deltaTime;

    // Quaternion integration using angular velocity
    float3 rigidBodyAngularVelocity = rigidBodyAngularVelocities[id.x];
    
    // Convert angular velocity to quaternion derivative
    // dq/dt = 0.5 * ω * q  (where ω is treated as a pure quaternion)
    float4 omega = float4(rigidBodyAngularVelocity, 0);
    float4 q = rigidBodyQuaternions[id.x];
    
    // Euler integration of quaternion
    rigidBodyQuaternions[id.x] = normalize(q + deltaTime * (0.5 * quat_concat(omega, q)));
}
```

**Quaternion Integration Math:**

The quaternion derivative is:
```
dq/dt = ½ * ω * q
```

Where ω is the angular velocity vector embedded as a pure quaternion (w=0).

Integration:
```
q_new = normalize(q_old + dt * dq/dt)
       = normalize(q_old + dt * ½ * ω * q_old)
```

The `quat_concat` function performs quaternion multiplication:
```hlsl
// From Quaternion.cginc line 42-45
inline float4 quat_concat(float4 q1, float4 q2)
{
    return float4(q1.w * q2.xyz + q2.w * q1.xyz + cross(q1.xyz, q2.xyz), 
                  q1.w * q2.w - dot(q1.xyz, q2.xyz));
}
```

### Kernel 7: SavePreviousPositionAndRotation (Per Rigid Body)

**Purpose:** Store current state for interpolated rendering.

```hlsl
// lines 436-441
[numthreads(RIGID_BODY_THREAD_COUNT,1,1)]
void SavePreviousPositionAndRotation (uint3 id : SV_DispatchThreadID)
{
    previousRigidBodyPositions[id.x] = rigidBodyPositions[id.x];
    previousRigidBodyQuaternions[id.x] = rigidBodyQuaternions[id.x];
}
```

Used for smooth rendering when physics runs at fixed timestep.

---

## Rendering Pipeline

### Instanced Indirect Rendering

The render shader reads directly from GPU buffers without CPU involvement:

```hlsl
// From InstancedIndirectSurfaceShader.shader lines 36-83
#ifdef UNITY_PROCEDURAL_INSTANCING_ENABLED
    StructuredBuffer<float3> positions;
    StructuredBuffer<float4> quaternions;
    StructuredBuffer<float3> previousPositions;
    StructuredBuffer<float4> previousQuaternions;
    float blendAlpha;
#endif

void setup()
{
#ifdef UNITY_PROCEDURAL_INSTANCING_ENABLED
    // Build transform matrix from quaternion
    float4x4 rotation = quaternion_to_matrix(quaternions[unity_InstanceID]);
    float3 position = positions[unity_InstanceID];
    
    // Translation matrix
    float4x4 translation = {
        1,0,0,position.x,
        0,1,0,position.y,
        0,0,1,position.z,
        0,0,0,1
    };
    
    // Combined object-to-world transform
    unity_ObjectToWorld = mul(translation, rotation);
    // ... also compute inverse for lighting
#endif
}
```

### Position Interpolation

The renderer can interpolate between physics frames for smooth 60fps display even with slower physics update:

```csharp
// From GPUPhysics.cs lines 442-451
ticker += Time.deltaTime;
float _dt = 1.0f / tick_rate;
while (ticker >= _dt) {
    ticker -= _dt;
    m_computeShader.SetFloat(m_deltaTimeShaderProperty, dt);
    Graphics.ExecuteCommandBuffer(m_commandBuffer);
}
float blendAlpha = ticker / _dt;
cubeMaterial.SetFloat("blendAlpha", blendAlpha);
```

The shader can then lerp/slerp between previous and current states.

---

## Physics Parameters

| Parameter | Description | Typical Value |
|-----------|-------------|---------------|
| `springCoefficient` | Stiffness of collision response | Negative (pushes apart) |
| `dampingCoefficient` | Energy dissipation on collision | 0.1 - 0.5 |
| `tangentialCoefficient` | Surface friction | 0.1 - 0.3 |
| `frictionCoefficient` | Linear velocity damping | 0.01 - 0.1 |
| `angularFrictionCoefficient` | Angular velocity damping | 0.01 - 0.1 |
| `gravityCoefficient` | Gravitational acceleration | 9.8 |
| `particleDiameter` | Size of collision spheres | scale / particlesPerEdge |
| `particleMass` | Mass per particle | cubeMass / particlesPerBody |

---

## Key Implementation Details

### Inertia Tensor (Simplified)

For a cube, the inertia tensor is diagonal and equal on all axes:
```csharp
// From GPUPhysics.cs lines 222-228
float twoDimSq = 2.0f * (scale * scale);
float inertialTensorFactor = m_cubeMass * 1.0f / 12.0f * twoDimSq;
float[] inertialTensor = {
    inertialTensorFactor, 0.0f, 0.0f,
    0.0f, inertialTensorFactor, 0.0f,
    0.0f, 0.0f, inertialTensorFactor
};
```

The simplified angular update ignores proper inertia tensor rotation (noted as "probably wrong" in comments).

### Ground Collision

Ground is handled as a special case - an infinite plane at y=0:

```hlsl
// From GPUPhysicsComputeShader.compute lines 218-251
float3 _collisionReactionWithGround(int i_id) {
    float3 groundParticlePosition = particlePositions[i_id];
    groundParticlePosition.y = -particleDiameter*0.5;
    
    // Same collision response as particle-particle
    float3 relativePosition = groundParticlePosition - particlePositions[i_id];
    // ... spring + damping + tangential forces
}
```

### Thread Group Sizing

```csharp
// From GPUPhysics.cs lines 172-174
m_threadGroupsPerRigidBody = Mathf.CeilToInt(total / 8.0f);
m_threadGroupsPerParticle = Mathf.CeilToInt(n_particles / 8f);
m_threadGroupsPerGridCell = Mathf.CeilToInt((gridX * gridY * gridZ) / 8f);
```

All kernels use 8 threads per group (could be optimized for specific GPU architectures).

---

## Known Limitations (From README)

1. **Voxel Grid is Static** - Grid bounds don't adapt to simulation bounds
2. **Grid Clear Step** - Could be eliminated with smarter data structures
3. **Damping/Tangential Forces** - "do not seem to have good effects" per original author
4. **Shadow Pass** - May need optimization
5. **Inertia Tensor** - Simplified implementation, not physically accurate for rotation

---

## Comparison to Our Current System

| Aspect | gpu-physics-unity | Our GPU Collision |
|--------|-------------------|-------------------|
| Body representation | Particles on surface | AABB or voxel occupancy |
| Collision detection | Spatial hash grid | Terrain texture lookup |
| Physics integration | Fully on GPU | GPU detection, CPU integration |
| Rotation support | Full quaternion | Fixed for floor contacts |
| Inter-body collision | Yes (particle-particle) | No (terrain only) |
| Ground handling | Special-cased plane | Terrain voxels |

---

## Concepts to Integrate

### High Priority (Direct Application)

1. **Surface Particle Representation** - Place collision spheres on body surfaces instead of checking every voxel
2. **Spatial Hash Grid** - For fragment-fragment collision, not just terrain
3. **Torque from Off-Center Forces** - Already in our design, needs better implementation
4. **Quaternion Integration** - Proper angular velocity → quaternion update

### Medium Priority (Adaptation Required)

1. **Full GPU Integration** - Move velocity/position updates to GPU
2. **Instanced Rendering** - Read transforms directly from GPU buffers
3. **Fixed Timestep with Interpolation** - Smooth rendering at any framerate

### Lower Priority (Future)

1. **Damping/Friction Tuning** - The original notes these need work
2. **Inertia Tensor Rotation** - For accurate rotation physics
3. **Dynamic Grid Bounds** - Adapt grid to simulation area

---

## References

1. Harada, T. "Real-Time Rigid Body Simulation on GPUs" GPU Gems 3, Chapter 29
   https://developer.nvidia.com/gpugems/gpugems3/part-v-physics-simulation/chapter-29-real-time-rigid-body-simulation-gpus

2. Fiedler, G. "Fix Your Timestep!" 
   https://gafferongames.com/post/fix_your_timestep/

3. Van Verth, J. & Bishop, L. "Essential Mathematics for Games and Interactive Applications"
   (Referenced for inertia tensor formulas and quaternion integration)

4. CJ Lib (Quaternion utilities)
   https://github.com/TheAllenChou/unity-cj-lib
