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

use crate::voxel::VoxelWorld;
use crate::voxel_collision::FragmentOccupancy;
use crate::deferred::GpuCollisionContacts;
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

/// Bundle for spawning a voxel fragment with physics.
#[derive(Bundle)]
pub struct VoxelFragmentBundle {
    pub fragment: VoxelFragment,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub inherited_visibility: InheritedVisibility,
    pub view_visibility: ViewVisibility,
    pub rigid_body: RigidBody,
    pub collider: Collider,
    pub velocity: Velocity,
    pub external_impulse: ExternalImpulse,
    pub gravity_scale: GravityScale,
    pub sleeping: Sleeping,
    pub ccd: Ccd,
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
    // Generate collider from voxel data - use merged cuboids for better performance
    let collider = generate_merged_cuboid_collider(&data)?;
    
    // Calculate origin as integer position
    let origin = IVec3::new(
        position.x.round() as i32,
        position.y.round() as i32,
        position.z.round() as i32,
    );
    
    let entity = commands.spawn(VoxelFragmentBundle {
        fragment: VoxelFragment::new(data, origin),
        transform: Transform::from_translation(position),
        global_transform: GlobalTransform::default(),
        visibility: Visibility::default(),
        inherited_visibility: InheritedVisibility::default(),
        view_visibility: ViewVisibility::default(),
        rigid_body: RigidBody::Dynamic,
        collider,
        velocity: Velocity::default(),
        external_impulse: ExternalImpulse {
            impulse,
            torque_impulse: Vec3::ZERO,
        },
        gravity_scale: GravityScale(1.0),
        sleeping: Sleeping::disabled(),
        ccd: Ccd::enabled(),
    }).id();
    
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
    // Generate collider - use merged cuboids for better performance
    let collider = generate_merged_cuboid_collider(&data)?;
    let chunk_meshes = build_world_meshes_cross_chunk(&data);
    
    // Calculate origin as integer position
    let origin = IVec3::new(
        position.x.round() as i32,
        position.y.round() as i32,
        position.z.round() as i32,
    );
    
    // Spawn parent with physics
    let entity = commands.spawn(VoxelFragmentBundle {
        fragment: VoxelFragment::new(data, origin),
        transform: Transform::from_translation(position),
        global_transform: GlobalTransform::default(),
        visibility: Visibility::default(),
        inherited_visibility: InheritedVisibility::default(),
        view_visibility: ViewVisibility::default(),
        rigid_body: RigidBody::Dynamic,
        collider,
        velocity: Velocity::default(),
        external_impulse: ExternalImpulse {
            impulse,
            torque_impulse: Vec3::ZERO,
        },
        gravity_scale: GravityScale(1.0),
        sleeping: Sleeping::disabled(),
        ccd: Ccd::enabled(),
    }).with_children(|parent| {
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
    }).id();
    
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

/// Configuration for fragment-terrain collision response.
#[derive(Resource)]
pub struct FragmentCollisionConfig {
    /// Force multiplier for collision response.
    /// Higher values = stronger push-out force.
    pub force_multiplier: f32,
    
    /// Damping applied when in contact with terrain.
    /// Helps fragments settle instead of bouncing forever.
    pub contact_damping: f32,
    
    /// Minimum penetration to trigger collision response.
    /// Helps avoid jitter from tiny penetrations.
    pub min_penetration: f32,
    
    /// Enable collision system (for toggling CPU/GPU in benchmarks).
    pub enabled: bool,
}

impl Default for FragmentCollisionConfig {
    fn default() -> Self {
        Self {
            force_multiplier: 500.0,  // Strong enough to counter gravity
            contact_damping: 0.8,      // Dampen velocity on contact
            min_penetration: 0.01,     // Ignore tiny penetrations
            enabled: true,
        }
    }
}

/// System that detects fragment-terrain collisions and applies response forces.
///
/// This uses the CPU occupancy collision system. The same interface will be
/// used when GPU collision is enabled - only the detection method changes.
///
/// ## How it works
///
/// 1. For each fragment with physics, query its occupancy against terrain
/// 2. If collision contacts are found, calculate resolution vector
/// 3. Apply ExternalForce to push fragment out of terrain
/// 4. Apply damping to velocity when in contact
///
/// ## Integration with Rapier
///
/// We use ExternalForce rather than directly modifying position because:
/// - Rapier handles the physics integration properly
/// - Other forces (gravity, other collisions) are preserved
/// - The fragment's Collider still handles fragment-fragment collision via Rapier
pub fn fragment_terrain_collision_system(
    terrain: Res<TerrainOccupancy>,
    collision_config: Res<FragmentCollisionConfig>,
    mut fragments: Query<(
        &VoxelFragment,
        &Transform,
        &mut Velocity,
        &mut ExternalForce,
    )>,
) {
    if !collision_config.enabled {
        return;
    }
    
    for (fragment, transform, mut velocity, mut external_force) in fragments.iter_mut() {
        // Check fragment against terrain using occupancy collision
        let collision_result = terrain.occupancy.check_fragment(
            &fragment.occupancy,
            transform.translation,
            transform.rotation,
        );
        
        if !collision_result.has_collision() {
            continue;
        }
        
        // Calculate resolution vector (direction and magnitude to push out)
        let resolution = collision_result.resolution_vector();
        
        // Skip tiny penetrations to avoid jitter
        if resolution.length() < collision_config.min_penetration {
            continue;
        }
        
        // Apply force proportional to penetration
        // Force = resolution_direction * penetration * multiplier
        let force = resolution * collision_config.force_multiplier;
        external_force.force = force;
        
        // Apply damping when in contact (especially for floor contact)
        if collision_result.has_floor_contact() {
            // Dampen vertical velocity more aggressively on floor contact
            if velocity.linvel.y < 0.0 {
                velocity.linvel.y *= 1.0 - collision_config.contact_damping;
            }
            
            // Also apply some horizontal damping to help settling
            velocity.linvel.x *= 1.0 - collision_config.contact_damping * 0.5;
            velocity.linvel.z *= 1.0 - collision_config.contact_damping * 0.5;
            
            // Dampen angular velocity on floor contact
            velocity.angvel *= 1.0 - collision_config.contact_damping * 0.3;
        }
        
        // Apply torque if contacts are off-center (causes rotation)
        // This makes fragments tumble realistically when hitting at an angle
        let center = transform.translation;
        let avg_contact = collision_result.average_contact_position();
        let lever_arm = avg_contact - center;
        
        if lever_arm.length_squared() > 0.01 {
            let torque = lever_arm.cross(resolution) * collision_config.force_multiplier * 0.1;
            external_force.torque = torque;
        }
    }
}

/// System to clear external forces each frame.
/// 
/// ExternalForce should be re-applied each frame based on current state.
/// This runs before collision detection to ensure fresh state.
pub fn clear_fragment_forces(
    mut fragments: Query<&mut ExternalForce, With<VoxelFragment>>,
) {
    for mut force in fragments.iter_mut() {
        force.force = Vec3::ZERO;
        force.torque = Vec3::ZERO;
    }
}

/// Configuration for GPU collision mode.
#[derive(Resource, Default)]
pub struct GpuCollisionMode {
    /// Use GPU collision instead of CPU.
    pub enabled: bool,
}

/// System that applies GPU collision results to fragments.
///
/// This is an alternative to `fragment_terrain_collision_system` that uses
/// pre-computed GPU collision results instead of running CPU collision.
///
/// GPU collision system with entity-keyed contact application.
///
/// The GPU collision results are computed in the render world and shared
/// via `GpuCollisionContacts`. The results include a `fragment_entities` map
/// that allows us to look up contacts by entity, ensuring forces are applied
/// to the correct fragments regardless of query order.
pub fn gpu_fragment_terrain_collision_system(
    gpu_mode: Option<Res<GpuCollisionMode>>,
    gpu_contacts: Option<Res<GpuCollisionContacts>>,
    collision_config: Res<FragmentCollisionConfig>,
    mut fragments: Query<(
        Entity,
        &VoxelFragment,
        &Transform,
        &mut Velocity,
        &mut ExternalForce,
    )>,
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
    
    // Process each fragment by entity lookup (not query order)
    for (entity, _fragment, transform, mut velocity, mut external_force) in fragments.iter_mut() {
        // Look up this entity's fragment index from the GPU results
        let Some(&fragment_idx) = entity_to_idx.get(&entity) else {
            // This entity wasn't in the GPU collision batch (spawned after extraction?)
            continue;
        };
        
        // Get resolution vector for this fragment from GPU results
        let resolution = collision_result.resolution_vector_for_fragment(fragment_idx);
        
        // Skip if no collision
        if resolution.length_squared() < collision_config.min_penetration * collision_config.min_penetration {
            continue;
        }
        
        // Apply force proportional to penetration
        let force = resolution * collision_config.force_multiplier;
        external_force.force = force;
        
        // Check for floor contact
        let has_floor = collision_result.has_floor_contact_for_fragment(fragment_idx);
        
        if has_floor {
            // Dampen vertical velocity on floor contact
            if velocity.linvel.y < 0.0 {
                velocity.linvel.y *= 1.0 - collision_config.contact_damping;
            }
            
            // Horizontal damping
            velocity.linvel.x *= 1.0 - collision_config.contact_damping * 0.5;
            velocity.linvel.z *= 1.0 - collision_config.contact_damping * 0.5;
            
            // Angular damping
            velocity.angvel *= 1.0 - collision_config.contact_damping * 0.3;
        }
        
        // Calculate torque from contact offset
        let center = transform.translation;
        
        // Get average contact position for this fragment
        let contacts: Vec<_> = collision_result.contacts_for_fragment(fragment_idx).collect();
        if !contacts.is_empty() {
            let avg_pos: Vec3 = contacts.iter()
                .map(|c| Vec3::from(c.position))
                .sum::<Vec3>() / contacts.len() as f32;
            
            let lever_arm = avg_pos - center;
            if lever_arm.length_squared() > 0.01 {
                let torque = lever_arm.cross(resolution) * collision_config.force_multiplier * 0.1;
                external_force.torque = torque;
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
            .add_systems(Update, (
                clear_fragment_forces,
                // Run CPU collision if GPU mode is disabled
                fragment_terrain_collision_system.run_if(|mode: Option<Res<GpuCollisionMode>>| {
                    mode.map_or(true, |m| !m.enabled)
                }),
                // Run GPU collision if enabled
                gpu_fragment_terrain_collision_system.run_if(|mode: Option<Res<GpuCollisionMode>>| {
                    mode.map_or(false, |m| m.enabled)
                }),
                detect_settling_fragments,
            ).chain());
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
        assert!(collider.is_some(), "Non-empty world should produce collider");
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
        assert!(config.force_multiplier > 0.0);
        assert!(config.contact_damping > 0.0);
        assert!(config.contact_damping < 1.0);
        assert!(config.min_penetration > 0.0);
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
        
        assert!(collision.has_collision(), "Fragment inside terrain should collide");
        assert!(collision.has_floor_contact(), "Should detect floor contact");
        
        // Test no collision when fragment is above terrain
        let collision_above = terrain.occupancy.check_fragment(
            &fragment.occupancy,
            Vec3::new(5.5, 5.0, 5.5), // Above terrain
            Quat::IDENTITY,
        );
        
        assert!(!collision_above.has_collision(), "Fragment above terrain should not collide");
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
        assert!(resolution.y > 0.0, "Resolution should push up, got {:?}", resolution);
        assert!(resolution.x.abs() < 0.1, "Should not push X significantly");
        assert!(resolution.z.abs() < 0.1, "Should not push Z significantly");
    }
}
