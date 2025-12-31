//! GTAO (Ground Truth Ambient Occlusion) resources and preparation.
//!
//! This module handles:
//! - GTAO configuration (100% XeGTAO compliant)
//! - GTAO output texture allocation
//! - Config extraction to render world
//!
//! Based on Intel's XeGTAO algorithm.
//! Reference: https://github.com/GameTechDev/XeGTAO

use bevy::prelude::*;
use bevy::render::{
    extract_resource::ExtractResource,
    render_resource::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages},
    renderer::RenderDevice,
    texture::{CachedTexture, TextureCache},
};

/// XeGTAO quality presets.
/// Maps to XeGTAO's QualityLevel setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GtaoQualityLevel {
    /// 1 slice, 2 steps = 4 samples
    Low = 0,
    /// 2 slices, 2 steps = 8 samples
    Medium = 1,
    /// 3 slices, 3 steps = 18 samples (recommended)
    #[default]
    High = 2,
    /// 9 slices, 3 steps = 54 samples
    Ultra = 3,
}

impl GtaoQualityLevel {
    /// Get slice count for this quality level.
    /// From XeGTAO: Low=1, Medium=2, High=3, Ultra=9
    pub fn slice_count(&self) -> u32 {
        match self {
            GtaoQualityLevel::Low => 1,
            GtaoQualityLevel::Medium => 2,
            GtaoQualityLevel::High => 3,
            GtaoQualityLevel::Ultra => 9,
        }
    }

    /// Get steps per slice for this quality level.
    /// From XeGTAO: Low=2, Medium=2, High=3, Ultra=3
    pub fn steps_per_slice(&self) -> u32 {
        match self {
            GtaoQualityLevel::Low => 2,
            GtaoQualityLevel::Medium => 2,
            GtaoQualityLevel::High => 3,
            GtaoQualityLevel::Ultra => 3,
        }
    }
}

/// XeGTAO denoise level.
/// Maps to XeGTAO's DenoisePasses setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GtaoDenoiseLevel {
    /// No denoising (DenoiseBlurBeta = 1e4)
    Disabled = 0,
    /// 1 pass, sharp (DenoiseBlurBeta = 1.2)
    #[default]
    Sharp = 1,
    /// 2 passes, medium
    Medium = 2,
    /// 3 passes, soft
    Soft = 3,
}

impl GtaoDenoiseLevel {
    /// Get number of denoise passes.
    pub fn passes(&self) -> u32 {
        match self {
            GtaoDenoiseLevel::Disabled => 0,
            GtaoDenoiseLevel::Sharp => 1,
            GtaoDenoiseLevel::Medium => 2,
            GtaoDenoiseLevel::Soft => 3,
        }
    }

    /// Get DenoiseBlurBeta value.
    /// From XeGTAO: disabled=1e4, enabled=1.2
    pub fn blur_beta(&self) -> f32 {
        match self {
            GtaoDenoiseLevel::Disabled => 1e4,
            _ => 1.2,
        }
    }
}

/// GTAO configuration resource - 100% XeGTAO compliant.
///
/// All defaults match XeGTAO.h:
/// - XE_GTAO_DEFAULT_RADIUS_MULTIPLIER = 1.457
/// - XE_GTAO_DEFAULT_FALLOFF_RANGE = 0.615
/// - XE_GTAO_DEFAULT_SAMPLE_DISTRIBUTION_POWER = 2.0
/// - XE_GTAO_DEFAULT_THIN_OCCLUDER_COMPENSATION = 0.0
/// - XE_GTAO_DEFAULT_FINAL_VALUE_POWER = 2.2
/// - XE_GTAO_DEFAULT_DEPTH_MIP_SAMPLING_OFFSET = 3.30
#[derive(Resource, Clone, ExtractResource)]
pub struct GtaoConfig {
    /// Quality preset (determines slice count and steps per slice)
    pub quality_level: GtaoQualityLevel,

    /// Denoise level (determines number of blur passes)
    pub denoise_level: GtaoDenoiseLevel,

    /// World-space effect radius.
    /// Scene dependent - larger scenes need larger radius.
    /// XeGTAO default: 0.5
    pub effect_radius: f32,

    /// Falloff range as fraction of effect radius.
    /// Distant samples contribute less.
    /// XeGTAO default: 0.615
    pub effect_falloff_range: f32,

    /// Radius multiplier to counter screen-space biases.
    /// Range: [0.3, 3.0]
    /// XeGTAO default: 1.457
    pub radius_multiplier: f32,

    /// Power function applied to final visibility value.
    /// Range: [0.5, 5.0]
    /// XeGTAO default: 2.2
    pub final_value_power: f32,

    /// Sample distribution power - higher values focus samples on small crevices.
    /// Range: [1.0, 3.0]
    /// XeGTAO default: 2.0
    pub sample_distribution_power: f32,

    /// Thin occluder compensation heuristic.
    /// Reduces occlusion from thin foreground objects.
    /// Range: [0.0, 0.7]
    /// XeGTAO default: 0.0
    pub thin_occluder_compensation: f32,

    /// Depth MIP sampling offset.
    /// Trade-off between performance and quality.
    /// Range: [2.0, 6.0]
    /// XeGTAO default: 3.30
    pub depth_mip_sampling_offset: f32,

    /// Enable/disable GTAO entirely
    pub enabled: bool,

    /// Debug mode for denoiser: 0=normal, 1=sum_weight, 2=edges_c, 3=blur_amount, 4=diff
    pub denoise_debug_mode: u32,
}

impl Default for GtaoConfig {
    fn default() -> Self {
        Self {
            // HIGH quality preset - 3 slices, 3 steps = 18 samples
            quality_level: GtaoQualityLevel::High,
            // Medium denoise - 2 passes for good balance of quality and performance
            denoise_level: GtaoDenoiseLevel::Medium,
            // XeGTAO default
            effect_radius: 0.5,
            // XeGTAO defaults below
            effect_falloff_range: 0.615,
            radius_multiplier: 1.457,
            final_value_power: 2.2,
            sample_distribution_power: 2.0,
            thin_occluder_compensation: 0.0,
            depth_mip_sampling_offset: 3.30,
            enabled: true,
            denoise_debug_mode: 0,
        }
    }
}

impl GtaoConfig {
    /// Create config for Low quality (4 samples)
    pub fn low() -> Self {
        Self {
            quality_level: GtaoQualityLevel::Low,
            ..Default::default()
        }
    }

    /// Create config for Medium quality (8 samples)
    pub fn medium() -> Self {
        Self {
            quality_level: GtaoQualityLevel::Medium,
            ..Default::default()
        }
    }

    /// Create config for High quality (18 samples) - DEFAULT
    pub fn high() -> Self {
        Self::default()
    }

    /// Create config for Ultra quality (54 samples)
    pub fn ultra() -> Self {
        Self {
            quality_level: GtaoQualityLevel::Ultra,
            ..Default::default()
        }
    }

    /// Get slice count from quality level
    pub fn slice_count(&self) -> u32 {
        self.quality_level.slice_count()
    }

    /// Get steps per slice from quality level
    pub fn steps_per_slice(&self) -> u32 {
        self.quality_level.steps_per_slice()
    }

    /// Get total sample count
    pub fn total_samples(&self) -> u32 {
        self.slice_count() * self.steps_per_slice() * 2 // *2 for both directions
    }

    /// Get denoise passes
    pub fn denoise_passes(&self) -> u32 {
        self.denoise_level.passes()
    }

    /// Get denoise blur beta
    pub fn denoise_blur_beta(&self) -> f32 {
        self.denoise_level.blur_beta()
    }
}

/// Per-view GTAO texture that stores the computed ambient occlusion.
#[derive(Component)]
pub struct ViewGtaoTexture {
    pub texture: CachedTexture,
}

/// Per-view GTAO edges texture for edge-aware denoising.
#[derive(Component)]
pub struct ViewGtaoEdgesTexture {
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

        // Create edges texture for denoiser (R8 packed edges)
        let edges_texture = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("gtao_edges_texture"),
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

        commands.entity(entity).insert((
            ViewGtaoTexture { texture: gtao_texture },
            ViewGtaoEdgesTexture { texture: edges_texture },
        ));
    }
}


