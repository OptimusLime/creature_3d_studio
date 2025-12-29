//! Phase 16: Multi-Chunk World Test
//!
//! This example demonstrates rendering a world with multiple chunks:
//! - 3x3x1 grid of chunks (9 chunks total)
//! - Each chunk has distinct terrain features
//! - Voxels can span chunk boundaries
//!
//! Run with: `cargo run --example p16_multi_chunk`
//!
//! Expected output: `screenshots/p16_multi_chunk.png`
//! - 9 chunks arranged in a 3x3 grid
//! - Visible chunk boundaries (seams expected - no cross-chunk culling yet)
//! - Different colored terrain per chunk

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;
use studio_core::{
    build_world_meshes, ChunkPos, DeferredCamera, DeferredRenderable, DeferredRenderingPlugin,
    Voxel, VoxelMaterial, VoxelMaterialPlugin, VoxelWorld, CHUNK_SIZE,
};

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p16_multi_chunk.png";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");

    println!("Running Phase 16: Multi-Chunk World Test...");
    println!("Creating 2x1x2 world ({} chunks)", 2 * 2);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1024, 768).into(),
                title: "Phase 16: Multi-Chunk World".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        // Fog color as clear color
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

/// Create a procedural multi-chunk world.
fn create_world() -> VoxelWorld {
    let mut world = VoxelWorld::new();

    // Create a 2x1x2 grid of chunks (4 chunks) - simpler for initial testing
    // Each chunk will have a different base color to distinguish them
    let chunk_colors: [[(u8, u8, u8); 2]; 2] = [
        // Row 0 (z = 0)
        [
            (180, 60, 60),   // Red-ish
            (60, 180, 60),   // Green-ish
        ],
        // Row 1 (z = 1)
        [
            (60, 60, 180),   // Blue-ish
            (180, 180, 60),  // Yellow-ish
        ],
    ];

    // For each chunk in the 2x2 grid
    for cx in 0..=1 {
        for cz in 0..=1 {
            let color_idx_x = cx as usize;
            let color_idx_z = cz as usize;
            let (base_r, base_g, base_b) = chunk_colors[color_idx_z][color_idx_x];

            // World coordinates for this chunk
            let world_x_start = cx * CHUNK_SIZE as i32;
            let world_z_start = cz * CHUNK_SIZE as i32;

            // Create terrain in this chunk
            for lx in 0..CHUNK_SIZE as i32 {
                for lz in 0..CHUNK_SIZE as i32 {
                    let wx = world_x_start + lx;
                    let wz = world_z_start + lz;

                    // Simple height variation based on position
                    let height = 3 + ((wx.abs() + wz.abs()) % 5) as i32;

                    // Fill from y=0 up to height
                    for wy in 0..height {
                        // Color varies slightly with height
                        let height_factor = (wy as f32 / height as f32 * 50.0) as u8;
                        let r = base_r.saturating_add(height_factor);
                        let g = base_g.saturating_add(height_factor);
                        let b = base_b.saturating_add(height_factor);

                        world.set_voxel(wx, wy, wz, Voxel::solid(r, g, b));
                    }
                }
            }

            // Add a glowing crystal in the center of each chunk
            let crystal_x = world_x_start + CHUNK_SIZE as i32 / 2;
            let crystal_z = world_z_start + CHUNK_SIZE as i32 / 2;
            let crystal_height = 8;

            for cy in 5..5 + crystal_height {
                // Crystal color is more saturated version of chunk color
                let crystal_r = (base_r as u16 * 3 / 2).min(255) as u8;
                let crystal_g = (base_g as u16 * 3 / 2).min(255) as u8;
                let crystal_b = (base_b as u16 * 3 / 2).min(255) as u8;

                world.set_voxel(
                    crystal_x,
                    cy,
                    crystal_z,
                    Voxel::new(crystal_r, crystal_g, crystal_b, 200),
                );
            }
        }
    }

    // Add some voxels that span chunk boundaries to test cross-chunk placement
    // Line along X axis crossing chunks (0,0,0) -> (1,0,0)
    for x in 10..54 {
        world.set_voxel(x, 10, 16, Voxel::new(255, 255, 255, 128)); // White bridge
    }

    // Line along Z axis crossing chunks
    for z in 10..54 {
        world.set_voxel(16, 10, z, Voxel::new(255, 200, 100, 128)); // Orange bridge
    }

    world
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    // Create the multi-chunk world
    let world = create_world();

    println!(
        "World created: {} chunks, {} total voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );

    // Print chunk positions
    if let Some((min, max)) = world.chunk_bounds() {
        println!(
            "Chunk bounds: ({}, {}, {}) to ({}, {}, {})",
            min.x, min.y, min.z, max.x, max.y, max.z
        );
    }

    // Build meshes for all chunks
    let chunk_meshes = build_world_meshes(&world);

    println!("Generated {} chunk meshes", chunk_meshes.len());

    // Shared material for all chunks
    let material = materials.add(VoxelMaterial::default());

    // Spawn each chunk as a separate entity
    let mut total_vertices = 0;
    let mut total_indices = 0;

    for chunk_mesh in chunk_meshes {
        // Log per-chunk stats
        let verts = chunk_mesh
            .mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .map(|a| a.len())
            .unwrap_or(0);
        let inds = chunk_mesh.mesh.indices().map(|i| i.len()).unwrap_or(0);

        println!(
            "  Chunk ({}, {}, {}): {} vertices, {} indices, offset {:?}",
            chunk_mesh.chunk_pos.x,
            chunk_mesh.chunk_pos.y,
            chunk_mesh.chunk_pos.z,
            verts,
            inds,
            chunk_mesh.world_offset
        );

        total_vertices += verts;
        total_indices += inds;

        // Store values before consuming chunk_mesh
        let translation = chunk_mesh.translation();
        let chunk_pos = chunk_mesh.chunk_pos;
        let mesh_handle = meshes.add(chunk_mesh.mesh);

        commands.spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(translation),
            DeferredRenderable,
            ChunkMarker(chunk_pos),
        ));
    }

    println!(
        "Total: {} vertices, {} indices across {} chunks",
        total_vertices,
        total_indices,
        world.chunk_count()
    );

    // Camera positioned to see all chunks
    // The world spans roughly 0 to 64 in X and Z (2 chunks * 32 each)
    // Position camera to see the entire grid
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(80.0, 50.0, 80.0).looking_at(Vec3::new(32.0, 5.0, 32.0), Vec3::Y),
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

    println!("Multi-chunk world setup complete.");
}

/// Marker component to identify chunk entities.
#[derive(Component)]
struct ChunkMarker(ChunkPos);

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

    if frame_count.0 >= 25 {
        println!("Exiting after {} frames", frame_count.0);
        exit.write(AppExit::Success);
    }
}
