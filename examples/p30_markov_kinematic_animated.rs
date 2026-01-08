//! Phase 30: MarkovJunior Kinematic + Animated Generation
//!
//! Walk around a large world while watching a MarkovJunior building generate in real-time.
//! The building spawns on a platform you can walk around and observe from any angle.
//!
//! The model uses animated mode to show incremental progress during generation.
//!
//! Run with: `cargo run --example p30_markov_kinematic_animated`
//!
//! Controls:
//! - WASD: Move (relative to camera direction)
//! - Space: Jump
//! - Right-click + mouse: Rotate camera
//! - Scroll wheel or [ / ]: Zoom in/out
//! - G: Start/restart generation
//! - +/-: Speed up/slow down generation

use bevy::ecs::system::{NonSend, NonSendMut};
use bevy::input::mouse::{AccumulatedMouseMotion, MouseWheel};
use bevy::prelude::*;
use std::path::Path;
use studio_core::markov_junior::render::RenderPalette;
use studio_core::markov_junior::{MjGrid, Model};
use studio_core::{
    compute_kinematic_correction, detect_terrain_collisions, has_ceiling_contact,
    has_floor_contact, DeferredRenderable, TerrainOccupancy, Voxel, VoxelMaterial, VoxelWorld,
    VoxelWorldApp, WorldSource, ATTRIBUTE_VOXEL_AO, ATTRIBUTE_VOXEL_COLOR,
    ATTRIBUTE_VOXEL_EMISSION,
};

fn main() {
    println!("Phase 30: MarkovJunior Kinematic + Animated Generation");
    println!("Controls:");
    println!("  WASD: Move (relative to camera direction)");
    println!("  Space: Jump");
    println!("  Right-click + mouse: Rotate camera");
    println!("  Scroll wheel or [/]: Zoom in/out");
    println!("  G: Start/restart generation");
    println!("  +/-: Speed up/slow down");

    // Check for --test flag
    let test_mode = std::env::args().any(|arg| arg == "--test");

    // Build the base terrain (large platform)
    let terrain = build_terrain();

    let mut app = VoxelWorldApp::new("Phase 30: MarkovJunior Kinematic + Animated")
        .with_resolution(1280, 720)
        .with_world(WorldSource::World(terrain.clone()))
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_clear_color(Color::srgb(0.4, 0.6, 0.9)) // Sky blue
        .with_shadow_light(Vec3::new(30.0, 50.0, 30.0))
        .with_camera_position(Vec3::new(20.0, 25.0, 40.0), Vec3::new(0.0, 10.0, 0.0))
        .with_resource(MovementConfig::default())
        .with_resource(TerrainOccupancy::from_voxel_world(&terrain))
        .with_resource(NeedPlayerSpawn)
        .with_resource(PlayerPhysicsConfig::default())
        .with_resource(GenerationState::new())
        .with_update_systems(|app| {
            app.add_systems(Startup, setup_model);
            app.add_systems(PostStartup, spawn_player_system);
            app.add_systems(
                Update,
                (
                    spawn_player_deferred,
                    attach_player_camera,
                    player_input,
                    generation_input,
                    step_generation,
                    update_generation_mesh,
                    player_physics,
                    camera_follow,
                )
                    .chain(),
            );
        });

    if test_mode {
        app = app
            .with_resource(GenerationState::test_mode())
            // Wait for generation to complete before screenshot (generation takes ~15-20 frames)
            .with_screenshot_timed("screenshots/p30_markov_kinematic_animated.png", 30, 40);
    } else {
        app = app.with_interactive();
    }

    app.run();
}

/// Build the base terrain - a large platform with the building area in the center
fn build_terrain() -> VoxelWorld {
    let mut terrain = VoxelWorld::new();

    // Large ground platform (60x60, 2 blocks thick)
    let ground_color = Voxel::solid(60, 90, 60); // Dark green grass
    for x in -30..30 {
        for z in -30..30 {
            for y in 0..2 {
                terrain.set_voxel(x, y, z, ground_color);
            }
        }
    }

    // Stone border around the building area
    let stone_color = Voxel::solid(100, 100, 110);
    for x in -10..10 {
        terrain.set_voxel(x, 2, -10, stone_color);
        terrain.set_voxel(x, 2, 9, stone_color);
    }
    for z in -10..10 {
        terrain.set_voxel(-10, 2, z, stone_color);
        terrain.set_voxel(9, 2, z, stone_color);
    }

    // Raised platform for the building (slightly above ground)
    let platform_color = Voxel::solid(80, 80, 85);
    for x in -9..9 {
        for z in -9..9 {
            terrain.set_voxel(x, 2, z, platform_color);
        }
    }

    // Some decorative elements around the perimeter
    let pillar_color = Voxel::solid(150, 140, 130);
    let corners = [(-12, -12), (-12, 11), (11, -12), (11, 11)];
    for (cx, cz) in corners {
        for y in 2..8 {
            terrain.set_voxel(cx, y, cz, pillar_color);
        }
    }

    // Glowing crystals for atmosphere
    let crystal_color = Voxel::emissive(100, 200, 255);
    terrain.set_voxel(-12, 8, -12, crystal_color);
    terrain.set_voxel(-12, 8, 11, crystal_color);
    terrain.set_voxel(11, 8, -12, crystal_color);
    terrain.set_voxel(11, 8, 11, crystal_color);

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
            yaw: 0.8,   // Camera behind player, looking toward center
            pitch: 0.4, // Slightly elevated
            distance: 25.0,
        }
    }
}

#[derive(Resource)]
struct NeedPlayerSpawn;

/// Non-Send resource holding the MarkovJunior model
struct MarkovModel {
    model: Model,
    #[allow(dead_code)]
    size: usize,
    initialized: bool,
}

#[derive(Resource)]
struct GenerationState {
    started: bool,
    paused: bool,
    steps_per_second: f32,
    accumulated_time: f32,
    step_count: usize,
    seed: u64,
    dirty: bool,
    mesh_update_timer: f32,
}

impl GenerationState {
    fn new() -> Self {
        Self {
            started: false,
            paused: true,
            steps_per_second: 10.0, // Slow enough to see animation
            accumulated_time: 0.0,
            step_count: 0,
            seed: 42,
            dirty: false,
            mesh_update_timer: 0.0,
        }
    }

    fn test_mode() -> Self {
        Self {
            started: true,
            paused: false,
            steps_per_second: 100.0, // Faster for test mode but not crazy
            accumulated_time: 0.0,
            step_count: 0,
            seed: 42,
            dirty: true,
            mesh_update_timer: 0.0,
        }
    }
}

#[derive(Component)]
struct GeneratedBuilding;

// ============================================================================
// Systems
// ============================================================================

fn setup_model(world: &mut World) {
    let xml_path = Path::new("MarkovJunior/models/Apartemazements.xml");
    let size = 8; // Smaller size for better performance
    let mut model =
        Model::load_with_size(xml_path, size, size, size).expect("Failed to load Apartemazements");

    // Enable animated mode for step-by-step visualization
    model.set_animated(true);

    // Check if we're in test mode (auto-start)
    let test_mode = world
        .get_resource::<GenerationState>()
        .map(|s| s.started)
        .unwrap_or(false);

    if test_mode {
        model.reset(42);
        info!(
            "Loaded Apartemazements model ({}x{}x{}) - TEST MODE (animated)",
            size, size, size
        );
        world.insert_non_send_resource(MarkovModel {
            model,
            size,
            initialized: true,
        });
    } else {
        info!(
            "Loaded Apartemazements model ({}x{}x{}). Press G to start generation!",
            size, size, size
        );
        world.insert_non_send_resource(MarkovModel {
            model,
            size,
            initialized: false,
        });
    }
}

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
    // Start near the building area so both player and building are visible
    let start_pos = Vec3::new(15.0, 5.0, 15.0);

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
        [-w, -h, d],
        [w, -h, d],
        [w, h, d],
        [-w, h, d],
        [w, -h, -d],
        [-w, -h, -d],
        [-w, h, -d],
        [w, h, -d],
        [-w, h, d],
        [w, h, d],
        [w, h, -d],
        [-w, h, -d],
        [-w, -h, -d],
        [w, -h, -d],
        [w, -h, d],
        [-w, -h, d],
        [w, -h, d],
        [w, -h, -d],
        [w, h, -d],
        [w, h, d],
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
        0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 8, 9, 10, 10, 11, 8, 12, 13, 14, 14, 15, 12, 16, 17,
        18, 18, 19, 16, 20, 21, 22, 22, 23, 20,
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
    mut mouse_wheel: EventReader<MouseWheel>,
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

    // Rotate input by camera yaw for ego-centric movement
    let rotation = Quat::from_rotation_y(-camera.yaw);
    let mut input_dir = rotation * input;
    if input_dir.length_squared() > 0.0 {
        input_dir = input_dir.normalize();
    }

    player.velocity.x = input_dir.x * config.move_speed;
    player.velocity.z = input_dir.z * config.move_speed;

    // Jump (only when grounded)
    if keyboard.just_pressed(KeyCode::Space) && player.grounded {
        player.velocity.y = config.jump_speed;
        player.grounded = false;
        player.jump_timer = 0.15;
    }

    // Camera rotation (hold right mouse button)
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        camera.yaw += delta.x * 0.003;
        camera.pitch = (camera.pitch - delta.y * 0.003).clamp(-1.5, 1.5);
    }

    // Camera zoom (scroll wheel or [ / ] keys)
    let mut zoom_delta = 0.0;
    for event in mouse_wheel.read() {
        zoom_delta -= event.y * 2.0; // Scroll up = zoom in (decrease distance)
    }
    if keyboard.pressed(KeyCode::BracketLeft) {
        zoom_delta -= 20.0 * time.delta_secs(); // Zoom in
    }
    if keyboard.pressed(KeyCode::BracketRight) {
        zoom_delta += 20.0 * time.delta_secs(); // Zoom out
    }
    if zoom_delta != 0.0 {
        camera.distance = (camera.distance + zoom_delta).clamp(5.0, 50.0);
    }
}

fn generation_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<GenerationState>,
    mut model: NonSendMut<MarkovModel>,
) {
    // G: Start/restart generation
    if keyboard.just_pressed(KeyCode::KeyG) {
        state.seed = if state.started {
            state.seed.wrapping_add(1)
        } else {
            state.seed
        };
        model.model.reset(state.seed);
        model.initialized = true;
        state.started = true;
        state.paused = false;
        state.step_count = 0;
        state.dirty = true;
        info!("Generation started with seed {}", state.seed);
    }

    // Speed control
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        state.steps_per_second = (state.steps_per_second * 1.5).min(500.0);
        info!("Speed: {:.0} steps/second", state.steps_per_second);
    }
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        state.steps_per_second = (state.steps_per_second / 1.5).max(5.0);
        info!("Speed: {:.0} steps/second", state.steps_per_second);
    }
}

fn step_generation(
    time: Res<Time>,
    mut model: NonSendMut<MarkovModel>,
    mut state: ResMut<GenerationState>,
) {
    if !state.started || state.paused || !model.initialized {
        return;
    }

    state.accumulated_time += time.delta_secs();
    let step_interval = 1.0 / state.steps_per_second;
    let mut steps_this_frame = 0;

    while state.accumulated_time >= step_interval {
        state.accumulated_time -= step_interval;

        if model.model.step() {
            state.step_count += 1;
            steps_this_frame += 1;
        } else {
            state.paused = true;
            state.dirty = true; // Ensure final state is rendered
            info!(
                "Generation complete after {} steps! Setting dirty=true for final render",
                state.step_count
            );
            break;
        }
    }

    if steps_this_frame > 0 {
        state.dirty = true;
    }
}

fn update_generation_mesh(
    mut commands: Commands,
    model: NonSend<MarkovModel>,
    mut state: ResMut<GenerationState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
    existing_meshes: Query<Entity, With<GeneratedBuilding>>,
    time: Res<Time>,
) {
    state.mesh_update_timer += time.delta_secs();

    if !state.dirty || state.mesh_update_timer < 0.05 {
        return;
    }
    state.mesh_update_timer = 0.0;
    state.dirty = false;

    // Despawn old mesh entities
    for entity in existing_meshes.iter() {
        commands.entity(entity).despawn();
    }

    if !model.initialized {
        return;
    }

    let grid = model.model.grid();

    if let Some(mesh) = build_building_mesh(grid) {
        // Position the building on top of the platform (y=3)
        commands.spawn((
            Name::new("GeneratedBuilding"),
            GeneratedBuilding,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(VoxelMaterial::default())),
            DeferredRenderable,
            Transform::from_translation(Vec3::new(0.0, 3.0, 0.0)),
        ));
    }
}

fn build_building_mesh(grid: &MjGrid) -> Option<Mesh> {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    let render_palette = RenderPalette::from_palette_xml();

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut emissions: Vec<f32> = Vec::new();
    let mut aos: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let offset_x = (grid.mx / 2) as f32;
    let offset_y = (grid.my / 2) as f32;
    let offset_z = (grid.mz / 2) as f32;

    for (x, y, z, value) in grid.iter_nonzero() {
        let ch = grid.characters.get(value as usize).copied().unwrap_or('?');
        let rgba = render_palette.get(ch).unwrap_or([128, 128, 128, 255]);
        let color = [
            rgba[0] as f32 / 255.0,
            rgba[1] as f32 / 255.0,
            rgba[2] as f32 / 255.0,
        ];

        // Apply coordinate swap: MJ Y -> VoxelWorld Z
        let wx = x as f32 - offset_x;
        let wy = z as f32 - offset_z;
        let wz = y as f32 - offset_y;

        add_cube(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut emissions,
            &mut aos,
            &mut indices,
            wx,
            wy,
            wz,
            color,
        );
    }

    if positions.is_empty() {
        return None;
    }

    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(ATTRIBUTE_VOXEL_COLOR, colors)
        .with_inserted_attribute(ATTRIBUTE_VOXEL_EMISSION, emissions)
        .with_inserted_attribute(ATTRIBUTE_VOXEL_AO, aos)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

fn add_cube(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 3]>,
    emissions: &mut Vec<f32>,
    aos: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    x: f32,
    y: f32,
    z: f32,
    color: [f32; 3],
) {
    let base_idx = positions.len() as u32;
    let s = 0.5;

    let cube_positions = [
        [x - s, y - s, z + s],
        [x + s, y - s, z + s],
        [x + s, y + s, z + s],
        [x - s, y + s, z + s],
        [x + s, y - s, z - s],
        [x - s, y - s, z - s],
        [x - s, y + s, z - s],
        [x + s, y + s, z - s],
        [x - s, y + s, z + s],
        [x + s, y + s, z + s],
        [x + s, y + s, z - s],
        [x - s, y + s, z - s],
        [x - s, y - s, z - s],
        [x + s, y - s, z - s],
        [x + s, y - s, z + s],
        [x - s, y - s, z + s],
        [x + s, y - s, z + s],
        [x + s, y - s, z - s],
        [x + s, y + s, z - s],
        [x + s, y + s, z + s],
        [x - s, y - s, z - s],
        [x - s, y - s, z + s],
        [x - s, y + s, z + s],
        [x - s, y + s, z - s],
    ];

    let cube_normals = [
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

    positions.extend_from_slice(&cube_positions);
    normals.extend_from_slice(&cube_normals);
    colors.extend(std::iter::repeat(color).take(24));
    emissions.extend(std::iter::repeat(0.0).take(24));
    aos.extend(std::iter::repeat(1.0).take(24));

    for face in 0..6 {
        let f = base_idx + face * 4;
        indices.extend_from_slice(&[f, f + 1, f + 2, f + 2, f + 3, f]);
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
