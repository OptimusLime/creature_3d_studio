//! Fragment and AABB extraction for GPU collision.
//!
//! This module extracts VoxelFragment and GpuCollisionAABB data from the main world
//! to the render world for GPU collision detection.

use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::Extract;

use crate::voxel_fragment::VoxelFragment;
use crate::voxel_collision::{ChunkOccupancy, GpuCollisionAABB};

/// Extracted fragment data for GPU collision.
#[derive(Clone)]
pub struct ExtractedFragment {
    /// Entity ID (for mapping contacts back to fragments)
    pub entity: Entity,
    /// World position of fragment center
    pub position: Vec3,
    /// Rotation quaternion
    pub rotation: Quat,
    /// Size in voxels (for AABB: ceil of half_extents * 2)
    pub size: UVec3,
    /// Bit-packed occupancy data (copied from FragmentOccupancy).
    /// Empty for AABB entities - shader treats empty occupancy as fully solid.
    pub occupancy_data: Vec<u32>,
    /// Whether this is an AABB (true) or voxel fragment (false).
    /// AABB entities have no rotation applied and use half_extents directly.
    pub is_aabb: bool,
}

/// Resource containing all extracted fragments for the current frame.
#[derive(Resource, Default, Clone)]
pub struct ExtractedFragments {
    /// All fragments to check for collision this frame
    pub fragments: Vec<ExtractedFragment>,
}

// Manual ExtractResource implementation since we need custom extraction logic
impl ExtractResource for ExtractedFragments {
    type Source = ExtractedFragments;
    
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Extracted terrain occupancy data.
/// 
/// Contains the chunk occupancy data from the main world's TerrainOccupancy resource.
#[derive(Resource, Default, Clone)]
pub struct ExtractedTerrainChunks {
    /// Chunk coordinates and their occupancy data
    pub chunks: Vec<(IVec3, ChunkOccupancy)>,
    /// Whether terrain has changed since last frame (triggers re-upload)
    pub dirty: bool,
}

impl ExtractResource for ExtractedTerrainChunks {
    type Source = ExtractedTerrainChunks;
    
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// System to extract fragment and AABB data from main world to render world.
///
/// Runs in ExtractSchedule to copy fragment transforms and occupancy data
/// to the render world for GPU collision processing.
///
/// Extracts both:
/// - `VoxelFragment` entities (dynamic voxel fragments with occupancy)
/// - `GpuCollisionAABB` entities (kinematic characters with AABB collision)
pub fn extract_fragments_system(
    mut extracted: ResMut<ExtractedFragments>,
    fragments: Extract<Query<(Entity, &VoxelFragment, &Transform)>>,
    aabbs: Extract<Query<(Entity, &GpuCollisionAABB, &Transform), Without<VoxelFragment>>>,
) {
    extracted.fragments.clear();
    
    // Extract VoxelFragment entities
    for (entity, fragment, transform) in fragments.iter() {
        extracted.fragments.push(ExtractedFragment {
            entity,
            position: transform.translation,
            rotation: transform.rotation,
            size: fragment.occupancy.size,
            occupancy_data: fragment.occupancy.as_u32_slice().to_vec(),
            is_aabb: false,
        });
    }
    
    // Extract GpuCollisionAABB entities
    for (entity, aabb, transform) in aabbs.iter() {
        // Convert half_extents to voxel size.
        // Add +1 to each dimension to account for voxel boundary straddling.
        // Without this, an AABB at position y=3.9 with half_y=0.9 (bottom at y=3.0)
        // would only check voxels y=3,4 and miss terrain at y=2.
        let size = UVec3::new(
            (aabb.half_extents.x * 2.0).ceil() as u32 + 1,
            (aabb.half_extents.y * 2.0).ceil() as u32 + 1,
            (aabb.half_extents.z * 2.0).ceil() as u32 + 1,
        );
        
        extracted.fragments.push(ExtractedFragment {
            entity,
            position: transform.translation,
            rotation: Quat::IDENTITY, // AABBs don't rotate
            size,
            occupancy_data: Vec::new(), // Empty = fully solid in shader
            is_aabb: true,
        });
    }
    
    if !extracted.fragments.is_empty() {
        trace!(
            "Extracted {} entities for GPU collision ({} fragments, {} AABBs)",
            extracted.fragments.len(),
            fragments.iter().count(),
            aabbs.iter().count()
        );
    }
}

/// System to extract terrain occupancy from main world.
///
/// Only extracts when terrain changes to avoid unnecessary GPU uploads.
pub fn extract_terrain_occupancy_system(
    mut extracted: ResMut<ExtractedTerrainChunks>,
    terrain: Extract<Option<Res<crate::voxel_fragment::TerrainOccupancy>>>,
    // Track if terrain changed - could use a Changed<> query or version counter
) {
    // For now, we only extract once (terrain is static in p22)
    // A proper implementation would track changes
    if extracted.chunks.is_empty() {
        if let Some(terrain) = terrain.as_ref() {
            extracted.chunks = terrain.occupancy
                .iter_chunks()
                .map(|(coord, chunk)| (coord, chunk.clone()))
                .collect();
            extracted.dirty = true;
            
            if !extracted.chunks.is_empty() {
                info!("Extracted {} terrain chunks for GPU collision", extracted.chunks.len());
            }
        }
    } else {
        extracted.dirty = false;
    }
}

// Add iter_chunks method to WorldOccupancy if it doesn't exist
// We'll add this to voxel_collision.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extracted_fragment_default() {
        let extracted = ExtractedFragments::default();
        assert!(extracted.fragments.is_empty());
    }
    
    #[test]
    fn test_extracted_terrain_default() {
        let extracted = ExtractedTerrainChunks::default();
        assert!(extracted.chunks.is_empty());
        assert!(!extracted.dirty);
    }
    
    #[test]
    fn test_aabb_size_calculation() {
        // Verify that AABB half_extents are correctly converted to voxel size
        // Player has half_extents (0.4, 0.9, 0.4)
        // Size should be ceil(half * 2) = ceil(0.8, 1.8, 0.8) = (1, 2, 1)
        
        let half_extents = Vec3::new(0.4, 0.9, 0.4);
        let size = UVec3::new(
            (half_extents.x * 2.0).ceil() as u32,
            (half_extents.y * 2.0).ceil() as u32,
            (half_extents.z * 2.0).ceil() as u32,
        );
        
        assert_eq!(size.x, 1, "X size should be 1 (ceil of 0.8)");
        assert_eq!(size.y, 2, "Y size should be 2 (ceil of 1.8)");
        assert_eq!(size.z, 1, "Z size should be 1 (ceil of 0.8)");
    }
    
    #[test]
    fn test_aabb_extraction_produces_empty_occupancy() {
        // For AABB entities, occupancy_data should be empty
        // This signals the shader to treat it as a solid box
        
        let extracted = ExtractedFragment {
            entity: Entity::PLACEHOLDER,
            position: Vec3::new(0.0, 5.0, 0.0),
            rotation: Quat::IDENTITY,
            size: UVec3::new(1, 2, 1),
            occupancy_data: Vec::new(), // Key: empty = solid
            is_aabb: true,
        };
        
        assert!(extracted.occupancy_data.is_empty(), "AABB should have empty occupancy data");
        assert!(extracted.is_aabb, "Should be marked as AABB");
        assert_eq!(extracted.rotation, Quat::IDENTITY, "AABB should have identity rotation");
    }
    
    #[test]
    fn test_aabb_voxel_positions_for_collision() {
        // For an AABB at position (0, 5, 0) with size (1, 2, 1),
        // the shader checks voxels at local positions (0,0,0) and (0,1,0)
        // 
        // World position calculation in shader:
        // half_size = size * 0.5 = (0.5, 1.0, 0.5)
        // local_float = local_pos + 0.5 (e.g., (0.5, 0.5, 0.5) for local (0,0,0))
        // centered = local_float - half_size (e.g., (0.0, -0.5, 0.0))
        // world_pos = position + rotated(centered) = (0, 5, 0) + (0, -0.5, 0) = (0, 4.5, 0)
        //
        // For local (0,1,0):
        // local_float = (0.5, 1.5, 0.5)
        // centered = (0.0, 0.5, 0.0)
        // world_pos = (0, 5, 0) + (0, 0.5, 0) = (0, 5.5, 0)
        //
        // So voxels checked are at world Y = 4.5 and 5.5
        // floor(4.5) = 4, floor(5.5) = 5 → checks voxel grid positions y=4 and y=5
        
        let position = Vec3::new(0.0, 5.0, 0.0);
        let size = UVec3::new(1, 2, 1);
        let half_size = Vec3::new(size.x as f32, size.y as f32, size.z as f32) * 0.5;
        
        // Local position (0, 0, 0)
        let local_float_0 = Vec3::new(0.5, 0.5, 0.5);
        let centered_0 = local_float_0 - half_size;
        let world_pos_0 = position + centered_0;
        assert!((world_pos_0.y - 4.5).abs() < 0.001, 
            "Bottom voxel center should be at y=4.5, got {}", world_pos_0.y);
        
        // Local position (0, 1, 0)
        let local_float_1 = Vec3::new(0.5, 1.5, 0.5);
        let centered_1 = local_float_1 - half_size;
        let world_pos_1 = position + centered_1;
        assert!((world_pos_1.y - 5.5).abs() < 0.001, 
            "Top voxel center should be at y=5.5, got {}", world_pos_1.y);
        
        // Voxel grid positions (what the shader checks for terrain collision)
        let voxel_y_0 = world_pos_0.y.floor() as i32;
        let voxel_y_1 = world_pos_1.y.floor() as i32;
        assert_eq!(voxel_y_0, 4, "Bottom checks voxel y=4");
        assert_eq!(voxel_y_1, 5, "Top checks voxel y=5");
    }
    
    #[test]
    fn test_aabb_collision_with_floor_at_y3() {
        // Terrain floor is at y=0,1,2 (top surface at y=3)
        // Player AABB at position y=4 with half_extents (0.4, 0.9, 0.4)
        // 
        // The player's AABB bottom is at y = 4 - 0.9 = 3.1
        // But the shader checks discrete voxel positions, not AABB bounds!
        //
        // Size = (1, 2, 1), so voxel checks are at:
        // - world y=3.5 → floor(3.5)=3 → CHECK VOXEL AT y=3
        // - world y=4.5 → floor(4.5)=4 → CHECK VOXEL AT y=4
        //
        // Terrain has voxels at y=0,1,2 (NOT y=3)
        // So neither voxel check would find terrain!
        //
        // This reveals the bug: the AABB size (1,2,1) is too small to detect
        // collision with a floor at y=3 when the AABB center is at y=4.
        
        let position = Vec3::new(0.0, 4.0, 0.0);
        let half_extents = Vec3::new(0.4, 0.9, 0.4);
        let size = UVec3::new(
            (half_extents.x * 2.0).ceil() as u32,
            (half_extents.y * 2.0).ceil() as u32,
            (half_extents.z * 2.0).ceil() as u32,
        );
        // size = (1, 2, 1)
        
        let half_size = Vec3::new(size.x as f32, size.y as f32, size.z as f32) * 0.5;
        // half_size = (0.5, 1.0, 0.5)
        
        // Check what voxel positions the shader would check
        let mut voxel_y_positions = Vec::new();
        for local_y in 0..size.y {
            let local_float_y = local_y as f32 + 0.5;
            let centered_y = local_float_y - half_size.y;
            let world_y = position.y + centered_y;
            let voxel_y = world_y.floor() as i32;
            voxel_y_positions.push(voxel_y);
        }
        
        // With position y=4, size y=2:
        // local_y=0: world_y = 4 + (0.5 - 1.0) = 3.5 → voxel 3
        // local_y=1: world_y = 4 + (1.5 - 1.0) = 4.5 → voxel 4
        assert_eq!(voxel_y_positions, vec![3, 4], 
            "Should check voxels at y=3 and y=4");
        
        // Terrain floor (y=0,1,2) means voxel y=3 is EMPTY
        // So no collision would be detected at y=4!
        // This is expected - at y=4 the AABB bottom is at 3.1, above floor top at 3.0
    }
    
    #[test]
    fn test_aabb_collision_when_landing_at_correct_height() {
        // When player lands correctly at y=3.9 (bottom at y=3.0, exactly at floor top):
        // The shader should detect collision.
        //
        // Position y=3.9, size=(1,2,1), half_size=(0.5, 1.0, 0.5)
        // Voxel checks:
        // - local_y=0: world_y = 3.9 + (0.5 - 1.0) = 3.4 → voxel 3 (EMPTY)
        // - local_y=1: world_y = 3.9 + (1.5 - 1.0) = 4.4 → voxel 4 (EMPTY)
        //
        // Still no collision detected! The problem is the size (1,2,1) doesn't
        // extend low enough to check voxel y=2 (the actual terrain).
        
        let position = Vec3::new(0.0, 3.9, 0.0);
        let size = UVec3::new(1, 2, 1);
        let half_size = Vec3::new(0.5, 1.0, 0.5);
        
        let mut voxel_y_positions = Vec::new();
        for local_y in 0..size.y {
            let local_float_y = local_y as f32 + 0.5;
            let centered_y = local_float_y - half_size.y;
            let world_y = position.y + centered_y;
            let voxel_y = world_y.floor() as i32;
            voxel_y_positions.push(voxel_y);
        }
        
        assert_eq!(voxel_y_positions, vec![3, 4], 
            "At y=3.9, still checks voxels 3 and 4 (both empty)");
        
        // BUG: The GPU shader never checks voxel y=2 where terrain actually is!
        // 
        // The issue is that the AABB is discretized to a small voxel grid (1x2x1)
        // centered on the position, but the actual AABB extends lower than this grid.
        //
        // AABB actual bounds: y = [3.9 - 0.9, 3.9 + 0.9] = [3.0, 4.8]
        // Floor at y=2 has top at y=3
        // AABB bottom at y=3.0 touches floor top at y=3.0 → SHOULD collide
        //
        // But shader grid: checks y=3 and y=4 (both empty)
        //
        // FIX NEEDED: The size calculation should account for actual half_extents,
        // not just ceil(half_extents * 2). The shader needs to check ALL voxels
        // that the actual AABB overlaps, not just a small centered grid.
    }
    
    #[test]
    fn test_correct_aabb_size_for_collision() {
        // To correctly detect collision, the shader needs to check all voxels
        // that the AABB could overlap. For half_extents (0.4, 0.9, 0.4):
        //
        // The AABB spans 2*half = (0.8, 1.8, 0.8) units.
        // But it could be positioned anywhere within a voxel.
        // In the worst case, the AABB straddles voxel boundaries.
        //
        // For Y: 1.8 units could span up to 3 voxels (e.g., y=2.1 to y=3.9
        // spans voxels 2, 3, and partially 4).
        //
        // Correct size should be: ceil(half_extents * 2) + 1 in each dimension
        // to ensure we always check enough voxels.
        
        let half_extents = Vec3::new(0.4, 0.9, 0.4);
        
        // Current (buggy) calculation:
        let buggy_size = UVec3::new(
            (half_extents.x * 2.0).ceil() as u32,
            (half_extents.y * 2.0).ceil() as u32,
            (half_extents.z * 2.0).ceil() as u32,
        );
        assert_eq!(buggy_size, UVec3::new(1, 2, 1), "Current buggy size");
        
        // Correct calculation: add 1 to account for boundary straddling
        let correct_size = UVec3::new(
            (half_extents.x * 2.0).ceil() as u32 + 1,
            (half_extents.y * 2.0).ceil() as u32 + 1,
            (half_extents.z * 2.0).ceil() as u32 + 1,
        );
        assert_eq!(correct_size, UVec3::new(2, 3, 2), "Correct size with boundary padding");
        
        // With size (2, 3, 2), the shader would check more voxels:
        // For Y at position 3.9: half_size.y = 1.5
        // local_y=0: world_y = 3.9 + (0.5 - 1.5) = 2.9 → voxel 2 (TERRAIN!)
        // local_y=1: world_y = 3.9 + (1.5 - 1.5) = 3.9 → voxel 3
        // local_y=2: world_y = 3.9 + (2.5 - 1.5) = 4.9 → voxel 4
        
        let position_y = 3.9f32;
        let correct_half_size_y = correct_size.y as f32 * 0.5; // 1.5
        
        let mut voxel_y_positions = Vec::new();
        for local_y in 0..correct_size.y {
            let local_float_y = local_y as f32 + 0.5;
            let centered_y = local_float_y - correct_half_size_y;
            let world_y = position_y + centered_y;
            let voxel_y = world_y.floor() as i32;
            voxel_y_positions.push(voxel_y);
        }
        
        assert_eq!(voxel_y_positions, vec![2, 3, 4], 
            "With correct size, checks voxel 2 which has terrain!");
        assert!(voxel_y_positions.contains(&2), 
            "Must check voxel y=2 to detect floor collision");
    }
}
