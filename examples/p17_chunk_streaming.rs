//! Phase 17: Chunk Streaming Test
//!
//! Demonstrates chunk streaming with distance-based loading.
//! This example uses the ChunkStreamingPlugin directly since streaming
//! requires runtime chunk management beyond what VoxelWorldApp provides.
//!
//! Controls:
//! - WASD / Arrow keys: Move camera
//! - Q/E: Move up/down
//!
//! Run with: `cargo run --example p17_chunk_streaming`
//!
//! Expected output: `screenshots/p17_chunk_streaming.png`

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    chunk_streaming_system, ChunkManager, ChunkMaterialHandle, ChunkStreamingConfig,
    DeferredCamera, DeferredPointLight, DeferredRenderingPlugin, Voxel, VoxelMaterial,
    VoxelMaterialPlugin, VoxelWorld, CHUNK_SIZE,
};

const SCREENSHOT_PATH: &str = "screenshots/p17_chunk_streaming.png";

fn main() {
    std::fs::create_dir_all("screenshots").expect("Failed to create screenshots directory");

    println!("Running Phase 17: Chunk Streaming Test...");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1024, 768).into(),
                title: "Phase 17: Chunk Streaming".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        .insert_resource(ClearColor(Color::srgb(0.102, 0.039, 0.180)))
        .insert_resource(FrameCount(0))
        .insert_resource(ChunkStreamingConfig {
            load_radius: 6,  // Larger radius to load more chunks
            unload_radius: 8,
            max_loads_per_frame: 4,  // Faster loading for screenshot
            max_unloads_per_frame: 4,
            use_greedy_meshing: true,  // Re-enabled: SSAO fixes the vertex AO interpolation bug
            y_range: Some((-1, 1)),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (chunk_streaming_system, capture_and_exit))
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

fn create_large_world() -> VoxelWorld {
    let mut world = VoxelWorld::new();

    // 8x1x8 grid of chunks with procedural terrain
    for cx in 0..8 {
        for cz in 0..8 {
            let world_x_start = cx * CHUNK_SIZE as i32;
            let world_z_start = cz * CHUNK_SIZE as i32;

            // Color varies by chunk position
            let hue = ((cx + cz * 8) as f32 / 64.0) * 360.0;
            let (r, g, b) = hue_to_rgb(hue);

            for lx in 0..CHUNK_SIZE as i32 {
                for lz in 0..CHUNK_SIZE as i32 {
                    let wx = world_x_start + lx;
                    let wz = world_z_start + lz;

                    // Height varies based on distance from chunk center
                    let cx_center = world_x_start + CHUNK_SIZE as i32 / 2;
                    let cz_center = world_z_start + CHUNK_SIZE as i32 / 2;
                    let dist = ((wx - cx_center).pow(2) + (wz - cz_center).pow(2)) as f32;
                    let height = (5.0 - dist.sqrt() / 4.0).max(1.0) as i32;

                    for wy in 0..height {
                        let factor = (wy as f32 / height as f32 * 0.5 + 0.5).min(1.0);
                        let vr = (r as f32 * factor) as u8;
                        let vg = (g as f32 * factor) as u8;
                        let vb = (b as f32 * factor) as u8;
                        world.set_voxel(wx, wy, wz, Voxel::solid(vr, vg, vb));
                    }
                }
            }

            // Glowing pillar at center of each chunk
            let pillar_x = world_x_start + CHUNK_SIZE as i32 / 2;
            let pillar_z = world_z_start + CHUNK_SIZE as i32 / 2;
            for py in 3..8 {
                world.set_voxel(pillar_x, py, pillar_z, Voxel::new(r, g, b, 200));
            }
        }
    }

    world
}

fn hue_to_rgb(hue: f32) -> (u8, u8, u8) {
    let h = hue / 60.0;
    let x = (1.0 - (h % 2.0 - 1.0).abs()) * 255.0;
    let c = 200.0;

    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    ((r + 55.0) as u8, (g + 55.0) as u8, (b + 55.0) as u8)
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<VoxelMaterial>>) {
    let world = create_large_world();
    println!(
        "World created: {} chunks, {} voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );

    // Shared material for all chunks
    let material = materials.add(VoxelMaterial::default());
    commands.insert_resource(ChunkMaterialHandle(material));
    commands.insert_resource(ChunkManager::new(world));

    // Camera - positioned to see the streaming world from an isometric angle
    // The camera Y position is used for chunk loading calculations,
    // so keep Y reasonable while using a higher viewing angle
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(160.0, 80.0, 160.0).looking_at(Vec3::new(128.0, 0.0, 128.0), Vec3::Y),
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

    // Spawn DeferredPointLights for each glowing pillar (at center of each chunk)
    // The emissive voxels alone don't cast light - we need actual light entities
    // Must use DeferredPointLight (not Bevy's PointLight) for deferred rendering
    for cx in 0..8 {
        for cz in 0..8 {
            let pillar_x = cx * CHUNK_SIZE as i32 + CHUNK_SIZE as i32 / 2;
            let pillar_z = cz * CHUNK_SIZE as i32 + CHUNK_SIZE as i32 / 2;
            
            // Color matches the chunk's hue
            let hue = ((cx + cz * 8) as f32 / 64.0) * 360.0;
            let (r, g, b) = hue_to_rgb(hue);
            
            commands.spawn((
                DeferredPointLight {
                    color: Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
                    intensity: 80.0,
                    radius: 20.0,
                },
                Transform::from_xyz(pillar_x as f32 + 0.5, 6.0, pillar_z as f32 + 0.5),
            ));
        }
    }
    println!("Spawned 64 deferred point lights for glowing pillars");

    println!("Streaming world setup complete.");
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
    manager: Res<ChunkManager>,
) {
    frame_count.0 += 1;

    // Print stats periodically
    if frame_count.0 % 30 == 0 {
        println!(
            "Frame {}: {} chunks loaded",
            frame_count.0, manager.stats.loaded_count
        );
    }

    // Capture screenshot after chunks have loaded
    if frame_count.0 == 60 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }

    if frame_count.0 >= 80 {
        println!("Exiting after {} frames", frame_count.0);
        exit.write(AppExit::Success);
    }
}
