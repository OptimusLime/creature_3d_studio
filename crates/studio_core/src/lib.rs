//! Core utilities and shared types for Creature 3D Studio.
//!
//! This crate provides:
//! - Screenshot capture utilities for testing
//! - Orbit camera controller
//! - Voxel data structures for creature modeling
//! - Deferred rendering pipeline (Bonsai-style)
//! - Chunk streaming for infinite worlds
//! - Common types used across crates
//! - Configuration management
//! - Shared utilities

use bevy::prelude::*;

pub mod chunk_streaming;
pub mod creature_script;
pub mod deferred;
pub mod orbit_camera;
pub mod scene_utils;
pub mod screenshot;
pub mod voxel;
pub mod voxel_mesh;

pub use creature_script::{execute_creature_script, load_creature_script};
pub use deferred::{
    DeferredCamera, DeferredLabel, DeferredLightingConfig, DeferredPointLight, DeferredRenderable,
    DeferredRenderingPlugin,
};
pub use orbit_camera::{OrbitCamera, OrbitCameraBundle, OrbitCameraPlugin};
pub use screenshot::{capture_screenshot, ScreenshotPlugin, ScreenshotRequest};
pub use voxel::{
    extract_clustered_emissive_lights, extract_emissive_lights, world_to_local, BorderDirection,
    BorderSlice, ChunkBorders, ChunkPos, EmissiveLight, Voxel, VoxelChunk, VoxelWorld, CHUNK_SIZE,
    CHUNK_SIZE_I32,
};
pub use voxel_mesh::{
    build_chunk_mesh, build_chunk_mesh_greedy, build_chunk_mesh_greedy_with_borders,
    build_chunk_mesh_with_borders, build_single_chunk_mesh, build_world_meshes,
    build_world_meshes_cross_chunk, build_world_meshes_cross_chunk_with_options,
    build_world_meshes_with_options, ChunkMesh, VoxelMaterial, VoxelMaterialPlugin,
    ATTRIBUTE_VOXEL_AO, ATTRIBUTE_VOXEL_COLOR, ATTRIBUTE_VOXEL_EMISSION,
};
pub use chunk_streaming::{
    chunk_streaming_system, load_all_chunks_in_radius, world_pos_to_chunk, ChunkEntity,
    ChunkManager, ChunkMaterialHandle, ChunkStreamingConfig, ChunkStreamingPlugin, StreamingStats,
};
pub use scene_utils::{
    centered_offset, chunk_world_bounds, compute_camera_framing, ground_level_offset,
    spawn_chunk, spawn_chunk_with_lights, spawn_chunk_with_lights_config, spawn_framed_camera,
    spawn_point_light, spawn_world_with_lights, spawn_world_with_lights_config, CameraFraming,
    CameraPreset, EmissiveLightConfig, SpawnedChunk, SpawnedWorld, WorldSpawnConfig,
};

/// Core plugin for shared functionality.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ScreenshotPlugin)
            .add_plugins(OrbitCameraPlugin);
    }
}
