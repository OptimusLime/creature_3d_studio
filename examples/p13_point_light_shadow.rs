//! Phase 13: Point Light Shadow Test
//!
//! A minimal test scene to verify point light shadows work correctly:
//! - Flat ground plane
//! - One point light above the center
//! - One occluder (pillar) that should cast a shadow
//!
//! The test validates that:
//! 1. Point light illuminates the ground
//! 2. The pillar blocks light, creating a shadow
//! 3. Shadow has correct shape (elongated away from light)
//!
//! Run with: `cargo run --example p13_point_light_shadow`

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    build_chunk_mesh, DeferredCamera, DeferredPointLight, DeferredRenderable,
    DeferredRenderingPlugin, Voxel, VoxelChunk, VoxelMaterial, VoxelMaterialPlugin, CHUNK_SIZE,
};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p13_point_light_shadow.png";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 13: Point Light Shadow Test...");
    println!("This test verifies point lights cast shadows from occluders.");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 13: Point Light Shadow Test".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        // Very dark background to see point light clearly
        .insert_resource(ClearColor(Color::srgb(0.01, 0.01, 0.02)))
        .insert_resource(FrameCount(0))
        .insert_resource(AutoExit(true))
        .add_systems(Startup, setup)
        .add_systems(Update, (capture_and_exit, toggle_auto_exit))
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

#[derive(Resource)]
struct AutoExit(bool);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    println!("\n=== POINT LIGHT SHADOW TEST SETUP ===\n");

    // Create voxel chunk
    let mut chunk = VoxelChunk::new();

    // === GROUND: 16x16 light gray voxels at y=0 ===
    let ground_color = Voxel::solid(180, 180, 180); // Light gray for visibility
    for x in 8..24 {
        for z in 8..24 {
            chunk.set(x, 0, z, ground_color);
        }
    }
    println!("Ground: 16x16 light gray voxels at y=0");

    // === OCCLUDER: 2x4x2 pillar that will cast shadow ===
    // Positioned off-center so shadow is visible on the ground
    let pillar_color = Voxel::solid(100, 60, 60); // Dark red-brown
    for y in 1..=4 {
        for x in 14..16 {
            for z in 18..20 {
                chunk.set(x, y, z, pillar_color);
            }
        }
    }
    println!("Pillar: 2x4x2 dark red-brown at (14-15, 1-4, 18-19)");

    // === SECOND OCCLUDER: Smaller pillar on opposite side ===
    let pillar2_color = Voxel::solid(60, 100, 60); // Dark green
    for y in 1..=2 {
        for x in 18..20 {
            for z in 12..14 {
                chunk.set(x, y, z, pillar2_color);
            }
        }
    }
    println!("Pillar 2: 2x2x2 dark green at (18-19, 1-2, 12-13)");

    println!("Total voxels: {}", chunk.count());

    // Build mesh
    let mesh = build_chunk_mesh(&chunk);
    let mesh_handle = meshes.add(mesh);
    let material = materials.add(VoxelMaterial::default());

    // Scene offset - ground at world Y=0
    let scene_offset = Vec3::new(0.0, CHUNK_SIZE as f32 / 2.0, 0.0);

    // Spawn voxel mesh
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::from_translation(scene_offset),
        DeferredRenderable,
    ));

    // === POINT LIGHT ===
    // Position above center of ground, between the pillars
    // Ground center in chunk coords: (16, 0, 16)
    // Mesh position: (16-16+0.5, 0-16+0.5, 16-16+0.5) = (0.5, -15.5, 0.5)
    // World position: (0.5, -15.5+16, 0.5) = (0.5, 0.5, 0.5)
    // Light above ground at Y=6
    let light_pos = Vec3::new(0.0, 6.0, 0.0);
    
    commands.spawn((
        DeferredPointLight {
            color: Color::srgb(1.0, 0.9, 0.7), // Warm white
            intensity: 50.0,
            radius: 20.0, // Large enough to cover entire scene
            // TODO: Add shadow_casting: true field
        },
        Transform::from_translation(light_pos),
    ));
    println!("Point light at {:?} (warm white, intensity 50, radius 20)", light_pos);

    // === CAMERA ===
    // Overhead angle to see ground and shadows clearly
    let camera_pos = Vec3::new(12.0, 15.0, 12.0);
    let look_at = Vec3::new(0.0, 0.0, 0.0);
    
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_translation(camera_pos).looking_at(look_at, Vec3::Y),
        DeferredCamera,
    ));
    println!("Camera at {:?} looking at {:?}", camera_pos, look_at);

    println!("\n=== EXPECTED RESULTS ===");
    println!("1. Ground is illuminated by warm white point light");
    println!("2. Two pillars visible (red-brown and green)");
    println!("3. [TODO] Each pillar casts a shadow away from the light");
    println!("4. [TODO] Shadows are soft near edges, sharp near base");
    println!("\nPress SPACE to disable auto-exit and interact with scene.");
}

fn toggle_auto_exit(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut auto_exit: ResMut<AutoExit>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        auto_exit.0 = false;
        println!("Auto-exit disabled. Press ESC to close.");
    }
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    auto_exit: Res<AutoExit>,
    mut exit: EventWriter<AppExit>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    frame_count.0 += 1;

    // Manual exit with ESC
    if keyboard.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }

    // Only auto-capture and exit if enabled
    if !auto_exit.0 {
        return;
    }

    // Give render graph time to initialize
    if frame_count.0 == 15 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }

    // Exit after screenshot is captured
    if frame_count.0 >= 25 {
        println!("Exiting after {} frames", frame_count.0);
        exit.write(AppExit::Success);
    }
}
