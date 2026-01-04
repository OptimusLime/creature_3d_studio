//! G-Buffer textures and resources for deferred rendering.
//!
//! The G-Buffer stores geometry information in multiple render targets:
//! - gColor (RGBA16F): RGB = albedo, A = emission intensity
//! - gNormal (RGBA16F): RGB = world-space normal (normalized)
//! - gPosition (RGBA32F): XYZ = world position, W = linear depth
//!
//! These textures are created in the render world and managed via TextureCache.

use bevy::prelude::*;
use bevy::render::{
    extract_component::ExtractComponent,
    render_resource::{
        Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    },
    renderer::RenderDevice,
    texture::{CachedTexture, TextureCache},
};

/// Marker component for cameras that should use deferred rendering.
///
/// Add this to a camera to enable the deferred rendering pipeline.
#[derive(Component, Default, Clone, ExtractComponent)]
pub struct DeferredCamera;

/// G-Buffer textures for a camera in the render world.
///
/// This component is attached to camera entities during the Prepare phase
/// and contains cached GPU textures for the G-buffer.
#[derive(Component)]
pub struct ViewGBufferTextures {
    /// RGB = albedo color, A = emission intensity
    pub color: CachedTexture,
    /// RGB = world-space normal (normalized)
    pub normal: CachedTexture,
    /// XYZ = world position, W = linear depth
    pub position: CachedTexture,
    /// Depth buffer for G-buffer pass
    pub depth: CachedTexture,
    /// Size of the G-buffer textures
    pub size: Extent3d,
}

/// Depth texture format for G-buffer pass
pub const GBUFFER_DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// G-Buffer texture formats (matching Bonsai)
pub const GBUFFER_COLOR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
pub const GBUFFER_NORMAL_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
pub const GBUFFER_POSITION_FORMAT: TextureFormat = TextureFormat::Rgba32Float;

impl ViewGBufferTextures {
    /// Create G-buffer textures for a given size.
    pub fn new(
        render_device: &RenderDevice,
        texture_cache: &mut TextureCache,
        size: Extent3d,
    ) -> Self {
        let color = texture_cache.get(
            render_device,
            TextureDescriptor {
                label: Some("gbuffer_color"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: GBUFFER_COLOR_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        let normal = texture_cache.get(
            render_device,
            TextureDescriptor {
                label: Some("gbuffer_normal"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: GBUFFER_NORMAL_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        let position = texture_cache.get(
            render_device,
            TextureDescriptor {
                label: Some("gbuffer_position"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: GBUFFER_POSITION_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        let depth = texture_cache.get(
            render_device,
            TextureDescriptor {
                label: Some("gbuffer_depth"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: GBUFFER_DEPTH_FORMAT,
                // TEXTURE_BINDING needed for GTAO to sample the hardware depth buffer
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        Self {
            color,
            normal,
            position,
            depth,
            size,
        }
    }
}
