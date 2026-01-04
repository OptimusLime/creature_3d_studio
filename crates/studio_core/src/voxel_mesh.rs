//! Voxel mesh generation and custom material.
//!
//! This module provides:
//! - `VoxelMaterial`: Custom material with per-vertex color and emission
//! - `build_chunk_mesh()`: Generates a mesh with face culling only
//! - `build_chunk_mesh_greedy()`: Generates a mesh with face culling AND greedy meshing
//!
//! ## Face Culling
//!
//! Hidden faces (between adjacent solid voxels) are culled to reduce vertex count.
//! For a solid 8x8x8 cube:
//! - Without culling: 512 voxels * 6 faces = 3072 faces
//! - With culling: 6 sides * 64 surface faces = 384 faces (87.5% reduction)
//!
//! ## Greedy Meshing
//!
//! Adjacent faces with the same material (color + emission) are merged into larger quads.
//! For a solid 8x8x8 cube:
//! - With culling only: 384 quads
//! - With greedy meshing: 6 quads (one per side) - 98.4% reduction from culling!
//!
//! Greedy meshing uses the algorithm from Mikola Lysenko:
//! https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/
//!
//! Each visible voxel face becomes 1 quad (4 vertices, 6 indices) with attributes:
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

use crate::voxel::{ChunkPos, Voxel, VoxelChunk, VoxelWorld, CHUNK_SIZE};

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

/// Build a single mesh from a VoxelChunk with face culling.
///
/// Only generates faces for voxels where the adjacent neighbor is empty.
/// This dramatically reduces vertex count for dense voxel meshes.
///
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

        // Compute face visibility mask: only render faces where neighbor is empty
        // Bit 0 = +X, Bit 1 = -X, Bit 2 = +Y, Bit 3 = -Y, Bit 4 = +Z, Bit 5 = -Z
        let mut face_mask: u8 = 0;
        if !chunk.is_neighbor_solid(x, y, z, 1, 0, 0) {
            face_mask |= 1 << 0;
        } // +X
        if !chunk.is_neighbor_solid(x, y, z, -1, 0, 0) {
            face_mask |= 1 << 1;
        } // -X
        if !chunk.is_neighbor_solid(x, y, z, 0, 1, 0) {
            face_mask |= 1 << 2;
        } // +Y
        if !chunk.is_neighbor_solid(x, y, z, 0, -1, 0) {
            face_mask |= 1 << 3;
        } // -Y
        if !chunk.is_neighbor_solid(x, y, z, 0, 0, 1) {
            face_mask |= 1 << 4;
        } // +Z
        if !chunk.is_neighbor_solid(x, y, z, 0, 0, -1) {
            face_mask |= 1 << 5;
        } // -Z

        // Generate only visible faces for this voxel
        add_cube_faces_with_ao(
            chunk,
            x,
            y,
            z,
            face_mask,
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
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_COLOR, colors)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_EMISSION, emissions)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_AO, aos)
    .with_inserted_indices(Indices::U32(indices))
}

// =============================================================================
// GREEDY MESHING
// =============================================================================

/// Material key for greedy meshing - faces can only merge if they have the same key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct FaceKey {
    /// Packed color (RGB) and emission into a single u32 for fast comparison
    /// Format: 0xRRGGBBEE
    packed: u32,
}

impl FaceKey {
    fn new(color: [u8; 3], emission: u8) -> Self {
        Self {
            packed: ((color[0] as u32) << 24)
                | ((color[1] as u32) << 16)
                | ((color[2] as u32) << 8)
                | (emission as u32),
        }
    }

    fn from_voxel(voxel: &Voxel) -> Self {
        Self::new(voxel.color, voxel.emission)
    }

    fn color_f32(&self) -> [f32; 3] {
        [
            ((self.packed >> 24) & 0xFF) as f32 / 255.0,
            ((self.packed >> 16) & 0xFF) as f32 / 255.0,
            ((self.packed >> 8) & 0xFF) as f32 / 255.0,
        ]
    }

    fn emission_f32(&self) -> f32 {
        (self.packed & 0xFF) as f32 / 255.0
    }
}

/// A merged quad from greedy meshing
#[derive(Debug)]
struct GreedyQuad {
    /// Position in the slice (u, v coordinates)
    u: usize,
    v: usize,
    /// Which slice along the axis
    slice: usize,
    /// Size of merged quad
    width: usize,
    height: usize,
    /// Face direction
    direction: FaceDir,
    /// Material key
    key: FaceKey,
}

/// Build a mesh from a VoxelChunk using greedy meshing.
///
/// This combines face culling with greedy meshing for maximum optimization.
/// Adjacent faces with the same color and emission are merged into larger quads.
///
/// For a solid 8x8x8 cube:
/// - Face culling only: 384 quads (6 sides * 64 faces)
/// - Greedy meshing: 6 quads (one per side)
///
/// The mesh is centered at origin (chunk coords map to world centered at 0).
pub fn build_chunk_mesh_greedy(chunk: &VoxelChunk) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut emissions: Vec<f32> = Vec::new();
    let mut aos: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Offset to center chunk at origin
    let offset = CHUNK_SIZE as f32 / 2.0;

    // Process each of 6 face directions
    for direction in [
        FaceDir::PosX,
        FaceDir::NegX,
        FaceDir::PosY,
        FaceDir::NegY,
        FaceDir::PosZ,
        FaceDir::NegZ,
    ] {
        // Process each slice perpendicular to this direction
        for slice in 0..CHUNK_SIZE {
            // Build 2D mask of visible faces
            let mut mask = build_slice_mask(chunk, direction, slice);

            // Greedy merge the mask
            let quads = greedy_merge_slice(&mut mask, direction, slice);

            // Convert quads to vertices
            for quad in quads {
                emit_greedy_quad(
                    chunk,
                    &quad,
                    offset,
                    &mut positions,
                    &mut normals,
                    &mut colors,
                    &mut emissions,
                    &mut aos,
                    &mut indices,
                );
            }
        }
    }

    // Create mesh with custom attributes for VoxelMaterial
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_COLOR, colors)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_EMISSION, emissions)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_AO, aos)
    .with_inserted_indices(Indices::U32(indices))
}

// ============================================================================
// MULTI-CHUNK WORLD MESH GENERATION
// ============================================================================

/// Result of building a mesh for a single chunk in a world.
///
/// Contains the mesh and the world-space transform for positioning.
#[derive(Debug)]
pub struct ChunkMesh {
    /// The chunk position in chunk coordinates.
    pub chunk_pos: ChunkPos,
    /// The generated mesh.
    pub mesh: Mesh,
    /// World-space translation for this chunk.
    /// The mesh vertices are local to the chunk (centered at origin),
    /// so this transform positions the chunk in world space.
    pub world_offset: [f32; 3],
}

impl ChunkMesh {
    /// Get the world-space translation as a Vec3.
    pub fn translation(&self) -> Vec3 {
        Vec3::from_array(self.world_offset)
    }
}

/// Build meshes for all chunks in a VoxelWorld.
///
/// Returns a mesh for each non-empty chunk along with its world-space position.
/// Uses greedy meshing for optimal vertex count.
///
/// # Example
///
/// ```ignore
/// let world = VoxelWorld::new();
/// // ... populate world with voxels ...
///
/// for chunk_mesh in build_world_meshes(&world) {
///     let mesh_handle = meshes.add(chunk_mesh.mesh);
///     commands.spawn((
///         Mesh3d(mesh_handle),
///         Transform::from_translation(chunk_mesh.translation()),
///         // ... other components
///     ));
/// }
/// ```
pub fn build_world_meshes(world: &VoxelWorld) -> Vec<ChunkMesh> {
    build_world_meshes_with_options(world, true)
}

/// Build meshes for all chunks in a VoxelWorld with configurable options.
///
/// # Arguments
/// * `world` - The voxel world to mesh
/// * `use_greedy` - If true, use greedy meshing; if false, use face culling only
pub fn build_world_meshes_with_options(world: &VoxelWorld, use_greedy: bool) -> Vec<ChunkMesh> {
    world
        .iter_chunks()
        .filter(|(_, chunk)| !chunk.is_empty())
        .map(|(chunk_pos, chunk)| {
            // Build the mesh for this chunk
            let mesh = if use_greedy {
                build_chunk_mesh_greedy(chunk)
            } else {
                build_chunk_mesh(chunk)
            };

            // Calculate world-space offset for this chunk
            // The mesh is centered at origin, so we offset to the chunk's world position
            // Note: build_chunk_mesh centers the mesh, so a voxel at local (0,0,0)
            // is at mesh position (-CHUNK_SIZE/2, -CHUNK_SIZE/2, -CHUNK_SIZE/2).
            // The chunk's world origin is chunk_pos * CHUNK_SIZE.
            // To position correctly: translate by (chunk_pos * CHUNK_SIZE + CHUNK_SIZE/2)
            // which places the mesh center at the chunk center in world space.
            let half = CHUNK_SIZE as f32 / 2.0;
            let world_offset = [
                chunk_pos.x as f32 * CHUNK_SIZE as f32 + half,
                chunk_pos.y as f32 * CHUNK_SIZE as f32 + half,
                chunk_pos.z as f32 * CHUNK_SIZE as f32 + half,
            ];

            ChunkMesh {
                chunk_pos,
                mesh,
                world_offset,
            }
        })
        .collect()
}

/// Build a mesh for a single chunk at a specific position.
///
/// Convenience function when you only need to update one chunk.
pub fn build_single_chunk_mesh(
    chunk: &VoxelChunk,
    chunk_pos: ChunkPos,
    use_greedy: bool,
) -> ChunkMesh {
    let mesh = if use_greedy {
        build_chunk_mesh_greedy(chunk)
    } else {
        build_chunk_mesh(chunk)
    };

    let half = CHUNK_SIZE as f32 / 2.0;
    let world_offset = [
        chunk_pos.x as f32 * CHUNK_SIZE as f32 + half,
        chunk_pos.y as f32 * CHUNK_SIZE as f32 + half,
        chunk_pos.z as f32 * CHUNK_SIZE as f32 + half,
    ];

    ChunkMesh {
        chunk_pos,
        mesh,
        world_offset,
    }
}

// ============================================================================
// CROSS-CHUNK FACE CULLING
// ============================================================================

use crate::voxel::ChunkBorders;

/// Build meshes for all chunks in a VoxelWorld with cross-chunk face culling.
///
/// This is the preferred method for multi-chunk worlds as it eliminates seams
/// at chunk boundaries by checking neighbor chunks for adjacent voxels.
///
/// For each chunk, we extract border data from its 6 neighbors and use that
/// information when generating faces at chunk boundaries. This ensures that
/// faces between adjacent solid voxels across chunk boundaries are culled.
///
/// # Example
///
/// ```ignore
/// let world = VoxelWorld::new();
/// // ... populate world with voxels ...
///
/// // With cross-chunk culling (no seams at boundaries)
/// for chunk_mesh in build_world_meshes_cross_chunk(&world) {
///     let mesh_handle = meshes.add(chunk_mesh.mesh);
///     commands.spawn((
///         Mesh3d(mesh_handle),
///         Transform::from_translation(chunk_mesh.translation()),
///     ));
/// }
/// ```
pub fn build_world_meshes_cross_chunk(world: &VoxelWorld) -> Vec<ChunkMesh> {
    build_world_meshes_cross_chunk_with_options(world, true)
}

/// Build meshes with cross-chunk face culling and configurable options.
///
/// # Arguments
/// * `world` - The voxel world to mesh
/// * `use_greedy` - If true, use greedy meshing; if false, use face culling only
pub fn build_world_meshes_cross_chunk_with_options(
    world: &VoxelWorld,
    use_greedy: bool,
) -> Vec<ChunkMesh> {
    world
        .iter_chunks()
        .filter(|(_, chunk)| !chunk.is_empty())
        .map(|(chunk_pos, chunk)| {
            // Extract border data from neighboring chunks
            let borders = world.extract_borders(chunk_pos);

            // Build the mesh with cross-chunk culling
            let mesh = if use_greedy {
                build_chunk_mesh_greedy_with_borders(chunk, &borders)
            } else {
                build_chunk_mesh_with_borders(chunk, &borders)
            };

            let half = CHUNK_SIZE as f32 / 2.0;
            let world_offset = [
                chunk_pos.x as f32 * CHUNK_SIZE as f32 + half,
                chunk_pos.y as f32 * CHUNK_SIZE as f32 + half,
                chunk_pos.z as f32 * CHUNK_SIZE as f32 + half,
            ];

            ChunkMesh {
                chunk_pos,
                mesh,
                world_offset,
            }
        })
        .collect()
}

/// Build a mesh for a single chunk with border data from neighbors.
///
/// This is the cross-chunk-aware version of `build_chunk_mesh()`.
/// Faces at chunk boundaries are culled if the neighbor chunk has a solid voxel.
pub fn build_chunk_mesh_with_borders(chunk: &VoxelChunk, borders: &ChunkBorders) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut emissions: Vec<f32> = Vec::new();
    let mut aos: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let offset = CHUNK_SIZE as f32 / 2.0;

    for (x, y, z, voxel) in chunk.iter() {
        let base_pos = [x as f32 - offset, y as f32 - offset, z as f32 - offset];
        let color = voxel.color_f32();
        let emission = voxel.emission_f32();

        // Compute face visibility mask with cross-chunk awareness
        let mut face_mask: u8 = 0;

        // +X face
        if !is_neighbor_solid_cross_chunk(chunk, borders, x, y, z, 1, 0, 0) {
            face_mask |= 1 << 0;
        }
        // -X face
        if !is_neighbor_solid_cross_chunk(chunk, borders, x, y, z, -1, 0, 0) {
            face_mask |= 1 << 1;
        }
        // +Y face
        if !is_neighbor_solid_cross_chunk(chunk, borders, x, y, z, 0, 1, 0) {
            face_mask |= 1 << 2;
        }
        // -Y face
        if !is_neighbor_solid_cross_chunk(chunk, borders, x, y, z, 0, -1, 0) {
            face_mask |= 1 << 3;
        }
        // +Z face
        if !is_neighbor_solid_cross_chunk(chunk, borders, x, y, z, 0, 0, 1) {
            face_mask |= 1 << 4;
        }
        // -Z face
        if !is_neighbor_solid_cross_chunk(chunk, borders, x, y, z, 0, 0, -1) {
            face_mask |= 1 << 5;
        }

        add_cube_faces_with_ao_cross_chunk(
            chunk,
            borders,
            x,
            y,
            z,
            face_mask,
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

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_COLOR, colors)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_EMISSION, emissions)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_AO, aos)
    .with_inserted_indices(Indices::U32(indices))
}

/// Build a mesh with greedy meshing and cross-chunk face culling.
pub fn build_chunk_mesh_greedy_with_borders(chunk: &VoxelChunk, borders: &ChunkBorders) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut emissions: Vec<f32> = Vec::new();
    let mut aos: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let offset = CHUNK_SIZE as f32 / 2.0;

    // Process each of 6 face directions
    for direction in [
        FaceDir::PosX,
        FaceDir::NegX,
        FaceDir::PosY,
        FaceDir::NegY,
        FaceDir::PosZ,
        FaceDir::NegZ,
    ] {
        for slice in 0..CHUNK_SIZE {
            // Build 2D mask with cross-chunk awareness
            let mut mask = build_slice_mask_with_borders(chunk, borders, direction, slice);

            // Greedy merge the mask
            let quads = greedy_merge_slice(&mut mask, direction, slice);

            // Convert quads to vertices
            for quad in quads {
                emit_greedy_quad_with_borders(
                    chunk,
                    borders,
                    &quad,
                    offset,
                    &mut positions,
                    &mut normals,
                    &mut colors,
                    &mut emissions,
                    &mut aos,
                    &mut indices,
                );
            }
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_COLOR, colors)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_EMISSION, emissions)
    .with_inserted_attribute(ATTRIBUTE_VOXEL_AO, aos)
    .with_inserted_indices(Indices::U32(indices))
}

/// Check if a neighbor voxel is solid, including across chunk boundaries.
#[inline]
fn is_neighbor_solid_cross_chunk(
    chunk: &VoxelChunk,
    borders: &ChunkBorders,
    x: usize,
    y: usize,
    z: usize,
    dx: i32,
    dy: i32,
    dz: i32,
) -> bool {
    let nx = x as i32 + dx;
    let ny = y as i32 + dy;
    let nz = z as i32 + dz;

    // Check if within chunk bounds
    if nx >= 0
        && nx < CHUNK_SIZE as i32
        && ny >= 0
        && ny < CHUNK_SIZE as i32
        && nz >= 0
        && nz < CHUNK_SIZE as i32
    {
        // Within chunk - use chunk's data
        chunk.get(nx as usize, ny as usize, nz as usize).is_some()
    } else {
        // Outside chunk - use border data
        borders.is_neighbor_solid(x, y, z, dx, dy, dz)
    }
}

/// Build a 2D mask of visible faces with cross-chunk border awareness.
fn build_slice_mask_with_borders(
    chunk: &VoxelChunk,
    borders: &ChunkBorders,
    direction: FaceDir,
    slice: usize,
) -> [[Option<FaceKey>; CHUNK_SIZE]; CHUNK_SIZE] {
    let mut mask = [[None; CHUNK_SIZE]; CHUNK_SIZE];

    let (slice_axis, u_axis, v_axis, neighbor_offset) = direction.axis_config();

    for v in 0..CHUNK_SIZE {
        for u in 0..CHUNK_SIZE {
            let mut pos = [0usize; 3];
            pos[slice_axis] = slice;
            pos[u_axis] = u;
            pos[v_axis] = v;

            let (x, y, z) = (pos[0], pos[1], pos[2]);

            if let Some(voxel) = chunk.get(x, y, z) {
                let (dx, dy, dz) = neighbor_offset;
                // Use cross-chunk neighbor check
                if !is_neighbor_solid_cross_chunk(chunk, borders, x, y, z, dx, dy, dz) {
                    mask[v][u] = Some(FaceKey::from_voxel(&voxel));
                }
            }
        }
    }

    mask
}

/// Emit vertices for a greedy quad with cross-chunk AO calculation.
#[allow(clippy::too_many_arguments)]
fn emit_greedy_quad_with_borders(
    chunk: &VoxelChunk,
    borders: &ChunkBorders,
    quad: &GreedyQuad,
    offset: f32,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 3]>,
    emissions: &mut Vec<f32>,
    aos: &mut Vec<f32>,
    indices: &mut Vec<u32>,
) {
    let base_idx = positions.len() as u32;

    let corners = quad.world_corners(offset);
    let normal = quad.direction.normal();
    let color = quad.key.color_f32();
    let emission = quad.key.emission_f32();

    let corner_voxels = quad.corner_voxel_positions();

    // Calculate AO for all 4 corners first
    let mut ao_values = [0.0f32; 4];
    for (i, corner) in corners.iter().enumerate() {
        positions.push(*corner);
        normals.push(normal);
        colors.push(color);
        emissions.push(emission);

        let (vx, vy, vz) = corner_voxels[i];
        ao_values[i] =
            calculate_corner_ao_cross_chunk(chunk, borders, quad.direction, vx, vy, vz, i);
        aos.push(ao_values[i]);
    }

    // Quad flip for AO: choose the diagonal that minimizes interpolation artifacts.
    // If AO[0] + AO[2] > AO[1] + AO[3], use diagonal 1-3 instead of 0-2.
    // This prevents visible seams when AO values vary significantly across the quad.
    // Reference: https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/
    if ao_values[0] + ao_values[2] > ao_values[1] + ao_values[3] {
        // Flip: use triangles (1-2-3) and (1-3-0)
        indices.extend_from_slice(&[
            base_idx + 1,
            base_idx + 2,
            base_idx + 3,
            base_idx + 1,
            base_idx + 3,
            base_idx,
        ]);
    } else {
        // Normal: use triangles (0-1-2) and (0-2-3)
        indices.extend_from_slice(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx,
            base_idx + 2,
            base_idx + 3,
        ]);
    }
}

/// Calculate AO for a corner with cross-chunk awareness.
fn calculate_corner_ao_cross_chunk(
    chunk: &VoxelChunk,
    borders: &ChunkBorders,
    direction: FaceDir,
    vx: usize,
    vy: usize,
    vz: usize,
    corner_idx: usize,
) -> f32 {
    let ao_offsets = get_ao_offsets(direction);
    let offsets = &ao_offsets[corner_idx];

    let side1 = is_neighbor_solid_cross_chunk(
        chunk,
        borders,
        vx,
        vy,
        vz,
        offsets[0].0,
        offsets[0].1,
        offsets[0].2,
    );
    let side2 = is_neighbor_solid_cross_chunk(
        chunk,
        borders,
        vx,
        vy,
        vz,
        offsets[1].0,
        offsets[1].1,
        offsets[1].2,
    );
    let corner = is_neighbor_solid_cross_chunk(
        chunk,
        borders,
        vx,
        vy,
        vz,
        offsets[2].0,
        offsets[2].1,
        offsets[2].2,
    );

    calculate_vertex_ao(side1, side2, corner)
}

/// Add cube faces with AO, using cross-chunk neighbor checking.
#[allow(clippy::too_many_arguments)]
fn add_cube_faces_with_ao_cross_chunk(
    chunk: &VoxelChunk,
    borders: &ChunkBorders,
    vx: usize,
    vy: usize,
    vz: usize,
    face_mask: u8,
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
    let faces: [([f32; 3], [[f32; 3]; 4], FaceDir, u8); 6] = [
        (
            [1.0, 0.0, 0.0],
            [
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [1.0, 0.0, 1.0],
            ],
            FaceDir::PosX,
            1 << 0,
        ),
        (
            [-1.0, 0.0, 0.0],
            [
                [0.0, 0.0, 1.0],
                [0.0, 1.0, 1.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
            ],
            FaceDir::NegX,
            1 << 1,
        ),
        (
            [0.0, 1.0, 0.0],
            [
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
            ],
            FaceDir::PosY,
            1 << 2,
        ),
        (
            [0.0, -1.0, 0.0],
            [
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
            ],
            FaceDir::NegY,
            1 << 3,
        ),
        (
            [0.0, 0.0, 1.0],
            [
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
            FaceDir::PosZ,
            1 << 4,
        ),
        (
            [0.0, 0.0, -1.0],
            [
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
            ],
            FaceDir::NegZ,
            1 << 5,
        ),
    ];

    for (normal, corners, face_dir, mask_bit) in faces.iter() {
        if face_mask & mask_bit == 0 {
            continue;
        }

        let face_base = positions.len() as u32;
        let ao_offsets = get_ao_offsets(*face_dir);

        // Calculate AO for all 4 corners first
        let mut ao_values = [0.0f32; 4];
        for (vert_idx, corner) in corners.iter().enumerate() {
            positions.push([
                base[0] + corner[0],
                base[1] + corner[1],
                base[2] + corner[2],
            ]);
            normals.push(*normal);
            colors.push(color);
            emissions.push(emission);

            let offsets = &ao_offsets[vert_idx];
            let side1 = is_neighbor_solid_cross_chunk(
                chunk,
                borders,
                vx,
                vy,
                vz,
                offsets[0].0,
                offsets[0].1,
                offsets[0].2,
            );
            let side2 = is_neighbor_solid_cross_chunk(
                chunk,
                borders,
                vx,
                vy,
                vz,
                offsets[1].0,
                offsets[1].1,
                offsets[1].2,
            );
            let corner_solid = is_neighbor_solid_cross_chunk(
                chunk,
                borders,
                vx,
                vy,
                vz,
                offsets[2].0,
                offsets[2].1,
                offsets[2].2,
            );
            ao_values[vert_idx] = calculate_vertex_ao(side1, side2, corner_solid);
            aos.push(ao_values[vert_idx]);
        }

        // Quad flip for AO: choose the diagonal that minimizes interpolation artifacts.
        if ao_values[0] + ao_values[2] > ao_values[1] + ao_values[3] {
            // Flip: use triangles (1-2-3) and (1-3-0)
            indices.extend_from_slice(&[
                face_base + 1,
                face_base + 2,
                face_base + 3,
                face_base + 1,
                face_base + 3,
                face_base,
            ]);
        } else {
            // Normal: use triangles (0-1-2) and (0-2-3)
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
}

/// Build a 2D mask of visible faces for a single slice.
///
/// For each cell in the slice, we store Some(FaceKey) if there's a visible face,
/// or None if no face (either no voxel or face is hidden by neighbor).
fn build_slice_mask(
    chunk: &VoxelChunk,
    direction: FaceDir,
    slice: usize,
) -> [[Option<FaceKey>; CHUNK_SIZE]; CHUNK_SIZE] {
    let mut mask = [[None; CHUNK_SIZE]; CHUNK_SIZE];

    // Get the axis configuration for this direction
    let (slice_axis, u_axis, v_axis, neighbor_offset) = direction.axis_config();

    for v in 0..CHUNK_SIZE {
        for u in 0..CHUNK_SIZE {
            // Convert (slice, u, v) to (x, y, z)
            let mut pos = [0usize; 3];
            pos[slice_axis] = slice;
            pos[u_axis] = u;
            pos[v_axis] = v;

            let (x, y, z) = (pos[0], pos[1], pos[2]);

            // Check if there's a voxel at this position
            if let Some(voxel) = chunk.get(x, y, z) {
                // Check if the face in this direction is visible (neighbor is empty)
                let (dx, dy, dz) = neighbor_offset;
                if !chunk.is_neighbor_solid(x, y, z, dx, dy, dz) {
                    mask[v][u] = Some(FaceKey::from_voxel(&voxel));
                }
            }
        }
    }

    mask
}

/// Perform greedy merging on a 2D slice mask.
///
/// Scans the mask and merges adjacent cells with the same FaceKey into larger quads.
fn greedy_merge_slice(
    mask: &mut [[Option<FaceKey>; CHUNK_SIZE]; CHUNK_SIZE],
    direction: FaceDir,
    slice: usize,
) -> Vec<GreedyQuad> {
    let mut quads = Vec::new();

    for v in 0..CHUNK_SIZE {
        let mut u = 0;
        while u < CHUNK_SIZE {
            if let Some(key) = mask[v][u] {
                // Found a face - compute width (extend right while key matches)
                let mut w = 1;
                while u + w < CHUNK_SIZE && mask[v][u + w] == Some(key) {
                    w += 1;
                }

                // Compute height (extend down while entire row matches)
                let mut h = 1;
                'height: while v + h < CHUNK_SIZE {
                    for k in 0..w {
                        if mask[v + h][u + k] != Some(key) {
                            break 'height;
                        }
                    }
                    h += 1;
                }

                // Emit quad
                quads.push(GreedyQuad {
                    u,
                    v,
                    slice,
                    width: w,
                    height: h,
                    direction,
                    key,
                });

                // Clear mask in merged region
                for dv in 0..h {
                    for du in 0..w {
                        mask[v + dv][u + du] = None;
                    }
                }

                u += w;
            } else {
                u += 1;
            }
        }
    }

    quads
}

/// Emit vertices for a greedy quad.
#[allow(clippy::too_many_arguments)]
fn emit_greedy_quad(
    chunk: &VoxelChunk,
    quad: &GreedyQuad,
    offset: f32,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 3]>,
    emissions: &mut Vec<f32>,
    aos: &mut Vec<f32>,
    indices: &mut Vec<u32>,
) {
    let base_idx = positions.len() as u32;

    // Get the 4 corner positions of the merged quad in world space
    let corners = quad.world_corners(offset);
    let normal = quad.direction.normal();
    let color = quad.key.color_f32();
    let emission = quad.key.emission_f32();

    // Get the 4 corner voxel positions for AO calculation
    let corner_voxels = quad.corner_voxel_positions();

    // Calculate AO for all 4 corners first
    let mut ao_values = [0.0f32; 4];
    for (i, corner) in corners.iter().enumerate() {
        positions.push(*corner);
        normals.push(normal);
        colors.push(color);
        emissions.push(emission);

        // Calculate AO for this corner using the corner voxel
        let (vx, vy, vz) = corner_voxels[i];
        ao_values[i] = calculate_corner_ao(chunk, quad.direction, vx, vy, vz, i);
        aos.push(ao_values[i]);
    }

    // Quad flip for AO: choose the diagonal that minimizes interpolation artifacts.
    // If AO[0] + AO[2] > AO[1] + AO[3], use diagonal 1-3 instead of 0-2.
    // This prevents visible seams when AO values vary significantly across the quad.
    // Reference: https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/
    if ao_values[0] + ao_values[2] > ao_values[1] + ao_values[3] {
        // Flip: use triangles (1-2-3) and (1-3-0)
        indices.extend_from_slice(&[
            base_idx + 1,
            base_idx + 2,
            base_idx + 3,
            base_idx + 1,
            base_idx + 3,
            base_idx,
        ]);
    } else {
        // Normal: use triangles (0-1-2) and (0-2-3)
        indices.extend_from_slice(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx,
            base_idx + 2,
            base_idx + 3,
        ]);
    }
}

/// Calculate AO for a specific corner of a face.
fn calculate_corner_ao(
    chunk: &VoxelChunk,
    direction: FaceDir,
    vx: usize,
    vy: usize,
    vz: usize,
    corner_idx: usize,
) -> f32 {
    let ao_offsets = get_ao_offsets(direction);
    let offsets = &ao_offsets[corner_idx];

    let side1 = chunk.is_neighbor_solid(vx, vy, vz, offsets[0].0, offsets[0].1, offsets[0].2);
    let side2 = chunk.is_neighbor_solid(vx, vy, vz, offsets[1].0, offsets[1].1, offsets[1].2);
    let corner = chunk.is_neighbor_solid(vx, vy, vz, offsets[2].0, offsets[2].1, offsets[2].2);

    calculate_vertex_ao(side1, side2, corner)
}

impl FaceDir {
    /// Get the axis configuration for this face direction.
    ///
    /// Returns: (slice_axis, u_axis, v_axis, neighbor_offset)
    /// - slice_axis: which axis we're slicing along (0=X, 1=Y, 2=Z)
    /// - u_axis: first perpendicular axis
    /// - v_axis: second perpendicular axis
    /// - neighbor_offset: (dx, dy, dz) to check for face visibility
    fn axis_config(&self) -> (usize, usize, usize, (i32, i32, i32)) {
        match self {
            FaceDir::PosX => (0, 2, 1, (1, 0, 0)),  // slice X, u=Z, v=Y
            FaceDir::NegX => (0, 2, 1, (-1, 0, 0)), // slice X, u=Z, v=Y
            FaceDir::PosY => (1, 0, 2, (0, 1, 0)),  // slice Y, u=X, v=Z
            FaceDir::NegY => (1, 0, 2, (0, -1, 0)), // slice Y, u=X, v=Z
            FaceDir::PosZ => (2, 0, 1, (0, 0, 1)),  // slice Z, u=X, v=Y
            FaceDir::NegZ => (2, 0, 1, (0, 0, -1)), // slice Z, u=X, v=Y
        }
    }

    /// Get the normal vector for this face direction.
    fn normal(&self) -> [f32; 3] {
        match self {
            FaceDir::PosX => [1.0, 0.0, 0.0],
            FaceDir::NegX => [-1.0, 0.0, 0.0],
            FaceDir::PosY => [0.0, 1.0, 0.0],
            FaceDir::NegY => [0.0, -1.0, 0.0],
            FaceDir::PosZ => [0.0, 0.0, 1.0],
            FaceDir::NegZ => [0.0, 0.0, -1.0],
        }
    }
}

impl GreedyQuad {
    /// Get the 4 corner positions of this quad in world space.
    ///
    /// Returns corners in CCW order matching the vertex order used for indexing.
    fn world_corners(&self, offset: f32) -> [[f32; 3]; 4] {
        let (slice_axis, u_axis, v_axis, neighbor_offset) = self.direction.axis_config();

        // The face is at slice position, offset by 0 or 1 depending on direction
        let face_offset = if neighbor_offset.0 > 0 || neighbor_offset.1 > 0 || neighbor_offset.2 > 0
        {
            1.0 // Positive direction: face is at slice + 1
        } else {
            0.0 // Negative direction: face is at slice
        };

        // Build the 4 corners
        let mut corners = [[0.0f32; 3]; 4];

        // Corner positions in (u, v) space - CCW order
        // The order depends on face direction to maintain consistent winding
        let uv_corners = self.direction.corner_uv_order(self.width, self.height);

        for (i, (du, dv)) in uv_corners.iter().enumerate() {
            let mut pos = [0.0f32; 3];
            pos[slice_axis] = self.slice as f32 + face_offset - offset;
            pos[u_axis] = (self.u as f32 + *du as f32) - offset;
            pos[v_axis] = (self.v as f32 + *dv as f32) - offset;
            corners[i] = pos;
        }

        corners
    }

    /// Get the voxel positions for the 4 corners (for AO calculation).
    ///
    /// Returns the voxel coordinate that each corner vertex belongs to.
    /// IMPORTANT: The ordering MUST match corner_uv_order() for each face direction,
    /// otherwise AO values will be assigned to the wrong vertices!
    fn corner_voxel_positions(&self) -> [(usize, usize, usize); 4] {
        let (slice_axis, u_axis, v_axis, _) = self.direction.axis_config();

        let w = self.width.saturating_sub(1);
        let h = self.height.saturating_sub(1);

        // Voxel positions must match the vertex ordering in corner_uv_order().
        // For each vertex at (u_offset, v_offset), we map to the nearest voxel in the quad:
        // - Vertex at (0, 0) -> voxel (0, 0)
        // - Vertex at (width, 0) -> voxel (w, 0) where w = width-1
        // - Vertex at (width, height) -> voxel (w, h) where h = height-1
        // - Vertex at (0, height) -> voxel (0, h)
        //
        // The ordering matches corner_uv_order() for each face direction.
        let uv_voxels: [(usize, usize); 4] = match self.direction {
            // PosX: [(0, 0), (0, height), (width, height), (width, 0)]
            FaceDir::PosX => [(0, 0), (0, h), (w, h), (w, 0)],
            // NegX: [(width, 0), (width, height), (0, height), (0, 0)]
            FaceDir::NegX => [(w, 0), (w, h), (0, h), (0, 0)],
            // PosY: [(0, 0), (0, height), (width, height), (width, 0)]
            FaceDir::PosY => [(0, 0), (0, h), (w, h), (w, 0)],
            // NegY: [(0, height), (0, 0), (width, 0), (width, height)]
            FaceDir::NegY => [(0, h), (0, 0), (w, 0), (w, h)],
            // PosZ: [(0, 0), (width, 0), (width, height), (0, height)]
            FaceDir::PosZ => [(0, 0), (w, 0), (w, h), (0, h)],
            // NegZ: [(width, 0), (0, 0), (0, height), (width, height)]
            FaceDir::NegZ => [(w, 0), (0, 0), (0, h), (w, h)],
        };

        let mut voxels = [(0usize, 0usize, 0usize); 4];
        for (i, (du, dv)) in uv_voxels.iter().enumerate() {
            let mut pos = [0usize; 3];
            pos[slice_axis] = self.slice;
            pos[u_axis] = self.u + du;
            pos[v_axis] = self.v + dv;
            voxels[i] = (pos[0], pos[1], pos[2]);
        }

        voxels
    }
}

impl FaceDir {
    /// Get the corner UV order for proper CCW winding.
    ///
    /// Returns 4 (du, dv) offsets for the quad corners.
    fn corner_uv_order(&self, width: usize, height: usize) -> [(usize, usize); 4] {
        // The winding order depends on the face direction to ensure
        // consistent CCW winding when viewed from outside the voxel
        match self {
            // +X: looking from +X toward -X
            FaceDir::PosX => [(0, 0), (0, height), (width, height), (width, 0)],
            // -X: looking from -X toward +X
            FaceDir::NegX => [(width, 0), (width, height), (0, height), (0, 0)],
            // +Y: looking from +Y toward -Y
            FaceDir::PosY => [(0, 0), (0, height), (width, height), (width, 0)],
            // -Y: looking from -Y toward +Y
            FaceDir::NegY => [(0, height), (0, 0), (width, 0), (width, height)],
            // +Z: looking from +Z toward -Z
            FaceDir::PosZ => [(0, 0), (width, 0), (width, height), (0, height)],
            // -Z: looking from -Z toward +Z
            FaceDir::NegZ => [(width, 0), (0, 0), (0, height), (width, height)],
        }
    }
}

// =============================================================================
// FACE CULLING (NON-GREEDY)
// =============================================================================

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
#[derive(Clone, Copy, Debug)]
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

/// Add visible faces for a unit cube at the given position.
///
/// Only adds faces where the corresponding bit in `face_mask` is set:
/// - Bit 0 = +X, Bit 1 = -X, Bit 2 = +Y, Bit 3 = -Y, Bit 4 = +Z, Bit 5 = -Z
///
/// Includes per-vertex ambient occlusion calculation.
#[allow(clippy::too_many_arguments)]
fn add_cube_faces_with_ao(
    chunk: &VoxelChunk,
    vx: usize,
    vy: usize,
    vz: usize,
    face_mask: u8,
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
    // Face definitions: (normal, 4 corner offsets, face direction for AO, mask bit)
    let faces: [([f32; 3], [[f32; 3]; 4], FaceDir, u8); 6] = [
        // +X face (right) - bit 0
        (
            [1.0, 0.0, 0.0],
            [
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [1.0, 0.0, 1.0],
            ],
            FaceDir::PosX,
            1 << 0,
        ),
        // -X face (left) - bit 1
        (
            [-1.0, 0.0, 0.0],
            [
                [0.0, 0.0, 1.0],
                [0.0, 1.0, 1.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
            ],
            FaceDir::NegX,
            1 << 1,
        ),
        // +Y face (top) - bit 2
        (
            [0.0, 1.0, 0.0],
            [
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
            ],
            FaceDir::PosY,
            1 << 2,
        ),
        // -Y face (bottom) - bit 3
        (
            [0.0, -1.0, 0.0],
            [
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
            ],
            FaceDir::NegY,
            1 << 3,
        ),
        // +Z face (front) - bit 4
        (
            [0.0, 0.0, 1.0],
            [
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
            FaceDir::PosZ,
            1 << 4,
        ),
        // -Z face (back) - bit 5
        (
            [0.0, 0.0, -1.0],
            [
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
            ],
            FaceDir::NegZ,
            1 << 5,
        ),
    ];

    for (normal, corners, face_dir, mask_bit) in faces.iter() {
        // Skip this face if the corresponding bit is not set (neighbor is solid)
        if face_mask & mask_bit == 0 {
            continue;
        }

        let face_base = positions.len() as u32;

        // Get AO offsets for this face direction
        let ao_offsets = get_ao_offsets(*face_dir);

        // Calculate AO for all 4 corners first
        let mut ao_values = [0.0f32; 4];
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
            let side1 =
                chunk.is_neighbor_solid(vx, vy, vz, offsets[0].0, offsets[0].1, offsets[0].2);
            let side2 =
                chunk.is_neighbor_solid(vx, vy, vz, offsets[1].0, offsets[1].1, offsets[1].2);
            let corner_solid =
                chunk.is_neighbor_solid(vx, vy, vz, offsets[2].0, offsets[2].1, offsets[2].2);
            ao_values[vert_idx] = calculate_vertex_ao(side1, side2, corner_solid);
            aos.push(ao_values[vert_idx]);
        }

        // Quad flip for AO: choose the diagonal that minimizes interpolation artifacts.
        // If AO[0] + AO[2] > AO[1] + AO[3], use diagonal 1-3 instead of 0-2.
        // Reference: https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/
        if ao_values[0] + ao_values[2] > ao_values[1] + ao_values[3] {
            // Flip: use triangles (1-2-3) and (1-3-0)
            indices.extend_from_slice(&[
                face_base + 1,
                face_base + 2,
                face_base + 3,
                face_base + 1,
                face_base + 3,
                face_base,
            ]);
        } else {
            // Normal: use triangles (0-1-2) and (0-2-3)
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
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // 1 isolated voxel = 6 visible faces * 4 vertices = 24 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(positions.len(), 24);
    }

    #[test]
    fn test_single_voxel_produces_36_indices() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));

        let mesh = build_chunk_mesh(&chunk);

        // 1 isolated voxel = 6 visible faces * 2 triangles * 3 indices = 36 indices
        let indices = mesh.indices().unwrap();
        assert_eq!(indices.len(), 36);
    }

    #[test]
    fn test_two_adjacent_voxels_face_culling() {
        let mut chunk = VoxelChunk::new();
        // Two voxels adjacent along X axis
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));
        chunk.set(9, 8, 8, Voxel::solid(0, 255, 0));

        let mesh = build_chunk_mesh(&chunk);

        // Each voxel has 6 faces, but 1 face is hidden between them
        // So: 2 voxels * 6 faces - 2 hidden faces = 10 visible faces
        // 10 faces * 4 vertices = 40 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            40,
            "Two adjacent voxels should have 10 visible faces (40 vertices)"
        );

        // 10 faces * 6 indices = 60 indices
        let indices = mesh.indices().unwrap();
        assert_eq!(
            indices.len(),
            60,
            "Two adjacent voxels should have 60 indices"
        );
    }

    #[test]
    fn test_2x2x2_cube_face_culling() {
        let mut chunk = VoxelChunk::new();
        // Create a solid 2x2x2 cube
        for x in 8..10 {
            for y in 8..10 {
                for z in 8..10 {
                    chunk.set(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }

        let mesh = build_chunk_mesh(&chunk);

        // 2x2x2 cube has 6 sides, each side is 2x2 = 4 faces
        // Total visible faces = 6 * 4 = 24 faces
        // 24 faces * 4 vertices = 96 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            96,
            "2x2x2 cube should have 24 visible faces (96 vertices)"
        );

        // Without face culling: 8 voxels * 6 faces = 48 faces
        // With face culling: 24 faces (50% reduction)
    }

    #[test]
    fn test_3x3x3_cube_face_culling() {
        let mut chunk = VoxelChunk::new();
        // Create a solid 3x3x3 cube
        for x in 8..11 {
            for y in 8..11 {
                for z in 8..11 {
                    chunk.set(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }

        let mesh = build_chunk_mesh(&chunk);

        // 3x3x3 cube has 6 sides, each side is 3x3 = 9 faces
        // Total visible faces = 6 * 9 = 54 faces
        // 54 faces * 4 vertices = 216 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            216,
            "3x3x3 cube should have 54 visible faces (216 vertices)"
        );

        // Without face culling: 27 voxels * 6 faces = 162 faces
        // With face culling: 54 faces (66.7% reduction)
    }

    #[test]
    fn test_cross_shape_face_culling() {
        let mut chunk = VoxelChunk::new();
        // Create a plus/cross shape (5 voxels in a + pattern)
        // Center and 4 adjacent (no corner touching)
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0)); // center
        chunk.set(9, 8, 8, Voxel::solid(0, 255, 0)); // +X
        chunk.set(7, 8, 8, Voxel::solid(0, 0, 255)); // -X
        chunk.set(8, 9, 8, Voxel::solid(255, 255, 0)); // +Y
        chunk.set(8, 7, 8, Voxel::solid(0, 255, 255)); // -Y

        let mesh = build_chunk_mesh(&chunk);

        // Center voxel: 6 - 4 adjacent = 2 visible faces (+Z, -Z)
        // Each arm voxel: 6 - 1 adjacent = 5 visible faces
        // Total: 2 + (4 * 5) = 22 visible faces
        // 22 faces * 4 vertices = 88 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            88,
            "Cross shape should have 22 visible faces (88 vertices)"
        );
    }

    #[test]
    fn test_five_voxels_in_line_face_culling() {
        let mut chunk = VoxelChunk::new();
        // 5 voxels in a line along X axis
        for i in 0..5 {
            chunk.set(8 + i, 8, 8, Voxel::solid(255, 0, 0));
        }

        let mesh = build_chunk_mesh(&chunk);

        // Line of 5: end caps have 5 visible faces each, middle 3 have 4 visible faces each
        // Total: 2 * 5 + 3 * 4 = 10 + 12 = 22 visible faces
        // 22 faces * 4 vertices = 88 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            88,
            "Line of 5 voxels should have 22 visible faces (88 vertices)"
        );
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
    fn test_concave_corner_has_reduced_ao() {
        let mut chunk = VoxelChunk::new();
        // Create an L-shaped structure with a concave corner
        // The concave corner should have reduced AO
        //
        //   Y
        //   |  [B]
        //   | [A][C]
        //   +------ X
        //
        // Voxels A, B, C form an L-shape
        // The inner corner vertex (shared by A's +Y and +X faces, B's -Y face, C's -X face)
        // should have reduced AO
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0)); // A (base)
        chunk.set(8, 9, 8, Voxel::solid(255, 0, 0)); // B (above A)
        chunk.set(9, 8, 8, Voxel::solid(255, 0, 0)); // C (right of A)

        let mesh = build_chunk_mesh(&chunk);

        // The inner concave corner should have AO < 1.0
        // because the corner vertex is surrounded by solid neighbors
        if let Some(bevy::mesh::VertexAttributeValues::Float32(ao_values)) =
            mesh.attribute(ATTRIBUTE_VOXEL_AO)
        {
            let has_occluded = ao_values.iter().any(|ao| *ao < 0.99);
            assert!(
                has_occluded,
                "L-shaped structure should have reduced AO at concave corner"
            );
        } else {
            panic!("AO attribute not found or wrong type");
        }
    }

    // =========================================================================
    // GREEDY MESHING TESTS
    // =========================================================================

    #[test]
    fn test_greedy_single_voxel_6_quads() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0));

        let mesh = build_chunk_mesh_greedy(&chunk);

        // Single voxel: 6 quads, no merging possible
        // 6 quads * 4 vertices = 24 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            24,
            "Single voxel should produce 6 quads (24 vertices)"
        );
    }

    #[test]
    fn test_greedy_2x2x2_same_color_6_quads() {
        let mut chunk = VoxelChunk::new();
        // Create a solid 2x2x2 cube with same color
        for x in 8..10 {
            for y in 8..10 {
                for z in 8..10 {
                    chunk.set(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }

        let mesh = build_chunk_mesh_greedy(&chunk);

        // 2x2x2 same-color cube: all faces merge into 6 quads (one per side)
        // 6 quads * 4 vertices = 24 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            24,
            "2x2x2 same-color cube should produce 6 quads (24 vertices)"
        );
    }

    #[test]
    fn test_greedy_8x8x8_same_color_6_quads() {
        let mut chunk = VoxelChunk::new();
        // Create a solid 8x8x8 cube with same color
        for x in 8..16 {
            for y in 8..16 {
                for z in 8..16 {
                    chunk.set(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }

        let mesh = build_chunk_mesh_greedy(&chunk);

        // 8x8x8 same-color cube: all faces merge into 6 quads (one per side)
        // 6 quads * 4 vertices = 24 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            24,
            "8x8x8 same-color cube should produce 6 quads (24 vertices)"
        );
    }

    #[test]
    fn test_greedy_2x2x2_checkerboard_no_merge() {
        let mut chunk = VoxelChunk::new();
        // Create a 2x2x2 checkerboard - no faces should merge
        let colors = [
            Voxel::solid(255, 0, 0), // Red
            Voxel::solid(0, 255, 0), // Green
        ];
        for x in 8..10 {
            for y in 8..10 {
                for z in 8..10 {
                    let idx = (x + y + z) % 2;
                    chunk.set(x, y, z, colors[idx]);
                }
            }
        }

        let mesh = build_chunk_mesh_greedy(&chunk);

        // Checkerboard: no faces can merge (different colors)
        // Same as face-culling: 24 visible faces * 4 vertices = 96 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            96,
            "Checkerboard should not merge (96 vertices)"
        );
    }

    #[test]
    fn test_greedy_flat_layer_2_quads() {
        let mut chunk = VoxelChunk::new();
        // Create a 4x1x4 flat layer (16 voxels)
        for x in 8..12 {
            for z in 8..12 {
                chunk.set(x, 8, z, Voxel::solid(100, 200, 50));
            }
        }

        let mesh = build_chunk_mesh_greedy(&chunk);

        // Flat layer: top face merges into 1 quad, bottom face merges into 1 quad
        // Plus 4 edge faces (each is 4 voxels long, merges into 1 quad)
        // Total: 2 (top/bottom) + 4 (sides) = 6 quads
        // 6 quads * 4 vertices = 24 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            24,
            "4x1x4 flat layer should produce 6 quads (24 vertices)"
        );
    }

    #[test]
    fn test_greedy_two_colors_partial_merge() {
        let mut chunk = VoxelChunk::new();
        // Create a 2x2x1 layer with 2 colors (2x2 top, each color is a 2x1 strip)
        // Top row: red, red
        // Bottom row: blue, blue
        chunk.set(8, 8, 8, Voxel::solid(255, 0, 0)); // Red
        chunk.set(9, 8, 8, Voxel::solid(255, 0, 0)); // Red
        chunk.set(8, 8, 9, Voxel::solid(0, 0, 255)); // Blue
        chunk.set(9, 8, 9, Voxel::solid(0, 0, 255)); // Blue

        let mesh = build_chunk_mesh_greedy(&chunk);

        // Top face: 2 quads (red strip, blue strip)
        // Bottom face: 2 quads (red strip, blue strip)
        // Each side: varies based on color boundaries
        // Front (-Z): 1 quad (both red)
        // Back (+Z): 1 quad (both blue)
        // Left (-X): 2 quads (red, blue)
        // Right (+X): 2 quads (red, blue)
        // Total: 2 + 2 + 1 + 1 + 2 + 2 = 10 quads
        // 10 quads * 4 vertices = 40 vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(
            positions.len(),
            40,
            "2x1 dual-color strips should produce 10 quads (40 vertices)"
        );
    }

    #[test]
    fn test_greedy_vs_culling_improvement() {
        let mut chunk = VoxelChunk::new();
        // Create a solid 4x4x4 cube with same color
        for x in 8..12 {
            for y in 8..12 {
                for z in 8..12 {
                    chunk.set(x, y, z, Voxel::solid(128, 128, 128));
                }
            }
        }

        let culled_mesh = build_chunk_mesh(&chunk);
        let greedy_mesh = build_chunk_mesh_greedy(&chunk);

        let culled_verts = culled_mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .len();
        let greedy_verts = greedy_mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .len();

        // Face culling: 6 sides * 16 faces = 96 quads = 384 vertices
        assert_eq!(
            culled_verts, 384,
            "Face culling should produce 384 vertices"
        );

        // Greedy: 6 quads = 24 vertices
        assert_eq!(
            greedy_verts, 24,
            "Greedy meshing should produce 24 vertices"
        );

        // Greedy should be 16x better for uniform cubes
        assert!(
            greedy_verts < culled_verts / 10,
            "Greedy should be much better than culling"
        );
    }

    #[test]
    fn test_greedy_mesh_has_all_attributes() {
        let mut chunk = VoxelChunk::new();
        chunk.set(8, 8, 8, Voxel::new(255, 128, 64, 200));

        let mesh = build_chunk_mesh_greedy(&chunk);

        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(mesh.attribute(ATTRIBUTE_VOXEL_COLOR).is_some());
        assert!(mesh.attribute(ATTRIBUTE_VOXEL_EMISSION).is_some());
        assert!(mesh.attribute(ATTRIBUTE_VOXEL_AO).is_some());
    }

    // =========================================================================
    // CROSS-CHUNK FACE CULLING TESTS
    // =========================================================================

    #[test]
    fn test_cross_chunk_culling_two_adjacent_chunks_x() {
        let mut world = VoxelWorld::new();

        // Create two adjacent voxels across chunk boundary
        // World (31, 16, 16) in chunk (0,0,0), local (31, 16, 16)
        // World (32, 16, 16) in chunk (1,0,0), local (0, 16, 16)
        world.set_voxel(31, 16, 16, Voxel::solid(255, 0, 0));
        world.set_voxel(32, 16, 16, Voxel::solid(0, 255, 0));

        // Build meshes without cross-chunk culling
        let meshes_no_culling = build_world_meshes_with_options(&world, false);

        // Build meshes with cross-chunk culling
        let meshes_with_culling = build_world_meshes_cross_chunk_with_options(&world, false);

        // Count total vertices
        let verts_no_culling: usize = meshes_no_culling
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        let verts_with_culling: usize = meshes_with_culling
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        // Without cross-chunk culling: 2 voxels * 6 faces * 4 verts = 48 vertices
        // (boundary faces are NOT culled)
        assert_eq!(
            verts_no_culling, 48,
            "Without cross-chunk culling, should have 48 vertices"
        );

        // With cross-chunk culling: faces at x=31 (+X) and x=32 (-X) are culled
        // 2 voxels * 6 faces - 2 culled faces = 10 faces * 4 verts = 40 vertices
        assert_eq!(
            verts_with_culling, 40,
            "With cross-chunk culling, should have 40 vertices"
        );
    }

    #[test]
    fn test_cross_chunk_culling_two_adjacent_chunks_y() {
        let mut world = VoxelWorld::new();

        // Create two adjacent voxels across Y chunk boundary
        world.set_voxel(16, 31, 16, Voxel::solid(255, 0, 0)); // Top of chunk (0,0,0)
        world.set_voxel(16, 32, 16, Voxel::solid(0, 255, 0)); // Bottom of chunk (0,1,0)

        let meshes_no_culling = build_world_meshes_with_options(&world, false);
        let meshes_with_culling = build_world_meshes_cross_chunk_with_options(&world, false);

        let verts_no_culling: usize = meshes_no_culling
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        let verts_with_culling: usize = meshes_with_culling
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        assert_eq!(
            verts_no_culling, 48,
            "Without cross-chunk culling, should have 48 vertices"
        );
        assert_eq!(
            verts_with_culling, 40,
            "With cross-chunk culling, should have 40 vertices"
        );
    }

    #[test]
    fn test_cross_chunk_culling_two_adjacent_chunks_z() {
        let mut world = VoxelWorld::new();

        // Create two adjacent voxels across Z chunk boundary
        world.set_voxel(16, 16, 31, Voxel::solid(255, 0, 0)); // Front of chunk (0,0,0)
        world.set_voxel(16, 16, 32, Voxel::solid(0, 255, 0)); // Back of chunk (0,0,1)

        let meshes_no_culling = build_world_meshes_with_options(&world, false);
        let meshes_with_culling = build_world_meshes_cross_chunk_with_options(&world, false);

        let verts_no_culling: usize = meshes_no_culling
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        let verts_with_culling: usize = meshes_with_culling
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        assert_eq!(
            verts_no_culling, 48,
            "Without cross-chunk culling, should have 48 vertices"
        );
        assert_eq!(
            verts_with_culling, 40,
            "With cross-chunk culling, should have 40 vertices"
        );
    }

    #[test]
    fn test_cross_chunk_culling_greedy_same_color() {
        let mut world = VoxelWorld::new();

        // Create a 2x2x2 cube that spans two chunks
        // Voxels at x=30,31 in chunk 0 and x=32,33 in chunk 1
        for x in 30..34 {
            for y in 16..18 {
                for z in 16..18 {
                    world.set_voxel(x, y, z, Voxel::solid(128, 128, 128));
                }
            }
        }

        // With greedy meshing and cross-chunk culling
        let meshes = build_world_meshes_cross_chunk_with_options(&world, true);

        // The 4x2x2 cube should have the boundary faces culled
        // Each chunk should contribute to the overall shape
        let total_verts: usize = meshes
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        // 4x2x2 cube: 6 sides (some merged)
        // - Top/bottom: 4x2 faces each, merged
        // - Front/back: 4x2 faces each, merged
        // - Left/right: 2x2 faces each, merged
        // Total should be 6 merged quads (if same color) = 24 vertices
        // But split across 2 chunks, so slightly more due to chunk boundaries
        // The internal X faces between chunks ARE culled though

        // This is harder to predict exactly, but it should be less than
        // what we'd get without cross-chunk culling
        let meshes_no_cross = build_world_meshes_with_options(&world, true);
        let total_verts_no_cross: usize = meshes_no_cross
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        // Cross-chunk culling should reduce vertices
        assert!(
            total_verts < total_verts_no_cross,
            "Cross-chunk greedy should have fewer vertices ({}) than without ({})",
            total_verts,
            total_verts_no_cross
        );
    }

    #[test]
    fn test_cross_chunk_culling_solid_wall() {
        let mut world = VoxelWorld::new();

        // Create a solid 4x4 wall at the X boundary between chunks
        // 2 voxels thick (1 in each chunk)
        for y in 14..18 {
            for z in 14..18 {
                world.set_voxel(31, y, z, Voxel::solid(200, 100, 50));
                world.set_voxel(32, y, z, Voxel::solid(200, 100, 50));
            }
        }

        let meshes_no_culling = build_world_meshes_with_options(&world, false);
        let meshes_with_culling = build_world_meshes_cross_chunk_with_options(&world, false);

        let verts_no_culling: usize = meshes_no_culling
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        let verts_with_culling: usize = meshes_with_culling
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        // Without cross-chunk culling: each 4x4 layer has internal culling but
        // the boundary faces between chunks are NOT culled
        // With cross-chunk culling: the 16 faces at x=31 (+X) and 16 at x=32 (-X) are culled
        // That's 32 * 4 = 128 fewer vertices

        assert!(
            verts_with_culling < verts_no_culling,
            "Cross-chunk culling should reduce vertices: {} < {}",
            verts_with_culling,
            verts_no_culling
        );

        // Specifically: 32 faces culled * 4 verts = 128 difference
        assert_eq!(
            verts_no_culling - verts_with_culling,
            128,
            "Should cull exactly 32 faces (128 vertices)"
        );
    }

    #[test]
    fn test_cross_chunk_culling_isolated_chunks() {
        let mut world = VoxelWorld::new();

        // Create voxels in two non-adjacent chunks - no cross-chunk culling should apply
        world.set_voxel(16, 16, 16, Voxel::solid(255, 0, 0)); // Chunk (0,0,0)
        world.set_voxel(80, 16, 16, Voxel::solid(0, 255, 0)); // Chunk (2,0,0) - not adjacent!

        let meshes_no_culling = build_world_meshes_with_options(&world, false);
        let meshes_with_culling = build_world_meshes_cross_chunk_with_options(&world, false);

        let verts_no_culling: usize = meshes_no_culling
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        let verts_with_culling: usize = meshes_with_culling
            .iter()
            .map(|m| m.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len())
            .sum();

        // Both should be 48 (2 isolated voxels, no culling possible)
        assert_eq!(verts_no_culling, 48);
        assert_eq!(verts_with_culling, 48);
    }

    #[test]
    fn test_cross_chunk_mesh_has_all_attributes() {
        let mut world = VoxelWorld::new();
        world.set_voxel(31, 16, 16, Voxel::new(255, 128, 64, 200));
        world.set_voxel(32, 16, 16, Voxel::new(64, 128, 255, 150));

        let meshes = build_world_meshes_cross_chunk(&world);

        for chunk_mesh in meshes {
            assert!(chunk_mesh
                .mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .is_some());
            assert!(chunk_mesh.mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
            assert!(chunk_mesh.mesh.attribute(ATTRIBUTE_VOXEL_COLOR).is_some());
            assert!(chunk_mesh
                .mesh
                .attribute(ATTRIBUTE_VOXEL_EMISSION)
                .is_some());
            assert!(chunk_mesh.mesh.attribute(ATTRIBUTE_VOXEL_AO).is_some());
        }
    }
}
