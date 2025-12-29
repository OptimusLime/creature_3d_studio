//! Voxel mesh generation and custom material.
//!
//! This module provides:
//! - `VoxelMaterial`: Custom material with per-vertex color and emission
//! - `build_chunk_mesh()`: Generates a single mesh from a VoxelChunk
//!
//! Each voxel becomes 6 quads (12 triangles) with per-vertex attributes:
//! - position: world position
//! - normal: face normal
//! - color: RGB from voxel (custom attribute)
//! - emission: emission intensity from voxel (custom attribute)
//! - ao: ambient occlusion (0.0-1.0, where 1.0 = fully lit, lower = darker corners)
//!
//! The custom shader reads these attributes and applies lighting with emission support.
//!
//! ## Per-Vertex Ambient Occlusion
//!
//! AO is calculated at mesh generation time using the algorithm from:
//! https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/
//!
//! For each vertex of a face, we check the 3 corner-adjacent voxels:
//! - side1: adjacent along one axis of the face
//! - side2: adjacent along the other axis of the face  
//! - corner: diagonally adjacent
//!
//! AO value = 3 - (side1 + side2 + corner), normalized to [0,1]
//! Special case: if both sides are solid, corner is occluded (AO = 0)

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError, VertexFormat,
};
use bevy::shader::ShaderRef;

use crate::voxel::{VoxelChunk, CHUNK_SIZE};

/// Custom vertex attribute for per-vertex color (RGB).
pub const ATTRIBUTE_VOXEL_COLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("VoxelColor", 988540917, VertexFormat::Float32x3);

/// Custom vertex attribute for per-vertex emission.
pub const ATTRIBUTE_VOXEL_EMISSION: MeshVertexAttribute =
    MeshVertexAttribute::new("VoxelEmission", 988540918, VertexFormat::Float32);

/// Custom vertex attribute for per-vertex ambient occlusion.
/// 1.0 = fully lit (no occlusion), 0.0 = fully occluded (dark corner)
pub const ATTRIBUTE_VOXEL_AO: MeshVertexAttribute =
    MeshVertexAttribute::new("VoxelAO", 988540919, VertexFormat::Float32);

/// Custom material for voxel rendering.
///
/// Uses per-vertex color and emission attributes instead of textures.
/// The shader reads these attributes and applies simple lit shading with emission.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct VoxelMaterial {
    /// Ambient light contribution (0.0 - 1.0)
    #[uniform(0)]
    pub ambient: f32,
}

impl Default for VoxelMaterial {
    fn default() -> Self {
        Self { ambient: 0.05 }
    }
}

impl Material for VoxelMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/voxel.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/voxel.wgsl".into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Get the vertex buffer layout with our custom attributes
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            ATTRIBUTE_VOXEL_COLOR.at_shader_location(2),
            ATTRIBUTE_VOXEL_EMISSION.at_shader_location(3),
            ATTRIBUTE_VOXEL_AO.at_shader_location(4),
        ])?;

        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }
}

/// Plugin that registers the VoxelMaterial with Bevy.
pub struct VoxelMaterialPlugin;

impl Plugin for VoxelMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<VoxelMaterial>::default());
    }
}

/// Build a single mesh from a VoxelChunk.
///
/// Each filled voxel generates 6 quads (24 vertices, 36 indices).
/// Vertices include position, normal, color (RGB), emission, and AO attributes.
///
/// The mesh is centered at origin (chunk coords 0-15 map to world -8 to +7).
pub fn build_chunk_mesh(chunk: &VoxelChunk) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut emissions: Vec<f32> = Vec::new();
    let mut aos: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Offset to center chunk at origin
    let offset = CHUNK_SIZE as f32 / 2.0;

    for (x, y, z, voxel) in chunk.iter() {
        let base_pos = [x as f32 - offset, y as f32 - offset, z as f32 - offset];

        let color = voxel.color_f32();
        let emission = voxel.emission_f32();

        // Generate 6 faces for this voxel with AO
        add_cube_faces_with_ao(
            chunk,
            x,
            y,
            z,
            &mut positions,
            &mut normals,
            &mut colors,
            &mut emissions,
            &mut aos,
            &mut indices,
            base_pos,
            color,
            emission,
        );
    }

    // Create mesh with custom attributes for VoxelMaterial
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(ATTRIBUTE_VOXEL_COLOR, colors)
        .with_inserted_attribute(ATTRIBUTE_VOXEL_EMISSION, emissions)
        .with_inserted_attribute(ATTRIBUTE_VOXEL_AO, aos)
        .with_inserted_indices(Indices::U32(indices))
}

/// Calculate ambient occlusion for a vertex based on neighboring voxels.
///
/// For a vertex at a corner of a face, we check 3 neighbors:
/// - side1: adjacent voxel along one edge of the face
/// - side2: adjacent voxel along the other edge of the face
/// - corner: diagonally adjacent voxel
///
/// AO value is based on how many of these are solid:
/// - 0 solid: AO = 1.0 (fully lit)
/// - 1 solid: AO = 0.75
/// - 2 solid: AO = 0.5
/// - 3 solid: AO = 0.25
///
/// Special case: if both sides are solid, the corner is automatically
/// occluded even if empty (prevents light leaking through diagonal cracks).
fn calculate_vertex_ao(side1: bool, side2: bool, corner: bool) -> f32 {
    if side1 && side2 {
        // Both sides solid = maximum occlusion (corner blocked)
        0.0
    } else {
        // Count solid neighbors
        let count = side1 as u8 + side2 as u8 + corner as u8;
        // Map 0->1.0, 1->0.7, 2->0.4, 3->0.1
        1.0 - (count as f32 * 0.3)
    }
}

/// Face direction enum for AO calculation
#[derive(Clone, Copy)]
enum FaceDir {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

/// Get the AO neighbor offsets for each vertex of a face.
/// Returns [(side1, side2, corner); 4] for the 4 vertices of the face.
/// Each offset is (dx, dy, dz) relative to the voxel position.
fn get_ao_offsets(face: FaceDir) -> [[(i32, i32, i32); 3]; 4] {
    match face {
        // +X face: vertices at x+1, checking neighbors in Y and Z directions
        FaceDir::PosX => [
            // Vertex 0: (1,0,0) - bottom-back corner
            [(1, -1, 0), (1, 0, -1), (1, -1, -1)],
            // Vertex 1: (1,1,0) - top-back corner
            [(1, 1, 0), (1, 0, -1), (1, 1, -1)],
            // Vertex 2: (1,1,1) - top-front corner
            [(1, 1, 0), (1, 0, 1), (1, 1, 1)],
            // Vertex 3: (1,0,1) - bottom-front corner
            [(1, -1, 0), (1, 0, 1), (1, -1, 1)],
        ],
        // -X face: vertices at x-1
        FaceDir::NegX => [
            // Vertex 0: (0,0,1) - bottom-front corner
            [(-1, -1, 0), (-1, 0, 1), (-1, -1, 1)],
            // Vertex 1: (0,1,1) - top-front corner
            [(-1, 1, 0), (-1, 0, 1), (-1, 1, 1)],
            // Vertex 2: (0,1,0) - top-back corner
            [(-1, 1, 0), (-1, 0, -1), (-1, 1, -1)],
            // Vertex 3: (0,0,0) - bottom-back corner
            [(-1, -1, 0), (-1, 0, -1), (-1, -1, -1)],
        ],
        // +Y face (top): vertices at y+1, checking neighbors in X and Z
        FaceDir::PosY => [
            // Vertex 0: (0,1,0) - back-left corner
            [(0, 1, -1), (-1, 1, 0), (-1, 1, -1)],
            // Vertex 1: (0,1,1) - front-left corner
            [(0, 1, 1), (-1, 1, 0), (-1, 1, 1)],
            // Vertex 2: (1,1,1) - front-right corner
            [(0, 1, 1), (1, 1, 0), (1, 1, 1)],
            // Vertex 3: (1,1,0) - back-right corner
            [(0, 1, -1), (1, 1, 0), (1, 1, -1)],
        ],
        // -Y face (bottom): vertices at y-1
        FaceDir::NegY => [
            // Vertex 0: (0,0,1) - front-left corner
            [(0, -1, 1), (-1, -1, 0), (-1, -1, 1)],
            // Vertex 1: (0,0,0) - back-left corner
            [(0, -1, -1), (-1, -1, 0), (-1, -1, -1)],
            // Vertex 2: (1,0,0) - back-right corner
            [(0, -1, -1), (1, -1, 0), (1, -1, -1)],
            // Vertex 3: (1,0,1) - front-right corner
            [(0, -1, 1), (1, -1, 0), (1, -1, 1)],
        ],
        // +Z face (front): vertices at z+1, checking neighbors in X and Y
        FaceDir::PosZ => [
            // Vertex 0: (0,0,1) - bottom-left corner
            [(-1, 0, 1), (0, -1, 1), (-1, -1, 1)],
            // Vertex 1: (1,0,1) - bottom-right corner
            [(1, 0, 1), (0, -1, 1), (1, -1, 1)],
            // Vertex 2: (1,1,1) - top-right corner
            [(1, 0, 1), (0, 1, 1), (1, 1, 1)],
            // Vertex 3: (0,1,1) - top-left corner
            [(-1, 0, 1), (0, 1, 1), (-1, 1, 1)],
        ],
        // -Z face (back): vertices at z-1
        FaceDir::NegZ => [
            // Vertex 0: (1,0,0) - bottom-right corner
            [(1, 0, -1), (0, -1, -1), (1, -1, -1)],
            // Vertex 1: (0,0,0) - bottom-left corner
            [(-1, 0, -1), (0, -1, -1), (-1, -1, -1)],
            // Vertex 2: (0,1,0) - top-left corner
            [(-1, 0, -1), (0, 1, -1), (-1, 1, -1)],
            // Vertex 3: (1,1,0) - top-right corner
            [(1, 0, -1), (0, 1, -1), (1, 1, -1)],
        ],
    }
}

/// Add 6 faces (24 vertices, 36 indices) for a unit cube at the given position.
/// Includes per-vertex ambient occlusion calculation.
#[allow(clippy::too_many_arguments)]
fn add_cube_faces_with_ao(
    chunk: &VoxelChunk,
    vx: usize,
    vy: usize,
    vz: usize,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 3]>,
    emissions: &mut Vec<f32>,
    aos: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    base: [f32; 3],
    color: [f32; 3],
    emission: f32,
) {
    let base_index = positions.len() as u32;

    // Face definitions: (normal, 4 corner offsets, face direction for AO)
    let faces: [([f32; 3], [[f32; 3]; 4], FaceDir); 6] = [
        // +X face (right)
        (
            [1.0, 0.0, 0.0],
            [
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [1.0, 0.0, 1.0],
            ],
            FaceDir::PosX,
        ),
        // -X face (left)
        (
            [-1.0, 0.0, 0.0],
            [
                [0.0, 0.0, 1.0],
                [0.0, 1.0, 1.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
            ],
            FaceDir::NegX,
        ),
        // +Y face (top)
        (
            [0.0, 1.0, 0.0],
            [
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
            ],
            FaceDir::PosY,
        ),
        // -Y face (bottom)
        (
            [0.0, -1.0, 0.0],
            [
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
            ],
            FaceDir::NegY,
        ),
        // +Z face (front)
        (
            [0.0, 0.0, 1.0],
            [
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
            FaceDir::PosZ,
        ),
        // -Z face (back)
        (
            [0.0, 0.0, -1.0],
            [
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
            ],
            FaceDir::NegZ,
        ),
    ];

    for (face_idx, (normal, corners, face_dir)) in faces.iter().enumerate() {
        let face_base = base_index + (face_idx as u32 * 4);

        // Get AO offsets for this face direction
        let ao_offsets = get_ao_offsets(*face_dir);

        // Add 4 vertices for this face
        for (vert_idx, corner) in corners.iter().enumerate() {
            positions.push([
                base[0] + corner[0],
                base[1] + corner[1],
                base[2] + corner[2],
            ]);
            normals.push(*normal);
            colors.push(color);
            emissions.push(emission);

            // Calculate AO for this vertex
            let offsets = &ao_offsets[vert_idx];
            let side1 = chunk.is_neighbor_solid(vx, vy, vz, offsets[0].0, offsets[0].1, offsets[0].2);
            let side2 = chunk.is_neighbor_solid(vx, vy, vz, offsets[1].0, offsets[1].1, offsets[1].2);
            let corner_solid = chunk.is_neighbor_solid(vx, vy, vz, offsets[2].0, offsets[2].1, offsets[2].2);
            let ao = calculate_vertex_ao(side1, side2, corner_solid);
            aos.push(ao);
        }

        // Add 2 triangles (6 indices) for this face
        // CCW winding: 0-1-2, 0-2-3
        indices.extend_from_slice(&[
            face_base,
            face_base + 1,
            face_base + 2,
            face_base,
            face_base + 2,
            face_base + 3,
        ]);
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::Voxel;

    #[test]
    fn test_empty_chunk_produces_empty_mesh() {
        let chunk = VoxelChunk::new();
        let mesh = build_chunk_mesh(&chunk);

        // Empty mesh should have no vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(positions.len(), 0);
    }

    #[test]
    fn test_single_voxel_produces_24_vertices() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));

        let mesh = build_chunk_mesh(&chunk);

        // 1 voxel = 6 faces * 4 vertices = 24 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(positions.len(), 24);
    }

    #[test]
    fn test_single_voxel_produces_36_indices() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));

        let mesh = build_chunk_mesh(&chunk);

        // 1 voxel = 6 faces * 2 triangles * 3 indices = 36 indices
        let indices = mesh.indices().unwrap();
        assert_eq!(indices.len(), 36);
    }

    #[test]
    fn test_five_voxels_produce_correct_vertex_count() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));
        chunk.set(9, 8, 8, Voxel::solid(0, 255, 0));
        chunk.set(7, 8, 8, Voxel::solid(0, 0, 255));
        chunk.set(8, 8, 9, Voxel::solid(255, 255, 0));
        chunk.set(8, 8, 7, Voxel::solid(0, 255, 255));

        let mesh = build_chunk_mesh(&chunk);

        // 5 voxels = 5 * 24 = 120 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(positions.len(), 120);

        // 5 voxels = 5 * 36 = 180 indices
        let indices = mesh.indices().unwrap();
        assert_eq!(indices.len(), 180);
    }

    #[test]
    fn test_mesh_has_custom_color_attribute() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::new(255, 128, 64, 200));

        let mesh = build_chunk_mesh(&chunk);

        // Check custom voxel color attribute exists (RGB)
        assert!(mesh.attribute(ATTRIBUTE_VOXEL_COLOR).is_some());
    }

    #[test]
    fn test_mesh_has_emission_attribute() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::new(255, 128, 64, 200));

        let mesh = build_chunk_mesh(&chunk);

        // Check emission attribute exists
        assert!(mesh.attribute(ATTRIBUTE_VOXEL_EMISSION).is_some());
    }

    #[test]
    fn test_mesh_has_normal_attribute() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));

        let mesh = build_chunk_mesh(&chunk);

        // Check normal attribute exists
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
    }

    #[test]
    fn test_mesh_has_ao_attribute() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));

        let mesh = build_chunk_mesh(&chunk);

        // Check AO attribute exists
        assert!(mesh.attribute(ATTRIBUTE_VOXEL_AO).is_some());
    }

    #[test]
    fn test_isolated_voxel_has_full_ao() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));

        let mesh = build_chunk_mesh(&chunk);

        // An isolated voxel should have AO = 1.0 for all vertices (no neighbors)
        if let Some(bevy::mesh::VertexAttributeValues::Float32(ao_values)) =
            mesh.attribute(ATTRIBUTE_VOXEL_AO)
        {
            for ao in ao_values {
                assert!(
                    (*ao - 1.0).abs() < 0.01,
                    "Isolated voxel should have AO = 1.0, got {}",
                    ao
                );
            }
        } else {
            panic!("AO attribute not found or wrong type");
        }
    }

    #[test]
    fn test_corner_voxels_have_reduced_ao() {
        let mut chunk = VoxelChunk::new();
        // Create a 2x2x2 cube of voxels - inner corners should have reduced AO
        for x in 7..9 {
            for y in 7..9 {
                for z in 7..9 {
                    chunk.set(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }

        let mesh = build_chunk_mesh(&chunk);

        // With neighbors, some AO values should be less than 1.0
        if let Some(bevy::mesh::VertexAttributeValues::Float32(ao_values)) =
            mesh.attribute(ATTRIBUTE_VOXEL_AO)
        {
            let has_occluded = ao_values.iter().any(|ao| *ao < 0.99);
            assert!(
                has_occluded,
                "2x2x2 cube should have some occluded vertices"
            );
        } else {
            panic!("AO attribute not found or wrong type");
        }
    }
}
