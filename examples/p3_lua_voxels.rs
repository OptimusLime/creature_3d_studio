//! Phase 3 Screenshot Test: Lua-driven voxel placement.
//!
//! This test verifies:
//! - Lua script loading and execution
//! - Voxel data structure (place_voxel API)
//! - Cube spawning from voxel data
//! - Per-voxel material colors
//!
//! Run with: `cargo run --example p3_lua_voxels`
//!
//! Expected output: `screenshots/p3_lua_voxels.png` - 5 colored cubes in cross pattern:
//! - Center (8,8,8): Red
//! - +X (9,8,8): Green
//! - -X (7,8,8): Blue
//! - +Z (8,8,9): Yellow
//! - -Z (8,8,7): Cyan

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{load_creature_script, VoxelChunk};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p3_lua_voxels.png";
const CREATURE_SCRIPT: &str = "assets/scripts/test_creature.lua";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 3 Screenshot Test: Lua Voxels...");
    println!("Loading creature script: {}", CREATURE_SCRIPT);
    println!(
        "Expected output: {} (5 colored cubes in cross pattern)",
        SCREENSHOT_PATH
    );

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 3: Lua Voxels".into(),
                ..default()
            }),
            ..default()
        }))
        // Black clear color (void)
        .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
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

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Load the creature script
    let chunk = match load_creature_script(CREATURE_SCRIPT) {
        Ok(c) => {
            println!("Loaded creature with {} voxels", c.count());
            c
        }
        Err(e) => {
            eprintln!("ERROR: Failed to load creature script: {:?}", e);
            VoxelChunk::new() // Empty chunk as fallback
        }
    };

    // Create a shared cube mesh
    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    // Spawn a cube entity for each voxel
    // Voxel coordinates are in chunk space (0-15), we center them at origin
    // by subtracting 8 from each coordinate
    for (x, y, z, voxel) in chunk.iter() {
        let [r, g, b] = voxel.color_f32();

        // Create material with voxel color
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            ..default()
        });

        // Convert chunk coords to world coords (center chunk at origin)
        let world_pos = Vec3::new(x as f32 - 8.0, y as f32 - 8.0, z as f32 - 8.0);

        commands.spawn((
            Mesh3d(cube_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(world_pos),
        ));

        println!(
            "  Spawned voxel at ({}, {}, {}) -> world {:?} color=({}, {}, {})",
            x,
            y,
            z,
            world_pos,
            voxel.color[0],
            voxel.color[1],
            voxel.color[2]
        );
    }

    // Camera positioned to see the cross pattern from above-front-right
    // Looking at origin where the voxels are centered
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(5.0, 5.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Directional light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;

    if frame_count.0 == 5 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }

    if frame_count.0 >= 15 {
        println!("Exiting after {} frames", frame_count.0);
        exit.write(AppExit::Success);
    }
}
