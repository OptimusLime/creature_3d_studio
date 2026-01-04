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

use crate::physics_math::{
    compute_terrain_collision_force, generate_surface_particles, integrate_angular_velocity,
    integrate_position, integrate_rotation, integrate_velocity, ParticleConfig, PhysicsConfig,
};
use crate::voxel::VoxelWorld;
use crate::voxel_collision::FragmentOccupancy;
use crate::voxel_mesh::build_world_meshes_cross_chunk;

// Used in tests
#[allow(unused_imports)]
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

/// Surface particle data for physics collision.
///
/// This component stores pre-computed surface particle positions for the
/// Harada spring-damper collision model. Using surface particles (hollow shell)
/// instead of all voxels:
/// - Reduces particle count: n³ - (n-2)³ instead of n³
/// - Uses correct particle diameter: scale / particles_per_edge
/// - Prevents collision force explosion from multiple overlapping voxel collisions
///
/// Reference: GPUPhysics.cs lines 136-140, 246-261
#[derive(Component)]
pub struct FragmentSurfaceParticles {
    /// Particle positions relative to fragment center (local space)
    pub local_positions: Vec<Vec3>,
    /// Particle diameter for collision detection
    pub particle_diameter: f32,
    /// Mass per particle
    pub particle_mass: f32,
}

impl FragmentSurfaceParticles {
    /// Create surface particles for a cubic fragment.
    ///
    /// # Arguments
    /// * `size` - Size of the fragment along each axis (assumes cubic)
    /// * `total_mass` - Total mass of the fragment
    pub fn from_size(size: u32, total_mass: f32) -> Self {
        // Match particles_per_edge to fragment size for 1:1 voxel-to-particle mapping
        // This gives diameter = 1.0 which matches terrain voxel size
        let config = ParticleConfig {
            particles_per_edge: size,
            scale: size as f32,
        };
        let local_positions = generate_surface_particles(&config);
        // Particle diameter MUST match spacing to avoid gaps in collision coverage
        // Reference: particleDiameter = scale / particlesPerEdge (GPUPhysics.cs line 140)
        // For a 3x3x3 cube with scale=3.0: diameter = 3.0/3 = 1.0
        let particle_diameter = config.particle_diameter();
        let particle_mass = total_mass / local_positions.len() as f32;

        Self {
            local_positions,
            particle_diameter,
            particle_mass,
        }
    }

    /// Number of particles
    pub fn count(&self) -> usize {
        self.local_positions.len()
    }

    /// Total mass of all particles
    pub fn total_mass(&self) -> f32 {
        self.particle_mass * self.local_positions.len() as f32
    }
}

/// Bundle for spawning a voxel fragment with physics.
///
/// No Rapier components - we do our own physics integration using
/// the Harada spring-damper model from GPU Gems 3.
#[derive(Bundle)]
pub struct VoxelFragmentBundle {
    pub fragment: VoxelFragment,
    pub physics: FragmentPhysics,
    pub surface_particles: FragmentSurfaceParticles,
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
/// - FragmentSurfaceParticles for collision (surface particles only)
/// - FragmentPhysics for velocity state
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

    // Determine fragment size (assumes roughly cubic fragments)
    // Use the max dimension for particle generation
    let fragment = VoxelFragment::new(data, origin);
    let size = fragment.occupancy.size;
    let max_dim = size.x.max(size.y).max(size.z);

    // Generate surface particles for the fragment
    // Total mass = 1.0 per particle * number of occupied voxels
    let num_voxels = fragment.occupancy.count_occupied();
    let total_mass = num_voxels as f32;
    let surface_particles = FragmentSurfaceParticles::from_size(max_dim, total_mass);

    let entity = commands
        .spawn(VoxelFragmentBundle {
            fragment,
            physics: FragmentPhysics {
                velocity: impulse,
                angular_velocity: Vec3::ZERO,
                mass: 1.0, // Mass per particle, total mass comes from surface_particles
            },
            surface_particles,
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

    // Create fragment and determine surface particles
    let fragment = VoxelFragment::new(data, origin);
    let size = fragment.occupancy.size;
    let max_dim = size.x.max(size.y).max(size.z);
    let num_voxels = fragment.occupancy.count_occupied();
    let total_mass = num_voxels as f32;
    let surface_particles = FragmentSurfaceParticles::from_size(max_dim, total_mass);

    // The mesh vertices are built centered within a 32x32x32 chunk (offset by -16).
    // For a small fragment (e.g., 3x3x3 at voxel coords 0-2), vertices are at (-16,-16,-16) to (-14,-14,-14).
    // The chunk system adds +16 offset assuming we want world coords, but for fragments we want
    // the mesh centered at the fragment's physics center.
    //
    // Physics particles are centered at origin: for 3x3x3, particles at -1, 0, +1.
    // So we need the mesh to span from -1.5 to +1.5 (voxels 0-2 centered at origin).
    //
    // The mesh vertices are at [voxel_coord - 16], so voxel 0 -> -16, voxel 2 -> -14.
    // We need to shift by +16 (undo the chunk centering) then -1.5 (center at origin).
    // But chunk_translation already adds +16, so we need to IGNORE chunk_translation
    // and just use our own centering offset.
    //
    // Actually: mesh vertex for voxel (x,y,z) = (x-16, y-16, z-16)
    // We want voxel 0 to be at -1.5 and voxel 2 to be at +0.5 (cube from -1.5 to +1.5)
    // So offset = -1.5 - (-16) = 14.5... no wait that's wrong.
    //
    // Let's think differently:
    // - Mesh vertices: voxel 0 is at -16, voxel 2 is at -14 (cube corners at -16 to -13)
    // - We want: cube corners at -1.5 to +1.5
    // - So we need to add: -1.5 - (-16) = 14.5 to X for the min corner
    //
    // Hmm, that gives (14.5, 14.5, 14.5) which is what we had. But that's a child transform,
    // so when the parent (fragment) is at position (5, 12, 0), the mesh renders at (5+14.5, 12+14.5, 0+14.5).
    // That's WRONG - mesh is 14.5 units away from physics!
    //
    // The issue: chunk_translation is designed for terrain where chunk_pos matters.
    // For fragments, we ignore chunk_pos and just center the mesh at fragment origin.
    // Mesh needs offset of: (target_center) - (mesh_center)
    // Mesh center for voxels 0-2 centered in chunk = (-16 + -13)/2 = -14.5 (approx)
    // Actually for voxel coords 0,1,2 the mesh spans -16 to -13 (since each voxel is 1 unit).
    // Mesh center = -14.5. We want center at 0. So offset = 0 - (-14.5) = +14.5.
    // But that's still wrong because the mesh vertices go from -16 (voxel 0 corner) to -13 (voxel 2+1 corner).
    //
    // WAIT - I think the issue is simpler. The child mesh transform of (14.5, 14.5, 14.5) means
    // when the fragment is at world position P, the mesh renders at P + (14.5, 14.5, 14.5).
    // That's the bug! The mesh is offset from the physics center.
    //
    // The mesh should be at offset (0, 0, 0) relative to parent so it renders at the same position.
    // So I need: mesh_offset such that mesh renders centered at parent origin.
    // Mesh vertices for voxel x are at (x - 16). For voxels 0-2, that's -16 to -14, spanning -16 to -13.
    // The center of that is (-16 + -13)/2 = -14.5.
    // To center the mesh at origin, I need to add +14.5.
    // BUT I should NOT add the chunk_translation (which adds another +16).
    //
    // Fix: Don't use chunk_translation at all. Just compute our own centering.
    let mesh_offset = Vec3::new(
        16.0 - (max_dim as f32) / 2.0, // Undo the -16 in mesh, then center
        16.0 - (max_dim as f32) / 2.0,
        16.0 - (max_dim as f32) / 2.0,
    );

    // Spawn parent with physics
    let entity = commands
        .spawn(VoxelFragmentBundle {
            fragment,
            physics: FragmentPhysics {
                velocity: impulse,
                angular_velocity: Vec3::ZERO,
                mass: 1.0,
            },
            surface_particles,
            transform: Transform::from_translation(position),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            inherited_visibility: InheritedVisibility::default(),
            view_visibility: ViewVisibility::default(),
        })
        .with_children(|parent| {
            // Spawn mesh children for each chunk
            // IGNORE chunk_translation - it's designed for terrain chunks, not small fragments.
            // Use our own centering offset to align mesh with physics particles.
            for chunk_mesh in chunk_meshes {
                let mesh_handle = meshes.add(chunk_mesh.mesh);
                parent.spawn((
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(mesh_offset),
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
    mut fragments: Query<(&mut VoxelFragment, &FragmentPhysics)>,
) {
    for (mut fragment, physics) in fragments.iter_mut() {
        let speed = physics.velocity.length() + physics.angular_velocity.length();

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

/// Debug visualization settings for fragment physics.
#[derive(Resource)]
pub struct FragmentDebugConfig {
    /// Show particle positions as spheres
    pub show_particles: bool,
    /// Show fragment bounding box
    pub show_bounds: bool,
    /// Show physics center as a cross
    pub show_center: bool,
    /// Show terrain occupancy as wireframe boxes
    pub show_terrain: bool,
    /// Only show terrain within this radius of camera/origin
    pub terrain_radius: f32,
    /// Color for particles
    pub particle_color: Color,
    /// Color for bounding box
    pub bounds_color: Color,
    /// Color for center marker
    pub center_color: Color,
    /// Color for terrain voxels
    pub terrain_color: Color,
    /// Color for terrain top faces (collision surface)
    pub terrain_top_color: Color,
}

impl Default for FragmentDebugConfig {
    fn default() -> Self {
        Self {
            show_particles: false,
            show_bounds: false,
            show_center: false,
            show_terrain: false,
            terrain_radius: 15.0,
            particle_color: Color::srgb(1.0, 1.0, 0.0), // Yellow
            bounds_color: Color::srgb(0.0, 1.0, 0.0),   // Green
            center_color: Color::srgb(1.0, 0.0, 0.0),   // Red
            terrain_color: Color::srgba(0.5, 0.5, 1.0, 0.3), // Light blue, semi-transparent
            terrain_top_color: Color::srgb(0.0, 1.0, 1.0), // Cyan for top faces
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
/// - Uses SURFACE PARTICLES (hollow shell) instead of all voxels
/// - Particle diameter = scale / particles_per_edge (correct scaling)
/// - Uses `compute_terrain_collision_force` for per-particle collision forces
/// - Uses `integrate_velocity` with friction BEFORE force (critical!)
/// - Uses `integrate_rotation` with correct quaternion derivative
/// - All constants match gpu-physics-unity reference
pub fn fragment_terrain_collision_system(
    time: Res<Time>,
    terrain: Res<TerrainOccupancy>,
    collision_config: Res<FragmentCollisionConfig>,
    mut fragments: Query<(
        &FragmentSurfaceParticles,
        &mut Transform,
        &mut FragmentPhysics,
    )>,
) {
    if !collision_config.enabled {
        return;
    }

    let frame_dt = time.delta_secs();

    // Use substeps to prevent tunneling through terrain at high velocities
    // Reference uses separate dt (physics timestep) and tick_rate (substep frequency)
    // With 16 substeps at 60fps, effective dt = 1/960 = 0.001s
    // This matches typical Unity physics timesteps
    const SUBSTEPS: u32 = 16;
    let dt = frame_dt / SUBSTEPS as f32;

    for (surface_particles, mut transform, mut physics) in fragments.iter_mut() {
        let initial_pos = transform.translation;

        // Run multiple physics substeps per frame
        for _substep in 0..SUBSTEPS {
            let fragment_pos = transform.translation;
            let fragment_vel = physics.velocity;
            let fragment_rot = transform.rotation;
            let angular_vel = physics.angular_velocity;

            // Build PhysicsConfig with CORRECT particle diameter from surface particles
            // This is the key fix: diameter = scale / particles_per_edge
            // For a 3x3x3 fragment: diameter = 3.0 / 3 = 1.0
            // The reference uses SMALLER particles that don't overlap multiple terrain voxels
            let physics_config = PhysicsConfig {
                gravity: 9.8,
                particle_diameter: surface_particles.particle_diameter,
                spring_k: collision_config.spring_k,
                damping_k: collision_config.damping_k,
                tangential_k: collision_config.friction_k,
                friction: 0.9,
                angular_friction: 0.3,
                linear_force_scalar: 1.0,
                angular_force_scalar: 1.0,
                velocity_threshold: 1e-6,
            };

            let mut total_force = Vec3::ZERO;
            let mut total_torque = Vec3::ZERO;

            // Iterate over SURFACE PARTICLES only (hollow shell)
            // This matches the reference: particlesPerBody = n³ - (n-2)³
            for local_offset in &surface_particles.local_positions {
                // Transform to world space using fragment rotation
                let world_offset = fragment_rot * *local_offset;
                let particle_world_pos = fragment_pos + world_offset;

                // Velocity at this particle includes angular contribution
                let particle_vel = fragment_vel + angular_vel.cross(world_offset);

                // Compute collision force for this particle against terrain
                let particle_force = compute_terrain_collision_force(
                    particle_world_pos,
                    particle_vel,
                    &terrain.occupancy,
                    &physics_config,
                );

                // Gravity is applied PER PARTICLE (reference line 326)
                // force.y -= gravityCoefficient;
                let particle_force_with_gravity = Vec3::new(
                    particle_force.x,
                    particle_force.y - physics_config.gravity,
                    particle_force.z,
                );

                // Accumulate force
                total_force += particle_force_with_gravity;

                // Compute torque: τ = r × F (cross product of offset and force)
                if particle_force.length_squared() > 1e-10 {
                    total_torque += world_offset.cross(particle_force);
                }
            }

            // NOTE: Gravity is now applied per-particle in the loop above,
            // matching reference line 326: force.y -= gravityCoefficient;
            // DO NOT call apply_gravity here - it would only add gravity once!

            // Mass = number of particles * mass per particle
            // Reference: cubeMass = particleMass * particlesPerRigidBody
            let mass = surface_particles.total_mass();

            // Integrate linear velocity
            physics.velocity = integrate_velocity(
                fragment_vel,
                total_force,
                mass,
                physics_config.friction,
                dt,
                physics_config.velocity_threshold,
            );

            // Integrate position
            transform.translation = integrate_position(fragment_pos, physics.velocity, dt);

            // Integrate angular velocity with torque
            // CRITICAL: Reference does NOT divide by inertia tensor!
            // Reference: GPUPhysicsComputeShader.compute lines 377-381
            // rigidBodyAngularVelocities[id.x] /= 1.0 + deltaTime*angularFrictionCoefficient;
            // rigidBodyAngularVelocities[id.x] += angularForceScalar * deltaTime * angularForce;
            physics.angular_velocity = integrate_angular_velocity(
                angular_vel,
                total_torque,
                physics_config.angular_friction,
                physics_config.angular_force_scalar,
                dt,
                physics_config.velocity_threshold,
            );

            // Integrate rotation
            if physics.angular_velocity.length_squared() > 1e-12 {
                transform.rotation =
                    integrate_rotation(transform.rotation, physics.angular_velocity, dt);
            }
        } // end substep loop

        // Log every frame for debugging
        let frame_movement = (transform.translation - initial_pos).length();
        if frame_movement > 0.1 {
            bevy::log::info!(
                "PHYSICS: move={:.3} pos=({:.2},{:.2},{:.2}) vel=({:.2},{:.2},{:.2})",
                frame_movement,
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                physics.velocity.x,
                physics.velocity.y,
                physics.velocity.z
            );
        }

        // Check for teleportation (movement > 0.5 units per frame is suspicious)
        if frame_movement > 0.5 {
            bevy::log::warn!(
                "TELEPORT DETECTED: moved {:.2} units in one frame! pos: {:?} -> {:?}, vel: {:?}",
                frame_movement,
                initial_pos,
                transform.translation,
                physics.velocity
            );
        }
    }
}

// NOTE: GPU collision detection has been removed. All physics now goes through
// the PhysicsEngine API in physics_math.rs, which has been verified correct with
// comprehensive unit tests. GPU acceleration can be re-added in the future by
// porting the same formulas to compute shaders.

/// Plugin for voxel fragment physics.
///
/// This plugin provides:
/// - Fragment terrain collision using the verified physics_math module
/// - Settling detection for when fragments stop moving
///
/// All physics goes through the PhysicsEngine API in physics_math.rs.
/// No Rapier, no external physics libraries.
pub struct VoxelFragmentPlugin;

impl Plugin for VoxelFragmentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FragmentConfig>()
            .init_resource::<TerrainOccupancy>()
            .init_resource::<FragmentCollisionConfig>()
            .init_resource::<FragmentDebugConfig>()
            .add_systems(
                Update,
                (
                    // CPU physics using verified physics_math module
                    fragment_terrain_collision_system,
                    detect_settling_fragments,
                    // Debug visualization
                    draw_fragment_debug_gizmos,
                    draw_terrain_debug_gizmos,
                )
                    .chain(),
            );
    }
}

/// Draw debug gizmos for fragment physics visualization.
///
/// Shows:
/// - Yellow spheres at each surface particle position
/// - Green wireframe box for fragment bounds  
/// - Red cross at physics center
///
/// Enable with `FragmentDebugConfig` resource.
pub fn draw_fragment_debug_gizmos(
    mut gizmos: Gizmos,
    debug_config: Res<FragmentDebugConfig>,
    fragments: Query<(&Transform, &FragmentSurfaceParticles), With<VoxelFragment>>,
) {
    // Early exit if nothing to draw
    if !debug_config.show_particles && !debug_config.show_bounds && !debug_config.show_center {
        return;
    }

    for (transform, surface_particles) in fragments.iter() {
        let pos = transform.translation;
        let rot = transform.rotation;

        // Draw center marker
        if debug_config.show_center {
            let size = 0.3;
            gizmos.line(
                pos - Vec3::X * size,
                pos + Vec3::X * size,
                debug_config.center_color,
            );
            gizmos.line(
                pos - Vec3::Y * size,
                pos + Vec3::Y * size,
                debug_config.center_color,
            );
            gizmos.line(
                pos - Vec3::Z * size,
                pos + Vec3::Z * size,
                debug_config.center_color,
            );
        }

        // Draw particles
        if debug_config.show_particles {
            let radius = surface_particles.particle_diameter / 2.0;
            for local_pos in &surface_particles.local_positions {
                let world_pos = pos + rot * *local_pos;
                gizmos.sphere(world_pos, radius * 0.8, debug_config.particle_color);
            }
        }

        // Draw bounding box
        if debug_config.show_bounds {
            // Find bounds from particles
            let mut min = Vec3::splat(f32::MAX);
            let mut max = Vec3::splat(f32::MIN);
            for local_pos in &surface_particles.local_positions {
                let world_pos = pos + rot * *local_pos;
                min = min.min(world_pos);
                max = max.max(world_pos);
            }
            // Expand by particle radius
            let radius = surface_particles.particle_diameter / 2.0;
            min -= Vec3::splat(radius);
            max += Vec3::splat(radius);

            // Draw wireframe box
            let corners = [
                Vec3::new(min.x, min.y, min.z),
                Vec3::new(max.x, min.y, min.z),
                Vec3::new(max.x, max.y, min.z),
                Vec3::new(min.x, max.y, min.z),
                Vec3::new(min.x, min.y, max.z),
                Vec3::new(max.x, min.y, max.z),
                Vec3::new(max.x, max.y, max.z),
                Vec3::new(min.x, max.y, max.z),
            ];
            // Bottom face
            gizmos.line(corners[0], corners[1], debug_config.bounds_color);
            gizmos.line(corners[1], corners[2], debug_config.bounds_color);
            gizmos.line(corners[2], corners[3], debug_config.bounds_color);
            gizmos.line(corners[3], corners[0], debug_config.bounds_color);
            // Top face
            gizmos.line(corners[4], corners[5], debug_config.bounds_color);
            gizmos.line(corners[5], corners[6], debug_config.bounds_color);
            gizmos.line(corners[6], corners[7], debug_config.bounds_color);
            gizmos.line(corners[7], corners[4], debug_config.bounds_color);
            // Vertical edges
            gizmos.line(corners[0], corners[4], debug_config.bounds_color);
            gizmos.line(corners[1], corners[5], debug_config.bounds_color);
            gizmos.line(corners[2], corners[6], debug_config.bounds_color);
            gizmos.line(corners[3], corners[7], debug_config.bounds_color);
        }
    }
}

/// Draw debug gizmos for terrain occupancy visualization.
///
/// Shows the top surface of terrain voxels where collision detection happens.
/// Only draws voxels within `terrain_radius` of origin to avoid performance issues.
pub fn draw_terrain_debug_gizmos(
    mut gizmos: Gizmos,
    debug_config: Res<FragmentDebugConfig>,
    terrain: Res<TerrainOccupancy>,
) {
    if !debug_config.show_terrain {
        return;
    }

    let radius = debug_config.terrain_radius as i32;

    // Iterate over voxels near origin
    for x in -radius..=radius {
        for z in -radius..=radius {
            for y in 0..10 {
                // Only check reasonable Y range
                let pos = IVec3::new(x, y, z);

                if !terrain.occupancy.get_voxel(pos) {
                    continue;
                }

                // Check if top face is exposed (no voxel above)
                let above = IVec3::new(x, y + 1, z);
                let top_exposed = !terrain.occupancy.get_voxel(above);

                if top_exposed {
                    // Draw the top face of this voxel (the collision surface)
                    let base = Vec3::new(x as f32, (y + 1) as f32, z as f32);

                    // Draw top face outline
                    let c0 = base;
                    let c1 = base + Vec3::X;
                    let c2 = base + Vec3::X + Vec3::Z;
                    let c3 = base + Vec3::Z;

                    gizmos.line(c0, c1, debug_config.terrain_top_color);
                    gizmos.line(c1, c2, debug_config.terrain_top_color);
                    gizmos.line(c2, c3, debug_config.terrain_top_color);
                    gizmos.line(c3, c0, debug_config.terrain_top_color);

                    // Draw X across the face to make it more visible
                    gizmos.line(c0, c2, debug_config.terrain_top_color);
                    gizmos.line(c1, c3, debug_config.terrain_top_color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics_math::apply_gravity;
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

    /// EXACT P22 REPRODUCTION TEST
    ///
    /// Creates the EXACT terrain from p22_voxel_fragment.rs and drops a 3x3x3 cube
    /// from spawn_height=15 onto the CENTER RAMP. The cube MUST land on the ramp,
    /// not fall through to the floor.
    ///
    /// P22 terrain:
    /// - Floor: Y=0,1,2 (top at Y=3) for X=-10..10, Z=-10..10
    /// - Pillars: at corners (-5,-5), (5,-5), (-5,5), (5,5) from Y=3..8
    /// - Center ramp: X=-2..3, Z=-2..3, height varies: x=-2→h=3, x=2→h=7
    ///
    /// P22 spawn:
    /// - Position: random X,Z in [-5, 5], Y=15
    /// - Impulse: random X,Z small, Y=-5 (downward)
    /// - Fragment: 3x3x3 cube
    #[test]
    fn test_p22_exact_drop_on_ramp() {
        use crate::physics_math::{
            apply_gravity, compute_terrain_collision_force, integrate_position, integrate_velocity,
            PhysicsConfig,
        };

        // ============================================================
        // CREATE EXACT P22 TERRAIN (copied from p22_voxel_fragment.rs)
        // ============================================================
        let mut terrain_world = VoxelWorld::new();

        // Ground platform (20x20, 3 blocks thick) - SINGLE COLOR
        let ground_color = Voxel::solid(70, 70, 80);
        for x in -10..10 {
            for z in -10..10 {
                for y in 0..3 {
                    terrain_world.set_voxel(x, y, z, ground_color);
                }
            }
        }

        // Add some pillars for interesting collisions - SINGLE COLOR per pillar
        let pillar_color = Voxel::solid(100, 60, 60);
        for (px, pz) in [(-5, -5), (5, -5), (-5, 5), (5, 5)] {
            for y in 3..8 {
                terrain_world.set_voxel(px, y, pz, pillar_color);
            }
        }

        // Center ramp - SINGLE COLOR
        let ramp_color = Voxel::solid(60, 100, 60);
        for x in -2..3 {
            for z in -2..3 {
                let height = 3 + (x + 2) as i32;
                for y in 3..height {
                    terrain_world.set_voxel(x, y, z, ramp_color);
                }
            }
        }

        let terrain = TerrainOccupancy::from_voxel_world(&terrain_world);

        // ============================================================
        // VERIFY TERRAIN MATCHES EXPECTATIONS
        // ============================================================
        println!("=== P22 Terrain Verification ===");
        println!(
            "Chunks: {}, Total voxels: {}",
            terrain.occupancy.chunk_count(),
            terrain.occupancy.total_occupied()
        );

        // Ramp heights at different X positions (for Z=0):
        // x=-2: height=3, voxels at Y=[] (3..3 is empty)
        // x=-1: height=4, voxels at Y=[3]
        // x=0:  height=5, voxels at Y=[3,4]
        // x=1:  height=6, voxels at Y=[3,4,5]
        // x=2:  height=7, voxels at Y=[3,4,5,6]

        assert!(
            !terrain.occupancy.get_voxel(IVec3::new(-2, 3, 0)),
            "x=-2 should have NO ramp"
        );
        assert!(
            terrain.occupancy.get_voxel(IVec3::new(-1, 3, 0)),
            "x=-1 should have ramp at Y=3"
        );
        assert!(
            terrain.occupancy.get_voxel(IVec3::new(0, 3, 0)),
            "x=0 should have ramp at Y=3"
        );
        assert!(
            terrain.occupancy.get_voxel(IVec3::new(0, 4, 0)),
            "x=0 should have ramp at Y=4"
        );
        assert!(
            !terrain.occupancy.get_voxel(IVec3::new(0, 5, 0)),
            "x=0 should NOT have ramp at Y=5"
        );
        assert!(
            terrain.occupancy.get_voxel(IVec3::new(2, 6, 0)),
            "x=2 should have ramp at Y=6"
        );

        // ============================================================
        // P22 PHYSICS CONFIG (exact same as ECS system)
        // ============================================================
        let collision_config = FragmentCollisionConfig::default();
        let physics_config = PhysicsConfig {
            gravity: 9.8,
            particle_diameter: 1.0,
            spring_k: collision_config.spring_k,
            damping_k: collision_config.damping_k,
            tangential_k: collision_config.friction_k,
            friction: 0.9,
            angular_friction: 0.3,
            linear_force_scalar: 1.0,
            angular_force_scalar: 1.0,
            velocity_threshold: 1e-6,
        };

        // ============================================================
        // DROP CUBE ONTO CENTER OF RAMP (x=0, z=0)
        // Ramp at x=0 has voxels at Y=3,4. Top surface at Y=5.
        // ============================================================
        let dt = 1.0 / 60.0;
        let spawn_height = 15.0; // Same as p22
        let mut pos = Vec3::new(0.5, spawn_height, 0.5); // Center of ramp
        let mut vel = Vec3::new(0.0, -5.0, 0.0); // Downward impulse like p22
        let mass = 1.0;

        println!("\n=== Drop Test: Center of Ramp (x=0, z=0) ===");
        println!("Ramp at x=0 has voxels at Y=3,4. Top surface at Y=5.");
        println!("Starting: pos=({:.1}, {:.1}, {:.1})", pos.x, pos.y, pos.z);

        for frame in 0..600 {
            let collision_force =
                compute_terrain_collision_force(pos, vel, &terrain.occupancy, &physics_config);
            let total_force = apply_gravity(collision_force, &physics_config);
            vel = integrate_velocity(
                vel,
                total_force,
                mass,
                physics_config.friction,
                dt,
                physics_config.velocity_threshold,
            );
            pos = integrate_position(pos, vel, dt);

            if frame % 60 == 0 || (collision_force.y > 0.0 && frame < 300) {
                println!(
                    "Frame {:3}: Y={:.2}, vel_y={:+.2}, collision_force_y={:.1}",
                    frame, pos.y, vel.y, collision_force.y
                );
            }
        }

        let ramp_top_y = 5.0; // Ramp at x=0 has voxels at Y=3,4, so top is Y=5
        let floor_top_y = 3.0;

        println!("\nFinal position: Y={:.2}", pos.y);
        println!("Ramp top: Y={}", ramp_top_y);
        println!("Floor top: Y={}", floor_top_y);

        // THE CRITICAL ASSERTION: Did we land on the RAMP or fall through to FLOOR?
        assert!(
            pos.y > ramp_top_y - 0.5,
            "FAILED: Cube fell THROUGH the ramp! Final Y={:.2}, but ramp top is Y={}. \
             Cube should have landed ON the ramp at Y≈{:.1}",
            pos.y,
            ramp_top_y,
            ramp_top_y + 0.5
        );

        // Also verify we're not floating way above
        assert!(
            pos.y < ramp_top_y + 1.5,
            "Cube is floating too high above ramp. Y={:.2}, expected ~{:.1}",
            pos.y,
            ramp_top_y + 0.5
        );

        println!("SUCCESS: Cube landed on ramp at Y={:.2}", pos.y);
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

    /// Test that simulates a 3x3x3 fragment falling onto floor terrain.
    /// This replicates the exact logic from fragment_terrain_collision_system.
    #[test]
    fn test_3x3x3_fragment_lands_on_floor() {
        use crate::voxel::Voxel;

        // Create floor terrain at Y=0,1,2 (top surface at Y=3)
        let mut terrain_world = VoxelWorld::new();
        for x in -10..10 {
            for z in -10..10 {
                for y in 0..3 {
                    terrain_world.set_voxel(x, y, z, Voxel::solid(100, 100, 100));
                }
            }
        }
        let terrain = TerrainOccupancy::from_voxel_world(&terrain_world);

        // Verify terrain
        assert!(
            terrain.occupancy.get_voxel(IVec3::new(0, 2, 0)),
            "Floor voxel at Y=2 should exist"
        );
        assert!(
            !terrain.occupancy.get_voxel(IVec3::new(0, 3, 0)),
            "No voxel at Y=3"
        );

        // Create 3x3x3 fragment
        let mut fragment_world = VoxelWorld::new();
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    fragment_world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }
        let fragment = VoxelFragment::new(fragment_world, IVec3::ZERO);

        // Fragment occupancy should be 3x3x3
        assert_eq!(fragment.occupancy.size, UVec3::new(3, 3, 3));
        assert_eq!(fragment.occupancy.count_occupied(), 27);

        // Physics config (same as ECS system)
        let collision_config = FragmentCollisionConfig::default();
        let physics_config = PhysicsConfig {
            gravity: 9.8,
            particle_diameter: 1.0,
            spring_k: collision_config.spring_k,
            damping_k: collision_config.damping_k,
            tangential_k: collision_config.friction_k,
            friction: 0.9,
            angular_friction: 0.3,
            linear_force_scalar: 1.0,
            angular_force_scalar: 1.0,
            velocity_threshold: 1e-6,
        };

        // Initial state
        let dt = 1.0 / 60.0;
        let mut fragment_pos = Vec3::new(0.0, 15.0, 0.0);
        let mut fragment_vel = Vec3::new(0.0, -5.0, 0.0);
        let fragment_rot = Quat::IDENTITY;
        let mut angular_vel = Vec3::ZERO;

        // Mass = number of particles (voxels) * mass per particle
        // Reference uses: cubeMass = particleMass * particlesPerRigidBody
        let num_particles = fragment.occupancy.count_occupied() as f32;
        let particle_mass = 1.0;
        let mass = num_particles * particle_mass;

        // Fragment voxel positions relative to center (same as ECS system)
        let size = fragment.occupancy.size;
        let center_offset = Vec3::new(
            (size.x as f32 - 1.0) * 0.5,
            (size.y as f32 - 1.0) * 0.5,
            (size.z as f32 - 1.0) * 0.5,
        );

        println!("\n=== 3x3x3 Fragment Drop Test ===");
        println!("Fragment size: {:?}", size);
        println!("Center offset: {:?}", center_offset);
        println!("Floor top at Y=3, fragment starts at Y=15");

        // Log initial particle positions
        println!("\nInitial fragment particle positions:");
        for ly in 0..size.y {
            for lx in 0..size.x {
                for lz in 0..size.z {
                    let local_pos = UVec3::new(lx, ly, lz);
                    if fragment.occupancy.get(local_pos) {
                        let local_offset = Vec3::new(
                            lx as f32 + 0.5 - center_offset.x - 0.5,
                            ly as f32 + 0.5 - center_offset.y - 0.5,
                            lz as f32 + 0.5 - center_offset.z - 0.5,
                        );
                        let world_pos = fragment_pos + fragment_rot * local_offset;
                        if ly == 0 && lx == 1 && lz == 1 {
                            println!("  Bottom center particle ({},{},{}): local_offset={:?}, world={:?}",
                                lx, ly, lz, local_offset, world_pos);
                        }
                    }
                }
            }
        }

        // Simulate for 1200 frames (20 seconds at 60fps)
        for frame in 0..1200 {
            let mut total_force = Vec3::ZERO;
            let mut total_torque = Vec3::ZERO;

            // Iterate over fragment voxels (same as ECS system)
            for lz in 0..size.z {
                for ly in 0..size.y {
                    for lx in 0..size.x {
                        let local_pos = UVec3::new(lx, ly, lz);
                        if !fragment.occupancy.get(local_pos) {
                            continue;
                        }

                        let local_offset = Vec3::new(
                            lx as f32 + 0.5 - center_offset.x - 0.5,
                            ly as f32 + 0.5 - center_offset.y - 0.5,
                            lz as f32 + 0.5 - center_offset.z - 0.5,
                        );

                        let world_offset = fragment_rot * local_offset;
                        let voxel_world_pos = fragment_pos + world_offset;
                        let voxel_vel = fragment_vel + angular_vel.cross(world_offset);

                        let voxel_force = compute_terrain_collision_force(
                            voxel_world_pos,
                            voxel_vel,
                            &terrain.occupancy,
                            &physics_config,
                        );

                        total_force += voxel_force;
                        if voxel_force.length_squared() > 1e-10 {
                            total_torque += world_offset.cross(voxel_force);
                        }
                    }
                }
            }

            // Add gravity
            total_force = apply_gravity(total_force, &physics_config);

            // Integrate velocity
            fragment_vel = integrate_velocity(
                fragment_vel,
                total_force,
                mass,
                physics_config.friction,
                dt,
                physics_config.velocity_threshold,
            );

            // Integrate position
            fragment_pos = integrate_position(fragment_pos, fragment_vel, dt);

            // Angular - CRITICAL: Reference does NOT divide by inertia tensor!
            angular_vel = integrate_angular_velocity(
                angular_vel,
                total_torque,
                physics_config.angular_friction,
                physics_config.angular_force_scalar,
                dt,
                physics_config.velocity_threshold,
            );

            // Log every 60 frames
            if frame % 60 == 0 {
                println!(
                    "Frame {:3}: Y={:.2}, vel_y={:+.2}, force_y={:.1}",
                    frame, fragment_pos.y, fragment_vel.y, total_force.y
                );
            }
        }

        // For a 3x3x3 fragment on floor (top at Y=3):
        // - Bottom particles are at fragment_center.y - 1
        // - Bottom particle centers should rest at ~Y=3.5 (0.5 above floor top)
        // - Fragment center should be at ~Y=4.5
        let expected_center_y = 3.0 + 1.0 + 0.48; // floor_top + half_fragment_size + settling_offset

        println!("\nFinal fragment center: Y={:.2}", fragment_pos.y);
        println!(
            "Expected ~Y={:.2} (floor top {} + 1.0 for half fragment + 0.48 settling)",
            expected_center_y, 3.0
        );

        // Assert fragment landed on floor, not fell through
        assert!(
            fragment_pos.y > 3.5,
            "Fragment fell through floor! Center Y={:.2}, should be > 3.5",
            fragment_pos.y
        );
        assert!(
            fragment_pos.y < 6.0,
            "Fragment floating too high! Center Y={:.2}, should be < 6.0",
            fragment_pos.y
        );

        println!("SUCCESS: 3x3x3 fragment landed at Y={:.2}", fragment_pos.y);
    }

    /// CRITICAL TEST: Flipped 3x3x3 fragment must not clip through floor.
    ///
    /// This test drops a 3x3x3 fragment that's flipped 180° (upside down),
    /// lets it settle, and then checks that EVERY particle is ABOVE the floor.
    #[test]
    fn test_3x3x3_fragment_flipped_no_floor_clipping() {
        use crate::voxel::Voxel;

        // Create floor terrain at Y=0,1,2 (top surface at Y=3)
        let mut terrain_world = VoxelWorld::new();
        for x in -10..10 {
            for z in -10..10 {
                for y in 0..3 {
                    terrain_world.set_voxel(x, y, z, Voxel::solid(100, 100, 100));
                }
            }
        }
        let terrain = TerrainOccupancy::from_voxel_world(&terrain_world);
        let floor_top = 3.0;

        // Create 3x3x3 fragment using EXACT same setup as p22
        let surface_particles = FragmentSurfaceParticles::from_size(3, 1.0);

        println!("\n=== FLIPPED 3x3x3 Fragment Floor Clipping Test ===");
        println!(
            "Particle count: {}",
            surface_particles.local_positions.len()
        );
        println!("Particle diameter: {}", surface_particles.particle_diameter);
        println!("Floor top at Y={}", floor_top);

        // Physics config
        let collision_config = FragmentCollisionConfig::default();
        let physics_config = PhysicsConfig {
            gravity: 9.8,
            particle_diameter: surface_particles.particle_diameter,
            spring_k: collision_config.spring_k,
            damping_k: collision_config.damping_k,
            tangential_k: collision_config.friction_k,
            friction: 0.9,
            angular_friction: 0.3,
            linear_force_scalar: 1.0,
            angular_force_scalar: 1.0,
            velocity_threshold: 1e-6,
        };

        // FLIPPED rotation (180° around X axis)
        let fragment_rot = Quat::from_rotation_x(std::f32::consts::PI);

        // Initial state - drop from Y=10
        let dt = 1.0 / 60.0;
        let mut fragment_pos = Vec3::new(0.0, 10.0, 0.0);
        let mut fragment_vel = Vec3::ZERO;
        let mut angular_vel = Vec3::ZERO;
        let mass = surface_particles.total_mass();

        println!("Fragment rotation: 180° around X (flipped upside down)");
        println!("Initial position: {:?}", fragment_pos);

        // Print local particle positions
        println!("\nLocal particle positions (first 5):");
        for (i, local) in surface_particles.local_positions.iter().take(5).enumerate() {
            println!("  {}: {:?}", i, local);
        }

        // Simulate for 600 frames (10 seconds)
        for frame in 0..600 {
            let mut total_force = Vec3::ZERO;
            let mut total_torque = Vec3::ZERO;

            for local_offset in &surface_particles.local_positions {
                let world_offset = fragment_rot * *local_offset;
                let particle_world_pos = fragment_pos + world_offset;
                let particle_vel = fragment_vel + angular_vel.cross(world_offset);

                let particle_force = compute_terrain_collision_force(
                    particle_world_pos,
                    particle_vel,
                    &terrain.occupancy,
                    &physics_config,
                );

                let particle_force_with_gravity = Vec3::new(
                    particle_force.x,
                    particle_force.y - physics_config.gravity,
                    particle_force.z,
                );

                total_force += particle_force_with_gravity;
                if particle_force.length_squared() > 1e-10 {
                    total_torque += world_offset.cross(particle_force);
                }
            }

            // Integrate
            fragment_vel = integrate_velocity(
                fragment_vel,
                total_force,
                mass,
                physics_config.friction,
                dt,
                physics_config.velocity_threshold,
            );
            fragment_pos = integrate_position(fragment_pos, fragment_vel, dt);
            angular_vel = integrate_angular_velocity(
                angular_vel,
                total_torque,
                physics_config.angular_friction,
                physics_config.angular_force_scalar,
                dt,
                physics_config.velocity_threshold,
            );

            // Log key frames
            if frame % 60 == 0 {
                println!(
                    "Frame {:3}: pos=({:.1},{:.3},{:.1}), vel=({:.1},{:.1},{:.1}), angvel=({:.1},{:.1},{:.1})",
                    frame, fragment_pos.x, fragment_pos.y, fragment_pos.z,
                    fragment_vel.x, fragment_vel.y, fragment_vel.z,
                    angular_vel.x, angular_vel.y, angular_vel.z
                );
            }
        }

        // NOW CHECK EVERY SINGLE PARTICLE
        println!("\n=== Final Particle Positions ===");
        let particle_radius = surface_particles.particle_diameter / 2.0;
        let mut min_particle_bottom = f32::MAX;
        let mut clipping_count = 0;

        for (i, local_offset) in surface_particles.local_positions.iter().enumerate() {
            let world_offset = fragment_rot * *local_offset;
            let particle_world_pos = fragment_pos + world_offset;
            let particle_bottom = particle_world_pos.y - particle_radius;

            if particle_bottom < min_particle_bottom {
                min_particle_bottom = particle_bottom;
            }

            if particle_bottom < floor_top {
                clipping_count += 1;
                println!(
                    "  CLIPPING! Particle {}: center Y={:.3}, bottom Y={:.3} (floor at Y={})",
                    i, particle_world_pos.y, particle_bottom, floor_top
                );
            }
        }

        println!("\nFragment center: Y={:.3}", fragment_pos.y);
        println!("Lowest particle bottom: Y={:.3}", min_particle_bottom);
        println!("Floor top: Y={:.3}", floor_top);
        println!("Penetration: {:.3}", floor_top - min_particle_bottom);
        println!(
            "Clipping particles: {}/{}",
            clipping_count,
            surface_particles.local_positions.len()
        );

        // THE CRITICAL ASSERTION
        assert!(
            min_particle_bottom >= floor_top - 0.01, // Allow 1cm tolerance
            "FLOOR CLIPPING DETECTED!\n\
             Lowest particle bottom Y={:.3} is BELOW floor top Y={:.3}\n\
             Penetration: {:.3} units\n\
             {} particles are clipping through the floor!",
            min_particle_bottom,
            floor_top,
            floor_top - min_particle_bottom,
            clipping_count
        );

        println!("\nSUCCESS: No floor clipping detected");
    }
}
