//! Phase 29: MarkovJunior Animated Generation
//!
//! Watch a MarkovJunior model build step-by-step in real-time.
//! Press SPACE to start generation, then watch the building construct itself.
//!
//! Run with: `cargo run --example p29_markov_animated`
//!
//! Controls:
//! - Space: Start generation / Pause/resume
//! - R: Restart with new seed
//! - +/-: Speed up/slow down generation
//! - Right-click + mouse: Rotate camera

use bevy::ecs::system::NonSendMut;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use std::path::Path;
use studio_core::markov_junior::render::RenderPalette;
use studio_core::markov_junior::{MjGrid, MjPalette, Model};
use studio_core::{
    DeferredRenderable, VoxelMaterial, VoxelWorld, VoxelWorldApp, WorldSource, ATTRIBUTE_VOXEL_AO,
    ATTRIBUTE_VOXEL_COLOR, ATTRIBUTE_VOXEL_EMISSION,
};

fn main() {
    println!("Phase 29: MarkovJunior Animated Generation");
    println!("Controls:");
    println!("  Space: Start/Pause generation");
    println!("  R: Restart with new seed");
    println!("  +/-: Speed up/slow down");
    println!("  Right-click + mouse: Rotate camera");

    // Check for --test flag
    let test_mode = std::env::args().any(|arg| arg == "--test");

    // Create initial empty world
    let world = VoxelWorld::new();

    let mut app = VoxelWorldApp::new("Phase 29: MarkovJunior Animated Generation")
        .with_resolution(1280, 720)
        .with_world(WorldSource::World(world))
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_clear_color(Color::srgb(0.1, 0.1, 0.15))
        .with_shadow_light(Vec3::new(20.0, 40.0, 20.0))
        .with_camera_position(Vec3::new(30.0, 25.0, 30.0), Vec3::ZERO)
        .with_resource(AnimationState::new())
        .with_resource(CameraOrbit::default())
        .with_update_systems(|app| {
            app.add_systems(Startup, setup_model);
            app.add_systems(
                Update,
                (
                    handle_input,
                    step_model_animated,
                    update_world_mesh,
                    orbit_camera,
                )
                    .chain(),
            );
        });

    if test_mode {
        // In test mode, auto-start and run for a bit
        app = app
            .with_resource(AnimationState::test_mode())
            .with_screenshot("screenshots/p29_markov_animated.png");
    } else {
        app = app.with_interactive();
    }

    app.run();
}

// ============================================================================
// Resources
// ============================================================================

/// Non-Send resource holding the MarkovJunior model
struct MarkovModel {
    model: Model,
    size: usize,
    initialized: bool,
}

#[derive(Resource)]
struct AnimationState {
    /// Whether generation has started
    started: bool,
    /// Whether generation is paused
    paused: bool,
    /// Steps to run per second (controlled by +/-)
    steps_per_second: f32,
    /// Accumulated time for stepping
    accumulated_time: f32,
    /// Current step count
    step_count: usize,
    /// Current seed
    seed: u64,
    /// Whether mesh needs rebuild
    dirty: bool,
    /// Frame counter
    frame_count: usize,
    /// Time since last mesh update
    mesh_update_timer: f32,
}

impl AnimationState {
    fn new() -> Self {
        Self {
            started: false,
            paused: true,
            steps_per_second: 50.0, // 50 steps per second by default
            accumulated_time: 0.0,
            step_count: 0,
            seed: 42,
            dirty: false,
            frame_count: 0,
            mesh_update_timer: 0.0,
        }
    }

    fn test_mode() -> Self {
        Self {
            started: true,
            paused: false,
            steps_per_second: 1000.0, // Fast for testing
            accumulated_time: 0.0,
            step_count: 0,
            seed: 42,
            dirty: true,
            frame_count: 0,
            mesh_update_timer: 0.0,
        }
    }
}

#[derive(Resource)]
struct CameraOrbit {
    yaw: f32,
    pitch: f32,
    distance: f32,
    center: Vec3,
}

impl Default for CameraOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.8,
            pitch: 0.4,
            distance: 50.0, // Far enough to see full building from outside
            center: Vec3::new(0.0, 5.0, 0.0), // Center slightly above ground
        }
    }
}

// ============================================================================
// Systems
// ============================================================================

fn setup_model(world: &mut World) {
    let xml_path = Path::new("MarkovJunior/models/Apartemazements.xml");
    let size = 12; // 12x12x12 for reasonable animation speed
    let mut model =
        Model::load_with_size(xml_path, size, size, size).expect("Failed to load Apartemazements");

    // Check if we're in test mode (auto-start)
    let test_mode = world
        .get_resource::<AnimationState>()
        .map(|s| s.started)
        .unwrap_or(false);

    if test_mode {
        // Initialize for test mode
        model.reset(42);
        info!(
            "Loaded Apartemazements model ({}x{}x{}) - TEST MODE",
            size, size, size
        );
        world.insert_non_send_resource(MarkovModel {
            model,
            size,
            initialized: true,
        });
    } else {
        info!(
            "Loaded Apartemazements model ({}x{}x{}). Press SPACE to start!",
            size, size, size
        );
        world.insert_non_send_resource(MarkovModel {
            model,
            size,
            initialized: false,
        });
    }
}

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut state: ResMut<AnimationState>,
    mut model: NonSendMut<MarkovModel>,
    mut orbit: ResMut<CameraOrbit>,
) {
    // Space: Start or pause/resume
    if keyboard.just_pressed(KeyCode::Space) {
        if !state.started {
            // First press: initialize and start
            model.model.reset(state.seed);
            model.initialized = true;
            state.started = true;
            state.paused = false;
            state.dirty = true;
            info!("Generation started with seed {}", state.seed);
        } else {
            // Toggle pause
            state.paused = !state.paused;
            info!(
                "Generation {}",
                if state.paused { "paused" } else { "resumed" }
            );
        }
    }

    // R: Restart with new seed
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.seed = state.seed.wrapping_add(1);
        model.model.reset(state.seed);
        model.initialized = true;
        state.started = true;
        state.paused = false;
        state.step_count = 0;
        state.dirty = true;
        info!("Restarted with seed {}", state.seed);
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

    // Camera orbit with right mouse
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        orbit.yaw += delta.x * 0.005;
        orbit.pitch = (orbit.pitch - delta.y * 0.005).clamp(0.1, 1.5);
    }
}

fn step_model_animated(
    time: Res<Time>,
    mut model: NonSendMut<MarkovModel>,
    mut state: ResMut<AnimationState>,
) {
    state.frame_count += 1;

    if !state.started || state.paused || !model.initialized {
        return;
    }

    // Accumulate time
    state.accumulated_time += time.delta_secs();

    // Calculate how many steps to run this frame
    let step_interval = 1.0 / state.steps_per_second;
    let mut steps_this_frame = 0;

    while state.accumulated_time >= step_interval {
        state.accumulated_time -= step_interval;

        if model.model.step() {
            state.step_count += 1;
            steps_this_frame += 1;
        } else {
            // Model finished
            state.paused = true;
            info!("Generation complete after {} steps!", state.step_count);
            break;
        }
    }

    if steps_this_frame > 0 {
        state.dirty = true;
    }
}

fn update_world_mesh(
    mut commands: Commands,
    model: bevy::ecs::system::NonSend<MarkovModel>,
    mut state: ResMut<AnimationState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
    existing_meshes: Query<Entity, With<GeneratedMesh>>,
    time: Res<Time>,
) {
    // Rate limit mesh updates to ~20fps for performance
    state.mesh_update_timer += time.delta_secs();
    if !state.dirty || state.mesh_update_timer < 0.05 {
        return;
    }
    state.mesh_update_timer = 0.0;
    state.dirty = false;

    // Remove old mesh entities
    for entity in existing_meshes.iter() {
        commands.entity(entity).despawn();
    }

    if !model.initialized {
        return;
    }

    // Build new mesh from current grid state
    let grid = model.model.grid();

    if let Some(mesh) = build_grid_mesh(grid) {
        commands.spawn((
            Name::new("GeneratedMesh"),
            GeneratedMesh,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(VoxelMaterial::default())),
            DeferredRenderable,
            Transform::IDENTITY,
        ));
    }
}

#[derive(Component)]
struct GeneratedMesh;

fn build_grid_mesh(grid: &MjGrid) -> Option<Mesh> {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    let render_palette = RenderPalette::from_palette_xml();

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut emissions: Vec<f32> = Vec::new();
    let mut aos: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Center offset
    let offset_x = (grid.mx / 2) as f32;
    let offset_y = (grid.my / 2) as f32;
    let offset_z = (grid.mz / 2) as f32;

    // Build cubes for each non-zero voxel
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

fn orbit_camera(orbit: Res<CameraOrbit>, mut camera_query: Query<&mut Transform, With<Camera3d>>) {
    let Ok(mut transform) = camera_query.single_mut() else {
        return;
    };

    let offset = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    ) * orbit.distance;

    transform.translation = orbit.center + offset;
    transform.look_at(orbit.center, Vec3::Y);
}
