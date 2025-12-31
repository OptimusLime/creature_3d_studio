//! Bloom render graph node.
//!
//! This node performs bloom post-processing after the lighting pass:
//! 1. Downsample the HDR scene through a mip chain (applying threshold on first pass)
//! 2. Upsample back up, blending mip levels
//! 3. Composite bloom onto the final output

use bevy::prelude::*;
use bevy::render::{
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroup, BindGroupEntry, BindingResource, LoadOp, Operations, PipelineCache,
        RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::RenderContext,
    view::ViewTarget,
};

use super::bloom::{BloomConfig, BloomPipeline, ViewBloomTextures, BLOOM_MIP_LEVELS};

/// Render graph node for bloom post-processing.
#[derive(Default)]
pub struct BloomNode;

impl ViewNode for BloomNode {
    type ViewQuery = (&'static ViewTarget, &'static ViewBloomTextures);

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view_target, bloom_textures): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(bloom_pipeline) = world.get_resource::<BloomPipeline>() else {
            return Ok(());
        };
        let bloom_config = world
            .get_resource::<BloomConfig>()
            .cloned()
            .unwrap_or_default();

        // Skip bloom if disabled
        if !bloom_config.enabled {
            return Ok(());
        }

        // Get pipelines
        let Some(downsample_pipeline) =
            pipeline_cache.get_render_pipeline(bloom_pipeline.downsample_pipeline_id)
        else {
            return Ok(());
        };
        let Some(upsample_pipeline) =
            pipeline_cache.get_render_pipeline(bloom_pipeline.upsample_pipeline_id)
        else {
            return Ok(());
        };
        let Some(composite_pipeline) =
            pipeline_cache.get_render_pipeline(bloom_pipeline.composite_pipeline_id)
        else {
            return Ok(());
        };

        // Source texture is the post-lighting HDR output
        let source_texture = view_target.main_texture_view();

        // === PRE-CREATE ALL BIND GROUPS ===
        // We need to create bind groups while holding an immutable borrow of device,
        // then release that borrow before starting render passes (which need mutable borrow).

        // Collect downsample bind groups and push constants
        // Downsample writes to mips_a
        let mut downsample_data: Vec<(BindGroup, [u32; 4])> = Vec::with_capacity(BLOOM_MIP_LEVELS);
        {
            let device = render_context.render_device();
            
            for i in 0..BLOOM_MIP_LEVELS {
                let input_view = if i == 0 {
                    source_texture
                } else {
                    &bloom_textures.mips_a[i - 1].default_view
                };

                // Calculate texel size for input texture
                let (input_width, input_height) = if i == 0 {
                    (
                        bloom_textures.size.width as f32,
                        bloom_textures.size.height as f32,
                    )
                } else {
                    let mip_size = bloom_textures.mips_a[i - 1].texture.size();
                    (mip_size.width as f32, mip_size.height as f32)
                };

                let bind_group = device.create_bind_group(
                    Some("bloom_downsample_bind_group"),
                    &bloom_pipeline.texture_layout,
                    &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(input_view),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::Sampler(&bloom_pipeline.sampler),
                        },
                    ],
                );

                // Push constants: texel_size (8 bytes) + threshold (4) + is_first_pass (4)
                let push_constants = [
                    (1.0 / input_width).to_bits(),
                    (1.0 / input_height).to_bits(),
                    bloom_config.threshold.to_bits(),
                    if i == 0 {
                        1.0f32.to_bits()
                    } else {
                        0.0f32.to_bits()
                    },
                ];

                downsample_data.push((bind_group, push_constants));
            }
        }

        // Collect upsample bind groups and push constants
        // Upsample reads from mips_a (downsample result) and writes to mips_b (to avoid read-write hazard)
        // For first upsample pass: read mips_a[LAST], blend with mips_a[LAST-1], write to mips_b[LAST-1]
        // For subsequent passes: read mips_b[i+1] (previous upsample result), blend with mips_a[i], write to mips_b[i]
        let mut upsample_data: Vec<(BindGroup, [u32; 4])> = Vec::with_capacity(BLOOM_MIP_LEVELS - 1);
        {
            let device = render_context.render_device();
            
            for (pass_idx, i) in (0..BLOOM_MIP_LEVELS - 1).rev().enumerate() {
                // Input: For first pass, read from mips_a (smallest mip from downsample)
                //        For subsequent passes, read from mips_b (previous upsample output)
                let input_view = if pass_idx == 0 {
                    &bloom_textures.mips_a[i + 1].default_view // Smallest mip from downsample
                } else {
                    &bloom_textures.mips_b[i + 1].default_view // Previous upsample result
                };
                
                // Blend with the original downsampled mip at this level
                let blend_view = &bloom_textures.mips_a[i].default_view;
                
                // Output goes to mips_b
                let output_size = bloom_textures.mips_b[i].texture.size();

                let bind_group = device.create_bind_group(
                    Some("bloom_upsample_bind_group"),
                    &bloom_pipeline.dual_texture_layout,
                    &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(input_view),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(blend_view),
                        },
                        BindGroupEntry {
                            binding: 2,
                            resource: BindingResource::Sampler(&bloom_pipeline.sampler),
                        },
                    ],
                );

                // Push constants: texel_size (8) + blend_factor (4) + padding (4)
                let push_constants = [
                    (1.0 / output_size.width as f32).to_bits(),
                    (1.0 / output_size.height as f32).to_bits(),
                    bloom_config.blend_factor.to_bits(),
                    0u32, // padding
                ];

                upsample_data.push((bind_group, push_constants));
            }
        }

        // Create composite bind group
        // Final bloom result is in mips_b[0] (the largest mip after upsampling)
        let post_process_write = view_target.post_process_write();
        let bloom_result = &bloom_textures.mips_b[0].default_view;
        
        let composite_bind_group;
        let composite_push_constants;
        {
            let device = render_context.render_device();
            
            composite_bind_group = device.create_bind_group(
                Some("bloom_composite_bind_group"),
                &bloom_pipeline.dual_texture_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(post_process_write.source),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(bloom_result),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Sampler(&bloom_pipeline.sampler),
                    },
                ],
            );

            // Push constants: bloom_intensity (4) + threshold (4) + exposure (4) + padding (4)
            composite_push_constants = [
                bloom_config.intensity.to_bits(),
                bloom_config.threshold.to_bits(),
                bloom_config.exposure.to_bits(),
                0u32,
            ];
        }

        // === DOWNSAMPLE PASSES ===
        // Downsample writes to mips_a
        for (i, (bind_group, push_constants)) in downsample_data.iter().enumerate() {
            let output_view = &bloom_textures.mips_a[i].default_view;

            let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("bloom_downsample"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::BLACK),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_render_pipeline(downsample_pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_push_constants(
                wgpu::ShaderStages::FRAGMENT,
                0,
                bytemuck::cast_slice(push_constants),
            );
            render_pass.draw(0..3, 0..1);
        }

        // === UPSAMPLE PASSES ===
        // Upsample writes to mips_b (to avoid read-write hazard with mips_a)
        for (idx, (bind_group, push_constants)) in upsample_data.iter().enumerate() {
            // upsample_data is in reverse order (BLOOM_MIP_LEVELS-2 down to 0)
            let i = BLOOM_MIP_LEVELS - 2 - idx;
            let output_view = &bloom_textures.mips_b[i].default_view;

            let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("bloom_upsample"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::BLACK), // Clear - shader does the blending
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_render_pipeline(upsample_pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_push_constants(
                wgpu::ShaderStages::FRAGMENT,
                0,
                bytemuck::cast_slice(push_constants),
            );
            render_pass.draw(0..3, 0..1);
        }

        // === COMPOSITE PASS ===
        {
            let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("bloom_composite"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: post_process_write.destination,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::BLACK),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_render_pipeline(composite_pipeline);
            render_pass.set_bind_group(0, &composite_bind_group, &[]);
            render_pass.set_push_constants(
                wgpu::ShaderStages::FRAGMENT,
                0,
                bytemuck::cast_slice(&composite_push_constants),
            );
            render_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}
