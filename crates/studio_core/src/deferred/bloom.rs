//! Bloom post-processing for deferred rendering.
//!
//! Implements a multi-pass bloom effect:
//! 1. Extract bright pixels from the HDR scene
//! 2. Downsample through a mip chain (blur)
//! 3. Upsample back up, blending mip levels
//! 4. Composite bloom onto original image
//!
//! Based on Bonsai's bloom implementation.

use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        BindGroupLayout, BindGroupLayoutEntry,
        BindingType, CachedRenderPipelineId, ColorTargetState, ColorWrites, Extent3d,
        FragmentState, MultisampleState, PipelineCache, PrimitiveState, RenderPipelineDescriptor,
        Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, TextureDescriptor,
        TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
        TextureViewDimension, VertexState,
    },
    renderer::RenderDevice,
    texture::{CachedTexture, TextureCache},
};

/// Number of bloom mip levels (downsampling passes).
/// 6 levels: full -> 1/2 -> 1/4 -> 1/8 -> 1/16 -> 1/32 -> 1/64
pub const BLOOM_MIP_LEVELS: usize = 6;

/// Bloom configuration.
#[derive(Resource, Clone)]
pub struct BloomConfig {
    /// Minimum brightness for bloom (0.0-1.0)
    pub threshold: f32,
    /// Bloom intensity multiplier
    pub intensity: f32,
    /// Blend factor for upsample passes
    pub blend_factor: f32,
    /// Exposure for tone mapping
    pub exposure: f32,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            threshold: 0.6,      // Lower threshold to catch colored emissive surfaces
            intensity: 1.5,      // Moderate bloom - preserves color saturation
            blend_factor: 0.6,   // Moderate blend for visible glow without washout
            exposure: 1.2,       // Slightly higher exposure for darker scenes
        }
    }
}

/// Bloom textures for a camera view.
/// Uses ping-pong textures to avoid read-write hazards during upsampling.
#[derive(Component)]
pub struct ViewBloomTextures {
    /// Primary mip chain - used for downsample output and upsample input
    pub mips_a: Vec<CachedTexture>,
    /// Secondary mip chain - used for upsample output to avoid read-write hazard
    pub mips_b: Vec<CachedTexture>,
    /// Size of the full-resolution bloom texture
    pub size: Extent3d,
}

impl ViewBloomTextures {
    /// Create bloom textures for a given viewport size.
    /// Creates two sets of mip textures (A and B) for ping-pong rendering.
    pub fn new(
        render_device: &RenderDevice,
        texture_cache: &mut TextureCache,
        size: Extent3d,
    ) -> Self {
        let mut mips_a = Vec::with_capacity(BLOOM_MIP_LEVELS);
        let mut mips_b = Vec::with_capacity(BLOOM_MIP_LEVELS);
        let mut width = size.width;
        let mut height = size.height;

        // Static labels for mip textures
        const MIP_LABELS_A: [&str; BLOOM_MIP_LEVELS] = [
            "bloom_mip_a_0",
            "bloom_mip_a_1",
            "bloom_mip_a_2",
            "bloom_mip_a_3",
            "bloom_mip_a_4",
            "bloom_mip_a_5",
        ];
        const MIP_LABELS_B: [&str; BLOOM_MIP_LEVELS] = [
            "bloom_mip_b_0",
            "bloom_mip_b_1",
            "bloom_mip_b_2",
            "bloom_mip_b_3",
            "bloom_mip_b_4",
            "bloom_mip_b_5",
        ];

        for i in 0..BLOOM_MIP_LEVELS {
            // Each mip is half the size
            width = (width / 2).max(1);
            height = (height / 2).max(1);

            let mip_size = Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };

            // Create texture A (primary - for downsample and upsample input)
            let texture_a = texture_cache.get(
                render_device,
                TextureDescriptor {
                    label: Some(MIP_LABELS_A[i]),
                    size: mip_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba16Float,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            );

            // Create texture B (secondary - for upsample output)
            let texture_b = texture_cache.get(
                render_device,
                TextureDescriptor {
                    label: Some(MIP_LABELS_B[i]),
                    size: mip_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba16Float,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            );

            mips_a.push(texture_a);
            mips_b.push(texture_b);
        }

        Self { mips_a, mips_b, size }
    }
}

/// Bloom pipeline resources.
#[derive(Resource)]
pub struct BloomPipeline {
    /// Downsample pipeline
    pub downsample_pipeline_id: CachedRenderPipelineId,
    /// Upsample pipeline  
    pub upsample_pipeline_id: CachedRenderPipelineId,
    /// Composite pipeline
    pub composite_pipeline_id: CachedRenderPipelineId,
    /// Bind group layout for single texture + sampler
    pub texture_layout: BindGroupLayout,
    /// Bind group layout for two textures + sampler (upsample)
    pub dual_texture_layout: BindGroupLayout,
    /// Linear sampler for bloom
    pub sampler: Sampler,
}

/// Initialize the bloom pipeline.
pub fn init_bloom_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<BloomPipeline>>,
) {
    if existing.is_some() {
        return;
    }

    // Create sampler
    let sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("bloom_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..default()
    });

    // Single texture layout (downsample, composite scene input)
    let texture_layout = render_device.create_bind_group_layout(
        "bloom_texture_layout",
        &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    );

    // Dual texture layout (upsample, composite)
    let dual_texture_layout = render_device.create_bind_group_layout(
        "bloom_dual_texture_layout",
        &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    );

    // Load shaders
    let downsample_shader = asset_server.load("shaders/bloom_downsample.wgsl");
    let upsample_shader = asset_server.load("shaders/bloom_upsample.wgsl");
    let composite_shader = asset_server.load("shaders/bloom_composite.wgsl");

    let target_format = TextureFormat::Rgba16Float;

    // Downsample pipeline
    let downsample_pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("bloom_downsample_pipeline".into()),
        layout: vec![texture_layout.clone()],
        push_constant_ranges: vec![wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::FRAGMENT,
            range: 0..16, // texel_size (8) + threshold (4) + is_first_pass (4)
        }],
        vertex: VertexState {
            shader: downsample_shader.clone(),
            shader_defs: vec![],
            entry_point: Some("vs_main".into()),
            buffers: vec![],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            shader: downsample_shader,
            shader_defs: vec![],
            entry_point: Some("fs_main".into()),
            targets: vec![Some(ColorTargetState {
                format: target_format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        zero_initialize_workgroup_memory: false,
    });

    // Upsample pipeline
    let upsample_pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("bloom_upsample_pipeline".into()),
        layout: vec![dual_texture_layout.clone()],
        push_constant_ranges: vec![wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::FRAGMENT,
            range: 0..16, // texel_size (8) + blend_factor (4) + padding (4)
        }],
        vertex: VertexState {
            shader: upsample_shader.clone(),
            shader_defs: vec![],
            entry_point: Some("vs_main".into()),
            buffers: vec![],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            shader: upsample_shader,
            shader_defs: vec![],
            entry_point: Some("fs_main".into()),
            targets: vec![Some(ColorTargetState {
                format: target_format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        zero_initialize_workgroup_memory: false,
    });

    // Composite pipeline (outputs to view target, which may be different format)
    let composite_pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("bloom_composite_pipeline".into()),
        layout: vec![dual_texture_layout.clone()],
        push_constant_ranges: vec![wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::FRAGMENT,
            range: 0..16, // bloom_intensity (4) + threshold (4) + exposure (4) + padding (4)
        }],
        vertex: VertexState {
            shader: composite_shader.clone(),
            shader_defs: vec![],
            entry_point: Some("vs_main".into()),
            buffers: vec![],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            shader: composite_shader,
            shader_defs: vec![],
            entry_point: Some("fs_main".into()),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba8UnormSrgb, // View target format
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        zero_initialize_workgroup_memory: false,
    });

    commands.insert_resource(BloomPipeline {
        downsample_pipeline_id,
        upsample_pipeline_id,
        composite_pipeline_id,
        texture_layout,
        dual_texture_layout,
        sampler,
    });

}

/// Prepare bloom textures for cameras.
pub fn prepare_bloom_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    cameras: Query<(Entity, &bevy::render::camera::ExtractedCamera), With<super::DeferredCamera>>,
    existing_bloom: Query<&ViewBloomTextures>,
) {
    for (entity, camera) in cameras.iter() {
        // Skip if already has bloom textures
        if existing_bloom.get(entity).is_ok() {
            continue;
        }

        let Some(size) = camera.physical_viewport_size else {
            continue;
        };

        let extent = Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        };

        let bloom_textures = ViewBloomTextures::new(&render_device, &mut texture_cache, extent);

        commands.entity(entity).insert(bloom_textures);
    }
}
