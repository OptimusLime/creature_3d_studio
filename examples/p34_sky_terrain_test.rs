//! Phase 34: Sky + Terrain Integration Test
//!
//! Tests the sky dome with MJ-generated cloud texture in context with large terrain.
//! Uses the shared character controller for navigation.
//!
//! Run: `cargo run --example p34_sky_terrain_test`
//! Test mode: `cargo run --example p34_sky_terrain_test -- --test`
//!
//! Controls:
//! - WASD: Move, Space: Jump
//! - Q/E: Zoom, I/K: Fast pitch (look at sky)
//! - T: Moon1 (purple) orbit, Y: Moon2 (orange) orbit
//! - G: Time of day (sun/sky), Shift: reverse direction
//! - F12: Screenshot

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::f32::consts::TAU;
use studio_core::deferred::SkyDomeConfig;
use studio_core::{
    CharacterControllerConfig, CharacterControllerPlugin, DeferredLightingConfig,
    DeferredRenderable, MoonConfig, PlayerCharacter, TerrainOccupancy, ThirdPersonCamera, Voxel,
    VoxelMaterial, VoxelWorld, VoxelWorldApp, WorldSource, ATTRIBUTE_VOXEL_AO,
    ATTRIBUTE_VOXEL_COLOR, ATTRIBUTE_VOXEL_EMISSION,
};

const OUTPUT_DIR: &str = "screenshots/sky_terrain";

fn main() {
    println!("==============================================");
    println!("  Phase 34: Sky + Terrain Integration Test");
    println!("==============================================");
    println!();
    println!("Movement Controls:");
    println!("  WASD  - Move");
    println!("  Space - Jump");
    println!("  Q/E   - Zoom camera in/out");
    println!("  I/K   - Fast pitch (look up/down at sky)");
    println!();
    println!("Sky Controls:");
    println!("  T     - Move moon1 (purple) through orbit");
    println!("  Y     - Move moon2 (orange) through orbit");
    println!("  G     - Change time of day (sun/sky color)");
    println!("  R     - Hold to reverse direction (or Shift)");
    println!();
    println!("Moon orbit: 0.0=rising east, 0.25=zenith, 0.5=setting west, 0.75=below horizon");
    println!();
    println!("Other:");
    println!("  F12   - Take screenshot");
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
        // Sky dome config for deferred pipeline
        .with_resource(SkyDomeConfig {
            enabled: true,
            time_of_day: 0.35, // Mid-morning
            cloud_texture_path: Some("textures/generated/mj_clouds_001.png".to_string()),
            ..Default::default()
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
        .with_resource(TimeControl {
            time_of_day: 0.15,
            moon1_time: 0.25, // Purple moon at zenith
            moon2_time: 0.15, // Orange moon rising
        })
        .with_resource(NeedPlayerSpawn)
        .with_plugin(|app| {
            app.add_plugins(CharacterControllerPlugin);
        })
        .with_update_systems(|app| {
            app.add_systems(PostStartup, (spawn_player_system, init_moon_config_system));
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

    // Scattered glowing light posts for visibility in the dark world
    let light_purple = Voxel::emissive(180, 120, 255); // Purple glow
    let light_orange = Voxel::emissive(255, 160, 80); // Orange glow
    let light_post = Voxel::solid(50, 45, 55); // Dark stone post

    // Place light posts in a grid pattern across the terrain
    for gx in -4i32..=4 {
        for gz in -4i32..=4 {
            // Skip the center tower area
            if gx.abs() <= 1 && gz.abs() <= 1 {
                continue;
            }

            let lx = gx * 40;
            let lz = gz * 40;

            // Get ground height at this position
            let fx = lx as f32;
            let fz = lz as f32;
            let hill1 = ((fx * 0.008).sin() * (fz * 0.006).cos()) * 15.0;
            let hill2 = ((fx * 0.015 + 1.0).cos() * (fz * 0.012 + 2.0).sin()) * 8.0;
            let ground_y = ground_base + (hill1 + hill2) as i32;

            // Build a small light post (3 blocks tall + light on top)
            for y in 0..3 {
                terrain.set_voxel(lx, ground_y + y, lz, light_post);
            }

            // Alternate purple and orange lights in a checkerboard
            let light = if (gx + gz) % 2 == 0 {
                light_purple
            } else {
                light_orange
            };
            terrain.set_voxel(lx, ground_y + 3, lz, light);
        }
    }

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
// Moon Orbit Calculation (matches sky_dome_node.rs)
// ============================================================================

/// Calculate moon direction from orbital time, matching sky_dome_node.rs MoonOrbit.
/// Returns direction TO the moon (positive Y = above horizon).
fn calculate_moon_direction(moon_time: f32, inclination_deg: f32, azimuth_offset_deg: f32) -> Vec3 {
    // Convert time to angle for orbital position
    // At time 0.0: moon rising in east, Y=0
    // At time 0.25: moon at zenith, Y=1
    // At time 0.5: moon setting in west, Y=0
    // At time 0.75: moon at nadir, Y=-1
    let angle = moon_time * TAU;

    // Base orbit: X tracks east-west position, Y tracks altitude
    let base_x = angle.cos(); // East (+1) -> West (-1) -> East (+1)
    let base_y = angle.sin(); // Horizon (0) -> Zenith (+1) -> Horizon (0) -> Nadir (-1)
    let base_z = 0.0;

    // Apply inclination: rotate around X axis (tilts the orbital plane north/south)
    let incline_rad = inclination_deg.to_radians();
    let cos_inc = incline_rad.cos();
    let sin_inc = incline_rad.sin();

    let tilted_x = base_x;
    let tilted_y = base_y * cos_inc - base_z * sin_inc;
    let tilted_z = base_y * sin_inc + base_z * cos_inc;

    // Apply azimuth offset: rotate around Y axis (rotates rise/set direction)
    let azimuth_rad = azimuth_offset_deg.to_radians();
    let cos_az = azimuth_rad.cos();
    let sin_az = azimuth_rad.sin();

    let final_x = tilted_x * cos_az + tilted_z * sin_az;
    let final_y = tilted_y;
    let final_z = -tilted_x * sin_az + tilted_z * cos_az;

    Vec3::new(final_x, final_y, final_z).normalize()
}

/// Get moon1 (purple) direction - uses same orbital params as SkyDomeConfig defaults
fn moon1_direction_from_time(moon_time: f32) -> Vec3 {
    // Match sky_dome_node.rs MoonOrbit defaults for moon1
    calculate_moon_direction(moon_time, 25.0, 15.0)
}

/// Get moon2 (orange) direction - uses same orbital params as SkyDomeConfig defaults
fn moon2_direction_from_time(moon_time: f32) -> Vec3 {
    // Match sky_dome_node.rs MoonOrbit defaults for moon2
    calculate_moon_direction(moon_time, 15.0, -10.0)
}

// ============================================================================
// Resources and Components
// ============================================================================

#[derive(Resource)]
struct TimeControl {
    time_of_day: f32,
    moon1_time: f32,
    moon2_time: f32,
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

/// Initialize MoonConfig with directions matching the starting moon times
fn init_moon_config_system(time_control: Res<TimeControl>, mut moon_config: ResMut<MoonConfig>) {
    // Set initial moon directions based on starting times
    // MoonConfig expects direction FROM moon TO scene (negate the TO-moon direction)
    let dir1 = moon1_direction_from_time(time_control.moon1_time);
    let dir2 = moon2_direction_from_time(time_control.moon2_time);

    moon_config.moon1_direction = -dir1;
    moon_config.moon2_direction = -dir2;

    println!(
        "Initialized MoonConfig: moon1_dir={:?}, moon2_dir={:?}",
        moon_config.moon1_direction, moon_config.moon2_direction
    );
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
    mut sky_config: ResMut<SkyDomeConfig>,
    mut moon_config: ResMut<MoonConfig>,
) {
    let speed = time.delta_secs() * 0.1;

    // R key toggles reverse mode (alternative to Shift which can be finicky)
    // Also check Shift keys
    let reverse = keyboard.pressed(KeyCode::ShiftLeft)
        || keyboard.pressed(KeyCode::ShiftRight)
        || keyboard.pressed(KeyCode::KeyR);

    // T key: Control moon1 (purple) orbital time - forward
    // Shift+T or R+T: reverse
    if keyboard.pressed(KeyCode::KeyT) {
        if reverse {
            time_control.moon1_time -= speed;
        } else {
            time_control.moon1_time += speed;
        }
        time_control.moon1_time = time_control.moon1_time.rem_euclid(1.0);
        sky_config.moon1_time = time_control.moon1_time;

        // Update MoonConfig direction for shadow system
        // MoonConfig expects direction FROM moon TO scene (negate the TO-moon direction)
        let dir_to_moon = moon1_direction_from_time(time_control.moon1_time);
        moon_config.moon1_direction = -dir_to_moon;

        // Print moon position for debugging
        let phase = match time_control.moon1_time {
            t if t < 0.125 => "rising (east)",
            t if t < 0.375 => "high in sky",
            t if t < 0.625 => "setting (west)",
            _ => "below horizon",
        };
        let dir = if reverse { "<-" } else { "->" };
        println!(
            "Moon1 (purple): {:.2} {} {} (dir: {:.2}, {:.2}, {:.2})",
            time_control.moon1_time,
            dir,
            phase,
            moon_config.moon1_direction.x,
            moon_config.moon1_direction.y,
            moon_config.moon1_direction.z
        );
    }

    // Y key: Control moon2 (orange) orbital time
    if keyboard.pressed(KeyCode::KeyY) {
        if reverse {
            time_control.moon2_time -= speed;
        } else {
            time_control.moon2_time += speed;
        }
        time_control.moon2_time = time_control.moon2_time.rem_euclid(1.0);
        sky_config.moon2_time = time_control.moon2_time;

        // Update MoonConfig direction for shadow system
        let dir_to_moon = moon2_direction_from_time(time_control.moon2_time);
        moon_config.moon2_direction = -dir_to_moon;

        let phase = match time_control.moon2_time {
            t if t < 0.125 => "rising (east)",
            t if t < 0.375 => "high in sky",
            t if t < 0.625 => "setting (west)",
            _ => "below horizon",
        };
        let dir = if reverse { "<-" } else { "->" };
        println!(
            "Moon2 (orange): {:.2} {} {} (dir: {:.2}, {:.2}, {:.2})",
            time_control.moon2_time,
            dir,
            phase,
            moon_config.moon2_direction.x,
            moon_config.moon2_direction.y,
            moon_config.moon2_direction.z
        );
    }

    // G key: Control time of day (sun position, sky color)
    if keyboard.pressed(KeyCode::KeyG) {
        if reverse {
            time_control.time_of_day -= speed;
        } else {
            time_control.time_of_day += speed;
        }
        time_control.time_of_day = time_control.time_of_day.rem_euclid(1.0);
        sky_config.time_of_day = time_control.time_of_day;

        let phase = match time_control.time_of_day {
            t if t < 0.125 => "late night",
            t if t < 0.25 => "pre-dawn",
            t if t < 0.375 => "morning",
            t if t < 0.5 => "midday",
            t if t < 0.625 => "afternoon",
            t if t < 0.75 => "evening",
            t if t < 0.875 => "dusk",
            _ => "night",
        };
        println!("Time of day: {:.2} - {}", time_control.time_of_day, phase);
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
