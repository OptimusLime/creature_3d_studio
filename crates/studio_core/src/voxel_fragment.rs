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

/// Plugin for voxel fragment physics.
pub struct VoxelFragmentPlugin;

impl Plugin for VoxelFragmentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FragmentConfig>()
            .add_systems(Update, detect_settling_fragments);
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
}
