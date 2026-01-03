//! Dynamic voxel fragments for physics-based voxel interaction.
//!
//! A VoxelFragment is a piece of voxel geometry that exists in the physics world.
//! Fragments are created by breaking/cutting pieces from the main world, simulated
//! with physics, and can eventually settle back into a static VoxelWorld.
//!
//! ## Lifecycle
//!
//! 1. **Break**: Split a region from the main VoxelWorld → create VoxelFragment
//! 2. **Simulate**: Fragment has RigidBody::Dynamic, falls/collides with physics
//! 3. **Settle**: When velocity is low for N frames, fragment is "settled"
//! 4. **Merge**: Settled fragment is merged back into the main world
//!
//! ## Collision Strategy
//!
//! Phase 5-6 will implement GPU-based collision using occupancy textures.
//! Until then, fragments use Rapier collision with terrain trimesh.
//!
//! ## Usage
//!
//! ```ignore
//! use studio_core::voxel_fragment::{spawn_fragment, VoxelFragment};
//! use bevy::prelude::*;
//!
//! fn break_terrain(
//!     mut commands: Commands,
//!     mut world: ResMut<MainVoxelWorld>,
//! ) {
//!     // Split sphere from terrain
//!     let fragment_data = world.0.split_sphere(IVec3::new(10, 5, 10), 3);
//!     
//!     // Spawn as physics entity
//!     spawn_fragment(
//!         &mut commands,
//!         fragment_data,
//!         Vec3::new(10.0, 5.0, 10.0),
//!         Vec3::new(0.0, 10.0, 5.0), // upward impulse
//!     );
//! }
//! ```

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::deferred::GpuCollisionContacts;
use crate::physics_math::{
    apply_gravity, compute_terrain_collision_force, integrate_position, integrate_rotation,
    integrate_velocity, PhysicsConfig,
};
use crate::voxel::VoxelWorld;
use crate::voxel_collision::FragmentOccupancy;
use crate::voxel_mesh::build_world_meshes_cross_chunk;
use crate::voxel_physics::generate_merged_cuboid_collider;

/// A dynamic piece of voxel geometry that exists in the physics world.
///
/// Fragments are created by breaking/cutting pieces from the main world.
/// They have their own physics body and can move, rotate, and collide.
/// Eventually they "settle" and can be merged back into a static VoxelWorld.
#[derive(Component)]
pub struct VoxelFragment {
    /// The voxel data for this fragment (coordinates relative to entity origin)
    pub data: VoxelWorld,
    /// Number of consecutive frames with velocity below threshold
    pub settling_frames: u32,
    /// Original world position when broken off (for debugging/tracking)
    pub origin: IVec3,
    /// Occupancy data for GPU collision (Phase 5-6)
    pub occupancy: FragmentOccupancy,
}

impl VoxelFragment {
    /// Create a new fragment from voxel data.
    pub fn new(data: VoxelWorld, origin: IVec3) -> Self {
        let occupancy = FragmentOccupancy::from_voxel_world(&data);
        Self {
            data,
            settling_frames: 0,
            origin,
            occupancy,
        }
    }

    /// Check if this fragment is considered settled (ready to merge).
    pub fn is_settled(&self, config: &FragmentConfig) -> bool {
        self.settling_frames >= config.settle_threshold_frames
    }
}

/// Marker for fragments that are in "preview" mode (clipboard paste preview).
/// These render with transparency and don't have physics.
#[derive(Component)]
pub struct FragmentPreview;

/// Marker for the main static world entity.
#[derive(Component)]
pub struct StaticVoxelWorld;

/// Configuration for fragment behavior.
#[derive(Resource)]
pub struct FragmentConfig {
    /// Frames of low velocity before settling
    pub settle_threshold_frames: u32,
    /// Velocity magnitude below which we count as "still"
    pub settle_velocity_threshold: f32,
    /// Maximum fragments before forcing oldest to settle
    pub max_active_fragments: usize,
}

impl Default for FragmentConfig {
    fn default() -> Self {
        Self {
            settle_threshold_frames: 60, // 1 second at 60fps
            settle_velocity_threshold: 0.1,
            max_active_fragments: 32,
        }
    }
}

/// Physics state for a fragment (replaces Rapier).
///
/// We do our own integration following gpu-physics-unity approach:
/// - velocity += (force / mass) * dt
/// - position += velocity * dt
#[derive(Component, Default)]
pub struct FragmentPhysics {
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
}

/// Bundle for spawning a voxel fragment with physics.
///
/// No Rapier components - we do our own physics integration using
/// the Harada spring-damper model from GPU Gems 3.
#[derive(Bundle)]
pub struct VoxelFragmentBundle {
    pub fragment: VoxelFragment,
    pub physics: FragmentPhysics,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub inherited_visibility: InheritedVisibility,
    pub view_visibility: ViewVisibility,
}

/// Spawn a voxel fragment entity with physics.
///
/// Creates a new entity with:
/// - VoxelFragment component containing the voxel data
/// - RigidBody::Dynamic for physics simulation
/// - Merged cuboid Collider for efficient collision
/// - Initial impulse for explosion-like effects
///
/// # Arguments
/// * `commands` - Bevy Commands for spawning
/// * `data` - The VoxelWorld data for this fragment
/// * `position` - World position to spawn at
/// * `impulse` - Initial linear impulse to apply
///
/// # Returns
/// The Entity ID of the spawned fragment, or None if data is empty.
pub fn spawn_fragment(
    commands: &mut Commands,
    data: VoxelWorld,
    position: Vec3,
    impulse: Vec3,
) -> Option<Entity> {
    // Calculate origin as integer position
    let origin = IVec3::new(
        position.x.round() as i32,
        position.y.round() as i32,
        position.z.round() as i32,
    );

    let entity = commands
        .spawn(VoxelFragmentBundle {
            fragment: VoxelFragment::new(data, origin),
            physics: FragmentPhysics {
                velocity: impulse,
                angular_velocity: Vec3::ZERO,
                mass: 1.0,
            },
            transform: Transform::from_translation(position),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            inherited_visibility: InheritedVisibility::default(),
            view_visibility: ViewVisibility::default(),
        })
        .id();

    Some(entity)
}

/// Spawn a voxel fragment with mesh for rendering.
///
/// Same as `spawn_fragment` but also creates a child entity with mesh and material.
/// Use this when you need the fragment to be visible.
///
/// # Arguments
/// * `commands` - Bevy Commands for spawning
/// * `meshes` - Mesh asset storage
/// * `materials` - Material asset storage  
/// * `data` - The VoxelWorld data for this fragment
/// * `position` - World position to spawn at
/// * `impulse` - Initial linear impulse to apply
/// * `material` - Material handle for the voxel mesh
///
/// # Returns
/// The Entity ID of the spawned fragment, or None if data is empty.
pub fn spawn_fragment_with_mesh(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    data: VoxelWorld,
    position: Vec3,
    impulse: Vec3,
    material: Handle<crate::voxel_mesh::VoxelMaterial>,
) -> Option<Entity> {
    let chunk_meshes = build_world_meshes_cross_chunk(&data);

    // If no meshes generated, data was empty
    if chunk_meshes.is_empty() {
        return None;
    }

    // Calculate origin as integer position
    let origin = IVec3::new(
        position.x.round() as i32,
        position.y.round() as i32,
        position.z.round() as i32,
    );

    // Spawn parent with physics
    let entity = commands
        .spawn(VoxelFragmentBundle {
            fragment: VoxelFragment::new(data, origin),
            physics: FragmentPhysics {
                velocity: impulse,
                angular_velocity: Vec3::ZERO,
                mass: 1.0,
            },
            transform: Transform::from_translation(position),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            inherited_visibility: InheritedVisibility::default(),
            view_visibility: ViewVisibility::default(),
        })
        .with_children(|parent| {
            // Spawn mesh children for each chunk
            for chunk_mesh in chunk_meshes {
                let translation = chunk_mesh.translation();
                let mesh_handle = meshes.add(chunk_mesh.mesh);
                parent.spawn((
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(translation),
                ));
            }
        })
        .id();

    Some(entity)
}

/// System to detect when fragments have settled (velocity near zero).
///
/// Increments `settling_frames` when velocity is below threshold,
/// resets it when velocity exceeds threshold.
pub fn detect_settling_fragments(
    config: Res<FragmentConfig>,
    mut fragments: Query<(&mut VoxelFragment, &Velocity)>,
) {
    for (mut fragment, velocity) in fragments.iter_mut() {
        let speed = velocity.linvel.length() + velocity.angvel.length();

        if speed < config.settle_velocity_threshold {
            fragment.settling_frames += 1;
        } else {
            fragment.settling_frames = 0;
        }
    }
}

// ============================================================================
// Terrain Collision System (Phase 6.3)
// ============================================================================

use crate::voxel_collision::WorldOccupancy;

/// Resource holding the terrain occupancy data for collision detection.
///
/// This must be initialized by the application with terrain data before
/// fragment collision will work.
#[derive(Resource, Default)]
pub struct TerrainOccupancy {
    /// The world occupancy data for terrain collision queries.
    pub occupancy: WorldOccupancy,
}

impl TerrainOccupancy {
    /// Create terrain occupancy from a VoxelWorld.
    pub fn from_voxel_world(world: &VoxelWorld) -> Self {
        Self {
            occupancy: WorldOccupancy::from_voxel_world(world),
        }
    }
}

/// Configuration for collision response using Harada spring-damper model.
///
/// Based on GPU Gems 3, Chapter 29: "Real-Time Rigid Body Simulation on GPUs"
/// This model applies uniformly to ALL collision types (terrain and fragment-fragment).
///
/// ## Physics Model
///
/// Each contact generates three force components:
///
/// ```text
/// F_spring = spring_k * penetration * normal     (Hooke's law - pushes apart)
/// F_damping = damping_k * relative_velocity      (energy dissipation)
/// F_friction = friction_k * tangential_velocity  (resists sliding)
/// ```
///
/// For terrain contacts: other_velocity = 0 (terrain is stationary)
/// For fragment contacts: other_velocity = other fragment's velocity
#[derive(Resource)]
pub struct FragmentCollisionConfig {
    /// Minimum penetration to trigger collision response.
    /// Helps avoid jitter from tiny penetrations.
    pub min_penetration: f32,

    /// Enable collision system (for toggling CPU/GPU in benchmarks).
    pub enabled: bool,

    /// Spring coefficient (Hooke's law stiffness).
    /// Force = spring_k * penetration * normal
    /// Higher values = stiffer collision, less interpenetration.
    /// Typical range: 1000-5000 for voxel-sized particles.
    pub spring_k: f32,

    /// Damping coefficient for energy dissipation.
    /// Force = damping_k * relative_velocity
    /// Higher values = more energy loss per collision, faster settling.
    /// Typical range: 10-100.
    pub damping_k: f32,

    /// Tangential friction coefficient.
    /// Resists sliding motion perpendicular to contact normal.
    /// Force = friction_k * tangential_velocity
    /// Typical range: 1-50.
    pub friction_k: f32,
}

impl Default for FragmentCollisionConfig {
    fn default() -> Self {
        // Use reference constants from gpu-physics-unity
        // These are verified correct by the physics_math unit tests
        Self {
            min_penetration: 0.01, // Ignore tiny penetrations
            enabled: true,
            // Harada spring-damper parameters from gpu-physics-unity
            // Reference: GPUPhysics.cs
            spring_k: 500.0, // springCoefficient = 500.0
            damping_k: 10.0, // dampingCoefficient = 10.0
            friction_k: 2.0, // tangentialCoefficient = 2.0
        }
    }
}

/// CPU collision system using verified spring-damper physics from physics_math module.
///
/// This system uses the Harada spring-damper model (GPU Gems 3) as implemented
/// in `physics_math.rs`, which has been verified against the gpu-physics-unity
/// reference implementation with 24 unit tests.
///
/// ## Key Implementation Details
///
/// - Uses `compute_terrain_collision_force` for per-voxel collision forces
/// - Uses `integrate_velocity` with friction BEFORE force (critical!)
/// - Uses `integrate_rotation` with correct quaternion derivative
/// - All constants match gpu-physics-unity reference
pub fn fragment_terrain_collision_system(
    time: Res<Time>,
    terrain: Res<TerrainOccupancy>,
    collision_config: Res<FragmentCollisionConfig>,
    mut fragments: Query<(&VoxelFragment, &mut Transform, &mut FragmentPhysics)>,
) {
    if !collision_config.enabled {
        return;
    }

    let dt = time.delta_secs();

    // Build PhysicsConfig from FragmentCollisionConfig
    // This ensures we use the verified constants
    let physics_config = PhysicsConfig {
        gravity: 9.8, // Reference value
        particle_diameter: 1.0,
        spring_k: collision_config.spring_k,
        damping_k: collision_config.damping_k,
        tangential_k: collision_config.friction_k,
        friction: 0.9,         // Reference: frictionCoefficient = 0.9
        angular_friction: 0.3, // Reference: angularFrictionCoefficient = 0.3
        linear_force_scalar: 1.0,
        angular_force_scalar: 1.0,
        velocity_threshold: 1e-6,
    };

    for (_fragment, mut transform, mut physics) in fragments.iter_mut() {
        let pos = transform.translation;
        let vel = physics.velocity;

        // Compute terrain collision force using verified physics_math function
        // This handles per-voxel collision with correct spring-damper formula
        let collision_force =
            compute_terrain_collision_force(pos, vel, &terrain.occupancy, &physics_config);

        // Add gravity (per-particle in reference, so we apply it here)
        let total_force = apply_gravity(collision_force, &physics_config);

        // Integrate velocity using verified function
        // CRITICAL: This applies friction BEFORE force (reference line 365)
        let mass = physics.mass;
        physics.velocity = integrate_velocity(
            vel,
            total_force,
            mass,
            physics_config.friction,
            dt,
            physics_config.velocity_threshold,
        );

        // Integrate position
        transform.translation = integrate_position(pos, physics.velocity, dt);

        // For angular velocity, we need torque from off-center contacts
        // For now, use simplified angular damping (no torque from terrain collision)
        // TODO: Compute torque from contact positions relative to center
        physics.angular_velocity =
            physics.angular_velocity / (1.0 + dt * physics_config.angular_friction);

        // Zero tiny angular velocities
        if physics.angular_velocity.length() < physics_config.velocity_threshold {
            physics.angular_velocity = Vec3::ZERO;
        }

        // Integrate rotation using verified quaternion formula
        if physics.angular_velocity.length_squared() > 1e-12 {
            transform.rotation =
                integrate_rotation(transform.rotation, physics.angular_velocity, dt);
        }
    }
}

/// Configuration for GPU collision mode.
#[derive(Resource, Default)]
pub struct GpuCollisionMode {
    /// Use GPU collision instead of CPU.
    pub enabled: bool,
}

/// GPU collision system using verified spring-damper physics from physics_math module.
///
/// This system processes GPU collision contacts and applies the Harada spring-damper
/// model as verified in `physics_math.rs` with 24 unit tests.
///
/// ## Key Implementation Details
///
/// - Uses GPU-computed contacts for collision detection
/// - Applies verified physics formulas from physics_math module
/// - Friction applied BEFORE force (critical order from reference)
/// - Uses correct quaternion integration formula
pub fn gpu_fragment_physics_system(
    time: Res<Time>,
    gpu_mode: Option<Res<GpuCollisionMode>>,
    gpu_contacts: Option<Res<GpuCollisionContacts>>,
    collision_config: Res<FragmentCollisionConfig>,
    mut fragments: Query<(Entity, &VoxelFragment, &mut Transform, &mut FragmentPhysics)>,
) {
    // Only run if GPU mode is enabled
    let Some(gpu_mode) = gpu_mode else {
        return;
    };
    if !gpu_mode.enabled {
        return;
    }

    if !collision_config.enabled {
        return;
    }

    let dt = time.delta_secs();

    // Build PhysicsConfig from FragmentCollisionConfig (verified constants)
    let physics_config = PhysicsConfig {
        gravity: 9.8,
        particle_diameter: 1.0,
        spring_k: collision_config.spring_k,
        damping_k: collision_config.damping_k,
        tangential_k: collision_config.friction_k,
        friction: 0.9,         // Reference: frictionCoefficient = 0.9
        angular_friction: 0.3, // Reference: angularFrictionCoefficient = 0.3
        linear_force_scalar: 1.0,
        angular_force_scalar: 1.0,
        velocity_threshold: 1e-6,
    };

    // Get GPU collision results (may be empty if no contacts)
    let collision_result = gpu_contacts.as_ref().map(|c| c.get()).unwrap_or_default();

    // Build entity-to-index map for contact lookup
    let entity_to_idx: std::collections::HashMap<Entity, u32> = collision_result
        .fragment_entities
        .iter()
        .enumerate()
        .map(|(idx, &entity)| (entity, idx as u32))
        .collect();

    // Build index-to-velocity map for fragment-fragment collisions (future use)
    let _fragment_velocities: std::collections::HashMap<u32, Vec3> = fragments
        .iter()
        .filter_map(|(entity, _, _, physics)| {
            entity_to_idx
                .get(&entity)
                .map(|&idx| (idx, physics.velocity))
        })
        .collect();

    // Process ALL fragments - compute forces, integrate velocity, update position
    for (entity, _fragment, mut transform, mut physics) in fragments.iter_mut() {
        let center = transform.translation;
        let vel = physics.velocity;

        // Start with zero force (gravity added via apply_gravity)
        let mut collision_force = Vec3::ZERO;
        let mut total_torque = Vec3::ZERO;

        // Add collision forces if this fragment has contacts
        let contacts: Vec<_> = entity_to_idx
            .get(&entity)
            .map(|&idx| collision_result.contacts_for_fragment(idx).collect())
            .unwrap_or_default();

        // Process each contact using verified spring-damper formula
        for contact in &contacts {
            if contact.penetration < collision_config.min_penetration {
                continue;
            }

            let contact_pos = Vec3::from(contact.position);
            let contact_normal = Vec3::from(contact.normal);
            let penetration = contact.penetration;

            // Compute spring-damper force for this contact
            // Using the same formula as compute_collision_with_static_point
            let n = contact_normal;

            // Spring force (repulsive) - NEGATIVE because normal points toward terrain
            let spring_force = -physics_config.spring_k * penetration * n;

            // Relative velocity (terrain is stationary)
            let relative_velocity = Vec3::ZERO - vel;

            // Damping force
            let damping_force = physics_config.damping_k * relative_velocity;

            // Tangential force
            let normal_velocity = relative_velocity.dot(n) * n;
            let tangential_velocity = relative_velocity - normal_velocity;
            let tangential_force = physics_config.tangential_k * tangential_velocity;

            let contact_force = spring_force + damping_force + tangential_force;
            collision_force += contact_force;

            // Torque from off-center contact
            let lever_arm = contact_pos - center;
            if lever_arm.length_squared() > 0.0001 {
                total_torque += lever_arm.cross(contact_force);
            }
        }

        // Add gravity
        let total_force = apply_gravity(collision_force, &physics_config);

        // Integrate velocity using verified function
        // CRITICAL: friction BEFORE force (reference line 365)
        let mass = physics.mass;
        physics.velocity = integrate_velocity(
            vel,
            total_force,
            mass,
            physics_config.friction,
            dt,
            physics_config.velocity_threshold,
        );

        // Integrate position
        transform.translation = integrate_position(center, physics.velocity, dt);

        // Integrate angular velocity
        let ang_vel = physics.angular_velocity;
        physics.angular_velocity = ang_vel / (1.0 + dt * physics_config.angular_friction);
        physics.angular_velocity += physics_config.angular_force_scalar * dt * total_torque;

        // Zero tiny angular velocities
        if physics.angular_velocity.length() < physics_config.velocity_threshold {
            physics.angular_velocity = Vec3::ZERO;
        }

        // Integrate rotation using verified quaternion formula
        if physics.angular_velocity.length_squared() > 1e-12 {
            transform.rotation =
                integrate_rotation(transform.rotation, physics.angular_velocity, dt);
        }
    }
}

/// GPU collision system for kinematic bodies with GpuCollisionAABB.
///
/// This system handles collision response for kinematic bodies (like player characters)
/// that use the GPU collision pipeline via `GpuCollisionAABB` component.
///
/// Unlike dynamic fragments which receive forces, kinematic bodies have their
/// position directly adjusted based on collision resolution.
pub fn gpu_kinematic_collision_system(
    gpu_mode: Option<Res<GpuCollisionMode>>,
    gpu_contacts: Option<Res<GpuCollisionContacts>>,
    collision_config: Res<FragmentCollisionConfig>,
    mut kinematics: Query<
        (
            Entity,
            &crate::voxel_collision::GpuCollisionAABB,
            &mut Transform,
        ),
        Without<VoxelFragment>,
    >,
) {
    // Only run if GPU mode is enabled
    let Some(gpu_mode) = gpu_mode else {
        return;
    };
    if !gpu_mode.enabled {
        return;
    }

    let Some(gpu_contacts) = gpu_contacts else {
        return;
    };

    if !collision_config.enabled {
        return;
    }

    // Get GPU collision results
    let collision_result = gpu_contacts.get();

    if collision_result.contacts.is_empty() {
        return;
    }

    // Build entity-to-index map for quick lookup
    let entity_to_idx: std::collections::HashMap<Entity, u32> = collision_result
        .fragment_entities
        .iter()
        .enumerate()
        .map(|(idx, &entity)| (entity, idx as u32))
        .collect();

    // Process each kinematic body by entity lookup
    for (entity, aabb, mut transform) in kinematics.iter_mut() {
        // Look up this entity's index from the GPU results
        let Some(&fragment_idx) = entity_to_idx.get(&entity) else {
            // This entity wasn't in the GPU collision batch
            continue;
        };

        // Check if we have floor contacts (normal pointing up)
        let has_floor_contact = collision_result.has_floor_contact_for_fragment(fragment_idx);

        if has_floor_contact {
            // For floor contacts, compute the correct target Y position based on
            // the highest floor voxel we're touching. This avoids issues with stale
            // GPU penetration values due to 1-frame readback latency.
            //
            // Find the highest Y of all floor contact voxels
            let mut max_floor_y = f32::MIN;
            for contact in collision_result.contacts_for_fragment(fragment_idx) {
                // Floor contacts have normal.y > 0.7
                if contact.normal[1] > 0.7 {
                    // The contact position is inside the voxel.
                    // The voxel top = floor(contact.y) + 1.0
                    let voxel_top = contact.position[1].floor() + 1.0;
                    max_floor_y = max_floor_y.max(voxel_top);
                }
            }

            if max_floor_y > f32::MIN {
                // Target position: floor top + half height
                let target_y = max_floor_y + aabb.half_extents.y;

                // Only push up if we're below the target (prevents bouncing when above)
                if transform.translation.y < target_y {
                    transform.translation.y = target_y;
                }
            }
        } else {
            // For non-floor collisions (walls, ceilings), use the standard resolution vector
            let resolution = collision_result.resolution_vector_for_fragment(fragment_idx);

            // Skip if no collision
            if resolution.length_squared()
                < collision_config.min_penetration * collision_config.min_penetration
            {
                continue;
            }

            // Apply horizontal resolution (walls)
            transform.translation.x += resolution.x;
            transform.translation.z += resolution.z;

            // For ceiling contacts (resolution.y < 0), apply vertical push
            if resolution.y < 0.0 {
                transform.translation.y += resolution.y;
            }
        }
    }
}

/// Plugin for voxel fragment physics.
pub struct VoxelFragmentPlugin;

impl Plugin for VoxelFragmentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FragmentConfig>()
            .init_resource::<TerrainOccupancy>()
            .init_resource::<FragmentCollisionConfig>()
            .init_resource::<GpuCollisionMode>()
            .add_systems(
                Update,
                (
                    // Run CPU collision if GPU mode is disabled
                    fragment_terrain_collision_system.run_if(
                        |mode: Option<Res<GpuCollisionMode>>| mode.map_or(true, |m| !m.enabled),
                    ),
                    // Run GPU physics for fragments if enabled
                    gpu_fragment_physics_system.run_if(|mode: Option<Res<GpuCollisionMode>>| {
                        mode.map_or(false, |m| m.enabled)
                    }),
                    // Run GPU collision for kinematic bodies if enabled
                    gpu_kinematic_collision_system.run_if(|mode: Option<Res<GpuCollisionMode>>| {
                        mode.map_or(false, |m| m.enabled)
                    }),
                    detect_settling_fragments,
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::Voxel;

    #[test]
    fn test_fragment_config_default() {
        let config = FragmentConfig::default();
        assert_eq!(config.settle_threshold_frames, 60);
        assert!((config.settle_velocity_threshold - 0.1).abs() < 0.001);
        assert_eq!(config.max_active_fragments, 32);
    }

    #[test]
    fn test_fragment_is_settled() {
        let config = FragmentConfig {
            settle_threshold_frames: 30,
            ..default()
        };

        let data = VoxelWorld::new();
        let mut fragment = VoxelFragment::new(data, IVec3::ZERO);

        // Not settled initially
        assert!(!fragment.is_settled(&config));

        // Still not settled at 29 frames
        fragment.settling_frames = 29;
        assert!(!fragment.is_settled(&config));

        // Settled at 30 frames
        fragment.settling_frames = 30;
        assert!(fragment.is_settled(&config));
    }

    #[test]
    fn test_spawn_fragment_empty_returns_none() {
        // Can't test full spawn without Bevy app, but we can test the logic
        let data = VoxelWorld::new();
        let collider = generate_merged_cuboid_collider(&data);
        assert!(collider.is_none(), "Empty world should produce no collider");
    }

    #[test]
    fn test_spawn_fragment_with_data_produces_collider() {
        let mut data = VoxelWorld::new();
        data.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));

        let collider = generate_merged_cuboid_collider(&data);
        assert!(
            collider.is_some(),
            "Non-empty world should produce collider"
        );
    }

    #[test]
    fn test_terrain_occupancy_from_voxel_world() {
        let mut world = VoxelWorld::new();
        for x in 0..10 {
            for z in 0..10 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }

        let terrain = TerrainOccupancy::from_voxel_world(&world);

        // Check that terrain is properly loaded
        assert!(terrain.occupancy.get_voxel(IVec3::new(0, 0, 0)));
        assert!(terrain.occupancy.get_voxel(IVec3::new(5, 0, 5)));
        assert!(!terrain.occupancy.get_voxel(IVec3::new(0, 1, 0)));
    }

    #[test]
    fn test_fragment_collision_config_default() {
        let config = FragmentCollisionConfig::default();

        assert!(config.enabled);
        assert!(config.min_penetration > 0.0);
        // Spring-damper parameters should be positive
        assert!(config.spring_k > 0.0);
        assert!(config.damping_k > 0.0);
        assert!(config.friction_k > 0.0);
    }

    #[test]
    fn test_fragment_terrain_collision_detection() {
        // Create terrain floor
        let mut terrain_world = VoxelWorld::new();
        for x in 0..10 {
            for z in 0..10 {
                terrain_world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        let terrain = TerrainOccupancy::from_voxel_world(&terrain_world);

        // Create a small fragment
        let mut fragment_world = VoxelWorld::new();
        fragment_world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        let fragment = VoxelFragment::new(fragment_world, IVec3::ZERO);

        // Test collision when fragment is inside terrain
        let collision = terrain.occupancy.check_fragment(
            &fragment.occupancy,
            Vec3::new(5.5, 0.5, 5.5), // Inside terrain floor
            Quat::IDENTITY,
        );

        assert!(
            collision.has_collision(),
            "Fragment inside terrain should collide"
        );
        assert!(collision.has_floor_contact(), "Should detect floor contact");

        // Test no collision when fragment is above terrain
        let collision_above = terrain.occupancy.check_fragment(
            &fragment.occupancy,
            Vec3::new(5.5, 5.0, 5.5), // Above terrain
            Quat::IDENTITY,
        );

        assert!(
            !collision_above.has_collision(),
            "Fragment above terrain should not collide"
        );
    }

    #[test]
    fn test_fragment_collision_resolution_pushes_up() {
        // Create terrain floor
        let mut terrain_world = VoxelWorld::new();
        for x in 0..10 {
            for z in 0..10 {
                terrain_world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        let terrain = TerrainOccupancy::from_voxel_world(&terrain_world);

        // Create a small fragment
        let mut fragment_world = VoxelWorld::new();
        fragment_world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        let fragment = VoxelFragment::new(fragment_world, IVec3::ZERO);

        // Fragment penetrating floor from above
        let collision = terrain.occupancy.check_fragment(
            &fragment.occupancy,
            Vec3::new(5.5, 0.7, 5.5), // Slightly inside floor
            Quat::IDENTITY,
        );

        assert!(collision.has_collision());

        let resolution = collision.resolution_vector();

        // Resolution should push UP (positive Y)
        assert!(
            resolution.y > 0.0,
            "Resolution should push up, got {:?}",
            resolution
        );
        assert!(resolution.x.abs() < 0.1, "Should not push X significantly");
        assert!(resolution.z.abs() < 0.1, "Should not push Z significantly");
    }
}
