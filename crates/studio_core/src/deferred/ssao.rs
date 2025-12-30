//! SSAO (Screen-Space Ambient Occlusion) resources and preparation.
//!
//! This module handles:
//! - SSAO kernel generation (hemisphere sampling points)
//! - Noise texture generation (random rotation vectors)
//! - SSAO texture allocation

use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    },
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
            kernel_size: 32,
            radius: 0.5,
            intensity: 1.0,
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
        Self::new(32)
    }
}

impl SsaoKernel {
    /// Generate a new SSAO kernel with the specified number of samples.
    /// 
    /// The samples are distributed in a hemisphere with:
    /// - Random directions in tangent space
    /// - Scaled to cluster samples closer to the origin (more local occlusion)
    pub fn new(size: u32) -> Self {
        let mut rng = rand::thread_rng();
        let mut samples = Vec::with_capacity(size as usize);
        
        for i in 0..size {
            // Generate random point in hemisphere
            // x, y: random in [-1, 1]
            // z: random in [0, 1] (hemisphere facing +Z)
            let x: f32 = rng.gen::<f32>() * 2.0 - 1.0;
            let y: f32 = rng.gen::<f32>() * 2.0 - 1.0;
            let z: f32 = rng.gen::<f32>(); // Only positive Z (hemisphere)
            
            // Normalize to unit sphere
            let mut sample = Vec3::new(x, y, z).normalize();
            
            // Scale to be within hemisphere
            sample *= rng.gen::<f32>();
            
            // Accelerate interpolation - samples closer to origin are more important
            // This creates a distribution where most samples are near the surface
            let scale = i as f32 / size as f32;
            let scale = lerp(0.1, 1.0, scale * scale);
            sample *= scale;
            
            samples.push([sample.x, sample.y, sample.z, 0.0]);
        }
        
        Self { samples }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// SSAO noise texture data.
/// A small texture of random rotation vectors used to reduce banding artifacts.
#[derive(Resource)]
pub struct SsaoNoiseTexture {
    /// The cached texture on GPU
    pub texture: Option<CachedTexture>,
}

impl Default for SsaoNoiseTexture {
    fn default() -> Self {
        Self { texture: None }
    }
}

/// Generate noise texture data (4x4 random rotation vectors).
/// Each pixel contains a random vector in the XY plane (Z=0).
pub fn generate_noise_data() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let size = 4; // 4x4 noise texture
    let mut data = Vec::with_capacity(size * size * 4);
    
    for _ in 0..(size * size) {
        // Random vector in XY plane, normalized
        let x: f32 = rng.gen::<f32>() * 2.0 - 1.0;
        let y: f32 = rng.gen::<f32>() * 2.0 - 1.0;
        let len = (x * x + y * y).sqrt();
        let nx = if len > 0.0 { x / len } else { 1.0 };
        let ny = if len > 0.0 { y / len } else { 0.0 };
        
        // Store as RGBA8 (normalize to 0-255 range)
        data.push(((nx * 0.5 + 0.5) * 255.0) as u8);
        data.push(((ny * 0.5 + 0.5) * 255.0) as u8);
        data.push(128); // Z = 0 (stored as 0.5)
        data.push(255); // Alpha = 1
    }
    
    data
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
        
        commands.entity(entity).insert(ViewSsaoTexture {
            texture: ssao_texture,
        });
    }
}

/// System to prepare the SSAO noise texture (runs once).
pub fn prepare_ssao_noise_texture(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    existing: Option<Res<SsaoNoiseTexture>>,
) {
    // Only create once
    if existing.is_some() && existing.as_ref().unwrap().texture.is_some() {
        return;
    }
    
    let noise_texture = texture_cache.get(
        &render_device,
        TextureDescriptor {
            label: Some("ssao_noise_texture"),
            size: Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        },
    );
    
    // Write noise data to texture
    let noise_data = generate_noise_data();
    render_device.create_buffer_with_data(&bevy::render::render_resource::BufferInitDescriptor {
        label: Some("ssao_noise_staging"),
        contents: &noise_data,
        usage: bevy::render::render_resource::BufferUsages::COPY_SRC,
    });
    
    // Note: We'd need to use a command encoder to copy buffer to texture
    // For now, the texture exists but may not have data until we set up proper upload
    
    commands.insert_resource(SsaoNoiseTexture {
        texture: Some(noise_texture),
    });
}

/// System to initialize the SSAO kernel (runs once).
pub fn init_ssao_kernel(
    mut commands: Commands,
    existing: Option<Res<SsaoKernel>>,
) {
    if existing.is_some() {
        return;
    }
    
    commands.insert_resource(SsaoKernel::new(32));
}
