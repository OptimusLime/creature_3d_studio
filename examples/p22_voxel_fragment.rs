//! Phase 22: Voxel Fragment Physics Demo
//!
//! Demonstrates dynamic voxel fragments with physics:
//! - Static terrain with occupancy-based collision
//! - Falling voxel fragments using our unified PhysicsEngine
//! - All physics goes through physics_math.rs - no external physics libraries
//!
//! Run with: `cargo run --example p22_voxel_fragment`
//!
//! Controls:
//! - SPACE: Spawn a new fragment above the terrain
//! - R: Reset all fragments
//! - B: Run benchmark (spawns 1, 2, 4, 8 fragments and measures physics time)
//! - C: Toggle collision on/off
//! - P: Print physics stats

use bevy::app::AppExit;
use bevy::diagnostic::DiagnosticsStore;
use bevy::ecs::event::EventWriter;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::time::Instant;
use studio_core::{
    build_world_meshes_cross_chunk, spawn_fragment_with_mesh, BenchmarkPlugin,
    DeferredRenderingPlugin, FragmentCollisionConfig, FragmentPhysics, OrbitCamera,
    OrbitCameraPlugin, TerrainOccupancy, Voxel, VoxelFragment, VoxelFragmentPlugin, VoxelMaterial,
    VoxelMaterialPlugin, VoxelWorld,
};

// Simple random number generator state (avoid external dependency)
static mut SEED: u64 = 12345;

fn simple_random() -> f32 {
    unsafe {
        SEED = SEED.wrapping_mul(1103515245).wrapping_add(12345);
        ((SEED >> 16) & 0x7FFF) as f32 / 32767.0
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Phase 22: Voxel Fragment Physics".into(),
                resolution: bevy::window::WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        // Benchmark (includes FrameTimeDiagnosticsPlugin)
        .add_plugins(BenchmarkPlugin)
        // Voxel rendering
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        .add_plugins(OrbitCameraPlugin)
        // VoxelFragmentPlugin provides our unified physics via physics_math.rs
        .add_plugins(VoxelFragmentPlugin)
        // Systems
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                spawn_fragment_on_space,
                reset_fragments,
                run_benchmark,
                log_physics_stats,
                toggle_collision_system,
                auto_spawn_fragments,
                verify_physics_behavior,
            ),
        )
        .insert_resource(FragmentSpawnConfig::default())
        .insert_resource(BenchmarkState::default())
        .insert_resource(VerificationState::default())
        .run();
}

/// State for physics verification
#[derive(Resource)]
struct VerificationState {
    frame_count: u32,
    screenshot_taken: bool,
    verification_done: bool,
    verification_logged: bool,
}

impl Default for VerificationState {
    fn default() -> Self {
        Self {
            frame_count: 0,
            screenshot_taken: false,
            verification_done: false,
            verification_logged: false,
        }
    }
}

/// System to verify physics behavior and take screenshots
#[allow(deprecated)]
fn verify_physics_behavior(
    mut commands: Commands,
    mut state: ResMut<VerificationState>,
    fragments: Query<(&Name, &Transform, &FragmentPhysics), With<SpawnedFragment>>,
    mut app_exit: EventWriter<AppExit>,
) {
    state.frame_count += 1;

    // Take screenshot at frame 300 (5 seconds at 60fps) - fragments should be settled
    if state.frame_count == 300 && !state.screenshot_taken {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("screenshots/p22_physics_verification.png"));
        info!("=== SCREENSHOT CAPTURED at frame 300 ===");
        info!("Saved to: p22_physics_verification.png");
        state.screenshot_taken = true;
    }

    // Verify physics at frame 360 (6 seconds) - should be fully settled
    if state.frame_count == 360 && !state.verification_done {
        state.verification_done = true;

        info!("");
        info!("╔══════════════════════════════════════════════════════════════╗");
        info!("║           PHYSICS VERIFICATION RESULTS (Frame 360)           ║");
        info!("╠══════════════════════════════════════════════════════════════╣");

        let mut ramp_fragment_ok = false;
        let mut floor_fragment_ok = false;
        let mut all_settled = true;

        for (name, transform, physics) in fragments.iter() {
            let pos = transform.translation;
            let speed = physics.velocity.length();
            let ang_speed = physics.angular_velocity.length();
            let is_settled = speed < 0.5 && ang_speed < 0.5;

            if !is_settled {
                all_settled = false;
            }

            info!(
                "║ {:15} pos=({:6.2}, {:6.2}, {:6.2}) vel={:.3} settled={}",
                name.as_str(),
                pos.x,
                pos.y,
                pos.z,
                speed,
                is_settled
            );

            // Check RampFragment - should be on ramp (Y > 4.0, since ramp top is ~5)
            if name.as_str() == "RampFragment" {
                if pos.y > 4.0 && pos.y < 8.0 {
                    ramp_fragment_ok = true;
                    info!(
                        "║   ✓ RED fragment correctly landed on RAMP (Y={:.2} > 4.0)",
                        pos.y
                    );
                } else if pos.y < 4.0 {
                    info!(
                        "║   ✗ RED fragment FELL THROUGH ramp! (Y={:.2} < 4.0)",
                        pos.y
                    );
                } else {
                    info!("║   ? RED fragment position unexpected (Y={:.2})", pos.y);
                }
            }

            // Check FloorFragment - should be on floor (Y ~ 3-5, floor top is at Y=3)
            if name.as_str() == "FloorFragment" {
                if pos.y > 3.0 && pos.y < 6.0 {
                    floor_fragment_ok = true;
                    info!(
                        "║   ✓ BLUE fragment correctly landed on FLOOR (Y={:.2})",
                        pos.y
                    );
                } else if pos.y < 3.0 {
                    info!(
                        "║   ✗ BLUE fragment FELL THROUGH floor! (Y={:.2} < 3.0)",
                        pos.y
                    );
                } else {
                    info!("║   ? BLUE fragment position unexpected (Y={:.2})", pos.y);
                }
            }
        }

        info!("╠══════════════════════════════════════════════════════════════╣");

        // Summary
        let all_ok = ramp_fragment_ok && floor_fragment_ok && all_settled;
        if all_ok {
            info!("║  ✓✓✓ ALL TESTS PASSED - Physics working correctly! ✓✓✓      ║");
        } else {
            info!("║  ISSUES DETECTED:                                           ║");
            if !ramp_fragment_ok {
                info!("║    - Ramp fragment did not land correctly                   ║");
            }
            if !floor_fragment_ok {
                info!("║    - Floor fragment did not land correctly                  ║");
            }
            if !all_settled {
                info!("║    - Some fragments still moving                            ║");
            }
        }

        info!("╠══════════════════════════════════════════════════════════════╣");
        info!("║  Screenshot saved to: p22_physics_verification.png           ║");
        info!("╚══════════════════════════════════════════════════════════════╝");
        info!("");
    }

    // Exit after frame 400 (give time for screenshot to save)
    if state.frame_count >= 400 {
        info!("Auto-exiting after verification complete.");
        app_exit.write(AppExit::Success);
    }
}

/// Configuration for spawning fragments
#[derive(Resource)]
struct FragmentSpawnConfig {
    spawn_height: f32,
    fragment_size: i32,
}

impl Default for FragmentSpawnConfig {
    fn default() -> Self {
        Self {
            spawn_height: 15.0,
            fragment_size: 3,
        }
    }
}

/// Resource to hold the voxel material handle
#[derive(Resource)]
struct VoxelMaterialHandle(Handle<VoxelMaterial>);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    // Create terrain - USE UNIFORM COLORS for greedy meshing to work!
    let mut terrain = VoxelWorld::new();

    // Ground platform (20x20, 3 blocks thick) - SINGLE COLOR
    let ground_color = Voxel::solid(70, 70, 80);
    for x in -10..10 {
        for z in -10..10 {
            for y in 0..3 {
                terrain.set_voxel(x, y, z, ground_color);
            }
        }
    }

    // Add some pillars for interesting collisions - SINGLE COLOR per pillar
    let pillar_color = Voxel::solid(100, 60, 60);
    for (px, pz) in [(-5, -5), (5, -5), (-5, 5), (5, 5)] {
        for y in 3..8 {
            terrain.set_voxel(px, y, pz, pillar_color);
        }
    }

    // Center ramp - SINGLE COLOR
    let ramp_color = Voxel::solid(60, 100, 60);
    for x in -2..3 {
        for z in -2..3 {
            let height = 3 + (x + 2) as i32;
            for y in 3..height {
                terrain.set_voxel(x, y, z, ramp_color);
            }
        }
    }

    // Initialize terrain occupancy for fragment collision
    // This is used by VoxelFragmentPlugin's collision systems
    commands.insert_resource(TerrainOccupancy::from_voxel_world(&terrain));

    // Create voxel material
    let material = materials.add(VoxelMaterial { ambient: 0.1 });
    commands.insert_resource(VoxelMaterialHandle(material.clone()));

    // Spawn terrain mesh
    // Note: No Rapier terrain collider needed - GPU handles terrain collision
    let chunk_meshes = build_world_meshes_cross_chunk(&terrain);

    commands
        .spawn((
            Name::new("Terrain"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
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

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(25.0, 20.0, 25.0).looking_at(Vec3::new(0.0, 5.0, 0.0), Vec3::Y),
        OrbitCamera {
            target: Vec3::new(0.0, 5.0, 0.0),
            distance: 35.0,
            ..default()
        },
    ));

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: 15000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        affects_lightmapped_meshes: false,
    });

    info!("Press SPACE to spawn a fragment, R to reset, P for physics stats");
    info!("Press C to toggle collision on/off");
    info!("Using unified PhysicsEngine - all physics through physics_math.rs");

    // AUTO-SPAWN: Insert resource to spawn test fragments after a short delay
    commands.insert_resource(AutoSpawnState { frames_waited: 0 });
}

/// State for auto-spawning test fragments
#[derive(Resource)]
struct AutoSpawnState {
    frames_waited: u32,
}

/// Auto-spawn test fragments after terrain is loaded
fn auto_spawn_fragments(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: ResMut<AutoSpawnState>,
    material_handle: Option<Res<VoxelMaterialHandle>>,
) {
    // Only run once, after 10 frames to ensure terrain is loaded
    state.frames_waited += 1;
    if state.frames_waited != 10 {
        return;
    }

    let Some(material_handle) = material_handle else {
        return;
    };

    info!("AUTO-SPAWN: Spawning test fragments over ramp and floor");

    // Spawn fragment directly over ramp center (should land on ramp at Y~5)
    let mut ramp_fragment = VoxelWorld::new();
    for x in 0..3 {
        for y in 0..3 {
            for z in 0..3 {
                ramp_fragment.set_voxel(x, y, z, Voxel::solid(255, 100, 100)); // Red
            }
        }
    }
    if let Some(entity) = spawn_fragment_with_mesh(
        &mut commands,
        &mut meshes,
        ramp_fragment,
        Vec3::new(0.5, 15.0, 0.5), // Over ramp center
        Vec3::new(0.0, -5.0, 0.0), // Downward impulse
        material_handle.0.clone(),
    ) {
        commands
            .entity(entity)
            .insert((SpawnedFragment, Name::new("RampFragment")));
        info!("  Spawned RED fragment at (0.5, 15.0, 0.5) - should land on ramp");
    }

    // Spawn fragment over floor only (should land on floor at Y~3)
    let mut floor_fragment = VoxelWorld::new();
    for x in 0..3 {
        for y in 0..3 {
            for z in 0..3 {
                floor_fragment.set_voxel(x, y, z, Voxel::solid(100, 100, 255)); // Blue
            }
        }
    }
    if let Some(entity) = spawn_fragment_with_mesh(
        &mut commands,
        &mut meshes,
        floor_fragment,
        Vec3::new(-8.0, 15.0, -8.0), // Over floor, away from ramp
        Vec3::new(0.0, -5.0, 0.0),   // Downward impulse
        material_handle.0.clone(),
    ) {
        commands
            .entity(entity)
            .insert((SpawnedFragment, Name::new("FloorFragment")));
        info!("  Spawned BLUE fragment at (-8.0, 15.0, -8.0) - should land on floor");
    }
}

/// Marker component for spawned fragments
#[derive(Component)]
struct SpawnedFragment;

/// Spawn a new fragment when space is pressed
fn spawn_fragment_on_space(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    config: Res<FragmentSpawnConfig>,
    material_handle: Res<VoxelMaterialHandle>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        // Create a small voxel fragment
        let mut fragment_data = VoxelWorld::new();
        let size = config.fragment_size;

        // Random color for this fragment
        let r = (simple_random() * 155.0 + 100.0) as u8;
        let g = (simple_random() * 155.0 + 100.0) as u8;
        let b = (simple_random() * 155.0 + 100.0) as u8;
        let color = Voxel::solid(r, g, b);

        for x in 0..size {
            for y in 0..size {
                for z in 0..size {
                    fragment_data.set_voxel(x, y, z, color);
                }
            }
        }

        // Random horizontal position
        let x = (simple_random() - 0.5) * 10.0;
        let z = (simple_random() - 0.5) * 10.0;
        let position = Vec3::new(x, config.spawn_height, z);

        // Small random impulse
        let impulse = Vec3::new(
            (simple_random() - 0.5) * 2.0,
            -5.0, // downward
            (simple_random() - 0.5) * 2.0,
        );

        // spawn_fragment_with_mesh creates:
        // - VoxelFragment component (with occupancy for collision)
        // - FragmentPhysics component (velocity, angular_velocity, mass)
        // - Mesh + Material children (for rendering)
        //
        // VoxelFragmentPlugin's fragment_terrain_collision_system handles all physics
        // using the unified PhysicsEngine in physics_math.rs
        if let Some(entity) = spawn_fragment_with_mesh(
            &mut commands,
            &mut meshes,
            fragment_data,
            position,
            impulse,
            material_handle.0.clone(),
        ) {
            commands
                .entity(entity)
                .insert((SpawnedFragment, Name::new("Fragment")));
            info!("Spawned fragment at {:?}", position);
        }
    }
}

/// Reset all fragments when R is pressed
fn reset_fragments(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    fragments: Query<Entity, With<SpawnedFragment>>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        let count = fragments.iter().count();
        for entity in fragments.iter() {
            commands.entity(entity).despawn();
        }
        info!("Reset {} fragments", count);
    }
}

/// Log detailed physics stats
fn log_physics_stats(
    keyboard: Res<ButtonInput<KeyCode>>,
    fragments: Query<(&VoxelFragment, &Transform, &FragmentPhysics), With<SpawnedFragment>>,
    collision_config: Res<FragmentCollisionConfig>,
) {
    if keyboard.just_pressed(KeyCode::KeyP) {
        info!("=== PHYSICS STATS ===");
        info!("Physics engine: unified PhysicsEngine (physics_math.rs)");
        info!(
            "Collision: {}",
            if collision_config.enabled {
                "ENABLED"
            } else {
                "DISABLED"
            }
        );
        info!("Fragment count: {}", fragments.iter().count());

        // Fragment info
        for (i, (_fragment, transform, physics)) in fragments.iter().enumerate() {
            info!(
                "Fragment {}: pos={:?}, vel={:.2}, ang_vel={:.2}",
                i,
                transform.translation,
                physics.velocity.length(),
                physics.angular_velocity.length()
            );
        }
    }
}

/// Benchmark state
#[derive(Resource, Default)]
struct BenchmarkState {
    running: bool,
    stage: usize,
    start_time: Option<Instant>,
    results: Vec<(usize, f64)>, // (fragment_count, avg_frame_time_ms)
    frames_in_stage: u32,
    accumulated_time: f64,
}

/// Run benchmark when B is pressed
fn run_benchmark(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    material_handle: Res<VoxelMaterialHandle>,
    mut state: ResMut<BenchmarkState>,
    fragments: Query<Entity, With<SpawnedFragment>>,
    diagnostics: Res<DiagnosticsStore>,
    _time: Res<Time>,
) {
    // Start benchmark
    if keyboard.just_pressed(KeyCode::KeyB) && !state.running {
        info!("=== STARTING PHYSICS BENCHMARK ===");
        state.running = true;
        state.stage = 0;
        state.results.clear();
        state.frames_in_stage = 0;
        state.accumulated_time = 0.0;

        // Clear existing fragments
        for entity in fragments.iter() {
            commands.entity(entity).despawn();
        }
    }

    if !state.running {
        return;
    }

    let stages = [1, 2, 4, 8, 16];

    if state.stage >= stages.len() {
        // Benchmark complete
        info!("=== BENCHMARK RESULTS ===");
        for (count, time) in &state.results {
            info!("  {} fragments: {:.2}ms avg frame time", count, time);
        }
        state.running = false;
        return;
    }

    let target_fragments = stages[state.stage];
    let current_fragments = fragments.iter().count();

    // Spawn fragments if needed
    if current_fragments < target_fragments {
        let mut fragment_data = VoxelWorld::new();
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    fragment_data.set_voxel(x, y, z, Voxel::solid(200, 100, 100));
                }
            }
        }

        let x = (simple_random() - 0.5) * 8.0;
        let z = (simple_random() - 0.5) * 8.0;
        let position = Vec3::new(x, 12.0, z);

        if let Some(entity) = spawn_fragment_with_mesh(
            &mut commands,
            &mut meshes,
            fragment_data,
            position,
            Vec3::ZERO,
            material_handle.0.clone(),
        ) {
            commands.entity(entity).insert(SpawnedFragment);
        }

        // Reset timing for this stage
        state.start_time = Some(Instant::now());
        state.frames_in_stage = 0;
        state.accumulated_time = 0.0;
        return;
    }

    // Measure frame time
    if let Some(frame_time) = diagnostics
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
    {
        state.accumulated_time += frame_time;
        state.frames_in_stage += 1;
    }

    // Run each stage for 3 seconds
    if let Some(start) = state.start_time {
        if start.elapsed().as_secs_f32() > 3.0 {
            let avg_time = if state.frames_in_stage > 0 {
                state.accumulated_time / state.frames_in_stage as f64
            } else {
                0.0
            };

            info!(
                "Stage {}: {} fragments, {:.2}ms avg frame time ({} frames)",
                state.stage, target_fragments, avg_time, state.frames_in_stage
            );

            state.results.push((target_fragments, avg_time));
            state.stage += 1;
            state.start_time = None;
        }
    }
}

/// Toggle collision system on/off when C is pressed
fn toggle_collision_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<FragmentCollisionConfig>,
) {
    if keyboard.just_pressed(KeyCode::KeyC) {
        config.enabled = !config.enabled;
        if config.enabled {
            info!("Collision ENABLED - fragments collide with terrain");
        } else {
            info!("Collision DISABLED - fragments fall through terrain");
        }
    }
}
