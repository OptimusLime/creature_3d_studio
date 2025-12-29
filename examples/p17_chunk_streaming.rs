//! Phase 17: Chunk Streaming Test
//!
//! This example demonstrates chunk streaming:
//! - Large world (8x1x8 = 64 chunks)
//! - Only chunks near camera are loaded
//! - Camera movement triggers load/unload
//! - Demonstrates rate-limited loading
//!
//! Controls:
//! - WASD / Arrow keys: Move camera
//! - Q/E: Move up/down
//! - Mouse drag: Orbit camera
//!
//! Run with: `cargo run --example p17_chunk_streaming`
//!
//! Expected output: `screenshots/p17_chunk_streaming.png`
//! - Visible chunks around camera position
//! - Console logs showing chunk load/unload activity

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use bevy::prelude::MessageReader;
use studio_core::{
    chunk_streaming_system, ChunkManager, ChunkMaterialHandle, ChunkStreamingConfig, DeferredCamera,
    DeferredRenderingPlugin, Voxel, VoxelMaterial, VoxelMaterialPlugin, VoxelWorld, CHUNK_SIZE,
};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p17_chunk_streaming.png";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 17: Chunk Streaming Test...");
    println!("Creating 8x1x8 world ({} chunks)", 8 * 8);

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
        // Fog color as clear color
        .insert_resource(ClearColor(Color::srgb(0.102, 0.039, 0.180)))
        .insert_resource(FrameCount(0))
        // Streaming configuration
        .insert_resource(ChunkStreamingConfig {
            load_radius: 3, // Load chunks within 3 chunk-lengths (~96 units)
            unload_radius: 5, // Unload beyond 5 chunk-lengths (~160 units)
            max_loads_per_frame: 2, // Load 2 per frame
            max_unloads_per_frame: 4,
            use_greedy_meshing: true,
            y_range: Some((-1, 1)), // Load Y=-1, 0, 1 chunks
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                chunk_streaming_system,
                camera_controller,
                capture_and_exit,
                print_stats,
            ),
        )
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

/// Create a large procedural world (8x8 chunks).
fn create_large_world() -> VoxelWorld {
    let mut world = VoxelWorld::new();

    // Create an 8x1x8 grid of chunks (64 chunks total)
    // Each chunk will have distinct terrain based on position
    for cx in 0..8 {
        for cz in 0..8 {
            // World coordinates for this chunk
            let world_x_start = cx * CHUNK_SIZE as i32;
            let world_z_start = cz * CHUNK_SIZE as i32;

            // Base color varies by chunk position
            let hue = ((cx + cz * 8) as f32 / 64.0) * 360.0;
            let (r, g, b) = hue_to_rgb(hue);

            // Create terrain in this chunk
            for lx in 0..CHUNK_SIZE as i32 {
                for lz in 0..CHUNK_SIZE as i32 {
                    let wx = world_x_start + lx;
                    let wz = world_z_start + lz;

                    // Height varies based on distance from center of chunk
                    let cx_center = world_x_start + CHUNK_SIZE as i32 / 2;
                    let cz_center = world_z_start + CHUNK_SIZE as i32 / 2;
                    let dist = ((wx - cx_center).pow(2) + (wz - cz_center).pow(2)) as f32;
                    let height = (5.0 - dist.sqrt() / 4.0).max(1.0) as i32;

                    // Fill from y=0 up to height
                    for wy in 0..height {
                        // Darken lower layers
                        let factor = (wy as f32 / height as f32 * 0.5 + 0.5).min(1.0);
                        let vr = (r as f32 * factor) as u8;
                        let vg = (g as f32 * factor) as u8;
                        let vb = (b as f32 * factor) as u8;

                        world.set_voxel(wx, wy, wz, Voxel::solid(vr, vg, vb));
                    }
                }
            }

            // Add a glowing pillar at center of each chunk
            let pillar_x = world_x_start + CHUNK_SIZE as i32 / 2;
            let pillar_z = world_z_start + CHUNK_SIZE as i32 / 2;

            for py in 3..8 {
                world.set_voxel(pillar_x, py, pillar_z, Voxel::new(r, g, b, 200));
            }
        }
    }

    world
}

/// Convert HSL hue (0-360) to RGB.
fn hue_to_rgb(hue: f32) -> (u8, u8, u8) {
    let h = hue / 60.0;
    let x = (1.0 - (h % 2.0 - 1.0).abs()) * 255.0;
    let c = 200.0; // Saturation

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

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    // Create the large world
    let world = create_large_world();

    println!(
        "World created: {} chunks, {} total voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );

    // Print chunk bounds
    if let Some((min, max)) = world.chunk_bounds() {
        println!(
            "Chunk bounds: ({}, {}, {}) to ({}, {}, {})",
            min.x, min.y, min.z, max.x, max.y, max.z
        );
    }

    // Shared material for all chunks - must be inserted as resource for streaming system
    let material = materials.add(VoxelMaterial::default());
    commands.insert_resource(ChunkMaterialHandle(material));

    // Insert chunk manager (streaming system will load chunks as needed)
    commands.insert_resource(ChunkManager::new(world));

    // Camera positioned at center of the world, looking at terrain
    // World spans 0 to 256 in X and Z (8 chunks * 32 each)
    // Position camera at an angle to see terrain clearly
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(128.0, 25.0, 180.0).looking_at(Vec3::new(128.0, 5.0, 128.0), Vec3::Y),
        DeferredCamera,
        CameraController::default(),
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

    println!("Streaming world setup complete. Chunks will load as camera moves.");
}

/// Simple camera controller component.
#[derive(Component)]
struct CameraController {
    move_speed: f32,
    look_speed: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            move_speed: 50.0,
            look_speed: 0.003,
        }
    }
}

fn camera_controller(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut query: Query<(&CameraController, &mut Transform)>,
) {
    let Ok((controller, mut transform)) = query.single_mut() else {
        return;
    };

    let dt = time.delta_secs();
    let speed = controller.move_speed * dt;

    // Get forward and right vectors (projected to horizontal plane)
    let forward = transform.forward().as_vec3();
    let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let right = forward_flat.cross(Vec3::Y);

    // WASD movement
    let mut movement = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        movement += forward_flat;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        movement -= forward_flat;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        movement -= right;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        movement += right;
    }
    if keys.pressed(KeyCode::KeyQ) {
        movement += Vec3::Y;
    }
    if keys.pressed(KeyCode::KeyE) {
        movement -= Vec3::Y;
    }

    if movement.length_squared() > 0.0 {
        transform.translation += movement.normalize() * speed;
    }

    // Mouse look (only when right mouse button held)
    if mouse_button.pressed(MouseButton::Right) {
        let mut delta = Vec2::ZERO;
        for ev in mouse_motion.read() {
            delta += ev.delta;
        }

        if delta.length_squared() > 0.0 {
            let yaw = -delta.x * controller.look_speed;
            let pitch = -delta.y * controller.look_speed;

            transform.rotate_y(yaw);
            transform.rotate_local_x(pitch);
        }
    } else {
        // Clear events if not using them
        mouse_motion.clear();
    }
}

fn print_stats(manager: Res<ChunkManager>, frame_count: Res<FrameCount>) {
    // Print stats every 30 frames
    if frame_count.0 % 30 == 0 {
        let stats = &manager.stats;
        println!(
            "Frame {}: {} chunks loaded, camera at chunk ({}, {}, {})",
            frame_count.0,
            stats.loaded_count,
            stats.camera_chunk.x,
            stats.camera_chunk.y,
            stats.camera_chunk.z
        );
    }
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;

    // Give more time for chunks to load
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
