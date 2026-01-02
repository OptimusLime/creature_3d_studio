//! Voxel occupancy collision system.
//!
//! This module provides efficient collision detection for voxel worlds by using
//! bit-packed occupancy data instead of trimesh colliders. This enables:
//!
//! - O(1) voxel lookups instead of O(n) triangle tests
//! - Minecraft-scale worlds without performance death
//! - Easy GPU upload for compute shader collision
//!
//! ## Architecture
//!
//! ```text
//! VoxelWorld (full voxel data with colors)
//!       │
//!       ▼
//! ChunkOccupancy (32x32x32 bit-packed = 4KB per chunk)
//!       │
//!       ▼
//! WorldOccupancy (HashMap of chunks, CPU collision queries)
//!       │
//!       ▼
//! GpuWorldOccupancy (texture array, GPU collision - see voxel_collision_gpu.rs)
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! use studio_core::voxel_collision::{ChunkOccupancy, WorldOccupancy};
//! use studio_core::voxel::VoxelWorld;
//!
//! // Convert VoxelWorld to occupancy
//! let mut world_occ = WorldOccupancy::new();
//! for (chunk_pos, chunk) in voxel_world.iter_chunks() {
//!     let occ = ChunkOccupancy::from_chunk(chunk);
//!     world_occ.load_chunk(chunk_pos.as_ivec3(), occ);
//! }
//!
//! // Query collision
//! let hit = world_occ.get_voxel(IVec3::new(10, 5, 10));
//! ```

use bevy::prelude::*;
use std::collections::HashMap;

use crate::voxel::{VoxelChunk, VoxelWorld, CHUNK_SIZE};

/// Size of a chunk in one dimension (must match voxel.rs).
pub const OCCUPANCY_CHUNK_SIZE: usize = CHUNK_SIZE;

/// Number of u32s needed to store one chunk's occupancy.
/// 32 * 32 * 32 = 32768 bits = 1024 u32s = 4096 bytes
const CHUNK_U32_COUNT: usize = (OCCUPANCY_CHUNK_SIZE * OCCUPANCY_CHUNK_SIZE * OCCUPANCY_CHUNK_SIZE) / 32;

/// Bit-packed occupancy data for a single 32x32x32 chunk.
///
/// Each bit represents whether a voxel position is occupied (1) or empty (0).
/// Total size: 4096 bytes (4KB) per chunk.
#[derive(Clone)]
pub struct ChunkOccupancy {
    /// Bit-packed occupancy data.
    /// Index formula: (x + y * 32 + z * 32 * 32) / 32 for u32 index
    /// Bit position: (x + y * 32 + z * 32 * 32) % 32
    data: [u32; CHUNK_U32_COUNT],
}

impl Default for ChunkOccupancy {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkOccupancy {
    /// Create a new empty chunk occupancy.
    pub fn new() -> Self {
        Self {
            data: [0; CHUNK_U32_COUNT],
        }
    }

    /// Create occupancy from a VoxelChunk.
    pub fn from_chunk(chunk: &VoxelChunk) -> Self {
        let mut occ = Self::new();
        for (x, y, z, _voxel) in chunk.iter() {
            occ.set(UVec3::new(x as u32, y as u32, z as u32), true);
        }
        occ
    }

    /// Create occupancy from a VoxelWorld for a specific chunk region.
    ///
    /// `chunk_min` is the world-space minimum corner of the chunk (must be chunk-aligned).
    pub fn from_voxel_world(world: &VoxelWorld, chunk_min: IVec3) -> Self {
        let mut occ = Self::new();
        
        for lx in 0..OCCUPANCY_CHUNK_SIZE {
            for ly in 0..OCCUPANCY_CHUNK_SIZE {
                for lz in 0..OCCUPANCY_CHUNK_SIZE {
                    let wx = chunk_min.x + lx as i32;
                    let wy = chunk_min.y + ly as i32;
                    let wz = chunk_min.z + lz as i32;
                    
                    if world.get_voxel(wx, wy, wz).is_some() {
                        occ.set(UVec3::new(lx as u32, ly as u32, lz as u32), true);
                    }
                }
            }
        }
        
        occ
    }

    /// Convert linear index to bit position.
    #[inline]
    fn index_to_bit(local_pos: UVec3) -> (usize, u32) {
        let linear = local_pos.x + local_pos.y * 32 + local_pos.z * 32 * 32;
        let u32_idx = (linear / 32) as usize;
        let bit_pos = linear % 32;
        (u32_idx, bit_pos)
    }

    /// Get occupancy at local position (0-31 in each dimension).
    #[inline]
    pub fn get(&self, local_pos: UVec3) -> bool {
        debug_assert!(local_pos.x < 32 && local_pos.y < 32 && local_pos.z < 32);
        let (idx, bit) = Self::index_to_bit(local_pos);
        (self.data[idx] & (1 << bit)) != 0
    }

    /// Set occupancy at local position.
    #[inline]
    pub fn set(&mut self, local_pos: UVec3, occupied: bool) {
        debug_assert!(local_pos.x < 32 && local_pos.y < 32 && local_pos.z < 32);
        let (idx, bit) = Self::index_to_bit(local_pos);
        if occupied {
            self.data[idx] |= 1 << bit;
        } else {
            self.data[idx] &= !(1 << bit);
        }
    }

    /// Get raw bytes for GPU upload.
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.data)
    }

    /// Count occupied voxels.
    pub fn count_occupied(&self) -> usize {
        self.data.iter().map(|&x| x.count_ones() as usize).sum()
    }

    /// Check if chunk is entirely empty.
    pub fn is_empty(&self) -> bool {
        self.data.iter().all(|&x| x == 0)
    }
}

// Note: ChunkOccupancy is too large (4KB) to implement Copy, so we can't use bytemuck::Pod directly.
// Instead, we cast the inner array when needed via as_bytes().

/// World-level occupancy manager.
///
/// Stores occupancy data for multiple chunks and provides collision queries.
#[derive(Default)]
pub struct WorldOccupancy {
    /// Chunk occupancy data indexed by chunk coordinate.
    chunks: HashMap<IVec3, ChunkOccupancy>,
}

impl WorldOccupancy {
    /// Create a new empty world occupancy.
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }

    /// Create from a VoxelWorld.
    pub fn from_voxel_world(world: &VoxelWorld) -> Self {
        let mut occ = Self::new();
        
        for (chunk_pos, chunk) in world.iter_chunks() {
            let coord = IVec3::new(chunk_pos.x, chunk_pos.y, chunk_pos.z);
            let chunk_occ = ChunkOccupancy::from_chunk(chunk);
            occ.chunks.insert(coord, chunk_occ);
        }
        
        occ
    }

    /// Load a chunk's occupancy data.
    pub fn load_chunk(&mut self, coord: IVec3, occupancy: ChunkOccupancy) {
        self.chunks.insert(coord, occupancy);
    }

    /// Unload a chunk.
    pub fn unload_chunk(&mut self, coord: IVec3) {
        self.chunks.remove(&coord);
    }

    /// Get chunk at coordinate.
    pub fn get_chunk(&self, coord: IVec3) -> Option<&ChunkOccupancy> {
        self.chunks.get(&coord)
    }

    /// Check if a world position is occupied.
    pub fn get_voxel(&self, world_pos: IVec3) -> bool {
        let chunk_coord = world_pos_to_chunk_coord(world_pos);
        let local_pos = world_pos_to_local(world_pos);
        
        self.chunks
            .get(&chunk_coord)
            .map(|chunk| chunk.get(local_pos))
            .unwrap_or(false)
    }

    /// Get all chunk coordinates that overlap an AABB.
    pub fn chunks_overlapping_aabb(&self, min: IVec3, max: IVec3) -> Vec<IVec3> {
        let min_chunk = world_pos_to_chunk_coord(min);
        let max_chunk = world_pos_to_chunk_coord(max);
        
        let mut result = Vec::new();
        for cx in min_chunk.x..=max_chunk.x {
            for cy in min_chunk.y..=max_chunk.y {
                for cz in min_chunk.z..=max_chunk.z {
                    let coord = IVec3::new(cx, cy, cz);
                    if self.chunks.contains_key(&coord) {
                        result.push(coord);
                    }
                }
            }
        }
        result
    }

    /// Check if a region is entirely clear (no occupied voxels).
    pub fn region_is_clear(&self, min: IVec3, max: IVec3) -> bool {
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    if self.get_voxel(IVec3::new(x, y, z)) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Get all occupied positions in a region.
    pub fn get_overlaps(&self, min: IVec3, max: IVec3) -> Vec<IVec3> {
        let mut result = Vec::new();
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    let pos = IVec3::new(x, y, z);
                    if self.get_voxel(pos) {
                        result.push(pos);
                    }
                }
            }
        }
        result
    }

    /// Number of loaded chunks.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Total occupied voxels across all chunks.
    pub fn total_occupied(&self) -> usize {
        self.chunks.values().map(|c| c.count_occupied()).sum()
    }
}

/// Convert world position to chunk coordinate.
#[inline]
pub fn world_pos_to_chunk_coord(world_pos: IVec3) -> IVec3 {
    IVec3::new(
        world_pos.x.div_euclid(OCCUPANCY_CHUNK_SIZE as i32),
        world_pos.y.div_euclid(OCCUPANCY_CHUNK_SIZE as i32),
        world_pos.z.div_euclid(OCCUPANCY_CHUNK_SIZE as i32),
    )
}

/// Convert world position to local chunk position (0-31).
#[inline]
pub fn world_pos_to_local(world_pos: IVec3) -> UVec3 {
    UVec3::new(
        world_pos.x.rem_euclid(OCCUPANCY_CHUNK_SIZE as i32) as u32,
        world_pos.y.rem_euclid(OCCUPANCY_CHUNK_SIZE as i32) as u32,
        world_pos.z.rem_euclid(OCCUPANCY_CHUNK_SIZE as i32) as u32,
    )
}

/// Convert chunk coordinate to world position (min corner).
#[inline]
pub fn chunk_coord_to_world(chunk_coord: IVec3) -> IVec3 {
    chunk_coord * OCCUPANCY_CHUNK_SIZE as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::Voxel;

    #[test]
    fn test_chunk_occupancy_new_is_empty() {
        let occ = ChunkOccupancy::new();
        assert!(occ.is_empty());
        assert_eq!(occ.count_occupied(), 0);
    }

    #[test]
    fn test_chunk_occupancy_roundtrip() {
        let mut occ = ChunkOccupancy::new();
        
        // Set some positions
        occ.set(UVec3::new(0, 0, 0), true);
        occ.set(UVec3::new(31, 31, 31), true);
        occ.set(UVec3::new(15, 15, 15), true);
        
        // Verify
        assert!(occ.get(UVec3::new(0, 0, 0)));
        assert!(occ.get(UVec3::new(31, 31, 31)));
        assert!(occ.get(UVec3::new(15, 15, 15)));
        assert!(!occ.get(UVec3::new(1, 0, 0)));
        assert!(!occ.get(UVec3::new(0, 1, 0)));
        
        assert_eq!(occ.count_occupied(), 3);
    }

    #[test]
    fn test_chunk_occupancy_set_unset() {
        let mut occ = ChunkOccupancy::new();
        
        occ.set(UVec3::new(5, 5, 5), true);
        assert!(occ.get(UVec3::new(5, 5, 5)));
        
        occ.set(UVec3::new(5, 5, 5), false);
        assert!(!occ.get(UVec3::new(5, 5, 5)));
    }

    #[test]
    fn test_chunk_occupancy_bit_packing() {
        let mut occ = ChunkOccupancy::new();
        
        // Set all voxels in first u32 (positions 0-31 in x, y=0, z=0)
        for x in 0..32 {
            occ.set(UVec3::new(x, 0, 0), true);
        }
        
        // First u32 should be all 1s
        assert_eq!(occ.data[0], u32::MAX);
        // Second u32 should be 0
        assert_eq!(occ.data[1], 0);
        
        assert_eq!(occ.count_occupied(), 32);
    }

    #[test]
    fn test_chunk_occupancy_from_voxel_world() {
        let mut world = VoxelWorld::new();
        
        // Set some voxels in chunk (0,0,0)
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        world.set_voxel(5, 10, 15, Voxel::solid(0, 255, 0));
        world.set_voxel(31, 31, 31, Voxel::solid(0, 0, 255));
        
        let occ = ChunkOccupancy::from_voxel_world(&world, IVec3::ZERO);
        
        assert!(occ.get(UVec3::new(0, 0, 0)));
        assert!(occ.get(UVec3::new(5, 10, 15)));
        assert!(occ.get(UVec3::new(31, 31, 31)));
        assert!(!occ.get(UVec3::new(1, 1, 1)));
        
        assert_eq!(occ.count_occupied(), 3);
    }

    #[test]
    fn test_chunk_occupancy_as_bytes() {
        let occ = ChunkOccupancy::new();
        let bytes = occ.as_bytes();
        
        // 1024 u32s * 4 bytes = 4096 bytes
        assert_eq!(bytes.len(), 4096);
    }

    #[test]
    fn test_world_pos_to_chunk_coord() {
        // Positive positions
        assert_eq!(world_pos_to_chunk_coord(IVec3::new(0, 0, 0)), IVec3::ZERO);
        assert_eq!(world_pos_to_chunk_coord(IVec3::new(31, 31, 31)), IVec3::ZERO);
        assert_eq!(world_pos_to_chunk_coord(IVec3::new(32, 0, 0)), IVec3::new(1, 0, 0));
        assert_eq!(world_pos_to_chunk_coord(IVec3::new(64, 64, 64)), IVec3::new(2, 2, 2));
        
        // Negative positions
        assert_eq!(world_pos_to_chunk_coord(IVec3::new(-1, 0, 0)), IVec3::new(-1, 0, 0));
        assert_eq!(world_pos_to_chunk_coord(IVec3::new(-32, 0, 0)), IVec3::new(-1, 0, 0));
        assert_eq!(world_pos_to_chunk_coord(IVec3::new(-33, 0, 0)), IVec3::new(-2, 0, 0));
    }

    #[test]
    fn test_world_pos_to_local() {
        assert_eq!(world_pos_to_local(IVec3::new(0, 0, 0)), UVec3::ZERO);
        assert_eq!(world_pos_to_local(IVec3::new(5, 10, 15)), UVec3::new(5, 10, 15));
        assert_eq!(world_pos_to_local(IVec3::new(31, 31, 31)), UVec3::new(31, 31, 31));
        assert_eq!(world_pos_to_local(IVec3::new(32, 0, 0)), UVec3::new(0, 0, 0));
        assert_eq!(world_pos_to_local(IVec3::new(37, 0, 0)), UVec3::new(5, 0, 0));
        
        // Negative positions
        assert_eq!(world_pos_to_local(IVec3::new(-1, 0, 0)), UVec3::new(31, 0, 0));
        assert_eq!(world_pos_to_local(IVec3::new(-32, 0, 0)), UVec3::new(0, 0, 0));
    }

    #[test]
    fn test_world_occupancy_single_chunk() {
        let mut world = VoxelWorld::new();
        world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
        world.set_voxel(10, 10, 10, Voxel::solid(0, 255, 0));
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        assert!(occ.get_voxel(IVec3::new(5, 5, 5)));
        assert!(occ.get_voxel(IVec3::new(10, 10, 10)));
        assert!(!occ.get_voxel(IVec3::new(0, 0, 0)));
        
        assert_eq!(occ.chunk_count(), 1);
        assert_eq!(occ.total_occupied(), 2);
    }

    #[test]
    fn test_world_occupancy_cross_chunk_query() {
        let mut world = VoxelWorld::new();
        
        // Voxels in different chunks
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));     // Chunk (0,0,0)
        world.set_voxel(32, 0, 0, Voxel::solid(0, 255, 0));    // Chunk (1,0,0)
        world.set_voxel(-1, 0, 0, Voxel::solid(0, 0, 255));    // Chunk (-1,0,0)
        world.set_voxel(32, 32, 32, Voxel::solid(255, 255, 0)); // Chunk (1,1,1)
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        assert!(occ.get_voxel(IVec3::new(0, 0, 0)));
        assert!(occ.get_voxel(IVec3::new(32, 0, 0)));
        assert!(occ.get_voxel(IVec3::new(-1, 0, 0)));
        assert!(occ.get_voxel(IVec3::new(32, 32, 32)));
        
        // Non-existent positions
        assert!(!occ.get_voxel(IVec3::new(100, 100, 100)));
        assert!(!occ.get_voxel(IVec3::new(1, 0, 0)));
        
        assert_eq!(occ.chunk_count(), 4);
    }

    #[test]
    fn test_world_occupancy_region_is_clear() {
        let mut world = VoxelWorld::new();
        world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        // Region containing the voxel
        assert!(!occ.region_is_clear(IVec3::new(0, 0, 0), IVec3::new(10, 10, 10)));
        
        // Region not containing the voxel
        assert!(occ.region_is_clear(IVec3::new(10, 10, 10), IVec3::new(20, 20, 20)));
        
        // Exact voxel
        assert!(!occ.region_is_clear(IVec3::new(5, 5, 5), IVec3::new(5, 5, 5)));
    }

    #[test]
    fn test_world_occupancy_get_overlaps() {
        let mut world = VoxelWorld::new();
        world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
        world.set_voxel(6, 5, 5, Voxel::solid(0, 255, 0));
        world.set_voxel(10, 10, 10, Voxel::solid(0, 0, 255));
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        let overlaps = occ.get_overlaps(IVec3::new(4, 4, 4), IVec3::new(7, 7, 7));
        
        assert_eq!(overlaps.len(), 2);
        assert!(overlaps.contains(&IVec3::new(5, 5, 5)));
        assert!(overlaps.contains(&IVec3::new(6, 5, 5)));
    }

    #[test]
    fn test_world_occupancy_chunks_overlapping_aabb() {
        let mut occ = WorldOccupancy::new();
        
        // Load some chunks
        occ.load_chunk(IVec3::new(0, 0, 0), ChunkOccupancy::new());
        occ.load_chunk(IVec3::new(1, 0, 0), ChunkOccupancy::new());
        occ.load_chunk(IVec3::new(0, 1, 0), ChunkOccupancy::new());
        occ.load_chunk(IVec3::new(5, 5, 5), ChunkOccupancy::new()); // Far away
        
        // AABB spanning chunks (0,0,0) and (1,0,0)
        let chunks = occ.chunks_overlapping_aabb(IVec3::new(16, 0, 0), IVec3::new(48, 16, 16));
        
        assert_eq!(chunks.len(), 2);
        assert!(chunks.contains(&IVec3::new(0, 0, 0)));
        assert!(chunks.contains(&IVec3::new(1, 0, 0)));
    }

    #[test]
    fn test_chunk_occupancy_from_chunk() {
        use crate::voxel::ChunkPos;
        
        let mut world = VoxelWorld::new();
        world.set_voxel(1, 2, 3, Voxel::solid(255, 0, 0));
        world.set_voxel(10, 20, 5, Voxel::solid(0, 255, 0));
        
        // Get the chunk directly
        let chunk_pos = ChunkPos::new(0, 0, 0);
        if let Some(chunk) = world.get_chunk(chunk_pos) {
            let occ = ChunkOccupancy::from_chunk(chunk);
            
            assert!(occ.get(UVec3::new(1, 2, 3)));
            assert!(occ.get(UVec3::new(10, 20, 5)));
            assert!(!occ.get(UVec3::new(0, 0, 0)));
            assert_eq!(occ.count_occupied(), 2);
        }
    }
}
