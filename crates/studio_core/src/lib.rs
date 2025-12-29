//! Core utilities and shared types for Creature 3D Studio.
//!
//! This crate provides:
//! - Screenshot capture utilities for testing
//! - Orbit camera controller
//! - Voxel data structures for creature modeling
//! - Deferred rendering pipeline (Bonsai-style)
//! - Common types used across crates
//! - Configuration management
//! - Shared utilities

use bevy::prelude::*;

pub mod creature_script;
pub mod deferred;
pub mod orbit_camera;
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
pub use voxel::{Voxel, VoxelChunk, CHUNK_SIZE};
pub use voxel_mesh::{
    build_chunk_mesh, VoxelMaterial, VoxelMaterialPlugin, ATTRIBUTE_VOXEL_COLOR,
    ATTRIBUTE_VOXEL_EMISSION,
};

/// Core plugin for shared functionality.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ScreenshotPlugin)
            .add_plugins(OrbitCameraPlugin);
    }
}
