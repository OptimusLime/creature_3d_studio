//! SSAO (Screen-Space Ambient Occlusion) resources and preparation.
//!
//! This module handles:
//! - SSAO kernel generation (hemisphere sampling points)
//! - SSAO texture allocation

use bevy::prelude::*;
use bevy::render::{
    render_resource::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages},
    renderer::RenderDevice,
    texture::{CachedTexture, TextureCache},
};
use rand::prelude::*;

/// SSAO configuration resource.
#[derive(Resource, Clone)]
pub struct SsaoConfig {
    /// Number of samples in the hemisphere kernel
    pub kernel_size: u32,
    /// World-space radius for occlusion sampling
    pub radius: f32,
    /// Intensity multiplier for the final AO
    pub intensity: f32,
    /// Bias to prevent self-occlusion artifacts
    pub bias: f32,
    /// Enable/disable SSAO
    pub enabled: bool,
}

impl Default for SsaoConfig {
    fn default() -> Self {
        Self {
            kernel_size: 64,
            radius: 0.5,
            intensity: 1.5,
            bias: 0.025,
            enabled: true,
        }
    }
}

/// SSAO kernel containing hemisphere sample directions.
/// These points are distributed in a hemisphere and used to sample
/// the depth buffer around each fragment.
#[derive(Resource)]
pub struct SsaoKernel {
    /// Sample directions in tangent space, packed as vec4 for GPU alignment
    /// xyz = direction, w = unused (padding)
    pub samples: Vec<[f32; 4]>,
}

impl Default for SsaoKernel {
    fn default() -> Self {
        Self::new(64)
    }
}

impl SsaoKernel {
    /// Generate a new SSAO kernel with the specified number of samples.
    ///
    /// The samples are distributed in a hemisphere with:
    /// - Random directions in tangent space (Z+ is the normal direction)
    /// - Scaled to cluster samples closer to the origin (more local occlusion)
    pub fn new(size: u32) -> Self {
        let mut rng = rand::thread_rng();
        let mut samples = Vec::with_capacity(size as usize);

        for i in 0..size {
            // Generate random point in hemisphere using spherical coordinates
            // This gives better distribution than rejection sampling
            let xi1: f32 = rng.gen();
            let xi2: f32 = rng.gen();

            // Cosine-weighted hemisphere sampling for better quality
            let phi = 2.0 * std::f32::consts::PI * xi1;
            let cos_theta = (1.0 - xi2).sqrt(); // cosine-weighted
            let sin_theta = xi2.sqrt();

            let x = sin_theta * phi.cos();
            let y = sin_theta * phi.sin();
            let z = cos_theta; // Always positive (hemisphere)

            let mut sample = Vec3::new(x, y, z);

            // Scale sample to be within radius
            // Accelerate falloff - more samples near the surface
            let scale = (i as f32 + 1.0) / size as f32;
            // lerp(0.1, 1.0, scale^2) for quadratic falloff
            let scale = 0.1 + 0.9 * scale * scale;
            sample *= scale;

            samples.push([sample.x, sample.y, sample.z, 0.0]);
        }

        Self { samples }
    }
}

/// Per-view SSAO texture that stores the computed ambient occlusion.
#[derive(Component)]
pub struct ViewSsaoTexture {
    pub texture: CachedTexture,
}

/// System to prepare SSAO textures for each view.
pub fn prepare_ssao_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    views: Query<(Entity, &bevy::render::camera::ExtractedCamera), Without<ViewSsaoTexture>>,
) {
    for (entity, camera) in views.iter() {
        let Some(size) = camera.physical_viewport_size else {
            continue;
        };

        // Create SSAO output texture (single channel, R8)
        let ssao_texture = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("ssao_texture"),
                size: Extent3d {
                    width: size.x,
                    height: size.y,
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
            .insert(ViewSsaoTexture { texture: ssao_texture });
    }
}

/// System to initialize the SSAO kernel (runs once).
pub fn init_ssao_kernel(mut commands: Commands, existing: Option<Res<SsaoKernel>>) {
    if existing.is_some() {
        return;
    }

    commands.insert_resource(SsaoKernel::new(64));
}
