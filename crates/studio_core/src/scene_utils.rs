//! Scene setup utilities for consistent rendering across all examples.
//!
//! This module provides shared utilities to avoid reimplementing common patterns
//! in every example. It handles:
//! - Automatic point light extraction from emissive voxels
//! - Consistent coordinate transforms between mesh and world space
//! - Standard camera and lighting presets
//!
//! ## Usage
//!
//! ```ignore
//! use studio_core::scene_utils::{SceneBuilder, spawn_chunk_with_lights};
//!
//! fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, ...) {
//!     let chunk = load_creature_script("script.lua")?;
//!     
//!     // One-liner that handles mesh + lights + correct transforms
//!     spawn_chunk_with_lights(&mut commands, &mut meshes, &mut materials, &chunk, Vec3::ZERO);
//! }
//! ```

use bevy::prelude::*;

use crate::deferred::{DeferredPointLight, DeferredRenderable};
use crate::voxel::{extract_emissive_lights, EmissiveLight, VoxelChunk, VoxelWorld, CHUNK_SIZE};
use crate::voxel_mesh::{build_chunk_mesh, build_chunk_mesh_greedy, VoxelMaterial};

/// Configuration for spawning point lights from emissive voxels.
#[derive(Clone, Debug)]
pub struct EmissiveLightConfig {
    /// Minimum emission value (0-255) to create a point light.
    pub min_emission: u8,
    /// Intensity multiplier applied to emission value.
    pub intensity_multiplier: f32,
    /// Base radius for point lights.
    pub base_radius: f32,
    /// Additional Y offset for lights above emissive voxels.
    pub y_offset: f32,
}

impl Default for EmissiveLightConfig {
    fn default() -> Self {
        Self {
            min_emission: 100,
            intensity_multiplier: 15.0,
            base_radius: 12.0,
            y_offset: 1.0,
        }
    }
}

impl EmissiveLightConfig {
    /// Create config for bright, large-radius lights (good for dark scenes).
    pub fn bright() -> Self {
        Self {
            min_emission: 50,
            intensity_multiplier: 25.0,
            base_radius: 20.0,
            y_offset: 2.0,
        }
    }

    /// Create config for subtle, small-radius lights.
    pub fn subtle() -> Self {
        Self {
            min_emission: 150,
            intensity_multiplier: 10.0,
            base_radius: 8.0,
            y_offset: 0.5,
        }
    }
}

/// Result of spawning a chunk with lights.
pub struct SpawnedChunk {
    /// Entity ID of the mesh.
    pub mesh_entity: Entity,
    /// Entity IDs of spawned point lights.
    pub light_entities: Vec<Entity>,
    /// Number of emissive voxels found.
    pub emissive_count: usize,
}

/// Spawn a voxel chunk mesh with automatic point lights from emissive voxels.
///
/// This is the recommended way to add voxel content to a scene. It:
/// 1. Builds the mesh with greedy meshing
/// 2. Extracts emissive voxels
/// 3. Spawns point lights at correct world positions
/// 4. Handles all coordinate transforms consistently
///
/// # Arguments
/// * `commands` - Bevy commands for spawning entities
/// * `meshes` - Mesh asset storage
/// * `materials` - Material asset storage  
/// * `chunk` - The voxel chunk to render
/// * `world_offset` - World-space position for the chunk center
///
/// # Example
/// ```ignore
/// let chunk = load_creature_script("test.lua")?;
/// let result = spawn_chunk_with_lights(
///     &mut commands, &mut meshes, &mut materials,
///     &chunk,
///     Vec3::new(0.0, 16.0, 0.0), // Center chunk at Y=16
/// );
/// println!("Spawned {} lights", result.light_entities.len());
/// ```
pub fn spawn_chunk_with_lights(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<VoxelMaterial>,
    chunk: &VoxelChunk,
    world_offset: Vec3,
) -> SpawnedChunk {
    spawn_chunk_with_lights_config(
        commands,
        meshes,
        materials,
        chunk,
        world_offset,
        &EmissiveLightConfig::default(),
    )
}

/// Spawn a voxel chunk with custom light configuration.
pub fn spawn_chunk_with_lights_config(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<VoxelMaterial>,
    chunk: &VoxelChunk,
    world_offset: Vec3,
    config: &EmissiveLightConfig,
) -> SpawnedChunk {
    // Build mesh with greedy meshing for best performance
    let mesh = build_chunk_mesh_greedy(chunk);
    let mesh_handle = meshes.add(mesh);
    let material = materials.add(VoxelMaterial::default());

    // Spawn mesh entity
    // The mesh is centered at origin (voxels at 0-31 become -16 to +15)
    // world_offset positions the chunk center in world space
    let mesh_entity = commands
        .spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(material),
            Transform::from_translation(world_offset),
            DeferredRenderable,
        ))
        .id();

    // Extract and spawn emissive lights
    let emissive_lights = extract_emissive_lights(chunk, config.min_emission);
    let emissive_count = emissive_lights.len();
    let mut light_entities = Vec::with_capacity(emissive_count);

    for light in &emissive_lights {
        // mesh_position() returns coordinates relative to mesh center
        // Apply world_offset to get final world position
        let mesh_pos = light.mesh_position();
        let world_pos = Vec3::new(mesh_pos[0], mesh_pos[1], mesh_pos[2]) 
            + world_offset 
            + Vec3::new(0.0, config.y_offset, 0.0);

        let intensity = config.intensity_multiplier * light.emission;
        let color = Color::srgb(light.color[0], light.color[1], light.color[2]);

        let entity = commands
            .spawn((
                DeferredPointLight {
                    color,
                    intensity,
                    radius: config.base_radius,
                },
                Transform::from_translation(world_pos),
            ))
            .id();

        light_entities.push(entity);
    }

    SpawnedChunk {
        mesh_entity,
        light_entities,
        emissive_count,
    }
}

/// Spawn a voxel chunk without automatic lights.
///
/// Use this when you want full control over lighting, or for chunks
/// that don't have emissive voxels.
pub fn spawn_chunk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<VoxelMaterial>,
    chunk: &VoxelChunk,
    world_offset: Vec3,
) -> Entity {
    let mesh = build_chunk_mesh_greedy(chunk);
    let mesh_handle = meshes.add(mesh);
    let material = materials.add(VoxelMaterial::default());

    commands
        .spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(material),
            Transform::from_translation(world_offset),
            DeferredRenderable,
        ))
        .id()
}

/// Calculate the standard world offset for a chunk to place it at ground level.
///
/// This places the chunk so that voxel Y=0 is at world Y=0.
/// Useful for terrain-style scenes.
pub fn ground_level_offset() -> Vec3 {
    // Mesh is centered, so Y=-16 is chunk Y=0
    // To put chunk Y=0 at world Y=0: offset by (0, 16, 0)
    Vec3::new(0.0, CHUNK_SIZE as f32 / 2.0, 0.0)
}

/// Calculate the standard world offset for a chunk centered at origin.
///
/// This places the chunk center at world origin (0, 0, 0).
pub fn centered_offset() -> Vec3 {
    Vec3::ZERO
}

/// Extract emissive voxels from a chunk and convert to world-space light positions.
///
/// Lower-level function if you need custom light spawning logic.
pub fn extract_world_lights(
    chunk: &VoxelChunk,
    world_offset: Vec3,
    min_emission: u8,
) -> Vec<(Vec3, EmissiveLight)> {
    extract_emissive_lights(chunk, min_emission)
        .into_iter()
        .map(|light| {
            let mesh_pos = light.mesh_position();
            let world_pos = Vec3::new(mesh_pos[0], mesh_pos[1], mesh_pos[2]) + world_offset;
            (world_pos, light)
        })
        .collect()
}

/// Spawn a manual point light at a world position.
///
/// Convenience wrapper for consistent light setup.
pub fn spawn_point_light(
    commands: &mut Commands,
    position: Vec3,
    color: Color,
    intensity: f32,
    radius: f32,
) -> Entity {
    commands
        .spawn((
            DeferredPointLight {
                color,
                intensity,
                radius,
            },
            Transform::from_translation(position),
        ))
        .id()
}

// ============================================================================
// MULTI-CHUNK WORLD UTILITIES
// ============================================================================

/// Configuration for spawning a multi-chunk world.
#[derive(Clone, Debug)]
pub struct WorldSpawnConfig {
    /// Light configuration for emissive voxels.
    pub light_config: EmissiveLightConfig,
    /// Whether to use greedy meshing.
    pub use_greedy_meshing: bool,
    /// Shared material handle (if None, creates one per chunk).
    pub shared_material: Option<Handle<VoxelMaterial>>,
}

impl Default for WorldSpawnConfig {
    fn default() -> Self {
        Self {
            light_config: EmissiveLightConfig::default(),
            use_greedy_meshing: true,
            shared_material: None,
        }
    }
}

/// Result of spawning a multi-chunk world.
pub struct SpawnedWorld {
    /// Entity IDs of chunk meshes.
    pub chunk_entities: Vec<Entity>,
    /// Entity IDs of all spawned point lights.
    pub light_entities: Vec<Entity>,
    /// Total emissive voxels found.
    pub total_emissive: usize,
}

/// Spawn all chunks in a VoxelWorld with automatic point lights.
///
/// This handles multi-chunk worlds correctly, placing each chunk
/// at its proper world position and extracting lights from each.
pub fn spawn_world_with_lights(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<VoxelMaterial>,
    world: &VoxelWorld,
) -> SpawnedWorld {
    spawn_world_with_lights_config(commands, meshes, materials, world, &WorldSpawnConfig::default())
}

/// Spawn a multi-chunk world with custom configuration.
pub fn spawn_world_with_lights_config(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<VoxelMaterial>,
    world: &VoxelWorld,
    config: &WorldSpawnConfig,
) -> SpawnedWorld {
    let material = config.shared_material.clone()
        .unwrap_or_else(|| materials.add(VoxelMaterial::default()));

    let mut chunk_entities = Vec::new();
    let mut light_entities = Vec::new();
    let mut total_emissive = 0;

    for (chunk_pos, chunk) in world.iter_chunks() {
        if chunk.is_empty() {
            continue;
        }

        // Build mesh
        let mesh = if config.use_greedy_meshing {
            build_chunk_mesh_greedy(chunk)
        } else {
            build_chunk_mesh(chunk)
        };
        let mesh_handle = meshes.add(mesh);

        // Calculate world position for this chunk
        // Chunk at (1, 0, 2) with CHUNK_SIZE=32 should be at world (32, 0, 64) + centering
        let half = CHUNK_SIZE as f32 / 2.0;
        let world_offset = Vec3::new(
            chunk_pos.x as f32 * CHUNK_SIZE as f32 + half,
            chunk_pos.y as f32 * CHUNK_SIZE as f32 + half,
            chunk_pos.z as f32 * CHUNK_SIZE as f32 + half,
        );

        // Spawn chunk mesh
        let entity = commands
            .spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(world_offset),
                DeferredRenderable,
            ))
            .id();
        chunk_entities.push(entity);

        // Extract and spawn lights for this chunk
        let emissive = extract_emissive_lights(chunk, config.light_config.min_emission);
        total_emissive += emissive.len();

        for light in &emissive {
            let mesh_pos = light.mesh_position();
            let world_pos = Vec3::new(mesh_pos[0], mesh_pos[1], mesh_pos[2])
                + world_offset
                + Vec3::new(0.0, config.light_config.y_offset, 0.0);

            let intensity = config.light_config.intensity_multiplier * light.emission;
            let color = Color::srgb(light.color[0], light.color[1], light.color[2]);

            let light_entity = commands
                .spawn((
                    DeferredPointLight {
                        color,
                        intensity,
                        radius: config.light_config.base_radius,
                    },
                    Transform::from_translation(world_pos),
                ))
                .id();
            light_entities.push(light_entity);
        }
    }

    SpawnedWorld {
        chunk_entities,
        light_entities,
        total_emissive,
    }
}

// ============================================================================
// CAMERA PRESETS
// ============================================================================

/// Standard camera configurations for common viewing angles.
pub struct CameraPreset {
    pub position: Vec3,
    pub look_at: Vec3,
}

impl CameraPreset {
    /// Isometric-style view - classic 3/4 angle showing top, front, and side.
    pub fn isometric(target: Vec3, distance: f32) -> Self {
        Self {
            position: target + Vec3::new(distance, distance * 0.75, distance),
            look_at: target,
        }
    }

    /// Overhead view looking down at the scene.
    pub fn overhead(target: Vec3, height: f32) -> Self {
        Self {
            position: target + Vec3::new(0.0, height, height * 0.3),
            look_at: target,
        }
    }

    /// Front-facing view.
    pub fn front(target: Vec3, distance: f32) -> Self {
        Self {
            position: target + Vec3::new(0.0, distance * 0.3, distance),
            look_at: target,
        }
    }

    /// Side view.
    pub fn side(target: Vec3, distance: f32) -> Self {
        Self {
            position: target + Vec3::new(distance, distance * 0.3, 0.0),
            look_at: target,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::Voxel;

    #[test]
    fn test_ground_level_offset() {
        let offset = ground_level_offset();
        assert_eq!(offset.y, 16.0);
    }

    #[test]
    fn test_emissive_light_config_defaults() {
        let config = EmissiveLightConfig::default();
        assert_eq!(config.min_emission, 100);
        assert!(config.intensity_multiplier > 0.0);
    }

    #[test]
    fn test_extract_world_lights() {
        let mut chunk = VoxelChunk::new();
        chunk.set(16, 16, 16, Voxel::new(255, 0, 0, 200)); // Red emissive at center

        let lights = extract_world_lights(&chunk, Vec3::new(100.0, 0.0, 0.0), 100);
        assert_eq!(lights.len(), 1);
        
        let (world_pos, _light) = &lights[0];
        // Mesh position for (16,16,16) is (0.5, 0.5, 0.5) after centering
        // Plus world_offset (100, 0, 0)
        assert!((world_pos.x - 100.5).abs() < 0.01);
    }
}
