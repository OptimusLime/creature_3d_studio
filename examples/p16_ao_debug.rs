//! Debug test to understand AO calculation in greedy meshing
//!
//! Run with: `cargo run --example p16_ao_debug`

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::Mesh;
use studio_core::{Voxel, VoxelChunk};
use studio_core::voxel_mesh::{build_chunk_mesh_greedy, build_chunk_mesh, ATTRIBUTE_VOXEL_AO};

fn main() {
    println!("=== AO Debug Test ===\n");

    // Create a simple stepped terrain: 3 columns at different heights
    let mut chunk = VoxelChunk::new();
    
    // Column at x=0: height 3
    // Column at x=1: height 4
    // Column at x=2: height 3
    // All at z=0, uniform color
    
    for y in 0..3 {
        chunk.set(0, y, 0, Voxel::solid(100, 100, 100));
    }
    for y in 0..4 {
        chunk.set(1, y, 0, Voxel::solid(100, 100, 100));
    }
    for y in 0..3 {
        chunk.set(2, y, 0, Voxel::solid(100, 100, 100));
    }

    println!("Created simple stepped terrain:");
    println!("  x=0: height 3");
    println!("  x=1: height 4 (taller)");
    println!("  x=2: height 3");
    println!();

    // Build both greedy and non-greedy meshes
    let greedy_mesh = build_chunk_mesh_greedy(&chunk);
    let simple_mesh = build_chunk_mesh(&chunk);
    
    println!("=== GREEDY MESH ===");
    print_top_face_ao(&greedy_mesh, "greedy");
    
    println!("\n=== SIMPLE (NON-GREEDY) MESH ===");
    print_top_face_ao(&simple_mesh, "simple");
}

fn print_top_face_ao(mesh: &Mesh, label: &str) {
    use bevy::mesh::Indices;
    
    let ao_attr = mesh.attribute(ATTRIBUTE_VOXEL_AO).unwrap();
    let pos_attr = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
    let normal_attr = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap();
    
    let positions: Vec<[f32; 3]> = match pos_attr {
        VertexAttributeValues::Float32x3(v) => v.clone(),
        _ => vec![],
    };
    let aos: Vec<f32> = match ao_attr {
        VertexAttributeValues::Float32(v) => v.clone(),
        _ => vec![],
    };
    let normals: Vec<[f32; 3]> = match normal_attr {
        VertexAttributeValues::Float32x3(v) => v.clone(),
        _ => vec![],
    };
    
    println!("{} mesh has {} vertices total", label, positions.len());
    
    // Find vertices with upward normals (top faces)
    println!("Top face vertices (normal.y > 0.9):");
    for (i, ((pos, ao), normal)) in positions.iter().zip(aos.iter()).zip(normals.iter()).enumerate() {
        if normal[1] > 0.9 {
            // Convert mesh coords to chunk coords (mesh is centered, offset = 16)
            let chunk_x = pos[0] + 16.0;
            let chunk_y = pos[1] + 16.0;
            let chunk_z = pos[2] + 16.0;
            println!("  v{}: mesh=({:.1},{:.1},{:.1}) chunk=({:.1},{:.1},{:.1}) ao={:.2}", 
                     i, pos[0], pos[1], pos[2], chunk_x, chunk_y, chunk_z, ao);
        }
    }
    
    // Print indices
    if let Some(indices) = mesh.indices() {
        let idx_vec: Vec<u32> = match indices {
            Indices::U32(v) => v.clone(),
            Indices::U16(v) => v.iter().map(|&x| x as u32).collect(),
        };
        println!("Indices ({} total):", idx_vec.len());
        
        // Find triangles that use top-face vertices (16-27 for greedy, 80-127 for simple)
        // Just print first few triangles to see the pattern
        for i in (0..idx_vec.len().min(36)).step_by(3) {
            let i0 = idx_vec[i];
            let i1 = idx_vec[i + 1];
            let i2 = idx_vec[i + 2];
            if normals.get(i0 as usize).map(|n| n[1] > 0.9).unwrap_or(false) {
                println!("  tri: ({}, {}, {})", i0, i1, i2);
            }
        }
    }
}
