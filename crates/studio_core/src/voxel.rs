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

use bevy::prelude::{IVec3, Resource, Vec3};
use bevy::render::extract_resource::ExtractResource;
use serde::{Deserialize, Serialize};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// A 32³ chunk of voxels.
///
/// Uses a dense array with Option<Voxel> for each cell.
/// Empty cells are None, filled cells contain the voxel data.
#[derive(Debug, Clone)]
pub struct VoxelChunk {
    /// Dense storage: index = x + y * CHUNK_SIZE + z * CHUNK_SIZE²
    voxels: Box<[Option<Voxel>; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE]>,
}

// Custom serialization for VoxelChunk - only serialize non-empty voxels
impl Serialize for VoxelChunk {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as a list of (index, voxel) pairs for non-empty voxels
        let sparse: Vec<(usize, Voxel)> = self
            .voxels
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|voxel| (i, voxel)))
            .collect();
        sparse.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for VoxelChunk {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let sparse: Vec<(usize, Voxel)> = Vec::deserialize(deserializer)?;
        let mut chunk = VoxelChunk::new();
        for (i, voxel) in sparse {
            if i < CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE {
                chunk.voxels[i] = Some(voxel);
            }
        }
        Ok(chunk)
    }
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

    /// Get the bounding box of all voxels in this chunk (in local chunk coordinates).
    /// Returns None if chunk is empty.
    /// Returns Some((min, max)) where min/max are inclusive corners.
    pub fn bounds(&self) -> Option<((usize, usize, usize), (usize, usize, usize))> {
        let mut min_x = CHUNK_SIZE;
        let mut min_y = CHUNK_SIZE;
        let mut min_z = CHUNK_SIZE;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut max_z = 0usize;
        let mut found = false;

        for (x, y, z, _) in self.iter() {
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            min_z = min_z.min(z);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            max_z = max_z.max(z);
        }

        if found {
            Some(((min_x, min_y, min_z), (max_x, max_y, max_z)))
        } else {
            None
        }
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
    pub fn is_neighbor_solid(
        &self,
        x: usize,
        y: usize,
        z: usize,
        dx: i32,
        dy: i32,
        dz: i32,
    ) -> bool {
        self.is_solid(x as i32 + dx, y as i32 + dy, z as i32 + dz)
    }

    /// Iterate over emissive voxels (voxels with emission > threshold).
    /// Returns (x, y, z, voxel) for each emissive voxel.
    pub fn iter_emissive(
        &self,
        min_emission: u8,
    ) -> impl Iterator<Item = (usize, usize, usize, Voxel)> + '_ {
        self.iter()
            .filter(move |(_, _, _, v)| v.emission >= min_emission)
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
            position: (
                centroid_x as usize,
                centroid_y as usize,
                centroid_z as usize,
            ),
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
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
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

// Custom serialization for VoxelWorld - convert to Vec for JSON compatibility
impl Serialize for VoxelWorld {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as list of (pos, chunk) pairs for JSON compatibility
        let chunks: Vec<(ChunkPos, &VoxelChunk)> = self
            .chunks
            .iter()
            .map(|(pos, chunk)| (*pos, chunk))
            .collect();
        chunks.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for VoxelWorld {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let chunks: Vec<(ChunkPos, VoxelChunk)> = Vec::deserialize(deserializer)?;
        let mut world = VoxelWorld::new();
        for (pos, chunk) in chunks {
            world.chunks.insert(pos, chunk);
        }
        Ok(world)
    }
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

    /// Clear all chunks from the world.
    pub fn clear(&mut self) {
        self.chunks.clear();
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

    /// Extract border occupancy data for a chunk from its neighbors.
    ///
    /// This is used for cross-chunk face culling and AO calculation. When generating a mesh for a chunk,
    /// we need to know if voxels at the chunk boundary have neighbors in adjacent chunks.
    ///
    /// Returns a `ChunkBorders` struct containing:
    /// - Face borders (6): 2D slices from face-adjacent chunks
    /// - Edge borders (12): 1D lines from edge-adjacent chunks (diagonal in 2 axes)
    /// - Corner borders (8): Single voxels from corner-adjacent chunks (diagonal in 3 axes)
    pub fn extract_borders(&self, chunk_pos: ChunkPos) -> ChunkBorders {
        ChunkBorders {
            // Face borders
            neg_x: self.extract_border_slice(chunk_pos, BorderDirection::NegX),
            pos_x: self.extract_border_slice(chunk_pos, BorderDirection::PosX),
            neg_y: self.extract_border_slice(chunk_pos, BorderDirection::NegY),
            pos_y: self.extract_border_slice(chunk_pos, BorderDirection::PosY),
            neg_z: self.extract_border_slice(chunk_pos, BorderDirection::NegZ),
            pos_z: self.extract_border_slice(chunk_pos, BorderDirection::PosZ),
            // Edge borders
            edge_neg_x_neg_y: self.extract_border_edge(chunk_pos, -1, -1, 0),
            edge_neg_x_pos_y: self.extract_border_edge(chunk_pos, -1, 1, 0),
            edge_pos_x_neg_y: self.extract_border_edge(chunk_pos, 1, -1, 0),
            edge_pos_x_pos_y: self.extract_border_edge(chunk_pos, 1, 1, 0),
            edge_neg_x_neg_z: self.extract_border_edge(chunk_pos, -1, 0, -1),
            edge_neg_x_pos_z: self.extract_border_edge(chunk_pos, -1, 0, 1),
            edge_pos_x_neg_z: self.extract_border_edge(chunk_pos, 1, 0, -1),
            edge_pos_x_pos_z: self.extract_border_edge(chunk_pos, 1, 0, 1),
            edge_neg_y_neg_z: self.extract_border_edge(chunk_pos, 0, -1, -1),
            edge_neg_y_pos_z: self.extract_border_edge(chunk_pos, 0, -1, 1),
            edge_pos_y_neg_z: self.extract_border_edge(chunk_pos, 0, 1, -1),
            edge_pos_y_pos_z: self.extract_border_edge(chunk_pos, 0, 1, 1),
            // Corner borders
            corner_neg_x_neg_y_neg_z: self.extract_border_corner(chunk_pos, -1, -1, -1),
            corner_neg_x_neg_y_pos_z: self.extract_border_corner(chunk_pos, -1, -1, 1),
            corner_neg_x_pos_y_neg_z: self.extract_border_corner(chunk_pos, -1, 1, -1),
            corner_neg_x_pos_y_pos_z: self.extract_border_corner(chunk_pos, -1, 1, 1),
            corner_pos_x_neg_y_neg_z: self.extract_border_corner(chunk_pos, 1, -1, -1),
            corner_pos_x_neg_y_pos_z: self.extract_border_corner(chunk_pos, 1, -1, 1),
            corner_pos_x_pos_y_neg_z: self.extract_border_corner(chunk_pos, 1, 1, -1),
            corner_pos_x_pos_y_pos_z: self.extract_border_corner(chunk_pos, 1, 1, 1),
        }
    }

    /// Extract a single border slice from a neighboring chunk.
    fn extract_border_slice(&self, chunk_pos: ChunkPos, direction: BorderDirection) -> BorderSlice {
        let neighbor_pos = match direction {
            BorderDirection::NegX => ChunkPos::new(chunk_pos.x - 1, chunk_pos.y, chunk_pos.z),
            BorderDirection::PosX => ChunkPos::new(chunk_pos.x + 1, chunk_pos.y, chunk_pos.z),
            BorderDirection::NegY => ChunkPos::new(chunk_pos.x, chunk_pos.y - 1, chunk_pos.z),
            BorderDirection::PosY => ChunkPos::new(chunk_pos.x, chunk_pos.y + 1, chunk_pos.z),
            BorderDirection::NegZ => ChunkPos::new(chunk_pos.x, chunk_pos.y, chunk_pos.z - 1),
            BorderDirection::PosZ => ChunkPos::new(chunk_pos.x, chunk_pos.y, chunk_pos.z + 1),
        };

        let Some(neighbor_chunk) = self.get_chunk(neighbor_pos) else {
            return BorderSlice::empty();
        };

        // Extract the edge slice from the neighbor chunk that borders our chunk
        let mut occupancy = [false; CHUNK_SIZE * CHUNK_SIZE];

        match direction {
            // -X neighbor: we need their +X edge (x = CHUNK_SIZE-1)
            BorderDirection::NegX => {
                for y in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        occupancy[y * CHUNK_SIZE + z] =
                            neighbor_chunk.get(CHUNK_SIZE - 1, y, z).is_some();
                    }
                }
            }
            // +X neighbor: we need their -X edge (x = 0)
            BorderDirection::PosX => {
                for y in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        occupancy[y * CHUNK_SIZE + z] = neighbor_chunk.get(0, y, z).is_some();
                    }
                }
            }
            // -Y neighbor: we need their +Y edge (y = CHUNK_SIZE-1)
            BorderDirection::NegY => {
                for x in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        occupancy[x * CHUNK_SIZE + z] =
                            neighbor_chunk.get(x, CHUNK_SIZE - 1, z).is_some();
                    }
                }
            }
            // +Y neighbor: we need their -Y edge (y = 0)
            BorderDirection::PosY => {
                for x in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        occupancy[x * CHUNK_SIZE + z] = neighbor_chunk.get(x, 0, z).is_some();
                    }
                }
            }
            // -Z neighbor: we need their +Z edge (z = CHUNK_SIZE-1)
            BorderDirection::NegZ => {
                for x in 0..CHUNK_SIZE {
                    for y in 0..CHUNK_SIZE {
                        occupancy[x * CHUNK_SIZE + y] =
                            neighbor_chunk.get(x, y, CHUNK_SIZE - 1).is_some();
                    }
                }
            }
            // +Z neighbor: we need their -Z edge (z = 0)
            BorderDirection::PosZ => {
                for x in 0..CHUNK_SIZE {
                    for y in 0..CHUNK_SIZE {
                        occupancy[x * CHUNK_SIZE + y] = neighbor_chunk.get(x, y, 0).is_some();
                    }
                }
            }
        }

        BorderSlice { occupancy }
    }

    /// Extract a 1D edge from a diagonally-adjacent chunk (shares an edge with this chunk).
    ///
    /// # Arguments
    /// * `chunk_pos` - Position of the chunk we're extracting borders FOR
    /// * `dx, dy, dz` - Direction to the edge neighbor (-1, 0, or 1). Exactly two must be non-zero.
    fn extract_border_edge(&self, chunk_pos: ChunkPos, dx: i32, dy: i32, dz: i32) -> BorderEdge {
        let neighbor_pos = ChunkPos::new(chunk_pos.x + dx, chunk_pos.y + dy, chunk_pos.z + dz);

        let Some(neighbor_chunk) = self.get_chunk(neighbor_pos) else {
            return BorderEdge::empty();
        };

        let mut occupancy = [false; CHUNK_SIZE];

        // Determine which voxel position to sample based on direction
        // If dx = -1, we need x = CHUNK_SIZE-1 from neighbor (their +X edge)
        // If dx = +1, we need x = 0 from neighbor (their -X edge)
        // If dx = 0, x varies along the edge
        let x_fixed = if dx < 0 {
            Some(CHUNK_SIZE - 1)
        } else if dx > 0 {
            Some(0)
        } else {
            None
        };
        let y_fixed = if dy < 0 {
            Some(CHUNK_SIZE - 1)
        } else if dy > 0 {
            Some(0)
        } else {
            None
        };
        let z_fixed = if dz < 0 {
            Some(CHUNK_SIZE - 1)
        } else if dz > 0 {
            Some(0)
        } else {
            None
        };

        // The varying axis is the one where dx/dy/dz == 0
        for i in 0..CHUNK_SIZE {
            let x = x_fixed.unwrap_or(i);
            let y = y_fixed.unwrap_or(i);
            let z = z_fixed.unwrap_or(i);
            occupancy[i] = neighbor_chunk.get(x, y, z).is_some();
        }

        BorderEdge { occupancy }
    }

    /// Extract a single corner voxel from a diagonally-adjacent chunk (shares only a corner).
    ///
    /// # Arguments
    /// * `chunk_pos` - Position of the chunk we're extracting borders FOR
    /// * `dx, dy, dz` - Direction to the corner neighbor (-1 or 1 for each axis)
    fn extract_border_corner(&self, chunk_pos: ChunkPos, dx: i32, dy: i32, dz: i32) -> bool {
        let neighbor_pos = ChunkPos::new(chunk_pos.x + dx, chunk_pos.y + dy, chunk_pos.z + dz);

        let Some(neighbor_chunk) = self.get_chunk(neighbor_pos) else {
            return false;
        };

        // Sample the corner voxel from the neighbor that's closest to us
        // If dx = -1, we need x = CHUNK_SIZE-1 (their +X corner)
        // If dx = +1, we need x = 0 (their -X corner)
        let x = if dx < 0 { CHUNK_SIZE - 1 } else { 0 };
        let y = if dy < 0 { CHUNK_SIZE - 1 } else { 0 };
        let z = if dz < 0 { CHUNK_SIZE - 1 } else { 0 };

        neighbor_chunk.get(x, y, z).is_some()
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

    /// Extract voxels within an AABB, removing them from self.
    ///
    /// Returns a new VoxelWorld with coordinates relative to `min` corner.
    /// The AABB is defined as `[min, max)` - inclusive of min, exclusive of max.
    ///
    /// # Arguments
    /// * `min` - Minimum corner of the AABB (inclusive)
    /// * `max` - Maximum corner of the AABB (exclusive)
    ///
    /// # Returns
    /// A new VoxelWorld containing the extracted voxels, with coordinates
    /// shifted so that `min` becomes the origin (0, 0, 0).
    ///
    /// # Example
    /// ```
    /// use studio_core::voxel::{VoxelWorld, Voxel};
    /// use bevy::prelude::IVec3;
    ///
    /// let mut world = VoxelWorld::new();
    /// world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
    ///
    /// let fragment = world.split_aabb(IVec3::new(5, 5, 5), IVec3::new(6, 6, 6));
    ///
    /// assert!(world.get_voxel(5, 5, 5).is_none()); // Removed from original
    /// assert!(fragment.get_voxel(0, 0, 0).is_some()); // At origin in fragment
    /// ```
    pub fn split_aabb(&mut self, min: IVec3, max: IVec3) -> VoxelWorld {
        let mut fragment = VoxelWorld::new();

        // Early return if invalid bounds
        if min.x >= max.x || min.y >= max.y || min.z >= max.z {
            return fragment;
        }

        // Calculate which chunks might contain voxels in this AABB
        let chunk_min = ChunkPos::from_world(min.x, min.y, min.z);
        let chunk_max = ChunkPos::from_world(max.x - 1, max.y - 1, max.z - 1);

        // Iterate through relevant chunks
        for chunk_pos in ChunkPos::iter_range(chunk_min, chunk_max) {
            let Some(chunk) = self.chunks.get(&chunk_pos) else {
                continue;
            };

            let (chunk_origin_x, chunk_origin_y, chunk_origin_z) = chunk_pos.world_origin();

            // Collect voxels to extract from this chunk
            let mut to_extract: Vec<(i32, i32, i32, Voxel)> = Vec::new();

            for (lx, ly, lz, voxel) in chunk.iter() {
                let wx = chunk_origin_x + lx as i32;
                let wy = chunk_origin_y + ly as i32;
                let wz = chunk_origin_z + lz as i32;

                // Check if within AABB (min inclusive, max exclusive)
                if wx >= min.x
                    && wx < max.x
                    && wy >= min.y
                    && wy < max.y
                    && wz >= min.z
                    && wz < max.z
                {
                    to_extract.push((wx, wy, wz, voxel));
                }
            }

            // Move voxels to fragment (with coordinates relative to min)
            for (wx, wy, wz, voxel) in to_extract {
                // Add to fragment with coordinates relative to min
                fragment.set_voxel(wx - min.x, wy - min.y, wz - min.z, voxel);
            }
        }

        // Remove extracted voxels from self
        for (chunk_pos, chunk) in fragment.iter_chunks() {
            let (frag_origin_x, frag_origin_y, frag_origin_z) = chunk_pos.world_origin();

            for (lx, ly, lz, _) in chunk.iter() {
                // Convert fragment coordinates back to world coordinates
                let fx = frag_origin_x + lx as i32;
                let fy = frag_origin_y + ly as i32;
                let fz = frag_origin_z + lz as i32;

                let wx = fx + min.x;
                let wy = fy + min.y;
                let wz = fz + min.z;

                self.clear_voxel(wx, wy, wz);
            }
        }

        // Prune empty chunks from self
        self.prune_empty_chunks();

        fragment
    }

    /// Extract voxels within a sphere, removing them from self.
    ///
    /// Returns a new VoxelWorld with coordinates relative to the sphere center.
    /// Uses integer distance check: a voxel at position (x,y,z) is included if
    /// `(x - center.x)² + (y - center.y)² + (z - center.z)² <= radius²`.
    ///
    /// # Arguments
    /// * `center` - Center of the sphere in world coordinates
    /// * `radius` - Radius of the sphere (inclusive)
    ///
    /// # Returns
    /// A new VoxelWorld containing the extracted voxels, with coordinates
    /// shifted so that `center` becomes the origin (0, 0, 0).
    ///
    /// # Example
    /// ```
    /// use studio_core::voxel::{VoxelWorld, Voxel};
    /// use bevy::prelude::IVec3;
    ///
    /// let mut world = VoxelWorld::new();
    /// // Create a 10x10x10 cube
    /// for x in 0..10 {
    ///     for y in 0..10 {
    ///         for z in 0..10 {
    ///             world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
    ///         }
    ///     }
    /// }
    ///
    /// // Extract a sphere of radius 2 centered at (5, 5, 5)
    /// let fragment = world.split_sphere(IVec3::new(5, 5, 5), 2);
    ///
    /// // Center voxel should be at origin in fragment
    /// assert!(fragment.get_voxel(0, 0, 0).is_some());
    /// // Original should have a hole
    /// assert!(world.get_voxel(5, 5, 5).is_none());
    /// ```
    pub fn split_sphere(&mut self, center: IVec3, radius: i32) -> VoxelWorld {
        let mut fragment = VoxelWorld::new();

        if radius < 0 {
            return fragment;
        }

        let radius_sq = (radius as i64) * (radius as i64);

        // Calculate AABB that bounds the sphere
        let min = IVec3::new(center.x - radius, center.y - radius, center.z - radius);
        let max = IVec3::new(
            center.x + radius + 1,
            center.y + radius + 1,
            center.z + radius + 1,
        );

        // Calculate which chunks might contain voxels in this sphere
        let chunk_min = ChunkPos::from_world(min.x, min.y, min.z);
        let chunk_max = ChunkPos::from_world(max.x - 1, max.y - 1, max.z - 1);

        // Iterate through relevant chunks
        for chunk_pos in ChunkPos::iter_range(chunk_min, chunk_max) {
            let Some(chunk) = self.chunks.get(&chunk_pos) else {
                continue;
            };

            let (chunk_origin_x, chunk_origin_y, chunk_origin_z) = chunk_pos.world_origin();

            // Collect voxels to extract from this chunk
            let mut to_extract: Vec<(i32, i32, i32, Voxel)> = Vec::new();

            for (lx, ly, lz, voxel) in chunk.iter() {
                let wx = chunk_origin_x + lx as i32;
                let wy = chunk_origin_y + ly as i32;
                let wz = chunk_origin_z + lz as i32;

                // Check if within sphere using squared distance
                let dx = (wx - center.x) as i64;
                let dy = (wy - center.y) as i64;
                let dz = (wz - center.z) as i64;
                let dist_sq = dx * dx + dy * dy + dz * dz;

                if dist_sq <= radius_sq {
                    to_extract.push((wx, wy, wz, voxel));
                }
            }

            // Move voxels to fragment (with coordinates relative to center)
            for (wx, wy, wz, voxel) in to_extract {
                fragment.set_voxel(wx - center.x, wy - center.y, wz - center.z, voxel);
            }
        }

        // Remove extracted voxels from self
        for (chunk_pos, chunk) in fragment.iter_chunks() {
            let (frag_origin_x, frag_origin_y, frag_origin_z) = chunk_pos.world_origin();

            for (lx, ly, lz, _) in chunk.iter() {
                // Convert fragment coordinates back to world coordinates
                let fx = frag_origin_x + lx as i32;
                let fy = frag_origin_y + ly as i32;
                let fz = frag_origin_z + lz as i32;

                let wx = fx + center.x;
                let wy = fy + center.y;
                let wz = fz + center.z;

                self.clear_voxel(wx, wy, wz);
            }
        }

        // Prune empty chunks from self
        self.prune_empty_chunks();

        fragment
    }

    /// Merge another VoxelWorld into self at the given offset.
    ///
    /// Voxels from `other` are added to `self` with their positions shifted by `offset`.
    /// If a voxel position already exists in self, it will be overwritten.
    ///
    /// # Arguments
    /// * `other` - The VoxelWorld to merge from
    /// * `offset` - Position offset to apply to other's voxels
    ///
    /// # Example
    /// ```
    /// use studio_core::voxel::{VoxelWorld, Voxel};
    /// use bevy::prelude::IVec3;
    ///
    /// let mut world = VoxelWorld::new();
    /// let mut fragment = VoxelWorld::new();
    /// fragment.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
    ///
    /// world.merge_from(&fragment, IVec3::new(10, 10, 10));
    ///
    /// assert!(world.get_voxel(10, 10, 10).is_some());
    /// ```
    pub fn merge_from(&mut self, other: &VoxelWorld, offset: IVec3) {
        for (chunk_pos, chunk) in other.iter_chunks() {
            let (chunk_origin_x, chunk_origin_y, chunk_origin_z) = chunk_pos.world_origin();

            for (lx, ly, lz, voxel) in chunk.iter() {
                let other_x = chunk_origin_x + lx as i32;
                let other_y = chunk_origin_y + ly as i32;
                let other_z = chunk_origin_z + lz as i32;

                // Apply offset to get position in self
                let world_x = other_x + offset.x;
                let world_y = other_y + offset.y;
                let world_z = other_z + offset.z;

                self.set_voxel(world_x, world_y, world_z, voxel);
            }
        }
    }

    /// Shift all voxels by the given offset.
    ///
    /// This is useful for recentering a VoxelWorld after extraction,
    /// e.g., to place the centroid at the origin.
    ///
    /// # Arguments
    /// * `offset` - The offset to apply to all voxel positions
    ///
    /// # Example
    /// ```
    /// use studio_core::voxel::{VoxelWorld, Voxel};
    /// use bevy::prelude::IVec3;
    ///
    /// let mut world = VoxelWorld::new();
    /// world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
    ///
    /// world.translate(IVec3::new(10, 20, 30));
    ///
    /// assert!(world.get_voxel(0, 0, 0).is_none());
    /// assert!(world.get_voxel(10, 20, 30).is_some());
    /// ```
    pub fn translate(&mut self, offset: IVec3) {
        if offset == IVec3::ZERO {
            return;
        }

        // Collect all voxels with their current positions
        let voxels: Vec<(i32, i32, i32, Voxel)> = self
            .iter_chunks()
            .flat_map(|(chunk_pos, chunk)| {
                let (ox, oy, oz) = chunk_pos.world_origin();
                chunk.iter().map(move |(lx, ly, lz, voxel)| {
                    (ox + lx as i32, oy + ly as i32, oz + lz as i32, voxel)
                })
            })
            .collect();

        // Clear all chunks
        self.chunks.clear();

        // Re-insert at new positions
        for (x, y, z, voxel) in voxels {
            self.set_voxel(x + offset.x, y + offset.y, z + offset.z, voxel);
        }
    }

    /// Get the centroid (center of mass) of all voxels.
    ///
    /// Returns the average position of all voxels, with 0.5 added to each
    /// component to represent the center of each voxel cube (not its corner).
    ///
    /// # Returns
    /// The centroid position in world coordinates, or None if the world is empty.
    ///
    /// # Example
    /// ```
    /// use studio_core::voxel::{VoxelWorld, Voxel};
    ///
    /// let mut world = VoxelWorld::new();
    /// world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
    /// world.set_voxel(2, 0, 0, Voxel::solid(255, 0, 0));
    ///
    /// let centroid = world.centroid().unwrap();
    /// // Average of (0.5, 0.5, 0.5) and (2.5, 0.5, 0.5) = (1.5, 0.5, 0.5)
    /// assert!((centroid.x - 1.5).abs() < 0.01);
    /// ```
    pub fn centroid(&self) -> Option<Vec3> {
        let mut sum_x: i64 = 0;
        let mut sum_y: i64 = 0;
        let mut sum_z: i64 = 0;
        let mut count: u64 = 0;

        for (chunk_pos, chunk) in self.iter_chunks() {
            let (ox, oy, oz) = chunk_pos.world_origin();

            for (lx, ly, lz, _) in chunk.iter() {
                sum_x += (ox + lx as i32) as i64;
                sum_y += (oy + ly as i32) as i64;
                sum_z += (oz + lz as i32) as i64;
                count += 1;
            }
        }

        if count == 0 {
            return None;
        }

        // Return centroid at center of voxels (add 0.5)
        Some(Vec3::new(
            (sum_x as f64 / count as f64 + 0.5) as f32,
            (sum_y as f64 / count as f64 + 0.5) as f32,
            (sum_z as f64 / count as f64 + 0.5) as f32,
        ))
    }

    /// Check if merging would cause any voxel collisions.
    ///
    /// Returns a list of world positions where both `self` and `other` (after offset)
    /// have voxels. Useful for previewing merge operations or detecting if a fragment
    /// can be placed without overwriting existing voxels.
    ///
    /// # Arguments
    /// * `other` - The VoxelWorld to check for collisions with
    /// * `offset` - Position offset to apply to other's voxels
    ///
    /// # Returns
    /// A Vec of world positions where collisions would occur.
    /// Empty vec means no collisions.
    ///
    /// # Example
    /// ```
    /// use studio_core::voxel::{VoxelWorld, Voxel};
    /// use bevy::prelude::IVec3;
    ///
    /// let mut world = VoxelWorld::new();
    /// world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
    ///
    /// let mut other = VoxelWorld::new();
    /// other.set_voxel(0, 0, 0, Voxel::solid(0, 255, 0));
    ///
    /// // No collision at offset (10, 0, 0)
    /// assert!(world.check_merge_collisions(&other, IVec3::new(10, 0, 0)).is_empty());
    ///
    /// // Collision at offset (5, 5, 5)
    /// assert_eq!(world.check_merge_collisions(&other, IVec3::new(5, 5, 5)).len(), 1);
    /// ```
    pub fn check_merge_collisions(&self, other: &VoxelWorld, offset: IVec3) -> Vec<IVec3> {
        let mut collisions = Vec::new();

        for (chunk_pos, chunk) in other.iter_chunks() {
            let (chunk_origin_x, chunk_origin_y, chunk_origin_z) = chunk_pos.world_origin();

            for (lx, ly, lz, _) in chunk.iter() {
                let other_x = chunk_origin_x + lx as i32;
                let other_y = chunk_origin_y + ly as i32;
                let other_z = chunk_origin_z + lz as i32;

                // Apply offset to get position in self
                let world_x = other_x + offset.x;
                let world_y = other_y + offset.y;
                let world_z = other_z + offset.z;

                // Check if self has a voxel at this position
                if self.get_voxel(world_x, world_y, world_z).is_some() {
                    collisions.push(IVec3::new(world_x, world_y, world_z));
                }
            }
        }

        collisions
    }

    /// Get the actual voxel bounding box in world coordinates.
    ///
    /// Unlike `chunk_bounds()` which returns chunk-level bounds,
    /// this iterates through all voxels to find the exact min/max positions.
    ///
    /// Returns `(min_corner, max_corner)` in world coordinates, or None if empty.
    pub fn voxel_bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut min_z = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut max_z = i32::MIN;
        let mut found_any = false;

        for (chunk_pos, chunk) in &self.chunks {
            let chunk_offset_x = chunk_pos.x * CHUNK_SIZE_I32;
            let chunk_offset_y = chunk_pos.y * CHUNK_SIZE_I32;
            let chunk_offset_z = chunk_pos.z * CHUNK_SIZE_I32;

            for (lx, ly, lz, _voxel) in chunk.iter() {
                found_any = true;
                let wx = chunk_offset_x + lx as i32;
                let wy = chunk_offset_y + ly as i32;
                let wz = chunk_offset_z + lz as i32;

                min_x = min_x.min(wx);
                min_y = min_y.min(wy);
                min_z = min_z.min(wz);
                max_x = max_x.max(wx);
                max_y = max_y.max(wy);
                max_z = max_z.max(wz);
            }
        }

        if !found_any {
            return None;
        }

        // Convert to Vec3, adding 1 to max because voxels are 1x1x1 cubes
        Some((
            Vec3::new(min_x as f32, min_y as f32, min_z as f32),
            Vec3::new((max_x + 1) as f32, (max_y + 1) as f32, (max_z + 1) as f32),
        ))
    }
}

// ============================================================================
// Cross-Chunk Border Data
// ============================================================================

/// Direction of a chunk border for cross-chunk face culling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderDirection {
    NegX,
    PosX,
    NegY,
    PosY,
    NegZ,
    PosZ,
}

/// A 2D slice of occupancy data from a chunk border.
///
/// This represents a CHUNK_SIZE x CHUNK_SIZE grid of boolean values
/// indicating whether each voxel position is solid.
///
/// The indexing depends on the border direction:
/// - X borders: `occupancy[y * CHUNK_SIZE + z]`
/// - Y borders: `occupancy[x * CHUNK_SIZE + z]`
/// - Z borders: `occupancy[x * CHUNK_SIZE + y]`
#[derive(Clone, Debug)]
pub struct BorderSlice {
    /// Occupancy data for the border slice.
    /// True = solid voxel, False = empty.
    pub occupancy: [bool; CHUNK_SIZE * CHUNK_SIZE],
}

impl BorderSlice {
    /// Create an empty border slice (all false - no solid voxels).
    pub fn empty() -> Self {
        Self {
            occupancy: [false; CHUNK_SIZE * CHUNK_SIZE],
        }
    }

    /// Check if a position in this border slice is solid.
    ///
    /// For X borders, use (y, z). For Y borders, use (x, z). For Z borders, use (x, y).
    #[inline]
    pub fn is_solid(&self, u: usize, v: usize) -> bool {
        if u < CHUNK_SIZE && v < CHUNK_SIZE {
            self.occupancy[u * CHUNK_SIZE + v]
        } else {
            false
        }
    }
}

/// A 1D edge of occupancy data from a chunk edge (where two faces meet).
///
/// This represents a CHUNK_SIZE line of boolean values for edge neighbors
/// (diagonal neighbors that cross two chunk boundaries).
#[derive(Clone, Debug)]
pub struct BorderEdge {
    /// Occupancy data for the edge.
    /// True = solid voxel, False = empty.
    pub occupancy: [bool; CHUNK_SIZE],
}

impl BorderEdge {
    /// Create an empty border edge (all false - no solid voxels).
    pub fn empty() -> Self {
        Self {
            occupancy: [false; CHUNK_SIZE],
        }
    }

    /// Check if a position along this edge is solid.
    #[inline]
    pub fn is_solid(&self, idx: usize) -> bool {
        if idx < CHUNK_SIZE {
            self.occupancy[idx]
        } else {
            false
        }
    }
}

/// Border occupancy data from all 6 neighboring chunks, 12 edge neighbors, and 8 corner neighbors.
///
/// Used for cross-chunk face culling and AO calculation when generating meshes.
///
/// Face borders (6): Data from chunks sharing a face with this chunk.
/// Edge borders (12): Data from chunks sharing an edge with this chunk (diagonal in 2 axes).
/// Corner borders (8): Data from chunks sharing only a corner with this chunk (diagonal in 3 axes).
#[derive(Clone, Debug)]
pub struct ChunkBorders {
    // === Face borders (6) - 2D slices ===
    /// Border from -X neighbor (their x=CHUNK_SIZE-1 slice)
    pub neg_x: BorderSlice,
    /// Border from +X neighbor (their x=0 slice)
    pub pos_x: BorderSlice,
    /// Border from -Y neighbor (their y=CHUNK_SIZE-1 slice)
    pub neg_y: BorderSlice,
    /// Border from +Y neighbor (their y=0 slice)
    pub pos_y: BorderSlice,
    /// Border from -Z neighbor (their z=CHUNK_SIZE-1 slice)
    pub neg_z: BorderSlice,
    /// Border from +Z neighbor (their z=0 slice)
    pub pos_z: BorderSlice,

    // === Edge borders (12) - 1D lines ===
    // XY edges (vary along Z)
    pub edge_neg_x_neg_y: BorderEdge,
    pub edge_neg_x_pos_y: BorderEdge,
    pub edge_pos_x_neg_y: BorderEdge,
    pub edge_pos_x_pos_y: BorderEdge,
    // XZ edges (vary along Y)
    pub edge_neg_x_neg_z: BorderEdge,
    pub edge_neg_x_pos_z: BorderEdge,
    pub edge_pos_x_neg_z: BorderEdge,
    pub edge_pos_x_pos_z: BorderEdge,
    // YZ edges (vary along X)
    pub edge_neg_y_neg_z: BorderEdge,
    pub edge_neg_y_pos_z: BorderEdge,
    pub edge_pos_y_neg_z: BorderEdge,
    pub edge_pos_y_pos_z: BorderEdge,

    // === Corner borders (8) - single booleans ===
    pub corner_neg_x_neg_y_neg_z: bool,
    pub corner_neg_x_neg_y_pos_z: bool,
    pub corner_neg_x_pos_y_neg_z: bool,
    pub corner_neg_x_pos_y_pos_z: bool,
    pub corner_pos_x_neg_y_neg_z: bool,
    pub corner_pos_x_neg_y_pos_z: bool,
    pub corner_pos_x_pos_y_neg_z: bool,
    pub corner_pos_x_pos_y_pos_z: bool,
}

impl ChunkBorders {
    /// Create empty borders (no neighbors).
    pub fn empty() -> Self {
        Self {
            // Face borders
            neg_x: BorderSlice::empty(),
            pos_x: BorderSlice::empty(),
            neg_y: BorderSlice::empty(),
            pos_y: BorderSlice::empty(),
            neg_z: BorderSlice::empty(),
            pos_z: BorderSlice::empty(),
            // Edge borders
            edge_neg_x_neg_y: BorderEdge::empty(),
            edge_neg_x_pos_y: BorderEdge::empty(),
            edge_pos_x_neg_y: BorderEdge::empty(),
            edge_pos_x_pos_y: BorderEdge::empty(),
            edge_neg_x_neg_z: BorderEdge::empty(),
            edge_neg_x_pos_z: BorderEdge::empty(),
            edge_pos_x_neg_z: BorderEdge::empty(),
            edge_pos_x_pos_z: BorderEdge::empty(),
            edge_neg_y_neg_z: BorderEdge::empty(),
            edge_neg_y_pos_z: BorderEdge::empty(),
            edge_pos_y_neg_z: BorderEdge::empty(),
            edge_pos_y_pos_z: BorderEdge::empty(),
            // Corner borders
            corner_neg_x_neg_y_neg_z: false,
            corner_neg_x_neg_y_pos_z: false,
            corner_neg_x_pos_y_neg_z: false,
            corner_neg_x_pos_y_pos_z: false,
            corner_pos_x_neg_y_neg_z: false,
            corner_pos_x_neg_y_pos_z: false,
            corner_pos_x_pos_y_neg_z: false,
            corner_pos_x_pos_y_pos_z: false,
        }
    }

    /// Check if a neighbor voxel across the chunk boundary is solid.
    ///
    /// This is called when checking faces at the chunk boundary during mesh generation.
    /// It looks up the appropriate border data based on which chunk boundaries the
    /// target position crosses.
    ///
    /// Handles:
    /// - Single-axis crossings (face neighbors): Use 2D BorderSlice
    /// - Two-axis crossings (edge neighbors): Use 1D BorderEdge
    /// - Three-axis crossings (corner neighbors): Use single bool
    ///
    /// # Arguments
    /// * `x, y, z` - Local position within the chunk (0 to CHUNK_SIZE-1)
    /// * `dx, dy, dz` - Neighbor offset (should cross chunk boundary)
    ///
    /// # Returns
    /// True if the neighbor position in the adjacent chunk is solid.
    pub fn is_neighbor_solid(
        &self,
        x: usize,
        y: usize,
        z: usize,
        dx: i32,
        dy: i32,
        dz: i32,
    ) -> bool {
        // Calculate target position
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        let nz = z as i32 + dz;

        // Determine which boundaries the TARGET position crosses
        let cross_neg_x = nx < 0;
        let cross_pos_x = nx >= CHUNK_SIZE as i32;
        let cross_neg_y = ny < 0;
        let cross_pos_y = ny >= CHUNK_SIZE as i32;
        let cross_neg_z = nz < 0;
        let cross_pos_z = nz >= CHUNK_SIZE as i32;

        let cross_x = cross_neg_x || cross_pos_x;
        let cross_y = cross_neg_y || cross_pos_y;
        let cross_z = cross_neg_z || cross_pos_z;

        let num_crossings = cross_x as u8 + cross_y as u8 + cross_z as u8;

        // Calculate the position within the neighbor chunk
        // If crossing negative boundary, wrap to CHUNK_SIZE-1
        // If crossing positive boundary, wrap to 0
        // If not crossing, use the target position as-is
        let target_x = if cross_neg_x {
            (CHUNK_SIZE as i32 + nx) as usize
        } else if cross_pos_x {
            (nx - CHUNK_SIZE as i32) as usize
        } else {
            nx as usize
        };
        let target_y = if cross_neg_y {
            (CHUNK_SIZE as i32 + ny) as usize
        } else if cross_pos_y {
            (ny - CHUNK_SIZE as i32) as usize
        } else {
            ny as usize
        };
        let target_z = if cross_neg_z {
            (CHUNK_SIZE as i32 + nz) as usize
        } else if cross_pos_z {
            (nz - CHUNK_SIZE as i32) as usize
        } else {
            nz as usize
        };

        match num_crossings {
            0 => false, // Not crossing any boundary - shouldn't happen if called correctly
            1 => {
                // Single-axis crossing - use face border
                // Look up using the coordinates that DON'T cross the boundary
                if cross_neg_x {
                    self.neg_x.is_solid(target_y, target_z)
                } else if cross_pos_x {
                    self.pos_x.is_solid(target_y, target_z)
                } else if cross_neg_y {
                    self.neg_y.is_solid(target_x, target_z)
                } else if cross_pos_y {
                    self.pos_y.is_solid(target_x, target_z)
                } else if cross_neg_z {
                    self.neg_z.is_solid(target_x, target_y)
                } else if cross_pos_z {
                    self.pos_z.is_solid(target_x, target_y)
                } else {
                    false
                }
            }
            2 => {
                // Two-axis crossing - use edge border
                // Look up using the coordinate that DOESN'T cross the boundary
                // XY edges (vary along Z) - Z doesn't cross
                if cross_neg_x && cross_neg_y {
                    self.edge_neg_x_neg_y.is_solid(target_z)
                } else if cross_neg_x && cross_pos_y {
                    self.edge_neg_x_pos_y.is_solid(target_z)
                } else if cross_pos_x && cross_neg_y {
                    self.edge_pos_x_neg_y.is_solid(target_z)
                } else if cross_pos_x && cross_pos_y {
                    self.edge_pos_x_pos_y.is_solid(target_z)
                }
                // XZ edges (vary along Y) - Y doesn't cross
                else if cross_neg_x && cross_neg_z {
                    self.edge_neg_x_neg_z.is_solid(target_y)
                } else if cross_neg_x && cross_pos_z {
                    self.edge_neg_x_pos_z.is_solid(target_y)
                } else if cross_pos_x && cross_neg_z {
                    self.edge_pos_x_neg_z.is_solid(target_y)
                } else if cross_pos_x && cross_pos_z {
                    self.edge_pos_x_pos_z.is_solid(target_y)
                }
                // YZ edges (vary along X) - X doesn't cross
                else if cross_neg_y && cross_neg_z {
                    self.edge_neg_y_neg_z.is_solid(target_x)
                } else if cross_neg_y && cross_pos_z {
                    self.edge_neg_y_pos_z.is_solid(target_x)
                } else if cross_pos_y && cross_neg_z {
                    self.edge_pos_y_neg_z.is_solid(target_x)
                } else if cross_pos_y && cross_pos_z {
                    self.edge_pos_y_pos_z.is_solid(target_x)
                } else {
                    false
                }
            }
            3 => {
                // Three-axis crossing - use corner border
                if cross_neg_x && cross_neg_y && cross_neg_z {
                    self.corner_neg_x_neg_y_neg_z
                } else if cross_neg_x && cross_neg_y && cross_pos_z {
                    self.corner_neg_x_neg_y_pos_z
                } else if cross_neg_x && cross_pos_y && cross_neg_z {
                    self.corner_neg_x_pos_y_neg_z
                } else if cross_neg_x && cross_pos_y && cross_pos_z {
                    self.corner_neg_x_pos_y_pos_z
                } else if cross_pos_x && cross_neg_y && cross_neg_z {
                    self.corner_pos_x_neg_y_neg_z
                } else if cross_pos_x && cross_neg_y && cross_pos_z {
                    self.corner_pos_x_neg_y_pos_z
                } else if cross_pos_x && cross_pos_y && cross_neg_z {
                    self.corner_pos_x_pos_y_neg_z
                } else if cross_pos_x && cross_pos_y && cross_pos_z {
                    self.corner_pos_x_pos_y_pos_z
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

/// Configuration for voxel scale in world space.
///
/// This controls how large voxels appear in the world. A scale of 1.0 means
/// 1 voxel = 1 world unit. A scale of 0.5 means 1 voxel = 0.5 world units,
/// making buildings appear half-sized.
///
/// ## How Scale Works
///
/// - **Rendering**: Entity transforms are scaled by this factor
/// - **Collision**: World positions are divided by scale before voxel lookup
/// - **Physics**: Penetration depths and contact points are scaled back to world space
///
/// ## Example
///
/// ```ignore
/// // Make voxels appear half-sized
/// app.insert_resource(VoxelScaleConfig::new(0.5));
///
/// // Or use default (1.0)
/// app.init_resource::<VoxelScaleConfig>();
/// ```
#[derive(Resource, Clone, Debug, ExtractResource)]
pub struct VoxelScaleConfig {
    /// Scale factor for voxels in world space.
    /// - 1.0 = 1 voxel = 1 world unit (default)
    /// - 0.5 = 1 voxel = 0.5 world units (half-sized)
    /// - 2.0 = 1 voxel = 2 world units (double-sized)
    pub scale: f32,
}

impl Default for VoxelScaleConfig {
    fn default() -> Self {
        Self { scale: 1.0 }
    }
}

impl VoxelScaleConfig {
    /// Create a new scale config with the given scale factor.
    pub fn new(scale: f32) -> Self {
        Self { scale }
    }

    /// Convert a world position to voxel space.
    #[inline]
    pub fn world_to_voxel(&self, world_pos: Vec3) -> Vec3 {
        world_pos / self.scale
    }

    /// Convert a voxel position to world space.
    #[inline]
    pub fn voxel_to_world(&self, voxel_pos: Vec3) -> Vec3 {
        voxel_pos * self.scale
    }

    /// Scale a distance/size from voxel space to world space.
    #[inline]
    pub fn scale_to_world(&self, voxel_distance: f32) -> f32 {
        voxel_distance * self.scale
    }

    /// Scale a distance/size from world space to voxel space.
    #[inline]
    pub fn scale_to_voxel(&self, world_distance: f32) -> f32 {
        world_distance / self.scale
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

    // ========================================================================
    // ChunkBorders tests (cross-chunk face culling)
    // ========================================================================

    #[test]
    fn test_border_slice_empty() {
        let slice = BorderSlice::empty();
        // All positions should be false
        for u in 0..CHUNK_SIZE {
            for v in 0..CHUNK_SIZE {
                assert!(!slice.is_solid(u, v));
            }
        }
    }

    #[test]
    fn test_border_slice_is_solid() {
        let mut slice = BorderSlice::empty();
        // Set a specific position
        slice.occupancy[5 * CHUNK_SIZE + 10] = true;

        assert!(slice.is_solid(5, 10));
        assert!(!slice.is_solid(5, 9));
        assert!(!slice.is_solid(4, 10));
    }

    #[test]
    fn test_chunk_borders_empty() {
        let borders = ChunkBorders::empty();

        // All border queries should return false
        assert!(!borders.is_neighbor_solid(0, 5, 5, -1, 0, 0)); // -X
        assert!(!borders.is_neighbor_solid(CHUNK_SIZE - 1, 5, 5, 1, 0, 0)); // +X
        assert!(!borders.is_neighbor_solid(5, 0, 5, 0, -1, 0)); // -Y
        assert!(!borders.is_neighbor_solid(5, CHUNK_SIZE - 1, 5, 0, 1, 0)); // +Y
        assert!(!borders.is_neighbor_solid(5, 5, 0, 0, 0, -1)); // -Z
        assert!(!borders.is_neighbor_solid(5, 5, CHUNK_SIZE - 1, 0, 0, 1)); // +Z
    }

    #[test]
    fn test_extract_borders_no_neighbors() {
        let mut world = VoxelWorld::new();

        // Create a single chunk with some voxels
        world.set_voxel(16, 16, 16, Voxel::solid(255, 0, 0));

        let borders = world.extract_borders(ChunkPos::new(0, 0, 0));

        // No neighbors, so all borders should be empty
        assert!(!borders.is_neighbor_solid(0, 16, 16, -1, 0, 0));
        assert!(!borders.is_neighbor_solid(CHUNK_SIZE - 1, 16, 16, 1, 0, 0));
    }

    #[test]
    fn test_extract_borders_with_pos_x_neighbor() {
        let mut world = VoxelWorld::new();

        // Create chunk (0,0,0) and chunk (1,0,0)
        // Put a voxel at the +X edge of chunk (0,0,0)
        world.set_voxel(31, 10, 15, Voxel::solid(255, 0, 0));

        // Put a voxel at the -X edge of chunk (1,0,0) - this is adjacent!
        world.set_voxel(32, 10, 15, Voxel::solid(0, 255, 0));

        // Extract borders for chunk (0,0,0)
        let borders = world.extract_borders(ChunkPos::new(0, 0, 0));

        // The +X border should show the voxel from chunk (1,0,0)
        // Chunk (1,0,0)'s voxel at local (0, 10, 15) should appear in pos_x border
        assert!(borders.pos_x.is_solid(10, 15)); // y=10, z=15

        // Check that other positions are empty
        assert!(!borders.pos_x.is_solid(10, 14));
        assert!(!borders.pos_x.is_solid(9, 15));
    }

    #[test]
    fn test_extract_borders_with_neg_x_neighbor() {
        let mut world = VoxelWorld::new();

        // Create chunk (0,0,0) and chunk (-1,0,0)
        // Put a voxel at the -X edge of chunk (0,0,0)
        world.set_voxel(0, 5, 5, Voxel::solid(255, 0, 0));

        // Put a voxel at the +X edge of chunk (-1,0,0) - adjacent!
        // World coord (-1, 5, 5) -> chunk (-1,0,0), local (31, 5, 5)
        world.set_voxel(-1, 5, 5, Voxel::solid(0, 255, 0));

        // Extract borders for chunk (0,0,0)
        let borders = world.extract_borders(ChunkPos::new(0, 0, 0));

        // The -X border should show the voxel from chunk (-1,0,0)
        assert!(borders.neg_x.is_solid(5, 5)); // y=5, z=5
    }

    #[test]
    fn test_extract_borders_with_pos_y_neighbor() {
        let mut world = VoxelWorld::new();

        // Put a voxel at y=0 of chunk (0,1,0)
        world.set_voxel(10, 32, 15, Voxel::solid(0, 0, 255));

        // Extract borders for chunk (0,0,0)
        let borders = world.extract_borders(ChunkPos::new(0, 0, 0));

        // The +Y border should show the voxel
        assert!(borders.pos_y.is_solid(10, 15)); // x=10, z=15
    }

    #[test]
    fn test_chunk_borders_is_neighbor_solid_at_boundary() {
        let mut borders = ChunkBorders::empty();

        // Set up some data in the borders
        borders.neg_x.occupancy[5 * CHUNK_SIZE + 10] = true; // y=5, z=10
        borders.pos_z.occupancy[3 * CHUNK_SIZE + 7] = true; // x=3, y=7

        // Query -X neighbor from position (0, 5, 10)
        assert!(borders.is_neighbor_solid(0, 5, 10, -1, 0, 0));

        // Query +Z neighbor from position (3, 7, CHUNK_SIZE-1)
        assert!(borders.is_neighbor_solid(3, 7, CHUNK_SIZE - 1, 0, 0, 1));

        // Non-boundary positions should return false even with the same offset
        assert!(!borders.is_neighbor_solid(1, 5, 10, -1, 0, 0)); // x=1, not at boundary
    }

    #[test]
    fn test_cross_chunk_boundary_detection() {
        let mut world = VoxelWorld::new();

        // Create two adjacent chunks with touching voxels at the X boundary
        // Chunk (0,0,0): voxel at local (31, 16, 16)
        // Chunk (1,0,0): voxel at local (0, 16, 16)
        world.set_voxel(31, 16, 16, Voxel::solid(255, 0, 0)); // Last X in chunk 0
        world.set_voxel(32, 16, 16, Voxel::solid(0, 255, 0)); // First X in chunk 1

        // Extract borders for chunk (0,0,0)
        let borders0 = world.extract_borders(ChunkPos::new(0, 0, 0));

        // The +X border should detect the neighbor
        assert!(
            borders0.is_neighbor_solid(CHUNK_SIZE - 1, 16, 16, 1, 0, 0),
            "Should detect +X neighbor at boundary"
        );

        // Extract borders for chunk (1,0,0)
        let borders1 = world.extract_borders(ChunkPos::new(1, 0, 0));

        // The -X border should detect the neighbor
        assert!(
            borders1.is_neighbor_solid(0, 16, 16, -1, 0, 0),
            "Should detect -X neighbor at boundary"
        );
    }

    #[test]
    fn test_diagonal_neighbor_across_two_boundaries() {
        let mut world = VoxelWorld::new();

        // Create floor spanning 4 chunks at y=3
        // Chunk (0,0,0): floor at local x=24..31, z=24..31, y=3
        // Chunk (1,0,0): floor at local x=0..7, z=24..31, y=3
        // Chunk (0,0,1): floor at local x=24..31, z=0..7, y=3
        // Chunk (1,0,1): floor at local x=0..7, z=0..7, y=3
        for x in 24..40 {
            for z in 24..40 {
                world.set_voxel(x, 3, z, Voxel::solid(80, 80, 90));
            }
        }

        // Extract borders for chunk (0,0,0)
        let borders = world.extract_borders(ChunkPos::new(0, 0, 0));

        // From position (31, 3, 31), check diagonal offset (1, 1, 1)
        // Target position: (32, 4, 32) which is in chunk (1,0,1) at local (0, 4, 0)
        // This is ABOVE the floor, so should be air (false)
        let result = borders.is_neighbor_solid(31, 3, 31, 1, 1, 1);
        assert!(
            !result,
            "Position above diagonal floor should be air, got solid"
        );

        // From position (31, 3, 31), check diagonal offset (1, 0, 1)
        // Target position: (32, 3, 32) which is in chunk (1,0,1) at local (0, 3, 0)
        // This IS the floor, so should be solid (true)
        let result = borders.is_neighbor_solid(31, 3, 31, 1, 0, 1);
        assert!(result, "Diagonal floor position should be solid, got air");
    }

    #[test]
    fn test_edge_border_extraction() {
        let mut world = VoxelWorld::new();

        // Put a voxel in chunk (1,0,1) at local position (0, 5, 0)
        // World position: (32, 5, 32)
        world.set_voxel(32, 5, 32, Voxel::solid(255, 0, 0));

        // Extract borders for chunk (0,0,0)
        let borders = world.extract_borders(ChunkPos::new(0, 0, 0));

        // The edge_pos_x_pos_z should have this voxel at index 5 (y=5)
        assert!(
            borders.edge_pos_x_pos_z.is_solid(5),
            "Edge border should contain voxel at y=5"
        );
        assert!(
            !borders.edge_pos_x_pos_z.is_solid(4),
            "Edge border should not contain voxel at y=4"
        );
    }

    #[test]
    fn test_face_border_for_floor_ao() {
        let mut world = VoxelWorld::new();

        // Create floor at y=3 spanning chunks
        for x in 24..40 {
            for z in 24..40 {
                world.set_voxel(x, 3, z, Voxel::solid(80, 80, 90));
            }
        }

        // Extract borders for chunk (0,0,0)
        let borders = world.extract_borders(ChunkPos::new(0, 0, 0));

        // For AO calculation on floor voxel at (31, 3, 27), checking (1, 1, 0):
        // Target: (32, 4, 27) in chunk (1,0,0) at local (0, 4, 27)
        // This should be air (above floor)

        // The pos_x border should NOT have solid at (4, 27) since that's above the floor
        assert!(
            !borders.pos_x.is_solid(4, 27),
            "pos_x border at (y=4, z=27) should be air (above floor)"
        );

        // The pos_x border SHOULD have solid at (3, 27) since that's the floor level
        assert!(
            borders.pos_x.is_solid(3, 27),
            "pos_x border at (y=3, z=27) should be solid (floor level)"
        );

        // Test is_neighbor_solid for the AO case
        // From voxel at (31, 3, 27), offset (1, 1, 0) should return false (air above)
        let result = borders.is_neighbor_solid(31, 3, 27, 1, 1, 0);
        assert!(
            !result,
            "AO check (1,1,0) from floor voxel should find air above"
        );
    }

    // ========================================================================
    // split_aabb tests (Phase 22.1)
    // ========================================================================

    #[test]
    fn test_split_aabb_basic() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        // Create 4x4x4 cube at origin
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }
        assert_eq!(world.total_voxel_count(), 64);

        // Split out 2x2x2 corner
        let fragment = world.split_aabb(IVec3::ZERO, IVec3::new(2, 2, 2));

        // Fragment has 8 voxels (2x2x2)
        assert_eq!(fragment.total_voxel_count(), 8);
        // Original has 64 - 8 = 56 voxels
        assert_eq!(world.total_voxel_count(), 56);
        // Fragment coordinates are relative to min (0,0,0)
        assert!(fragment.get_voxel(0, 0, 0).is_some());
        assert!(fragment.get_voxel(1, 1, 1).is_some());
        assert!(fragment.get_voxel(2, 2, 2).is_none()); // exclusive upper bound
    }

    #[test]
    fn test_split_aabb_empty_region() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        world.set_voxel(10, 10, 10, Voxel::solid(255, 0, 0));

        // Split empty region
        let fragment = world.split_aabb(IVec3::ZERO, IVec3::new(5, 5, 5));

        assert_eq!(fragment.total_voxel_count(), 0);
        assert_eq!(world.total_voxel_count(), 1); // Original unchanged
    }

    #[test]
    fn test_split_aabb_negative_coordinates() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        // Voxels spanning negative to positive
        for x in -2..2 {
            for y in -2..2 {
                for z in -2..2 {
                    world.set_voxel(x, y, z, Voxel::solid(100, 100, 100));
                }
            }
        }
        assert_eq!(world.total_voxel_count(), 64);

        // Split negative quadrant
        let fragment = world.split_aabb(IVec3::new(-2, -2, -2), IVec3::ZERO);

        assert_eq!(fragment.total_voxel_count(), 8);
        // Fragment coords relative to min (-2,-2,-2), so (0,0,0) in fragment = (-2,-2,-2) in world
        assert!(fragment.get_voxel(0, 0, 0).is_some());
        assert!(fragment.get_voxel(1, 1, 1).is_some());
    }

    #[test]
    fn test_split_aabb_preserves_voxel_data() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        world.set_voxel(5, 5, 5, Voxel::new(100, 150, 200, 128));

        let fragment = world.split_aabb(IVec3::new(5, 5, 5), IVec3::new(6, 6, 6));

        let voxel = fragment.get_voxel(0, 0, 0).unwrap();
        assert_eq!(voxel.color, [100, 150, 200]);
        assert_eq!(voxel.emission, 128);
    }

    #[test]
    fn test_split_aabb_cross_chunk_boundary() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        // Voxels spanning chunk boundary (chunk size = 32)
        for x in 30..34 {
            world.set_voxel(x, 0, 0, Voxel::solid(255, 0, 0));
        }
        assert_eq!(world.chunk_count(), 2); // Chunks (0,0,0) and (1,0,0)

        let fragment = world.split_aabb(IVec3::new(30, 0, 0), IVec3::new(34, 1, 1));

        assert_eq!(fragment.total_voxel_count(), 4);
        assert_eq!(world.total_voxel_count(), 0);
    }

    // ========================================================================
    // split_sphere tests (Phase 22.2)
    // ========================================================================

    #[test]
    fn test_split_sphere_basic() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        // Create solid 10x10x10 cube centered at (5,5,5)
        for x in 0..10 {
            for y in 0..10 {
                for z in 0..10 {
                    world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }

        // Split sphere of radius 2 at center (5,5,5)
        let fragment = world.split_sphere(IVec3::new(5, 5, 5), 2);

        // Sphere of radius 2: approximately 33 voxels (4/3 * pi * 2^3 ~ 33)
        // Exact count depends on discrete sampling
        assert!(
            fragment.total_voxel_count() > 20,
            "Expected >20 voxels, got {}",
            fragment.total_voxel_count()
        );
        assert!(
            fragment.total_voxel_count() < 50,
            "Expected <50 voxels, got {}",
            fragment.total_voxel_count()
        );

        // Center voxel should be at (0,0,0) in fragment (relative to center)
        assert!(
            fragment.get_voxel(0, 0, 0).is_some(),
            "Center voxel should exist at origin"
        );

        // Original should have hole
        assert!(
            world.get_voxel(5, 5, 5).is_none(),
            "Original should have hole at center"
        );
    }

    #[test]
    fn test_split_sphere_at_edge() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        // Floor at y=0
        for x in 0..20 {
            for z in 0..20 {
                world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
            }
        }

        // Sphere at floor level - should only get hemisphere
        let fragment = world.split_sphere(IVec3::new(10, 0, 10), 3);

        // Should get roughly half a sphere (floor cuts it) plus center point
        // A sphere of radius 3 has ~113 voxels (4/3 * pi * 3^3)
        // We only have 1 layer (y=0), so we get a circular slice
        // Circle of radius 3: pi * 3^2 ~ 28 voxels
        assert!(
            fragment.total_voxel_count() > 10,
            "Expected >10 voxels, got {}",
            fragment.total_voxel_count()
        );
        assert!(
            fragment.total_voxel_count() < 40,
            "Expected <40 voxels, got {}",
            fragment.total_voxel_count()
        );
    }

    // ========================================================================
    // merge_from tests (Phase 22.3)
    // ========================================================================

    #[test]
    fn test_merge_from_empty_into_empty() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        let other = VoxelWorld::new();

        world.merge_from(&other, IVec3::ZERO);

        assert_eq!(world.total_voxel_count(), 0);
    }

    #[test]
    fn test_merge_from_basic() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        let mut other = VoxelWorld::new();

        other.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        other.set_voxel(1, 0, 0, Voxel::solid(0, 255, 0));

        world.merge_from(&other, IVec3::new(10, 10, 10));

        assert_eq!(world.total_voxel_count(), 2);
        assert!(world.get_voxel(10, 10, 10).is_some());
        assert!(world.get_voxel(11, 10, 10).is_some());
    }

    #[test]
    fn test_merge_from_overwrites() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0)); // Red

        let mut other = VoxelWorld::new();
        other.set_voxel(0, 0, 0, Voxel::solid(0, 0, 255)); // Blue

        world.merge_from(&other, IVec3::new(5, 5, 5));

        let voxel = world.get_voxel(5, 5, 5).unwrap();
        assert_eq!(voxel.color, [0, 0, 255]); // Should be blue (overwritten)
    }

    #[test]
    fn test_merge_from_negative_offset() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        let mut other = VoxelWorld::new();

        other.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));

        world.merge_from(&other, IVec3::new(-10, -10, -10));

        assert!(world.get_voxel(-5, -5, -5).is_some());
    }

    #[test]
    fn test_split_then_merge_roundtrip() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }
        let original_count = world.total_voxel_count();

        // Split out a piece
        let fragment = world.split_aabb(IVec3::new(1, 1, 1), IVec3::new(3, 3, 3));
        let _fragment_count = fragment.total_voxel_count();

        // Merge back at same location
        world.merge_from(&fragment, IVec3::new(1, 1, 1));

        assert_eq!(world.total_voxel_count(), original_count);
    }

    // ========================================================================
    // translate and centroid tests (Phase 22.4)
    // ========================================================================

    #[test]
    fn test_translate_basic() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        world.set_voxel(1, 0, 0, Voxel::solid(0, 255, 0));

        world.translate(IVec3::new(10, 20, 30));

        assert!(world.get_voxel(0, 0, 0).is_none());
        assert!(world.get_voxel(10, 20, 30).is_some());
        assert!(world.get_voxel(11, 20, 30).is_some());
    }

    #[test]
    fn test_centroid_single_voxel() {
        let mut world = VoxelWorld::new();
        world.set_voxel(10, 20, 30, Voxel::solid(255, 0, 0));

        let centroid = world.centroid().unwrap();

        // Centroid should be center of voxel (10.5, 20.5, 30.5)
        assert!((centroid.x - 10.5).abs() < 0.01);
        assert!((centroid.y - 20.5).abs() < 0.01);
        assert!((centroid.z - 30.5).abs() < 0.01);
    }

    #[test]
    fn test_centroid_symmetric() {
        let mut world = VoxelWorld::new();
        // 2x2x2 cube at origin
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }

        let centroid = world.centroid().unwrap();

        // Centroid should be at (1, 1, 1) - average of (0.5,0.5,0.5) to (1.5,1.5,1.5)
        assert!((centroid.x - 1.0).abs() < 0.01);
        assert!((centroid.y - 1.0).abs() < 0.01);
        assert!((centroid.z - 1.0).abs() < 0.01);
    }

    // ========================================================================
    // check_merge_collisions tests (Phase 22.5)
    // ========================================================================

    #[test]
    fn test_check_merge_collisions_none() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));

        let mut other = VoxelWorld::new();
        other.set_voxel(0, 0, 0, Voxel::solid(0, 255, 0));

        // No collision - different positions
        let collisions = world.check_merge_collisions(&other, IVec3::new(10, 0, 0));
        assert!(collisions.is_empty());
    }

    #[test]
    fn test_check_merge_collisions_overlap() {
        use bevy::prelude::IVec3;

        let mut world = VoxelWorld::new();
        world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));

        let mut other = VoxelWorld::new();
        other.set_voxel(0, 0, 0, Voxel::solid(0, 255, 0));

        // Collision at (5,5,5)
        let collisions = world.check_merge_collisions(&other, IVec3::new(5, 5, 5));
        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0], IVec3::new(5, 5, 5));
    }
}
