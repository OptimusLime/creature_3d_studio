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
use crate::voxel_collision::{FragmentOccupancy, WorldOccupancy};
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
    /// Occupancy data for fast terrain collision
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
    /// Enable occupancy-based terrain collision (faster than Rapier trimesh)
    pub use_occupancy_collision: bool,
    /// Force multiplier for collision response
    pub collision_force_scale: f32,
    /// Damping applied when colliding with terrain
    pub collision_damping: f32,
}

impl Default for FragmentConfig {
    fn default() -> Self {
        Self {
            settle_threshold_frames: 60, // 1 second at 60fps
            settle_velocity_threshold: 0.1,
            max_active_fragments: 32,
            use_occupancy_collision: true,
            collision_force_scale: 50.0,
            collision_damping: 0.8,
        }
    }
}

/// Resource holding the terrain's occupancy data for fragment collision.
///
/// This should be updated whenever the terrain changes.
#[derive(Resource, Default)]
pub struct TerrainOccupancy(pub WorldOccupancy);

impl TerrainOccupancy {
    /// Create from a VoxelWorld.
    pub fn from_voxel_world(world: &VoxelWorld) -> Self {
        Self(WorldOccupancy::from_voxel_world(world))
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
    pub collision_groups: CollisionGroups,
    pub velocity: Velocity,
    pub external_impulse: ExternalImpulse,
    pub external_force: ExternalForce,
    pub gravity_scale: GravityScale,
    pub sleeping: Sleeping,
    pub ccd: Ccd,
}

/// Spawn a voxel fragment entity with physics.
///
/// Creates a new entity with:
/// - VoxelFragment component containing the voxel data
/// - RigidBody::Dynamic for physics simulation
/// - Trimesh Collider generated from the voxel data
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
    // Generate collider from voxel data - use merged cuboids for MUCH better performance
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
        collision_groups: collision_groups::fragment_groups(),
        velocity: Velocity::default(),
        external_impulse: ExternalImpulse {
            impulse,
            torque_impulse: Vec3::ZERO,
        },
        external_force: ExternalForce::default(),
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
    // Generate collider - use merged cuboids for MUCH better performance
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
        collision_groups: collision_groups::fragment_groups(),
        velocity: Velocity::default(),
        external_impulse: ExternalImpulse {
            impulse,
            torque_impulse: Vec3::ZERO,
        },
        external_force: ExternalForce::default(),
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

/// System to handle fragment-terrain collision using occupancy data.
///
/// This is much faster than Rapier trimesh collision for voxel terrain.
/// It checks each fragment's voxels against the terrain occupancy and applies
/// forces to push the fragment out of the terrain.
pub fn fragment_terrain_collision(
    config: Res<FragmentConfig>,
    terrain: Option<Res<TerrainOccupancy>>,
    mut fragments: Query<(
        &VoxelFragment,
        &Transform,
        &mut Velocity,
        &mut ExternalForce,
    )>,
) {
    // Skip if occupancy collision is disabled or no terrain loaded
    if !config.use_occupancy_collision {
        return;
    }
    
    let Some(terrain) = terrain else {
        return;
    };
    
    for (fragment, transform, mut velocity, mut force) in fragments.iter_mut() {
        // Check fragment against terrain
        let collision = terrain.0.check_fragment(
            &fragment.occupancy,
            transform.translation,
            transform.rotation,
        );
        
        if !collision.has_collision() {
            continue;
        }
        
        // Calculate resolution force
        let resolution = collision.resolution_vector();
        
        // Apply force to push fragment out of terrain
        // Scale by number of contacts to avoid over-correction
        let force_magnitude = resolution.length() * config.collision_force_scale;
        if force_magnitude > 0.001 {
            let force_dir = resolution.normalize_or_zero();
            force.force += force_dir * force_magnitude;
            
            // Apply damping to velocity in the collision direction
            let vel_into_collision = velocity.linvel.dot(-force_dir);
            if vel_into_collision > 0.0 {
                velocity.linvel += force_dir * vel_into_collision * config.collision_damping;
            }
        }
        
        // Apply torque if collision is off-center
        let avg_contact = collision.average_contact_position();
        let to_contact = avg_contact - transform.translation;
        if to_contact.length_squared() > 0.01 {
            let torque = to_contact.cross(resolution) * config.collision_force_scale * 0.1;
            force.torque += torque;
        }
    }
}

/// Collision groups for fragment physics.
///
/// When using occupancy-based terrain collision:
/// - Fragments are in GROUP_2, filter GROUP_2 (only collide with other fragments)
/// - Terrain uses default groups (no fragment collision through Rapier)
pub mod collision_groups {
    use bevy_rapier3d::prelude::*;
    
    /// Group for dynamic fragments
    pub const FRAGMENT_GROUP: Group = Group::GROUP_2;
    
    /// Collision filter for fragments - only collide with other fragments
    pub fn fragment_groups() -> CollisionGroups {
        CollisionGroups::new(FRAGMENT_GROUP, FRAGMENT_GROUP)
    }
}

/// Plugin for voxel fragment physics.
pub struct VoxelFragmentPlugin;

impl Plugin for VoxelFragmentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FragmentConfig>()
            .init_resource::<TerrainOccupancy>()
            .add_systems(Update, (
                detect_settling_fragments,
                fragment_terrain_collision,
            ));
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

    /// Simulates a fragment dropping onto flat ground and verifies it stays there.
    /// 
    /// This test mimics the physics loop behavior:
    /// 1. Fragment starts above ground
    /// 2. Gravity pulls it down
    /// 3. It collides with ground
    /// 4. It should settle and stay on the ground (not fly away!)
    #[test]
    fn test_fragment_drops_onto_ground_and_stays() {
        // Create flat terrain (10x10, 1 block thick at y=0)
        let mut terrain_world = VoxelWorld::new();
        for x in -5..5 {
            for z in -5..5 {
                terrain_world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        let terrain = WorldOccupancy::from_voxel_world(&terrain_world);
        
        // Create a 2x2x2 fragment
        let mut fragment_world = VoxelWorld::new();
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    fragment_world.set_voxel(x, y, z, Voxel::solid(200, 100, 100));
                }
            }
        }
        let fragment = VoxelFragment::new(fragment_world, IVec3::ZERO);
        
        // Simulation state
        let mut position = Vec3::new(0.0, 5.0, 0.0); // Start above ground
        let mut velocity = Vec3::ZERO;
        let rotation = Quat::IDENTITY;
        
        let config = FragmentConfig::default();
        let gravity = 9.81;
        let dt = 1.0 / 60.0; // 60 FPS
        
        println!("=== Fragment Drop Simulation ===");
        println!("Start position: {:?}", position);
        
        // Simulate 3 seconds (180 frames)
        for frame in 0..180 {
            // Apply gravity
            velocity.y -= gravity * dt;
            
            // Move
            position += velocity * dt;
            
            // Check collision
            let collision = terrain.check_fragment(
                &fragment.occupancy,
                position,
                rotation,
            );
            
            if collision.has_collision() {
                let resolution = collision.resolution_vector();
                
                // Apply the same logic as fragment_terrain_collision system
                let force_magnitude = resolution.length() * config.collision_force_scale;
                if force_magnitude > 0.001 {
                    let force_dir = resolution.normalize_or_zero();
                    
                    // This is what the system does - apply force as acceleration
                    // (assuming mass=1 for simplicity)
                    velocity += force_dir * force_magnitude * dt;
                    
                    // Apply damping
                    let vel_into_collision = velocity.dot(-force_dir);
                    if vel_into_collision > 0.0 {
                        velocity += force_dir * vel_into_collision * config.collision_damping;
                    }
                }
            }
            
            // Log every 30 frames
            if frame % 30 == 0 {
                println!(
                    "Frame {}: pos=({:.2}, {:.2}, {:.2}), vel=({:.2}, {:.2}, {:.2}), collision={}",
                    frame, position.x, position.y, position.z,
                    velocity.x, velocity.y, velocity.z,
                    collision.has_collision()
                );
            }
        }
        
        println!("Final position: {:?}", position);
        println!("Final velocity: {:?}", velocity);
        
        // CRITICAL ASSERTIONS:
        // 1. Fragment should be near ground level (y ~ 1-2, since fragment is 2 units tall)
        assert!(
            position.y > 0.5 && position.y < 5.0,
            "Fragment should be near ground, not at y={:.2}", position.y
        );
        
        // 2. Fragment should not have flown away horizontally
        assert!(
            position.x.abs() < 5.0 && position.z.abs() < 5.0,
            "Fragment flew away horizontally! pos=({:.2}, {:.2})", position.x, position.z
        );
        
        // 3. Velocity should be small (settled)
        assert!(
            velocity.length() < 5.0,
            "Fragment has high velocity after 3 seconds: {:?}", velocity
        );
        
        // 4. Fragment should NOT be flying upward
        assert!(
            velocity.y < 2.0,
            "Fragment is flying upward! vel.y={:.2}", velocity.y
        );
    }
    
    /// Test that simulates the exact collision response logic to find bugs.
    #[test]
    fn test_collision_response_logic() {
        // Simple scenario: fragment at y=0.5 (partially in ground at y=0)
        let mut terrain_world = VoxelWorld::new();
        terrain_world.set_voxel(0, 0, 0, Voxel::solid(100, 100, 100));
        let terrain = WorldOccupancy::from_voxel_world(&terrain_world);
        
        // Single voxel fragment
        let mut fragment_world = VoxelWorld::new();
        fragment_world.set_voxel(0, 0, 0, Voxel::solid(200, 100, 100));
        let fragment = VoxelFragment::new(fragment_world, IVec3::ZERO);
        
        // Fragment partially in ground
        let position = Vec3::new(0.5, 0.5, 0.5);
        let rotation = Quat::IDENTITY;
        
        let collision = terrain.check_fragment(&fragment.occupancy, position, rotation);
        
        println!("=== Collision Response Test ===");
        println!("Fragment position: {:?}", position);
        println!("Has collision: {}", collision.has_collision());
        println!("Contact count: {}", collision.contact_count());
        
        if collision.has_collision() {
            let resolution = collision.resolution_vector();
            println!("Resolution vector: {:?}", resolution);
            
            for (i, contact) in collision.contacts.iter().enumerate() {
                println!(
                    "  Contact {}: pos={:?}, normal={:?}, penetration={:.3}",
                    i, contact.world_pos, contact.normal, contact.penetration
                );
            }
            
            // The resolution should push UP (positive Y) since we're in the ground
            assert!(
                resolution.y >= 0.0,
                "Resolution should push up, not down! resolution.y={:.3}", resolution.y
            );
            
            // Resolution should not push horizontally (we're centered on the voxel)
            assert!(
                resolution.x.abs() < 0.5 && resolution.z.abs() < 0.5,
                "Unexpected horizontal resolution: ({:.3}, {:.3})", resolution.x, resolution.z
            );
        }
    }
}
