//! VoxelLayer and VoxelLayers for layered voxel architecture.
//!
//! This module provides a layer-based system for compositing multiple voxel sources
//! (terrain, generated content, player modifications) into a unified world view.
//!
//! ## Key Concepts
//!
//! - **VoxelLayer**: A single layer with its own VoxelWorld, offset, and dirty tracking
//! - **VoxelLayers**: Resource holding all layers with priority-based compositing
//! - **Layer Offset**: Each layer's local (0,0,0) can map to any world position
//! - **Dirty Tracking**: Automatic tracking of modified chunks for incremental mesh rebuilds
//!
//! ## Example
//!
//! ```ignore
//! let mut layers = VoxelLayers::new();
//!
//! // Terrain at origin
//! layers.get_mut("terrain").unwrap()
//!     .set_voxel(0, 0, 0, Voxel::solid(100, 100, 100));
//!
//! // Generated content offset to sit on terrain
//! let gen = layers.get_mut("generated").unwrap();
//! gen.offset = IVec3::new(5, 1, 5);
//! gen.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));  // Appears at world (5, 1, 5)
//!
//! // Query merged view
//! assert!(layers.get_voxel(5, 1, 5).is_some());
//! ```

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::voxel::{ChunkPos, Voxel, VoxelWorld, CHUNK_SIZE_I32};

/// A layer of voxels that can be positioned anywhere in world space.
/// Multiple layers composite together for rendering and collision.
#[derive(Debug)]
pub struct VoxelLayer {
    /// Human-readable name for debugging ("terrain", "generated", etc.)
    pub name: String,

    /// Priority for compositing. Higher priority layers override lower.
    /// terrain=0, generated=10, player_placed=20
    pub priority: i32,

    /// World offset - layer's local (0,0,0) maps to this world position.
    /// Allows placing generated content anywhere without coordinate math.
    pub offset: IVec3,

    /// The actual voxel data, stored in chunks.
    pub world: VoxelWorld,

    /// Whether this layer renders.
    pub visible: bool,

    /// Whether this layer participates in collision detection.
    pub collidable: bool,

    /// Chunks that have been modified since last mesh rebuild.
    /// Stored as LOCAL chunk positions (before offset applied).
    dirty_chunks: HashSet<ChunkPos>,
}

impl VoxelLayer {
    /// Create a new empty layer with default settings.
    pub fn new(name: &str, priority: i32) -> Self {
        Self {
            name: name.to_string(),
            priority,
            offset: IVec3::ZERO,
            world: VoxelWorld::new(),
            visible: true,
            collidable: true,
            dirty_chunks: HashSet::new(),
        }
    }

    /// Set voxel at layer-local coordinates. Automatically marks chunk dirty.
    pub fn set_voxel(&mut self, x: i32, y: i32, z: i32, voxel: Voxel) {
        self.world.set_voxel(x, y, z, voxel);
        let chunk_pos = ChunkPos::from_world(x, y, z);
        self.dirty_chunks.insert(chunk_pos);
        // Also mark neighbors dirty if at chunk boundary (for face culling)
        self.mark_neighbors_if_boundary(x, y, z, chunk_pos);
    }

    /// Clear voxel at layer-local coordinates. Automatically marks chunk dirty.
    pub fn clear_voxel(&mut self, x: i32, y: i32, z: i32) {
        self.world.clear_voxel(x, y, z);
        let chunk_pos = ChunkPos::from_world(x, y, z);
        self.dirty_chunks.insert(chunk_pos);
        self.mark_neighbors_if_boundary(x, y, z, chunk_pos);
    }

    /// Get voxel at layer-local coordinates.
    pub fn get_voxel(&self, x: i32, y: i32, z: i32) -> Option<Voxel> {
        self.world.get_voxel(x, y, z)
    }

    /// Convert layer-local coords to world coords using offset.
    pub fn local_to_world(&self, local: IVec3) -> IVec3 {
        local + self.offset
    }

    /// Convert world coords to layer-local coords.
    pub fn world_to_local(&self, world: IVec3) -> IVec3 {
        world - self.offset
    }

    /// Check if there are any dirty chunks.
    pub fn has_dirty_chunks(&self) -> bool {
        !self.dirty_chunks.is_empty()
    }

    /// Take dirty chunks (returns set and clears internal tracking).
    /// Returns LOCAL chunk positions.
    pub fn take_dirty_chunks(&mut self) -> HashSet<ChunkPos> {
        std::mem::take(&mut self.dirty_chunks)
    }

    /// Re-mark a chunk as dirty (used when frame budget exceeded).
    pub fn mark_chunk_dirty(&mut self, local_chunk_pos: ChunkPos) {
        self.dirty_chunks.insert(local_chunk_pos);
    }

    /// Clear a rectangular region efficiently.
    pub fn clear_region(&mut self, min: IVec3, max: IVec3) {
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    self.world.clear_voxel(x, y, z);
                }
            }
        }
        // Mark all affected chunks dirty
        let chunk_min = ChunkPos::from_world(min.x, min.y, min.z);
        let chunk_max = ChunkPos::from_world(max.x, max.y, max.z);
        for chunk_pos in ChunkPos::iter_range(chunk_min, chunk_max) {
            self.dirty_chunks.insert(chunk_pos);
        }
    }

    /// Mark neighbor chunks dirty if position is at chunk boundary.
    /// This is needed for correct face culling when a voxel at the edge changes.
    fn mark_neighbors_if_boundary(&mut self, x: i32, y: i32, z: i32, chunk_pos: ChunkPos) {
        let local_x = x.rem_euclid(CHUNK_SIZE_I32);
        let local_y = y.rem_euclid(CHUNK_SIZE_I32);
        let local_z = z.rem_euclid(CHUNK_SIZE_I32);

        // Check each axis for boundary condition
        if local_x == 0 {
            self.dirty_chunks
                .insert(ChunkPos::new(chunk_pos.x - 1, chunk_pos.y, chunk_pos.z));
        }
        if local_x == CHUNK_SIZE_I32 - 1 {
            self.dirty_chunks
                .insert(ChunkPos::new(chunk_pos.x + 1, chunk_pos.y, chunk_pos.z));
        }
        if local_y == 0 {
            self.dirty_chunks
                .insert(ChunkPos::new(chunk_pos.x, chunk_pos.y - 1, chunk_pos.z));
        }
        if local_y == CHUNK_SIZE_I32 - 1 {
            self.dirty_chunks
                .insert(ChunkPos::new(chunk_pos.x, chunk_pos.y + 1, chunk_pos.z));
        }
        if local_z == 0 {
            self.dirty_chunks
                .insert(ChunkPos::new(chunk_pos.x, chunk_pos.y, chunk_pos.z - 1));
        }
        if local_z == CHUNK_SIZE_I32 - 1 {
            self.dirty_chunks
                .insert(ChunkPos::new(chunk_pos.x, chunk_pos.y, chunk_pos.z + 1));
        }
    }
}

/// Bevy resource containing all voxel layers.
/// Handles merging for render and collision queries.
#[derive(Resource)]
pub struct VoxelLayers {
    /// Layers sorted by priority (lowest first for iteration).
    layers: Vec<VoxelLayer>,
}

impl Default for VoxelLayers {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxelLayers {
    /// Create standard layer setup with terrain and generated layers.
    pub fn new() -> Self {
        Self {
            layers: vec![
                VoxelLayer::new("terrain", 0),
                VoxelLayer::new("generated", 10),
            ],
        }
    }

    /// Create with custom layers.
    pub fn with_layers(layers: Vec<VoxelLayer>) -> Self {
        let mut s = Self { layers };
        s.sort_by_priority();
        s
    }

    /// Sort layers by priority (lowest first).
    fn sort_by_priority(&mut self) {
        self.layers.sort_by_key(|l| l.priority);
    }

    /// Get reference to layer by name.
    pub fn get(&self, name: &str) -> Option<&VoxelLayer> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Get mutable reference to layer by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut VoxelLayer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }

    /// Iterate layers by priority (lowest first).
    pub fn iter_by_priority(&self) -> impl Iterator<Item = &VoxelLayer> {
        self.layers.iter()
    }

    /// Get voxel at world position, checking layers by priority (highest first).
    /// Returns first non-empty voxel found.
    pub fn get_voxel(&self, world_x: i32, world_y: i32, world_z: i32) -> Option<Voxel> {
        // Iterate in reverse (highest priority first)
        for layer in self.layers.iter().rev() {
            if !layer.visible {
                continue;
            }
            let local = layer.world_to_local(IVec3::new(world_x, world_y, world_z));
            if let Some(voxel) = layer.world.get_voxel(local.x, local.y, local.z) {
                return Some(voxel);
            }
        }
        None
    }

    /// Check if position is solid in any collidable layer.
    pub fn is_solid(&self, world_x: i32, world_y: i32, world_z: i32) -> bool {
        for layer in self.layers.iter().rev() {
            if !layer.collidable {
                continue;
            }
            let local = layer.world_to_local(IVec3::new(world_x, world_y, world_z));
            if layer.world.is_solid(local.x, local.y, local.z) {
                return true;
            }
        }
        false
    }

    /// Collect all dirty chunk positions across all layers.
    /// Returns WORLD-space chunk positions (with layer offsets applied).
    pub fn collect_dirty_chunks(&mut self) -> HashSet<ChunkPos> {
        let mut all_dirty = HashSet::new();
        for layer in &mut self.layers {
            for local_chunk in layer.take_dirty_chunks() {
                // Convert local chunk origin to world space
                let local_origin = IVec3::new(
                    local_chunk.x * CHUNK_SIZE_I32,
                    local_chunk.y * CHUNK_SIZE_I32,
                    local_chunk.z * CHUNK_SIZE_I32,
                );
                let world_origin = layer.local_to_world(local_origin);
                let world_chunk =
                    ChunkPos::from_world(world_origin.x, world_origin.y, world_origin.z);
                all_dirty.insert(world_chunk);
            }
        }
        all_dirty
    }

    /// Get all layers (for iteration).
    pub fn layers(&self) -> &[VoxelLayer] {
        &self.layers
    }
}

// ============================================================================
// ChunkEntityMap - tracks mesh entities for incremental updates
// ============================================================================

/// Maps world chunk positions to their mesh entities.
/// Used by update_dirty_chunks to know which entities to rebuild.
#[derive(Resource, Default)]
pub struct ChunkEntityMap {
    /// World chunk position → mesh entity
    chunks: HashMap<ChunkPos, Entity>,
}

impl ChunkEntityMap {
    /// Create a new empty map.
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }

    /// Register a chunk entity.
    pub fn register(&mut self, pos: ChunkPos, entity: Entity) {
        self.chunks.insert(pos, entity);
    }

    /// Get entity for chunk position.
    pub fn get(&self, pos: &ChunkPos) -> Option<Entity> {
        self.chunks.get(pos).copied()
    }

    /// Remove and return entity for chunk position.
    pub fn remove(&mut self, pos: &ChunkPos) -> Option<Entity> {
        self.chunks.remove(pos)
    }

    /// Check if a chunk is registered.
    pub fn contains(&self, pos: &ChunkPos) -> bool {
        self.chunks.contains_key(pos)
    }

    /// Iterate all registered chunks.
    pub fn iter(&self) -> impl Iterator<Item = (&ChunkPos, &Entity)> {
        self.chunks.iter()
    }

    /// Number of registered chunks.
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Clear all registrations.
    pub fn clear(&mut self) {
        self.chunks.clear();
    }
}

// ============================================================================
// Bevy Systems for dirty chunk updates
// ============================================================================

use crate::voxel_mesh::{
    build_merged_chunk_mesh, ATTRIBUTE_VOXEL_AO, ATTRIBUTE_VOXEL_COLOR, ATTRIBUTE_VOXEL_EMISSION,
};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};

/// System that rebuilds mesh for any dirty chunks.
/// Runs every frame, only does work if chunks are dirty.
///
/// This system should be added to your app:
/// ```ignore
/// app.add_systems(Update, update_dirty_chunks);
/// ```
pub fn update_dirty_chunks(
    mut layers: ResMut<VoxelLayers>,
    chunk_map: Res<ChunkEntityMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mesh_query: Query<&Mesh3d>,
) {
    let dirty = layers.collect_dirty_chunks();
    if dirty.is_empty() {
        return;
    }

    info!("Rebuilding {} dirty chunks", dirty.len());

    for chunk_pos in dirty {
        // Get existing entity for this chunk
        let Some(entity) = chunk_map.get(&chunk_pos) else {
            // No entity yet - will be created by initial spawn or separate system
            debug!("No entity registered for chunk {:?}", chunk_pos);
            continue;
        };

        // Get mesh handle from entity
        let Ok(mesh_handle) = mesh_query.get(entity) else {
            warn!("Chunk entity {:?} missing Mesh3d component", entity);
            continue;
        };

        // Build merged chunk mesh from all layers
        if let Some(chunk_mesh) = build_merged_chunk_mesh(&layers, chunk_pos, true) {
            // Rebuild mesh in place
            if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
                *mesh = chunk_mesh.mesh;
            }
        } else {
            // Chunk is now empty - clear the mesh to an empty mesh
            if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
                // Create empty mesh with same topology
                let mut empty = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::default(),
                );
                empty.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
                empty.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
                empty.insert_attribute(ATTRIBUTE_VOXEL_COLOR, Vec::<[f32; 3]>::new());
                empty.insert_attribute(ATTRIBUTE_VOXEL_EMISSION, Vec::<f32>::new());
                empty.insert_attribute(ATTRIBUTE_VOXEL_AO, Vec::<f32>::new());
                empty.insert_indices(Indices::U32(Vec::new()));
                *mesh = empty;
            }
        }
    }
}

/// Plugin that registers VoxelLayers resources and systems.
pub struct VoxelLayersPlugin;

impl Plugin for VoxelLayersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelLayers>()
            .init_resource::<ChunkEntityMap>()
            .add_systems(Update, update_dirty_chunks);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_coordinate_transform() {
        let mut layer = VoxelLayer::new("test", 0);
        layer.offset = IVec3::new(100, 50, 200);

        // Local (0,0,0) → World (100,50,200)
        assert_eq!(layer.local_to_world(IVec3::ZERO), IVec3::new(100, 50, 200));

        // Local (5,3,7) → World (105,53,207)
        assert_eq!(
            layer.local_to_world(IVec3::new(5, 3, 7)),
            IVec3::new(105, 53, 207)
        );

        // World (100,50,200) → Local (0,0,0)
        assert_eq!(layer.world_to_local(IVec3::new(100, 50, 200)), IVec3::ZERO);

        // World (105,53,207) → Local (5,3,7)
        assert_eq!(
            layer.world_to_local(IVec3::new(105, 53, 207)),
            IVec3::new(5, 3, 7)
        );
    }

    #[test]
    fn test_set_voxel_marks_dirty() {
        let mut layer = VoxelLayer::new("test", 0);
        assert!(!layer.has_dirty_chunks());

        layer.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
        assert!(layer.has_dirty_chunks());

        let dirty = layer.take_dirty_chunks();
        assert_eq!(dirty.len(), 1);
        assert!(dirty.contains(&ChunkPos::from_world(5, 5, 5)));

        // After take, should be empty
        assert!(!layer.has_dirty_chunks());
    }

    #[test]
    fn test_boundary_marks_neighbor_dirty() {
        let mut layer = VoxelLayer::new("test", 0);

        // Voxel at x=31 (chunk boundary, CHUNK_SIZE=32)
        layer.set_voxel(31, 16, 16, Voxel::solid(255, 0, 0));

        let dirty = layer.take_dirty_chunks();
        // Should mark both chunk (0,0,0) and neighbor chunk (1,0,0)
        assert!(dirty.contains(&ChunkPos::new(0, 0, 0)));
        assert!(dirty.contains(&ChunkPos::new(1, 0, 0)));
    }

    #[test]
    fn test_boundary_x_0_marks_neighbor() {
        let mut layer = VoxelLayer::new("test", 0);

        // Voxel at x=0 (chunk boundary on negative side)
        layer.set_voxel(0, 16, 16, Voxel::solid(255, 0, 0));

        let dirty = layer.take_dirty_chunks();
        // Should mark chunk (0,0,0) and neighbor chunk (-1,0,0)
        assert!(dirty.contains(&ChunkPos::new(0, 0, 0)));
        assert!(dirty.contains(&ChunkPos::new(-1, 0, 0)));
    }

    #[test]
    fn test_voxel_layers_priority() {
        let mut layers = VoxelLayers::new();

        // Terrain layer (priority 0): red voxel at (5,5,5)
        layers
            .get_mut("terrain")
            .unwrap()
            .set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));

        // Generated layer (priority 10): blue voxel at same position
        layers
            .get_mut("generated")
            .unwrap()
            .set_voxel(5, 5, 5, Voxel::solid(0, 0, 255));

        // Merged result should be blue (higher priority wins)
        let voxel = layers.get_voxel(5, 5, 5).unwrap();
        assert_eq!(voxel.color, [0, 0, 255]);
    }

    #[test]
    fn test_voxel_layers_offset() {
        let mut layers = VoxelLayers::new();

        // Generated layer offset at (100, 0, 0)
        let gen = layers.get_mut("generated").unwrap();
        gen.offset = IVec3::new(100, 0, 0);
        gen.set_voxel(5, 5, 5, Voxel::solid(0, 255, 0)); // Local coords

        // Should appear at world (105, 5, 5)
        assert!(layers.get_voxel(105, 5, 5).is_some());
        // Should NOT appear at local coords in world space
        assert!(layers.get_voxel(5, 5, 5).is_none());
    }

    #[test]
    fn test_is_solid_respects_offset() {
        let mut layers = VoxelLayers::new();

        let gen = layers.get_mut("generated").unwrap();
        gen.offset = IVec3::new(100, 50, 200);
        gen.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));

        // Should be solid at world position
        assert!(layers.is_solid(100, 50, 200));
        // Should NOT be solid at origin
        assert!(!layers.is_solid(0, 0, 0));
    }

    #[test]
    fn test_collect_dirty_chunks_with_offset() {
        let mut layers = VoxelLayers::new();

        // Set voxel in generated layer with offset
        let gen = layers.get_mut("generated").unwrap();
        gen.offset = IVec3::new(100, 0, 0);
        gen.set_voxel(5, 5, 5, Voxel::solid(0, 255, 0)); // Local (5,5,5)

        // Dirty chunk should be at WORLD position
        let dirty = layers.collect_dirty_chunks();
        // World pos 105 / 32 = chunk 3
        assert!(dirty.contains(&ChunkPos::from_world(105, 5, 5)));
        // Should NOT contain local chunk pos
        assert!(!dirty.contains(&ChunkPos::from_world(5, 5, 5)));
    }

    #[test]
    fn test_clear_region() {
        let mut layer = VoxelLayer::new("test", 0);

        // Set some voxels
        for x in 0..5 {
            for y in 0..5 {
                for z in 0..5 {
                    layer.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
                }
            }
        }
        layer.take_dirty_chunks(); // Clear dirty state

        // Clear a region
        layer.clear_region(IVec3::new(1, 1, 1), IVec3::new(3, 3, 3));

        // Corners should still exist
        assert!(layer.get_voxel(0, 0, 0).is_some());
        assert!(layer.get_voxel(4, 4, 4).is_some());

        // Center should be cleared
        assert!(layer.get_voxel(2, 2, 2).is_none());

        // Should have dirty chunks
        assert!(layer.has_dirty_chunks());
    }

    #[test]
    fn test_layer_visibility() {
        let mut layers = VoxelLayers::new();

        // Set voxel in generated layer
        layers
            .get_mut("generated")
            .unwrap()
            .set_voxel(5, 5, 5, Voxel::solid(0, 255, 0));

        // Should be visible
        assert!(layers.get_voxel(5, 5, 5).is_some());

        // Hide the layer
        layers.get_mut("generated").unwrap().visible = false;

        // Should no longer be visible
        assert!(layers.get_voxel(5, 5, 5).is_none());
    }

    #[test]
    fn test_layer_collidable() {
        let mut layers = VoxelLayers::new();

        // Set voxel in generated layer
        layers
            .get_mut("generated")
            .unwrap()
            .set_voxel(5, 5, 5, Voxel::solid(0, 255, 0));

        // Should be solid
        assert!(layers.is_solid(5, 5, 5));

        // Make non-collidable
        layers.get_mut("generated").unwrap().collidable = false;

        // Should no longer be solid
        assert!(!layers.is_solid(5, 5, 5));
    }

    // ========================================================================
    // Phase 4: ChunkEntityMap tests
    // ========================================================================

    #[test]
    fn test_chunk_entity_map_basic() {
        let mut map = ChunkEntityMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);

        // Create a fake entity (just for testing the map)
        let entity = Entity::from_bits(42);
        let chunk_pos = ChunkPos::new(1, 2, 3);

        // Register
        map.register(chunk_pos, entity);
        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);
        assert!(map.contains(&chunk_pos));

        // Get
        assert_eq!(map.get(&chunk_pos), Some(entity));
        assert_eq!(map.get(&ChunkPos::new(0, 0, 0)), None);

        // Remove
        let removed = map.remove(&chunk_pos);
        assert_eq!(removed, Some(entity));
        assert!(map.is_empty());
        assert!(!map.contains(&chunk_pos));
    }

    #[test]
    fn test_chunk_entity_map_overwrite() {
        let mut map = ChunkEntityMap::new();
        let chunk_pos = ChunkPos::new(0, 0, 0);

        let entity1 = Entity::from_bits(1);
        let entity2 = Entity::from_bits(2);

        map.register(chunk_pos, entity1);
        assert_eq!(map.get(&chunk_pos), Some(entity1));

        // Overwrite with new entity
        map.register(chunk_pos, entity2);
        assert_eq!(map.get(&chunk_pos), Some(entity2));
        assert_eq!(map.len(), 1); // Still only one entry
    }

    #[test]
    fn test_dirty_tracking_flow() {
        let mut layers = VoxelLayers::new();

        // Initially no dirty chunks
        assert!(layers.collect_dirty_chunks().is_empty());

        // Set voxel in terrain layer
        layers
            .get_mut("terrain")
            .unwrap()
            .set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));

        // Now have dirty chunks
        let dirty = layers.collect_dirty_chunks();
        assert_eq!(dirty.len(), 1);
        assert!(dirty.contains(&ChunkPos::from_world(5, 5, 5)));

        // After collect, dirty is cleared
        assert!(layers.collect_dirty_chunks().is_empty());

        // Set voxel in generated layer with offset
        let gen = layers.get_mut("generated").unwrap();
        gen.offset = IVec3::new(100, 0, 0);
        gen.set_voxel(5, 5, 5, Voxel::solid(0, 255, 0)); // Local (5,5,5)

        // Dirty chunk should be at WORLD position (105, 5, 5)
        let dirty = layers.collect_dirty_chunks();
        assert_eq!(dirty.len(), 1);
        // World pos 105 / 32 = chunk 3
        assert!(dirty.contains(&ChunkPos::from_world(105, 5, 5)));
    }
}
