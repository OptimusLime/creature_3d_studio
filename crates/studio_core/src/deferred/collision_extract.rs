//! Fragment extraction for GPU collision.
//!
//! This module extracts VoxelFragment data from the main world to the render world
//! for GPU collision detection.

use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::Extract;

use crate::voxel_fragment::VoxelFragment;
use crate::voxel_collision::ChunkOccupancy;

/// Extracted fragment data for GPU collision.
#[derive(Clone)]
pub struct ExtractedFragment {
    /// Entity ID (for mapping contacts back to fragments)
    pub entity: Entity,
    /// World position of fragment center
    pub position: Vec3,
    /// Rotation quaternion
    pub rotation: Quat,
    /// Size in voxels
    pub size: UVec3,
    /// Bit-packed occupancy data (copied from FragmentOccupancy)
    pub occupancy_data: Vec<u32>,
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

/// System to extract fragment data from main world to render world.
///
/// Runs in ExtractSchedule to copy fragment transforms and occupancy data
/// to the render world for GPU collision processing.
pub fn extract_fragments_system(
    mut extracted: ResMut<ExtractedFragments>,
    fragments: Extract<Query<(Entity, &VoxelFragment, &Transform)>>,
) {
    extracted.fragments.clear();
    
    for (entity, fragment, transform) in fragments.iter() {
        extracted.fragments.push(ExtractedFragment {
            entity,
            position: transform.translation,
            rotation: transform.rotation,
            size: fragment.occupancy.size,
            occupancy_data: fragment.occupancy.as_u32_slice().to_vec(),
        });
    }
    
    if !extracted.fragments.is_empty() {
        trace!("Extracted {} fragments for GPU collision", extracted.fragments.len());
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
}
