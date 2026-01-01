//! Voxel physics utilities for generating colliders from voxel data.
//!
//! This module provides functions to generate physics colliders from VoxelWorld data,
//! using the same greedy meshing output as the rendering system to ensure consistency
//! between visual and physics geometry.
//!
//! ## Trimesh Colliders
//!
//! We use Trimesh colliders from bevy_rapier3d, generated from our existing greedy mesh.
//! This approach:
//! - Reuses the optimized mesh from greedy meshing (same vertex count for physics and rendering)
//! - Ensures physics and visual geometry are perfectly aligned
//! - Works well for both static terrain and dynamic fragments
//!
//! ## Usage
//!
//! ```ignore
//! use studio_core::voxel_physics::generate_trimesh_collider;
//! use studio_core::voxel::VoxelWorld;
//!
//! let mut world = VoxelWorld::new();
//! // ... populate world ...
//!
//! if let Some(collider) = generate_trimesh_collider(&world) {
//!     commands.spawn((
//!         RigidBody::Fixed,
//!         collider,
//!         Transform::default(),
//!     ));
//! }
//! ```

use bevy::prelude::*;
use bevy::mesh::Indices;
use bevy_rapier3d::prelude::*;

use crate::voxel::VoxelWorld;
use crate::voxel_mesh::build_world_meshes_cross_chunk;

/// Generate a single merged Trimesh collider from a VoxelWorld.
///
/// This combines all chunk meshes into one collider for physics.
/// The collider geometry matches the visual mesh exactly since both
/// use the same greedy meshing algorithm.
///
/// For small fragments (< ~1000 voxels), this is efficient.
/// For large worlds, consider using `generate_chunk_colliders` instead
/// to get per-chunk colliders for better performance.
///
/// # Arguments
/// * `world` - The VoxelWorld to generate a collider for
///
/// # Returns
/// * `Some(Collider)` - A trimesh collider if the world has any voxels
/// * `None` - If the world is empty
///
/// # Example
/// ```ignore
/// let collider = generate_trimesh_collider(&world)?;
/// commands.spawn((RigidBody::Dynamic, collider, Transform::default()));
/// ```
pub fn generate_trimesh_collider(world: &VoxelWorld) -> Option<Collider> {
    let chunk_meshes = build_world_meshes_cross_chunk(world);
    if chunk_meshes.is_empty() {
        return None;
    }

    let mut all_vertices: Vec<Vec3> = Vec::new();
    let mut all_indices: Vec<[u32; 3]> = Vec::new();

    for chunk_mesh in chunk_meshes {
        let base_idx = all_vertices.len() as u32;
        let offset = Vec3::from_array(chunk_mesh.world_offset);

        // Extract positions from mesh
        if let Some(positions) = chunk_mesh.mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            if let bevy::mesh::VertexAttributeValues::Float32x3(verts) = positions {
                for v in verts {
                    all_vertices.push(Vec3::from_array(*v) + offset);
                }
            }
        }

        // Extract indices
        if let Some(Indices::U32(indices)) = chunk_mesh.mesh.indices() {
            for chunk in indices.chunks(3) {
                if chunk.len() == 3 {
                    all_indices.push([
                        chunk[0] + base_idx,
                        chunk[1] + base_idx,
                        chunk[2] + base_idx,
                    ]);
                }
            }
        }
    }

    if all_vertices.is_empty() || all_indices.is_empty() {
        return None;
    }

    Collider::trimesh(all_vertices, all_indices).ok()
}

/// Generate per-chunk colliders for a VoxelWorld.
///
/// This is more efficient for large worlds as it allows:
/// - Spatial partitioning in the physics engine
/// - Selective updates when chunks change
/// - Better collision query performance
///
/// # Arguments
/// * `world` - The VoxelWorld to generate colliders for
///
/// # Returns
/// A Vec of (collider, world_position) tuples for each non-empty chunk.
pub fn generate_chunk_colliders(world: &VoxelWorld) -> Vec<(Collider, Vec3)> {
    let chunk_meshes = build_world_meshes_cross_chunk(world);
    
    chunk_meshes
        .into_iter()
        .filter_map(|chunk_mesh| {
            let offset = Vec3::from_array(chunk_mesh.world_offset);
            
            let mut vertices: Vec<Vec3> = Vec::new();
            let mut indices: Vec<[u32; 3]> = Vec::new();
            
            // Extract positions (local to chunk, no offset needed since we return position separately)
            if let Some(positions) = chunk_mesh.mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                if let bevy::mesh::VertexAttributeValues::Float32x3(verts) = positions {
                    for v in verts {
                        vertices.push(Vec3::from_array(*v));
                    }
                }
            }
            
            // Extract indices
            if let Some(Indices::U32(mesh_indices)) = chunk_mesh.mesh.indices() {
                for chunk in mesh_indices.chunks(3) {
                    if chunk.len() == 3 {
                        indices.push([chunk[0], chunk[1], chunk[2]]);
                    }
                }
            }
            
            if vertices.is_empty() || indices.is_empty() {
                return None;
            }
            
            Collider::trimesh(vertices, indices)
                .ok()
                .map(|collider| (collider, offset))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::Voxel;

    #[test]
    fn test_generate_trimesh_empty_world() {
        let world = VoxelWorld::new();
        let collider = generate_trimesh_collider(&world);
        assert!(collider.is_none());
    }

    #[test]
    fn test_generate_trimesh_single_voxel() {
        let mut world = VoxelWorld::new();
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));

        let collider = generate_trimesh_collider(&world);
        assert!(collider.is_some());
        
        // A single voxel cube has 6 faces = 12 triangles
        // We can verify the collider exists and has the right shape type
        let collider = collider.unwrap();
        assert!(collider.as_trimesh().is_some(), "Collider should be a trimesh");
    }

    #[test]
    fn test_generate_trimesh_greedy_merged() {
        let mut world = VoxelWorld::new();
        // 4x4x4 same-color cube should greedy merge to 6 quads = 12 triangles
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    world.set_voxel(x, y, z, Voxel::solid(128, 128, 128));
                }
            }
        }

        let collider = generate_trimesh_collider(&world);
        assert!(collider.is_some());
        
        let collider = collider.unwrap();
        let trimesh = collider.as_trimesh().expect("Should be trimesh");
        
        // Greedy meshing should produce 6 quads (one per face of the cube)
        // Each quad = 2 triangles, so 12 triangles total
        assert_eq!(trimesh.indices().len(), 12, "Expected 12 triangles (6 faces * 2 triangles each)");
    }

    #[test]
    fn test_generate_chunk_colliders_empty() {
        let world = VoxelWorld::new();
        let colliders = generate_chunk_colliders(&world);
        assert!(colliders.is_empty());
    }

    #[test]
    fn test_generate_chunk_colliders_single_chunk() {
        let mut world = VoxelWorld::new();
        world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
        
        let colliders = generate_chunk_colliders(&world);
        assert_eq!(colliders.len(), 1);
        
        let (collider, _pos) = &colliders[0];
        assert!(collider.as_trimesh().is_some());
    }

    #[test]
    fn test_generate_chunk_colliders_multiple_chunks() {
        let mut world = VoxelWorld::new();
        // Voxels in two different chunks
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));   // Chunk (0,0,0)
        world.set_voxel(32, 0, 0, Voxel::solid(0, 255, 0)); // Chunk (1,0,0)
        
        let colliders = generate_chunk_colliders(&world);
        assert_eq!(colliders.len(), 2);
    }
}
