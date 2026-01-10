//! Phase 34: Sky + Terrain Integration Test
//!
//! Tests the sky dome with MJ-generated cloud texture in context with large terrain.
//! Uses the shared character controller for navigation.
//!
//! Run: `cargo run --example p34_sky_terrain_test`
//! Test mode: `cargo run --example p34_sky_terrain_test -- --test`
//!
//! Controls:
//! - WASD: Move
//! - Space: Jump
//! - Right-click + Mouse: Look around (including up at sky)
//! - Q/E: Change time of day
//! - F12: Take screenshot

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use studio_core::{
    CharacterControllerConfig, CharacterControllerPlugin, DeferredLightingConfig,
    DeferredRenderable, PlayerCharacter, SkySphereConfig, SkySpherePlugin, TerrainOccupancy,
    ThirdPersonCamera, Voxel, VoxelMaterial, VoxelWorld, VoxelWorldApp, WorldSource,
    ATTRIBUTE_VOXEL_AO, ATTRIBUTE_VOXEL_COLOR, ATTRIBUTE_VOXEL_EMISSION,
};

const OUTPUT_DIR: &str = "screenshots/sky_terrain";

fn main() {
    println!("==============================================");
    println!("  Phase 34: Sky + Terrain Integration Test");
    println!("==============================================");
    println!();
    println!("Controls:");
    println!("  WASD - Move");
    println!("  Space - Jump");
    println!("  Right-click + Mouse - Look (including up at sky!)");
    println!("  Q/E - Change time of day");
    println!("  F12 - Take screenshot");
    println!();

    // Ensure output directory exists
    std::fs::create_dir_all(OUTPUT_DIR).expect("Failed to create output directory");

    // Build large terrain
    let terrain = build_rolling_hills_terrain();
    let occupancy = TerrainOccupancy::from_voxel_world(&terrain);

    // Check for --test flag
    let test_mode = std::env::args().any(|arg| arg == "--test");

    let mut app = VoxelWorldApp::new("Phase 34: Sky + Terrain Test")
        .with_resolution(1920, 1080)
        .with_world(WorldSource::World(terrain))
        .with_deferred(true)
        .with_greedy_meshing(true)
        .with_emissive_lights(true)
        .with_shadow_light(Vec3::new(100.0, 150.0, 100.0))
        .with_camera_position(Vec3::new(0.0, 30.0, 50.0), Vec3::new(0.0, 10.0, 0.0))
        .with_resource(DeferredLightingConfig {
            fog_start: 200.0,
            fog_end: 800.0,
            fog_color: Color::srgb(0.05, 0.03, 0.08),
            ..Default::default()
        })
        .with_resource(SkySphereConfig {
            radius: 900.0,
            cloud_texture_path: Some("textures/generated/mj_clouds_001.png".to_string()),
            time_of_day: 0.35, // Mid-morning
            enabled: true,
        })
        .with_plugin(|app| {
            app.add_plugins(SkySpherePlugin);
        })
        .with_resource(occupancy)
        .with_resource(CharacterControllerConfig {
            move_speed: 12.0,
            jump_speed: 14.0,
            gravity: 30.0,
            turn_speed: 2.5,
            pitch_speed: 1.5,
            zoom_speed: 10.0,
            min_pitch: -1.0,
            max_pitch: 0.8,
            min_distance: 5.0,
            max_distance: 50.0,
        })
        .with_resource(TimeControl { time_of_day: 0.15 })
        .with_resource(NeedPlayerSpawn)
        .with_plugin(|app| {
            app.add_plugins(CharacterControllerPlugin);
        })
        .with_update_systems(|app| {
            app.add_systems(PostStartup, spawn_player_system);
            app.add_systems(
                Update,
                (
                    spawn_player_deferred,
                    attach_third_person_camera,
                    time_control_system,
                    screenshot_system,
                ),
            );
        });

    if test_mode {
        app = app
            .with_resource(CaptureState::new())
            .with_update_systems(|app| {
                app.add_systems(Update, auto_capture_system);
            });
    } else {
        app = app.with_interactive();
    }

    app.run();
}

/// Build rolling hills terrain extending far in every direction
fn build_rolling_hills_terrain() -> VoxelWorld {
    let mut terrain = VoxelWorld::new();

    let extent = 200i32;
    let ground_base = 5;

    let grass_dark = Voxel::solid(45, 80, 35);
    let grass_light = Voxel::solid(55, 95, 40);
    let dirt = Voxel::solid(80, 60, 40);
    let stone = Voxel::solid(90, 85, 80);

    println!(
        "Generating terrain from -{} to {} (this may take a moment)...",
        extent, extent
    );

    for x in -extent..extent {
        for z in -extent..extent {
            let fx = x as f32;
            let fz = z as f32;

            let hill1 = ((fx * 0.008).sin() * (fz * 0.006).cos()) * 15.0;
            let hill2 = ((fx * 0.015 + 1.0).cos() * (fz * 0.012 + 2.0).sin()) * 8.0;
            let bump = ((fx * 0.05).sin() * (fz * 0.05).cos()) * 3.0;
            let detail = ((fx * 0.2).sin() * (fz * 0.2).cos()) * 0.5;

            let height = ground_base + (hill1 + hill2 + bump + detail) as i32;
            let height = height.max(1);

            for y in 0..height {
                let voxel = if y == height - 1 {
                    if ((x + z) % 3) == 0 {
                        grass_light
                    } else {
                        grass_dark
                    }
                } else if y >= height - 3 {
                    dirt
                } else {
                    stone
                };
                terrain.set_voxel(x, y, z, voxel);
            }
        }

        if (x + extent) % 100 == 0 {
            let progress = ((x + extent) as f32 / (2 * extent) as f32) * 100.0;
            println!("  Progress: {:.0}%", progress);
        }
    }

    // Central tower
    let tower_stone = Voxel::solid(70, 65, 60);
    for y in 0i32..30 {
        for dx in -2i32..=2 {
            for dz in -2i32..=2 {
                if dx.abs() == 2 || dz.abs() == 2 || y >= 25 {
                    terrain.set_voxel(dx, y + 10, dz, tower_stone);
                }
            }
        }
    }

    let beacon = Voxel::emissive(255, 200, 100);
    terrain.set_voxel(0, 41, 0, beacon);
    terrain.set_voxel(0, 42, 0, beacon);

    // Scattered trees
    let tree_positions = [
        (50, 70),
        (-30, 45),
        (80, -20),
        (-60, -80),
        (120, 30),
        (-100, 100),
    ];

    let trunk = Voxel::solid(80, 50, 30);
    let leaves = Voxel::solid(30, 90, 25);

    for (tx, tz) in tree_positions {
        let fx = tx as f32;
        let fz = tz as f32;
        let hill1 = ((fx * 0.008).sin() * (fz * 0.006).cos()) * 15.0;
        let hill2 = ((fx * 0.015 + 1.0).cos() * (fz * 0.012 + 2.0).sin()) * 8.0;
        let ground_y = ground_base + (hill1 + hill2) as i32;

        for y in 0..6 {
            terrain.set_voxel(tx, ground_y + y, tz, trunk);
        }
        for dy in 4i32..9 {
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    if dx.abs() + dz.abs() + (dy - 6).abs() < 5 {
                        terrain.set_voxel(tx + dx, ground_y + dy, tz + dz, leaves);
                    }
                }
            }
        }
    }

    println!("Terrain generation complete!");
    terrain
}

// ============================================================================
// Resources and Components
// ============================================================================

#[derive(Resource)]
struct TimeControl {
    time_of_day: f32,
}

#[derive(Resource)]
struct NeedPlayerSpawn;

#[derive(Resource)]
struct CaptureState {
    frames_waited: u32,
    captured: bool,
}

impl CaptureState {
    fn new() -> Self {
        Self {
            frames_waited: 0,
            captured: false,
        }
    }
}

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
    players: Query<&PlayerCharacter>,
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
    // Start lower - just above expected terrain height near origin
    let start_pos = Vec3::new(0.0, 20.0, 30.0);

    let mesh = create_player_box_mesh(half_extents, [0.2, 0.7, 0.9]);

    commands.spawn((
        Name::new("Player"),
        PlayerCharacter::with_half_extents(half_extents),
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

/// Attach ThirdPersonCamera to the main camera entity
fn attach_third_person_camera(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera3d>, Without<ThirdPersonCamera>)>,
) {
    for entity in cameras.iter() {
        // Start with camera looking slightly up to show sky
        commands.entity(entity).insert(ThirdPersonCamera {
            pitch: -0.1, // Slightly looking UP at sky
            distance: 20.0,
            height_offset: 2.0,
        });
    }
}

fn time_control_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut time_control: ResMut<TimeControl>,
    mut sky_config: ResMut<SkySphereConfig>,
) {
    let mut changed = false;

    if keyboard.pressed(KeyCode::KeyQ) {
        time_control.time_of_day -= time.delta_secs() * 0.1;
        changed = true;
    }
    if keyboard.pressed(KeyCode::KeyE) {
        time_control.time_of_day += time.delta_secs() * 0.1;
        changed = true;
    }

    if changed {
        time_control.time_of_day = time_control.time_of_day.rem_euclid(1.0);
        sky_config.time_of_day = time_control.time_of_day;
    }
}

fn screenshot_system(keyboard: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    if keyboard.just_pressed(KeyCode::F12) {
        let path = format!("{}/manual_capture.png", OUTPUT_DIR);
        println!("Capturing screenshot: {}", path);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

fn auto_capture_system(
    mut state: ResMut<CaptureState>,
    mut commands: Commands,
    mut exit: EventWriter<AppExit>,
) {
    if state.captured {
        // Wait a few more frames after capture before exit
        state.frames_waited += 1;
        if state.frames_waited > 135 {
            exit.write(AppExit::Success);
        }
        return;
    }

    state.frames_waited += 1;

    // Wait for scene to settle
    if state.frames_waited < 120 {
        return;
    }

    let path = format!("{}/terrain_sky_test.png", OUTPUT_DIR);
    println!("Capturing: {}", path);
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));

    state.captured = true;
}
