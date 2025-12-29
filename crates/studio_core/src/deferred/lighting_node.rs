//! Deferred lighting render graph node.
//!
//! This node performs a fullscreen pass that:
//! 1. Reads from the G-buffer textures (color, normal, position)
//! 2. Samples the shadow map for shadow determination
//! 3. Computes lighting (directional + point lights)
//! 4. Applies fog
//! 5. Outputs to the view target

use bevy::prelude::*;
use bevy::image::BevyDefault;
use bevy::render::{
    camera::ExtractedCamera,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
        BufferBindingType, CachedRenderPipelineId, ColorTargetState, ColorWrites, CompareFunction,
        FilterMode, FragmentState, LoadOp, MultisampleState, Operations, PipelineCache,
        PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
        Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, StoreOp, TextureFormat,
        TextureSampleType, TextureViewDimension, VertexState,
    },
    renderer::{RenderContext, RenderDevice},
    view::ViewTarget,
};

use super::gbuffer::ViewGBufferTextures;
use super::point_light::PointLightsBuffer;
use super::shadow::ViewShadowTextures;
use super::shadow_node::ViewShadowUniforms;

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
        &'static ViewShadowTextures,
        &'static ViewShadowUniforms,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, target, gbuffer, shadow_textures, shadow_uniforms): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
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

        // Create bind group for G-buffer textures (group 0)
        let gbuffer_bind_group = render_context.render_device().create_bind_group(
            "lighting_gbuffer_bind_group",
            &lighting_pipeline.gbuffer_layout,
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
                    resource: BindingResource::Sampler(&lighting_pipeline.gbuffer_sampler),
                },
            ],
        );
        
        // Create bind group for shadow map (group 1)
        let shadow_map_bind_group = render_context.render_device().create_bind_group(
            "lighting_shadow_bind_group",
            &lighting_pipeline.shadow_map_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&shadow_textures.depth.default_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&lighting_pipeline.shadow_sampler),
                },
            ],
        );
        
        // Create bind group for shadow uniforms (group 2)
        let shadow_uniforms_bind_group = render_context.render_device().create_bind_group(
            "lighting_shadow_uniforms_bind_group",
            &lighting_pipeline.shadow_uniforms_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: shadow_uniforms.buffer.as_entire_binding(),
            }],
        );
        
        // Create bind group for point lights (group 3)
        let point_lights_bind_group = if let Some(point_lights_buffer) = world.get_resource::<PointLightsBuffer>() {
            Some(render_context.render_device().create_bind_group(
                "lighting_point_lights_bind_group",
                &lighting_pipeline.point_lights_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: point_lights_buffer.buffer.as_entire_binding(),
                }],
            ))
        } else {
            None
        };

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
        render_pass.set_bind_group(0, &gbuffer_bind_group, &[]);
        render_pass.set_bind_group(1, &shadow_map_bind_group, &[]);
        render_pass.set_bind_group(2, &shadow_uniforms_bind_group, &[]);
        if let Some(ref point_lights_bg) = point_lights_bind_group {
            render_pass.set_bind_group(3, point_lights_bg, &[]);
        }
        
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

/// Pipeline resources for deferred lighting.
#[derive(Resource)]
pub struct LightingPipeline {
    pub pipeline_id: CachedRenderPipelineId,
    /// G-buffer textures layout (group 0)
    pub gbuffer_layout: BindGroupLayout,
    /// G-buffer sampler (point sampling)
    pub gbuffer_sampler: Sampler,
    /// Shadow map texture layout (group 1)
    pub shadow_map_layout: BindGroupLayout,
    /// Shadow comparison sampler
    pub shadow_sampler: Sampler,
    /// Shadow uniforms layout (group 2)
    pub shadow_uniforms_layout: BindGroupLayout,
    /// Point lights uniform layout (group 3)
    pub point_lights_layout: BindGroupLayout,
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

    // Create bind group layout for G-buffer textures (group 0)
    let gbuffer_layout = render_device.create_bind_group_layout(
        "lighting_gbuffer_layout",
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
            // G-buffer sampler (non-filtering since we sample position texture which is Rgba32Float)
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    );
    
    // Create bind group layout for shadow map (group 1)
    let shadow_map_layout = render_device.create_bind_group_layout(
        "lighting_shadow_map_layout",
        &[
            // Shadow depth texture
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Shadow comparison sampler
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Comparison),
                count: None,
            },
        ],
    );
    
    // Create bind group layout for shadow uniforms (group 2)
    let shadow_uniforms_layout = render_device.create_bind_group_layout(
        "lighting_shadow_uniforms_layout",
        &[
            // Light-space view-projection matrix
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
    
    // Create bind group layout for point lights (group 3)
    // Using Storage buffer for higher light counts (256+)
    let point_lights_layout = render_device.create_bind_group_layout(
        "lighting_point_lights_layout",
        &[
            // Point lights storage buffer (read-only)
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    );

    // Create G-buffer sampler (point sampling)
    let gbuffer_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("gbuffer_sampler"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..default()
    });
    
    // Create shadow comparison sampler
    let shadow_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("shadow_comparison_sampler"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        compare: Some(CompareFunction::LessEqual),
        ..default()
    });

    // Load shader via asset server
    let shader = asset_server.load("shaders/deferred_lighting.wgsl");

    // Queue pipeline creation with all bind group layouts
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("deferred_lighting_pipeline".into()),
        layout: vec![
            gbuffer_layout.clone(),
            shadow_map_layout.clone(),
            shadow_uniforms_layout.clone(),
            point_lights_layout.clone(),
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
                format: TextureFormat::bevy_default(),
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        zero_initialize_workgroup_memory: false,
    });

    commands.insert_resource(LightingPipeline {
        pipeline_id,
        gbuffer_layout,
        gbuffer_sampler,
        shadow_map_layout,
        shadow_sampler,
        shadow_uniforms_layout,
        point_lights_layout,
    });
    
    info!("LightingPipeline initialized with shadow mapping and point lights support (storage buffer, max {})", 
          super::point_light::MAX_POINT_LIGHTS);
}
