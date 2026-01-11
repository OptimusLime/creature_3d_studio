//! Core utilities and shared types for Creature 3D Studio.
//!
//! This crate provides:
//! - Screenshot capture utilities for testing
//! - Orbit camera controller
//! - Voxel data structures for creature modeling
//! - Deferred rendering pipeline (Bonsai-style)
//! - Chunk streaming for infinite worlds
//! - World save/load functionality
//! - Common types used across crates
//! - Configuration management
//! - Shared utilities

use bevy::prelude::*;

pub mod benchmark;
pub mod character_controller;
pub mod chunk_streaming;
pub mod creature_script;
pub mod day_night;
pub mod debug_screenshot;
pub mod deferred;
pub mod markov_junior;
pub mod orbit_camera;
pub mod physics_math;
pub mod scene_utils;
pub mod screenshot;
pub mod screenshot_sequence;
pub mod sky_sphere;
pub mod voxel;
pub mod voxel_collision;
pub mod voxel_collision_gpu;
pub mod voxel_fragment;
pub mod voxel_layer;
pub mod voxel_mesh;
pub mod voxel_physics;
pub mod voxel_world_plugin;
pub mod world_io;

pub use benchmark::{BenchmarkConfig, BenchmarkPlugin, BenchmarkResult, BenchmarkStats};
pub use character_controller::{
    CharacterControllerConfig, CharacterControllerPlugin, PlayerCharacter, ThirdPersonCamera,
};
pub use chunk_streaming::{
    chunk_streaming_system, load_all_chunks_in_radius, world_pos_to_chunk, ChunkEntity,
    ChunkManager, ChunkMaterialHandle, ChunkStreamingConfig, ChunkStreamingPlugin, StreamingStats,
};
pub use creature_script::{execute_creature_script, load_creature_script};
pub use day_night::{
    apply_cycle_to_bloom, apply_cycle_to_moon_config, update_day_night_cycle, ColorKeyframe,
    ColorLutConfig, DayNightCycle, DayNightCyclePlugin, InterpolationMode, MoonCycleConfig,
};
pub use debug_screenshot::{
    DebugCapture, DebugModes, DebugScreenshotConfig, DebugScreenshotPlugin, DebugScreenshotState,
};
pub use deferred::{
    DeferredCamera, DeferredLabel, DeferredLightingConfig, DeferredPointLight, DeferredRenderable,
    DeferredRenderingPlugin, GpuCollisionContacts, MoonConfig, PrimaryShadowCaster,
};
pub use orbit_camera::{OrbitCamera, OrbitCameraBundle, OrbitCameraPlugin};
pub use physics_math::{
    aggregate_particle_forces, apply_gravity, compute_ground_collision_force,
    compute_kinematic_correction, compute_particle_collision_force,
    compute_terrain_collision_force, compute_terrain_collision_force_scaled,
    detect_terrain_collisions, detect_terrain_collisions_scaled, generate_surface_particles,
    has_ceiling_contact, has_floor_contact, has_wall_contact, integrate_angular_velocity,
    integrate_position, integrate_rotation, integrate_velocity, simulate_single_body,
    simulate_single_body_on_terrain, simulate_two_bodies, BodyId, BodyState, FragmentParticleData,
    ParticleConfig, PhysicsConfig, PhysicsEngine, TerrainContact, VoxelFace,
};
pub use scene_utils::{
    centered_offset, chunk_world_bounds, compute_camera_framing, ground_level_offset, spawn_chunk,
    spawn_chunk_scaled, spawn_chunk_with_lights, spawn_chunk_with_lights_config,
    spawn_chunk_with_lights_scaled, spawn_framed_camera, spawn_point_light,
    spawn_world_with_lights, spawn_world_with_lights_config, CameraFraming, CameraPreset,
    EmissiveLightConfig, SpawnedChunk, SpawnedWorld, WorldSpawnConfig,
};
pub use screenshot::{capture_screenshot, ScreenshotPlugin, ScreenshotRequest};
pub use screenshot_sequence::{
    capture_screenshot_sequence, ScreenshotSequence, ScreenshotSequencePlugin, SequenceState,
};
pub use sky_sphere::{SkySphere, SkySphereConfig, SkySphereMaterial, SkySpherePlugin};
pub use voxel::{
    extract_clustered_emissive_lights, extract_emissive_lights, world_to_local, BorderDirection,
    BorderSlice, ChunkBorders, ChunkPos, EmissiveLight, Voxel, VoxelChunk, VoxelScaleConfig,
    VoxelWorld, CHUNK_SIZE, CHUNK_SIZE_I32,
};
pub use voxel_collision::{
    chunk_coord_to_world, world_pos_to_chunk_coord, world_pos_to_local, ChunkOccupancy,
    CollisionPoint, CollisionResult, FragmentCollisionResult, FragmentContact, FragmentOccupancy,
    GpuCollisionAABB, KinematicController, WorldOccupancy, OCCUPANCY_CHUNK_SIZE,
};
pub use voxel_collision_gpu::{
    CollisionUniforms, GpuCollisionPipeline, GpuCollisionResult, GpuContact, GpuFragmentData,
    GpuWorldOccupancy, MAX_GPU_CHUNKS, MAX_GPU_CONTACTS,
};
pub use voxel_fragment::{
    detect_settling_fragments, draw_fragment_debug_gizmos, draw_terrain_debug_gizmos,
    fragment_terrain_collision_system, spawn_fragment, spawn_fragment_with_mesh,
    FragmentCollisionConfig, FragmentConfig, FragmentDebugConfig, FragmentPhysics, FragmentPreview,
    FragmentSurfaceParticles, StaticVoxelWorld, TerrainOccupancy, VoxelFragment,
    VoxelFragmentBundle, VoxelFragmentPlugin,
};
pub use voxel_layer::{
    update_dirty_chunks, ChunkEntityMap, VoxelLayer, VoxelLayers, VoxelLayersPlugin,
};
pub use voxel_mesh::{
    build_chunk_mesh, build_chunk_mesh_greedy, build_chunk_mesh_greedy_with_borders,
    build_chunk_mesh_with_borders, build_merged_chunk, build_merged_chunk_mesh,
    build_single_chunk_mesh, build_world_meshes, build_world_meshes_cross_chunk,
    build_world_meshes_cross_chunk_with_options, build_world_meshes_with_options, ChunkMesh,
    VoxelMaterial, VoxelMaterialPlugin, ATTRIBUTE_VOXEL_AO, ATTRIBUTE_VOXEL_COLOR,
    ATTRIBUTE_VOXEL_EMISSION,
};
pub use voxel_physics::{
    generate_chunk_colliders, generate_cuboid_collider, generate_merged_cuboid_collider,
    generate_trimesh_collider,
};
pub use voxel_world_plugin::{
    BloomConfig, CameraConfig, ScreenshotConfig, VoxelWorldApp, VoxelWorldConfig, WorldSource,
};
pub use world_io::{
    load_world, load_world_binary, load_world_json, save_world, save_world_binary, save_world_json,
    world_file_info, WorldFileInfo, WorldFormat, WorldIoError, WorldIoResult,
};

/// Core plugin for shared functionality.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ScreenshotPlugin)
            .add_plugins(OrbitCameraPlugin);
    }
}
