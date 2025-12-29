//! Phase 16 Greedy: Multi-Chunk World Test WITH Greedy Meshing
//!
//! Same flat terrain as p16_simple but WITH greedy meshing enabled.
//! Compare with p16_multi_chunk_simple to see AO artifacts.
//!
//! Run with: `cargo run --example p16_multi_chunk_greedy`

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    spawn_world_with_lights_config, CameraPreset, DeferredCamera, DeferredRenderingPlugin, Voxel,
    VoxelMaterialPlugin, VoxelWorld, WorldSpawnConfig, CHUNK_SIZE,
};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p16_multi_chunk_greedy.png";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 16 GREEDY: Multi-Chunk WITH Greedy Meshing...");
    println!("Compare with p16_multi_chunk_simple to see AO artifacts.");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1024, 768).into(),
                title: "Phase 16 Greedy: WITH Greedy Meshing".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        .insert_resource(ClearColor(Color::srgb(0.102, 0.039, 0.180)))
        .insert_resource(FrameCount(0))
        .add_systems(Startup, setup)
        .add_systems(Update, capture_and_exit)
        .run();

    if Path::new(SCREENSHOT_PATH).exists() {
        println!("SUCCESS: Screenshot saved to {}", SCREENSHOT_PATH);
    } else {
        println!("FAILED: Screenshot was not created at {}", SCREENSHOT_PATH);
        std::process::exit(1);
    }
}

#[derive(Resource)]
struct FrameCount(u32);

/// Create identical world to p16_simple - flat terrain.
fn create_world() -> VoxelWorld {
    let mut world = VoxelWorld::new();

    let chunk_colors: [[(u8, u8, u8); 2]; 2] = [
        [(180, 60, 60), (60, 180, 60)],
        [(60, 60, 180), (180, 180, 60)],
    ];

    for cx in 0..=1 {
        for cz in 0..=1 {
            let (base_r, base_g, base_b) = chunk_colors[cz as usize][cx as usize];
            let world_x_start = cx * CHUNK_SIZE as i32;
            let world_z_start = cz * CHUNK_SIZE as i32;

            // FLAT terrain - constant height of 3
            for lx in 0..CHUNK_SIZE as i32 {
                for lz in 0..CHUNK_SIZE as i32 {
                    let wx = world_x_start + lx;
                    let wz = world_z_start + lz;

                    for wy in 0..3 {
                        world.set_voxel(wx, wy, wz, Voxel::solid(base_r, base_g, base_b));
                    }
                }
            }

            // Add a glowing pillar in center of each chunk
            let crystal_x = world_x_start + CHUNK_SIZE as i32 / 2;
            let crystal_z = world_z_start + CHUNK_SIZE as i32 / 2;

            for cy in 3..10 {
                let crystal_r = (base_r as u16 * 3 / 2).min(255) as u8;
                let crystal_g = (base_g as u16 * 3 / 2).min(255) as u8;
                let crystal_b = (base_b as u16 * 3 / 2).min(255) as u8;
                world.set_voxel(crystal_x, cy, crystal_z, Voxel::new(crystal_r, crystal_g, crystal_b, 200));
            }
        }
    }

    world
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<studio_core::VoxelMaterial>>,
) {
    let world = create_world();

    println!(
        "World created: {} chunks, {} total voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );

    // KEY DIFFERENCE: Enable greedy meshing!
    let config = WorldSpawnConfig {
        use_greedy_meshing: true,  // <-- ENABLED
        use_cross_chunk_culling: true,
        ..Default::default()
    };

    let result = spawn_world_with_lights_config(&mut commands, &mut meshes, &mut materials, &world, &config);

    println!(
        "Spawned {} chunk meshes + {} point lights (WITH greedy meshing)",
        result.chunk_entities.len(),
        result.light_entities.len(),
    );

    // Camera
    let world_center = Vec3::new(32.0, 5.0, 32.0);
    let camera = CameraPreset::isometric(world_center, 60.0);
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_translation(camera.position).looking_at(camera.look_at, Vec3::Y),
        DeferredCamera,
    ));

    // Directional light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    println!("Setup complete. This should show AO artifacts on flat terrain.");
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;

    if frame_count.0 == 15 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }

    if frame_count.0 >= 25 {
        println!("Exiting after {} frames", frame_count.0);
        exit.write(AppExit::Success);
    }
}
