//! Phase 23: Kinematic Character Controller Demo
//!
//! Demonstrates a kinematic character controller walking on voxel terrain
//! using our spring-damper physics from physics_math.rs.
//!
//! Run with: `cargo run --example p23_kinematic_controller`
//!
//! Controls:
//! - WASD: Move
//! - Space: Jump
//! - Mouse: Look around (hold right click)

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use studio_core::{
    compute_kinematic_correction, detect_terrain_collisions, has_ceiling_contact,
    has_floor_contact, DeferredRenderable, TerrainOccupancy, Voxel, VoxelMaterial, VoxelWorld,
    VoxelWorldApp, WorldSource, ATTRIBUTE_VOXEL_AO, ATTRIBUTE_VOXEL_COLOR,
    ATTRIBUTE_VOXEL_EMISSION,
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
        .with_resource(NeedPlayerSpawn)
        .with_resource(PlayerPhysicsConfig::default())
        .with_setup(|_commands, _world| {
            info!("Controls: WASD to move, Space to jump, Right-click + mouse to look");
        })
        .with_update_systems(|app| {
            app.add_systems(PostStartup, spawn_player_system);
            app.add_systems(
                Update,
                (
                    spawn_player_deferred,
                    attach_player_camera,
                    player_input,
                    player_physics,
                    camera_follow,
                )
                    .chain(),
            );
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
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            move_speed: 8.0,
            jump_speed: 12.0,
        }
    }
}

/// Physics config for player
#[derive(Resource)]
struct PlayerPhysicsConfig {
    gravity: f32,
}

impl Default for PlayerPhysicsConfig {
    fn default() -> Self {
        Self { gravity: 30.0 }
    }
}

/// Component for player-specific state.
#[derive(Component, Default)]
struct Player {
    /// Current velocity
    velocity: Vec3,
    /// Whether player is on ground
    grounded: bool,
    /// Timer to ignore ground checks after jumping
    jump_timer: f32,
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
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(VoxelMaterial::default())),
        DeferredRenderable,
        Transform::from_translation(start_pos),
    ));

    info!("Spawned player at {:?}", start_pos);
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
        [-w, -h, d],
        [w, -h, d],
        [w, h, d],
        [-w, h, d],
        // Back face (-Z)
        [w, -h, -d],
        [-w, -h, -d],
        [-w, h, -d],
        [w, h, -d],
        // Top face (+Y)
        [-w, h, d],
        [w, h, d],
        [w, h, -d],
        [-w, h, -d],
        // Bottom face (-Y)
        [-w, -h, -d],
        [w, -h, -d],
        [w, -h, d],
        [-w, -h, d],
        // Right face (+X)
        [w, -h, d],
        [w, -h, -d],
        [w, h, -d],
        [w, h, d],
        // Left face (-X)
        [-w, -h, -d],
        [-w, -h, d],
        [-w, h, d],
        [-w, h, -d],
    ];

    let normals: Vec<[f32; 3]> = vec![
        // Front
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        // Back
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        // Top
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        // Bottom
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        // Right
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        // Left
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
    ];

    let colors: Vec<[f32; 3]> = vec![color; 24];
    let emissions: Vec<f32> = vec![0.0; 24];
    let aos: Vec<f32> = vec![1.0; 24];

    let indices: Vec<u32> = vec![
        0, 1, 2, 2, 3, 0, // Front
        4, 5, 6, 6, 7, 4, // Back
        8, 9, 10, 10, 11, 8, // Top
        12, 13, 14, 14, 15, 12, // Bottom
        16, 17, 18, 18, 19, 16, // Right
        20, 21, 22, 22, 23, 20, // Left
    ];

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_COLOR, colors)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_EMISSION, emissions)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_AO, aos)
    .with_inserted_indices(Indices::U32(indices))
}

fn attach_player_camera(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera3d>, Without<PlayerCamera>)>,
) {
    for entity in cameras.iter() {
        commands.entity(entity).insert(PlayerCamera::default());
    }
}

/// Handle player input
fn player_input(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    config: Res<MovementConfig>,
    mut player_query: Query<&mut Player>,
    mut camera_query: Query<&mut PlayerCamera>,
) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };
    let Ok(mut camera) = camera_query.single_mut() else {
        return;
    };

    // Decrement jump timer
    if player.jump_timer > 0.0 {
        player.jump_timer -= time.delta_secs();
    }

    // Movement input
    let mut input = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        input.z -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        input.z += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        input.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        input.x += 1.0;
    }

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
        player.jump_timer = 0.15;
    }

    // Camera look (when right mouse button held)
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        camera.yaw += delta.x * 0.003;
        camera.pitch = (camera.pitch - delta.y * 0.003).clamp(-1.5, 1.5);
    }
}

/// Physics simulation using physics_math collision detection with kinematic correction
fn player_physics(
    time: Res<Time>,
    terrain: Res<TerrainOccupancy>,
    physics_config: Res<PlayerPhysicsConfig>,
    mut player_query: Query<(&mut Player, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.05);

    // Player collision shape - sample multiple particles
    let half_w = 0.4;
    let half_h = 0.9;
    let half_d = 0.4;
    let particle_diameter = 0.5; // Size of collision particles

    for (mut player, mut transform) in player_query.iter_mut() {
        let check_ground = player.jump_timer <= 0.0;

        // Apply gravity
        if !player.grounded {
            player.velocity.y -= physics_config.gravity * dt;
        }

        // Integrate position
        transform.translation += player.velocity * dt;

        // Sample collision particles around the player's bounding box
        // Bottom layer (feet)
        let sample_offsets = [
            // Bottom corners and center
            Vec3::new(-half_w, -half_h, -half_d),
            Vec3::new(half_w, -half_h, -half_d),
            Vec3::new(-half_w, -half_h, half_d),
            Vec3::new(half_w, -half_h, half_d),
            Vec3::new(0.0, -half_h, 0.0),
            // Middle layer (sides)
            Vec3::new(-half_w, 0.0, 0.0),
            Vec3::new(half_w, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -half_d),
            Vec3::new(0.0, 0.0, half_d),
            // Top layer (head)
            Vec3::new(0.0, half_h, 0.0),
        ];

        // Collect all contacts from all sample points
        let mut all_contacts = Vec::new();
        for offset in &sample_offsets {
            let sample_pos = transform.translation + *offset;
            let contacts =
                detect_terrain_collisions(sample_pos, &terrain.occupancy, particle_diameter);
            all_contacts.extend(contacts);
        }

        // Check for floor/ceiling contact before correction
        let floor_contact = check_ground && has_floor_contact(&all_contacts);
        let ceiling_contact = has_ceiling_contact(&all_contacts);

        // Compute and apply position correction
        let correction = compute_kinematic_correction(&all_contacts);
        transform.translation += correction;

        // Update velocity based on collisions
        if floor_contact {
            player.grounded = true;
            if player.velocity.y < 0.0 {
                player.velocity.y = 0.0;
            }
        } else {
            player.grounded = false;
        }

        if ceiling_contact && player.velocity.y > 0.0 {
            player.velocity.y = 0.0;
        }

        // Cancel horizontal velocity into walls
        if correction.x.abs() > 0.001 {
            if correction.x > 0.0 && player.velocity.x < 0.0 {
                player.velocity.x = 0.0;
            } else if correction.x < 0.0 && player.velocity.x > 0.0 {
                player.velocity.x = 0.0;
            }
        }
        if correction.z.abs() > 0.001 {
            if correction.z > 0.0 && player.velocity.z < 0.0 {
                player.velocity.z = 0.0;
            } else if correction.z < 0.0 && player.velocity.z > 0.0 {
                player.velocity.z = 0.0;
            }
        }

        // Small downward velocity when grounded to maintain contact
        if player.grounded && player.velocity.y == 0.0 {
            player.velocity.y = -0.5;
        }
    }
}

/// Camera follows player
fn camera_follow(
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<(&mut Transform, &PlayerCamera), Without<Player>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok((mut camera_transform, camera)) = camera_query.single_mut() else {
        return;
    };

    let offset = Vec3::new(
        camera.yaw.sin() * camera.pitch.cos(),
        camera.pitch.sin(),
        camera.yaw.cos() * camera.pitch.cos(),
    ) * camera.distance;

    let target_pos = player_transform.translation + Vec3::Y * 1.5;
    camera_transform.translation = target_pos + offset;
    camera_transform.look_at(target_pos, Vec3::Y);
}
