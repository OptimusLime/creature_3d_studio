//! Phase 23: Kinematic Character Controller Demo
//!
//! Demonstrates a kinematic character controller walking on voxel terrain
//! using the occupancy collision system (no Rapier physics for terrain).
//!
//! Run with: `cargo run --example p23_kinematic_controller`
//!
//! Controls:
//! - WASD: Move
//! - Space: Jump
//! - Mouse: Look around (hold right click)
//! - Escape: Release mouse

use bevy::prelude::*;
use bevy::input::mouse::AccumulatedMouseMotion;
use studio_core::{
    BenchmarkPlugin,
    KinematicController, WorldOccupancy,
    Voxel, VoxelWorld,
    VoxelMaterial, VoxelMaterialPlugin,
    build_world_meshes_cross_chunk,
    DeferredRenderingPlugin,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Phase 23: Kinematic Character Controller".into(),
                resolution: bevy::window::WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BenchmarkPlugin)
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (
            player_input,
            player_movement,
            camera_follow,
            draw_debug_info,
        ).chain())
        .run();
}

/// Player component with kinematic controller
#[derive(Component)]
struct Player {
    controller: KinematicController,
    velocity: Vec3,
    input_dir: Vec3,
    jump_requested: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            controller: KinematicController::new(Vec3::new(0.4, 0.9, 0.4)),
            velocity: Vec3::ZERO,
            input_dir: Vec3::ZERO,
            jump_requested: false,
        }
    }
}

/// World occupancy resource
#[derive(Resource)]
struct TerrainOccupancy(WorldOccupancy);

/// Player movement configuration
#[derive(Resource)]
struct MovementConfig {
    move_speed: f32,
    jump_speed: f32,
    gravity: f32,
    air_control: f32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            move_speed: 8.0,
            jump_speed: 10.0,
            gravity: 25.0,
            air_control: 0.3,
        }
    }
}

/// Camera state
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
            pitch: 0.3, // Slight downward angle
            distance: 10.0,
        }
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    // Create terrain
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
    
    // Create occupancy from terrain
    let occupancy = WorldOccupancy::from_voxel_world(&terrain);
    commands.insert_resource(TerrainOccupancy(occupancy));
    commands.insert_resource(MovementConfig::default());
    
    // Create terrain mesh
    let material = materials.add(VoxelMaterial { ambient: 0.1 });
    let chunk_meshes = build_world_meshes_cross_chunk(&terrain);
    
    commands.spawn((
        Name::new("Terrain"),
        Transform::default(),
        Visibility::default(),
    )).with_children(|parent| {
        for chunk_mesh in chunk_meshes {
            let translation = chunk_mesh.translation();
            let mesh_handle = meshes.add(chunk_mesh.mesh);
            parent.spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(translation),
            ));
        }
    });
    
    // Spawn player (represented as a wireframe capsule later, for now just a marker)
    // Start at y=10 to give plenty of room to fall and land
    commands.spawn((
        Name::new("Player"),
        Player::default(),
        Transform::from_xyz(0.0, 10.0, 0.0),
    ));
    
    // Camera
    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        PlayerCamera::default(),
        Transform::from_xyz(0.0, 15.0, 20.0).looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::Y),
    ));
    
    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: 20000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 30.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    
    // Ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
        ..default()
    });
    
    info!("Controls: WASD to move, Space to jump, Right-click + mouse to look");
}

fn player_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
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
    player.input_dir = rotation * input;
    if player.input_dir.length_squared() > 0.0 {
        player.input_dir = player.input_dir.normalize();
    }
    
    // Jump
    player.jump_requested = keyboard.just_pressed(KeyCode::Space);
    
    // Camera look (when right mouse button held)
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        camera.yaw += delta.x * 0.003;
        camera.pitch = (camera.pitch - delta.y * 0.003).clamp(-1.5, 1.5);
    }
}

fn player_movement(
    time: Res<Time>,
    config: Res<MovementConfig>,
    occupancy: Res<TerrainOccupancy>,
    mut player_query: Query<(&mut Player, &mut Transform)>,
) {
    let delta = time.delta_secs();
    let Ok((mut player, mut transform)) = player_query.single_mut() else { return };
    
    // Apply input to velocity
    let control = if player.controller.grounded { 1.0 } else { config.air_control };
    let target_horizontal = player.input_dir * config.move_speed;
    player.velocity.x = lerp(player.velocity.x, target_horizontal.x, control * 10.0 * delta);
    player.velocity.z = lerp(player.velocity.z, target_horizontal.z, control * 10.0 * delta);
    
    // Jump
    if player.jump_requested && player.controller.can_jump() {
        player.velocity.y = config.jump_speed;
        player.controller.grounded = false;
    }
    
    // Gravity
    if !player.controller.grounded {
        player.velocity.y -= config.gravity * delta;
    }
    
    // Move - extract values to avoid borrow issues
    let mut position = transform.translation;
    let mut velocity = player.velocity;
    player.controller.move_and_slide(&occupancy.0, &mut position, &mut velocity, delta);
    transform.translation = position;
    player.velocity = velocity;
}

fn camera_follow(
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<(&mut Transform, &PlayerCamera), Without<Player>>,
) {
    let Ok(player_transform) = player_query.single() else { return };
    let Ok((mut camera_transform, camera)) = camera_query.single_mut() else { return };
    
    // Calculate camera position based on player position and camera angles
    let offset = Vec3::new(
        camera.yaw.sin() * camera.pitch.cos(),
        camera.pitch.sin(),
        camera.yaw.cos() * camera.pitch.cos(),
    ) * camera.distance;
    
    let target_pos = player_transform.translation + Vec3::Y * 1.5; // Look at player head height
    camera_transform.translation = target_pos + offset;
    camera_transform.look_at(target_pos, Vec3::Y);
}

fn draw_debug_info(
    player_query: Query<(&Player, &Transform)>,
    mut gizmos: Gizmos,
) {
    let Ok((player, transform)) = player_query.single() else { return };
    
    // Draw player collision box
    let half = player.controller.half_extents;
    let pos = transform.translation;
    let color = if player.controller.grounded { 
        Color::srgb(0.0, 1.0, 0.0) 
    } else { 
        Color::srgb(1.0, 0.5, 0.0) 
    };
    
    // Draw wireframe box
    let min = pos - half;
    let max = pos + half;
    
    // Bottom face
    gizmos.line(Vec3::new(min.x, min.y, min.z), Vec3::new(max.x, min.y, min.z), color);
    gizmos.line(Vec3::new(max.x, min.y, min.z), Vec3::new(max.x, min.y, max.z), color);
    gizmos.line(Vec3::new(max.x, min.y, max.z), Vec3::new(min.x, min.y, max.z), color);
    gizmos.line(Vec3::new(min.x, min.y, max.z), Vec3::new(min.x, min.y, min.z), color);
    
    // Top face
    gizmos.line(Vec3::new(min.x, max.y, min.z), Vec3::new(max.x, max.y, min.z), color);
    gizmos.line(Vec3::new(max.x, max.y, min.z), Vec3::new(max.x, max.y, max.z), color);
    gizmos.line(Vec3::new(max.x, max.y, max.z), Vec3::new(min.x, max.y, max.z), color);
    gizmos.line(Vec3::new(min.x, max.y, max.z), Vec3::new(min.x, max.y, min.z), color);
    
    // Vertical edges
    gizmos.line(Vec3::new(min.x, min.y, min.z), Vec3::new(min.x, max.y, min.z), color);
    gizmos.line(Vec3::new(max.x, min.y, min.z), Vec3::new(max.x, max.y, min.z), color);
    gizmos.line(Vec3::new(max.x, min.y, max.z), Vec3::new(max.x, max.y, max.z), color);
    gizmos.line(Vec3::new(min.x, min.y, max.z), Vec3::new(min.x, max.y, max.z), color);
    
    // Velocity arrow
    if player.velocity.length_squared() > 0.1 {
        gizmos.arrow(pos, pos + player.velocity * 0.2, Color::srgb(1.0, 1.0, 0.0));
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}
