//! Deferred lighting render graph node.
//!
//! This node performs a fullscreen pass that:
//! 1. Reads from the G-buffer textures (color, normal, position)
//! 2. Computes lighting (directional + point lights)
//! 3. Applies fog
//! 4. Outputs to the view target

use bevy::prelude::*;
use bevy::image::BevyDefault;
use bevy::render::{
    camera::ExtractedCamera,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
        CachedRenderPipelineId, ColorTargetState, ColorWrites, FilterMode, FragmentState, LoadOp,
        MultisampleState, Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment,
        RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
        SamplerDescriptor, ShaderStages, StoreOp, TextureFormat, TextureSampleType,
        TextureViewDimension, VertexState,
    },
    renderer::{RenderContext, RenderDevice},
    view::ViewTarget,
};

use super::gbuffer::ViewGBufferTextures;

/// Render graph node that performs deferred lighting.
///
/// Draws a fullscreen triangle, samples G-buffer textures,
/// computes lighting, and writes to the view target.
#[derive(Default)]
pub struct LightingPassNode;

impl ViewNode for LightingPassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        &'static ViewGBufferTextures,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, target, gbuffer): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let lighting_pipeline = world.get_resource::<LightingPipeline>();
        
        let Some(lighting_pipeline) = lighting_pipeline else {
            // Pipeline not ready yet
            return Ok(());
        };
        
        let Some(pipeline) = pipeline_cache.get_render_pipeline(lighting_pipeline.pipeline_id) else {
            // Pipeline still compiling
            return Ok(());
        };

        // Create bind group for G-buffer textures
        let bind_group = render_context.render_device().create_bind_group(
            "lighting_bind_group",
            &lighting_pipeline.bind_group_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&gbuffer.color.default_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&gbuffer.normal.default_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&gbuffer.position.default_view),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::Sampler(&lighting_pipeline.sampler),
                },
            ],
        );

        // Begin render pass writing to view target
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("lighting_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target.main_texture_view(),
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color {
                        r: 0.102, // #1a = 26/255
                        g: 0.039, // #0a = 10/255
                        b: 0.180, // #2e = 46/255
                        a: 1.0,
                    }),
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
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

/// Pipeline resources for deferred lighting.
#[derive(Resource)]
pub struct LightingPipeline {
    pub pipeline_id: CachedRenderPipelineId,
    pub bind_group_layout: BindGroupLayout,
    pub sampler: Sampler,
}

/// System to initialize the lighting pipeline on first run.
/// This runs in the Render schedule after the RenderDevice exists.
pub fn init_lighting_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<LightingPipeline>>,
) {
    // Only initialize once
    if existing.is_some() {
        return;
    }
    

    // Create bind group layout for G-buffer textures
    let bind_group_layout = render_device.create_bind_group_layout(
        "lighting_bind_group_layout",
        &[
            // gColor texture (Rgba16Float is filterable but we use point sampling)
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
            // gNormal texture (Rgba16Float is filterable but we use point sampling)
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
            // gPosition texture
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
            // Sampler (non-filtering since we sample position texture which is Rgba32Float)
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    );

    // Create sampler
    let sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("gbuffer_sampler"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..default()
    });

    // Load shader via asset server
    let shader = asset_server.load("shaders/deferred_lighting.wgsl");

    // Queue pipeline creation
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("deferred_lighting_pipeline".into()),
        layout: vec![bind_group_layout.clone()],
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

    commands.insert_resource(LightingPipeline {
        pipeline_id,
        bind_group_layout,
        sampler,
    });
}
