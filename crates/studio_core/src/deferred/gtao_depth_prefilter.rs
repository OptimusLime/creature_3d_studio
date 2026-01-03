//! GTAO Depth Prefilter - generates 5-level depth MIP pyramid.
//!
//! This compute shader pass converts the hardware depth buffer to linearized
//! viewspace depth and generates a 5-level MIP chain with weighted average
//! filtering that preserves depth edges.
//!
//! Based on XeGTAO_PrefilterDepths16x16 from XeGTAO.hlsli L617-684.
//!
//! Reference: https://github.com/GameTechDev/XeGTAO

use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
        BufferBindingType, BufferInitDescriptor, BufferUsages, CachedComputePipelineId,
        ComputePassDescriptor, ComputePipelineDescriptor, Extent3d, PipelineCache, ShaderStages,
        StorageTextureAccess, TextureDescriptor, TextureDimension, TextureFormat,
        TextureSampleType, TextureUsages, TextureViewDimension,
    },
    renderer::{RenderContext, RenderDevice},
    texture::{CachedTexture, TextureCache},
};

use super::gbuffer::ViewGBufferTextures;
use super::gtao::GtaoConfig;

/// Number of depth MIP levels (XeGTAO uses 5)
pub const DEPTH_MIP_LEVELS: u32 = 5;

/// GPU uniform for depth prefilter compute shader.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DepthPrefilterUniform {
    /// Full resolution viewport size
    pub viewport_size: [f32; 2],
    /// 1.0 / viewport_size
    pub viewport_pixel_size: [f32; 2],
    /// xy = depthLinearizeMul, depthLinearizeAdd
    pub depth_unpack_consts: [f32; 2],
    /// Effect radius for MIP filtering
    pub effect_radius: f32,
    /// Falloff range for MIP filtering
    pub effect_falloff_range: f32,
    /// Radius multiplier
    pub radius_multiplier: f32,
    /// Padding for alignment
    pub _padding: [f32; 3],
}

/// Per-view depth MIP chain textures.
#[derive(Component)]
pub struct ViewDepthMipTextures {
    /// MIP level 0 (full resolution linearized depth)
    pub mip0: CachedTexture,
    /// MIP level 1 (half resolution)
    pub mip1: CachedTexture,
    /// MIP level 2 (quarter resolution)
    pub mip2: CachedTexture,
    /// MIP level 3 (1/8 resolution)
    pub mip3: CachedTexture,
    /// MIP level 4 (1/16 resolution)
    pub mip4: CachedTexture,
}

/// Pipeline resources for depth prefilter.
#[derive(Resource)]
pub struct DepthPrefilterPipeline {
    pub pipeline_id: CachedComputePipelineId,
    /// Uniforms + source depth layout (group 0)
    pub uniforms_layout: BindGroupLayout,
    /// Output MIP textures layout (group 1)
    pub output_layout: BindGroupLayout,
}

/// Render graph node that generates the depth MIP chain.
#[derive(Default)]
pub struct DepthPrefilterNode;

impl ViewNode for DepthPrefilterNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewGBufferTextures,
        &'static ViewDepthMipTextures,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, gbuffer, depth_mips): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(prefilter_pipeline) = world.get_resource::<DepthPrefilterPipeline>() else {
            return Ok(());
        };

        let Some(pipeline) = pipeline_cache.get_compute_pipeline(prefilter_pipeline.pipeline_id)
        else {
            return Ok(());
        };

        let gtao_config = world
            .get_resource::<GtaoConfig>()
            .cloned()
            .unwrap_or_default();

        let viewport_size = camera
            .physical_viewport_size
            .unwrap_or(UVec2::new(1920, 1080));

        // Get near plane from camera for depth linearization
        // For Bevy reverse-Z: near is stored in projection[3][2]
        let near = 0.1_f32; // Default near plane

        let uniform = DepthPrefilterUniform {
            viewport_size: [viewport_size.x as f32, viewport_size.y as f32],
            viewport_pixel_size: [1.0 / viewport_size.x as f32, 1.0 / viewport_size.y as f32],
            depth_unpack_consts: [near, 0.0001], // mul, add for Bevy reverse-Z
            effect_radius: gtao_config.effect_radius,
            effect_falloff_range: gtao_config.effect_falloff_range,
            radius_multiplier: gtao_config.radius_multiplier,
            _padding: [0.0; 3],
        };

        let uniform_buffer =
            render_context
                .render_device()
                .create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("depth_prefilter_uniform_buffer"),
                    contents: bytemuck::bytes_of(&uniform),
                    usage: BufferUsages::UNIFORM,
                });

        // Create bind group for uniforms + source depth (group 0)
        let uniforms_bind_group = render_context.render_device().create_bind_group(
            "depth_prefilter_uniforms_bind_group",
            &prefilter_pipeline.uniforms_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&gbuffer.depth.default_view),
                },
            ],
        );

        // Create bind group for output MIP textures (group 1)
        let output_bind_group = render_context.render_device().create_bind_group(
            "depth_prefilter_output_bind_group",
            &prefilter_pipeline.output_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&depth_mips.mip0.default_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&depth_mips.mip1.default_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&depth_mips.mip2.default_view),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&depth_mips.mip3.default_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(&depth_mips.mip4.default_view),
                },
            ],
        );

        // Dispatch compute shader
        // Each workgroup handles 16x16 pixels at MIP 0 (8x8 threads, each handling 2x2)
        let dispatch_x = (viewport_size.x + 15) / 16;
        let dispatch_y = (viewport_size.y + 15) / 16;

        {
            let mut pass =
                render_context
                    .command_encoder()
                    .begin_compute_pass(&ComputePassDescriptor {
                        label: Some("depth_prefilter_pass"),
                        timestamp_writes: None,
                    });

            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &uniforms_bind_group, &[]);
            pass.set_bind_group(1, &output_bind_group, &[]);
            pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }

        Ok(())
    }
}

/// System to initialize the depth prefilter pipeline.
pub fn init_depth_prefilter_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<DepthPrefilterPipeline>>,
) {
    if existing.is_some() {
        return;
    }

    // Group 0: Uniforms + source depth
    let uniforms_layout = render_device.create_bind_group_layout(
        "depth_prefilter_uniforms_layout",
        &[
            // Uniforms
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Source depth texture
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    );

    // Group 1: Output MIP textures (storage textures)
    let output_layout = render_device.create_bind_group_layout(
        "depth_prefilter_output_layout",
        &[
            // MIP 0
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format: TextureFormat::R16Float,
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            },
            // MIP 1
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format: TextureFormat::R16Float,
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            },
            // MIP 2
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format: TextureFormat::R16Float,
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            },
            // MIP 3
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format: TextureFormat::R16Float,
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            },
            // MIP 4
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format: TextureFormat::R16Float,
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            },
        ],
    );

    // Load shader
    let shader = asset_server.load("shaders/gtao_depth_prefilter.wgsl");

    // Queue pipeline creation
    let pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("depth_prefilter_pipeline".into()),
        layout: vec![uniforms_layout.clone(), output_layout.clone()],
        push_constant_ranges: vec![],
        shader,
        shader_defs: vec![],
        entry_point: Some("main".into()),
        zero_initialize_workgroup_memory: true,
    });

    commands.insert_resource(DepthPrefilterPipeline {
        pipeline_id,
        uniforms_layout,
        output_layout,
    });
}

/// System to prepare depth MIP textures for each view.
pub fn prepare_depth_mip_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    views: Query<(Entity, &ExtractedCamera), Without<ViewDepthMipTextures>>,
) {
    for (entity, camera) in views.iter() {
        let Some(size) = camera.physical_viewport_size else {
            continue;
        };

        // Create textures for each MIP level
        // MIP 0 = full resolution, MIP 1 = half, etc.
        let mut create_mip_texture = |label: &'static str, width: u32, height: u32| {
            texture_cache.get(
                &render_device,
                TextureDescriptor {
                    label: Some(label),
                    size: Extent3d {
                        width: width.max(1),
                        height: height.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::R16Float,
                    usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        };

        let mip0 = create_mip_texture("gtao_depth_mip0", size.x, size.y);
        let mip1 = create_mip_texture("gtao_depth_mip1", size.x / 2, size.y / 2);
        let mip2 = create_mip_texture("gtao_depth_mip2", size.x / 4, size.y / 4);
        let mip3 = create_mip_texture("gtao_depth_mip3", size.x / 8, size.y / 8);
        let mip4 = create_mip_texture("gtao_depth_mip4", size.x / 16, size.y / 16);

        commands.entity(entity).insert(ViewDepthMipTextures {
            mip0,
            mip1,
            mip2,
            mip3,
            mip4,
        });
    }
}
