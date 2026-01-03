//! Phase 23: Kinematic Character Controller Demo
//!
//! Demonstrates a kinematic character controller walking on voxel terrain
//! using the unified GPU collision pipeline: GPU collision → Rapier integration.
//!
//! Run with: `cargo run --example p23_kinematic_controller`
//!
//! Controls:
//! - WASD: Move
//! - Space: Jump
//! - Mouse: Look around (hold right click)

use bevy::prelude::*;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy_rapier3d::prelude::*;
use studio_core::{
    VoxelWorldApp, WorldSource,
    Voxel, VoxelWorld,
    VoxelMaterial, DeferredRenderable,
    GpuCollisionAABB, VoxelFragmentPlugin, TerrainOccupancy, GpuCollisionMode,
    ATTRIBUTE_VOXEL_COLOR, ATTRIBUTE_VOXEL_EMISSION, ATTRIBUTE_VOXEL_AO,
};

fn main() {
    // Build terrain
    let terrain = build_terrain();

    // Check for --test flag to run in screenshot mode
    let test_mode = std::env::args().any(|arg| arg == "--test");
    
    let mut app = VoxelWorldApp::new("Phase 23: Kinematic Character Controller")
        .with_resolution(1280, 720)
        .with_world(WorldSource::World(terrain.clone()))
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_emissive_lights(true)
        .with_shadow_light(Vec3::new(5.0, 15.0, 5.0))
        .with_camera_position(Vec3::new(0.0, 15.0, 20.0), Vec3::new(0.0, 5.0, 0.0))
        .with_resource(MovementConfig::default())
        .with_resource(TerrainOccupancy::from_voxel_world(&terrain))
        .with_resource(GpuCollisionMode { enabled: true })
        .with_resource(NeedPlayerSpawn)
        .with_plugin(|app| {
            // Add Rapier physics
            app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
            // Add VoxelFragmentPlugin for GPU collision systems
            app.add_plugins(VoxelFragmentPlugin);
        })
        .with_setup(|_commands, _world| {
            info!("Controls: WASD to move, Space to jump, Right-click + mouse to look");
            info!("Using unified GPU collision pipeline");
        })
        .with_update_systems(|app| {
            app.add_systems(PostStartup, spawn_player_system);
            // Input and gravity run first
            app.add_systems(Update, (
                spawn_player_deferred,
                attach_player_camera,
                player_input,
                apply_gravity,
                apply_player_velocity,
            ).chain());
            // check_grounded_state must run AFTER gpu_kinematic_collision_system
            // which runs in VoxelFragmentPlugin. Use Last to ensure it runs after.
            app.add_systems(Last, (
                check_grounded_state,
                camera_follow,
            ).chain());
        });
    
    if test_mode {
        app = app.with_screenshot("screenshots/p23_kinematic_controller.png");
    } else {
        app = app.with_interactive();
    }
    
    app.run();
}

fn build_terrain() -> VoxelWorld {
    let mut terrain = VoxelWorld::new();
    
    // Ground platform (30x30, 3 blocks thick)
    let ground_color = Voxel::solid(80, 120, 80);
    for x in -15..15 {
        for z in -15..15 {
            for y in 0..3 {
                terrain.set_voxel(x, y, z, ground_color);
            }
        }
    }
    
    // Some stairs
    let stair_color = Voxel::solid(120, 100, 80);
    for i in 0..5 {
        for x in 5..8 {
            for z in (5 + i)..(8 + i) {
                terrain.set_voxel(x, 3 + i, z, stair_color);
            }
        }
    }
    
    // A wall to slide along
    let wall_color = Voxel::solid(100, 80, 80);
    for y in 3..7 {
        for z in -10..0 {
            terrain.set_voxel(-8, y, z, wall_color);
        }
    }
    
    // A pillar to walk around
    let pillar_color = Voxel::solid(80, 80, 120);
    for y in 3..8 {
        terrain.set_voxel(0, y, 8, pillar_color);
        terrain.set_voxel(1, y, 8, pillar_color);
        terrain.set_voxel(0, y, 9, pillar_color);
        terrain.set_voxel(1, y, 9, pillar_color);
    }
    
    // A raised platform to jump onto
    let platform_color = Voxel::solid(120, 80, 120);
    for x in -5..-2 {
        for z in -8..-5 {
            terrain.set_voxel(x, 5, z, platform_color);
        }
    }
    
    // Add some emissive crystals for visual interest
    let crystal_color = Voxel::emissive(100, 200, 255);
    terrain.set_voxel(10, 4, 10, crystal_color);
    terrain.set_voxel(10, 5, 10, crystal_color);
    terrain.set_voxel(-10, 4, -10, crystal_color);
    
    terrain
}

// ============================================================================
// Resources and Components
// ============================================================================

#[derive(Resource)]
struct MovementConfig {
    move_speed: f32,
    jump_speed: f32,
    gravity: f32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            move_speed: 8.0,
            jump_speed: 10.0,
            gravity: 25.0,
        }
    }
}

/// Component for player-specific state.
#[derive(Component, Default)]
struct Player {
    /// Current velocity (we track this manually for kinematic bodies)
    velocity: Vec3,
    /// Whether player is on ground (detected from GPU collision)
    grounded: bool,
}

#[derive(Component)]
struct PlayerCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for PlayerCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.3,
            distance: 10.0,
        }
    }
}

/// Marker resource indicating we need to spawn the player.
#[derive(Resource)]
struct NeedPlayerSpawn;

// ============================================================================
// Systems
// ============================================================================

/// PostStartup system to spawn player after world setup is complete
fn spawn_player_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
    need_spawn: Option<Res<NeedPlayerSpawn>>,
) {
    if need_spawn.is_none() {
        return;
    }
    
    spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.remove_resource::<NeedPlayerSpawn>();
}

/// Deferred spawn system - handles case where PostStartup system didn't run yet
fn spawn_player_deferred(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
    need_spawn: Option<Res<NeedPlayerSpawn>>,
    players: Query<&Player>,
) {
    if need_spawn.is_none() || !players.is_empty() {
        return;
    }
    
    spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.remove_resource::<NeedPlayerSpawn>();
}

fn spawn_player(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<VoxelMaterial>>,
) {
    let half_extents = Vec3::new(0.4, 0.9, 0.4);
    let start_pos = Vec3::new(0.0, 10.0, 0.0);
    
    // Create player mesh
    let mesh = create_player_box_mesh(half_extents, [0.2, 0.8, 0.9]);
    
    commands.spawn((
        Name::new("Player"),
        Player::default(),
        // Rapier kinematic body - position is controlled directly, not by physics
        RigidBody::KinematicPositionBased,
        Collider::cuboid(half_extents.x, half_extents.y, half_extents.z),
        // GpuCollisionAABB marks this entity for GPU collision detection
        GpuCollisionAABB::new(half_extents),
        // Rendering
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(VoxelMaterial::default())),
        DeferredRenderable,
        Transform::from_translation(start_pos),
    ));
    
    info!("Spawned player at {:?} with GPU collision AABB", start_pos);
}

/// Create a box mesh with voxel vertex attributes
fn create_player_box_mesh(half: Vec3, color: [f32; 3]) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    
    let w = half.x;
    let h = half.y;
    let d = half.z;
    
    // 24 vertices (4 per face, 6 faces) for proper normals
    let positions: Vec<[f32; 3]> = vec![
        // Front face (+Z)
        [-w, -h,  d], [ w, -h,  d], [ w,  h,  d], [-w,  h,  d],
        // Back face (-Z)
        [ w, -h, -d], [-w, -h, -d], [-w,  h, -d], [ w,  h, -d],
        // Top face (+Y)
        [-w,  h,  d], [ w,  h,  d], [ w,  h, -d], [-w,  h, -d],
        // Bottom face (-Y)
        [-w, -h, -d], [ w, -h, -d], [ w, -h,  d], [-w, -h,  d],
        // Right face (+X)
        [ w, -h,  d], [ w, -h, -d], [ w,  h, -d], [ w,  h,  d],
        // Left face (-X)
        [-w, -h, -d], [-w, -h,  d], [-w,  h,  d], [-w,  h, -d],
    ];
    
    let normals: Vec<[f32; 3]> = vec![
        // Front
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        // Back
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0],
        // Top
        [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0],
        // Bottom
        [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0],
        // Right
        [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0],
        // Left
        [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0],
    ];
    
    // All vertices same color, no emission, full AO (1.0 = no darkening)
    let colors: Vec<[f32; 3]> = vec![color; 24];
    let emissions: Vec<f32> = vec![0.0; 24];
    let aos: Vec<f32> = vec![1.0; 24];
    
    // Indices for 12 triangles (2 per face)
    let indices: Vec<u32> = vec![
        0, 1, 2, 2, 3, 0,       // Front
        4, 5, 6, 6, 7, 4,       // Back
        8, 9, 10, 10, 11, 8,    // Top
        12, 13, 14, 14, 15, 12, // Bottom
        16, 17, 18, 18, 19, 16, // Right
        20, 21, 22, 22, 23, 20, // Left
    ];
    
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(ATTRIBUTE_VOXEL_COLOR, colors)
        .with_inserted_attribute(ATTRIBUTE_VOXEL_EMISSION, emissions)
        .with_inserted_attribute(ATTRIBUTE_VOXEL_AO, aos)
        .with_inserted_indices(Indices::U32(indices))
}

/// One-shot system to attach PlayerCamera component to the VoxelWorldApp-spawned camera
fn attach_player_camera(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera3d>, Without<PlayerCamera>)>,
) {
    for entity in cameras.iter() {
        commands.entity(entity).insert(PlayerCamera::default());
    }
}

/// Handle player input and calculate desired velocity.
fn player_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    config: Res<MovementConfig>,
    mut player_query: Query<&mut Player>,
    mut camera_query: Query<&mut PlayerCamera>,
) {
    let Ok(mut player) = player_query.single_mut() else { return };
    let Ok(mut camera) = camera_query.single_mut() else { return };
    
    // Movement input
    let mut input = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) { input.z -= 1.0; }
    if keyboard.pressed(KeyCode::KeyS) { input.z += 1.0; }
    if keyboard.pressed(KeyCode::KeyA) { input.x -= 1.0; }
    if keyboard.pressed(KeyCode::KeyD) { input.x += 1.0; }
    
    // Rotate input by camera yaw
    let rotation = Quat::from_rotation_y(-camera.yaw);
    let mut input_dir = rotation * input;
    if input_dir.length_squared() > 0.0 {
        input_dir = input_dir.normalize();
    }
    
    // Set horizontal velocity from input
    player.velocity.x = input_dir.x * config.move_speed;
    player.velocity.z = input_dir.z * config.move_speed;
    
    // Jump (only when grounded)
    if keyboard.just_pressed(KeyCode::Space) && player.grounded {
        player.velocity.y = config.jump_speed;
        player.grounded = false;
    }
    
    // Camera look (when right mouse button held)
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        camera.yaw += delta.x * 0.003;
        camera.pitch = (camera.pitch - delta.y * 0.003).clamp(-1.5, 1.5);
    }
}

/// Apply gravity to player velocity.
fn apply_gravity(
    time: Res<Time>,
    config: Res<MovementConfig>,
    mut player_query: Query<&mut Player>,
) {
    let dt = time.delta_secs();
    
    for mut player in player_query.iter_mut() {
        if !player.grounded {
            player.velocity.y -= config.gravity * dt;
        }
    }
}

/// Apply player velocity to transform (kinematic body).
/// GPU collision system (`gpu_kinematic_collision_system`) will handle collision response
/// by adjusting the transform after this runs.
fn apply_player_velocity(
    time: Res<Time>,
    mut player_query: Query<(&mut Player, &mut Transform)>,
) {
    let dt = time.delta_secs();
    
    // Clamp dt to prevent tunneling on large timesteps
    let clamped_dt = dt.min(0.05);
    
    for (mut player, mut transform) in player_query.iter_mut() {
        // Apply velocity
        transform.translation += player.velocity * clamped_dt;
        
        // Reset grounded each frame - will be set true by check_grounded_state if we have floor contact
        player.grounded = false;
    }
}

/// Check GPU collision contacts to determine grounded state and zero velocity when landing.
/// This runs AFTER gpu_kinematic_collision_system so position has been corrected.
fn check_grounded_state(
    gpu_contacts: Option<Res<studio_core::GpuCollisionContacts>>,
    mut player_query: Query<(Entity, &mut Player)>,
) {
    let Some(gpu_contacts) = gpu_contacts else { return };
    let result = gpu_contacts.get();
    
    if result.contacts.is_empty() {
        return;
    }
    
    // Build entity-to-index map
    let entity_to_idx: std::collections::HashMap<Entity, u32> = result
        .fragment_entities
        .iter()
        .enumerate()
        .map(|(idx, &entity)| (entity, idx as u32))
        .collect();
    
    for (entity, mut player) in player_query.iter_mut() {
        // Look up this entity's collision index
        let Some(&fragment_idx) = entity_to_idx.get(&entity) else {
            continue;
        };
        
        // Check if we have floor contact
        if result.has_floor_contact_for_fragment(fragment_idx) {
            player.grounded = true;
            
            // Zero vertical velocity when landing to prevent bouncing
            if player.velocity.y < 0.0 {
                player.velocity.y = 0.0;
            }
        }
    }
}

/// Camera follows player
fn camera_follow(
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<(&mut Transform, &PlayerCamera), Without<Player>>,
) {
    let Ok(player_transform) = player_query.single() else { return };
    let Ok((mut camera_transform, camera)) = camera_query.single_mut() else { return };
    
    let offset = Vec3::new(
        camera.yaw.sin() * camera.pitch.cos(),
        camera.pitch.sin(),
        camera.yaw.cos() * camera.pitch.cos(),
    ) * camera.distance;
    
    let target_pos = player_transform.translation + Vec3::Y * 1.5;
    camera_transform.translation = target_pos + offset;
    camera_transform.look_at(target_pos, Vec3::Y);
}
