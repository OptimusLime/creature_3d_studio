//! GTAO Edge-Aware Denoiser render node.
//!
//! Implements XeGTAO's edge-aware spatial denoiser (XeGTAO.hlsli L686-826).
//! This replaces the previous 7x7 bilateral blur in deferred_lighting.wgsl.
//!
//! Key features:
//! - Uses packed edges (2 bits per direction = 4 gradient levels)
//! - 3x3 kernel with diagonal weighting
//! - Edge symmetry enforcement for sharper blur
//! - AO leaking prevention for edge cases
//!
//! Reference: https://github.com/GameTechDev/XeGTAO

use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
        BufferBindingType, BufferInitDescriptor, BufferUsages, CachedComputePipelineId,
        ComputePassDescriptor, ComputePipelineDescriptor, Extent3d, FilterMode, PipelineCache,
        Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, StorageTextureAccess,
        TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
        TextureViewDimension,
    },
    renderer::{RenderContext, RenderDevice},
    texture::{CachedTexture, TextureCache},
};

use super::gtao::{GtaoConfig, ViewGtaoTexture, ViewGtaoEdgesTexture};
use crate::debug_screenshot::DebugModes;

/// GPU uniform for denoise parameters.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DenoiseUniform {
    /// Viewport size (width, height)
    pub viewport_size: [f32; 2],
    /// Pixel size (1/width, 1/height)
    pub viewport_pixel_size: [f32; 2],
    /// XeGTAO default: 1.2
    pub denoise_blur_beta: f32,
    /// 1 if final pass, 0 otherwise
    pub is_final_pass: u32,
    /// Debug mode: 0=normal, 1=sum_weight, 2=edges_c, 3=blur_amount, 4=diff
    pub debug_mode: u32,
    /// Padding for alignment
    pub padding: f32,
}

/// Per-view denoised GTAO texture (output of denoiser).
/// Contains two textures for ping-pong multi-pass denoising.
#[derive(Component)]
pub struct ViewGtaoDenoised {
    /// Primary output texture (also used as ping-pong A)
    pub texture: CachedTexture,
    /// Secondary texture for ping-pong B (multi-pass denoising)
    pub texture_b: CachedTexture,
}

/// Render graph node for GTAO denoising.
#[derive(Default)]
pub struct GtaoDenoiseNode;

impl ViewNode for GtaoDenoiseNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewGtaoTexture,
        &'static ViewGtaoEdgesTexture,
        &'static ViewGtaoDenoised,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, gtao_texture, edges_texture, denoised_texture): bevy::ecs::query::QueryItem<
            'w,
            '_,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(denoise_pipeline) = world.get_resource::<GtaoDenoiseResources>() else {
            return Ok(());
        };

        let Some(pipeline) = pipeline_cache.get_compute_pipeline(denoise_pipeline.pipeline_id)
        else {
            return Ok(());
        };

        // Get GTAO config for denoise parameters
        let gtao_config = world
            .get_resource::<GtaoConfig>()
            .cloned()
            .unwrap_or_default();

        // Skip if GTAO is disabled
        if !gtao_config.enabled {
            return Ok(());
        }

        // Get debug modes for denoiser debug visualization
        let debug_modes = world
            .get_resource::<DebugModes>()
            .cloned()
            .unwrap_or_default();

        let full_screen_size = camera
            .physical_viewport_size
            .unwrap_or(UVec2::new(1920, 1080));

        // GTAO is at half resolution
        let half_width = (full_screen_size.x / 2).max(1);
        let half_height = (full_screen_size.y / 2).max(1);

        // Number of denoise passes from config
        let num_passes = gtao_config.denoise_passes().max(1);

        // Dispatch compute shader
        // Each workgroup is 8x8, and each thread processes 2 pixels horizontally
        // So effective coverage per workgroup is 16x8 pixels
        let workgroups_x = (half_width + 15) / 16;
        let workgroups_y = (half_height + 7) / 8;

        // Multi-pass denoising with ping-pong textures
        // Pass 0: raw GTAO -> texture A
        // Pass 1: texture A -> texture B
        // Pass 2: texture B -> texture A
        // Final output is always in denoised_texture.texture (A)
        for pass in 0..num_passes {
            let is_final_pass = pass == num_passes - 1;
            
            // Create uniform buffer for this pass
            let uniform = DenoiseUniform {
                viewport_size: [half_width as f32, half_height as f32],
                viewport_pixel_size: [1.0 / half_width as f32, 1.0 / half_height as f32],
                denoise_blur_beta: gtao_config.denoise_blur_beta(),
                is_final_pass: if is_final_pass { 1 } else { 0 },
                debug_mode: if is_final_pass { debug_modes.denoise_debug_mode } else { 0 },
                padding: 0.0,
            };

            let uniform_buffer =
                render_context
                    .render_device()
                    .create_buffer_with_data(&BufferInitDescriptor {
                        label: Some("gtao_denoise_uniform_buffer"),
                        contents: bytemuck::bytes_of(&uniform),
                        usage: BufferUsages::UNIFORM,
                    });

            // Determine input/output textures based on pass number
            // Pass 0: input=raw GTAO, output=texture A
            // Pass 1: input=texture A, output=texture B  
            // Pass 2: input=texture B, output=texture A
            // etc. (ping-pong)
            let (input_ao_view, output_view) = if pass == 0 {
                // First pass: read from raw GTAO
                (&gtao_texture.texture.default_view, &denoised_texture.texture.default_view)
            } else if pass % 2 == 1 {
                // Odd passes: read from A, write to B
                (&denoised_texture.texture.default_view, &denoised_texture.texture_b.default_view)
            } else {
                // Even passes (2, 4, ...): read from B, write to A
                (&denoised_texture.texture_b.default_view, &denoised_texture.texture.default_view)
            };

            // Create bind groups
            let input_bind_group = render_context.render_device().create_bind_group(
                "gtao_denoise_input_bind_group",
                &denoise_pipeline.input_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(input_ao_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&edges_texture.texture.default_view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Sampler(&denoise_pipeline.sampler),
                    },
                ],
            );

            let output_bind_group = render_context.render_device().create_bind_group(
                "gtao_denoise_output_bind_group",
                &denoise_pipeline.output_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(output_view),
                }],
            );

            let uniform_bind_group = render_context.render_device().create_bind_group(
                "gtao_denoise_uniform_bind_group",
                &denoise_pipeline.uniform_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            );

            {
                let mut compute_pass =
                    render_context
                        .command_encoder()
                        .begin_compute_pass(&ComputePassDescriptor {
                            label: Some(&format!("gtao_denoise_pass_{}", pass)),
                            timestamp_writes: None,
                        });

                compute_pass.set_pipeline(pipeline);
                compute_pass.set_bind_group(0, &input_bind_group, &[]);
                compute_pass.set_bind_group(1, &output_bind_group, &[]);
                compute_pass.set_bind_group(2, &uniform_bind_group, &[]);
                compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
            }
        }

        // If we ended on an odd number of passes (1, 3, ...), the final output is in texture A
        // If we ended on an even number of passes (2, 4, ...), the final output is in texture B
        // We need to copy texture B to texture A if needed
        if num_passes > 1 && num_passes % 2 == 0 {
            // Final result is in texture B, need to copy to A
            // For simplicity, we'll do an extra pass from B->A
            // TODO: Could optimize with a direct copy instead
            let uniform = DenoiseUniform {
                viewport_size: [half_width as f32, half_height as f32],
                viewport_pixel_size: [1.0 / half_width as f32, 1.0 / half_height as f32],
                denoise_blur_beta: gtao_config.denoise_blur_beta(),
                is_final_pass: 1,
                debug_mode: debug_modes.denoise_debug_mode,
                padding: 0.0,
            };

            let uniform_buffer =
                render_context
                    .render_device()
                    .create_buffer_with_data(&BufferInitDescriptor {
                        label: Some("gtao_denoise_uniform_buffer_final"),
                        contents: bytemuck::bytes_of(&uniform),
                        usage: BufferUsages::UNIFORM,
                    });

            let input_bind_group = render_context.render_device().create_bind_group(
                "gtao_denoise_input_bind_group_final",
                &denoise_pipeline.input_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&denoised_texture.texture_b.default_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&edges_texture.texture.default_view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Sampler(&denoise_pipeline.sampler),
                    },
                ],
            );

            let output_bind_group = render_context.render_device().create_bind_group(
                "gtao_denoise_output_bind_group_final",
                &denoise_pipeline.output_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&denoised_texture.texture.default_view),
                }],
            );

            let uniform_bind_group = render_context.render_device().create_bind_group(
                "gtao_denoise_uniform_bind_group_final",
                &denoise_pipeline.uniform_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            );

            {
                let mut compute_pass =
                    render_context
                        .command_encoder()
                        .begin_compute_pass(&ComputePassDescriptor {
                            label: Some("gtao_denoise_pass_final_copy"),
                            timestamp_writes: None,
                        });

                compute_pass.set_pipeline(pipeline);
                compute_pass.set_bind_group(0, &input_bind_group, &[]);
                compute_pass.set_bind_group(1, &output_bind_group, &[]);
                compute_pass.set_bind_group(2, &uniform_bind_group, &[]);
                compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
            }
        }

        Ok(())
    }
}

/// Pipeline resources for GTAO denoiser.
#[derive(Resource)]
pub struct GtaoDenoiseResources {
    pub pipeline_id: CachedComputePipelineId,
    /// Input textures layout (group 0)
    pub input_layout: BindGroupLayout,
    /// Output texture layout (group 1)
    pub output_layout: BindGroupLayout,
    /// Uniform layout (group 2)
    pub uniform_layout: BindGroupLayout,
    /// Sampler for input textures
    pub sampler: Sampler,
}

/// System to initialize the GTAO denoise pipeline.
pub fn init_gtao_denoise_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<GtaoDenoiseResources>>,
) {
    if existing.is_some() {
        return;
    }

    // Group 0: Input textures (AO, edges, sampler)
    let input_layout = render_device.create_bind_group_layout(
        "gtao_denoise_input_layout",
        &[
            // Input AO texture
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Input edges texture
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Sampler
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    );

    // Group 1: Output texture (storage image)
    let output_layout = render_device.create_bind_group_layout(
        "gtao_denoise_output_layout",
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::WriteOnly,
                format: TextureFormat::R8Unorm,
                view_dimension: TextureViewDimension::D2,
            },
            count: None,
        }],
    );

    // Group 2: Uniforms
    let uniform_layout = render_device.create_bind_group_layout(
        "gtao_denoise_uniform_layout",
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    );

    // Create sampler (linear filtering)
    let sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("gtao_denoise_sampler"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });

    // Load shader
    let shader = asset_server.load("shaders/gtao_denoise.wgsl");

    // Queue pipeline creation
    let pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("gtao_denoise_pipeline".into()),
        layout: vec![
            input_layout.clone(),
            output_layout.clone(),
            uniform_layout.clone(),
        ],
        push_constant_ranges: vec![],
        shader,
        shader_defs: vec![],
        entry_point: Some("main".into()),
        zero_initialize_workgroup_memory: false,
    });

    commands.insert_resource(GtaoDenoiseResources {
        pipeline_id,
        input_layout,
        output_layout,
        uniform_layout,
        sampler,
    });
}

/// System to prepare denoised GTAO textures for each view.
pub fn prepare_gtao_denoised_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    views: Query<(Entity, &ExtractedCamera), Without<ViewGtaoDenoised>>,
) {
    for (entity, camera) in views.iter() {
        let Some(size) = camera.physical_viewport_size else {
            continue;
        };

        // Half resolution to match GTAO
        let half_width = (size.x / 2).max(1);
        let half_height = (size.y / 2).max(1);

        let texture_descriptor = TextureDescriptor {
            label: Some("gtao_denoised_texture"),
            size: Extent3d {
                width: half_width,
                height: half_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            // Need STORAGE_BINDING for compute shader write
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        // Create primary denoised output texture (A)
        let denoised_texture = texture_cache.get(&render_device, texture_descriptor.clone());

        // Create secondary texture for ping-pong (B)
        let mut texture_b_descriptor = texture_descriptor;
        texture_b_descriptor.label = Some("gtao_denoised_texture_b");
        let denoised_texture_b = texture_cache.get(&render_device, texture_b_descriptor);

        commands.entity(entity).insert(ViewGtaoDenoised {
            texture: denoised_texture,
            texture_b: denoised_texture_b,
        });
    }
}
