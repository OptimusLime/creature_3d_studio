//! Deferred rendering pipeline for voxel rendering.
//!
//! This module implements a full custom render graph for deferred rendering:
//!
//! ## Pipeline Overview
//!
//! ```text
//! ┌─────────────────┐
//! │   G-Buffer Pass │  ← Renders geometry to MRT (color, normal, position)
//! └────────┬────────┘
//!          │
//! ┌────────▼────────┐
//! │  Lighting Pass  │  ← Fullscreen quad, reads G-buffer, computes lighting
//! └────────┬────────┘
//!          │
//! ┌────────▼────────┐
//! │   Final Output  │  ← Written to ViewTarget
//! └─────────────────┘
//! ```
//!
//! ## G-Buffer Layout (Bonsai-compatible)
//!
//! - **gColor** (RGBA16F): RGB = albedo, A = emission intensity
//! - **gNormal** (RGBA16F): RGB = world-space normal (normalized)
//! - **gPosition** (RGBA32F): XYZ = world position, W = linear depth
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Add the plugin
//! app.add_plugins(DeferredRenderingPlugin);
//!
//! // Mark cameras for deferred rendering
//! commands.spawn((
//!     Camera3d::default(),
//!     DeferredCamera,
//! ));
//! ```
//!
//! Reference: bonsai/shaders/gBuffer.fragmentshader, Lighting.fragmentshader

mod bloom;
mod bloom_node;
mod extract;
mod gbuffer;
mod gbuffer_geometry;
mod gbuffer_material;
mod gbuffer_node;
mod labels;
mod lighting;
mod lighting_node;
mod plugin;
mod point_light;
mod point_light_shadow;
mod point_light_shadow_node;
mod prepare;
mod shadow;
mod shadow_node;
mod gtao;
mod gtao_node;

pub use bloom::*;
pub use bloom_node::*;
pub use extract::*;
pub use gbuffer::*;
pub use gbuffer_geometry::*;
pub use gbuffer_material::*;
pub use labels::*;
pub use lighting::*;
pub use plugin::*;
pub use point_light::*;
pub use point_light_shadow::*;
pub use point_light_shadow_node::*;
pub use shadow::*;
pub use shadow_node::*;
pub use gtao::*;
pub use gtao_node::*;
