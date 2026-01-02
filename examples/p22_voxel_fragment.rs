//! Phase 22: Voxel Fragment Physics Demo
//!
//! Demonstrates dynamic voxel fragments with physics:
//! - Static terrain with occupancy-based collision (Phase 6)
//! - Falling voxel fragments with physics simulation
//! - Collision between fragments and terrain using voxel occupancy
//!
//! Run with: `cargo run --example p22_voxel_fragment`
//!
//! Press SPACE to spawn a new fragment above the terrain.
//! Press R to reset all fragments.
//! Press B to run benchmark (spawns 1, 2, 4, 8 fragments and measures physics time)
//! Press C to toggle collision system (CPU occupancy vs Rapier trimesh)
//! Press G to toggle GPU collision (GPU compute shader vs CPU)
//!
//! Screenshots saved to: screenshots/voxel_fragment/

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
    GpuCollisionMode, VoxelPhysicsWorld, PhysicsConfig, PhysicsBody, BodyHandle,
    WorldOccupancy, FragmentOccupancy,
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
        // Physics
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(RapierDebugRenderPlugin::default())
        // Voxel rendering
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        .add_plugins(OrbitCameraPlugin)
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
            toggle_unified_physics,
            step_unified_physics,
            sync_unified_physics_to_transforms,
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

/// Resource wrapping VoxelPhysicsWorld for unified physics.
#[derive(Resource)]
struct PhysicsWorldRes(VoxelPhysicsWorld);

/// Component to track body handle in VoxelPhysicsWorld.
#[derive(Component)]
struct PhysicsBodyHandle(BodyHandle);

/// Whether to use VoxelPhysicsWorld (true) or Rapier (false) for fragment physics.
#[derive(Resource)]
struct UseUnifiedPhysics(bool);

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
    
    // Initialize terrain occupancy for fragment collision (Phase 6)
    commands.insert_resource(TerrainOccupancy::from_voxel_world(&terrain));
    
    // Initialize VoxelPhysicsWorld for unified physics (Phase 3+)
    let world_occupancy = WorldOccupancy::from_voxel_world(&terrain);
    let physics_config = PhysicsConfig {
        gravity: Vec3::new(0.0, -25.0, 0.0),
        ..default()
    };
    commands.insert_resource(PhysicsWorldRes(VoxelPhysicsWorld::new(world_occupancy, physics_config)));
    commands.insert_resource(UseUnifiedPhysics(true)); // Enable unified physics by default
    
    // NOTE: We no longer need Rapier terrain collider - VoxelPhysicsWorld handles
    // fragment-terrain collision via occupancy. The trimesh was only needed when
    // using Rapier for physics. Fragment-fragment collision would still need Rapier
    // but we're not implementing that yet (fragments don't collide with each other).
    
    // Create voxel material
    let material = materials.add(VoxelMaterial { ambient: 0.1 });
    commands.insert_resource(VoxelMaterialHandle(material.clone()));
    
    // Spawn terrain mesh (no Rapier collider needed for unified physics)
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
    info!("Press C to toggle occupancy collision on/off");
    info!("Press U to toggle unified physics (VoxelPhysicsWorld vs Rapier)");
    info!("Using unified VoxelPhysicsWorld for fragment physics");
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
    mut physics_world: ResMut<PhysicsWorldRes>,
    use_unified: Res<UseUnifiedPhysics>,
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
        
        // Create fragment occupancy for VoxelPhysicsWorld
        let frag_occupancy = FragmentOccupancy::from_voxel_world(&fragment_data);
        
        if let Some(entity) = spawn_fragment_with_mesh(
            &mut commands,
            &mut meshes,
            fragment_data,
            position,
            impulse,
            material_handle.0.clone(),
        ) {
            let mut entity_commands = commands.entity(entity);
            entity_commands.insert((SpawnedFragment, Name::new("Fragment")));
            
            // If using unified physics, add to VoxelPhysicsWorld and make Rapier kinematic
            if use_unified.0 {
                // Create dynamic body in VoxelPhysicsWorld
                let mut body = PhysicsBody::dynamic(position, frag_occupancy);
                body.velocity = impulse; // Apply initial impulse as velocity
                let handle = physics_world.0.add_body(body);
                
                // Track the handle and switch to kinematic (so Rapier doesn't fight)
                entity_commands.insert(PhysicsBodyHandle(handle));
                entity_commands.insert(RigidBody::KinematicPositionBased);
                
                info!("Spawned unified physics fragment at {:?}", position);
            } else {
                info!("Spawned Rapier fragment at {:?}", position);
            }
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
    terrain: Query<&Collider, Without<SpawnedFragment>>,
) {
    if keyboard.just_pressed(KeyCode::KeyP) {
        info!("=== PHYSICS STATS ===");
        
        // Terrain info
        for collider in terrain.iter() {
            if let Some(trimesh) = collider.as_trimesh() {
                info!("Terrain: {} vertices, {} triangles", 
                    trimesh.vertices().len(),
                    trimesh.indices().len()
                );
            }
        }
        
        // Fragment info
        for (i, (collider, transform, velocity)) in fragments.iter().enumerate() {
            let collider_info = if collider.as_compound().is_some() {
                "compound cuboids".to_string()
            } else if let Some(trimesh) = collider.as_trimesh() {
                format!("{} tris", trimesh.indices().len())
            } else {
                "unknown".to_string()
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

/// Toggle unified physics mode when U is pressed
fn toggle_unified_physics(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut use_unified: ResMut<UseUnifiedPhysics>,
) {
    if keyboard.just_pressed(KeyCode::KeyU) {
        use_unified.0 = !use_unified.0;
        if use_unified.0 {
            info!("Unified physics ENABLED - new fragments use VoxelPhysicsWorld");
        } else {
            info!("Unified physics DISABLED - new fragments use Rapier directly");
        }
    }
}

/// Step the unified physics world
fn step_unified_physics(
    mut physics_world: ResMut<PhysicsWorldRes>,
    time: Res<Time>,
    use_unified: Res<UseUnifiedPhysics>,
) {
    if !use_unified.0 {
        return;
    }
    
    physics_world.0.step(time.delta_secs());
}

/// Sync unified physics state to Transform components
fn sync_unified_physics_to_transforms(
    physics_world: Res<PhysicsWorldRes>,
    mut fragments: Query<(&PhysicsBodyHandle, &mut Transform)>,
    use_unified: Res<UseUnifiedPhysics>,
) {
    if !use_unified.0 {
        return;
    }
    
    for (handle, mut transform) in fragments.iter_mut() {
        if let Some((pos, rot)) = physics_world.0.get_transform(handle.0) {
            transform.translation = pos;
            transform.rotation = rot;
        }
    }
}
