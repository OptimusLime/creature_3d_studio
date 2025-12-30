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
        BufferBindingType, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId, 
        ColorTargetState, ColorWrites, CompareFunction, FilterMode, FragmentState, LoadOp, 
        MultisampleState, Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment, 
        RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType, 
        SamplerDescriptor, ShaderStages, StoreOp, TextureFormat, TextureSampleType, 
        TextureViewDimension, VertexState,
    },
    renderer::{RenderContext, RenderDevice},
    view::ViewTarget,
};

use super::gbuffer::ViewGBufferTextures;
use super::point_light::PointLightsBuffer;
use super::point_light_shadow::{ViewPointShadowTextures, ShadowCastingLights, CubeFaceMatrices};
use super::shadow::ViewDirectionalShadowTextures;
use super::shadow_node::ViewDirectionalShadowUniforms;
use super::gtao::ViewGtaoTexture;
use super::gtao_denoise::ViewGtaoDenoised;

/// GPU uniform data for point shadow view-projection matrices.
/// Contains the 6 face matrices needed to sample the cube shadow map correctly.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointShadowMatricesUniform {
    /// View-projection matrices for each cube face: +X, -X, +Y, -Y, +Z, -Z
    pub face_matrices: [[[f32; 4]; 4]; 6],
    /// Light position (xyz) and radius (w)
    pub light_pos_radius: [f32; 4],
}

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
        &'static ViewDirectionalShadowTextures,
        &'static ViewDirectionalShadowUniforms,
        &'static ViewPointShadowTextures,
        Option<&'static ViewGtaoTexture>,
        Option<&'static ViewGtaoDenoised>,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, target, gbuffer, shadow_textures, shadow_uniforms, point_shadow_textures, gtao_texture, gtao_denoised): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
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
        
        // Create bind group for dual directional shadow maps (group 1)
        let shadow_map_bind_group = render_context.render_device().create_bind_group(
            "lighting_directional_shadow_bind_group",
            &lighting_pipeline.directional_shadow_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&shadow_textures.moon1.default_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&shadow_textures.moon2.default_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&lighting_pipeline.shadow_sampler),
                },
            ],
        );
        
        // Create bind group for directional shadow uniforms (group 2)
        let shadow_uniforms_bind_group = render_context.render_device().create_bind_group(
            "lighting_directional_shadow_uniforms_bind_group",
            &lighting_pipeline.directional_shadow_uniforms_layout,
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
        
        // Create bind group for point light shadows (group 4)
        // Uses the 6 face textures from the first shadow-casting light
        let point_shadow_bind_group = if let Some(shadow_map) = point_shadow_textures.shadow_maps.first() {

            Some(render_context.render_device().create_bind_group(
                "lighting_point_shadow_bind_group",
                &lighting_pipeline.point_shadow_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&shadow_map.faces[0].default_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&shadow_map.faces[1].default_view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::TextureView(&shadow_map.faces[2].default_view),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::TextureView(&shadow_map.faces[3].default_view),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: BindingResource::TextureView(&shadow_map.faces[4].default_view),
                    },
                    BindGroupEntry {
                        binding: 5,
                        resource: BindingResource::TextureView(&shadow_map.faces[5].default_view),
                    },
                    BindGroupEntry {
                        binding: 6,
                        resource: BindingResource::Sampler(&lighting_pipeline.point_shadow_sampler),
                    },
                ],
            ))
        } else {
            None
        };
        
        // Create bind group for point shadow matrices (group 5)
        // Get the shadow casting lights to compute the matrices
        // ALWAYS create this bind group - use identity matrices if no lights
        let point_shadow_matrices_bind_group = {
            let uniform = if let Some(shadow_lights) = world.get_resource::<ShadowCastingLights>() {
                if let Some(light) = shadow_lights.lights.first() {
                    // Compute the view-proj matrices for this light
                    let matrices = CubeFaceMatrices::new(light.position, 0.1, light.radius);
                    
                    PointShadowMatricesUniform {
                        face_matrices: [
                            matrices.view_proj[0].to_cols_array_2d(),
                            matrices.view_proj[1].to_cols_array_2d(),
                            matrices.view_proj[2].to_cols_array_2d(),
                            matrices.view_proj[3].to_cols_array_2d(),
                            matrices.view_proj[4].to_cols_array_2d(),
                            matrices.view_proj[5].to_cols_array_2d(),
                        ],
                        light_pos_radius: [light.position.x, light.position.y, light.position.z, light.radius],
                    }
                } else {
                    // No lights - use dummy matrices with radius 0 so no shadows are cast
                    // Setting radius to 0 means calculate_point_shadow will return 1.0 (fully lit)
                    // immediately due to distance > shadow_radius check
                    PointShadowMatricesUniform {
                        face_matrices: [[[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]; 6],
                        light_pos_radius: [0.0, 0.0, 0.0, 0.0],  // radius = 0 disables shadows
                    }
                }
            } else {
                // No shadow lights resource - use dummy matrices with radius 0
                PointShadowMatricesUniform {
                    face_matrices: [[[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]; 6],
                    light_pos_radius: [0.0, 0.0, 0.0, 0.0],  // radius = 0 disables shadows
                }
            };
            
            let buffer = render_context.render_device().create_buffer_with_data(&BufferInitDescriptor {
                label: Some("point_shadow_matrices_uniform"),
                contents: bytemuck::bytes_of(&uniform),
                usage: BufferUsages::UNIFORM,
            });
            
            render_context.render_device().create_bind_group(
                Some("lighting_point_shadow_matrices_bind_group"),
                &lighting_pipeline.point_shadow_matrices_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            )
        };
        
        // Create bind group for GTAO texture (group 6)
        // Prefer denoised texture if available, otherwise fall back to raw GTAO
        let gtao_bind_group = if let Some(denoised) = gtao_denoised {
            // Use XeGTAO denoised output
            Some(render_context.render_device().create_bind_group(
                "lighting_gtao_denoised_bind_group",
                &lighting_pipeline.gtao_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&denoised.texture.default_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&lighting_pipeline.gtao_sampler),
                    },
                ],
            ))
        } else if let Some(gtao) = gtao_texture {
            // Fall back to raw GTAO (shouldn't happen if pipeline is set up correctly)
            Some(render_context.render_device().create_bind_group(
                "lighting_gtao_raw_bind_group",
                &lighting_pipeline.gtao_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&gtao.texture.default_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&lighting_pipeline.gtao_sampler),
                    },
                ],
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
        if let Some(ref point_shadow_bg) = point_shadow_bind_group {
            render_pass.set_bind_group(4, point_shadow_bg, &[]);
        }
        render_pass.set_bind_group(5, &point_shadow_matrices_bind_group, &[]);
        if let Some(ref gtao_bg) = gtao_bind_group {
            render_pass.set_bind_group(6, gtao_bg, &[]);
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
    /// Dual directional shadow maps layout (group 1) - moon1 + moon2 depth textures
    pub directional_shadow_layout: BindGroupLayout,
    /// Shadow comparison sampler (shared by directional and point shadows)
    pub shadow_sampler: Sampler,
    /// Directional shadow uniforms layout (group 2) - moon matrices, colors, softness
    pub directional_shadow_uniforms_layout: BindGroupLayout,
    /// Point lights uniform layout (group 3)
    pub point_lights_layout: BindGroupLayout,
    /// Point light shadow maps layout (group 4) - 6 depth textures for first shadow light
    pub point_shadow_layout: BindGroupLayout,
    /// Point shadow comparison sampler
    pub point_shadow_sampler: Sampler,
    /// Point shadow matrices layout (group 5) - view-proj matrices for cube faces
    pub point_shadow_matrices_layout: BindGroupLayout,
    /// GTAO texture layout (group 6) - screen-space ambient occlusion
    pub gtao_layout: BindGroupLayout,
    /// GTAO sampler
    pub gtao_sampler: Sampler,
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
    
    // Create bind group layout for dual directional shadow maps (group 1)
    // Contains moon1 + moon2 shadow textures + shared comparison sampler
    let directional_shadow_layout = render_device.create_bind_group_layout(
        "lighting_directional_shadow_layout",
        &[
            // Moon 1 shadow depth texture
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
            // Moon 2 shadow depth texture
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Shadow comparison sampler (shared)
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Comparison),
                count: None,
            },
        ],
    );
    
    // Create bind group layout for directional shadow uniforms (group 2)
    // Contains both moon matrices, colors, intensities, and shadow softness
    let directional_shadow_uniforms_layout = render_device.create_bind_group_layout(
        "lighting_directional_shadow_uniforms_layout",
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
    
    // Create bind group layout for point light shadows (group 4)
    // 6 depth textures (one per cube face) + comparison sampler
    let point_shadow_layout = render_device.create_bind_group_layout(
        "lighting_point_shadow_layout",
        &[
            // Face +X
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
            // Face -X
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Face +Y
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Face -Y
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Face +Z
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Face -Z
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Comparison sampler
            BindGroupLayoutEntry {
                binding: 6,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Comparison),
                count: None,
            },
        ],
    );
    
    // Create bind group layout for point shadow matrices (group 5)
    // Contains the 6 view-proj matrices for cube face sampling
    let point_shadow_matrices_layout = render_device.create_bind_group_layout(
        "lighting_point_shadow_matrices_layout",
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
    
    // Create bind group layout for GTAO texture (group 6)
    let gtao_layout = render_device.create_bind_group_layout(
        "lighting_gtao_layout",
        &[
            // GTAO texture (R8Unorm)
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
            // GTAO sampler
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
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
    
    // Create point shadow comparison sampler
    // LessEqual: returns 1.0 (lit) when compare_depth <= shadow_depth
    // Logic: if our fragment distance is <= closest blocker distance, we're lit
    // If our distance > blocker distance, something is closer → we're shadowed
    let point_shadow_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("point_shadow_comparison_sampler"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        compare: Some(CompareFunction::LessEqual),
        ..default()
    });
    
    // Create GTAO sampler (linear filtering for smooth AO)
    let gtao_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("gtao_sampler"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });

    // Load shader via asset server
    let shader = asset_server.load("shaders/deferred_lighting.wgsl");

    // Queue pipeline creation with all bind group layouts
    // Use new dual shadow layouts (group 1 & 2) to match updated shader
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("deferred_lighting_pipeline".into()),
        layout: vec![
            gbuffer_layout.clone(),                      // Group 0: G-buffer
            directional_shadow_layout.clone(),           // Group 1: Dual shadow maps (moon1, moon2, sampler)
            directional_shadow_uniforms_layout.clone(),  // Group 2: Shadow uniforms
            point_lights_layout.clone(),                 // Group 3: Point lights
            point_shadow_layout.clone(),                 // Group 4: Point shadow faces
            point_shadow_matrices_layout.clone(),        // Group 5: Point shadow matrices
            gtao_layout.clone(),                         // Group 6: GTAO texture
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
        directional_shadow_layout,
        shadow_sampler,
        directional_shadow_uniforms_layout,
        point_lights_layout,
        point_shadow_layout,
        point_shadow_sampler,
        point_shadow_matrices_layout,
        gtao_layout,
        gtao_sampler,
    });
}
