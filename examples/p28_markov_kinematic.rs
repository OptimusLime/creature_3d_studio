//! Phase 28: MarkovJunior Kinematic Controller
//!
//! Generates a 3D building using Apartemazements and lets you walk around inside it.
//!
//! Run with: `cargo run --example p28_markov_kinematic`
//!
//! Controls:
//! - WASD: Move
//! - Space: Jump
//! - Right-click + mouse: Look around

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use std::path::Path;
use studio_core::markov_junior::{MjPalette, Model};
use studio_core::{
    compute_kinematic_correction, detect_terrain_collisions, has_ceiling_contact,
    has_floor_contact, DeferredRenderable, TerrainOccupancy, VoxelMaterial, VoxelWorld,
    VoxelWorldApp, WorldSource, ATTRIBUTE_VOXEL_AO, ATTRIBUTE_VOXEL_COLOR,
    ATTRIBUTE_VOXEL_EMISSION,
};

fn main() {
    println!("Phase 28: MarkovJunior Kinematic Controller");

    // Load and run the Apartemazements model
    let xml_path = Path::new("MarkovJunior/models/Apartemazements.xml");
    let size = 16; // 16x16x16 for reasonable generation time
    let mut model =
        Model::load_with_size(xml_path, size, size, size).expect("Failed to load Apartemazements");

    println!(
        "Grid size: {}x{}x{}",
        model.grid().mx,
        model.grid().my,
        model.grid().mz
    );

    // Run the model with a fixed seed
    let seed = 42;
    let max_steps = 100000;
    let steps = model.run(seed, max_steps);
    let grid = model.grid();

    let nonzero = grid.count_nonzero();
    let total = grid.mx * grid.my * grid.mz;
    println!(
        "Generation complete: {} steps, {} voxels ({:.1}% filled)",
        steps,
        nonzero,
        100.0 * nonzero as f64 / total as f64
    );

    // Convert to VoxelWorld using palette
    let palette = MjPalette::from_grid(grid);
    let world = grid.to_voxel_world(&palette);

    let voxel_count: usize = world.iter_chunks().map(|(_, c)| c.count()).sum();
    println!("VoxelWorld contains {} voxels", voxel_count);

    // Check for --test flag to run in screenshot mode
    let test_mode = std::env::args().any(|arg| arg == "--test");

    let mut app = VoxelWorldApp::new("Phase 28: MarkovJunior Kinematic Controller")
        .with_resolution(1280, 720)
        .with_world(WorldSource::World(world.clone()))
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_clear_color(Color::srgb(0.4, 0.6, 0.9)) // Sky blue
        .with_shadow_light(Vec3::new(20.0, 40.0, 20.0))
        .with_camera_angle(45.0, 30.0)
        .with_zoom(0.5)
        .with_resource(MovementConfig::default())
        .with_resource(TerrainOccupancy::from_voxel_world(&world))
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
        app = app.with_screenshot("screenshots/p28_markov_kinematic.png");
    } else {
        app = app.with_interactive();
    }

    app.run();
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

#[derive(Resource)]
struct PlayerPhysicsConfig {
    gravity: f32,
}

impl Default for PlayerPhysicsConfig {
    fn default() -> Self {
        Self { gravity: 30.0 }
    }
}

#[derive(Component, Default)]
struct Player {
    velocity: Vec3,
    grounded: bool,
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
            distance: 12.0,
        }
    }
}

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
    // Start above the building so we fall down onto it
    let start_pos = Vec3::new(0.0, 20.0, 0.0);

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

fn create_player_box_mesh(half: Vec3, color: [f32; 3]) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    let w = half.x;
    let h = half.y;
    let d = half.z;

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
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
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

    if player.jump_timer > 0.0 {
        player.jump_timer -= time.delta_secs();
    }

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

    let rotation = Quat::from_rotation_y(-camera.yaw);
    let mut input_dir = rotation * input;
    if input_dir.length_squared() > 0.0 {
        input_dir = input_dir.normalize();
    }

    player.velocity.x = input_dir.x * config.move_speed;
    player.velocity.z = input_dir.z * config.move_speed;

    if keyboard.just_pressed(KeyCode::Space) && player.grounded {
        player.velocity.y = config.jump_speed;
        player.grounded = false;
        player.jump_timer = 0.15;
    }

    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        camera.yaw += delta.x * 0.003;
        camera.pitch = (camera.pitch - delta.y * 0.003).clamp(-1.5, 1.5);
    }
}

fn player_physics(
    time: Res<Time>,
    terrain: Res<TerrainOccupancy>,
    physics_config: Res<PlayerPhysicsConfig>,
    mut player_query: Query<(&mut Player, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.05);

    let half_w = 0.4;
    let half_h = 0.9;
    let half_d = 0.4;
    let particle_diameter = 0.5;

    for (mut player, mut transform) in player_query.iter_mut() {
        let check_ground = player.jump_timer <= 0.0;

        if !player.grounded {
            player.velocity.y -= physics_config.gravity * dt;
        }

        transform.translation += player.velocity * dt;

        let sample_offsets = [
            Vec3::new(-half_w, -half_h, -half_d),
            Vec3::new(half_w, -half_h, -half_d),
            Vec3::new(-half_w, -half_h, half_d),
            Vec3::new(half_w, -half_h, half_d),
            Vec3::new(0.0, -half_h, 0.0),
            Vec3::new(-half_w, 0.0, 0.0),
            Vec3::new(half_w, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -half_d),
            Vec3::new(0.0, 0.0, half_d),
            Vec3::new(0.0, half_h, 0.0),
        ];

        let mut all_contacts = Vec::new();
        for offset in &sample_offsets {
            let sample_pos = transform.translation + *offset;
            let contacts =
                detect_terrain_collisions(sample_pos, &terrain.occupancy, particle_diameter);
            all_contacts.extend(contacts);
        }

        let floor_contact = check_ground && has_floor_contact(&all_contacts);
        let ceiling_contact = has_ceiling_contact(&all_contacts);

        let correction = compute_kinematic_correction(&all_contacts);
        transform.translation += correction;

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

        if player.grounded && player.velocity.y == 0.0 {
            player.velocity.y = -0.5;
        }
    }
}

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
