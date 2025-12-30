//! GTAO (Ground Truth Ambient Occlusion) resources and preparation.
//!
//! This module handles:
//! - GTAO configuration
//! - GTAO output texture allocation
//!
//! Based on Intel's XeGTAO algorithm.
//! Reference: https://github.com/GameTechDev/XeGTAO

use bevy::prelude::*;
use bevy::render::{
    render_resource::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages},
    renderer::RenderDevice,
    texture::{CachedTexture, TextureCache},
};

/// GTAO configuration resource.
#[derive(Resource, Clone)]
pub struct GtaoConfig {
    /// World-space effect radius
    pub effect_radius: f32,
    /// Falloff range (0.0 to 1.0)
    pub effect_falloff_range: f32,
    /// Radius multiplier (0.3 to 3.0, default 1.457)
    pub radius_multiplier: f32,
    /// Final value power (0.5 to 5.0, default 2.2)
    pub final_value_power: f32,
    /// Number of direction slices (default 3)
    pub slice_count: u32,
    /// Steps per slice (default 3)
    pub steps_per_slice: u32,
    /// Enable/disable GTAO
    pub enabled: bool,
}

impl Default for GtaoConfig {
    fn default() -> Self {
        Self {
            effect_radius: 0.5,
            effect_falloff_range: 0.615,
            radius_multiplier: 1.457,
            final_value_power: 2.2,
            slice_count: 3,
            steps_per_slice: 3,
            enabled: true,
        }
    }
}

/// Per-view GTAO texture that stores the computed ambient occlusion.
#[derive(Component)]
pub struct ViewGtaoTexture {
    pub texture: CachedTexture,
}

/// System to prepare GTAO textures for each view.
/// Renders at HALF resolution - upsampling naturally smooths the output.
pub fn prepare_gtao_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    views: Query<(Entity, &bevy::render::camera::ExtractedCamera), Without<ViewGtaoTexture>>,
) {
    for (entity, camera) in views.iter() {
        let Some(size) = camera.physical_viewport_size else {
            continue;
        };

        // HALF RESOLUTION - upsampling acts as a natural blur
        let half_width = (size.x / 2).max(1);
        let half_height = (size.y / 2).max(1);

        // Create GTAO output texture (single channel, R8) at half resolution
        let gtao_texture = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("gtao_texture"),
                size: Extent3d {
                    width: half_width,
                    height: half_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R8Unorm,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        commands
            .entity(entity)
            .insert(ViewGtaoTexture { texture: gtao_texture });
    }
}
