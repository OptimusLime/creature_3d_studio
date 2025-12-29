//! Phase 16: Multi-Chunk World Test (Simple Version)
//!
//! This is a simplified version that uses standard Bevy rendering
//! instead of the custom deferred pipeline, to verify the multi-chunk
//! mesh generation works correctly.
//!
//! Run with: `cargo run --example p16_multi_chunk_simple`

use bevy::prelude::*;
use studio_core::{
    build_world_meshes, ChunkPos, Voxel, VoxelWorld, CHUNK_SIZE,
};

fn main() {
    println!("Running Phase 16: Multi-Chunk World Test (Simple)...");
    println!("Creating 2x1x2 world ({} chunks)", 2 * 2);
    println!("Press ESC to exit");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1024, 768).into(),
                title: "Phase 16: Multi-Chunk World (Simple)".into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.15)))
        .add_systems(Startup, setup)
        .add_systems(Update, exit_on_esc)
        .run();
}

/// Create a procedural multi-chunk world.
fn create_world() -> VoxelWorld {
    let mut world = VoxelWorld::new();

    // Create a 2x1x2 grid of chunks (4 chunks)
    let chunk_colors: [[(u8, u8, u8); 2]; 2] = [
        [(180, 60, 60), (60, 180, 60)],
        [(60, 60, 180), (180, 180, 60)],
    ];

    for cx in 0..=1 {
        for cz in 0..=1 {
            let (base_r, base_g, base_b) = chunk_colors[cz as usize][cx as usize];
            let world_x_start = cx * CHUNK_SIZE as i32;
            let world_z_start = cz * CHUNK_SIZE as i32;

            // Create terrain
            for lx in 0..CHUNK_SIZE as i32 {
                for lz in 0..CHUNK_SIZE as i32 {
                    let wx = world_x_start + lx;
                    let wz = world_z_start + lz;
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

            // Add crystal in center
            let crystal_x = world_x_start + CHUNK_SIZE as i32 / 2;
            let crystal_z = world_z_start + CHUNK_SIZE as i32 / 2;
            for cy in 5..13 {
                world.set_voxel(
                    crystal_x, cy, crystal_z,
                    Voxel::new(255, 255, 255, 200),
                );
            }
        }
    }

    // Cross-chunk bridges
    for x in 10..54 {
        world.set_voxel(x, 10, 16, Voxel::solid(255, 255, 255));
    }
    for z in 10..54 {
        world.set_voxel(16, 10, z, Voxel::solid(255, 200, 100));
    }

    world
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let world = create_world();

    println!(
        "World created: {} chunks, {} total voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );

    if let Some((min, max)) = world.chunk_bounds() {
        println!(
            "Chunk bounds: ({}, {}, {}) to ({}, {}, {})",
            min.x, min.y, min.z, max.x, max.y, max.z
        );
    }

    // Build meshes
    let chunk_meshes = build_world_meshes(&world);
    println!("Generated {} chunk meshes", chunk_meshes.len());

    // Spawn chunks with standard material
    let mut total_vertices = 0;
    let mut total_indices = 0;

    for chunk_mesh in chunk_meshes {
        let verts = chunk_mesh
            .mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .map(|a| a.len())
            .unwrap_or(0);
        let inds = chunk_mesh.mesh.indices().map(|i| i.len()).unwrap_or(0);

        println!(
            "  Chunk ({}, {}, {}): {} vertices, {} indices",
            chunk_mesh.chunk_pos.x,
            chunk_mesh.chunk_pos.y,
            chunk_mesh.chunk_pos.z,
            verts,
            inds,
        );

        total_vertices += verts;
        total_indices += inds;

        // Use per-chunk colored material based on chunk position
        let chunk_pos = chunk_mesh.chunk_pos;
        let color = match (chunk_pos.x, chunk_pos.z) {
            (0, 0) => Color::srgb(0.8, 0.3, 0.3),
            (1, 0) => Color::srgb(0.3, 0.8, 0.3),
            (0, 1) => Color::srgb(0.3, 0.3, 0.8),
            (1, 1) => Color::srgb(0.8, 0.8, 0.3),
            _ => Color::WHITE,
        };

        let translation = chunk_mesh.translation();
        let mesh_handle = meshes.add(chunk_mesh.mesh);
        let material = materials.add(StandardMaterial {
            base_color: color,
            ..default()
        });

        commands.spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(material),
            Transform::from_translation(translation),
        ));
    }

    println!(
        "Total: {} vertices, {} indices across {} chunks",
        total_vertices,
        total_indices,
        world.chunk_count()
    );

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(80.0, 50.0, 80.0).looking_at(Vec3::new(32.0, 5.0, 32.0), Vec3::Y),
    ));

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: 15000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(50.0, 100.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    println!("Setup complete!");
}

fn exit_on_esc(input: Res<ButtonInput<KeyCode>>, mut exit: EventWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
