# GPU Physics Reference Implementation

This documents the **gpu-physics-unity** reference that our physics system is based on.

**Location:** `gpu-physics-unity/Assets/Physics/`

**CRITICAL RULE:** Match the reference EXACTLY. No adaptations until reference behavior is achieved.

---

## Files

| File | Purpose |
|------|---------|
| `GPUPhysics.cs` | Setup, particle generation, buffer management |
| `GPUPhysicsComputeShader.compute` | Physics kernels (collision, integration) |
| `Quaternion.cginc` | Quaternion math utilities |

---

## Physics Pipeline (Per Frame)

The reference runs these kernels **IN ORDER** each physics tick:

```
1. GenerateParticleValues     (per rigid body) - transform particles to world space
2. ClearGrid                  (per grid cell)  - reset spatial hash
3. PopulateGrid               (per particle)   - insert into spatial hash
4. CollisionDetection         (per particle)   - compute forces
5. ComputeMomenta             (per rigid body) - aggregate to linear/angular
6. ComputePositionAndRotation (per rigid body) - integrate
```

---

## Constants (GPUPhysics.cs)

| Constant | Value | Our Field |
|----------|-------|-----------|
| gravity | 9.8 | `PhysicsConfig::gravity` |
| springCoefficient | 500.0 | `PhysicsConfig::spring_k` |
| dampingCoefficient | 10.0 | `PhysicsConfig::damping_k` |
| tangentialCoefficient | 2.0 | `PhysicsConfig::tangential_k` |
| frictionCoefficient | 0.9 | `PhysicsConfig::friction` |
| angularFrictionCoefficient | 0.3 | `PhysicsConfig::angular_friction` |
| linearForceScalar | 1.0 | `PhysicsConfig::linear_force_scalar` |
| angularForceScalar | 1.0 | `PhysicsConfig::angular_force_scalar` |
| threshold | 1e-6 | `PhysicsConfig::velocity_threshold` |

---

## Surface Particle Generation (GPUPhysics.cs lines 246-261)

Particles are placed on the SURFACE of a cube (hollow shell), not the interior.

```csharp
// Key values
int particlesPerEdge = 4;  // e.g., 4
float particleDiameter = scale / particlesPerEdge;  // e.g., 1.0/4 = 0.25
int particlesPerBody = n^3 - (n-2)^3;  // 64 - 8 = 56 for n=4

// Centering offset
float centerer = scale * -0.5f + particleDiameter * 0.5f;
Vector3 centeringOffset = new Vector3(centerer, centerer, centerer);

// Generate ONLY surface particles
for (int x = 0; x < particlesPerEdge; x++) {
    for (int y = 0; y < particlesPerEdge; y++) {
        for (int z = 0; z < particlesPerEdge; z++) {
            // Surface check: at least one coord at min or max
            if (x == 0 || x == particlesPerEdge-1 ||
                y == 0 || y == particlesPerEdge-1 ||
                z == 0 || z == particlesPerEdge-1) {
                particles[i++] = centeringOffset + 
                    new Vector3(x, y, z) * particleDiameter;
            }
        }
    }
}
```

---

## Collision Force Formula (GPUPhysicsComputeShader.compute)

Same formula for ground, particle-particle, and terrain collision:

```hlsl
// Relative position (line 178 for particles, 224 for ground)
float3 relativePosition = otherPos - myPos;
float distance = length(relativePosition);

// Collision check (line 181/227)
if (distance >= particleDiameter) return float3(0,0,0);

// Normal and penetration (lines 183-186)
float3 normal = relativePosition / distance;
float penetration = particleDiameter - distance;

// Spring force - NEGATIVE (line 186/232)
float3 repulsiveForce = -springCoefficient * penetration * normal;

// Relative velocity (line 192/238)
float3 relativeVelocity = otherVel - myVel;  // ground: otherVel = 0

// Damping force (line 207/240)
float3 dampingForce = dampingCoefficient * relativeVelocity;

// Tangential force (lines 210-211/243-244)
float3 normalVelocity = dot(relativeVelocity, normal) * normal;
float3 tangentialVelocity = relativeVelocity - normalVelocity;
float3 tangentialForce = tangentialCoefficient * tangentialVelocity;

// Total force (line 213/246)
return repulsiveForce + dampingForce + tangentialForce;
```

### Ground Collision Special Case (line 221)

Ground is modeled as a virtual stationary particle at Y = -particleDiameter/2:
```hlsl
float3 groundParticlePosition = float3(particlePos.x, -particleDiameter*0.5, particlePos.z);
```

This means the ground SURFACE is at Y=0.

---

## Force Aggregation (ComputeMomenta, lines 352-366)

```hlsl
float3 linearForce = float3(0,0,0);
float3 angularForce = float3(0,0,0);

for (int i = 0; i < particlesPerRigidBody; i++) {
    int p_id = id.x * particlesPerRigidBody + i;
    float3 relativePosition = particleRelativePositions[p_id];
    
    linearForce += particleForces[p_id];
    angularForce += cross(relativePosition, particleForces[p_id]);
}

float cubeMass = particleMass * particlesPerRigidBody;
```

---

## Velocity Integration (ComputeMomenta, lines 362-381)

**CRITICAL:** Friction is applied BEFORE adding force. Angular has NO inertia division.

```hlsl
// Linear velocity (lines 364-369)
rigidBodyVelocities[id.x] /= 1.0 + deltaTime * frictionCoefficient;  // FRICTION FIRST
rigidBodyVelocities[id.x] += linearForceScalar * deltaTime * linearForce / cubeMass;
if (length(rigidBodyVelocities[id.x]) < threshold) {
    rigidBodyVelocities[id.x] = float3(0,0,0);
}

// Angular velocity (lines 377-381) - NO INERTIA TENSOR!
rigidBodyAngularVelocities[id.x] /= 1.0 + deltaTime * angularFrictionCoefficient;
rigidBodyAngularVelocities[id.x] += angularForceScalar * deltaTime * angularForce;
if (length(rigidBodyAngularVelocities[id.x]) < threshold) {
    rigidBodyAngularVelocities[id.x] = float3(0,0,0);
}
```

---

## Position/Rotation Integration (ComputePositionAndRotation, lines 397-431)

```hlsl
// Position (line 401)
rigidBodyPositions[id.x] += rigidBodyVelocities[id.x] * deltaTime;

// Rotation - quaternion derivative (lines 428-431)
float4 omega = float4(rigidBodyAngularVelocities[id.x], 0);
float4 q = rigidBodyQuaternions[id.x];
rigidBodyQuaternions[id.x] = normalize(q + deltaTime * 0.5 * quat_concat(omega, q));
```

### Quaternion Concat (Quaternion.cginc lines 42-45)

```hlsl
float4 quat_concat(float4 q1, float4 q2) {
    return float4(
        q1.w * q2.xyz + q2.w * q1.xyz + cross(q1.xyz, q2.xyz),
        q1.w * q2.w - dot(q1.xyz, q2.xyz)
    );
}
```

---

## Particle World Transform (GenerateParticleValues, lines 87-101)

```hlsl
for (int i = 0; i < particlesPerRigidBody; i++) {
    int p_id = id.x * particlesPerRigidBody + i;
    
    // Rotate initial position by body quaternion
    float3 relativePosition = quat_mul(rigidBodyQuaternion, 
                                       particleInitialRelativePositions[p_id]);
    particleRelativePositions[p_id] = relativePosition;
    
    // World position = body position + rotated offset
    particlePositions[p_id] = rigidBodyPosition + relativePosition;
    
    // Velocity = body velocity + angular contribution
    particleVelocities[p_id] = rigidBodyVelocity + 
                               cross(rigidBodyAngularVelocity, relativePosition);
}
```

---

## Our Extensions

We extended the reference to support voxel terrain (reference only has flat ground at Y=0):

1. **6-Face Collision:** Check all 6 faces of voxels (top, bottom, +X, -X, +Z, -Z)
2. **Virtual Particle Model:** Each exposed voxel face has a virtual stationary particle
3. **Kinematic Support:** `detect_terrain_collisions()` + `compute_kinematic_correction()` for player controllers

These extensions use the SAME collision math as the reference, just applied to voxel faces instead of a flat ground plane.
