//! Phase 16 Simple Stepped: NO Greedy meshing with stepped terrain
//!
//! Comparison test - same terrain as p16_stepped but without greedy meshing.
//!
//! Run with: `cargo run --example p16_simple_stepped`

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
const SCREENSHOT_PATH: &str = "screenshots/p16_simple_stepped.png";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 16 SIMPLE STEPPED: NO greedy, with stepped terrain...");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1024, 768).into(),
                title: "Phase 16: No Greedy + Stepped".into(),
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

/// Create world with stepped terrain - same as p16_stepped.
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

            for lx in 0..CHUNK_SIZE as i32 {
                for lz in 0..CHUNK_SIZE as i32 {
                    let wx = world_x_start + lx;
                    let wz = world_z_start + lz;

                    // STEPPED terrain
                    let height = 3 + ((wx.abs() + wz.abs()) % 5) as i32;

                    for wy in 0..height {
                        let height_factor = (wy as f32 / height as f32 * 50.0) as u8;
                        let r = base_r.saturating_add(height_factor);
                        let g = base_g.saturating_add(height_factor);
                        let b = base_b.saturating_add(height_factor);
                        world.set_voxel(wx, wy, wz, Voxel::solid(r, g, b));
                    }
                }
            }

            // Add a glowing pillar
            let crystal_x = world_x_start + CHUNK_SIZE as i32 / 2;
            let crystal_z = world_z_start + CHUNK_SIZE as i32 / 2;

            for cy in 5..13 {
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

    // Greedy meshing OFF!
    let config = WorldSpawnConfig {
        use_greedy_meshing: false,  // <-- OFF
        use_cross_chunk_culling: true,
        ..Default::default()
    };

    let result = spawn_world_with_lights_config(&mut commands, &mut meshes, &mut materials, &world, &config);

    println!(
        "Spawned {} chunk meshes + {} point lights (NO greedy)",
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

    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    println!("Setup complete.");
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
