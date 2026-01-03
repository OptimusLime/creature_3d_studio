//! Chunk streaming for distance-based load/unload of voxel chunks.
//!
//! This module provides systems for dynamically loading and unloading chunks
//! based on camera position, enabling infinite voxel worlds.
//!
//! ## Overview
//!
//! The streaming system maintains a set of "loaded" chunks around the camera:
//! - Chunks within `load_radius` are loaded and rendered
//! - Chunks beyond `unload_radius` are unloaded to save memory
//! - Hysteresis (unload_radius > load_radius) prevents thrashing at boundaries
//!
//! ## Usage
//!
//! ```ignore
//! use studio_core::chunk_streaming::{ChunkStreamingPlugin, ChunkStreamingConfig, ChunkManager};
//!
//! fn setup(mut commands: Commands) {
//!     // Configure streaming
//!     commands.insert_resource(ChunkStreamingConfig {
//!         load_radius: 3,      // Load chunks within 3 chunk-lengths
//!         unload_radius: 5,    // Unload chunks beyond 5 chunk-lengths
//!         max_loads_per_frame: 2,
//!         max_unloads_per_frame: 4,
//!     });
//!
//!     // ChunkManager tracks what's loaded
//!     commands.insert_resource(ChunkManager::new(my_voxel_world));
//! }
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     CHUNK STREAMING                              │
//! └─────────────────────────────────────────────────────────────────┘
//!
//!                    Camera at world position
//!                            │
//!                            ▼
//!     ┌──────────────────────┴──────────────────────┐
//!     │     Convert to chunk coordinates            │
//!     │     camera_chunk = floor(camera_pos / 32)   │
//!     └──────────────────────┬──────────────────────┘
//!                            │
//!        ┌───────────────────┼───────────────────┐
//!        ▼                   ▼                   ▼
//!   ┌─────────┐        ┌─────────┐        ┌─────────┐
//!   │ UNLOAD  │        │ Current │        │  LOAD   │
//!   │ beyond  │◀──────▶│ camera  │◀──────▶│ within  │
//!   │ radius  │        │  chunk  │        │ radius  │
//!   └─────────┘        └─────────┘        └─────────┘
//!       │                                      │
//!       ▼                                      ▼
//!   Despawn entity                        Spawn entity
//!   Remove from loaded                    Add to loaded
//! ```

use bevy::prelude::*;
use std::collections::HashMap;

use crate::deferred::DeferredRenderable;
use crate::voxel::{ChunkPos, VoxelChunk, VoxelWorld, CHUNK_SIZE};
use crate::voxel_mesh::{build_single_chunk_mesh, VoxelMaterial};

/// Configuration for chunk streaming behavior.
#[derive(Resource, Debug, Clone)]
pub struct ChunkStreamingConfig {
    /// Chunks within this radius (in chunk units) from camera are loaded.
    /// Default: 3 (loads a 7x7x7 area = 343 chunks max)
    pub load_radius: i32,

    /// Chunks beyond this radius (in chunk units) from camera are unloaded.
    /// Should be > load_radius to prevent thrashing.
    /// Default: 5
    pub unload_radius: i32,

    /// Maximum chunks to load per frame (rate limiting).
    /// Prevents frame spikes when teleporting.
    /// Default: 2
    pub max_loads_per_frame: usize,

    /// Maximum chunks to unload per frame.
    /// Default: 4
    pub max_unloads_per_frame: usize,

    /// Whether to use greedy meshing (true) or face culling only (false).
    /// Default: true
    pub use_greedy_meshing: bool,

    /// Y range to consider for loading (min, max chunk Y).
    /// None = load all Y levels.
    /// Some((0, 2)) = only load chunks at Y=0, 1, 2.
    /// Default: Some((-1, 2)) for typical ground-level worlds
    pub y_range: Option<(i32, i32)>,
}

impl Default for ChunkStreamingConfig {
    fn default() -> Self {
        Self {
            load_radius: 3,
            unload_radius: 5,
            max_loads_per_frame: 2,
            max_unloads_per_frame: 4,
            use_greedy_meshing: true,
            y_range: Some((-1, 2)),
        }
    }
}

impl ChunkStreamingConfig {
    /// Create config for a small viewing area (good for testing).
    pub fn small() -> Self {
        Self {
            load_radius: 2,
            unload_radius: 3,
            max_loads_per_frame: 1,
            max_unloads_per_frame: 2,
            ..Default::default()
        }
    }

    /// Create config for a large viewing area.
    pub fn large() -> Self {
        Self {
            load_radius: 6,
            unload_radius: 8,
            max_loads_per_frame: 4,
            max_unloads_per_frame: 8,
            ..Default::default()
        }
    }

    /// Create config with unlimited Y range (for flying/3D worlds).
    pub fn unlimited_y(mut self) -> Self {
        self.y_range = None;
        self
    }

    /// Set the Y range for loading.
    pub fn with_y_range(mut self, min_y: i32, max_y: i32) -> Self {
        self.y_range = Some((min_y, max_y));
        self
    }
}

/// Marker component for chunk mesh entities.
///
/// Attached to entities spawned by the streaming system so they can be tracked and despawned.
#[derive(Component, Debug)]
pub struct ChunkEntity {
    /// The chunk position this entity represents.
    pub chunk_pos: ChunkPos,
}

/// Manages chunk loading state and entity tracking.
///
/// This resource maintains:
/// - The source VoxelWorld with all chunk data
/// - Which chunks are currently loaded (have mesh entities)
/// - Entity handles for loaded chunks (for despawning)
#[derive(Resource)]
pub struct ChunkManager {
    /// The voxel world containing all chunk data.
    /// Chunks are loaded from here when they come into range.
    world: VoxelWorld,

    /// Currently loaded chunks and their mesh entities.
    loaded_chunks: HashMap<ChunkPos, Entity>,

    /// Chunks queued for loading (sorted by distance, nearest first).
    load_queue: Vec<ChunkPos>,

    /// Last camera chunk position (for detecting movement).
    last_camera_chunk: Option<ChunkPos>,

    /// Statistics for debugging.
    pub stats: StreamingStats,
}

/// Statistics about streaming activity.
#[derive(Debug, Default, Clone)]
pub struct StreamingStats {
    /// Total chunks currently loaded.
    pub loaded_count: usize,
    /// Chunks loaded this frame.
    pub loaded_this_frame: usize,
    /// Chunks unloaded this frame.
    pub unloaded_this_frame: usize,
    /// Total chunks in the world.
    pub total_chunks: usize,
    /// Current camera chunk position.
    pub camera_chunk: ChunkPos,
}

impl ChunkManager {
    /// Create a new chunk manager with the given voxel world.
    pub fn new(world: VoxelWorld) -> Self {
        let total_chunks = world.chunk_count();
        Self {
            world,
            loaded_chunks: HashMap::new(),
            load_queue: Vec::new(),
            last_camera_chunk: None,
            stats: StreamingStats {
                total_chunks,
                ..Default::default()
            },
        }
    }

    /// Get reference to the voxel world.
    pub fn world(&self) -> &VoxelWorld {
        &self.world
    }

    /// Get mutable reference to the voxel world.
    pub fn world_mut(&mut self) -> &mut VoxelWorld {
        &mut self.world
    }

    /// Check if a chunk is currently loaded.
    pub fn is_loaded(&self, pos: ChunkPos) -> bool {
        self.loaded_chunks.contains_key(&pos)
    }

    /// Get the entity for a loaded chunk.
    pub fn get_entity(&self, pos: ChunkPos) -> Option<Entity> {
        self.loaded_chunks.get(&pos).copied()
    }

    /// Get all loaded chunk positions.
    pub fn loaded_positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.loaded_chunks.keys().copied()
    }

    /// Get the number of loaded chunks.
    pub fn loaded_count(&self) -> usize {
        self.loaded_chunks.len()
    }

    /// Mark a chunk as loaded with its entity.
    fn mark_loaded(&mut self, pos: ChunkPos, entity: Entity) {
        self.loaded_chunks.insert(pos, entity);
        self.stats.loaded_count = self.loaded_chunks.len();
    }

    /// Mark a chunk as unloaded.
    fn mark_unloaded(&mut self, pos: ChunkPos) -> Option<Entity> {
        let entity = self.loaded_chunks.remove(&pos);
        self.stats.loaded_count = self.loaded_chunks.len();
        entity
    }

    /// Get the chunk data for a position, if it exists in the world.
    pub fn get_chunk(&self, pos: ChunkPos) -> Option<&VoxelChunk> {
        self.world.get_chunk(pos)
    }

    /// Rebuild the load queue based on camera position.
    fn rebuild_load_queue(&mut self, camera_chunk: ChunkPos, config: &ChunkStreamingConfig) {
        self.load_queue.clear();

        let radius = config.load_radius;
        let (min_y, max_y) = config
            .y_range
            .unwrap_or((camera_chunk.y - radius, camera_chunk.y + radius));

        // Find all chunks within load radius that aren't loaded yet
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                for dz in -radius..=radius {
                    let pos = ChunkPos::new(
                        camera_chunk.x + dx,
                        camera_chunk.y + dy,
                        camera_chunk.z + dz,
                    );

                    // Check Y range constraint
                    if pos.y < min_y || pos.y > max_y {
                        continue;
                    }

                    // Skip if already loaded
                    if self.is_loaded(pos) {
                        continue;
                    }

                    // Skip if chunk doesn't exist in world
                    if !self.world.has_chunk(pos) {
                        continue;
                    }

                    // Check if within spherical radius (not just cubic)
                    let dist_sq = dx * dx + dy * dy + dz * dz;
                    if dist_sq <= radius * radius {
                        self.load_queue.push(pos);
                    }
                }
            }
        }

        // Sort by distance descending (furthest first, nearest at end for pop())
        self.load_queue.sort_by_key(|pos| {
            let dx = pos.x - camera_chunk.x;
            let dy = pos.y - camera_chunk.y;
            let dz = pos.z - camera_chunk.z;
            std::cmp::Reverse(dx * dx + dy * dy + dz * dz)
        });
    }
}

/// Convert world position to chunk position.
pub fn world_pos_to_chunk(world_pos: Vec3) -> ChunkPos {
    ChunkPos::new(
        (world_pos.x / CHUNK_SIZE as f32).floor() as i32,
        (world_pos.y / CHUNK_SIZE as f32).floor() as i32,
        (world_pos.z / CHUNK_SIZE as f32).floor() as i32,
    )
}

/// Resource holding the shared material handle for chunk meshes.
///
/// Must be inserted before `chunk_streaming_system` runs.
#[derive(Resource)]
pub struct ChunkMaterialHandle(pub Handle<VoxelMaterial>);

/// System that handles chunk loading and unloading based on camera position.
///
/// Run this in `Update` to continuously stream chunks.
///
/// Requires `ChunkMaterialHandle` resource to be inserted with a valid material.
pub fn chunk_streaming_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut manager: ResMut<ChunkManager>,
    config: Res<ChunkStreamingConfig>,
    material_handle: Option<Res<ChunkMaterialHandle>>,
    camera_query: Query<&Transform, With<Camera3d>>,
    chunk_entities: Query<(Entity, &ChunkEntity)>,
) {
    // Need material handle to spawn chunks
    let Some(material_handle) = material_handle else {
        return;
    };
    // Get camera position
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    let camera_pos = camera_transform.translation;
    let camera_chunk = world_pos_to_chunk(camera_pos);

    // Reset per-frame stats
    manager.stats.loaded_this_frame = 0;
    manager.stats.unloaded_this_frame = 0;
    manager.stats.camera_chunk = camera_chunk;

    // Check if camera moved to a new chunk
    let camera_moved = manager.last_camera_chunk != Some(camera_chunk);
    if camera_moved {
        manager.last_camera_chunk = Some(camera_chunk);
        manager.rebuild_load_queue(camera_chunk, &config);
    }

    // === UNLOAD: Remove chunks beyond unload_radius ===
    let unload_radius_sq = config.unload_radius * config.unload_radius;
    let mut to_unload: Vec<ChunkPos> = Vec::new();

    for (_entity, chunk_entity) in chunk_entities.iter() {
        let pos = chunk_entity.chunk_pos;
        let dx = pos.x - camera_chunk.x;
        let dy = pos.y - camera_chunk.y;
        let dz = pos.z - camera_chunk.z;
        let dist_sq = dx * dx + dy * dy + dz * dz;

        if dist_sq > unload_radius_sq {
            to_unload.push(pos);
            if to_unload.len() >= config.max_unloads_per_frame {
                break;
            }
        }
    }

    for pos in to_unload {
        if let Some(entity) = manager.mark_unloaded(pos) {
            commands.entity(entity).despawn();
            manager.stats.unloaded_this_frame += 1;
        }
    }

    // === LOAD: Add chunks within load_radius ===
    let mut loads_this_frame = 0;

    while loads_this_frame < config.max_loads_per_frame {
        let Some(pos) = manager.load_queue.pop() else {
            break;
        };

        // Double-check it's still valid
        if manager.is_loaded(pos) {
            continue;
        }

        let Some(chunk) = manager.world.get_chunk(pos) else {
            continue;
        };

        if chunk.is_empty() {
            continue;
        }

        // Build mesh for this chunk
        let chunk_mesh = build_single_chunk_mesh(chunk, pos, config.use_greedy_meshing);
        let translation = chunk_mesh.translation();

        // Spawn entity with material for deferred rendering
        let entity = commands
            .spawn((
                Mesh3d(meshes.add(chunk_mesh.mesh)),
                MeshMaterial3d(material_handle.0.clone()),
                Transform::from_translation(translation),
                DeferredRenderable,
                ChunkEntity { chunk_pos: pos },
            ))
            .id();

        manager.mark_loaded(pos, entity);
        loads_this_frame += 1;
        manager.stats.loaded_this_frame += 1;
    }
}

/// System that logs streaming statistics (for debugging).
pub fn chunk_streaming_debug_system(manager: Res<ChunkManager>) {
    if manager.is_changed() {
        let stats = &manager.stats;
        if stats.loaded_this_frame > 0 || stats.unloaded_this_frame > 0 {
            info!(
                "Chunks: {} loaded ({} total in world) | +{} -{} this frame | camera at {:?}",
                stats.loaded_count,
                stats.total_chunks,
                stats.loaded_this_frame,
                stats.unloaded_this_frame,
                stats.camera_chunk
            );
        }
    }
}

/// Plugin that sets up chunk streaming systems.
///
/// Adds:
/// - `chunk_streaming_system` in Update
/// - Default `ChunkStreamingConfig` (if not already present)
///
/// You must insert a `ChunkManager` resource with your world data.
pub struct ChunkStreamingPlugin;

impl Plugin for ChunkStreamingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkStreamingConfig>()
            .add_systems(
                Update,
                chunk_streaming_system.run_if(resource_exists::<ChunkManager>),
            )
            .add_systems(
                Update,
                chunk_streaming_debug_system.run_if(resource_exists::<ChunkManager>),
            );
    }
}

/// Immediately load all chunks within radius (bypasses rate limiting).
///
/// Useful for initial scene setup before the game loop starts.
pub fn load_all_chunks_in_radius(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<VoxelMaterial>,
    manager: &mut ChunkManager,
    camera_pos: Vec3,
    radius: i32,
    use_greedy: bool,
) {
    let camera_chunk = world_pos_to_chunk(camera_pos);
    let radius_sq = radius * radius;

    for dx in -radius..=radius {
        for dy in -radius..=radius {
            for dz in -radius..=radius {
                let dist_sq = dx * dx + dy * dy + dz * dz;
                if dist_sq > radius_sq {
                    continue;
                }

                let pos = ChunkPos::new(
                    camera_chunk.x + dx,
                    camera_chunk.y + dy,
                    camera_chunk.z + dz,
                );

                if manager.is_loaded(pos) {
                    continue;
                }

                let Some(chunk) = manager.world.get_chunk(pos) else {
                    continue;
                };

                if chunk.is_empty() {
                    continue;
                }

                let chunk_mesh = build_single_chunk_mesh(chunk, pos, use_greedy);
                let translation = chunk_mesh.translation();

                let entity = commands
                    .spawn((
                        Mesh3d(meshes.add(chunk_mesh.mesh)),
                        MeshMaterial3d(material.clone()),
                        Transform::from_translation(translation),
                        DeferredRenderable,
                        ChunkEntity { chunk_pos: pos },
                    ))
                    .id();

                manager.mark_loaded(pos, entity);
            }
        }
    }

    info!(
        "Initial load: {} chunks loaded around {:?}",
        manager.loaded_count(),
        camera_chunk
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::Voxel;

    #[test]
    fn test_world_pos_to_chunk() {
        // Origin
        assert_eq!(world_pos_to_chunk(Vec3::ZERO), ChunkPos::new(0, 0, 0));

        // Just inside chunk 0
        assert_eq!(
            world_pos_to_chunk(Vec3::new(31.0, 0.0, 0.0)),
            ChunkPos::new(0, 0, 0)
        );

        // Just inside chunk 1
        assert_eq!(
            world_pos_to_chunk(Vec3::new(32.0, 0.0, 0.0)),
            ChunkPos::new(1, 0, 0)
        );

        // Negative coordinates
        assert_eq!(
            world_pos_to_chunk(Vec3::new(-1.0, 0.0, 0.0)),
            ChunkPos::new(-1, 0, 0)
        );
        assert_eq!(
            world_pos_to_chunk(Vec3::new(-32.0, 0.0, 0.0)),
            ChunkPos::new(-1, 0, 0)
        );
        assert_eq!(
            world_pos_to_chunk(Vec3::new(-33.0, 0.0, 0.0)),
            ChunkPos::new(-2, 0, 0)
        );
    }

    #[test]
    fn test_chunk_manager_creation() {
        let world = VoxelWorld::new();
        let manager = ChunkManager::new(world);
        assert_eq!(manager.loaded_count(), 0);
    }

    #[test]
    fn test_chunk_manager_tracking() {
        let mut world = VoxelWorld::new();
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));

        let mut manager = ChunkManager::new(world);
        let pos = ChunkPos::new(0, 0, 0);

        // Simulate loading
        let fake_entity = Entity::from_bits(42);
        manager.mark_loaded(pos, fake_entity);

        assert!(manager.is_loaded(pos));
        assert_eq!(manager.get_entity(pos), Some(fake_entity));
        assert_eq!(manager.loaded_count(), 1);

        // Simulate unloading
        let removed = manager.mark_unloaded(pos);
        assert_eq!(removed, Some(fake_entity));
        assert!(!manager.is_loaded(pos));
        assert_eq!(manager.loaded_count(), 0);
    }

    #[test]
    fn test_streaming_config_defaults() {
        let config = ChunkStreamingConfig::default();
        assert_eq!(config.load_radius, 3);
        assert_eq!(config.unload_radius, 5);
        assert!(
            config.unload_radius > config.load_radius,
            "Hysteresis requires unload > load"
        );
    }

    #[test]
    fn test_load_queue_sorting() {
        let mut world = VoxelWorld::new();

        // Create chunks at various positions
        for x in -2..=2 {
            for z in -2..=2 {
                world.set_voxel(x * 32, 0, z * 32, Voxel::solid(255, 0, 0));
            }
        }

        let mut manager = ChunkManager::new(world);
        let camera_chunk = ChunkPos::new(0, 0, 0);
        let config = ChunkStreamingConfig::default().with_y_range(0, 0);

        manager.rebuild_load_queue(camera_chunk, &config);

        // Queue should be sorted by distance (nearest first at end for pop)
        if manager.load_queue.len() >= 2 {
            let last = manager.load_queue.last().unwrap();
            let first = manager.load_queue.first().unwrap();

            let last_dist = last.x * last.x + last.z * last.z;
            let first_dist = first.x * first.x + first.z * first.z;

            // Last item (nearest) should have smaller distance
            assert!(
                last_dist <= first_dist,
                "Queue should be sorted by distance"
            );
        }
    }
}
