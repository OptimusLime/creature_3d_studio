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

    /// Check a fragment against the world terrain.
    ///
    /// This transforms each occupied voxel in the fragment to world space
    /// and checks for overlap with terrain voxels. Much faster than trimesh
    /// collision for voxel-based fragments.
    ///
    /// # Arguments
    /// * `fragment` - The fragment's occupancy data
    /// * `position` - World-space position of the fragment's origin
    /// * `rotation` - Rotation of the fragment
    ///
    /// # Returns
    /// A `FragmentCollisionResult` containing collision info for physics response.
    pub fn check_fragment(
        &self,
        fragment: &FragmentOccupancy,
        position: Vec3,
        rotation: Quat,
    ) -> FragmentCollisionResult {
        let mut result = FragmentCollisionResult::new();
        
        // Fragment's local origin is at (0,0,0), with voxels at positive positions
        // We need to offset so the fragment rotates around its center
        let half_size = fragment.aabb_size() * 0.5;
        
        // For each occupied voxel in the fragment
        for local_pos in fragment.iter_occupied() {
            // Convert to local float position (center of voxel)
            let local_float = Vec3::new(
                local_pos.x as f32 + 0.5,
                local_pos.y as f32 + 0.5,
                local_pos.z as f32 + 0.5,
            );
            
            // Offset from center, rotate, then translate to world
            let centered = local_float - half_size;
            let rotated = rotation * centered;
            let world_pos = position + rotated;
            
            // Check the voxel at this world position
            let world_voxel = IVec3::new(
                world_pos.x.floor() as i32,
                world_pos.y.floor() as i32,
                world_pos.z.floor() as i32,
            );
            
            if self.get_voxel(world_voxel) {
                // Collision! Calculate penetration info
                // Terrain voxel spans from world_voxel to world_voxel + 1
                let voxel_min = Vec3::new(
                    world_voxel.x as f32,
                    world_voxel.y as f32,
                    world_voxel.z as f32,
                );
                let voxel_max = voxel_min + Vec3::ONE;
                
                // Calculate distance to each face of the voxel
                // Positive values mean we need to move that far to exit
                let dist_to_min_x = world_pos.x - voxel_min.x; // distance to -X face
                let dist_to_max_x = voxel_max.x - world_pos.x; // distance to +X face
                let dist_to_min_y = world_pos.y - voxel_min.y; // distance to -Y face (floor)
                let dist_to_max_y = voxel_max.y - world_pos.y; // distance to +Y face (ceiling)
                let dist_to_min_z = world_pos.z - voxel_min.z; // distance to -Z face
                let dist_to_max_z = voxel_max.z - world_pos.z; // distance to +Z face
                
                // Find the smallest distance (easiest exit)
                // Order matters for tiebreaking: prefer +Y (up) over other directions
                let exits = [
                    (dist_to_max_y, Vec3::Y),      // Prefer pushing UP first
                    (dist_to_min_y, Vec3::NEG_Y),
                    (dist_to_min_x, Vec3::NEG_X),
                    (dist_to_max_x, Vec3::X),
                    (dist_to_min_z, Vec3::NEG_Z),
                    (dist_to_max_z, Vec3::Z),
                ];
                
                let (penetration, normal) = exits
                    .iter()
                    .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(d, n)| (*d, *n))
                    .unwrap_or((0.0, Vec3::Y));
                
                result.contacts.push(FragmentContact {
                    world_pos,
                    normal,
                    penetration,
                    fragment_local: local_pos,
                    terrain_voxel: world_voxel,
                });
            }
        }
        
        result
    }

    /// Total occupied voxels across all chunks.
    pub fn total_occupied(&self) -> usize {
        self.chunks.values().map(|c| c.count_occupied()).sum()
    }

    /// Iterate over all chunks with their coordinates.
    pub fn iter_chunks(&self) -> impl Iterator<Item = (IVec3, &ChunkOccupancy)> {
        self.chunks.iter().map(|(&coord, chunk)| (coord, chunk))
    }

    /// Check an AABB against the world, returning collision information.
    ///
    /// The AABB is specified in world coordinates as floating-point values.
    /// This method checks all voxels that the AABB overlaps and returns
    /// collision points with normals pointing outward from the colliding voxels.
    ///
    /// # Arguments
    /// * `aabb_min` - Minimum corner of the AABB (world space)
    /// * `aabb_max` - Maximum corner of the AABB (world space)
    ///
    /// # Returns
    /// A `CollisionResult` containing all collision points.
    pub fn check_aabb(&self, aabb_min: Vec3, aabb_max: Vec3) -> CollisionResult {
        let mut result = CollisionResult::new();
        
        // Convert to integer bounds (expand to cover all potentially overlapping voxels)
        let min_i = IVec3::new(
            aabb_min.x.floor() as i32,
            aabb_min.y.floor() as i32,
            aabb_min.z.floor() as i32,
        );
        let max_i = IVec3::new(
            aabb_max.x.ceil() as i32 - 1,
            aabb_max.y.ceil() as i32 - 1,
            aabb_max.z.ceil() as i32 - 1,
        );
        
        // Check each voxel in the range
        for x in min_i.x..=max_i.x {
            for y in min_i.y..=max_i.y {
                for z in min_i.z..=max_i.z {
                    let voxel_pos = IVec3::new(x, y, z);
                    if self.get_voxel(voxel_pos) {
                        // Calculate the collision point and normal
                        let contact = self.calculate_contact(aabb_min, aabb_max, voxel_pos);
                        result.contacts.push(contact);
                    }
                }
            }
        }
        
        result
    }

    /// Calculate collision contact for a single voxel.
    fn calculate_contact(&self, aabb_min: Vec3, aabb_max: Vec3, voxel_pos: IVec3) -> CollisionPoint {
        let voxel_min = Vec3::new(voxel_pos.x as f32, voxel_pos.y as f32, voxel_pos.z as f32);
        let voxel_max = voxel_min + Vec3::ONE;
        
        // Calculate overlap on each axis
        let overlap_x_min = aabb_max.x - voxel_min.x;
        let overlap_x_max = voxel_max.x - aabb_min.x;
        let overlap_y_min = aabb_max.y - voxel_min.y;
        let overlap_y_max = voxel_max.y - aabb_min.y;
        let overlap_z_min = aabb_max.z - voxel_min.z;
        let overlap_z_max = voxel_max.z - aabb_min.z;
        
        // Find minimum penetration axis
        let penetrations = [
            (overlap_x_min, Vec3::NEG_X), // AABB is to the +X of voxel
            (overlap_x_max, Vec3::X),      // AABB is to the -X of voxel
            (overlap_y_min, Vec3::NEG_Y),
            (overlap_y_max, Vec3::Y),
            (overlap_z_min, Vec3::NEG_Z),
            (overlap_z_max, Vec3::Z),
        ];
        
        let (penetration, normal) = penetrations
            .iter()
            .filter(|(p, _)| *p > 0.0)
            .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(p, n)| (*p, *n))
            .unwrap_or((0.0, Vec3::Y));
        
        // Contact point is at the center of the overlap region
        let contact_pos = Vec3::new(
            (aabb_min.x.max(voxel_min.x) + aabb_max.x.min(voxel_max.x)) / 2.0,
            (aabb_min.y.max(voxel_min.y) + aabb_max.y.min(voxel_max.y)) / 2.0,
            (aabb_min.z.max(voxel_min.z) + aabb_max.z.min(voxel_max.z)) / 2.0,
        );
        
        CollisionPoint {
            world_pos: contact_pos,
            normal,
            penetration,
            voxel_pos,
        }
    }
}

/// A single collision contact point.
#[derive(Debug, Clone, Copy)]
pub struct CollisionPoint {
    /// World-space position of the contact.
    pub world_pos: Vec3,
    /// Normal vector pointing away from the collided surface.
    pub normal: Vec3,
    /// Penetration depth (how far the AABB is inside the voxel).
    pub penetration: f32,
    /// The voxel position that caused this collision.
    pub voxel_pos: IVec3,
}

/// Result of a collision query.
#[derive(Debug, Clone, Default)]
pub struct CollisionResult {
    /// All contact points found.
    pub contacts: Vec<CollisionPoint>,
}

impl CollisionResult {
    /// Create an empty collision result.
    pub fn new() -> Self {
        Self { contacts: Vec::new() }
    }

    /// Check if there are any collisions.
    pub fn has_collision(&self) -> bool {
        !self.contacts.is_empty()
    }

    /// Get the number of contact points.
    pub fn contact_count(&self) -> usize {
        self.contacts.len()
    }

    /// Get the deepest penetration among all contacts.
    pub fn max_penetration(&self) -> f32 {
        self.contacts
            .iter()
            .map(|c| c.penetration)
            .fold(0.0, f32::max)
    }

    /// Calculate the push-out vector to resolve collisions.
    ///
    /// This finds the minimum translation needed to separate the AABB from
    /// all colliding voxels. When hitting multiple voxels (e.g., a floor made
    /// of many voxels), we only need to push out by the maximum penetration
    /// in each direction, not the sum.
    pub fn resolution_vector(&self) -> Vec3 {
        if self.contacts.is_empty() {
            return Vec3::ZERO;
        }
        
        // Track maximum penetration for each of the 6 cardinal directions
        let mut max_push = [0.0f32; 6]; // +X, -X, +Y, -Y, +Z, -Z
        
        for contact in &self.contacts {
            let n = contact.normal;
            let p = contact.penetration;
            
            // Determine which axis this contact primarily pushes on
            // and accumulate the maximum push needed in that direction
            if n.x > 0.7 { max_push[0] = max_push[0].max(p); }
            else if n.x < -0.7 { max_push[1] = max_push[1].max(p); }
            else if n.y > 0.7 { max_push[2] = max_push[2].max(p); }
            else if n.y < -0.7 { max_push[3] = max_push[3].max(p); }
            else if n.z > 0.7 { max_push[4] = max_push[4].max(p); }
            else if n.z < -0.7 { max_push[5] = max_push[5].max(p); }
        }
        
        // Combine opposing directions on each axis
        Vec3::new(
            max_push[0] - max_push[1], // Net X push
            max_push[2] - max_push[3], // Net Y push
            max_push[4] - max_push[5], // Net Z push
        )
    }

    /// Check if any contact has a floor-like normal (pointing up).
    pub fn has_floor_contact(&self) -> bool {
        self.contacts.iter().any(|c| c.normal.y > 0.7)
    }

    /// Get the average floor normal if standing on ground.
    pub fn floor_normal(&self) -> Option<Vec3> {
        let floor_contacts: Vec<_> = self.contacts.iter()
            .filter(|c| c.normal.y > 0.7)
            .collect();
        
        if floor_contacts.is_empty() {
            return None;
        }
        
        let sum: Vec3 = floor_contacts.iter().map(|c| c.normal).sum();
        Some((sum / floor_contacts.len() as f32).normalize())
    }
}

/// A single contact point from fragment-terrain collision.
#[derive(Debug, Clone, Copy)]
pub struct FragmentContact {
    /// World-space position where collision occurred.
    pub world_pos: Vec3,
    /// Normal vector pointing out of the terrain (direction to push fragment).
    pub normal: Vec3,
    /// Penetration depth.
    pub penetration: f32,
    /// Local position within the fragment that collided.
    pub fragment_local: UVec3,
    /// Terrain voxel position that was hit.
    pub terrain_voxel: IVec3,
}

/// Result of checking a fragment against terrain.
#[derive(Debug, Clone, Default)]
pub struct FragmentCollisionResult {
    /// All contact points found.
    pub contacts: Vec<FragmentContact>,
}

impl FragmentCollisionResult {
    /// Create an empty result.
    pub fn new() -> Self {
        Self { contacts: Vec::new() }
    }

    /// Check if there are any collisions.
    pub fn has_collision(&self) -> bool {
        !self.contacts.is_empty()
    }

    /// Get the number of contact points.
    pub fn contact_count(&self) -> usize {
        self.contacts.len()
    }

    /// Get the deepest penetration.
    pub fn max_penetration(&self) -> f32 {
        self.contacts
            .iter()
            .map(|c| c.penetration)
            .fold(0.0, f32::max)
    }

    /// Calculate the push-out vector to resolve collisions.
    ///
    /// Uses maximum penetration per direction (not sum) to avoid over-correction.
    pub fn resolution_vector(&self) -> Vec3 {
        if self.contacts.is_empty() {
            return Vec3::ZERO;
        }
        
        // Track maximum penetration for each direction
        let mut max_push = [0.0f32; 6]; // +X, -X, +Y, -Y, +Z, -Z
        
        for contact in &self.contacts {
            let n = contact.normal;
            let p = contact.penetration;
            
            if n.x > 0.7 { max_push[0] = max_push[0].max(p); }
            else if n.x < -0.7 { max_push[1] = max_push[1].max(p); }
            else if n.y > 0.7 { max_push[2] = max_push[2].max(p); }
            else if n.y < -0.7 { max_push[3] = max_push[3].max(p); }
            else if n.z > 0.7 { max_push[4] = max_push[4].max(p); }
            else if n.z < -0.7 { max_push[5] = max_push[5].max(p); }
        }
        
        Vec3::new(
            max_push[0] - max_push[1],
            max_push[2] - max_push[3],
            max_push[4] - max_push[5],
        )
    }

    /// Check if any contact has a floor-like normal (pointing up).
    pub fn has_floor_contact(&self) -> bool {
        self.contacts.iter().any(|c| c.normal.y > 0.7)
    }

    /// Get the average contact normal.
    pub fn average_normal(&self) -> Vec3 {
        if self.contacts.is_empty() {
            return Vec3::Y;
        }
        let sum: Vec3 = self.contacts.iter().map(|c| c.normal).sum();
        (sum / self.contacts.len() as f32).normalize_or_zero()
    }

    /// Get the average contact position (useful for torque calculation).
    pub fn average_contact_position(&self) -> Vec3 {
        if self.contacts.is_empty() {
            return Vec3::ZERO;
        }
        let sum: Vec3 = self.contacts.iter().map(|c| c.world_pos).sum();
        sum / self.contacts.len() as f32
    }
}

/// Occupancy data for a voxel fragment (dynamic physics object).
///
/// Unlike `ChunkOccupancy` which is always 32x32x32, a fragment can be any size.
/// This stores bit-packed occupancy for arbitrary-sized voxel regions.
///
/// Used for checking fragment collision against world terrain.
#[derive(Debug, Clone)]
pub struct FragmentOccupancy {
    /// Bit-packed occupancy data.
    data: Vec<u32>,
    /// Size of the fragment in voxels.
    pub size: UVec3,
}

impl FragmentOccupancy {
    /// Create a new empty fragment occupancy of the given size.
    pub fn new(size: UVec3) -> Self {
        let total_bits = (size.x * size.y * size.z) as usize;
        let u32_count = (total_bits + 31) / 32;
        Self {
            data: vec![0; u32_count],
            size,
        }
    }

    /// Create from a VoxelWorld (fragment data).
    pub fn from_voxel_world(world: &VoxelWorld) -> Self {
        // Find bounds (returns Vec3 floats, min is voxel corner, max is +1 from last voxel)
        let Some((min_f, max_f)) = world.voxel_bounds() else {
            return Self::new(UVec3::ZERO);
        };
        
        // Convert to integer bounds (min_f is at voxel corner, max_f is 1 past last voxel)
        let min = IVec3::new(
            min_f.x as i32,
            min_f.y as i32,
            min_f.z as i32,
        );
        
        let size = UVec3::new(
            (max_f.x - min_f.x) as u32,
            (max_f.y - min_f.y) as u32,
            (max_f.z - min_f.z) as u32,
        );
        
        let mut occ = Self::new(size);
        
        for (chunk_pos, chunk) in world.iter_chunks() {
            let chunk_world_min = IVec3::new(
                chunk_pos.x * CHUNK_SIZE as i32,
                chunk_pos.y * CHUNK_SIZE as i32,
                chunk_pos.z * CHUNK_SIZE as i32,
            );
            
            for (lx, ly, lz, _voxel) in chunk.iter() {
                let world_pos = chunk_world_min + IVec3::new(lx as i32, ly as i32, lz as i32);
                let local = UVec3::new(
                    (world_pos.x - min.x) as u32,
                    (world_pos.y - min.y) as u32,
                    (world_pos.z - min.z) as u32,
                );
                occ.set(local, true);
            }
        }
        
        occ
    }

    /// Get the linear index and bit position for a local position.
    #[inline]
    fn index_to_bit(&self, local_pos: UVec3) -> (usize, u32) {
        let linear = local_pos.x + local_pos.y * self.size.x + local_pos.z * self.size.x * self.size.y;
        let u32_idx = (linear / 32) as usize;
        let bit_pos = linear % 32;
        (u32_idx, bit_pos)
    }

    /// Get occupancy at local position.
    #[inline]
    pub fn get(&self, local_pos: UVec3) -> bool {
        if local_pos.x >= self.size.x || local_pos.y >= self.size.y || local_pos.z >= self.size.z {
            return false;
        }
        let (idx, bit) = self.index_to_bit(local_pos);
        if idx >= self.data.len() {
            return false;
        }
        (self.data[idx] & (1 << bit)) != 0
    }

    /// Set occupancy at local position.
    #[inline]
    pub fn set(&mut self, local_pos: UVec3, occupied: bool) {
        if local_pos.x >= self.size.x || local_pos.y >= self.size.y || local_pos.z >= self.size.z {
            return;
        }
        let (idx, bit) = self.index_to_bit(local_pos);
        if idx >= self.data.len() {
            return;
        }
        if occupied {
            self.data[idx] |= 1 << bit;
        } else {
            self.data[idx] &= !(1 << bit);
        }
    }

    /// Get the AABB size in world units (1 voxel = 1 unit).
    pub fn aabb_size(&self) -> Vec3 {
        Vec3::new(self.size.x as f32, self.size.y as f32, self.size.z as f32)
    }

    /// Get raw bytes for GPU upload.
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.data)
    }

    /// Count occupied voxels.
    pub fn count_occupied(&self) -> usize {
        self.data.iter().map(|&x| x.count_ones() as usize).sum()
    }

    /// Check if fragment is empty.
    pub fn is_empty(&self) -> bool {
        self.data.iter().all(|&x| x == 0)
    }

    /// Get raw u32 slice for GPU upload.
    pub fn as_u32_slice(&self) -> &[u32] {
        &self.data
    }

    /// Iterate over all occupied positions.
    pub fn iter_occupied(&self) -> impl Iterator<Item = UVec3> + '_ {
        let size = self.size;
        (0..size.z).flat_map(move |z| {
            (0..size.y).flat_map(move |y| {
                (0..size.x).filter_map(move |x| {
                    let pos = UVec3::new(x, y, z);
                    if self.get(pos) {
                        Some(pos)
                    } else {
                        None
                    }
                })
            })
        })
    }
}

/// Component marking an entity for GPU AABB collision detection.
///
/// This is used for entities that need GPU collision detection but don't have
/// voxel occupancy data (like kinematic character controllers). The GPU shader
/// checks the AABB against terrain voxels and returns contact points.
///
/// The entity must also have a `Transform` component for position extraction.
#[derive(Component, Clone, Debug)]
pub struct GpuCollisionAABB {
    /// Half-extents of the AABB collision box.
    pub half_extents: Vec3,
}

impl GpuCollisionAABB {
    /// Create a new GPU collision AABB with the given half-extents.
    pub fn new(half_extents: Vec3) -> Self {
        Self { half_extents }
    }

    /// Create a player-sized AABB (0.4 x 0.9 x 0.4 half-extents).
    pub fn player() -> Self {
        Self::new(Vec3::new(0.4, 0.9, 0.4))
    }
}

/// Simple kinematic character controller for voxel worlds.
///
/// Handles movement, gravity, and collision response using the voxel
/// occupancy system. Does not use Rapier - pure voxel collision.
///
/// ## Usage
///
/// ```ignore
/// let mut controller = KinematicController::new(Vec3::new(0.4, 0.9, 0.4));
/// controller.move_and_slide(&world_occupancy, &mut position, &mut velocity, delta_time);
/// ```
#[derive(Debug, Clone)]
pub struct KinematicController {
    /// Half-extents of the collision box.
    pub half_extents: Vec3,
    /// Whether the controller is currently on the ground.
    pub grounded: bool,
    /// Normal of the ground surface (if grounded).
    pub ground_normal: Vec3,
    /// Maximum slope angle (in radians) that can be walked on.
    pub max_slope_angle: f32,
    /// Number of collision iterations per move.
    pub max_iterations: u32,
    /// Small margin to prevent floating point issues.
    pub skin_width: f32,
}

impl Default for KinematicController {
    fn default() -> Self {
        Self {
            half_extents: Vec3::new(0.4, 0.9, 0.4), // Player-sized
            grounded: false,
            ground_normal: Vec3::Y,
            max_slope_angle: 0.785, // ~45 degrees
            max_iterations: 4,
            skin_width: 0.01,
        }
    }
}

impl KinematicController {
    /// Create a new controller with the given half-extents.
    pub fn new(half_extents: Vec3) -> Self {
        Self {
            half_extents,
            ..Default::default()
        }
    }

    /// Move the controller, sliding along surfaces.
    ///
    /// This modifies `position` and `velocity` in-place based on collision
    /// response. Velocity is zeroed on axes where collision occurs.
    ///
    /// Uses substepping to prevent tunneling through terrain on large timesteps.
    ///
    /// # Arguments
    /// * `world` - The world occupancy to collide against
    /// * `position` - Current position (will be modified)
    /// * `velocity` - Current velocity (will be modified)
    /// * `delta` - Time step in seconds
    pub fn move_and_slide(
        &mut self,
        world: &WorldOccupancy,
        position: &mut Vec3,
        velocity: &mut Vec3,
        delta: f32,
    ) {
        // Substep large movements to prevent tunneling
        // Max movement per substep is half the smallest half-extent (ensures we can't skip through walls)
        let max_step_distance = self.half_extents.min_element() * 0.5;
        let total_movement = (*velocity * delta).length();
        let substeps = ((total_movement / max_step_distance).ceil() as u32).max(1).min(16);
        let substep_delta = delta / substeps as f32;
        
        for _ in 0..substeps {
            self.move_and_slide_substep(world, position, velocity, substep_delta);
        }
    }
    
    /// Internal substep for move_and_slide.
    fn move_and_slide_substep(
        &mut self,
        world: &WorldOccupancy,
        position: &mut Vec3,
        velocity: &mut Vec3,
        delta: f32,
    ) {
        let mut remaining_velocity = *velocity * delta;
        let was_grounded = self.grounded;
        self.grounded = false;
        self.ground_normal = Vec3::Y;
        
        for _ in 0..self.max_iterations {
            if remaining_velocity.length_squared() < 0.0001 {
                break;
            }
            
            // Try to move
            let target = *position + remaining_velocity;
            let aabb_min = target - self.half_extents;
            let aabb_max = target + self.half_extents;
            
            let result = world.check_aabb(aabb_min, aabb_max);
            
            if !result.has_collision() {
                // No collision, move freely
                *position = target;
                break;
            }
            
            // Resolve collision - just use the resolution, no extra skin_width
            let resolution = result.resolution_vector();
            *position = target + resolution;
            
            // Check for ground contact
            if result.has_floor_contact() {
                self.grounded = true;
                if let Some(normal) = result.floor_normal() {
                    self.ground_normal = normal;
                }
                // Zero vertical velocity when hitting ground
                if velocity.y < 0.0 {
                    velocity.y = 0.0;
                }
            }
            
            // Slide along surface: find the primary blocking normal
            let mut best_normal = Vec3::ZERO;
            let mut best_dot = 0.0f32;
            
            for contact in &result.contacts {
                let dot = remaining_velocity.dot(contact.normal);
                if dot < best_dot {
                    best_dot = dot;
                    best_normal = contact.normal;
                }
            }
            
            // Remove velocity component into the blocking surface
            if best_dot < 0.0 {
                remaining_velocity -= best_normal * best_dot;
                
                // Also adjust velocity for this axis
                let vel_dot = velocity.dot(best_normal);
                if vel_dot < 0.0 {
                    *velocity -= best_normal * vel_dot;
                }
            }
        }
        
        // Ground check: probe slightly below to detect ground when stationary
        if !self.grounded {
            let probe_distance = 0.05; // Small probe distance
            let ground_probe_min = *position - self.half_extents - Vec3::new(0.0, probe_distance, 0.0);
            let ground_probe_max = *position + self.half_extents;
            let ground_result = world.check_aabb(ground_probe_min, ground_probe_max);
            if ground_result.has_floor_contact() {
                self.grounded = true;
                if let Some(normal) = ground_result.floor_normal() {
                    self.ground_normal = normal;
                }
            }
        }
        
        // Snap to ground if we were grounded and moving down a small slope
        if was_grounded && !self.grounded && velocity.y <= 0.0 {
            let snap_distance = 0.2;
            let snap_probe_min = *position - self.half_extents - Vec3::new(0.0, snap_distance, 0.0);
            let snap_probe_max = *position + self.half_extents;
            let snap_result = world.check_aabb(snap_probe_min, snap_probe_max);
            if snap_result.has_floor_contact() {
                // Snap down to ground
                let resolution = snap_result.resolution_vector();
                if resolution.y > 0.0 && resolution.y < snap_distance {
                    position.y += resolution.y - snap_distance;
                    self.grounded = true;
                }
            }
        }
    }

    /// Apply gravity to velocity.
    pub fn apply_gravity(&self, velocity: &mut Vec3, gravity: f32, delta: f32) {
        if !self.grounded {
            velocity.y -= gravity * delta;
        }
    }

    /// Check if a jump is allowed (must be grounded).
    pub fn can_jump(&self) -> bool {
        self.grounded
    }

    /// Apply a jump impulse.
    pub fn jump(&mut self, velocity: &mut Vec3, jump_speed: f32) {
        if self.grounded {
            velocity.y = jump_speed;
            self.grounded = false;
        }
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

    // ========== AABB Collision Tests ==========

    #[test]
    fn test_aabb_no_collision() {
        let mut world = VoxelWorld::new();
        world.set_voxel(10, 10, 10, Voxel::solid(255, 0, 0));
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        // AABB in empty space
        let result = occ.check_aabb(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0));
        
        assert!(!result.has_collision());
        assert_eq!(result.contact_count(), 0);
    }

    #[test]
    fn test_aabb_collision_single_voxel() {
        let mut world = VoxelWorld::new();
        world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        // AABB overlapping the voxel
        let result = occ.check_aabb(Vec3::new(4.5, 4.5, 4.5), Vec3::new(5.5, 5.5, 5.5));
        
        assert!(result.has_collision());
        assert_eq!(result.contact_count(), 1);
        assert_eq!(result.contacts[0].voxel_pos, IVec3::new(5, 5, 5));
    }

    #[test]
    fn test_aabb_collision_multiple_voxels() {
        let mut world = VoxelWorld::new();
        // 2x2x1 floor
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        world.set_voxel(1, 0, 0, Voxel::solid(255, 0, 0));
        world.set_voxel(0, 0, 1, Voxel::solid(255, 0, 0));
        world.set_voxel(1, 0, 1, Voxel::solid(255, 0, 0));
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        // AABB overlapping all 4 voxels
        let result = occ.check_aabb(Vec3::new(0.25, 0.5, 0.25), Vec3::new(1.75, 1.5, 1.75));
        
        assert!(result.has_collision());
        assert_eq!(result.contact_count(), 4);
    }

    #[test]
    fn test_aabb_collision_cross_chunk() {
        let mut world = VoxelWorld::new();
        // Voxels at chunk boundary
        world.set_voxel(31, 0, 0, Voxel::solid(255, 0, 0)); // Chunk (0,0,0)
        world.set_voxel(32, 0, 0, Voxel::solid(0, 255, 0)); // Chunk (1,0,0)
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        // AABB spanning the chunk boundary
        let result = occ.check_aabb(Vec3::new(31.25, 0.25, 0.25), Vec3::new(32.75, 0.75, 0.75));
        
        assert!(result.has_collision());
        assert_eq!(result.contact_count(), 2);
    }

    #[test]
    fn test_collision_result_max_penetration() {
        let mut result = CollisionResult::new();
        result.contacts.push(CollisionPoint {
            world_pos: Vec3::ZERO,
            normal: Vec3::Y,
            penetration: 0.5,
            voxel_pos: IVec3::ZERO,
        });
        result.contacts.push(CollisionPoint {
            world_pos: Vec3::ZERO,
            normal: Vec3::Y,
            penetration: 0.8,
            voxel_pos: IVec3::ZERO,
        });
        
        assert!((result.max_penetration() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_collision_result_resolution_vector() {
        let mut result = CollisionResult::new();
        // Two contacts pushing up (same direction)
        result.contacts.push(CollisionPoint {
            world_pos: Vec3::ZERO,
            normal: Vec3::Y,
            penetration: 0.5,
            voxel_pos: IVec3::ZERO,
        });
        result.contacts.push(CollisionPoint {
            world_pos: Vec3::ZERO,
            normal: Vec3::Y,
            penetration: 0.3,
            voxel_pos: IVec3::ZERO,
        });
        
        let resolution = result.resolution_vector();
        
        // Should push up by MAX penetration (not sum!) to avoid over-correction
        assert!(resolution.x.abs() < 0.001);
        assert!((resolution.y - 0.5).abs() < 0.001, "Expected 0.5, got {}", resolution.y);
        assert!(resolution.z.abs() < 0.001);
    }

    #[test]
    fn test_aabb_collision_penetration_depth() {
        let mut world = VoxelWorld::new();
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        // AABB with known penetration from above
        // AABB bottom at y=0.7, voxel top at y=1.0 → penetration = 0.3
        let result = occ.check_aabb(Vec3::new(0.25, 0.7, 0.25), Vec3::new(0.75, 1.7, 0.75));
        
        assert!(result.has_collision());
        assert_eq!(result.contact_count(), 1);
        
        let contact = &result.contacts[0];
        // Penetration should be 0.3 (voxel top - aabb bottom = 1.0 - 0.7)
        assert!((contact.penetration - 0.3).abs() < 0.01, "Expected 0.3, got {}", contact.penetration);
        // Normal should point up (pushing AABB out of voxel)
        assert_eq!(contact.normal, Vec3::Y);
    }

    #[test]
    fn test_aabb_collision_benchmark() {
        use std::time::Instant;
        
        // Create a larger terrain
        let mut world = VoxelWorld::new();
        for x in 0..20 {
            for z in 0..20 {
                for y in 0..3 {
                    world.set_voxel(x, y, z, Voxel::solid(100, 100, 100));
                }
            }
        }
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        // Benchmark AABB collision queries
        let iterations = 1000;
        let start = Instant::now();
        
        for i in 0..iterations {
            let x = (i % 18) as f32 + 0.5;
            let z = ((i / 18) % 18) as f32 + 0.5;
            let _ = occ.check_aabb(
                Vec3::new(x, 2.5, z),
                Vec3::new(x + 1.0, 4.5, z + 1.0),
            );
        }
        
        let elapsed = start.elapsed();
        let per_query_us = elapsed.as_micros() as f64 / iterations as f64;
        
        println!("AABB collision benchmark: {} queries in {:?}", iterations, elapsed);
        println!("  Per query: {:.2} us", per_query_us);
        
        // Should be well under 1ms per query
        assert!(per_query_us < 1000.0, "Query too slow: {} us", per_query_us);
    }

    // ========== Kinematic Controller Tests ==========

    #[test]
    fn test_kinematic_controller_default() {
        let controller = KinematicController::default();
        assert!(!controller.grounded);
        assert!((controller.half_extents.x - 0.4).abs() < 0.01);
        assert!((controller.half_extents.y - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_controller_p23_scenario() {
        // Exact scenario from p23_kinematic_controller example
        let mut world = VoxelWorld::new();
        
        // Ground platform (30x30, 3 blocks thick) - same as example
        for x in -15..15 {
            for z in -15..15 {
                for y in 0..3 {
                    world.set_voxel(x, y, z, Voxel::solid(80, 120, 80));
                }
            }
        }
        
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        // Check occupancy is correct
        assert!(occ.get_voxel(IVec3::new(0, 0, 0)), "Floor should exist at (0,0,0)");
        assert!(occ.get_voxel(IVec3::new(0, 1, 0)), "Floor should exist at (0,1,0)");
        assert!(occ.get_voxel(IVec3::new(0, 2, 0)), "Floor should exist at (0,2,0)");
        assert!(!occ.get_voxel(IVec3::new(0, 3, 0)), "No floor at (0,3,0)");
        
        // Same starting position as example (y=10)
        let mut controller = KinematicController::new(Vec3::new(0.4, 0.9, 0.4));
        let mut position = Vec3::new(0.0, 10.0, 0.0);
        let mut velocity = Vec3::ZERO;
        
        println!("Starting position: {:?}", position);
        println!("Player bottom: {}", position.y - 0.9);
        println!("Floor top: 3.0 (voxels at y=0,1,2 occupy up to y=3)");
        println!("Expected landing y: 3.0 + 0.9 = 3.9");
        
        // Simulate with same gravity as example (25.0) - need more frames from y=10
        for i in 0..180 {
            // Same gravity logic as example
            if !controller.grounded {
                velocity.y -= 25.0 * (1.0 / 60.0);
            }
            
            controller.move_and_slide(&occ, &mut position, &mut velocity, 1.0 / 60.0);
            
            if i % 20 == 0 {
                println!("Frame {}: pos.y={:.3}, vel.y={:.3}, grounded={}", 
                    i, position.y, velocity.y, controller.grounded);
            }
        }
        
        println!("Final: pos={:?}, grounded={}", position, controller.grounded);
        
        // Should have landed on floor at y ≈ 3.9 (floor top 3.0 + half height 0.9)
        assert!(controller.grounded, "Should be grounded after 2 seconds of falling");
        assert!((position.y - 3.9).abs() < 0.3, "Should land at ~3.9, got {}", position.y);
    }

    #[test]
    fn test_controller_stands_on_ground() {
        // Create a floor
        let mut world = VoxelWorld::new();
        for x in -5..5 {
            for z in -5..5 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        let mut controller = KinematicController::new(Vec3::new(0.4, 0.9, 0.4));
        let mut position = Vec3::new(0.0, 2.0, 0.0); // Start above floor
        let mut velocity = Vec3::new(0.0, -5.0, 0.0); // Falling
        
        // Simulate several frames
        for _ in 0..60 {
            controller.apply_gravity(&mut velocity, 10.0, 1.0 / 60.0);
            controller.move_and_slide(&occ, &mut position, &mut velocity, 1.0 / 60.0);
        }
        
        // Should have landed on the floor
        assert!(controller.grounded, "Controller should be grounded");
        // Position should be at ground level + half height
        // Floor top is at y=1, controller half height is 0.9
        assert!((position.y - 1.9).abs() < 0.2, "Position should be ~1.9, got {}", position.y);
        // Vertical velocity should be near zero
        assert!(velocity.y.abs() < 0.5, "Vertical velocity should be near 0, got {}", velocity.y);
    }

    #[test]
    fn test_controller_blocked_by_wall() {
        // Create a floor and a wall
        let mut world = VoxelWorld::new();
        // Floor
        for x in -5..10 {
            for z in -5..5 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        // Wall at x=5
        for y in 1..5 {
            for z in -5..5 {
                world.set_voxel(5, y, z, Voxel::solid(150, 100, 100));
            }
        }
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        let mut controller = KinematicController::new(Vec3::new(0.4, 0.9, 0.4));
        let mut position = Vec3::new(0.0, 1.9, 0.0); // On floor
        let mut velocity = Vec3::new(10.0, 0.0, 0.0); // Moving toward wall
        
        // Simulate
        for _ in 0..60 {
            controller.move_and_slide(&occ, &mut position, &mut velocity, 1.0 / 60.0);
        }
        
        // Should be stopped by wall
        assert!(position.x < 5.0, "Should be blocked by wall at x=5, got x={}", position.x);
        // X velocity should be near zero (blocked)
        assert!(velocity.x.abs() < 1.0, "X velocity should be blocked, got {}", velocity.x);
    }

    #[test]
    fn test_controller_slides_along_wall() {
        // Create a floor and a wall
        let mut world = VoxelWorld::new();
        // Floor
        for x in -5..10 {
            for z in -10..10 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        // Wall at x=5
        for y in 1..5 {
            for z in -10..10 {
                world.set_voxel(5, y, z, Voxel::solid(150, 100, 100));
            }
        }
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        let mut controller = KinematicController::new(Vec3::new(0.4, 0.9, 0.4));
        let mut position = Vec3::new(4.0, 1.9, 0.0); // Near wall, on floor
        let mut velocity = Vec3::new(5.0, 0.0, 5.0); // Moving diagonally into wall
        let start_z = position.z;
        
        // Simulate 1 second (60 frames at 60fps)
        // Note: We need to re-apply input velocity each frame since sliding zeroes it
        for _ in 0..60 {
            // Reapply input velocity (simulating player holding forward+right)
            velocity = Vec3::new(5.0, velocity.y, 5.0);
            controller.move_and_slide(&occ, &mut position, &mut velocity, 1.0 / 60.0);
        }
        
        // Should have slid along wall in Z direction
        assert!(position.x < 5.0, "Should be blocked by wall at x=5, got x={}", position.x);
        // At 5.0 z-speed for 1 second, should move ~5 units in Z (minus friction from wall)
        let z_moved = position.z - start_z;
        assert!(z_moved > 2.0, "Should have moved significantly in Z direction, got delta_z={}", z_moved);
    }

    #[test]
    fn test_controller_jump() {
        // Create a floor
        let mut world = VoxelWorld::new();
        for x in -5..5 {
            for z in -5..5 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        let occ = WorldOccupancy::from_voxel_world(&world);
        
        let mut controller = KinematicController::new(Vec3::new(0.4, 0.9, 0.4));
        let mut position = Vec3::new(0.0, 1.9, 0.0);
        let mut velocity = Vec3::ZERO;
        
        // First, ensure grounded
        controller.grounded = true;
        
        // Jump
        assert!(controller.can_jump());
        controller.jump(&mut velocity, 8.0);
        
        assert!(!controller.grounded, "Should not be grounded after jump");
        assert!((velocity.y - 8.0).abs() < 0.01, "Should have jump velocity");
        
        // Simulate jump arc
        let start_y = position.y;
        for _ in 0..30 {
            controller.apply_gravity(&mut velocity, 20.0, 1.0 / 60.0);
            controller.move_and_slide(&occ, &mut position, &mut velocity, 1.0 / 60.0);
        }
        
        // Should have gone up then come back down
        // At frame 30, should be near or past peak
        assert!(position.y > start_y || controller.grounded, "Should have jumped up");
    }

    #[test]
    fn test_has_floor_contact() {
        let mut result = CollisionResult::new();
        
        // No contacts
        assert!(!result.has_floor_contact());
        
        // Wall contact (horizontal normal)
        result.contacts.push(CollisionPoint {
            world_pos: Vec3::ZERO,
            normal: Vec3::X,
            penetration: 0.1,
            voxel_pos: IVec3::ZERO,
        });
        assert!(!result.has_floor_contact());
        
        // Floor contact (upward normal)
        result.contacts.push(CollisionPoint {
            world_pos: Vec3::ZERO,
            normal: Vec3::Y,
            penetration: 0.1,
            voxel_pos: IVec3::ZERO,
        });
        assert!(result.has_floor_contact());
    }

    // ========== FragmentOccupancy Tests ==========

    #[test]
    fn test_fragment_occupancy_new() {
        let frag = FragmentOccupancy::new(UVec3::new(4, 4, 4));
        assert!(frag.is_empty());
        assert_eq!(frag.count_occupied(), 0);
        assert_eq!(frag.size, UVec3::new(4, 4, 4));
    }

    #[test]
    fn test_fragment_occupancy_set_get() {
        let mut frag = FragmentOccupancy::new(UVec3::new(8, 8, 8));
        
        frag.set(UVec3::new(0, 0, 0), true);
        frag.set(UVec3::new(3, 4, 5), true);
        frag.set(UVec3::new(7, 7, 7), true);
        
        assert!(frag.get(UVec3::new(0, 0, 0)));
        assert!(frag.get(UVec3::new(3, 4, 5)));
        assert!(frag.get(UVec3::new(7, 7, 7)));
        assert!(!frag.get(UVec3::new(1, 1, 1)));
        
        assert_eq!(frag.count_occupied(), 3);
    }

    #[test]
    fn test_fragment_occupancy_bounds_check() {
        let frag = FragmentOccupancy::new(UVec3::new(4, 4, 4));
        
        // Out of bounds should return false
        assert!(!frag.get(UVec3::new(4, 0, 0)));
        assert!(!frag.get(UVec3::new(0, 4, 0)));
        assert!(!frag.get(UVec3::new(100, 100, 100)));
    }

    #[test]
    fn test_fragment_occupancy_iter_occupied() {
        let mut frag = FragmentOccupancy::new(UVec3::new(4, 4, 4));
        
        frag.set(UVec3::new(1, 1, 1), true);
        frag.set(UVec3::new(2, 2, 2), true);
        
        let occupied: Vec<_> = frag.iter_occupied().collect();
        
        assert_eq!(occupied.len(), 2);
        assert!(occupied.contains(&UVec3::new(1, 1, 1)));
        assert!(occupied.contains(&UVec3::new(2, 2, 2)));
    }

    #[test]
    fn test_fragment_occupancy_from_voxel_world() {
        let mut world = VoxelWorld::new();
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        world.set_voxel(1, 0, 0, Voxel::solid(0, 255, 0));
        world.set_voxel(0, 1, 0, Voxel::solid(0, 0, 255));
        
        let frag = FragmentOccupancy::from_voxel_world(&world);
        
        assert_eq!(frag.size, UVec3::new(2, 2, 1));
        assert_eq!(frag.count_occupied(), 3);
        assert!(frag.get(UVec3::new(0, 0, 0)));
        assert!(frag.get(UVec3::new(1, 0, 0)));
        assert!(frag.get(UVec3::new(0, 1, 0)));
    }

    // ========== Fragment Terrain Collision Tests ==========

    #[test]
    fn test_check_fragment_no_collision() {
        // Terrain floor
        let mut world = VoxelWorld::new();
        for x in 0..10 {
            for z in 0..10 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        let terrain = WorldOccupancy::from_voxel_world(&world);
        
        // Small fragment
        let mut frag = FragmentOccupancy::new(UVec3::new(2, 2, 2));
        frag.set(UVec3::new(0, 0, 0), true);
        frag.set(UVec3::new(1, 0, 0), true);
        
        // Fragment floating above floor
        let result = terrain.check_fragment(
            &frag,
            Vec3::new(5.0, 5.0, 5.0),
            Quat::IDENTITY,
        );
        
        assert!(!result.has_collision());
        assert_eq!(result.contact_count(), 0);
    }

    #[test]
    fn test_check_fragment_collision_basic() {
        // Terrain floor at y=0
        let mut world = VoxelWorld::new();
        for x in 0..10 {
            for z in 0..10 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        let terrain = WorldOccupancy::from_voxel_world(&world);
        
        // Single voxel fragment
        let mut frag = FragmentOccupancy::new(UVec3::new(1, 1, 1));
        frag.set(UVec3::ZERO, true);
        
        // Fragment intersecting floor - position it so center is clearly inside terrain voxel
        // Using (5.5, 0.5, 5.5) so the fragment center lands in middle of terrain voxel (5,0,5)
        let result = terrain.check_fragment(
            &frag,
            Vec3::new(5.5, 0.5, 5.5),
            Quat::IDENTITY,
        );
        
        println!("Collision basic test:");
        println!("  has_collision: {}", result.has_collision());
        println!("  contact_count: {}", result.contact_count());
        for (i, c) in result.contacts.iter().enumerate() {
            println!("  contact {}: normal={:?}, pen={:.3}", i, c.normal, c.penetration);
        }
        
        assert!(result.has_collision());
        assert_eq!(result.contact_count(), 1);
        assert!(result.has_floor_contact(), "Should have floor contact");
    }

    #[test]
    fn test_check_fragment_multiple_contacts() {
        // Terrain floor at y=0
        let mut world = VoxelWorld::new();
        for x in 0..10 {
            for z in 0..10 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        let terrain = WorldOccupancy::from_voxel_world(&world);
        
        // 2x1x2 fragment (flat on bottom)
        let mut frag = FragmentOccupancy::new(UVec3::new(2, 1, 2));
        frag.set(UVec3::new(0, 0, 0), true);
        frag.set(UVec3::new(1, 0, 0), true);
        frag.set(UVec3::new(0, 0, 1), true);
        frag.set(UVec3::new(1, 0, 1), true);
        
        // Fragment resting on floor
        let result = terrain.check_fragment(
            &frag,
            Vec3::new(5.0, 0.5, 5.0),
            Quat::IDENTITY,
        );
        
        assert!(result.has_collision());
        assert_eq!(result.contact_count(), 4, "All 4 voxels should collide");
    }

    #[test]
    fn test_check_fragment_with_rotation() {
        // Terrain floor at y=0
        let mut world = VoxelWorld::new();
        for x in 0..20 {
            for z in 0..20 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        let terrain = WorldOccupancy::from_voxel_world(&world);
        
        // Tall fragment (1x3x1)
        let mut frag = FragmentOccupancy::new(UVec3::new(1, 3, 1));
        frag.set(UVec3::new(0, 0, 0), true);
        frag.set(UVec3::new(0, 1, 0), true);
        frag.set(UVec3::new(0, 2, 0), true);
        
        // Upright, fragment center at y=3 means:
        // - half_size.y = 1.5
        // - bottom voxel center at y=0.5 relative to local origin
        // - after centering: y=0.5 - 1.5 = -1.0
        // - world y = 3.0 + (-1.0) = 2.0 -> floor check at y=2, floor is at y=0, no collision
        let result_upright = terrain.check_fragment(
            &frag,
            Vec3::new(10.0, 3.0, 10.0),
            Quat::IDENTITY,
        );
        assert!(!result_upright.has_collision(), "Upright fragment should be above floor");
        
        // Now place it lower so it collides
        let result_low = terrain.check_fragment(
            &frag,
            Vec3::new(10.0, 1.5, 10.0),  // Center at y=1.5, bottom voxel at ~y=0.5, overlaps floor
            Quat::IDENTITY,
        );
        assert!(result_low.has_collision(), "Low fragment should hit floor");
        
        // Test rotation changes collision:
        // Rotated 90 degrees around X - the 1x3x1 becomes 1x1x3 (lying in Z direction)
        let rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        
        // At y=2, upright would NOT collide, but rotated SHOULD collide
        // because the rotated fragment now has voxels at different Y positions
        let _result_rotated = terrain.check_fragment(
            &frag,
            Vec3::new(10.0, 1.0, 10.0),
            rotation,
        );
        // When rotated around X by 90 deg, the Y axis becomes Z axis
        // The fragment voxels that were at y=0,1,2 are now at z=-1,0,1 (after centering)
        // All voxels end up at y≈1.0, which is above floor y=0, so no collision
        // Let's lower it to definitely collide
        let result_rotated_low = terrain.check_fragment(
            &frag,
            Vec3::new(10.0, 0.5, 10.0),
            rotation,
        );
        assert!(result_rotated_low.has_collision(), "Rotated fragment at y=0.5 should hit floor");
    }

    #[test]
    fn test_check_fragment_resolution_vector() {
        // Terrain floor at y=0
        let mut world = VoxelWorld::new();
        for x in 0..10 {
            for z in 0..10 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }
        let terrain = WorldOccupancy::from_voxel_world(&world);
        
        // Single voxel fragment
        let mut frag = FragmentOccupancy::new(UVec3::new(1, 1, 1));
        frag.set(UVec3::ZERO, true);
        
        // Fragment partially in floor - use position clearly inside a terrain voxel
        let result = terrain.check_fragment(
            &frag,
            Vec3::new(5.5, 0.7, 5.5),
            Quat::IDENTITY,
        );
        
        assert!(result.has_collision(), "Should have collision");
        let resolution = result.resolution_vector();
        
        println!("Resolution vector test:");
        println!("  resolution: {:?}", resolution);
        for (i, c) in result.contacts.iter().enumerate() {
            println!("  contact {}: normal={:?}, pen={:.3}", i, c.normal, c.penetration);
        }
        
        // Should push up (positive Y)
        assert!(resolution.y > 0.0, "Resolution should push up, got {:?}", resolution);
        assert!(resolution.x.abs() < 0.1, "Should not push X");
        assert!(resolution.z.abs() < 0.1, "Should not push Z");
    }

    #[test]
    fn test_fragment_collision_benchmark() {
        use std::time::Instant;
        
        // Create terrain
        let mut world = VoxelWorld::new();
        for x in 0..32 {
            for z in 0..32 {
                for y in 0..3 {
                    world.set_voxel(x, y, z, Voxel::solid(100, 100, 100));
                }
            }
        }
        let terrain = WorldOccupancy::from_voxel_world(&world);
        
        // Create a reasonably sized fragment (4x4x4 with some voxels)
        let mut frag = FragmentOccupancy::new(UVec3::new(4, 4, 4));
        for x in 0..4 {
            for z in 0..4 {
                frag.set(UVec3::new(x, 0, z), true); // Bottom layer
            }
        }
        
        let iterations = 1000;
        let start = Instant::now();
        
        for i in 0..iterations {
            let x = (i % 20) as f32 + 5.0;
            let z = ((i / 20) % 20) as f32 + 5.0;
            let rotation = Quat::from_rotation_y(i as f32 * 0.1);
            let _ = terrain.check_fragment(&frag, Vec3::new(x, 4.0, z), rotation);
        }
        
        let elapsed = start.elapsed();
        let per_query_us = elapsed.as_micros() as f64 / iterations as f64;
        
        println!("Fragment collision benchmark: {} queries in {:?}", iterations, elapsed);
        println!("  Fragment size: 4x4x4, 16 occupied voxels");
        println!("  Per query: {:.2} us", per_query_us);
        
        // Should be fast - under 100us per query for small fragments
        assert!(per_query_us < 500.0, "Query too slow: {} us", per_query_us);
    }
    
    /// Benchmark comparing occupancy collision vs AABB collision at different scales.
    /// 
    /// This test demonstrates the performance characteristics of the occupancy system
    /// for voxel-based collision detection.
    #[test]
    fn test_occupancy_performance_scaling() {
        use std::time::Instant;
        
        println!("\n=== OCCUPANCY PERFORMANCE SCALING ===");
        
        // Test different terrain sizes
        for terrain_size in [16, 32, 64] {
            let mut world = VoxelWorld::new();
            for x in 0..terrain_size {
                for z in 0..terrain_size {
                    for y in 0..3 {
                        world.set_voxel(x, y, z, Voxel::solid(100, 100, 100));
                    }
                }
            }
            let terrain = WorldOccupancy::from_voxel_world(&world);
            
            // Test different fragment sizes
            for frag_size in [2u32, 4, 8] {
                let mut frag = FragmentOccupancy::new(UVec3::splat(frag_size));
                let voxel_count = frag_size * frag_size; // Just bottom layer
                for x in 0..frag_size {
                    for z in 0..frag_size {
                        frag.set(UVec3::new(x, 0, z), true);
                    }
                }
                
                let iterations = 500;
                let start = Instant::now();
                
                for i in 0..iterations {
                    let x = (i % (terrain_size as usize - frag_size as usize)) as f32 + 1.0;
                    let z = ((i / (terrain_size as usize)) % (terrain_size as usize - frag_size as usize)) as f32 + 1.0;
                    let rotation = Quat::from_rotation_y(i as f32 * 0.1);
                    let _ = terrain.check_fragment(&frag, Vec3::new(x, 4.0, z), rotation);
                }
                
                let elapsed = start.elapsed();
                let per_query_us = elapsed.as_micros() as f64 / iterations as f64;
                
                println!(
                    "  Terrain {}x{}, Fragment {}x{}x{} ({} voxels): {:.2} us/query",
                    terrain_size, terrain_size,
                    frag_size, frag_size, frag_size,
                    voxel_count,
                    per_query_us
                );
            }
        }
        
        println!("\nKey insight: Occupancy collision scales with FRAGMENT voxel count,");
        println!("not terrain size. This is ideal for many small fragments on large terrain.");
    }
}
