//! Phase 22: Voxel Fragment Physics Demo
//!
//! Demonstrates dynamic voxel fragments with physics:
//! - Static terrain with occupancy-based collision
//! - Falling voxel fragments with Rapier physics simulation
//! - Collision between fragments and terrain using GPU compute shader
//!
//! Run with: `cargo run --example p22_voxel_fragment`
//!
//! Controls:
//! - SPACE: Spawn a new fragment above the terrain
//! - R: Reset all fragments
//! - B: Run benchmark (spawns 1, 2, 4, 8 fragments and measures physics time)
//! - C: Toggle occupancy collision on/off
//! - G: Toggle GPU collision (GPU compute shader vs CPU)
//! - P: Print physics stats

use bevy::prelude::*;
use bevy::diagnostic::DiagnosticsStore;
use bevy_rapier3d::prelude::*;
use std::time::Instant;
use studio_core::{
    spawn_fragment_with_mesh, Voxel, VoxelFragmentPlugin,
    VoxelMaterial, VoxelMaterialPlugin, VoxelWorld,
    build_world_meshes_cross_chunk, DeferredRenderingPlugin,
    OrbitCameraPlugin, OrbitCamera,
    BenchmarkPlugin, TerrainOccupancy, FragmentCollisionConfig,
    GpuCollisionMode,
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
        // Physics - Rapier handles fragment dynamics
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(RapierDebugRenderPlugin::default())
        // Voxel rendering
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        .add_plugins(OrbitCameraPlugin)
        // VoxelFragmentPlugin provides GPU collision system
        .add_plugins(VoxelFragmentPlugin)
        // Systems
        .add_systems(Startup, setup)
        .add_systems(Update, (
            spawn_fragment_on_space, 
            reset_fragments,
            run_benchmark,
            log_physics_stats,
            toggle_collision_system,
            toggle_gpu_collision,
        ))
        .insert_resource(FragmentSpawnConfig::default())
        .insert_resource(BenchmarkState::default())
        .run();
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
    
    // Initialize terrain occupancy for GPU fragment collision
    // This is used by VoxelFragmentPlugin's collision systems
    commands.insert_resource(TerrainOccupancy::from_voxel_world(&terrain));
    
    // Enable GPU collision by default
    commands.insert_resource(GpuCollisionMode { enabled: true });
    
    // Create voxel material
    let material = materials.add(VoxelMaterial { ambient: 0.1 });
    commands.insert_resource(VoxelMaterialHandle(material.clone()));
    
    // Spawn terrain mesh
    // Note: No Rapier terrain collider needed - GPU handles terrain collision
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
    info!("Press C to toggle occupancy collision, G to toggle GPU collision");
    info!("GPU collision enabled - fragments collide with terrain via compute shader");
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
        // - VoxelFragment component (with occupancy for GPU collision)
        // - Rapier RigidBody::Dynamic + Collider (for physics simulation)
        // - Mesh + Material children (for rendering)
        //
        // The VoxelFragmentPlugin's gpu_fragment_terrain_collision_system
        // will detect terrain collision and apply response forces
        if let Some(entity) = spawn_fragment_with_mesh(
            &mut commands,
            &mut meshes,
            fragment_data,
            position,
            impulse,
            material_handle.0.clone(),
        ) {
            commands.entity(entity).insert((SpawnedFragment, Name::new("Fragment")));
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
    fragments: Query<(&Collider, &Transform, &Velocity), With<SpawnedFragment>>,
    gpu_mode: Res<GpuCollisionMode>,
    collision_config: Res<FragmentCollisionConfig>,
) {
    if keyboard.just_pressed(KeyCode::KeyP) {
        info!("=== PHYSICS STATS ===");
        info!("GPU collision: {}", if gpu_mode.enabled { "ENABLED" } else { "DISABLED" });
        info!("Occupancy collision: {}", if collision_config.enabled { "ENABLED" } else { "DISABLED" });
        info!("Fragment count: {}", fragments.iter().count());
        
        // Fragment info
        for (i, (collider, transform, velocity)) in fragments.iter().enumerate() {
            let collider_info = if collider.as_compound().is_some() {
                "compound cuboids".to_string()
            } else if let Some(trimesh) = collider.as_trimesh() {
                format!("{} tris", trimesh.indices().len())
            } else {
                "cuboid".to_string()
            };
            
            info!(
                "Fragment {}: {}, pos={:?}, vel={:.2}",
                i,
                collider_info,
                transform.translation,
                velocity.linvel.length()
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
            info!("Occupancy collision ENABLED - fragments collide with terrain voxels");
        } else {
            info!("Occupancy collision DISABLED - fragments only use Rapier colliders");
        }
    }
}

/// Toggle GPU collision mode when G is pressed
fn toggle_gpu_collision(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut gpu_mode: ResMut<GpuCollisionMode>,
) {
    if keyboard.just_pressed(KeyCode::KeyG) {
        gpu_mode.enabled = !gpu_mode.enabled;
        if gpu_mode.enabled {
            info!("GPU collision ENABLED - using compute shader for collision detection");
        } else {
            info!("GPU collision DISABLED - using CPU collision detection");
        }
    }
}
