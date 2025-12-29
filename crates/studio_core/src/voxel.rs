//! Voxel data structures for creature modeling.
//!
//! A voxel represents a single unit cube in 3D space with color and emission properties.
//! VoxelChunk stores a 16³ dense array of optional voxels.

/// Size of a voxel chunk in each dimension.
/// 32 allows for reasonably sized test scenes.
/// For larger scenes, multiple chunks would be needed.
pub const CHUNK_SIZE: usize = 32;

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
    /// Get world position given chunk offset.
    /// The position is at the center of the voxel (+0.5 offset).
    pub fn world_position(&self, chunk_offset: [f32; 3]) -> [f32; 3] {
        [
            self.position.0 as f32 + 0.5 + chunk_offset[0],
            self.position.1 as f32 + 0.5 + chunk_offset[1],
            self.position.2 as f32 + 0.5 + chunk_offset[2],
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

        assert!(!chunk.set(16, 0, 0, voxel)); // x out of bounds
        assert!(!chunk.set(0, 16, 0, voxel)); // y out of bounds
        assert!(!chunk.set(0, 0, 16, voxel)); // z out of bounds
        assert_eq!(chunk.get(16, 0, 0), None);
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
}
