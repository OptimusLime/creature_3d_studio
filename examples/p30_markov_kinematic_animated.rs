//! Phase 30: MarkovJunior Kinematic + Animated Generation
//!
//! Walk around a large world while watching a MarkovJunior building generate in real-time.
//! The building spawns on a platform you can walk around and observe from any angle.
//!
//! Controls:
//! - W/S: Move forward/backward
//! - A/D: Turn left/right
//! - Space: Jump
//! - Right-click + mouse: Rotate camera around player
//! - Scroll wheel or [ / ]: Zoom camera in/out
//! - G: Generate new building (creates fresh layer)
//! - R: Remove building (full cleanup)
//! - +/-: Speed up/slow down generation

use bevy::ecs::system::NonSendMut;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseWheel};
use bevy::prelude::*;
use std::f32::consts::PI;
use std::path::Path;
use studio_core::markov_junior::render::RenderPalette;
use studio_core::markov_junior::Model;
use studio_core::voxel_layer::{ChunkEntityMap, VoxelLayer, VoxelLayers};
use studio_core::voxel_mesh::build_merged_chunk_mesh;
use studio_core::{
    compute_kinematic_correction, detect_terrain_collisions, has_ceiling_contact,
    has_floor_contact, DeferredLightingConfig, DeferredPointLight, DeferredRenderable,
    TerrainContact, TerrainOccupancy, Voxel, VoxelFace, VoxelMaterial, VoxelWorld, VoxelWorldApp,
    WorldSource, ATTRIBUTE_VOXEL_AO, ATTRIBUTE_VOXEL_COLOR, ATTRIBUTE_VOXEL_EMISSION,
};

// Grid is 40x40x40 after map expansion
const GRID_SIZE: usize = 40;

fn main() {
    println!("Phase 30: MarkovJunior Kinematic + Animated Generation");
    println!("Controls:");
    println!("  W/S: Move forward/backward");
    println!("  A/D: Turn left/right");
    println!("  Space: Jump");
    println!("  G: Generate new building");
    println!("  R: Remove building");
    println!("  +/-: Speed up/slow down");

    let test_mode = std::env::args().any(|arg| arg == "--test");
    let terrain = build_terrain();

    let mut app = VoxelWorldApp::new("Phase 30: MarkovJunior Kinematic + Animated")
        .with_resolution(1280, 720)
        .with_world(WorldSource::World(terrain.clone()))
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_clear_color(Color::srgb(0.4, 0.6, 0.9))
        .with_shadow_light(Vec3::new(50.0, 80.0, 50.0))
        .with_camera_position(Vec3::new(40.0, 50.0, 60.0), Vec3::new(0.0, 20.0, 0.0))
        .with_resource(MovementConfig::default())
        .with_resource(TerrainOccupancy::from_voxel_world(&terrain))
        .with_resource(NeedPlayerSpawn)
        .with_resource(PlayerPhysicsConfig::default())
        .with_resource(GenerationState::new())
        .with_resource(MjPalette(
            RenderPalette::from_palette_xml().with_emission('R', 130), // Red windows glow
        ))
        .with_resource(DeferredLightingConfig {
            fog_start: 100.0, // Start fog further away
            fog_end: 500.0,   // Push fog end much further
            ..Default::default()
        })
        .with_update_systems(|app| {
            app.add_systems(Startup, (setup_model, setup_voxel_layers));
            app.add_systems(PostStartup, spawn_player_system);
            app.add_systems(
                Update,
                (
                    spawn_player_deferred,
                    attach_player_camera,
                    player_input,
                    generation_input,
                    step_generation,
                    sync_generation_to_layer,
                    update_dirty_chunks_system,
                    player_physics,
                    camera_follow,
                )
                    .chain(),
            );
        });

    if test_mode {
        app = app
            .with_resource(GenerationState::test_mode())
            .with_screenshot_timed("screenshots/p30_markov_kinematic_animated.png", 60, 70);
    } else {
        app = app.with_interactive();
    }

    app.run();
}

/// Build terrain with 40x40 center stage for the building
fn build_terrain() -> VoxelWorld {
    let mut terrain = VoxelWorld::new();

    // Large ground platform (80x80, 2 blocks thick)
    let ground_color = Voxel::solid(60, 90, 60);
    for x in -40..40 {
        for z in -40..40 {
            for y in 0..2 {
                terrain.set_voxel(x, y, z, ground_color);
            }
        }
    }

    // Stone border around center stage (42x42)
    let stone_color = Voxel::solid(100, 100, 110);
    for x in -21..21 {
        terrain.set_voxel(x, 2, -21, stone_color);
        terrain.set_voxel(x, 2, 20, stone_color);
    }
    for z in -21..21 {
        terrain.set_voxel(-21, 2, z, stone_color);
        terrain.set_voxel(20, 2, z, stone_color);
    }

    // Center stage platform (40x40) - matches grid size
    let platform_color = Voxel::solid(80, 80, 85);
    for x in -20..20 {
        for z in -20..20 {
            terrain.set_voxel(x, 2, z, platform_color);
        }
    }

    // Corner pillars
    let pillar_color = Voxel::solid(150, 140, 130);
    let corners = [(-22, -22), (-22, 21), (21, -22), (21, 21)];
    for (cx, cz) in corners {
        for y in 2..12 {
            terrain.set_voxel(cx, y, cz, pillar_color);
        }
    }

    // Glowing crystals
    let crystal_color = Voxel::emissive(100, 200, 255);
    terrain.set_voxel(-22, 12, -22, crystal_color);
    terrain.set_voxel(-22, 12, 21, crystal_color);
    terrain.set_voxel(21, 12, -22, crystal_color);
    terrain.set_voxel(21, 12, 21, crystal_color);

    terrain
}

// ============================================================================
// Resources
// ============================================================================

#[derive(Resource)]
struct MovementConfig {
    move_speed: f32,
    turn_speed: f32,
    jump_speed: f32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            move_speed: 10.0,
            turn_speed: 2.5,
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

#[derive(Component)]
struct Player {
    velocity: Vec3,
    grounded: bool,
    jump_timer: f32,
    facing: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            grounded: false,
            jump_timer: 0.0,
            facing: 0.0,
        }
    }
}

#[derive(Component)]
struct PlayerCamera {
    yaw_offset: f32,
    pitch: f32,
    distance: f32,
}

impl Default for PlayerCamera {
    fn default() -> Self {
        Self {
            yaw_offset: 0.0,
            pitch: 0.5,
            distance: 30.0,
        }
    }
}

#[derive(Resource)]
struct NeedPlayerSpawn;

/// MarkovJunior model - NonSend because Model may not be thread-safe
struct MarkovModel {
    model: Model,
}

#[derive(Resource)]
struct GenerationState {
    /// Is generation active?
    active: bool,
    /// Steps per frame (1 = slow and visible)
    steps_per_frame: usize,
    /// Total steps taken
    step_count: usize,
    /// Current seed
    seed: u64,
    /// Needs full sync this frame
    needs_sync: bool,
}

impl GenerationState {
    fn new() -> Self {
        Self {
            active: false,
            steps_per_frame: 2, // 2 steps per frame for faster generation
            step_count: 0,
            seed: 42,
            needs_sync: false,
        }
    }

    fn test_mode() -> Self {
        Self {
            active: true,
            steps_per_frame: 50, // Fast for test
            step_count: 0,
            seed: 42,
            needs_sync: true,
        }
    }

    fn reset(&mut self) {
        self.active = false;
        self.step_count = 0;
        self.needs_sync = false;
    }
}

#[derive(Resource)]
struct MjPalette(RenderPalette);

// ============================================================================
// Setup Systems
// ============================================================================

fn setup_model(world: &mut World) {
    let xml_path = Path::new("MarkovJunior/models/Apartemazements.xml");
    // Load with small initial size - it will expand to 40x40x40 via map node
    let mut model =
        Model::load_with_size(xml_path, 8, 8, 8).expect("Failed to load Apartemazements");
    model.set_animated(true);

    // If test mode, initialize immediately
    let test_mode = world
        .get_resource::<GenerationState>()
        .map(|s| s.active)
        .unwrap_or(false);

    if test_mode {
        model.reset(42);
        info!("Model loaded - TEST MODE");
    } else {
        info!("Model loaded. Press G to generate!");
    }

    world.insert_non_send_resource(MarkovModel { model });
}

fn setup_voxel_layers(mut commands: Commands, state: Res<GenerationState>) {
    let mut layers = VoxelLayers::new();

    // In test mode, create the generated layer immediately
    if state.active {
        let mut gen_layer = VoxelLayer::new("generated", 10);
        gen_layer.offset = IVec3::new(-20, 3, -20);
        layers.add_layer(gen_layer);
    }

    commands.insert_resource(layers);
    commands.insert_resource(ChunkEntityMap::new());
    info!("VoxelLayers initialized");
}

// ============================================================================
// Player Systems
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
    let start_pos = Vec3::new(-30.0, 5.0, -30.0);
    let mesh = create_player_box_mesh(half_extents, [0.2, 0.8, 0.9]);

    commands
        .spawn((
            Name::new("Player"),
            Player {
                facing: PI * 0.25,
                ..default()
            },
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(VoxelMaterial::default())),
            DeferredRenderable,
            Transform::from_translation(start_pos),
        ))
        .with_child((
            Name::new("PlayerLight"),
            DeferredPointLight {
                color: Color::srgb(1.0, 0.95, 0.8),
                intensity: 2.0,
                radius: 15.0,
            },
            Transform::from_translation(Vec3::new(0.0, 1.5, 0.0)),
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
        // Nose
        [0.0, 0.0, d + 0.3],
        [-0.15, -0.15, d],
        [0.15, -0.15, d],
        [0.0, 0.0, d + 0.3],
        [0.15, -0.15, d],
        [0.15, 0.15, d],
        [0.0, 0.0, d + 0.3],
        [0.15, 0.15, d],
        [-0.15, 0.15, d],
        [0.0, 0.0, d + 0.3],
        [-0.15, 0.15, d],
        [-0.15, -0.15, d],
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
        [0.0, -0.7, 0.7],
        [0.0, -0.7, 0.7],
        [0.0, -0.7, 0.7],
        [0.7, 0.0, 0.7],
        [0.7, 0.0, 0.7],
        [0.7, 0.0, 0.7],
        [0.0, 0.7, 0.7],
        [0.0, 0.7, 0.7],
        [0.0, 0.7, 0.7],
        [-0.7, 0.0, 0.7],
        [-0.7, 0.0, 0.7],
        [-0.7, 0.0, 0.7],
    ];

    let mut colors: Vec<[f32; 3]> = vec![color; 24];
    colors.extend(vec![[1.0, 0.5, 0.0]; 12]); // Orange nose

    let emissions: Vec<f32> = vec![0.0; 36];
    let aos: Vec<f32> = vec![1.0; 36];

    let indices: Vec<u32> = vec![
        0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 8, 9, 10, 10, 11, 8, 12, 13, 14, 14, 15, 12, 16, 17,
        18, 18, 19, 16, 20, 21, 22, 22, 23, 20, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35,
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

// ============================================================================
// Generation Systems
// ============================================================================

fn generation_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<GenerationState>,
    mut model: NonSendMut<MarkovModel>,
    mut layers: ResMut<VoxelLayers>,
    mut chunk_map: ResMut<ChunkEntityMap>,
    mut commands: Commands,
    existing_chunks: Query<Entity, With<GeneratedChunkMesh>>,
) {
    // G: Generate new building
    if keyboard.just_pressed(KeyCode::KeyG) {
        // Full cleanup first
        cleanup_generation(&mut layers, &mut chunk_map, &mut commands, &existing_chunks);

        // Create fresh "generated" layer
        let mut gen_layer = VoxelLayer::new("generated", 10);
        // Offset to center the 40x40x40 grid on the platform (y=3 is on top of platform)
        gen_layer.offset = IVec3::new(-20, 3, -20);
        layers.add_layer(gen_layer);

        // Reset model with new seed
        state.seed = state.seed.wrapping_add(1);
        model.model.reset(state.seed);

        // Reset generation state
        state.active = true;
        state.step_count = 0;
        state.needs_sync = true;

        info!("Generation started with seed {}", state.seed);
    }

    // R: Remove building
    if keyboard.just_pressed(KeyCode::KeyR) {
        cleanup_generation(&mut layers, &mut chunk_map, &mut commands, &existing_chunks);
        state.reset();
        info!("Building removed");
    }

    // Speed control
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        state.steps_per_frame = (state.steps_per_frame * 2).min(100);
        info!("Speed: {} steps/frame", state.steps_per_frame);
    }
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        state.steps_per_frame = (state.steps_per_frame / 2).max(1);
        info!("Speed: {} steps/frame", state.steps_per_frame);
    }
}

fn cleanup_generation(
    layers: &mut ResMut<VoxelLayers>,
    chunk_map: &mut ResMut<ChunkEntityMap>,
    commands: &mut Commands,
    existing_chunks: &Query<Entity, With<GeneratedChunkMesh>>,
) {
    // Remove all chunk mesh entities
    for entity in existing_chunks.iter() {
        commands.entity(entity).despawn();
    }
    chunk_map.clear();

    // Remove the generated layer entirely
    layers.remove_layer("generated");
}

fn step_generation(mut model: NonSendMut<MarkovModel>, mut state: ResMut<GenerationState>) {
    if !state.active {
        return;
    }

    // Run steps
    let mut made_progress = false;
    for _ in 0..state.steps_per_frame {
        if model.model.step() {
            state.step_count += 1;
            made_progress = true;
        } else {
            state.active = false;
            info!("Generation complete after {} steps!", state.step_count);
            break;
        }
    }

    if made_progress {
        state.needs_sync = true;
    }
}

fn sync_generation_to_layer(
    model: NonSendMut<MarkovModel>,
    mut state: ResMut<GenerationState>,
    mut layers: ResMut<VoxelLayers>,
    palette: Res<MjPalette>,
) {
    if !state.needs_sync {
        return;
    }
    state.needs_sync = false;

    let gen_layer = match layers.get_mut("generated") {
        Some(l) => l,
        None => return,
    };

    // Get current grid state
    let grid = model.model.grid();
    let (mx, my, mz) = (grid.mx, grid.my, grid.mz);
    let characters: Vec<char> = grid.characters.iter().cloned().collect();

    // Clear the entire layer first
    gen_layer.world.clear();

    // Write all non-zero voxels
    for z in 0..mz {
        for y in 0..my {
            for x in 0..mx {
                let idx = x + y * mx + z * mx * my;
                let cur = grid.state[idx];

                if cur != 0 {
                    // MJ coords (x, y, z) -> Voxel coords (x, z, y) due to Y/Z swap
                    let vx = x as i32;
                    let vy = z as i32; // MJ Z -> Voxel Y
                    let vz = y as i32; // MJ Y -> Voxel Z

                    let ch = characters.get(cur as usize).copied().unwrap_or('?');
                    let voxel = palette.0.to_voxel(ch);
                    gen_layer.set_voxel(vx, vy, vz, voxel);
                }
            }
        }
    }
}

/// Component to mark generated chunk mesh entities
#[derive(Component)]
struct GeneratedChunkMesh;

fn update_dirty_chunks_system(
    mut commands: Commands,
    mut layers: ResMut<VoxelLayers>,
    mut chunk_map: ResMut<ChunkEntityMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    let dirty = layers.collect_dirty_chunks();
    if dirty.is_empty() {
        return;
    }

    for world_chunk_pos in dirty {
        // Remove old entity
        if let Some(old_entity) = chunk_map.remove(&world_chunk_pos) {
            commands.entity(old_entity).despawn();
        }

        // Build new mesh
        if let Some(chunk_mesh) = build_merged_chunk_mesh(&layers, world_chunk_pos, true) {
            let translation = chunk_mesh.translation();
            let entity = commands
                .spawn((
                    Name::new(format!("GenChunk_{:?}", world_chunk_pos)),
                    GeneratedChunkMesh,
                    Mesh3d(meshes.add(chunk_mesh.mesh)),
                    MeshMaterial3d(materials.add(VoxelMaterial::default())),
                    DeferredRenderable,
                    Transform::from_translation(translation),
                ))
                .id();
            chunk_map.register(world_chunk_pos, entity);
        }
    }
}

// ============================================================================
// Player Input & Physics
// ============================================================================

fn player_input(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    #[allow(deprecated)] mut mouse_wheel: bevy::ecs::event::EventReader<MouseWheel>,
    config: Res<MovementConfig>,
    mut player_query: Query<(&mut Player, &mut Transform)>,
    mut camera_query: Query<&mut PlayerCamera>,
) {
    let Ok((mut player, mut player_transform)) = player_query.single_mut() else {
        return;
    };
    let Ok(mut camera) = camera_query.single_mut() else {
        return;
    };

    let dt = time.delta_secs();

    if player.jump_timer > 0.0 {
        player.jump_timer -= dt;
    }

    // Turning
    if keyboard.pressed(KeyCode::KeyA) {
        player.facing += config.turn_speed * dt;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        player.facing -= config.turn_speed * dt;
    }

    if player.facing > PI {
        player.facing -= 2.0 * PI;
    } else if player.facing < -PI {
        player.facing += 2.0 * PI;
    }

    player_transform.rotation = Quat::from_rotation_y(player.facing);

    // Movement
    let mut move_input = 0.0;
    if keyboard.pressed(KeyCode::KeyW) {
        move_input += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        move_input -= 1.0;
    }

    let forward = Vec3::new(player.facing.sin(), 0.0, player.facing.cos());
    let move_dir = forward * move_input;

    player.velocity.x = move_dir.x * config.move_speed;
    player.velocity.z = move_dir.z * config.move_speed;

    // Jump
    if keyboard.just_pressed(KeyCode::Space) && player.grounded {
        player.velocity.y = config.jump_speed;
        player.grounded = false;
        player.jump_timer = 0.15;
    }

    // Camera orbit
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        camera.yaw_offset += delta.x * 0.003;
        camera.pitch = (camera.pitch - delta.y * 0.003).clamp(-1.2, 1.2);
    }

    // Camera zoom
    let mut zoom_delta = 0.0;
    for event in mouse_wheel.read() {
        zoom_delta -= event.y * 2.0;
    }
    if keyboard.pressed(KeyCode::BracketLeft) {
        zoom_delta -= 30.0 * dt;
    }
    if keyboard.pressed(KeyCode::BracketRight) {
        zoom_delta += 30.0 * dt;
    }
    if zoom_delta != 0.0 {
        camera.distance = (camera.distance + zoom_delta).clamp(10.0, 200.0);
    }
}

fn player_physics(
    time: Res<Time>,
    terrain: Res<TerrainOccupancy>,
    layers: Res<VoxelLayers>,
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

            let terrain_contacts =
                detect_terrain_collisions(sample_pos, &terrain.occupancy, particle_diameter);
            all_contacts.extend(terrain_contacts);

            let layer_contacts = detect_layer_collisions(sample_pos, &layers, particle_diameter);
            all_contacts.extend(layer_contacts);
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
            if (correction.x > 0.0 && player.velocity.x < 0.0)
                || (correction.x < 0.0 && player.velocity.x > 0.0)
            {
                player.velocity.x = 0.0;
            }
        }
        if correction.z.abs() > 0.001 {
            if (correction.z > 0.0 && player.velocity.z < 0.0)
                || (correction.z < 0.0 && player.velocity.z > 0.0)
            {
                player.velocity.z = 0.0;
            }
        }

        if player.grounded && player.velocity.y == 0.0 {
            player.velocity.y = -0.5;
        }
    }
}

fn detect_layer_collisions(
    pos: Vec3,
    layers: &VoxelLayers,
    particle_diameter: f32,
) -> Vec<TerrainContact> {
    let mut contacts = Vec::new();
    let half_size = particle_diameter / 2.0;

    let min_x = (pos.x - half_size).floor() as i32;
    let max_x = (pos.x + half_size).ceil() as i32;
    let min_y = (pos.y - half_size).floor() as i32;
    let max_y = (pos.y + half_size).ceil() as i32;
    let min_z = (pos.z - half_size).floor() as i32;
    let max_z = (pos.z + half_size).ceil() as i32;

    for vx in min_x..=max_x {
        for vy in min_y..=max_y {
            for vz in min_z..=max_z {
                if layers.is_solid(vx, vy, vz) {
                    let voxel_center = Vec3::new(vx as f32 + 0.5, vy as f32 + 0.5, vz as f32 + 0.5);
                    let diff = pos - voxel_center;
                    let abs_diff = diff.abs();
                    let combined = Vec3::splat(0.5 + half_size);

                    if abs_diff.x < combined.x && abs_diff.y < combined.y && abs_diff.z < combined.z
                    {
                        let penetration = combined - abs_diff;

                        let (normal, depth, face) =
                            if penetration.x <= penetration.y && penetration.x <= penetration.z {
                                let face = if diff.x > 0.0 {
                                    VoxelFace::PosX
                                } else {
                                    VoxelFace::NegX
                                };
                                (Vec3::new(diff.x.signum(), 0.0, 0.0), penetration.x, face)
                            } else if penetration.y <= penetration.z {
                                let face = if diff.y > 0.0 {
                                    VoxelFace::Top
                                } else {
                                    VoxelFace::Bottom
                                };
                                (Vec3::new(0.0, diff.y.signum(), 0.0), penetration.y, face)
                            } else {
                                let face = if diff.z > 0.0 {
                                    VoxelFace::PosZ
                                } else {
                                    VoxelFace::NegZ
                                };
                                (Vec3::new(0.0, 0.0, diff.z.signum()), penetration.z, face)
                            };

                        let contact_point = pos - normal * (half_size - depth * 0.5);

                        contacts.push(TerrainContact {
                            normal,
                            penetration: depth,
                            face,
                            point: contact_point,
                        });
                    }
                }
            }
        }
    }

    contacts
}

fn camera_follow(
    player_query: Query<(&Transform, &Player), Without<PlayerCamera>>,
    mut camera_query: Query<(&mut Transform, &PlayerCamera), Without<Player>>,
) {
    let Ok((player_transform, player)) = player_query.single() else {
        return;
    };
    let Ok((mut camera_transform, camera)) = camera_query.single_mut() else {
        return;
    };

    let total_yaw = player.facing + camera.yaw_offset + PI;

    let offset = Vec3::new(
        total_yaw.sin() * camera.pitch.cos(),
        camera.pitch.sin(),
        total_yaw.cos() * camera.pitch.cos(),
    ) * camera.distance;

    let target_pos = player_transform.translation + Vec3::Y * 1.5;
    camera_transform.translation = target_pos + offset;
    camera_transform.look_at(target_pos, Vec3::Y);
}
