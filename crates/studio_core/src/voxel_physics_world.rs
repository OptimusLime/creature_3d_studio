//! Voxel physics world simulation.
//!
//! This module provides a physics-engine-like API for kinematic bodies
//! colliding with voxel terrain. It wraps the `WorldOccupancy` collision
//! system and provides deterministic fixed-timestep simulation.
//!
//! ## Why This Exists
//!
//! The raw `KinematicController` in `voxel_collision.rs` requires the caller
//! to manage gravity, velocity, and timestep logic. This leads to:
//! - Physics logic scattered in examples
//! - Variable timestep causing instability
//! - Non-deterministic behavior at different frame rates
//!
//! `VoxelPhysicsWorld` encapsulates all of this:
//! - Fixed timestep with accumulator pattern
//! - Gravity and collision response
//! - Deterministic simulation regardless of frame rate
//!
//! ## Usage
//!
//! ```ignore
//! use studio_core::voxel_physics_world::{VoxelPhysicsWorld, PhysicsConfig, KinematicBody};
//!
//! // Create physics world
//! let occupancy = WorldOccupancy::from_voxel_world(&terrain);
//! let config = PhysicsConfig::default();
//! let mut physics = VoxelPhysicsWorld::new(occupancy, config);
//!
//! // Add a body
//! let body = physics.add_body(KinematicBody {
//!     position: Vec3::new(0.0, 10.0, 0.0),
//!     half_extents: Vec3::new(0.4, 0.9, 0.4),
//!     ..Default::default()
//! });
//!
//! // In your update loop - just call step with frame delta
//! physics.step(time.delta_secs());
//!
//! // Apply player input
//! physics.set_body_input_velocity(body, move_dir * speed);
//! if jump_pressed {
//!     physics.jump(body, 10.0);
//! }
//!
//! // Read position for rendering
//! let state = physics.get_body(body).unwrap();
//! transform.translation = state.position;
//! ```

use bevy::prelude::*;
use crate::voxel_collision::{WorldOccupancy, FragmentOccupancy};

/// Type of physics body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BodyKind {
    /// Kinematic body - player-style movement, no rotation.
    /// Controlled via input velocity and jump.
    #[default]
    Kinematic,
    /// Dynamic body - fragment-style physics with rotation.
    /// Affected by gravity and collisions, can tumble.
    Dynamic,
}

/// Handle to a kinematic body in the physics world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyHandle(usize);

/// Physics configuration.
#[derive(Debug, Clone)]
pub struct PhysicsConfig {
    /// Fixed timestep for physics simulation (default: 1/60 second).
    pub fixed_timestep: f32,
    /// Gravity vector (default: -25.0 on Y axis).
    pub gravity: Vec3,
    /// Maximum physics steps per frame to prevent spiral of death.
    pub max_steps_per_frame: u32,
    /// Maximum slope angle that can be walked on (radians).
    pub max_slope_angle: f32,
    /// Small margin to prevent floating point issues.
    pub skin_width: f32,
    /// Number of collision iterations per step.
    pub collision_iterations: u32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            fixed_timestep: 1.0 / 60.0,
            gravity: Vec3::new(0.0, -25.0, 0.0),
            max_steps_per_frame: 8,
            max_slope_angle: 0.785, // ~45 degrees
            skin_width: 0.01,
            collision_iterations: 4,
        }
    }
}

/// A physics body in the physics world.
///
/// Can be either `Kinematic` (player-style, no rotation) or `Dynamic` (fragment-style with rotation).
#[derive(Debug, Clone)]
pub struct PhysicsBody {
    /// What kind of body this is.
    pub kind: BodyKind,
    /// World-space position (center of AABB).
    pub position: Vec3,
    /// Rotation (identity for Kinematic bodies).
    pub rotation: Quat,
    /// Current velocity (includes gravity effects).
    pub velocity: Vec3,
    /// Angular velocity in radians/sec (zero for Kinematic bodies).
    pub angular_velocity: Vec3,
    /// Half-extents of the collision box.
    pub half_extents: Vec3,
    /// Whether the body is currently on the ground.
    pub grounded: bool,
    /// Normal of the ground surface (if grounded).
    pub ground_normal: Vec3,
    /// Voxel occupancy for Dynamic bodies (None for Kinematic).
    pub occupancy: Option<FragmentOccupancy>,
    /// Input velocity from player/AI control (horizontal movement).
    /// This is applied each frame before physics step. Only used for Kinematic bodies.
    input_velocity: Vec3,
    /// Whether a jump was requested this frame. Only used for Kinematic bodies.
    jump_requested: bool,
    /// Jump speed to apply if jump is requested. Only used for Kinematic bodies.
    jump_speed: f32,
}

impl Default for PhysicsBody {
    fn default() -> Self {
        Self {
            kind: BodyKind::Kinematic,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            half_extents: Vec3::new(0.4, 0.9, 0.4), // Player-sized
            grounded: false,
            ground_normal: Vec3::Y,
            occupancy: None,
            input_velocity: Vec3::ZERO,
            jump_requested: false,
            jump_speed: 0.0,
        }
    }
}

impl PhysicsBody {
    /// Create a player-sized kinematic body at the given position.
    pub fn player(position: Vec3) -> Self {
        Self {
            kind: BodyKind::Kinematic,
            position,
            half_extents: Vec3::new(0.4, 0.9, 0.4),
            ..Default::default()
        }
    }

    /// Create a kinematic body with custom half-extents.
    pub fn with_extents(position: Vec3, half_extents: Vec3) -> Self {
        Self {
            kind: BodyKind::Kinematic,
            position,
            half_extents,
            ..Default::default()
        }
    }

    /// Create a dynamic body with voxel occupancy for collision.
    ///
    /// Dynamic bodies have rotation and angular velocity, and use the
    /// fragment occupancy data for voxel-accurate collision detection.
    pub fn dynamic(position: Vec3, occupancy: FragmentOccupancy) -> Self {
        let aabb_size = occupancy.aabb_size();
        Self {
            kind: BodyKind::Dynamic,
            position,
            rotation: Quat::IDENTITY,
            half_extents: aabb_size * 0.5,
            occupancy: Some(occupancy),
            ..Default::default()
        }
    }
}

/// Type alias for backwards compatibility.
pub type KinematicBody = PhysicsBody;

/// Physics simulation for bodies in a voxel world.
///
/// This provides a self-contained physics world that:
/// - Uses fixed timestep for deterministic simulation
/// - Handles gravity and collision response internally
/// - Provides simple API for player/AI control (Kinematic bodies)
/// - Simulates tumbling voxel fragments (Dynamic bodies)
pub struct VoxelPhysicsWorld {
    /// Voxel occupancy data for collision queries.
    occupancy: WorldOccupancy,
    /// All physics bodies in the world.
    bodies: Vec<PhysicsBody>,
    /// Physics configuration.
    config: PhysicsConfig,
    /// Accumulator for fixed timestep.
    accumulator: f32,
}

impl VoxelPhysicsWorld {
    /// Create a new physics world.
    pub fn new(occupancy: WorldOccupancy, config: PhysicsConfig) -> Self {
        Self {
            occupancy,
            bodies: Vec::new(),
            config,
            accumulator: 0.0,
        }
    }

    /// Create with default configuration.
    pub fn with_default_config(occupancy: WorldOccupancy) -> Self {
        Self::new(occupancy, PhysicsConfig::default())
    }

    /// Add a physics body to the world.
    pub fn add_body(&mut self, body: PhysicsBody) -> BodyHandle {
        let handle = BodyHandle(self.bodies.len());
        self.bodies.push(body);
        handle
    }

    /// Get a body by handle.
    pub fn get_body(&self, handle: BodyHandle) -> Option<&PhysicsBody> {
        self.bodies.get(handle.0)
    }

    /// Get a mutable body by handle.
    pub fn get_body_mut(&mut self, handle: BodyHandle) -> Option<&mut PhysicsBody> {
        self.bodies.get_mut(handle.0)
    }

    /// Get the transform (position and rotation) of a body.
    ///
    /// This is useful for syncing physics state to rendering transforms.
    pub fn get_transform(&self, handle: BodyHandle) -> Option<(Vec3, Quat)> {
        self.bodies.get(handle.0).map(|b| (b.position, b.rotation))
    }

    /// Set the input velocity for a body (horizontal movement from player/AI).
    ///
    /// This velocity is applied each physics step. It does NOT include gravity.
    /// Typically you'd set this to something like `move_direction * speed`.
    pub fn set_body_input_velocity(&mut self, handle: BodyHandle, velocity: Vec3) {
        if let Some(body) = self.bodies.get_mut(handle.0) {
            body.input_velocity = velocity;
        }
    }

    /// Request a jump for a body. Only works if grounded.
    ///
    /// The jump will be applied on the next physics step if the body is grounded.
    pub fn jump(&mut self, handle: BodyHandle, speed: f32) {
        if let Some(body) = self.bodies.get_mut(handle.0) {
            body.jump_requested = true;
            body.jump_speed = speed;
        }
    }

    /// Check if a body is grounded.
    pub fn is_grounded(&self, handle: BodyHandle) -> bool {
        self.bodies.get(handle.0).map(|b| b.grounded).unwrap_or(false)
    }

    /// Get the position of a body.
    pub fn get_position(&self, handle: BodyHandle) -> Option<Vec3> {
        self.bodies.get(handle.0).map(|b| b.position)
    }

    /// Get the velocity of a body.
    pub fn get_velocity(&self, handle: BodyHandle) -> Option<Vec3> {
        self.bodies.get(handle.0).map(|b| b.velocity)
    }

    /// Step the physics simulation.
    ///
    /// This uses fixed timestep internally. Call this with your frame delta time.
    /// The simulation will run 0 or more fixed steps to catch up.
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

    /// Run a single fixed timestep.
    fn step_fixed(&mut self, dt: f32) {
        let gravity = self.config.gravity;
        let collision_iterations = self.config.collision_iterations;
        
        for i in 0..self.bodies.len() {
            let body = &mut self.bodies[i];
            
            match body.kind {
                BodyKind::Kinematic => {
                    Self::step_kinematic_body(&self.occupancy, body, dt, gravity, collision_iterations);
                }
                BodyKind::Dynamic => {
                    Self::step_dynamic_body(&self.occupancy, body, dt, gravity, collision_iterations);
                }
            }
        }
    }

    /// Step a kinematic body (player-style, no rotation).
    fn step_kinematic_body(
        occupancy: &WorldOccupancy,
        body: &mut PhysicsBody,
        dt: f32,
        gravity: Vec3,
        collision_iterations: u32,
    ) {
        // Apply input velocity (horizontal movement from player)
        body.velocity.x = body.input_velocity.x;
        body.velocity.z = body.input_velocity.z;

        // Process jump request
        if body.jump_requested && body.grounded {
            body.velocity.y = body.jump_speed;
            body.grounded = false;
        }
        body.jump_requested = false;

        // Apply gravity
        if !body.grounded {
            body.velocity += gravity * dt;
        } else {
            // Clamp downward velocity when grounded
            if body.velocity.y < 0.0 {
                body.velocity.y = 0.0;
            }
        }

        // Move and collide
        Self::move_kinematic_body(occupancy, body, dt, collision_iterations);
    }

    /// Step a dynamic body (fragment-style with rotation).
    fn step_dynamic_body(
        occupancy: &WorldOccupancy,
        body: &mut PhysicsBody,
        dt: f32,
        gravity: Vec3,
        collision_iterations: u32,
    ) {
        // Apply gravity (always, dynamic bodies don't have "grounded" prevention)
        body.velocity += gravity * dt;

        // Apply angular velocity to rotation
        if body.angular_velocity.length_squared() > 0.0001 {
            let angle = body.angular_velocity.length() * dt;
            let axis = body.angular_velocity.normalize_or_zero();
            if axis.length_squared() > 0.0 {
                body.rotation = Quat::from_axis_angle(axis, angle) * body.rotation;
                body.rotation = body.rotation.normalize();
            }
        }

        // Move
        body.position += body.velocity * dt;

        // Collision using fragment occupancy (if present)
        let Some(ref fragment_occupancy) = body.occupancy else {
            // No occupancy data - fallback to AABB collision (shouldn't normally happen)
            return;
        };

        body.grounded = false;

        for _ in 0..collision_iterations {
            let result = occupancy.check_fragment(
                fragment_occupancy,
                body.position,
                body.rotation,
            );

            if !result.has_collision() {
                break;
            }

            let resolution = result.resolution_vector();
            body.position += resolution;

            // Floor contact detection and response
            if result.has_floor_contact() {
                body.grounded = true;
                body.ground_normal = result.average_normal();
                
                // Damping on floor contact
                if body.velocity.y < 0.0 {
                    body.velocity.y *= -0.2; // Slight bounce, mostly absorbed
                }
                body.velocity.x *= 0.95; // Friction
                body.velocity.z *= 0.95;
                body.angular_velocity *= 0.9; // Angular damping
            }

            // Torque from off-center contact (causes tumbling)
            let avg_contact = result.average_contact_position();
            let lever = avg_contact - body.position;
            if lever.length_squared() > 0.01 {
                let torque = lever.cross(resolution.normalize_or_zero()) * resolution.length() * 0.5;
                body.angular_velocity += torque * dt;
            }
        }

        // Clamp angular velocity to prevent spinning too fast
        let max_angular = 10.0;
        if body.angular_velocity.length() > max_angular {
            body.angular_velocity = body.angular_velocity.normalize() * max_angular;
        }
    }

    /// Move a kinematic body and resolve collisions using AABB checks.
    fn move_kinematic_body(occupancy: &WorldOccupancy, body: &mut PhysicsBody, dt: f32, collision_iterations: u32) {
        let mut remaining_velocity = body.velocity * dt;
        let was_grounded = body.grounded;
        body.grounded = false;
        body.ground_normal = Vec3::Y;

        for _ in 0..collision_iterations {
            if remaining_velocity.length_squared() < 0.0001 {
                break;
            }

            // Try to move
            let target = body.position + remaining_velocity;
            let aabb_min = target - body.half_extents;
            let aabb_max = target + body.half_extents;

            let result = occupancy.check_aabb(aabb_min, aabb_max);

            if !result.has_collision() {
                // No collision, move freely
                body.position = target;
                break;
            }

            // Resolve collision
            let resolution = result.resolution_vector();
            body.position = target + resolution;

            // Check for ground contact
            if result.has_floor_contact() {
                body.grounded = true;
                if let Some(normal) = result.floor_normal() {
                    body.ground_normal = normal;
                }
                // Zero vertical velocity when hitting ground
                if body.velocity.y < 0.0 {
                    body.velocity.y = 0.0;
                }
            }

            // Slide along surface: find the primary blocking normal
            // We use maximum dot product (most opposing) to find main collision direction
            let mut best_normal = Vec3::ZERO;
            let mut best_dot = 0.0f32;
            
            for contact in &result.contacts {
                let dot = remaining_velocity.dot(contact.normal);
                if dot < best_dot {
                    best_dot = dot;
                    best_normal = contact.normal;
                }
            }
            
            // Remove velocity component into the blocking surface
            if best_dot < 0.0 {
                remaining_velocity -= best_normal * best_dot;
                
                // Also adjust body velocity for this axis
                let vel_dot = body.velocity.dot(best_normal);
                if vel_dot < 0.0 {
                    body.velocity -= best_normal * vel_dot;
                }
            }
        }

        // Ground check: probe slightly below to detect ground when stationary
        if !body.grounded {
            let probe_distance = 0.05;
            let ground_probe_min = body.position - body.half_extents - Vec3::new(0.0, probe_distance, 0.0);
            let ground_probe_max = body.position + body.half_extents;
            let ground_result = occupancy.check_aabb(ground_probe_min, ground_probe_max);
            if ground_result.has_floor_contact() {
                body.grounded = true;
                if let Some(normal) = ground_result.floor_normal() {
                    body.ground_normal = normal;
                }
            }
        }

        // Snap to ground if we were grounded and moving down a small slope
        if was_grounded && !body.grounded && body.velocity.y <= 0.0 {
            let snap_distance = 0.2;
            let snap_probe_min = body.position - body.half_extents - Vec3::new(0.0, snap_distance, 0.0);
            let snap_probe_max = body.position + body.half_extents;
            let snap_result = occupancy.check_aabb(snap_probe_min, snap_probe_max);
            if snap_result.has_floor_contact() {
                // Snap down to ground
                let resolution = snap_result.resolution_vector();
                if resolution.y > 0.0 && resolution.y < snap_distance {
                    body.position.y += resolution.y - snap_distance;
                    body.grounded = true;
                }
            }
        }
    }

    /// Get the occupancy (for external queries).
    pub fn occupancy(&self) -> &WorldOccupancy {
        &self.occupancy
    }

    /// Get the configuration.
    pub fn config(&self) -> &PhysicsConfig {
        &self.config
    }

    /// Number of bodies in the world.
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::{Voxel, VoxelWorld};

    /// Helper to create a test world with a flat floor.
    fn create_floor_world() -> VoxelWorld {
        let mut world = VoxelWorld::new();
        for x in -15..15 {
            for z in -15..15 {
                for y in 0..3 {
                    world.set_voxel(x, y, z, Voxel::solid(80, 120, 80));
                }
            }
        }
        world
    }

    #[test]
    fn test_body_falls_and_lands() {
        let terrain = create_floor_world();
        let occupancy = WorldOccupancy::from_voxel_world(&terrain);
        let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);

        // Add body at y=10, above the floor (which tops out at y=3)
        let body = physics.add_body(KinematicBody::player(Vec3::new(0.0, 10.0, 0.0)));

        // Simulate 3 seconds (180 steps at 60fps)
        for _ in 0..180 {
            physics.step(1.0 / 60.0);
        }

        let state = physics.get_body(body).unwrap();
        
        println!("Final position: {:?}, grounded: {}", state.position, state.grounded);
        
        // Should have landed on floor at y ≈ 3.9 (floor top 3.0 + half height 0.9)
        assert!(state.grounded, "Body should be grounded after falling");
        assert!((state.position.y - 3.9).abs() < 0.3, 
            "Body should land at ~3.9, got {}", state.position.y);
    }

    #[test]
    fn test_body_stops_at_wall() {
        let mut world = VoxelWorld::new();
        
        // Floor
        for x in -10..20 {
            for z in -5..5 {
                for y in 0..3 {
                    world.set_voxel(x, y, z, Voxel::solid(80, 120, 80));
                }
            }
        }
        
        // Wall at x=10
        for y in 3..8 {
            for z in -5..5 {
                world.set_voxel(10, y, z, Voxel::solid(150, 100, 100));
            }
        }

        let occupancy = WorldOccupancy::from_voxel_world(&world);
        let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);

        // Add body on floor, moving toward wall
        let body = physics.add_body(KinematicBody::player(Vec3::new(0.0, 3.9, 0.0)));

        // Move toward wall for 2 seconds
        for _ in 0..120 {
            physics.set_body_input_velocity(body, Vec3::new(10.0, 0.0, 0.0));
            physics.step(1.0 / 60.0);
        }

        let state = physics.get_body(body).unwrap();
        
        println!("Final position: {:?}", state.position);
        
        // Should be blocked by wall at x=10
        // Wall starts at x=10, body half-extent is 0.4, so max x is ~9.6
        assert!(state.position.x < 10.0, 
            "Body should be blocked by wall, got x={}", state.position.x);
    }

    #[test]
    fn test_body_slides_along_wall() {
        let mut world = VoxelWorld::new();
        
        // Floor
        for x in -10..20 {
            for z in -20..20 {
                for y in 0..3 {
                    world.set_voxel(x, y, z, Voxel::solid(80, 120, 80));
                }
            }
        }
        
        // Wall at x=10
        for y in 3..8 {
            for z in -20..20 {
                world.set_voxel(10, y, z, Voxel::solid(150, 100, 100));
            }
        }

        let occupancy = WorldOccupancy::from_voxel_world(&world);
        let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);

        // Add body near wall
        let body = physics.add_body(KinematicBody::player(Vec3::new(8.0, 3.9, 0.0)));
        let start_z = physics.get_position(body).unwrap().z;

        // Move diagonally into wall for 1 second
        for _ in 0..60 {
            physics.set_body_input_velocity(body, Vec3::new(10.0, 0.0, 10.0));
            physics.step(1.0 / 60.0);
        }

        let state = physics.get_body(body).unwrap();
        let z_moved = state.position.z - start_z;
        
        println!("Final position: {:?}, Z moved: {}", state.position, z_moved);
        
        // Should have slid along wall in Z direction
        assert!(state.position.x < 10.0, "Should be blocked by wall");
        assert!(z_moved > 3.0, "Should have moved significantly in Z, got {}", z_moved);
    }

    #[test]
    fn test_body_jumps() {
        let terrain = create_floor_world();
        let occupancy = WorldOccupancy::from_voxel_world(&terrain);
        let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);

        // Add body on floor
        let body = physics.add_body(KinematicBody::player(Vec3::new(0.0, 3.9, 0.0)));
        
        // First ensure grounded (run a few steps)
        for _ in 0..10 {
            physics.step(1.0 / 60.0);
        }
        
        assert!(physics.is_grounded(body), "Should start grounded");
        
        let start_y = physics.get_position(body).unwrap().y;
        
        // Jump
        physics.jump(body, 10.0);
        
        // Run a few steps
        for _ in 0..15 {
            physics.step(1.0 / 60.0);
        }
        
        let peak_y = physics.get_position(body).unwrap().y;
        
        println!("Start Y: {}, Peak Y: {}", start_y, peak_y);
        
        // Should have gone up
        assert!(peak_y > start_y + 0.5, "Should have jumped up, got peak={} start={}", peak_y, start_y);
        assert!(!physics.is_grounded(body), "Should be in air");
    }

    #[test]
    fn test_body_no_jump_in_air() {
        let terrain = create_floor_world();
        let occupancy = WorldOccupancy::from_voxel_world(&terrain);
        let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);

        // Add body in the air
        let body = physics.add_body(KinematicBody::player(Vec3::new(0.0, 10.0, 0.0)));
        
        // Try to jump while in air
        physics.jump(body, 10.0);
        
        let vel_before = physics.get_velocity(body).unwrap().y;
        
        physics.step(1.0 / 60.0);
        
        let vel_after = physics.get_velocity(body).unwrap().y;
        
        println!("Vel before: {}, after: {}", vel_before, vel_after);
        
        // Should NOT have gotten positive velocity from jump (in air = no jump)
        // Velocity should be negative (falling) after the step
        assert!(vel_after < 0.0, "Should be falling, not jumping, vel={}", vel_after);
    }

    #[test]
    fn test_fixed_timestep_determinism() {
        // Run the same simulation with different frame rates
        // Results should be nearly identical
        
        fn run_simulation(steps: u32, dt: f32) -> Vec3 {
            let terrain = create_floor_world();
            let occupancy = WorldOccupancy::from_voxel_world(&terrain);
            let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);
            
            let body = physics.add_body(KinematicBody::player(Vec3::new(0.0, 10.0, 0.0)));
            
            for _ in 0..steps {
                physics.step(dt);
            }
            
            physics.get_position(body).unwrap()
        }
        
        // 3 seconds at different "frame rates"
        let pos_60fps = run_simulation(180, 1.0 / 60.0);  // 60fps
        let pos_30fps = run_simulation(90, 1.0 / 30.0);   // 30fps
        let pos_120fps = run_simulation(360, 1.0 / 120.0); // 120fps
        
        println!("60fps: {:?}", pos_60fps);
        println!("30fps: {:?}", pos_30fps);
        println!("120fps: {:?}", pos_120fps);
        
        // All should land at approximately the same position
        assert!((pos_60fps.y - pos_30fps.y).abs() < 0.2, 
            "60fps and 30fps should match, got {} vs {}", pos_60fps.y, pos_30fps.y);
        assert!((pos_60fps.y - pos_120fps.y).abs() < 0.2, 
            "60fps and 120fps should match, got {} vs {}", pos_60fps.y, pos_120fps.y);
    }

    #[test]
    fn test_body_cross_chunk_collision() {
        let mut world = VoxelWorld::new();
        
        // Floor spanning chunk boundary (chunks are 32 voxels)
        for x in 28..36 {  // Spans from chunk 0 to chunk 1
            for z in -5..5 {
                for y in 0..3 {
                    world.set_voxel(x, y, z, Voxel::solid(80, 120, 80));
                }
            }
        }

        let occupancy = WorldOccupancy::from_voxel_world(&world);
        let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);

        // Add body above chunk boundary
        let body = physics.add_body(KinematicBody::player(Vec3::new(32.0, 10.0, 0.0)));

        // Let it fall
        for _ in 0..180 {
            physics.step(1.0 / 60.0);
        }

        let state = physics.get_body(body).unwrap();
        
        println!("Final position: {:?}, grounded: {}", state.position, state.grounded);
        
        // Should land on the cross-chunk floor
        assert!(state.grounded, "Should land on cross-chunk floor");
        assert!((state.position.y - 3.9).abs() < 0.3, 
            "Should land at ~3.9, got {}", state.position.y);
    }

    #[test]
    fn test_p23_exact_scenario() {
        // Exact scenario from p23_kinematic_controller example
        let mut world = VoxelWorld::new();
        
        // Ground platform (30x30, 3 blocks thick) - same as example
        for x in -15..15 {
            for z in -15..15 {
                for y in 0..3 {
                    world.set_voxel(x, y, z, Voxel::solid(80, 120, 80));
                }
            }
        }

        let occupancy = WorldOccupancy::from_voxel_world(&world);
        let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);

        // Same starting position as example (y=10)
        let body = physics.add_body(KinematicBody::player(Vec3::new(0.0, 10.0, 0.0)));

        println!("Starting position: {:?}", physics.get_position(body).unwrap());
        println!("Player bottom: {}", physics.get_position(body).unwrap().y - 0.9);
        println!("Floor top: 3.0 (voxels at y=0,1,2 occupy up to y=3)");
        println!("Expected landing y: 3.0 + 0.9 = 3.9");

        // Simulate 3 seconds
        for i in 0..180 {
            physics.step(1.0 / 60.0);
            
            if i % 20 == 0 {
                let state = physics.get_body(body).unwrap();
                println!("Frame {}: pos.y={:.3}, vel.y={:.3}, grounded={}", 
                    i, state.position.y, state.velocity.y, state.grounded);
            }
        }

        let state = physics.get_body(body).unwrap();
        println!("Final: pos={:?}, grounded={}", state.position, state.grounded);

        assert!(state.grounded, "Should be grounded after 3 seconds of falling");
        assert!((state.position.y - 3.9).abs() < 0.3, "Should land at ~3.9, got {}", state.position.y);
    }

    // ========== Dynamic Body Tests ==========

    #[test]
    fn test_dynamic_body_falls_and_lands() {
        let terrain = create_floor_world();
        let occupancy = WorldOccupancy::from_voxel_world(&terrain);
        let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);

        // Create a 2x2x2 fragment occupancy (small cube)
        let mut frag_world = VoxelWorld::new();
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    frag_world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }
        let frag_occupancy = FragmentOccupancy::from_voxel_world(&frag_world);

        // Add dynamic body above the floor
        let body = physics.add_body(PhysicsBody::dynamic(Vec3::new(0.0, 10.0, 0.0), frag_occupancy));

        println!("Dynamic body test - starting at y=10.0");
        
        // Simulate 3 seconds (should be enough to fall and settle)
        for i in 0..180 {
            physics.step(1.0 / 60.0);
            
            if i % 30 == 0 {
                let state = physics.get_body(body).unwrap();
                println!("Frame {}: pos.y={:.3}, vel.y={:.3}, grounded={}", 
                    i, state.position.y, state.velocity.y, state.grounded);
            }
        }

        let state = physics.get_body(body).unwrap();
        println!("Final: pos={:?}, grounded={}", state.position, state.grounded);

        // Should have fallen and landed
        // Floor top is at y=3.0, fragment is 2 units tall, center should be at ~4.0
        assert!(state.grounded, "Dynamic body should land on floor");
        assert!(state.position.y < 6.0, "Should have fallen from y=10, got {}", state.position.y);
        assert!(state.position.y > 3.0, "Should be above floor top (y=3), got {}", state.position.y);
    }

    #[test]
    fn test_dynamic_body_rotation() {
        let terrain = create_floor_world();
        let occupancy = WorldOccupancy::from_voxel_world(&terrain);
        let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);

        // Create a single voxel fragment
        let mut frag_world = VoxelWorld::new();
        frag_world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        let frag_occupancy = FragmentOccupancy::from_voxel_world(&frag_world);

        // Add dynamic body with initial angular velocity (spinning around Y)
        let mut body_data = PhysicsBody::dynamic(Vec3::new(0.0, 20.0, 0.0), frag_occupancy);
        body_data.angular_velocity = Vec3::new(0.0, 5.0, 0.0); // 5 rad/s around Y
        let body = physics.add_body(body_data);

        let initial_rot = physics.get_body(body).unwrap().rotation;
        println!("Initial rotation: {:?}", initial_rot);

        // Simulate 1 second (60 frames)
        for _ in 0..60 {
            physics.step(1.0 / 60.0);
        }

        let state = physics.get_body(body).unwrap();
        let final_rot = state.rotation;
        let angle_diff = initial_rot.angle_between(final_rot);

        println!("Final rotation: {:?}", final_rot);
        println!("Angle difference: {:.3} radians", angle_diff);

        // Should have rotated significantly (5 rad/s * 1s = 5 radians, but clamped/damped)
        assert!(angle_diff > 0.5, "Should have rotated at least 0.5 radians, got {}", angle_diff);
    }

    #[test]
    fn test_dynamic_body_kind_is_dynamic() {
        let mut frag_world = VoxelWorld::new();
        frag_world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        let frag_occupancy = FragmentOccupancy::from_voxel_world(&frag_world);

        let body = PhysicsBody::dynamic(Vec3::new(0.0, 10.0, 0.0), frag_occupancy);
        
        assert_eq!(body.kind, BodyKind::Dynamic);
        assert!(body.occupancy.is_some());
        assert_eq!(body.rotation, Quat::IDENTITY);
        assert_eq!(body.angular_velocity, Vec3::ZERO);
    }

    #[test]
    fn test_kinematic_body_kind_is_kinematic() {
        let body = PhysicsBody::player(Vec3::new(0.0, 10.0, 0.0));
        
        assert_eq!(body.kind, BodyKind::Kinematic);
        assert!(body.occupancy.is_none());
        assert_eq!(body.rotation, Quat::IDENTITY);
        assert_eq!(body.angular_velocity, Vec3::ZERO);
    }

    #[test]
    fn test_get_transform() {
        let terrain = create_floor_world();
        let occupancy = WorldOccupancy::from_voxel_world(&terrain);
        let mut physics = VoxelPhysicsWorld::with_default_config(occupancy);

        // Create a dynamic body with non-identity rotation
        let mut frag_world = VoxelWorld::new();
        frag_world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        let frag_occupancy = FragmentOccupancy::from_voxel_world(&frag_world);

        let mut body_data = PhysicsBody::dynamic(Vec3::new(5.0, 15.0, 3.0), frag_occupancy);
        body_data.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        let body = physics.add_body(body_data);

        let (pos, rot) = physics.get_transform(body).unwrap();
        
        assert!((pos.x - 5.0).abs() < 0.001);
        assert!((pos.y - 15.0).abs() < 0.001);
        assert!((pos.z - 3.0).abs() < 0.001);
        assert!((rot.angle_between(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4))).abs() < 0.001);
    }
}
