//! Phase 10: Dark World Test Scene
//!
//! This example creates a dark fantasy scene for testing:
//! - Dual moon lighting (purple + orange directional lights)
//! - Point lights from emissive voxels
//! - Shadow casting from multiple light sources
//!
//! The scene features:
//! - Dark rocky terrain
//! - Central altar with glowing orb
//! - Ruined pillars casting shadows
//! - Colored crystal clusters
//!
//! Run with: `cargo run --example p10_dark_world`
//!
//! Expected output: `screenshots/p10_dark_world.png`
//! - Dark scene lit by purple and orange moons
//! - Glowing crystals with bloom
//! - Shadows from pillars and structures

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    build_chunk_mesh, extract_emissive_lights, load_creature_script, DeferredCamera,
    DeferredPointLight, DeferredRenderable, DeferredRenderingPlugin, VoxelMaterial,
    VoxelMaterialPlugin,
};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p10_dark_world.png";
const CREATURE_SCRIPT: &str = "assets/scripts/test_darkworld.lua";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 10: Dark World Test Scene...");
    println!("Loading test script: {}", CREATURE_SCRIPT);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1024, 768).into(),
                title: "Phase 10: Dark World - Dual Moon Lighting".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        // Pure black void - no ambient fog, moons provide all light
        .insert_resource(ClearColor(Color::srgb(0.02, 0.01, 0.03)))
        .insert_resource(FrameCount(0))
        .add_systems(Startup, setup)
        .add_systems(Update, (rotate_camera, capture_and_exit))
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

#[derive(Component)]
struct MainCamera;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    // Load dark world script
    let chunk = match load_creature_script(CREATURE_SCRIPT) {
        Ok(c) => {
            println!("Loaded dark world scene with {} voxels", c.count());
            c
        }
        Err(e) => {
            eprintln!("ERROR: Failed to load creature script: {:?}", e);
            std::process::exit(1);
        }
    };

    // === PHASE 16: AUTO-GENERATE POINT LIGHTS FROM EMISSIVE VOXELS ===
    // Extract emissive voxels and create point lights from them
    // This ensures point light colors MATCH the emissive voxel colors
    // Using non-clustered extraction - each emissive voxel becomes a light
    // The voxel itself is visible (emissive surface) and the light illuminates surroundings
    let emissive_lights = extract_emissive_lights(&chunk, 100);
    println!("Found {} emissive voxels for point lights", emissive_lights.len());

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

    // Scene offset - the scene is built at 0-31 but we center it
    let scene_offset = Vec3::new(-16.0, 0.0, -16.0);

    // Spawn dark world - centered at origin
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::from_translation(scene_offset),
        DeferredRenderable,
    ));

    // Camera - closer and rotated to see more details
    // Position to see the altar, crystals, and show lighting effects clearly
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(6.0, 6.0, 10.0).looking_at(Vec3::new(-2.0, 2.0, -2.0), Vec3::Y),
        DeferredCamera,
        MainCamera,
    ));

    // Note: The actual moon lighting is handled in deferred_lighting.wgsl
    // These Bevy lights are for reference/forward pass only
    
    // Purple moon (high angle, left side)
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.6, 0.2, 0.8),
            illuminance: 5000.0,
            shadows_enabled: false, // Our custom pipeline handles shadows
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -0.8,  // Pitch down
            0.5,   // Yaw left
            0.0,
        )),
    ));

    // Orange moon (low angle, right side, opposite direction)
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.5, 0.2),
            illuminance: 3000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -0.3,  // Pitch down (lower angle)
            -0.7,  // Yaw right
            0.0,
        )),
    ));

    // === AUTO-GENERATED POINT LIGHTS FROM EMISSIVE VOXELS ===
    // Each emissive voxel creates a point light with MATCHING color
    // The emissive voxel itself glows (via emission in shader)
    // The point light illuminates surrounding surfaces with colored light
    let mut light_count = 0;
    
    for light in &emissive_lights {
        // Get world position (center of voxel + scene offset)
        let world_pos = light.world_position([0.0, 0.0, 0.0]);
        let pos = Vec3::new(world_pos[0], world_pos[1], world_pos[2]) + scene_offset;
        
        // Create point light with same color as emissive voxel
        // Use moderate intensity so lights illuminate but don't overpower
        // Radius determines how far the colored light spreads
        let intensity = 3.0 * light.emission;  // Scale with emission
        let radius = 6.0;  // Fixed moderate radius
        
        commands.spawn((
            DeferredPointLight {
                color: Color::srgb(light.color[0], light.color[1], light.color[2]),
                intensity,
                radius,
            },
            Transform::from_translation(pos),
        ));
        light_count += 1;
    }
    
    // Print summary of lights by color
    println!("Dark world scene setup complete.");
    println!("Auto-generated {} point lights from emissive voxels", light_count);
}

fn rotate_camera(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<MainCamera>>,
    frame_count: Res<FrameCount>,
) {
    // Only rotate in interactive mode (after screenshot frames)
    if frame_count.0 < 25 {
        return;
    }
    
    for mut transform in &mut query {
        // Slow orbit around the scene
        let angle = time.elapsed_secs() * 0.1;
        let radius = 35.0;
        let height = 20.0;
        
        transform.translation = Vec3::new(
            angle.cos() * radius,
            height,
            angle.sin() * radius,
        );
        transform.look_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y);
    }
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

    // Don't auto-exit - let the user explore the scene
    // Uncomment below for CI/testing:
    // if frame_count.0 >= 25 {
    //     println!("Exiting after {} frames", frame_count.0);
    //     exit.write(AppExit::Success);
    // }
}
