//! Sky dome render graph node.
//!
//! This node performs a fullscreen pass that renders procedural sky
//! where no geometry exists (depth > 999.0).
//!
//! Runs after bloom pass, before transparent pass.

use bevy::image::BevyDefault;
use bevy::prelude::*;
use bevy::render::{
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
        BufferBindingType, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId,
        ColorTargetState, ColorWrites, FilterMode, FragmentState, LoadOp, MultisampleState,
        Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor,
        RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
        StoreOp, TextureFormat, TextureSampleType, TextureViewDimension, VertexState,
    },
    renderer::{RenderContext, RenderDevice},
    view::{ExtractedView, ViewTarget},
};

use super::gbuffer::ViewGBufferTextures;
use super::sky_dome::SkyDomeConfig;

/// GPU uniform structure for sky dome rendering.
/// Must match the WGSL struct layout exactly.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkyDomeUniform {
    /// Inverse view-projection matrix for reconstructing view direction
    pub inv_view_proj: [[f32; 4]; 4],
    /// Horizon color (rgb, a unused)
    pub horizon_color: [f32; 4],
    /// Zenith color (rgb, a unused)
    pub zenith_color: [f32; 4],
    /// x = blend_power, yzw = unused
    pub params: [f32; 4],
}

/// Render graph node for sky dome rendering.
///
/// Draws a fullscreen triangle, checks depth from G-buffer,
/// and renders sky gradient where no geometry exists.
#[derive(Default)]
pub struct SkyDomeNode;

impl ViewNode for SkyDomeNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewGBufferTextures,
        &'static ExtractedView,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view_target, gbuffer, view): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // Check if sky dome is enabled
        let config = world
            .get_resource::<SkyDomeConfig>()
            .cloned()
            .unwrap_or_default();
        if !config.enabled {
            return Ok(());
        }

        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(sky_pipeline) = world.get_resource::<SkyDomePipeline>() else {
            return Ok(());
        };

        let Some(pipeline) = pipeline_cache.get_render_pipeline(sky_pipeline.pipeline_id) else {
            // Pipeline not ready yet (shader still loading)
            return Ok(());
        };

        // Use post_process_write to get source (current frame) and destination (output)
        // This swaps the buffers so we read from source and write to destination
        let post_process = view_target.post_process_write();

        // Compute inverse view-projection matrix for reconstructing view direction
        let view_from_world = view.world_from_view.to_matrix().inverse();
        let clip_from_world = view
            .clip_from_world
            .unwrap_or(view.clip_from_view * view_from_world);
        let inv_view_proj = clip_from_world.inverse();

        // Convert colors to linear space arrays
        let horizon_linear = config.horizon_color.to_linear();
        let zenith_linear = config.zenith_color.to_linear();

        // Create uniform data
        let uniform = SkyDomeUniform {
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            horizon_color: [
                horizon_linear.red,
                horizon_linear.green,
                horizon_linear.blue,
                1.0,
            ],
            zenith_color: [
                zenith_linear.red,
                zenith_linear.green,
                zenith_linear.blue,
                1.0,
            ],
            params: [config.horizon_blend_power, 0.0, 0.0, 0.0],
        };

        // Create uniform buffer
        let uniform_buffer =
            render_context
                .render_device()
                .create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("sky_dome_uniform"),
                    contents: bytemuck::bytes_of(&uniform),
                    usage: BufferUsages::UNIFORM,
                });

        // Create bind group for scene texture + G-buffer position (group 0)
        let textures_bind_group = render_context.render_device().create_bind_group(
            Some("sky_dome_textures_bind_group"),
            &sky_pipeline.textures_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(post_process.source),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sky_pipeline.scene_sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&gbuffer.position.default_view),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::Sampler(&sky_pipeline.position_sampler),
                },
            ],
        );

        // Create bind group for uniforms (group 1)
        let uniforms_bind_group = render_context.render_device().create_bind_group(
            Some("sky_dome_uniforms_bind_group"),
            &sky_pipeline.uniforms_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        );

        // Begin render pass writing to destination
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("sky_dome_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load, // Preserve existing content (we selectively overwrite sky pixels)
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Draw fullscreen triangle
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &textures_bind_group, &[]);
        render_pass.set_bind_group(1, &uniforms_bind_group, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

/// Pipeline resources for sky dome rendering.
#[derive(Resource)]
pub struct SkyDomePipeline {
    pub pipeline_id: CachedRenderPipelineId,
    /// Textures bind group layout (group 0)
    pub textures_layout: BindGroupLayout,
    /// Uniforms bind group layout (group 1)
    pub uniforms_layout: BindGroupLayout,
    /// Linear filtering sampler for scene texture
    pub scene_sampler: Sampler,
    /// Non-filtering sampler for G-buffer position (Rgba32Float not filterable)
    pub position_sampler: Sampler,
}

/// System to initialize the sky dome pipeline.
/// Runs in the Render schedule after RenderDevice exists.
pub fn init_sky_dome_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<SkyDomePipeline>>,
) {
    // Only initialize once
    if existing.is_some() {
        return;
    }

    // Create textures bind group layout (group 0)
    // - binding 0: scene texture (post-bloom output)
    // - binding 1: scene sampler (filtering)
    // - binding 2: G-buffer position texture (for depth check)
    // - binding 3: position sampler (non-filtering, since Rgba32Float)
    let textures_layout = render_device.create_bind_group_layout(
        "sky_dome_textures_layout",
        &[
            // Scene texture
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
            // Scene sampler (filtering)
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
            // G-buffer position texture (Rgba32Float - not filterable)
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Position sampler (non-filtering for Rgba32Float)
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    );

    // Create uniforms bind group layout (group 1)
    let uniforms_layout = render_device.create_bind_group_layout(
        "sky_dome_uniforms_layout",
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

    // Create scene sampler (linear filtering for scene texture)
    let scene_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("sky_dome_scene_sampler"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });

    // Create position sampler (non-filtering for G-buffer position)
    let position_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("sky_dome_position_sampler"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..default()
    });

    // Load shader
    let shader = asset_server.load("shaders/sky_dome.wgsl");

    // Queue pipeline creation with both bind group layouts
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("sky_dome_pipeline".into()),
        layout: vec![textures_layout.clone(), uniforms_layout.clone()],
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
                format: TextureFormat::bevy_default(),
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        zero_initialize_workgroup_memory: false,
    });

    commands.insert_resource(SkyDomePipeline {
        pipeline_id,
        textures_layout,
        uniforms_layout,
        scene_sampler,
        position_sampler,
    });
}
