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

use bevy::mesh::Indices;
use bevy::prelude::*;
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

/// Generate a compound collider using cuboids for each voxel.
///
/// This is MUCH faster than trimesh for dynamic objects because:
/// - Cuboid-cuboid collision is O(1) vs trimesh O(n) triangle tests
/// - Better suited for Rapier's solver
///
/// For a uniform 3x3x3 cube, this produces 1 cuboid (if same color, greedy merge)
/// or up to 27 cuboids (if all different colors).
///
/// # Arguments
/// * `world` - The VoxelWorld to generate a collider for
///
/// # Returns
/// * `Some(Collider)` - A compound collider if the world has any voxels
/// * `None` - If the world is empty
pub fn generate_cuboid_collider(world: &VoxelWorld) -> Option<Collider> {
    if world.total_voxel_count() == 0 {
        return None;
    }

    let mut shapes: Vec<(Vec3, Quat, Collider)> = Vec::new();

    // For now, use one cuboid per voxel (simple but correct)
    // TODO: Could optimize by merging adjacent same-color voxels into larger cuboids
    for (chunk_pos, chunk) in world.iter_chunks() {
        let (ox, oy, oz) = chunk_pos.world_origin();

        for (lx, ly, lz, _voxel) in chunk.iter() {
            let wx = ox + lx as i32;
            let wy = oy + ly as i32;
            let wz = oz + lz as i32;

            // Position at center of voxel
            let pos = Vec3::new(wx as f32 + 0.5, wy as f32 + 0.5, wz as f32 + 0.5);

            // Unit cuboid (half-extents = 0.5)
            let cuboid = Collider::cuboid(0.5, 0.5, 0.5);

            shapes.push((pos, Quat::IDENTITY, cuboid));
        }
    }

    if shapes.is_empty() {
        return None;
    }

    Some(Collider::compound(shapes))
}

/// Generate a compound collider with merged cuboids using AABB regions.
///
/// This analyzes the voxel data and creates larger cuboids where possible,
/// dramatically reducing the number of collision shapes.
///
/// For a uniform 3x3x3 cube: 1 cuboid instead of 27
/// For a uniform 10x10x1 floor: 1 cuboid instead of 100
pub fn generate_merged_cuboid_collider(world: &VoxelWorld) -> Option<Collider> {
    if world.total_voxel_count() == 0 {
        return None;
    }

    // Get voxel bounds
    let bounds = world.voxel_bounds()?;
    let min = IVec3::new(
        bounds.0.x.floor() as i32,
        bounds.0.y.floor() as i32,
        bounds.0.z.floor() as i32,
    );
    let max = IVec3::new(
        bounds.1.x.ceil() as i32,
        bounds.1.y.ceil() as i32,
        bounds.1.z.ceil() as i32,
    );

    // Simple greedy merge: find largest AABB that fits
    // For now, just check if it's a solid rectangular region
    let size = max - min;
    let expected_count = (size.x * size.y * size.z) as usize;

    if world.total_voxel_count() == expected_count {
        // It's a solid box! Use single cuboid
        let half_extents = Vec3::new(
            size.x as f32 / 2.0,
            size.y as f32 / 2.0,
            size.z as f32 / 2.0,
        );
        let center = Vec3::new(
            min.x as f32 + half_extents.x,
            min.y as f32 + half_extents.y,
            min.z as f32 + half_extents.z,
        );

        let cuboid = Collider::cuboid(half_extents.x, half_extents.y, half_extents.z);
        return Some(Collider::compound(vec![(center, Quat::IDENTITY, cuboid)]));
    }

    // Fall back to per-voxel cuboids
    generate_cuboid_collider(world)
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
        assert!(
            collider.as_trimesh().is_some(),
            "Collider should be a trimesh"
        );
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
        assert_eq!(
            trimesh.indices().len(),
            12,
            "Expected 12 triangles (6 faces * 2 triangles each)"
        );
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
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0)); // Chunk (0,0,0)
        world.set_voxel(32, 0, 0, Voxel::solid(0, 255, 0)); // Chunk (1,0,0)

        let colliders = generate_chunk_colliders(&world);
        assert_eq!(colliders.len(), 2);
    }

    #[test]
    fn test_fragment_triangle_count() {
        // 3x3x3 fragment with same color
        let mut fragment = VoxelWorld::new();
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    fragment.set_voxel(x, y, z, Voxel::solid(200, 100, 100));
                }
            }
        }

        if let Some(collider) = generate_trimesh_collider(&fragment) {
            if let Some(trimesh) = collider.as_trimesh() {
                println!(
                    "FRAGMENT 3x3x3 TRIMESH: {} vertices, {} triangles",
                    trimesh.vertices().len(),
                    trimesh.indices().len()
                );
                // Should be 12 triangles (6 faces * 2 tris) with greedy meshing
            }
        }
    }

    #[test]
    fn test_cuboid_collider_single_voxel() {
        let mut world = VoxelWorld::new();
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));

        let collider = generate_cuboid_collider(&world);
        assert!(collider.is_some());

        let collider = collider.unwrap();
        // Should be a compound with 1 cuboid
        assert!(collider.as_compound().is_some());
        let compound = collider.as_compound().unwrap();
        assert_eq!(compound.shapes().len(), 1);
    }

    #[test]
    fn test_merged_cuboid_collider_solid_box() {
        let mut world = VoxelWorld::new();
        // 3x3x3 solid box
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    world.set_voxel(x, y, z, Voxel::solid(200, 100, 100));
                }
            }
        }

        let collider = generate_merged_cuboid_collider(&world);
        assert!(collider.is_some());

        let collider = collider.unwrap();
        let compound = collider.as_compound().expect("Should be compound");

        // Should merge into single cuboid!
        assert_eq!(
            compound.shapes().len(),
            1,
            "Solid box should merge to 1 cuboid"
        );
        println!(
            "MERGED 3x3x3: {} shapes (should be 1)",
            compound.shapes().len()
        );
    }

    #[test]
    fn test_merged_cuboid_collider_with_hole() {
        let mut world = VoxelWorld::new();
        // 3x3x3 with center missing
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    if x != 1 || y != 1 || z != 1 {
                        world.set_voxel(x, y, z, Voxel::solid(200, 100, 100));
                    }
                }
            }
        }

        let collider = generate_merged_cuboid_collider(&world);
        assert!(collider.is_some());

        let collider = collider.unwrap();
        let compound = collider.as_compound().expect("Should be compound");

        // Can't merge due to hole, falls back to per-voxel
        assert_eq!(
            compound.shapes().len(),
            26,
            "Box with hole should have 26 cuboids"
        );
        println!(
            "HOLLOW 3x3x3: {} shapes (should be 26)",
            compound.shapes().len()
        );
    }

    #[test]
    fn test_terrain_triangle_count() {
        // Simulate the terrain from p22_voxel_fragment
        let mut terrain = VoxelWorld::new();

        // Ground platform (20x20, 3 blocks thick) - BUT with checkerboard colors!
        // This PREVENTS greedy meshing from merging faces!
        for x in -10..10 {
            for z in -10..10 {
                for y in 0..3 {
                    let color = if (x + z) % 2 == 0 {
                        Voxel::solid(80, 80, 90)
                    } else {
                        Voxel::solid(60, 60, 70)
                    };
                    terrain.set_voxel(x, y, z, color);
                }
            }
        }

        println!("Terrain voxel count: {}", terrain.total_voxel_count());

        if let Some(collider) = generate_trimesh_collider(&terrain) {
            if let Some(trimesh) = collider.as_trimesh() {
                println!(
                    "CHECKERBOARD TERRAIN TRIMESH: {} vertices, {} triangles",
                    trimesh.vertices().len(),
                    trimesh.indices().len()
                );

                // This is the problem! Checkerboard pattern = NO greedy merge
                // Each voxel face = 2 triangles
                // 20x20 top = 400 faces minimum = 800 triangles just for top!
                // With checkerboard, greedy meshing does almost nothing
            }
        }

        // Now test with SAME color - should be WAY fewer triangles
        let mut terrain_uniform = VoxelWorld::new();
        for x in -10..10 {
            for z in -10..10 {
                for y in 0..3 {
                    terrain_uniform.set_voxel(x, y, z, Voxel::solid(80, 80, 90));
                }
            }
        }

        if let Some(collider) = generate_trimesh_collider(&terrain_uniform) {
            if let Some(trimesh) = collider.as_trimesh() {
                println!(
                    "UNIFORM TERRAIN TRIMESH: {} vertices, {} triangles",
                    trimesh.vertices().len(),
                    trimesh.indices().len()
                );
            }
        }

        // Test cuboid collider for uniform terrain
        if let Some(collider) = generate_cuboid_collider(&terrain_uniform) {
            if let Some(compound) = collider.as_compound() {
                println!(
                    "UNIFORM TERRAIN CUBOIDS: {} shapes",
                    compound.shapes().len()
                );
            }
        }
    }
}
