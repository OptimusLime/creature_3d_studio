//! SSAO render graph node.
//!
//! This node performs a fullscreen pass that computes screen-space ambient occlusion
//! using hemisphere sampling in view-space, following the standard SSAO algorithm.

use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
        BufferBindingType, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId,
        ColorTargetState, ColorWrites, Extent3d, FilterMode, FragmentState, LoadOp,
        MultisampleState, Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment,
        RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
        SamplerDescriptor, ShaderStages, StoreOp, TextureDescriptor, TextureDimension,
        TextureFormat, TextureSampleType, TextureUsages, TextureViewDimension, VertexState,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue},
    view::ExtractedView,
};

use super::gbuffer::ViewGBufferTextures;
use super::ssao::{SsaoKernel, ViewSsaoTexture};

/// GPU uniform for SSAO kernel samples (64 samples for quality).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SsaoKernelUniform {
    /// 64 hemisphere sample directions (vec4 for alignment, only xyz used)
    pub samples: [[f32; 4]; 64],
}

/// GPU uniform for camera matrices needed for proper view-space SSAO.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SsaoCameraUniform {
    /// View matrix (world to view space)
    pub view: [[f32; 4]; 4],
    /// Projection matrix (view to clip space)
    pub projection: [[f32; 4]; 4],
    /// Inverse projection matrix (clip to view space) - for depth reconstruction
    pub inv_projection: [[f32; 4]; 4],
    /// Screen dimensions (width, height, 1/width, 1/height)
    pub screen_size: [f32; 4],
    /// XeGTAO parameters 1:
    /// x: EffectRadius (World space radius)
    /// y: EffectFalloffRange (0.0 to 1.0)
    /// z: RadiusMultiplier (0.3 to 3.0, default 1.457)
    /// w: SampleDistributionPower (1.0 to 3.0, default 2.0)
    pub params1: [f32; 4],
    /// XeGTAO parameters 2:
    /// x: ThinOccluderCompensation (0.0 to 0.7, default 0.0)
    /// y: FinalValuePower (0.5 to 5.0, default 2.2)
    /// z: DepthMIPSamplingOffset (default 3.3)
    /// w: Unused
    pub params2: [f32; 4],
    /// Camera TanHalfFOV (x, y) - for View Z reconstruction
    pub tan_half_fov: [f32; 2],
    pub padding: [f32; 2],
}

/// Render graph node that computes SSAO.
#[derive(Default)]
pub struct SsaoPassNode;

impl ViewNode for SsaoPassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ExtractedView,
        &'static ViewGBufferTextures,
        &'static ViewSsaoTexture,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, view, gbuffer, ssao_texture): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let ssao_pipeline = world.get_resource::<SsaoPipeline>();
        let ssao_kernel = world.get_resource::<SsaoKernel>();
        let noise_texture = world.get_resource::<SsaoNoiseTexture>();

        let Some(ssao_pipeline) = ssao_pipeline else {
            return Ok(());
        };

        let Some(pipeline) = pipeline_cache.get_render_pipeline(ssao_pipeline.pipeline_id) else {
            return Ok(());
        };

        let Some(ssao_kernel) = ssao_kernel else {
            return Ok(());
        };

        let Some(noise_texture) = noise_texture else {
            return Ok(());
        };

        // Create bind group for G-buffer textures (group 0)
        let gbuffer_bind_group = render_context.render_device().create_bind_group(
            "ssao_gbuffer_bind_group",
            &ssao_pipeline.gbuffer_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&gbuffer.normal.default_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&gbuffer.position.default_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&ssao_pipeline.gbuffer_sampler),
                },
            ],
        );

        // Create kernel uniform buffer with 64 samples
        let mut kernel_data = SsaoKernelUniform {
            samples: [[0.0; 4]; 64],
        };
        for (i, sample) in ssao_kernel.samples.iter().enumerate().take(64) {
            kernel_data.samples[i] = *sample;
        }

        let kernel_buffer =
            render_context
                .render_device()
                .create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("ssao_kernel_buffer"),
                    contents: bytemuck::bytes_of(&kernel_data),
                    usage: BufferUsages::UNIFORM,
                });

        // Create kernel + noise bind group (group 1)
        let kernel_bind_group = render_context.render_device().create_bind_group(
            "ssao_kernel_bind_group",
            &ssao_pipeline.kernel_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: kernel_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&noise_texture.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&ssao_pipeline.noise_sampler),
                },
            ],
        );

        // Compute matrices
        // view.world_from_view is the camera's GlobalTransform (world position/rotation)
        // We need view_from_world (the inverse) to transform world coords to view space
        let world_from_view = view.world_from_view.to_matrix();
        let view_from_world = world_from_view.inverse();

        // Projection matrix
        let projection = view.clip_from_view;

        // Inverse projection for reconstructing view positions from depth
        let inv_projection = projection.inverse();

        let full_screen_size = camera
            .physical_viewport_size
            .unwrap_or(UVec2::new(1920, 1080));
        
        // SSAO renders at half resolution like Bonsai
        let half_screen_size = UVec2::new(
            (full_screen_size.x / 2).max(1),
            (full_screen_size.y / 2).max(1),
        );

        let camera_uniform = SsaoCameraUniform {
            view: view_from_world.to_cols_array_2d(),
            projection: projection.to_cols_array_2d(),
            inv_projection: inv_projection.to_cols_array_2d(),
            // Use half resolution for SSAO pass
            screen_size: [
                half_screen_size.x as f32,
                half_screen_size.y as f32,
                1.0 / half_screen_size.x as f32,
                1.0 / half_screen_size.y as f32,
            ],
            // XeGTAO Defaults
            params1: [
                0.5,   // EffectRadius (World space)
                0.615, // EffectFalloffRange (Default)
                1.457, // RadiusMultiplier (Default)
                2.0,   // SampleDistributionPower (Default)
            ],
            params2: [
                0.0,   // ThinOccluderCompensation (Default)
                2.2,   // FinalValuePower (Default)
                3.30,  // DepthMIPSamplingOffset (Default)
                0.0,   // Unused
            ],
            tan_half_fov: [
                1.0 / projection.x_axis.x,
                1.0 / projection.y_axis.y,
            ],
            padding: [0.0; 2],
        };

        let camera_buffer =
            render_context
                .render_device()
                .create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("ssao_camera_buffer"),
                    contents: bytemuck::bytes_of(&camera_uniform),
                    usage: BufferUsages::UNIFORM,
                });

        let camera_bind_group = render_context.render_device().create_bind_group(
            "ssao_camera_bind_group",
            &ssao_pipeline.camera_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        );

        // Begin render pass writing to SSAO texture
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("ssao_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &ssao_texture.texture.default_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color::WHITE), // Default to fully lit
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Set viewport to half resolution
        render_pass.set_viewport(
            0.0,
            0.0,
            half_screen_size.x as f32,
            half_screen_size.y as f32,
            0.0,
            1.0,
        );

        // Draw fullscreen triangle
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &gbuffer_bind_group, &[]);
        render_pass.set_bind_group(1, &kernel_bind_group, &[]);
        render_pass.set_bind_group(2, &camera_bind_group, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

/// Pipeline resources for SSAO.
#[derive(Resource)]
pub struct SsaoPipeline {
    pub pipeline_id: CachedRenderPipelineId,
    /// G-buffer textures layout (group 0)
    pub gbuffer_layout: BindGroupLayout,
    /// G-buffer sampler
    pub gbuffer_sampler: Sampler,
    /// Kernel and noise layout (group 1)
    pub kernel_layout: BindGroupLayout,
    /// Noise sampler (repeating)
    pub noise_sampler: Sampler,
    /// Camera matrices layout (group 2)
    pub camera_layout: BindGroupLayout,
}

/// 4x4 noise texture containing random rotation vectors for SSAO.
#[derive(Resource)]
pub struct SsaoNoiseTexture {
    #[allow(dead_code)]
    pub texture: bevy::render::render_resource::Texture,
    pub view: bevy::render::render_resource::TextureView,
}

/// System to initialize the SSAO pipeline.
pub fn init_ssao_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<SsaoPipeline>>,
) {
    if existing.is_some() {
        return;
    }

    // Group 0: G-buffer textures (normal, position, sampler)
    let gbuffer_layout = render_device.create_bind_group_layout(
        "ssao_gbuffer_layout",
        &[
            // Normal texture
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Position texture
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Sampler
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    );

    // Group 1: Kernel samples and noise texture
    let kernel_layout = render_device.create_bind_group_layout(
        "ssao_kernel_layout",
        &[
            // Kernel uniform buffer (64 samples)
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Noise texture (4x4)
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
            // Noise sampler
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    );

    // Group 2: Camera matrices
    let camera_layout = render_device.create_bind_group_layout(
        "ssao_camera_layout",
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    );

    // Create samplers
    let gbuffer_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("ssao_gbuffer_sampler"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..default()
    });

    let noise_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("ssao_noise_sampler"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        address_mode_u: bevy::render::render_resource::AddressMode::Repeat,
        address_mode_v: bevy::render::render_resource::AddressMode::Repeat,
        ..default()
    });

    // Load shader
    let shader = asset_server.load("shaders/ssao.wgsl");

    // Queue pipeline creation
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("ssao_pipeline".into()),
        layout: vec![
            gbuffer_layout.clone(),
            kernel_layout.clone(),
            camera_layout.clone(),
        ],
        push_constant_ranges: vec![],
        vertex: VertexState {
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Some("vs_main".into()),
            buffers: vec![],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            shader,
            shader_defs: vec![],
            entry_point: Some("fs_main".into()),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::R8Unorm,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        zero_initialize_workgroup_memory: false,
    });

    commands.insert_resource(SsaoPipeline {
        pipeline_id,
        gbuffer_layout,
        gbuffer_sampler,
        kernel_layout,
        noise_sampler,
        camera_layout,
    });
}

/// Noise texture size for kernel rotation randomization
const NOISE_TEXTURE_SIZE: u32 = 32;

/// System to create the noise texture with random rotation vectors.
/// Using 32x32 (like Bonsai) instead of 4x4 to reduce visible tiling patterns.
pub fn init_ssao_noise_texture(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    existing: Option<Res<SsaoNoiseTexture>>,
) {
    if existing.is_some() {
        return;
    }

    use rand::prelude::*;
    let mut rng = rand::thread_rng();

    // Generate 32x32 noise texture with random tangent-space rotation vectors
    // Each pixel stores a random vector in the XY plane (Z=0) for rotating the sample kernel
    // Bonsai uses 32x32 which tiles every 32 pixels instead of every 4 - much less noticeable
    let pixel_count = (NOISE_TEXTURE_SIZE * NOISE_TEXTURE_SIZE) as usize;
    let mut noise_data = Vec::with_capacity(pixel_count * 4); // RGBA8

    for _ in 0..pixel_count {
        // Random angle for rotation (matching Bonsai's approach)
        let angle: f32 = rng.gen::<f32>() * std::f32::consts::TAU;
        let x = angle.cos();
        let y = angle.sin();

        // Store as RGBA8 normalized: [-1,1] -> [0,255]
        noise_data.push(((x * 0.5 + 0.5) * 255.0) as u8);
        noise_data.push(((y * 0.5 + 0.5) * 255.0) as u8);
        noise_data.push(128); // Z = 0
        noise_data.push(255); // A = 1
    }

    // Create the texture
    let texture = render_device.create_texture(&TextureDescriptor {
        label: Some("ssao_noise_texture"),
        size: Extent3d {
            width: NOISE_TEXTURE_SIZE,
            height: NOISE_TEXTURE_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Write data to texture using wgpu's write_texture
    render_queue.0.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &noise_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * NOISE_TEXTURE_SIZE), // 4 bytes per pixel
            rows_per_image: Some(NOISE_TEXTURE_SIZE),
        },
        wgpu::Extent3d {
            width: NOISE_TEXTURE_SIZE,
            height: NOISE_TEXTURE_SIZE,
            depth_or_array_layers: 1,
        },
    );

    // Create texture view
    let view = texture.create_view(&bevy::render::render_resource::TextureViewDescriptor::default());

    commands.insert_resource(SsaoNoiseTexture { texture, view });
}
