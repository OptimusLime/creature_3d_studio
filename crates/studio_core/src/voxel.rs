//! Voxel data structures for creature modeling.
//!
//! A voxel represents a single unit cube in 3D space with color and emission properties.
//! VoxelChunk stores a CHUNK_SIZE³ dense array of optional voxels.
//! VoxelWorld stores multiple chunks in a HashMap for larger scenes.
//!
//! ## Coordinate Systems
//!
//! - **World Position** (`IVec3`): Global voxel coordinates, can be negative
//! - **Chunk Position** (`ChunkPos`): Which chunk contains the voxel (world / CHUNK_SIZE)
//! - **Local Position** (`usize, usize, usize`): Position within chunk (0 to CHUNK_SIZE-1)

use std::collections::HashMap;

/// Size of a voxel chunk in each dimension.
/// 32 allows for reasonably sized test scenes.
pub const CHUNK_SIZE: usize = 32;

/// Signed chunk size for coordinate math.
pub const CHUNK_SIZE_I32: i32 = CHUNK_SIZE as i32;

/// A single voxel with color and emission.
///
/// Emission is stored as a u8 (0-255) where 0 means no emission
/// and 255 means full emission. Emissive voxels will bypass normal
/// lighting in the deferred renderer (like Bonsai).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voxel {
    /// RGB color components (0-255 each)
    pub color: [u8; 3],
    /// Emission intensity (0 = no glow, 255 = full glow)
    pub emission: u8,
}

impl Voxel {
    /// Create a new voxel with the given color and emission.
    pub fn new(r: u8, g: u8, b: u8, emission: u8) -> Self {
        Self {
            color: [r, g, b],
            emission,
        }
    }

    /// Create a non-emissive voxel with the given color.
    pub fn solid(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 0)
    }

    /// Create an emissive voxel (full emission by default).
    pub fn emissive(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// Get the color as normalized floats [0.0, 1.0].
    pub fn color_f32(&self) -> [f32; 3] {
        [
            self.color[0] as f32 / 255.0,
            self.color[1] as f32 / 255.0,
            self.color[2] as f32 / 255.0,
        ]
    }

    /// Get emission as normalized float [0.0, 1.0].
    pub fn emission_f32(&self) -> f32 {
        self.emission as f32 / 255.0
    }
}

/// A 16³ chunk of voxels.
///
/// Uses a dense array with Option<Voxel> for each cell.
/// Empty cells are None, filled cells contain the voxel data.
#[derive(Debug, Clone)]
pub struct VoxelChunk {
    /// Dense storage: index = x + y * CHUNK_SIZE + z * CHUNK_SIZE²
    voxels: Box<[Option<Voxel>; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE]>,
}

impl Default for VoxelChunk {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxelChunk {
    /// Create an empty chunk (all cells are None).
    pub fn new() -> Self {
        // Box the array to avoid stack overflow (16³ * size_of::<Option<Voxel>> = 16KB+)
        Self {
            voxels: Box::new([None; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE]),
        }
    }

    /// Convert (x, y, z) to linear index.
    /// Returns None if coordinates are out of bounds.
    fn index(x: usize, y: usize, z: usize) -> Option<usize> {
        if x < CHUNK_SIZE && y < CHUNK_SIZE && z < CHUNK_SIZE {
            Some(x + y * CHUNK_SIZE + z * CHUNK_SIZE * CHUNK_SIZE)
        } else {
            None
        }
    }

    /// Get the voxel at (x, y, z), or None if empty or out of bounds.
    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<Voxel> {
        Self::index(x, y, z).and_then(|i| self.voxels[i])
    }

    /// Set a voxel at (x, y, z). Returns false if out of bounds.
    pub fn set(&mut self, x: usize, y: usize, z: usize, voxel: Voxel) -> bool {
        if let Some(i) = Self::index(x, y, z) {
            self.voxels[i] = Some(voxel);
            true
        } else {
            false
        }
    }

    /// Clear a voxel at (x, y, z). Returns false if out of bounds.
    pub fn clear(&mut self, x: usize, y: usize, z: usize) -> bool {
        if let Some(i) = Self::index(x, y, z) {
            self.voxels[i] = None;
            true
        } else {
            false
        }
    }

    /// Iterate over all filled voxels with their coordinates.
    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, usize, Voxel)> + '_ {
        self.voxels.iter().enumerate().filter_map(|(i, v)| {
            v.map(|voxel| {
                let x = i % CHUNK_SIZE;
                let y = (i / CHUNK_SIZE) % CHUNK_SIZE;
                let z = i / (CHUNK_SIZE * CHUNK_SIZE);
                (x, y, z, voxel)
            })
        })
    }

    /// Count of filled voxels.
    pub fn count(&self) -> usize {
        self.voxels.iter().filter(|v| v.is_some()).count()
    }

    /// Check if chunk is empty.
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Check if a voxel exists at the given position (with signed coordinates).
    /// Returns true if there's a solid voxel, false if empty or out of bounds.
    /// This is useful for AO calculations where we need to check neighbors
    /// that might be outside the chunk bounds.
    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        if x < 0 || y < 0 || z < 0 {
            return false;
        }
        self.get(x as usize, y as usize, z as usize).is_some()
    }

    /// Check if a neighbor at offset (dx, dy, dz) from (x, y, z) is solid.
    /// Returns true if solid, false if empty or out of bounds.
    pub fn is_neighbor_solid(&self, x: usize, y: usize, z: usize, dx: i32, dy: i32, dz: i32) -> bool {
        self.is_solid(x as i32 + dx, y as i32 + dy, z as i32 + dz)
    }

    /// Iterate over emissive voxels (voxels with emission > threshold).
    /// Returns (x, y, z, voxel) for each emissive voxel.
    pub fn iter_emissive(&self, min_emission: u8) -> impl Iterator<Item = (usize, usize, usize, Voxel)> + '_ {
        self.iter().filter(move |(_, _, _, v)| v.emission >= min_emission)
    }
}

/// Data for an emissive voxel that can become a point light.
#[derive(Debug, Clone)]
pub struct EmissiveLight {
    /// Position of the emissive voxel (in chunk coordinates)
    pub position: (usize, usize, usize),
    /// RGB color (0.0-1.0)
    pub color: [f32; 3],
    /// Emission intensity (0.0-1.0)
    pub emission: f32,
}

impl EmissiveLight {
    /// Get mesh-space position (matches build_chunk_mesh coordinate system).
    /// The position is at the center of the voxel.
    /// This applies the same centering offset as build_chunk_mesh() so lights
    /// align with the mesh geometry.
    pub fn mesh_position(&self) -> [f32; 3] {
        let offset = CHUNK_SIZE as f32 / 2.0;
        [
            self.position.0 as f32 + 0.5 - offset,
            self.position.1 as f32 + 0.5 - offset,
            self.position.2 as f32 + 0.5 - offset,
        ]
    }
}

/// Extract emissive voxels from a chunk as potential point light sources.
/// 
/// This function finds all voxels with emission above the threshold and
/// groups adjacent emissive voxels into single lights (to avoid many
/// overlapping lights from a cluster of emissive voxels).
/// 
/// # Arguments
/// * `chunk` - The voxel chunk to scan
/// * `min_emission` - Minimum emission value to consider (0-255, typically 100+)
/// 
/// # Returns
/// A list of emissive light sources with position, color, and intensity.
pub fn extract_emissive_lights(chunk: &VoxelChunk, min_emission: u8) -> Vec<EmissiveLight> {
    // Simple approach: collect all emissive voxels
    // A more advanced approach could cluster adjacent voxels
    chunk
        .iter_emissive(min_emission)
        .map(|(x, y, z, voxel)| EmissiveLight {
            position: (x, y, z),
            color: voxel.color_f32(),
            emission: voxel.emission_f32(),
        })
        .collect()
}

/// Extract emissive lights and cluster adjacent voxels into single lights.
/// 
/// This reduces the number of point lights by merging adjacent emissive voxels
/// of the same color into a single light at their centroid.
/// 
/// # Arguments
/// * `chunk` - The voxel chunk to scan
/// * `min_emission` - Minimum emission value (0-255)
/// * `color_tolerance` - How similar colors must be to cluster (0.0-1.0, typically 0.1)
/// 
/// # Returns
/// Clustered emissive lights.
pub fn extract_clustered_emissive_lights(
    chunk: &VoxelChunk,
    min_emission: u8,
    color_tolerance: f32,
) -> Vec<EmissiveLight> {
    let emissive: Vec<_> = chunk.iter_emissive(min_emission).collect();
    
    if emissive.is_empty() {
        return Vec::new();
    }
    
    // Track which voxels have been assigned to a cluster
    let mut assigned = vec![false; emissive.len()];
    let mut clusters: Vec<EmissiveLight> = Vec::new();
    
    for i in 0..emissive.len() {
        if assigned[i] {
            continue;
        }
        
        let (x, y, z, base_voxel) = emissive[i];
        let base_color = base_voxel.color_f32();
        
        // Start a new cluster
        let mut cluster_positions: Vec<(usize, usize, usize)> = vec![(x, y, z)];
        let mut cluster_emission = base_voxel.emission_f32();
        assigned[i] = true;
        
        // Find adjacent voxels with similar color
        for j in (i + 1)..emissive.len() {
            if assigned[j] {
                continue;
            }
            
            let (ox, oy, oz, other_voxel) = emissive[j];
            let other_color = other_voxel.color_f32();
            
            // Check if colors are similar
            let color_diff = (base_color[0] - other_color[0]).abs()
                + (base_color[1] - other_color[1]).abs()
                + (base_color[2] - other_color[2]).abs();
            
            if color_diff > color_tolerance * 3.0 {
                continue;
            }
            
            // Check if adjacent to any voxel in the cluster
            let mut is_adjacent = false;
            for &(cx, cy, cz) in &cluster_positions {
                let dx = (ox as i32 - cx as i32).abs();
                let dy = (oy as i32 - cy as i32).abs();
                let dz = (oz as i32 - cz as i32).abs();
                
                // Consider adjacent if Manhattan distance <= 2
                if dx + dy + dz <= 2 {
                    is_adjacent = true;
                    break;
                }
            }
            
            if is_adjacent {
                cluster_positions.push((ox, oy, oz));
                cluster_emission = cluster_emission.max(other_voxel.emission_f32());
                assigned[j] = true;
            }
        }
        
        // Calculate centroid of cluster
        let count = cluster_positions.len() as f32;
        let centroid_x = cluster_positions.iter().map(|p| p.0).sum::<usize>() as f32 / count;
        let centroid_y = cluster_positions.iter().map(|p| p.1).sum::<usize>() as f32 / count;
        let centroid_z = cluster_positions.iter().map(|p| p.2).sum::<usize>() as f32 / count;
        
        clusters.push(EmissiveLight {
            position: (centroid_x as usize, centroid_y as usize, centroid_z as usize),
            color: base_color,
            emission: cluster_emission,
        });
    }
    
    clusters
}

// ============================================================================
// Multi-Chunk World Support
// ============================================================================

/// Position of a chunk in chunk-space coordinates.
///
/// Chunk coordinates are world coordinates divided by CHUNK_SIZE (floor division).
/// For example, with CHUNK_SIZE=32:
/// - World (0, 0, 0) to (31, 31, 31) → ChunkPos(0, 0, 0)
/// - World (32, 0, 0) to (63, 31, 31) → ChunkPos(1, 0, 0)
/// - World (-1, 0, 0) → ChunkPos(-1, 0, 0)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ChunkPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkPos {
    /// Create a new chunk position.
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Get chunk position from world coordinates.
    ///
    /// Uses floor division so negative coordinates work correctly:
    /// - World 31 → Chunk 0
    /// - World 32 → Chunk 1
    /// - World -1 → Chunk -1
    /// - World -32 → Chunk -1
    /// - World -33 → Chunk -2
    pub fn from_world(world_x: i32, world_y: i32, world_z: i32) -> Self {
        Self {
            x: world_x.div_euclid(CHUNK_SIZE_I32),
            y: world_y.div_euclid(CHUNK_SIZE_I32),
            z: world_z.div_euclid(CHUNK_SIZE_I32),
        }
    }

    /// Get the world-space origin of this chunk (minimum corner).
    pub fn world_origin(&self) -> (i32, i32, i32) {
        (
            self.x * CHUNK_SIZE_I32,
            self.y * CHUNK_SIZE_I32,
            self.z * CHUNK_SIZE_I32,
        )
    }

    /// Get the world-space center of this chunk.
    pub fn world_center(&self) -> (f32, f32, f32) {
        let half = CHUNK_SIZE as f32 / 2.0;
        (
            self.x as f32 * CHUNK_SIZE as f32 + half,
            self.y as f32 * CHUNK_SIZE as f32 + half,
            self.z as f32 * CHUNK_SIZE as f32 + half,
        )
    }

    /// Iterator over all positions in a 3D range (inclusive).
    pub fn iter_range(min: ChunkPos, max: ChunkPos) -> impl Iterator<Item = ChunkPos> {
        let x_range = min.x..=max.x;
        let y_range = min.y..=max.y;
        let z_range = min.z..=max.z;

        x_range.flat_map(move |x| {
            let y_range = y_range.clone();
            let z_range = z_range.clone();
            y_range.flat_map(move |y| {
                let z_range = z_range.clone();
                z_range.map(move |z| ChunkPos::new(x, y, z))
            })
        })
    }
}

impl std::ops::Add for ChunkPos {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl std::ops::Sub for ChunkPos {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl From<(i32, i32, i32)> for ChunkPos {
    fn from((x, y, z): (i32, i32, i32)) -> Self {
        Self::new(x, y, z)
    }
}

/// Convert world position to local chunk position (0 to CHUNK_SIZE-1).
///
/// Uses Euclidean remainder so negative coordinates work correctly.
pub fn world_to_local(world_x: i32, world_y: i32, world_z: i32) -> (usize, usize, usize) {
    (
        world_x.rem_euclid(CHUNK_SIZE_I32) as usize,
        world_y.rem_euclid(CHUNK_SIZE_I32) as usize,
        world_z.rem_euclid(CHUNK_SIZE_I32) as usize,
    )
}

/// A world containing multiple voxel chunks.
///
/// Chunks are stored in a HashMap keyed by their chunk position.
/// This allows for sparse worlds where only populated areas consume memory.
///
/// # Example
///
/// ```
/// use studio_core::voxel::{VoxelWorld, Voxel};
///
/// let mut world = VoxelWorld::new();
///
/// // Set voxels at world coordinates
/// world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
/// world.set_voxel(100, 50, -30, Voxel::solid(0, 255, 0));
///
/// // Get voxels back
/// assert!(world.get_voxel(0, 0, 0).is_some());
/// ```
#[derive(Debug, Clone, Default)]
pub struct VoxelWorld {
    chunks: HashMap<ChunkPos, VoxelChunk>,
}

impl VoxelWorld {
    /// Create an empty world.
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }

    /// Get chunk at position, if it exists.
    pub fn get_chunk(&self, pos: ChunkPos) -> Option<&VoxelChunk> {
        self.chunks.get(&pos)
    }

    /// Get mutable chunk at position, if it exists.
    pub fn get_chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut VoxelChunk> {
        self.chunks.get_mut(&pos)
    }

    /// Get or create chunk at position.
    pub fn get_or_create_chunk(&mut self, pos: ChunkPos) -> &mut VoxelChunk {
        self.chunks.entry(pos).or_insert_with(VoxelChunk::new)
    }

    /// Insert a chunk at position, replacing any existing chunk.
    pub fn insert_chunk(&mut self, pos: ChunkPos, chunk: VoxelChunk) {
        self.chunks.insert(pos, chunk);
    }

    /// Remove chunk at position.
    pub fn remove_chunk(&mut self, pos: ChunkPos) -> Option<VoxelChunk> {
        self.chunks.remove(&pos)
    }

    /// Check if a chunk exists at position.
    pub fn has_chunk(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }

    /// Get the number of chunks in the world.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Iterate over all chunks with their positions.
    pub fn iter_chunks(&self) -> impl Iterator<Item = (ChunkPos, &VoxelChunk)> {
        self.chunks.iter().map(|(pos, chunk)| (*pos, chunk))
    }

    /// Iterate over all chunk positions.
    pub fn chunk_positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.chunks.keys().copied()
    }

    /// Set a voxel at world coordinates.
    ///
    /// Creates the chunk if it doesn't exist.
    pub fn set_voxel(&mut self, world_x: i32, world_y: i32, world_z: i32, voxel: Voxel) {
        let chunk_pos = ChunkPos::from_world(world_x, world_y, world_z);
        let (local_x, local_y, local_z) = world_to_local(world_x, world_y, world_z);

        let chunk = self.get_or_create_chunk(chunk_pos);
        chunk.set(local_x, local_y, local_z, voxel);
    }

    /// Get a voxel at world coordinates.
    ///
    /// Returns None if the chunk doesn't exist or the voxel is empty.
    pub fn get_voxel(&self, world_x: i32, world_y: i32, world_z: i32) -> Option<Voxel> {
        let chunk_pos = ChunkPos::from_world(world_x, world_y, world_z);
        let (local_x, local_y, local_z) = world_to_local(world_x, world_y, world_z);

        self.chunks
            .get(&chunk_pos)
            .and_then(|chunk| chunk.get(local_x, local_y, local_z))
    }

    /// Clear a voxel at world coordinates.
    ///
    /// Does nothing if the chunk doesn't exist.
    pub fn clear_voxel(&mut self, world_x: i32, world_y: i32, world_z: i32) {
        let chunk_pos = ChunkPos::from_world(world_x, world_y, world_z);
        let (local_x, local_y, local_z) = world_to_local(world_x, world_y, world_z);

        if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
            chunk.clear(local_x, local_y, local_z);
        }
    }

    /// Check if a voxel exists at world coordinates.
    pub fn is_solid(&self, world_x: i32, world_y: i32, world_z: i32) -> bool {
        self.get_voxel(world_x, world_y, world_z).is_some()
    }

    /// Total count of voxels across all chunks.
    pub fn total_voxel_count(&self) -> usize {
        self.chunks.values().map(|c| c.count()).sum()
    }

    /// Remove empty chunks (chunks with no voxels).
    pub fn prune_empty_chunks(&mut self) {
        self.chunks.retain(|_, chunk| !chunk.is_empty());
    }

    /// Get the bounding box of all chunks (in chunk coordinates).
    ///
    /// Returns None if the world is empty.
    pub fn chunk_bounds(&self) -> Option<(ChunkPos, ChunkPos)> {
        let mut positions = self.chunks.keys();
        let first = positions.next()?;

        let mut min = *first;
        let mut max = *first;

        for pos in positions {
            min.x = min.x.min(pos.x);
            min.y = min.y.min(pos.y);
            min.z = min.z.min(pos.z);
            max.x = max.x.max(pos.x);
            max.y = max.y.max(pos.y);
            max.z = max.z.max(pos.z);
        }

        Some((min, max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voxel_creation() {
        let v = Voxel::new(255, 128, 64, 200);
        assert_eq!(v.color, [255, 128, 64]);
        assert_eq!(v.emission, 200);
    }

    #[test]
    fn test_voxel_solid() {
        let v = Voxel::solid(100, 150, 200);
        assert_eq!(v.emission, 0);
    }

    #[test]
    fn test_voxel_emissive() {
        let v = Voxel::emissive(255, 0, 255);
        assert_eq!(v.emission, 255);
    }

    #[test]
    fn test_voxel_color_f32() {
        let v = Voxel::solid(255, 127, 0);
        let c = v.color_f32();
        assert!((c[0] - 1.0).abs() < 0.001);
        assert!((c[1] - 0.498).abs() < 0.01);
        assert!((c[2] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_chunk_new_is_empty() {
        let chunk = VoxelChunk::new();
        assert!(chunk.is_empty());
        assert_eq!(chunk.count(), 0);
    }

    #[test]
    fn test_chunk_set_get() {
        let mut chunk = VoxelChunk::new();
        let voxel = Voxel::solid(255, 0, 0);

        assert!(chunk.set(5, 10, 3, voxel));
        assert_eq!(chunk.get(5, 10, 3), Some(voxel));
        assert_eq!(chunk.count(), 1);
    }

    #[test]
    fn test_chunk_out_of_bounds() {
        let mut chunk = VoxelChunk::new();
        let voxel = Voxel::solid(255, 0, 0);

        // CHUNK_SIZE is 32, so index 32 is out of bounds
        assert!(!chunk.set(CHUNK_SIZE, 0, 0, voxel)); // x out of bounds
        assert!(!chunk.set(0, CHUNK_SIZE, 0, voxel)); // y out of bounds
        assert!(!chunk.set(0, 0, CHUNK_SIZE, voxel)); // z out of bounds
        assert_eq!(chunk.get(CHUNK_SIZE, 0, 0), None);
    }

    #[test]
    fn test_chunk_clear() {
        let mut chunk = VoxelChunk::new();
        chunk.set(1, 2, 3, Voxel::solid(255, 0, 0));
        assert_eq!(chunk.count(), 1);

        assert!(chunk.clear(1, 2, 3));
        assert_eq!(chunk.count(), 0);
        assert_eq!(chunk.get(1, 2, 3), None);
    }

    #[test]
    fn test_chunk_iter() {
        let mut chunk = VoxelChunk::new();
        chunk.set(0, 0, 0, Voxel::solid(255, 0, 0));
        chunk.set(1, 0, 0, Voxel::solid(0, 255, 0));
        chunk.set(0, 1, 0, Voxel::solid(0, 0, 255));

        let voxels: Vec<_> = chunk.iter().collect();
        assert_eq!(voxels.len(), 3);
    }

    // ========================================================================
    // ChunkPos tests
    // ========================================================================

    #[test]
    fn test_chunk_pos_from_world_positive() {
        // World (0,0,0) to (31,31,31) → Chunk (0,0,0)
        assert_eq!(ChunkPos::from_world(0, 0, 0), ChunkPos::new(0, 0, 0));
        assert_eq!(ChunkPos::from_world(31, 31, 31), ChunkPos::new(0, 0, 0));

        // World (32,0,0) → Chunk (1,0,0)
        assert_eq!(ChunkPos::from_world(32, 0, 0), ChunkPos::new(1, 0, 0));
        assert_eq!(ChunkPos::from_world(63, 0, 0), ChunkPos::new(1, 0, 0));
        assert_eq!(ChunkPos::from_world(64, 0, 0), ChunkPos::new(2, 0, 0));
    }

    #[test]
    fn test_chunk_pos_from_world_negative() {
        // World (-1,0,0) → Chunk (-1,0,0) (floor division)
        assert_eq!(ChunkPos::from_world(-1, 0, 0), ChunkPos::new(-1, 0, 0));
        assert_eq!(ChunkPos::from_world(-32, 0, 0), ChunkPos::new(-1, 0, 0));
        assert_eq!(ChunkPos::from_world(-33, 0, 0), ChunkPos::new(-2, 0, 0));
    }

    #[test]
    fn test_chunk_pos_world_origin() {
        assert_eq!(ChunkPos::new(0, 0, 0).world_origin(), (0, 0, 0));
        assert_eq!(ChunkPos::new(1, 0, 0).world_origin(), (32, 0, 0));
        assert_eq!(ChunkPos::new(-1, 0, 0).world_origin(), (-32, 0, 0));
        assert_eq!(ChunkPos::new(1, 2, 3).world_origin(), (32, 64, 96));
    }

    #[test]
    fn test_chunk_pos_arithmetic() {
        let a = ChunkPos::new(1, 2, 3);
        let b = ChunkPos::new(4, 5, 6);

        assert_eq!(a + b, ChunkPos::new(5, 7, 9));
        assert_eq!(b - a, ChunkPos::new(3, 3, 3));
    }

    #[test]
    fn test_chunk_pos_from_tuple() {
        let pos: ChunkPos = (1, 2, 3).into();
        assert_eq!(pos, ChunkPos::new(1, 2, 3));
    }

    #[test]
    fn test_world_to_local_positive() {
        // World (0,0,0) → Local (0,0,0)
        assert_eq!(world_to_local(0, 0, 0), (0, 0, 0));

        // World (31,31,31) → Local (31,31,31)
        assert_eq!(world_to_local(31, 31, 31), (31, 31, 31));

        // World (32,0,0) → Local (0,0,0) in chunk (1,0,0)
        assert_eq!(world_to_local(32, 0, 0), (0, 0, 0));

        // World (45,10,67) → Local (13,10,3)
        assert_eq!(world_to_local(45, 10, 67), (13, 10, 3));
    }

    #[test]
    fn test_world_to_local_negative() {
        // World (-1,0,0) → Local (31,0,0) in chunk (-1,0,0)
        assert_eq!(world_to_local(-1, 0, 0), (31, 0, 0));

        // World (-32,0,0) → Local (0,0,0) in chunk (-1,0,0)
        assert_eq!(world_to_local(-32, 0, 0), (0, 0, 0));

        // World (-33,0,0) → Local (31,0,0) in chunk (-2,0,0)
        assert_eq!(world_to_local(-33, 0, 0), (31, 0, 0));
    }

    // ========================================================================
    // VoxelWorld tests
    // ========================================================================

    #[test]
    fn test_world_new_is_empty() {
        let world = VoxelWorld::new();
        assert_eq!(world.chunk_count(), 0);
        assert_eq!(world.total_voxel_count(), 0);
    }

    #[test]
    fn test_world_set_get_voxel() {
        let mut world = VoxelWorld::new();
        let voxel = Voxel::solid(255, 0, 0);

        world.set_voxel(10, 20, 30, voxel);
        assert_eq!(world.get_voxel(10, 20, 30), Some(voxel));
        assert_eq!(world.chunk_count(), 1);
        assert_eq!(world.total_voxel_count(), 1);
    }

    #[test]
    fn test_world_set_voxel_creates_chunk() {
        let mut world = VoxelWorld::new();

        // Setting a voxel should create the chunk
        assert!(!world.has_chunk(ChunkPos::new(0, 0, 0)));
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        assert!(world.has_chunk(ChunkPos::new(0, 0, 0)));
    }

    #[test]
    fn test_world_multiple_chunks() {
        let mut world = VoxelWorld::new();

        // Voxels in different chunks
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0)); // Chunk (0,0,0)
        world.set_voxel(32, 0, 0, Voxel::solid(0, 255, 0)); // Chunk (1,0,0)
        world.set_voxel(-1, 0, 0, Voxel::solid(0, 0, 255)); // Chunk (-1,0,0)

        assert_eq!(world.chunk_count(), 3);
        assert_eq!(world.total_voxel_count(), 3);

        // Verify each voxel
        assert_eq!(world.get_voxel(0, 0, 0), Some(Voxel::solid(255, 0, 0)));
        assert_eq!(world.get_voxel(32, 0, 0), Some(Voxel::solid(0, 255, 0)));
        assert_eq!(world.get_voxel(-1, 0, 0), Some(Voxel::solid(0, 0, 255)));
    }

    #[test]
    fn test_world_clear_voxel() {
        let mut world = VoxelWorld::new();

        world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
        assert!(world.is_solid(5, 5, 5));

        world.clear_voxel(5, 5, 5);
        assert!(!world.is_solid(5, 5, 5));
    }

    #[test]
    fn test_world_chunk_bounds() {
        let mut world = VoxelWorld::new();

        // Empty world has no bounds
        assert_eq!(world.chunk_bounds(), None);

        // Single chunk
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        assert_eq!(
            world.chunk_bounds(),
            Some((ChunkPos::new(0, 0, 0), ChunkPos::new(0, 0, 0)))
        );

        // Multiple chunks
        world.set_voxel(64, 32, -32, Voxel::solid(0, 255, 0));
        assert_eq!(
            world.chunk_bounds(),
            Some((ChunkPos::new(0, 0, -1), ChunkPos::new(2, 1, 0)))
        );
    }

    #[test]
    fn test_world_prune_empty_chunks() {
        let mut world = VoxelWorld::new();

        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        world.set_voxel(32, 0, 0, Voxel::solid(0, 255, 0));
        assert_eq!(world.chunk_count(), 2);

        // Clear all voxels in one chunk
        world.clear_voxel(32, 0, 0);

        // Chunk still exists but is empty
        assert_eq!(world.chunk_count(), 2);

        // Prune removes empty chunks
        world.prune_empty_chunks();
        assert_eq!(world.chunk_count(), 1);
    }

    #[test]
    fn test_chunk_pos_iter_range() {
        let min = ChunkPos::new(0, 0, 0);
        let max = ChunkPos::new(1, 1, 0);

        let positions: Vec<_> = ChunkPos::iter_range(min, max).collect();
        assert_eq!(positions.len(), 4); // 2x2x1 = 4 chunks
        assert!(positions.contains(&ChunkPos::new(0, 0, 0)));
        assert!(positions.contains(&ChunkPos::new(1, 0, 0)));
        assert!(positions.contains(&ChunkPos::new(0, 1, 0)));
        assert!(positions.contains(&ChunkPos::new(1, 1, 0)));
    }
}
