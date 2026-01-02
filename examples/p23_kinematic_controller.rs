//! Phase 23: Kinematic Character Controller Demo
//!
//! Demonstrates a kinematic character controller walking on voxel terrain
//! using the VoxelPhysicsWorld API with fixed timestep simulation.
//!
//! Run with: `cargo run --example p23_kinematic_controller`
//!
//! Controls:
//! - WASD: Move
//! - Space: Jump
//! - Mouse: Look around (hold right click)

use bevy::prelude::*;
use bevy::input::mouse::AccumulatedMouseMotion;
use studio_core::{
    VoxelWorldApp, WorldSource,
    VoxelPhysicsWorld, PhysicsConfig, KinematicBody, BodyHandle,
    WorldOccupancy,
    Voxel, VoxelWorld,
    VoxelMaterial, DeferredRenderable,
};

fn main() {
    // Build terrain
    let terrain = build_terrain();
    
    // Create physics world from terrain
    let occupancy = WorldOccupancy::from_voxel_world(&terrain);
    let mut physics = VoxelPhysicsWorld::new(occupancy, PhysicsConfig::default());
    
    // Add player body at y=10 (will fall and land on floor)
    let player_body = physics.add_body(KinematicBody::player(Vec3::new(0.0, 10.0, 0.0)));

    // Check for --test flag to run in screenshot mode
    let test_mode = std::env::args().any(|arg| arg == "--test");
    
    let mut app = VoxelWorldApp::new("Phase 23: Kinematic Character Controller")
        .with_resolution(1280, 720)
        .with_world(WorldSource::World(terrain))
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_emissive_lights(true) // Spawn lights from emissive crystals
        .with_shadow_light(Vec3::new(5.0, 15.0, 5.0)) // Add shadow-casting light
        .with_camera_position(Vec3::new(0.0, 15.0, 20.0), Vec3::new(0.0, 5.0, 0.0)) // Let VoxelWorldApp spawn camera
        .with_resource(PhysicsWorld(physics))
        .with_resource(PlayerBodyHandle(player_body))
        .with_resource(MovementConfig::default())
        .with_setup(|_commands, _world| {
            info!("Controls: WASD to move, Space to jump, Right-click + mouse to look");
        })
        .with_update_systems(|app| {
            app.add_systems(Startup, spawn_player_mesh);
            app.add_systems(Update, (
                attach_player_camera,
                player_input,
                physics_step,
                sync_transforms,
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
struct PhysicsWorld(VoxelPhysicsWorld);

#[derive(Resource)]
struct PlayerBodyHandle(BodyHandle);

#[derive(Resource)]
struct MovementConfig {
    move_speed: f32,
    jump_speed: f32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            move_speed: 8.0,
            jump_speed: 10.0,
        }
    }
}

#[derive(Component)]
struct Player;

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

// ============================================================================
// Systems
// ============================================================================

/// Spawn the player mesh - a box that participates in deferred rendering
fn spawn_player_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
    physics: Res<PhysicsWorld>,
    player_handle: Res<PlayerBodyHandle>,
) {
    // Get player dimensions from physics body
    let body = physics.0.get_body(player_handle.0).expect("Player body should exist");
    let half = body.half_extents;
    
    // Create a box mesh with voxel attributes (color, emission, AO)
    let mesh = create_player_box_mesh(half, [0.2, 0.8, 0.9]); // Cyan color
    
    commands.spawn((
        Name::new("Player"),
        Player,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(VoxelMaterial::default())),
        DeferredRenderable,
        Transform::from_translation(body.position),
    ));
}

/// Create a box mesh with voxel vertex attributes
fn create_player_box_mesh(half: Vec3, color: [f32; 3]) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    use studio_core::{ATTRIBUTE_VOXEL_COLOR, ATTRIBUTE_VOXEL_EMISSION, ATTRIBUTE_VOXEL_AO};
    
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

fn player_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    config: Res<MovementConfig>,
    mut physics: ResMut<PhysicsWorld>,
    player_handle: Res<PlayerBodyHandle>,
    mut camera_query: Query<&mut PlayerCamera>,
) {
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
    
    // Set input velocity on physics body
    physics.0.set_body_input_velocity(player_handle.0, input_dir * config.move_speed);
    
    // Jump request
    if keyboard.just_pressed(KeyCode::Space) {
        physics.0.jump(player_handle.0, config.jump_speed);
    }
    
    // Camera look (when right mouse button held)
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        camera.yaw += delta.x * 0.003;
        camera.pitch = (camera.pitch - delta.y * 0.003).clamp(-1.5, 1.5);
    }
}

fn physics_step(time: Res<Time>, mut physics: ResMut<PhysicsWorld>) {
    physics.0.step(time.delta_secs());
}

fn sync_transforms(
    physics: Res<PhysicsWorld>,
    player_handle: Res<PlayerBodyHandle>,
    mut player_query: Query<&mut Transform, With<Player>>,
) {
    if let Some(body) = physics.0.get_body(player_handle.0) {
        for mut transform in player_query.iter_mut() {
            transform.translation = body.position;
        }
    }
}

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


