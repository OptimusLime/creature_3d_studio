//! Pure physics math functions for spring-damper rigid body simulation.
//!
//! This module implements Harada's spring-damper model from GPU Gems 3,
//! faithfully reproducing the gpu-physics-unity reference implementation.
//!
//! **Reference:** `gpu-physics-unity/Assets/Physics/GPUPhysicsComputeShader.compute`
//!
//! # Design Principles
//!
//! - **Pure functions**: No ECS, no Bevy, just math
//! - **Exact match**: Constants and formulas match the reference exactly
//! - **Testable**: Each function can be unit tested in isolation
//!
//! # Physics Pipeline (per frame)
//!
//! 1. Generate particle values (transform local -> world)
//! 2. Collision detection (ground + particle-particle) -> particle forces
//! 3. Aggregate particle forces -> rigid body linear/angular forces
//! 4. Integrate velocity (friction FIRST, then forces)
//! 5. Integrate position and rotation

use bevy::math::{IVec3, Quat, UVec3, Vec3};

/// Physics configuration with reference constants from gpu-physics-unity.
///
/// These values are from `GPUPhysics.cs` and MUST NOT be changed until
/// physics is working correctly.
#[derive(Debug, Clone)]
pub struct PhysicsConfig {
    /// Gravity acceleration (subtracted from Y force per particle)
    /// Reference: `gravityCoefficient = 9.8` (GPUPhysics.cs line 39)
    pub gravity: f32,

    /// Diameter of collision particles (determines collision distance)
    /// Reference: `particleDiameter = 1.0`
    pub particle_diameter: f32,

    /// Spring stiffness for collision response (Hooke's law)
    /// Reference: `springCoefficient = 500.0`
    pub spring_k: f32,

    /// Velocity damping coefficient
    /// Reference: `dampingCoefficient = 10.0`
    pub damping_k: f32,

    /// Tangential/friction coefficient for sliding
    /// Reference: `tangentialCoefficient = 2.0`
    pub tangential_k: f32,

    /// Linear velocity friction (applied BEFORE forces)
    /// Reference: `frictionCoefficient = 0.9`
    pub friction: f32,

    /// Angular velocity friction (applied BEFORE forces)
    /// Reference: `angularFrictionCoefficient = 0.3`
    pub angular_friction: f32,

    /// Linear force multiplier
    /// Reference: `linearForceScalar = 1.0`
    pub linear_force_scalar: f32,

    /// Angular force multiplier
    /// Reference: `angularForceScalar = 1.0`
    pub angular_force_scalar: f32,

    /// Velocity threshold below which velocity is zeroed
    /// Reference: `threshold = 1e-6` (line 362)
    pub velocity_threshold: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: 9.8,
            particle_diameter: 1.0,
            spring_k: 500.0,
            damping_k: 10.0,
            tangential_k: 2.0,
            friction: 0.9,
            angular_friction: 0.3,
            linear_force_scalar: 1.0,
            angular_force_scalar: 1.0,
            velocity_threshold: 1e-6,
        }
    }
}

// =============================================================================
// PHASE 1: Force Computation (Sections A, B, C)
// =============================================================================

/// Compute collision force with ground plane at Y=0.
///
/// The ground is modeled as a virtual stationary particle at Y = -particle_diameter/2,
/// meaning the ground SURFACE is at Y=0.
///
/// Reference: `_collisionReactionWithGround` (lines 218-251)
///
/// # Arguments
/// * `particle_pos` - World position of the particle
/// * `particle_vel` - World velocity of the particle
/// * `config` - Physics configuration
///
/// # Returns
/// Force vector to apply to the particle (repulsive + damping + tangential)
pub fn compute_ground_collision_force(
    particle_pos: Vec3,
    particle_vel: Vec3,
    config: &PhysicsConfig,
) -> Vec3 {
    // Reference: _collisionReactionWithGround (lines 218-251)
    //
    // The ground is modeled as a virtual stationary particle directly below
    // the real particle, at Y = -particleDiameter * 0.5
    // This means the ground SURFACE is at Y = 0

    // A1: Ground particle Y position
    // groundParticlePosition.y = -particleDiameter * 0.5 (line 221)
    let ground_particle_pos = Vec3::new(
        particle_pos.x,
        -config.particle_diameter * 0.5,
        particle_pos.z,
    );

    // A2: Relative position direction - points FROM particle TO ground
    // relativePosition = groundParticlePosition - particlePositions[i] (line 224)
    let relative_position = ground_particle_pos - particle_pos;
    let relative_position_magnitude = relative_position.length();

    // A3: Collision condition
    // if (relativePositionMagnitude < particleDiameter) (line 227)
    if relative_position_magnitude >= config.particle_diameter {
        return Vec3::ZERO; // No collision
    }

    // Avoid division by zero
    if relative_position_magnitude < 1e-8 {
        return Vec3::ZERO;
    }

    // A4: Normal direction - points FROM particle TO ground (downward)
    // relativePositionNormalized = relativePosition / relativePositionMagnitude (line 229)
    let n = relative_position / relative_position_magnitude;

    // A5: Penetration calculation
    // penetration = particleDiameter - relativePositionMagnitude
    let penetration = config.particle_diameter - relative_position_magnitude;

    // A6: Spring force (Equation 10) - NEGATIVE because n points toward ground
    // but we want force to push particle AWAY from ground (upward)
    // repulsiveForce = -springCoefficient * penetration * n (line 232)
    let repulsive_force = -config.spring_k * penetration * n;

    // A7: Ground velocity is ZERO (stationary)
    // A8: Relative velocity = ground_vel - particle_vel = -particle_vel
    // relativeVelocity = float3(0,0,0) - particleVelocities[i] (line 238)
    let relative_velocity = Vec3::ZERO - particle_vel;

    // A9: Damping force (Equation 11)
    // dampingForce = dampingCoefficient * relativeVelocity (line 240)
    let damping_force = config.damping_k * relative_velocity;

    // A10: Normal velocity projection
    // normalVelocity = dot(relativeVelocity, n) * n
    let normal_velocity = relative_velocity.dot(n) * n;

    // A11: Tangential velocity
    // tangentialVelocity = relativeVelocity - normalVelocity (line 243)
    let tangential_velocity = relative_velocity - normal_velocity;

    // A12: Tangential force (Equation 12)
    // tangentialForce = tangentialCoefficient * tangentialVelocity (line 244)
    let tangential_force = config.tangential_k * tangential_velocity;

    // A13: Force sum
    // return repulsiveForce + dampingForce + tangentialForce (line 246)
    repulsive_force + damping_force + tangential_force
}

/// Compute collision force between two particles.
///
/// Returns the force on particle i due to collision with particle j.
///
/// Reference: `_collisionReaction` (lines 174-216)
///
/// # Arguments
/// * `pos_i` - Position of particle i (the one receiving force)
/// * `vel_i` - Velocity of particle i
/// * `pos_j` - Position of particle j (the colliding particle)
/// * `vel_j` - Velocity of particle j
/// * `config` - Physics configuration
///
/// # Returns
/// Force vector to apply to particle i
pub fn compute_particle_collision_force(
    pos_i: Vec3,
    vel_i: Vec3,
    pos_j: Vec3,
    vel_j: Vec3,
    config: &PhysicsConfig,
) -> Vec3 {
    // Reference: _collisionReaction (lines 174-216)
    //
    // Computes the force on particle i due to collision with particle j

    // B1: Relative position direction - points FROM i TO j
    // relativePosition = particlePositions[j] - particlePositions[i] (line 178)
    let relative_position = pos_j - pos_i;
    let relative_position_magnitude = relative_position.length();

    // B2: Collision condition
    // if (relativePositionMagnitude < particleDiameter) (line 181)
    if relative_position_magnitude >= config.particle_diameter {
        return Vec3::ZERO; // No collision
    }

    // Avoid division by zero
    if relative_position_magnitude < 1e-8 {
        return Vec3::ZERO;
    }

    // B3: Normal direction - points FROM i TO j
    // relativePositionNormalized = relativePosition / relativePositionMagnitude (line 183)
    let n = relative_position / relative_position_magnitude;

    // B4: Penetration calculation
    // penetration = particleDiameter - relativePositionMagnitude (line 186)
    let penetration = config.particle_diameter - relative_position_magnitude;

    // B5: Spring force (Equation 10) - NEGATIVE because n points toward j
    // but we want force to push i AWAY from j
    // repulsiveForce = -springCoefficient * penetration * n (line 186)
    let repulsive_force = -config.spring_k * penetration * n;

    // B6: Relative velocity direction
    // relativeVelocity = particleVelocities[j] - particleVelocities[i] (line 192)
    let relative_velocity = vel_j - vel_i;

    // B7: Damping force (Equation 11)
    // dampingForce = dampingCoefficient * relativeVelocity (line 207)
    let damping_force = config.damping_k * relative_velocity;

    // B8: Tangential velocity
    // tangentialVelocity = relativeVelocity - dot(rel_vel, n) * n (lines 210-211)
    let normal_velocity = relative_velocity.dot(n) * n;
    let tangential_velocity = relative_velocity - normal_velocity;

    // B9: Tangential force (Equation 12)
    // tangentialForce = tangentialCoefficient * tangentialVelocity (line 211)
    let tangential_force = config.tangential_k * tangential_velocity;

    // B10: Force sum
    // return repulsiveForce + dampingForce + tangentialForce (line 213)
    repulsive_force + damping_force + tangential_force
}

/// Add gravity to a force accumulator.
///
/// Reference: `force.y -= gravityCoefficient` (line 326)
///
/// # Arguments
/// * `force` - Current accumulated force
/// * `config` - Physics configuration
///
/// # Returns
/// Force with gravity subtracted from Y component
pub fn apply_gravity(force: Vec3, config: &PhysicsConfig) -> Vec3 {
    // TODO: Phase 1 - implement gravity
    // Checklist items: C2-C3
    Vec3::new(force.x, force.y - config.gravity, force.z)
}

// =============================================================================
// PHASE 2: Integration (Sections E, F)
// =============================================================================

/// Integrate linear velocity with friction and force.
///
/// CRITICAL: Friction is applied BEFORE adding force (reference line 365).
///
/// Reference: `ComputeMomenta` (lines 362-369)
///
/// # Arguments
/// * `velocity` - Current velocity
/// * `force` - Total force on the body
/// * `mass` - Total mass of the rigid body
/// * `friction` - Friction coefficient (0.9 in reference)
/// * `dt` - Delta time
/// * `threshold` - Velocity threshold for zeroing (1e-6)
///
/// # Returns
/// New velocity after integration
pub fn integrate_velocity(
    velocity: Vec3,
    force: Vec3,
    mass: f32,
    friction: f32,
    dt: f32,
    threshold: f32,
) -> Vec3 {
    // Reference: ComputeMomenta (lines 362-369)
    //
    // CRITICAL: Friction is applied BEFORE adding force!
    // This was one of our original bugs.

    // E1: Friction FIRST - divide velocity by (1 + dt * friction)
    // rigidBodyVelocities[id.x] /= 1.0 + deltaTime*frictionCoefficient (line 365)
    let vel_after_friction = velocity / (1.0 + dt * friction);

    // E3: Add force contribution
    // rigidBodyVelocities[id.x] += linearForceScalar * deltaTime * linearForce/cubeMass (line 366)
    // Note: linearForceScalar = 1.0, so we omit it
    let vel_after_force = vel_after_friction + dt * force / mass;

    // E2, E5: Velocity zeroing - if magnitude below threshold, zero it
    // if (length(rigidBodyVelocities[id.x]) < threshold) { rigidBodyVelocities[id.x] = float3(0,0,0); }
    if vel_after_force.length() < threshold {
        Vec3::ZERO
    } else {
        vel_after_force
    }
}

/// Integrate angular velocity with friction and torque.
///
/// Reference: `ComputeMomenta` (lines 377-381)
///
/// # Arguments
/// * `angular_velocity` - Current angular velocity
/// * `torque` - Total torque on the body
/// * `angular_friction` - Angular friction coefficient (0.3 in reference)
/// * `angular_force_scalar` - Force multiplier (1.0 in reference)
/// * `dt` - Delta time
/// * `threshold` - Velocity threshold for zeroing (1e-6)
///
/// # Returns
/// New angular velocity after integration
pub fn integrate_angular_velocity(
    angular_velocity: Vec3,
    torque: Vec3,
    angular_friction: f32,
    angular_force_scalar: f32,
    dt: f32,
    threshold: f32,
) -> Vec3 {
    // Reference: ComputeMomenta (lines 377-381)
    //
    // Same pattern as linear velocity: friction FIRST, then add torque

    // E6: Angular friction FIRST
    // rigidBodyAngularVelocities[id.x] /= 1.0 + deltaTime*angularFrictionCoefficient (line 377)
    let ang_vel_after_friction = angular_velocity / (1.0 + dt * angular_friction);

    // E7, E8: Add torque contribution
    // rigidBodyAngularVelocities[id.x] += angularForceScalar * deltaTime * angularForce (line 378)
    let ang_vel_after_torque = ang_vel_after_friction + angular_force_scalar * dt * torque;

    // E9: Angular velocity zeroing
    // if (length(rigidBodyAngularVelocities[id.x]) < threshold) { ... = float3(0,0,0); }
    if ang_vel_after_torque.length() < threshold {
        Vec3::ZERO
    } else {
        ang_vel_after_torque
    }
}

/// Integrate position using Euler integration.
///
/// Reference: `rigidBodyPositions[id.x] += rigidBodyVelocities[id.x] * deltaTime` (line 401)
///
/// # Arguments
/// * `position` - Current position
/// * `velocity` - Current velocity
/// * `dt` - Delta time
///
/// # Returns
/// New position
pub fn integrate_position(position: Vec3, velocity: Vec3, dt: f32) -> Vec3 {
    // TODO: Phase 2 - implement position integration
    // Checklist item: F1
    position + velocity * dt
}

/// Integrate rotation using quaternion derivative.
///
/// Reference: `ComputePositionAndRotation` (lines 428-431)
/// Formula: `q_new = normalize(q + dt * 0.5 * quat_concat(omega, q))`
/// where `omega = float4(angular_velocity.xyz, 0)`
///
/// # Arguments
/// * `rotation` - Current rotation quaternion
/// * `angular_velocity` - Current angular velocity
/// * `dt` - Delta time
///
/// # Returns
/// New rotation quaternion (normalized)
pub fn integrate_rotation(rotation: Quat, angular_velocity: Vec3, dt: f32) -> Quat {
    // Reference: ComputePositionAndRotation (lines 428-431)
    //
    // Formula: q_new = normalize(q + dt * 0.5 * quat_concat(omega, q))
    // where omega = float4(angular_velocity.xyz, 0)
    //
    // quat_concat from Quaternion.cginc line 44:
    // float4(q1.w * q2.xyz + q2.w * q1.xyz + cross(q1.xyz, q2.xyz),
    //        q1.w * q2.w - dot(q1.xyz, q2.xyz))

    // F2: Quaternion omega format - xyz in xyz, 0 in w
    // float4 omega = float4(rigidBodyAngularVelocity, 0) (line 428)
    // Note: We don't need to store omega as a Quat, we use angular_velocity directly below

    // F5: quat_concat(omega, q) - this is quaternion multiplication
    // In the reference: quat_concat(q1, q2) =
    //   float4(q1.w * q2.xyz + q2.w * q1.xyz + cross(q1.xyz, q2.xyz),
    //          q1.w * q2.w - dot(q1.xyz, q2.xyz))
    //
    // For omega (w=0) and q:
    //   xyz = 0 * q.xyz + q.w * omega.xyz + cross(omega.xyz, q.xyz)
    //       = q.w * angular_velocity + cross(angular_velocity, q.xyz)
    //   w   = 0 * q.w - dot(omega.xyz, q.xyz)
    //       = -dot(angular_velocity, q.xyz)
    let q_xyz = Vec3::new(rotation.x, rotation.y, rotation.z);
    let q_w = rotation.w;

    let concat_xyz = q_w * angular_velocity + angular_velocity.cross(q_xyz);
    let concat_w = -angular_velocity.dot(q_xyz);

    let omega_q = Quat::from_xyzw(concat_xyz.x, concat_xyz.y, concat_xyz.z, concat_w);

    // F3: Quaternion derivative: q + dt * 0.5 * quat_concat(omega, q)
    // rigidBodyQuaternions[id.x] = normalize(q + deltaTime * (0.5*quat_concat(omega, q))) (line 431)
    let dq = Quat::from_xyzw(
        omega_q.x * 0.5 * dt,
        omega_q.y * 0.5 * dt,
        omega_q.z * 0.5 * dt,
        omega_q.w * 0.5 * dt,
    );

    let new_q = Quat::from_xyzw(
        rotation.x + dq.x,
        rotation.y + dq.y,
        rotation.z + dq.z,
        rotation.w + dq.w,
    );

    // F4: Quaternion normalization
    new_q.normalize()
}

// =============================================================================
// PHASE 3: Force Aggregation (Section D)
// =============================================================================

/// Aggregate particle forces into rigid body linear and angular forces.
///
/// Reference: `ComputeMomenta` (lines 352-361)
///
/// # Arguments
/// * `particle_forces` - Forces on each particle
/// * `particle_relative_positions` - Position of each particle relative to body center
///
/// # Returns
/// Tuple of (linear_force, angular_force/torque)
pub fn aggregate_particle_forces(
    particle_forces: &[Vec3],
    particle_relative_positions: &[Vec3],
) -> (Vec3, Vec3) {
    // Reference: ComputeMomenta (lines 352-361)
    //
    // for (int i = 0; i < particlesPerRigidBody; i++) {
    //     int p_id = id.x * particlesPerRigidBody + i;
    //     relativePosition = particleRelativePositions[p_id];
    //     linearForce += particleForces[p_id];
    //     angularForce += cross(relativePosition, particleForces[p_id]);
    // }

    // D1: Linear force init
    // float3 linearForce = float3(0,0,0) (line 352)
    let mut linear_force = Vec3::ZERO;

    // D2: Angular force init
    // float3 angularForce = float3(0,0,0) (line 353)
    let mut angular_force = Vec3::ZERO;

    // D3, D4, D5: Sum forces and compute torques
    for (force, relative_pos) in particle_forces
        .iter()
        .zip(particle_relative_positions.iter())
    {
        // D3: linearForce += particleForces[p_id] (line 359)
        linear_force += *force;

        // D4: angularForce += cross(relativePosition, particleForces[p_id]) (line 360)
        // D5: relativePosition is the particle's position relative to body center
        angular_force += relative_pos.cross(*force);
    }

    // D6: Mass calculation is done by caller (cubeMass = particleMass * particlesPerRigidBody)
    (linear_force, angular_force)
}

// =============================================================================
// PHASE 4-5: Simulation Helpers
// =============================================================================

/// Simulate a single rigid body with ground collision for N steps.
///
/// This is a test helper that runs the full physics pipeline on a single body.
///
/// # Arguments
/// * `initial_pos` - Starting position
/// * `initial_vel` - Starting velocity
/// * `mass` - Body mass
/// * `steps` - Number of simulation steps
/// * `dt` - Delta time per step
/// * `config` - Physics configuration
///
/// # Returns
/// Vector of positions at each step
pub fn simulate_single_body(
    initial_pos: Vec3,
    initial_vel: Vec3,
    mass: f32,
    steps: usize,
    dt: f32,
    config: &PhysicsConfig,
) -> Vec<Vec3> {
    // Simulates a single point-particle rigid body with ground collision.
    // This is a simplified version - treats the body as a single particle at its center.
    //
    // For a full rigid body with multiple particles, we would:
    // 1. Generate particle positions from body position + rotation
    // 2. Compute forces on each particle
    // 3. Aggregate forces to get linear/angular force
    // 4. Integrate velocity and position
    //
    // For this test, we simplify to a single particle (the body center).

    let mut positions = Vec::with_capacity(steps);
    let mut pos = initial_pos;
    let mut vel = initial_vel;

    for _ in 0..steps {
        positions.push(pos);

        // Step 1: Compute forces on the single particle (body center)
        // In a full simulation, we'd compute forces on all particles and aggregate.
        // Here we just compute ground collision + gravity for the center particle.

        // Ground collision force
        let collision_force = compute_ground_collision_force(pos, vel, config);

        // Add gravity (applied per-particle in reference, so we apply it here)
        let total_force = apply_gravity(collision_force, config);

        // Step 2: Integrate velocity
        // For a single particle, linear_force = total_force, no torque
        vel = integrate_velocity(
            vel,
            total_force,
            mass,
            config.friction,
            dt,
            config.velocity_threshold,
        );

        // Step 3: Integrate position
        pos = integrate_position(pos, vel, dt);
    }

    positions
}

/// Simulate two rigid bodies with ground and inter-body collision for N steps.
///
/// Returns tuple of (body1_positions, body2_positions).
pub fn simulate_two_bodies(
    initial_pos_1: Vec3,
    initial_vel_1: Vec3,
    initial_pos_2: Vec3,
    initial_vel_2: Vec3,
    mass: f32,
    steps: usize,
    dt: f32,
    config: &PhysicsConfig,
) -> (Vec<Vec3>, Vec<Vec3>) {
    let mut positions_1 = Vec::with_capacity(steps);
    let mut positions_2 = Vec::with_capacity(steps);

    let mut pos_1 = initial_pos_1;
    let mut vel_1 = initial_vel_1;
    let mut pos_2 = initial_pos_2;
    let mut vel_2 = initial_vel_2;

    for _ in 0..steps {
        positions_1.push(pos_1);
        positions_2.push(pos_2);

        // Compute forces on body 1
        let ground_force_1 = compute_ground_collision_force(pos_1, vel_1, config);
        let collision_force_1 =
            compute_particle_collision_force(pos_1, vel_1, pos_2, vel_2, config);
        let total_force_1 = apply_gravity(ground_force_1 + collision_force_1, config);

        // Compute forces on body 2
        let ground_force_2 = compute_ground_collision_force(pos_2, vel_2, config);
        let collision_force_2 =
            compute_particle_collision_force(pos_2, vel_2, pos_1, vel_1, config);
        let total_force_2 = apply_gravity(ground_force_2 + collision_force_2, config);

        // Integrate velocities
        vel_1 = integrate_velocity(
            vel_1,
            total_force_1,
            mass,
            config.friction,
            dt,
            config.velocity_threshold,
        );
        vel_2 = integrate_velocity(
            vel_2,
            total_force_2,
            mass,
            config.friction,
            dt,
            config.velocity_threshold,
        );

        // Integrate positions
        pos_1 = integrate_position(pos_1, vel_1, dt);
        pos_2 = integrate_position(pos_2, vel_2, dt);
    }

    (positions_1, positions_2)
}

// =============================================================================
// PHASE 6: Terrain Collision (Section J)
// =============================================================================

use crate::voxel_collision::WorldOccupancy;

/// Compute collision force between a particle and voxel terrain.
///
/// For each occupied voxel the particle overlaps, we compute a collision force
/// as if the voxel center were a stationary particle (same as ground collision).
///
/// Reference: Adapted from `_collisionReactionWithGround`, treating each voxel
/// as a virtual ground particle at the voxel's center.
///
/// # Arguments
/// * `particle_pos` - World position of the particle
/// * `particle_vel` - World velocity of the particle
/// * `occupancy` - World occupancy data for terrain
/// * `config` - Physics configuration
///
/// # Returns
/// Sum of collision forces from all overlapping voxels
pub fn compute_terrain_collision_force(
    particle_pos: Vec3,
    particle_vel: Vec3,
    occupancy: &WorldOccupancy,
    config: &PhysicsConfig,
) -> Vec3 {
    // J1: We need to check all voxels the particle might overlap
    // Particle is a sphere of diameter particle_diameter centered at particle_pos
    let radius = config.particle_diameter * 0.5;

    // J2: Compute AABB of particle in voxel coordinates
    // Voxels are unit cubes, voxel at (x,y,z) occupies [x, x+1) x [y, y+1) x [z, z+1)
    let min_voxel = IVec3::new(
        (particle_pos.x - radius).floor() as i32,
        (particle_pos.y - radius).floor() as i32,
        (particle_pos.z - radius).floor() as i32,
    );
    let max_voxel = IVec3::new(
        (particle_pos.x + radius).floor() as i32,
        (particle_pos.y + radius).floor() as i32,
        (particle_pos.z + radius).floor() as i32,
    );

    let mut total_force = Vec3::ZERO;

    // Check each potentially overlapping voxel
    for vx in min_voxel.x..=max_voxel.x {
        for vy in min_voxel.y..=max_voxel.y {
            for vz in min_voxel.z..=max_voxel.z {
                let voxel_pos = IVec3::new(vx, vy, vz);

                // J1, J8: Check if voxel is occupied
                if !occupancy.get_voxel(voxel_pos) {
                    continue; // Empty voxel = no force
                }

                // J3: Voxel center position
                // Voxel at (x,y,z) has center at (x+0.5, y+0.5, z+0.5)
                let voxel_center = Vec3::new(
                    voxel_pos.x as f32 + 0.5,
                    voxel_pos.y as f32 + 0.5,
                    voxel_pos.z as f32 + 0.5,
                );

                // J4, J5, J6: Compute collision force using same formula as ground collision
                // Treat voxel center as a stationary particle
                let force = compute_collision_with_static_point(
                    particle_pos,
                    particle_vel,
                    voxel_center,
                    config,
                );

                // J7: Sum forces from all overlapping voxels
                total_force += force;
            }
        }
    }

    total_force
}

/// Compute collision force with a static point (helper for terrain collision).
///
/// This is the same formula as `compute_ground_collision_force` but with an
/// arbitrary static point instead of the ground plane.
fn compute_collision_with_static_point(
    particle_pos: Vec3,
    particle_vel: Vec3,
    static_point: Vec3,
    config: &PhysicsConfig,
) -> Vec3 {
    // Relative position: points FROM particle TO static point
    let relative_position = static_point - particle_pos;
    let relative_position_magnitude = relative_position.length();

    // Collision condition
    if relative_position_magnitude >= config.particle_diameter {
        return Vec3::ZERO;
    }

    // Avoid division by zero
    if relative_position_magnitude < 1e-8 {
        return Vec3::ZERO;
    }

    // Normal direction - points FROM particle TO static point
    let n = relative_position / relative_position_magnitude;

    // Penetration
    let penetration = config.particle_diameter - relative_position_magnitude;

    // Spring force (repulsive, pushes particle away from static point)
    let repulsive_force = -config.spring_k * penetration * n;

    // Relative velocity (static point has zero velocity)
    let relative_velocity = Vec3::ZERO - particle_vel;

    // Damping force
    let damping_force = config.damping_k * relative_velocity;

    // Tangential force
    let normal_velocity = relative_velocity.dot(n) * n;
    let tangential_velocity = relative_velocity - normal_velocity;
    let tangential_force = config.tangential_k * tangential_velocity;

    repulsive_force + damping_force + tangential_force
}

/// Simulate a single rigid body on voxel terrain for N steps.
///
/// # Arguments
/// * `initial_pos` - Starting position
/// * `initial_vel` - Starting velocity
/// * `mass` - Body mass
/// * `occupancy` - Terrain occupancy data
/// * `steps` - Number of simulation steps
/// * `dt` - Delta time per step
/// * `config` - Physics configuration
///
/// # Returns
/// Vector of positions at each step
pub fn simulate_single_body_on_terrain(
    initial_pos: Vec3,
    initial_vel: Vec3,
    mass: f32,
    occupancy: &WorldOccupancy,
    steps: usize,
    dt: f32,
    config: &PhysicsConfig,
) -> Vec<Vec3> {
    let mut positions = Vec::with_capacity(steps);
    let mut pos = initial_pos;
    let mut vel = initial_vel;

    for _ in 0..steps {
        positions.push(pos);

        // Compute terrain collision force (replaces ground collision)
        let collision_force = compute_terrain_collision_force(pos, vel, occupancy, config);

        // Add gravity
        let total_force = apply_gravity(collision_force, config);

        // Integrate velocity
        vel = integrate_velocity(
            vel,
            total_force,
            mass,
            config.friction,
            dt,
            config.velocity_threshold,
        );

        // Integrate position
        pos = integrate_position(pos, vel, dt);
    }

    positions
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Phase 0: Config Tests (should pass immediately)
    // =========================================================================

    #[test]
    fn test_physics_config_default_values() {
        let config = PhysicsConfig::default();

        // Verify all reference constants match
        assert_eq!(config.gravity, 9.8, "gravity should be 9.8");
        assert_eq!(
            config.particle_diameter, 1.0,
            "particle_diameter should be 1.0"
        );
        assert_eq!(config.spring_k, 500.0, "spring_k should be 500.0");
        assert_eq!(config.damping_k, 10.0, "damping_k should be 10.0");
        assert_eq!(config.tangential_k, 2.0, "tangential_k should be 2.0");
        assert_eq!(config.friction, 0.9, "friction should be 0.9");
        assert_eq!(
            config.angular_friction, 0.3,
            "angular_friction should be 0.3"
        );
        assert_eq!(
            config.linear_force_scalar, 1.0,
            "linear_force_scalar should be 1.0"
        );
        assert_eq!(
            config.angular_force_scalar, 1.0,
            "angular_force_scalar should be 1.0"
        );
        assert_eq!(
            config.velocity_threshold, 1e-6,
            "velocity_threshold should be 1e-6"
        );
    }

    #[test]
    fn test_apply_gravity() {
        let config = PhysicsConfig::default();
        let force = Vec3::new(10.0, 50.0, -5.0);
        let result = apply_gravity(force, &config);

        assert_eq!(result.x, 10.0);
        assert_eq!(result.y, 50.0 - 9.8);
        assert_eq!(result.z, -5.0);
    }

    #[test]
    fn test_integrate_position_basic() {
        let pos = Vec3::new(0.0, 10.0, 0.0);
        let vel = Vec3::new(0.0, -5.0, 0.0);
        let dt = 1.0 / 60.0;

        let new_pos = integrate_position(pos, vel, dt);

        let expected_y = 10.0 + (-5.0) * dt;
        assert!((new_pos.y - expected_y).abs() < 0.0001);
    }

    // =========================================================================
    // Phase 1: Force Computation Tests (A1-A13, B1-B10, C1-C5)
    // =========================================================================

    #[test]
    fn test_ground_collision_force_direction() {
        // A1-A6: Verify spring force points UP when particle is below ground surface
        let config = PhysicsConfig::default();

        // Particle at Y=0.3 (penetrating ground, since ground surface is at Y=0)
        let particle_pos = Vec3::new(0.0, 0.3, 0.0);
        let particle_vel = Vec3::new(0.0, -5.0, 0.0); // Falling

        let force = compute_ground_collision_force(particle_pos, particle_vel, &config);

        // Force should point UP (positive Y)
        assert!(
            force.y > 0.0,
            "Ground collision force should point up, got {}",
            force.y
        );
    }

    #[test]
    fn test_ground_collision_no_force_when_above() {
        // Particle well above ground should have no collision force
        let config = PhysicsConfig::default();

        let particle_pos = Vec3::new(0.0, 2.0, 0.0);
        let particle_vel = Vec3::new(0.0, -5.0, 0.0);

        let force = compute_ground_collision_force(particle_pos, particle_vel, &config);

        assert_eq!(force, Vec3::ZERO, "No force when above ground");
    }

    #[test]
    fn test_spring_force_magnitude() {
        // A5-A6: Verify force magnitude matches reference formula
        // Ground particle Y = -0.5 (particle_diameter * 0.5)
        // Our particle Y = 0.3
        // Distance = 0.3 - (-0.5) = 0.8
        // Penetration = diameter(1.0) - distance(0.8) = 0.2
        // Spring force magnitude = 500 * 0.2 = 100
        let config = PhysicsConfig::default();

        let particle_pos = Vec3::new(0.0, 0.3, 0.0);
        let particle_vel = Vec3::ZERO; // No velocity = no damping contribution

        let force = compute_ground_collision_force(particle_pos, particle_vel, &config);

        // Spring force should be approximately 100 (500 * 0.2)
        // Note: may have small differences due to damping even with zero velocity
        assert!(
            (force.y - 100.0).abs() < 5.0,
            "Expected spring force ~100, got {}",
            force.y
        );
    }

    #[test]
    fn test_damping_force_opposes_velocity() {
        // A7-A9: Verify damping force opposes particle velocity
        let config = PhysicsConfig::default();

        // Particle at ground level, moving down fast
        let particle_pos = Vec3::new(0.0, 0.4, 0.0);
        let particle_vel_down = Vec3::new(0.0, -10.0, 0.0);
        let particle_vel_up = Vec3::new(0.0, 10.0, 0.0);

        let force_when_falling =
            compute_ground_collision_force(particle_pos, particle_vel_down, &config);
        let force_when_rising =
            compute_ground_collision_force(particle_pos, particle_vel_up, &config);

        // Damping should add to upward force when falling (opposing downward motion)
        // Damping should reduce upward force when rising (opposing upward motion)
        assert!(
            force_when_falling.y > force_when_rising.y,
            "Force when falling ({}) should be greater than when rising ({})",
            force_when_falling.y,
            force_when_rising.y
        );
    }

    #[test]
    fn test_tangential_force_opposes_sliding() {
        // A10-A12: Verify tangential force opposes sliding motion
        let config = PhysicsConfig::default();

        // Particle at ground, sliding in +X direction
        let particle_pos = Vec3::new(0.0, 0.3, 0.0);
        let particle_vel = Vec3::new(10.0, 0.0, 0.0); // Sliding +X

        let force = compute_ground_collision_force(particle_pos, particle_vel, &config);

        // Tangential force should oppose sliding, so X component should be negative
        assert!(
            force.x < 0.0,
            "Tangential force should oppose +X sliding, got {}",
            force.x
        );
    }

    #[test]
    fn test_particle_collision_force_repels() {
        // B1-B5: Two overlapping particles should repel
        let config = PhysicsConfig::default();

        // Two particles, centers 0.5 apart (overlapping since diameter=1.0)
        let pos_i = Vec3::new(0.0, 0.0, 0.0);
        let pos_j = Vec3::new(0.5, 0.0, 0.0);
        let vel_i = Vec3::ZERO;
        let vel_j = Vec3::ZERO;

        let force_on_i = compute_particle_collision_force(pos_i, vel_i, pos_j, vel_j, &config);

        // Force on i should push it away from j (negative X)
        assert!(
            force_on_i.x < 0.0,
            "Force should push i away from j (-X direction), got {}",
            force_on_i.x
        );
    }

    #[test]
    fn test_particle_collision_no_force_when_separated() {
        // B2: No force when particles don't overlap
        let config = PhysicsConfig::default();

        // Two particles, centers 2.0 apart (no overlap since diameter=1.0)
        let pos_i = Vec3::new(0.0, 0.0, 0.0);
        let pos_j = Vec3::new(2.0, 0.0, 0.0);
        let vel_i = Vec3::ZERO;
        let vel_j = Vec3::ZERO;

        let force = compute_particle_collision_force(pos_i, vel_i, pos_j, vel_j, &config);

        assert_eq!(force, Vec3::ZERO, "No force when particles are separated");
    }

    // =========================================================================
    // Phase 2: Integration Tests (E1-E9, F1-F5)
    // =========================================================================

    #[test]
    fn test_friction_applied_before_force() {
        // E1: Friction divides velocity BEFORE adding force
        let vel = Vec3::new(0.0, 10.0, 0.0);
        let force = Vec3::new(0.0, 100.0, 0.0);
        let mass = 1.0;
        let friction = 0.9;
        let dt = 1.0 / 60.0;
        let threshold = 1e-6;

        let new_vel = integrate_velocity(vel, force, mass, friction, dt, threshold);

        // vel_after_friction = 10.0 / (1.0 + dt * 0.9)
        // vel_after_force = vel_after_friction + force/mass * dt
        let vel_after_friction = 10.0 / (1.0 + dt * friction);
        let expected = vel_after_friction + force.y / mass * dt;

        assert!(
            (new_vel.y - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            new_vel.y
        );
    }

    #[test]
    fn test_velocity_zeroing_threshold() {
        // E2, E5: Tiny velocities should be zeroed
        let vel = Vec3::new(0.0, 0.0000001, 0.0); // Below 1e-6 threshold
        let force = Vec3::ZERO;
        let mass = 1.0;
        let friction = 0.9;
        let dt = 1.0 / 60.0;
        let threshold = 1e-6;

        let new_vel = integrate_velocity(vel, force, mass, friction, dt, threshold);

        assert_eq!(new_vel, Vec3::ZERO, "Tiny velocity should be zeroed");
    }

    #[test]
    fn test_quaternion_integration_small_rotation() {
        // F2-F5: Verify quaternion derivative formula produces reasonable results
        let rotation = Quat::IDENTITY;
        let angular_velocity = Vec3::new(0.0, 1.0, 0.0); // Rotate around Y
        let dt = 1.0 / 60.0;

        let new_rotation = integrate_rotation(rotation, angular_velocity, dt);

        // Should still be close to identity for small dt
        assert!(new_rotation.is_normalized(), "Result should be normalized");

        // Y-axis rotation should produce a small angle
        let (axis, angle) = new_rotation.to_axis_angle();
        assert!(angle.abs() < 0.1, "Small dt should produce small angle");
        assert!(
            (axis.y.abs() - 1.0).abs() < 0.01 || angle.abs() < 0.001,
            "Rotation should be around Y axis"
        );
    }

    // =========================================================================
    // Phase 3: Aggregation Tests (D1-D6)
    // =========================================================================

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

        let (linear, _angular) =
            aggregate_particle_forces(&particle_forces, &particle_relative_positions);

        assert_eq!(linear, Vec3::new(0.0, 60.0, 0.0));
    }

    #[test]
    fn test_torque_from_off_center_force() {
        // D4: Torque = cross(relative_pos, force)
        // Force at +X offset should create torque around Z axis
        let particle_forces = vec![Vec3::new(0.0, 100.0, 0.0)]; // Upward force
        let particle_relative_positions = vec![Vec3::new(1.0, 0.0, 0.0)]; // At +X offset

        let (_linear, angular) =
            aggregate_particle_forces(&particle_forces, &particle_relative_positions);

        // cross((1,0,0), (0,100,0)) = (0*0-0*100, 0*0-1*0, 1*100-0*0) = (0, 0, 100)
        assert!(
            (angular.z - 100.0).abs() < 0.001,
            "Expected Z torque ~100, got {}",
            angular.z
        );
    }

    #[test]
    fn test_symmetric_forces_no_torque() {
        // Symmetric forces should produce no net torque
        let particle_forces = vec![
            Vec3::new(0.0, 50.0, 0.0), // Force at -X
            Vec3::new(0.0, 50.0, 0.0), // Force at +X (same magnitude)
        ];
        let particle_relative_positions = vec![Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];

        let (linear, angular) =
            aggregate_particle_forces(&particle_forces, &particle_relative_positions);

        assert_eq!(
            linear,
            Vec3::new(0.0, 100.0, 0.0),
            "Linear force should sum"
        );
        assert!(
            angular.length() < 0.001,
            "Symmetric forces should produce no torque, got {:?}",
            angular
        );
    }

    // =========================================================================
    // Phase 4: Single Body Integration Test
    // =========================================================================

    #[test]
    fn test_cube_falls_and_settles() {
        let config = PhysicsConfig::default();
        let initial_pos = Vec3::new(0.0, 10.0, 0.0);
        let initial_vel = Vec3::ZERO;
        let mass = 1.0;
        let dt = 1.0 / 60.0;
        let steps = 600; // 10 seconds

        let positions = simulate_single_body(initial_pos, initial_vel, mass, steps, dt, &config);

        // Should fall initially
        assert!(positions[10].y < positions[0].y, "Should fall");

        // Should hit ground (Y should go near 0.5 at some point)
        let min_y = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(min_y < 1.0, "Should hit ground, min_y was {}", min_y);

        // Should settle near Y=0.5 (half particle diameter above ground)
        let final_y = positions.last().unwrap().y;
        assert!(
            (final_y - 0.5).abs() < 0.1,
            "Should settle near Y=0.5, got {}",
            final_y
        );

        // Should not explode (Y should never go extremely high or negative)
        let max_y = positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(
            max_y < 15.0,
            "Should not explode upward, max_y was {}",
            max_y
        );
        assert!(
            min_y > -1.0,
            "Should not go through ground, min_y was {}",
            min_y
        );
    }

    // =========================================================================
    // Phase 5: Two Body Collision Test
    // =========================================================================

    #[test]
    fn test_two_cubes_collide() {
        // Two cubes start overlapping, should push apart and settle
        let config = PhysicsConfig::default();
        let mass = 1.0;
        let dt = 1.0 / 60.0;
        let steps = 600; // 10 seconds

        // Two bodies start overlapping at Y=5 (above ground)
        // Body 1 at origin, Body 2 at X=0.5 (overlapping since diameter=1.0)
        let initial_pos_1 = Vec3::new(0.0, 5.0, 0.0);
        let initial_vel_1 = Vec3::ZERO;
        let initial_pos_2 = Vec3::new(0.5, 5.0, 0.0);
        let initial_vel_2 = Vec3::ZERO;

        let (positions_1, positions_2) = simulate_two_bodies(
            initial_pos_1,
            initial_vel_1,
            initial_pos_2,
            initial_vel_2,
            mass,
            steps,
            dt,
            &config,
        );

        // They should push apart initially (X distance should increase)
        let initial_x_dist = (initial_pos_2.x - initial_pos_1.x).abs();
        let mid_x_dist = (positions_2[60].x - positions_1[60].x).abs(); // After 1 second
        assert!(
            mid_x_dist > initial_x_dist,
            "Bodies should push apart: initial dist={}, after 1s dist={}",
            initial_x_dist,
            mid_x_dist
        );

        // Both should fall and hit ground
        let min_y_1 = positions_1.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let min_y_2 = positions_2.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(min_y_1 < 2.0, "Body 1 should fall, min_y was {}", min_y_1);
        assert!(min_y_2 < 2.0, "Body 2 should fall, min_y was {}", min_y_2);

        // Both should settle near ground (Y ~= 0.5)
        let final_y_1 = positions_1.last().unwrap().y;
        let final_y_2 = positions_2.last().unwrap().y;
        assert!(
            (final_y_1 - 0.5).abs() < 0.2,
            "Body 1 should settle near Y=0.5, got {}",
            final_y_1
        );
        assert!(
            (final_y_2 - 0.5).abs() < 0.2,
            "Body 2 should settle near Y=0.5, got {}",
            final_y_2
        );

        // Final X distance should be >= particle diameter (not overlapping)
        let final_x_dist = (positions_2.last().unwrap().x - positions_1.last().unwrap().x).abs();
        assert!(
            final_x_dist >= config.particle_diameter * 0.95,
            "Bodies should not overlap at rest: final dist={}, diameter={}",
            final_x_dist,
            config.particle_diameter
        );

        // Neither should explode
        let max_y_1 = positions_1.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let max_y_2 = positions_2.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(
            max_y_1 < 10.0,
            "Body 1 should not explode, max_y={}",
            max_y_1
        );
        assert!(
            max_y_2 < 10.0,
            "Body 2 should not explode, max_y={}",
            max_y_2
        );
    }

    // =========================================================================
    // Phase 6: Terrain Collision Tests (J1-J8)
    // =========================================================================

    use crate::voxel_collision::{ChunkOccupancy, WorldOccupancy};

    /// Helper to create a flat floor of voxels at Y=0
    fn create_flat_floor(size: i32) -> WorldOccupancy {
        let mut occupancy = WorldOccupancy::new();
        let mut chunk = ChunkOccupancy::new();

        // Fill bottom layer (Y=0) of chunk
        for x in 0..size.min(32) {
            for z in 0..size.min(32) {
                chunk.set(UVec3::new(x as u32, 0, z as u32), true);
            }
        }

        occupancy.load_chunk(IVec3::ZERO, chunk);
        occupancy
    }

    /// Helper to create a floor with a gap
    fn create_floor_with_gap(size: i32, gap_min: i32, gap_max: i32) -> WorldOccupancy {
        let mut occupancy = WorldOccupancy::new();
        let mut chunk = ChunkOccupancy::new();

        for x in 0..size.min(32) {
            for z in 0..size.min(32) {
                // Skip the gap region
                if x >= gap_min && x <= gap_max && z >= gap_min && z <= gap_max {
                    continue;
                }
                chunk.set(UVec3::new(x as u32, 0, z as u32), true);
            }
        }

        occupancy.load_chunk(IVec3::ZERO, chunk);
        occupancy
    }

    #[test]
    fn test_terrain_collision_empty_world() {
        // J8: Empty voxel = no force
        let config = PhysicsConfig::default();
        let occupancy = WorldOccupancy::new(); // Empty world

        let particle_pos = Vec3::new(5.0, 0.5, 5.0);
        let particle_vel = Vec3::new(0.0, -5.0, 0.0);

        let force =
            compute_terrain_collision_force(particle_pos, particle_vel, &occupancy, &config);

        assert_eq!(force, Vec3::ZERO, "Empty world should produce no force");
    }

    #[test]
    fn test_terrain_collision_single_voxel() {
        // J1, J3, J6: Single voxel collision
        let config = PhysicsConfig::default();

        let mut occupancy = WorldOccupancy::new();
        let mut chunk = ChunkOccupancy::new();
        chunk.set(UVec3::new(5, 0, 5), true); // Voxel at (5, 0, 5)
        occupancy.load_chunk(IVec3::ZERO, chunk);

        // Particle directly above voxel center (5.5, 0.5, 5.5), penetrating
        // Voxel center is at (5.5, 0.5, 5.5), particle at (5.5, 0.7, 5.5)
        // Distance = 0.2, penetration = 1.0 - 0.2 = 0.8
        let particle_pos = Vec3::new(5.5, 0.7, 5.5);
        let particle_vel = Vec3::ZERO;

        let force =
            compute_terrain_collision_force(particle_pos, particle_vel, &occupancy, &config);

        // Force should push particle UP (away from voxel center)
        assert!(force.y > 0.0, "Force should push up, got {}", force.y);
    }

    #[test]
    fn test_terrain_collision_no_force_when_above() {
        // J2: No collision when particle doesn't overlap voxel
        let config = PhysicsConfig::default();

        let mut occupancy = WorldOccupancy::new();
        let mut chunk = ChunkOccupancy::new();
        chunk.set(UVec3::new(5, 0, 5), true);
        occupancy.load_chunk(IVec3::ZERO, chunk);

        // Particle well above voxel (Y=3.0, voxel top is at Y=1)
        let particle_pos = Vec3::new(5.5, 3.0, 5.5);
        let particle_vel = Vec3::new(0.0, -5.0, 0.0);

        let force =
            compute_terrain_collision_force(particle_pos, particle_vel, &occupancy, &config);

        assert_eq!(force, Vec3::ZERO, "Should be no force when above voxel");
    }

    #[test]
    fn test_cube_on_voxel_floor() {
        // Full integration: cube lands on voxel floor
        let config = PhysicsConfig::default();
        let mass = 1.0;
        let dt = 1.0 / 60.0;
        let steps = 600; // 10 seconds

        // Create 10x10 voxel floor at Y=0
        let occupancy = create_flat_floor(10);

        // Drop cube from Y=10, centered over floor
        let initial_pos = Vec3::new(5.5, 10.0, 5.5);
        let initial_vel = Vec3::ZERO;

        let positions = simulate_single_body_on_terrain(
            initial_pos,
            initial_vel,
            mass,
            &occupancy,
            steps,
            dt,
            &config,
        );

        // Should fall
        assert!(positions[10].y < positions[0].y, "Should fall");

        // Should settle on floor
        // Voxel at Y=0 has top surface at Y=1
        // Particle should settle with center at Y = 1 + radius = 1.5
        // But voxel center is at 0.5, so collision happens when distance < 1.0
        // Particle settles when distance to voxel center = 1.0
        // If voxel center Y = 0.5, particle Y = 0.5 + 1.0 = 1.5
        let final_y = positions.last().unwrap().y;
        assert!(
            (final_y - 1.5).abs() < 0.3,
            "Should settle near Y=1.5 (above voxel), got {}",
            final_y
        );

        // Should not explode
        let max_y = positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(max_y < 15.0, "Should not explode, max_y={}", max_y);

        // Should not fall through
        let min_y = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(
            min_y > 0.5,
            "Should not fall through floor, min_y={}",
            min_y
        );
    }

    #[test]
    fn test_cube_falls_through_gap() {
        // Cube should fall through a gap in the floor
        let config = PhysicsConfig::default();
        let mass = 1.0;
        let dt = 1.0 / 60.0;
        let steps = 300; // 5 seconds

        // Create floor with 2x2 gap at center (voxels 4-5 in X and Z)
        let occupancy = create_floor_with_gap(10, 4, 5);

        // Drop cube directly over the gap
        let initial_pos = Vec3::new(5.0, 10.0, 5.0);
        let initial_vel = Vec3::ZERO;

        let positions = simulate_single_body_on_terrain(
            initial_pos,
            initial_vel,
            mass,
            &occupancy,
            steps,
            dt,
            &config,
        );

        // Should fall through the gap (Y goes well below floor level)
        let final_y = positions.last().unwrap().y;
        assert!(
            final_y < 0.0,
            "Should fall through gap, final_y={}",
            final_y
        );
    }

    #[test]
    fn test_terrain_collision_force_similar_to_ground() {
        // J6: Force formula should match ground collision
        // A single voxel directly below should produce similar force to ground plane
        let config = PhysicsConfig::default();

        // Ground collision: particle at Y=0.3
        let particle_pos_ground = Vec3::new(0.0, 0.3, 0.0);
        let particle_vel = Vec3::ZERO;
        let ground_force =
            compute_ground_collision_force(particle_pos_ground, particle_vel, &config);

        // Terrain collision: voxel at (0,0,0), particle at same relative position
        // Voxel center is at (0.5, 0.5, 0.5)
        // To get same penetration, particle should be at distance 0.8 from voxel center
        // If particle at (0.5, Y, 0.5), distance = |Y - 0.5|
        // For distance 0.8, Y = 0.5 + 0.8 = 1.3 (above) or Y = 0.5 - 0.8 = -0.3 (below, inside voxel)
        // Let's use Y = 1.3 (particle above voxel, penetrating from above)
        // Actually for ground: ground particle at Y=-0.5, particle at Y=0.3
        // distance = 0.3 - (-0.5) = 0.8, penetration = 1.0 - 0.8 = 0.2

        let mut occupancy = WorldOccupancy::new();
        let mut chunk = ChunkOccupancy::new();
        chunk.set(UVec3::new(0, 0, 0), true);
        occupancy.load_chunk(IVec3::ZERO, chunk);

        // Voxel center at (0.5, 0.5, 0.5), particle 0.8 above = (0.5, 1.3, 0.5)
        let particle_pos_terrain = Vec3::new(0.5, 1.3, 0.5);
        let terrain_force = compute_terrain_collision_force(
            particle_pos_terrain,
            particle_vel,
            &occupancy,
            &config,
        );

        // Both should produce upward force
        assert!(ground_force.y > 0.0, "Ground force should be up");
        assert!(terrain_force.y > 0.0, "Terrain force should be up");

        // Magnitudes should be similar (same penetration depth)
        let ground_mag = ground_force.length();
        let terrain_mag = terrain_force.length();
        assert!(
            (ground_mag - terrain_mag).abs() < ground_mag * 0.1,
            "Forces should be similar: ground={}, terrain={}",
            ground_mag,
            terrain_mag
        );
    }
}
