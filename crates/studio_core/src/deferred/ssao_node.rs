//! SSAO render graph node.
//!
//! This node performs a fullscreen pass that computes screen-space ambient occlusion
//! by sampling the G-buffer depth/normal and outputting to an SSAO texture.

use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
        BufferBindingType, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId,
        ColorTargetState, ColorWrites, FilterMode, FragmentState, LoadOp, MultisampleState,
        Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment,
        RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
        SamplerDescriptor, ShaderStages, StoreOp, TextureFormat, TextureSampleType,
        TextureViewDimension, VertexState,
    },
    renderer::{RenderContext, RenderDevice},
    view::ExtractedView,
};

use super::gbuffer::ViewGBufferTextures;
use super::ssao::{SsaoKernel, ViewSsaoTexture};

/// GPU uniform for SSAO kernel samples.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SsaoKernelUniform {
    /// 32 hemisphere sample directions (vec4 for alignment, only xyz used)
    pub samples: [[f32; 4]; 32],
}

/// GPU uniform for camera matrices needed for SSAO.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SsaoCameraUniform {
    /// View-projection matrix for reprojecting sample points
    pub view_proj: [[f32; 4]; 4],
    /// Screen dimensions (width, height, 1/width, 1/height)
    pub screen_size: [f32; 4],
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

        let Some(ssao_pipeline) = ssao_pipeline else {
            return Ok(());
        };

        let Some(pipeline) = pipeline_cache.get_render_pipeline(ssao_pipeline.pipeline_id) else {
            return Ok(());
        };

        let Some(ssao_kernel) = ssao_kernel else {
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

        // Create kernel uniform buffer
        let mut kernel_data = SsaoKernelUniform { samples: [[0.0; 4]; 32] };
        for (i, sample) in ssao_kernel.samples.iter().enumerate().take(32) {
            kernel_data.samples[i] = *sample;
        }

        let kernel_buffer = render_context.render_device().create_buffer_with_data(&BufferInitDescriptor {
            label: Some("ssao_kernel_buffer"),
            contents: bytemuck::bytes_of(&kernel_data),
            usage: BufferUsages::UNIFORM,
        });

        // Create noise texture bind group (group 1)
        // For now, use a placeholder approach - we'll create proper noise later
        let kernel_bind_group = render_context.render_device().create_bind_group(
            "ssao_kernel_bind_group",
            &ssao_pipeline.kernel_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: kernel_buffer.as_entire_binding(),
                },
                // Use normal texture as noise placeholder for now
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&gbuffer.normal.default_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&ssao_pipeline.noise_sampler),
                },
            ],
        );

        // Create camera uniform from ExtractedView
        let view_proj = view.clip_from_world.unwrap_or(view.clip_from_view);
        let screen_size = camera.physical_viewport_size.unwrap_or(UVec2::new(1920, 1080));
        
        let camera_uniform = SsaoCameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            screen_size: [
                screen_size.x as f32,
                screen_size.y as f32,
                1.0 / screen_size.x as f32,
                1.0 / screen_size.y as f32,
            ],
        };

        let camera_buffer = render_context.render_device().create_buffer_with_data(&BufferInitDescriptor {
            label: Some("ssao_camera_buffer"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: BufferUsages::UNIFORM,
        });

        let camera_bind_group = render_context.render_device().create_bind_group(
            "ssao_camera_bind_group",
            &ssao_pipeline.camera_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
            ],
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

        // Set viewport
        if let Some(viewport) = &camera.viewport {
            render_pass.set_camera_viewport(viewport);
        }

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
            // Kernel uniform buffer
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
            // Noise texture
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
        &[
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
        ],
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
