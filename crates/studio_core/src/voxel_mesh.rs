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
//!
//! The custom shader reads these attributes and applies lighting with emission support.

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
/// Vertices include position, normal, color (RGB), and emission attributes.
///
/// The mesh is centered at origin (chunk coords 0-15 map to world -8 to +7).
pub fn build_chunk_mesh(chunk: &VoxelChunk) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut emissions: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Offset to center chunk at origin
    let offset = CHUNK_SIZE as f32 / 2.0;

    for (x, y, z, voxel) in chunk.iter() {
        let base_pos = [x as f32 - offset, y as f32 - offset, z as f32 - offset];

        let color = voxel.color_f32();
        let emission = voxel.emission_f32();

        // Generate 6 faces for this voxel
        add_cube_faces(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut emissions,
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
        .with_inserted_indices(Indices::U32(indices))
}

/// Add 6 faces (24 vertices, 36 indices) for a unit cube at the given position.
fn add_cube_faces(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 3]>,
    emissions: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    base: [f32; 3],
    color: [f32; 3],
    emission: f32,
) {
    let base_index = positions.len() as u32;

    // Face definitions: (normal, 4 corner offsets)
    // Each face is a quad with vertices in CCW order when viewed from outside
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        // +X face (right)
        (
            [1.0, 0.0, 0.0],
            [
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [1.0, 0.0, 1.0],
            ],
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
        ),
    ];

    for (face_idx, (normal, corners)) in faces.iter().enumerate() {
        let face_base = base_index + (face_idx as u32 * 4);

        // Add 4 vertices for this face
        for corner in corners {
            positions.push([
                base[0] + corner[0],
                base[1] + corner[1],
                base[2] + corner[2],
            ]);
            normals.push(*normal);
            colors.push(color);
            emissions.push(emission);
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
}
