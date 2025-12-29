//! Phase 12: Minimal Point Light Voxel Test
//!
//! A minimal test scene to debug point light issues:
//! - Small flat ground (5x5 gray voxels)
//! - One red emissive voxel above the ground
//! - One point light at the emissive voxel position
//!
//! Uses the same VoxelChunk + build_chunk_mesh() code path as p10_dark_world
//! to ensure we test the actual rendering pipeline.
//!
//! Run with: `cargo run --example p12_point_light_voxel`
//!
//! Expected: Red glow on the ground from the point light

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
const SCREENSHOT_PATH: &str = "screenshots/p12_point_light_voxel.png";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 12: Minimal Point Light Voxel Test...");
    println!("This test creates a simple scene to debug point light issues.");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 12: Point Light Voxel Test".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        // Dark background to see point light clearly
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.02)))
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
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    println!("Setting up minimal voxel scene...");

    // Create a minimal voxel chunk programmatically
    let mut chunk = VoxelChunk::new();

    // === GROUND: 7x7 gray voxels at y=0, centered at chunk position (12-18, 0, 12-18) ===
    let ground_color = Voxel::solid(128, 128, 128); // Gray
    for x in 12..=18 {
        for z in 12..=18 {
            chunk.set(x, 0, z, ground_color);
        }
    }
    println!("Created 7x7 ground at y=0");

    // === EMISSIVE VOXEL: Red emissive at center, y=3 ===
    let emissive = Voxel::emissive(255, 50, 50); // Bright red
    chunk.set(15, 3, 15, emissive);
    println!("Created red emissive voxel at (15, 3, 15)");

    println!("Total voxels in chunk: {}", chunk.count());

    // Build mesh using the same function as p10_dark_world
    let mesh = build_chunk_mesh(&chunk);

    // Log mesh statistics
    if let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        println!("Mesh vertices: {}", positions.len());
    }
    if let Some(indices) = mesh.indices() {
        println!("Mesh indices: {}", indices.len());
    }

    let mesh_handle = meshes.add(mesh);
    let material = materials.add(VoxelMaterial::default());

    // Scene transform - mesh is already centered by build_chunk_mesh()
    // Just translate Y so ground level (chunk Y=0) is at world Y=0
    let scene_offset = Vec3::new(0.0, CHUNK_SIZE as f32 / 2.0, 0.0);

    // Spawn voxel mesh
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::from_translation(scene_offset),
        DeferredRenderable,
    ));

    // === POINT LIGHT ===
    // Emissive voxel at chunk (15, 3, 15)
    // Mesh position (after centering): (15-16+0.5, 3-16+0.5, 15-16+0.5) = (-0.5, -12.5, -0.5)
    // World position: mesh + scene_offset = (-0.5, -12.5+16, -0.5) = (-0.5, 3.5, -0.5)
    // Place light slightly above the emissive voxel
    let light_world_pos = Vec3::new(-0.5, 5.0, -0.5);
    
    commands.spawn((
        DeferredPointLight {
            color: Color::srgb(1.0, 0.2, 0.2), // Red
            intensity: 30.0,
            radius: 12.0, // Should reach the ground (4 units away)
        },
        Transform::from_translation(light_world_pos),
    ));
    println!(
        "Created point light at {:?} with radius 12.0",
        light_world_pos
    );

    // === CAMERA ===
    // Position to see the ground and light from above-front
    let camera_pos = Vec3::new(5.0, 8.0, 8.0);
    let look_at = Vec3::new(0.0, 1.0, 0.0);
    
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_translation(camera_pos).looking_at(look_at, Vec3::Y),
        DeferredCamera,
    ));
    println!("Camera at {:?} looking at {:?}", camera_pos, look_at);

    println!("\n=== MINIMAL VOXEL TEST SETUP COMPLETE ===");
    println!("Ground: 7x7 gray voxels at y=0 (world: y=-15 to y=15 centered)");
    println!("Light: Red point light at (0.5, 4, 0.5) with radius 12");
    println!("Expected: Red illumination on the gray ground");
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;

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
