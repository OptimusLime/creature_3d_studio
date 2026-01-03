//! G-Buffer render graph node.
//!
//! This node renders geometry to the G-buffer textures using MRT (Multiple Render Targets).
//! It outputs:
//! - gColor: RGB = albedo, A = emission
//! - gNormal: RGB = world-space normal
//! - gPosition: XYZ = world position, W = linear depth

use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    mesh::allocator::MeshAllocator,
    mesh::RenderMesh,
    render_asset::RenderAssets,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        IndexFormat, LoadOp, Operations, PipelineCache, RenderPassColorAttachment,
        RenderPassDepthStencilAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::RenderContext,
    view::ViewTarget,
};

use super::gbuffer::ViewGBufferTextures;
use super::gbuffer_geometry::GBufferGeometryPipeline;
use super::prepare::ViewGBufferUniforms;

/// Render graph node that renders geometry to G-buffer textures.
///
/// This node creates a render pass with 3 color attachments (MRT) and depth:
/// 1. gColor - albedo + emission
/// 2. gNormal - world-space normal
/// 3. gPosition - world position + depth
#[derive(Default)]
pub struct GBufferPassNode;

impl ViewNode for GBufferPassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        &'static ViewGBufferTextures,
        &'static ViewGBufferUniforms,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, _target, gbuffer, view_uniforms): bevy::ecs::query::QueryItem<
            'w,
            '_,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // Get the geometry pipeline
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(geometry_pipeline) = world.get_resource::<GBufferGeometryPipeline>() else {
            bevy::log::warn!("GBufferPassNode: No GBufferGeometryPipeline resource");
            return Ok(());
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(geometry_pipeline.pipeline_id)
        else {
            bevy::log::warn!("GBufferPassNode: Pipeline not ready");
            return Ok(());
        };

        // Use per-view bind group from the camera entity (extracted from actual camera transform)
        let view_bind_group = &view_uniforms.bind_group;

        // Mesh bind group for fallback test cube
        let Some(mesh_bind_group) = &geometry_pipeline.mesh_bind_group else {
            return Ok(());
        };

        // Create MRT color attachments - clear to background values
        let color_attachments = [
            // gColor: RGB = albedo, A = emission (clear to black/no emission)
            Some(RenderPassColorAttachment {
                view: &gbuffer.color.default_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color::BLACK),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            }),
            // gNormal: RGB = world normal (clear to up vector)
            Some(RenderPassColorAttachment {
                view: &gbuffer.normal.default_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 1.0, // Up
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            }),
            // gPosition: XYZ = world pos, W = depth (clear to far depth)
            Some(RenderPassColorAttachment {
                view: &gbuffer.position.default_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1000.0, // Far depth
                    }),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            }),
        ];

        // Depth attachment
        let depth_attachment = RenderPassDepthStencilAttachment {
            view: &gbuffer.depth.default_view,
            depth_ops: Some(Operations {
                load: LoadOp::Clear(0.0), // Reverse-Z: 0 is far
                store: StoreOp::Store,
            }),
            stencil_ops: None,
        };

        // Begin render pass with MRT + depth
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("gbuffer_pass"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(depth_attachment),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Set viewport if camera has one
        if let Some(viewport) = &camera.viewport {
            render_pass.set_camera_viewport(viewport);
        }

        // Set pipeline and view bind group (same for all meshes)
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, view_bind_group, &[]);

        // Get mesh allocator and render meshes for accessing GPU buffers
        let mesh_allocator = world.resource::<MeshAllocator>();
        let render_meshes = world.resource::<RenderAssets<RenderMesh>>();

        // Render extracted meshes if we have any
        if !geometry_pipeline.meshes_to_render.is_empty() {
            for mesh_data in &geometry_pipeline.meshes_to_render {
                // Get the pre-built bind group for this mesh's transform
                let Some(mesh_bind_group) = &mesh_data.bind_group else {
                    continue;
                };

                // Look up the mesh's GPU data
                let Some(gpu_mesh) = render_meshes.get(mesh_data.mesh_asset_id) else {
                    continue;
                };

                // Get vertex buffer from allocator
                let Some(vertex_slice) = mesh_allocator.mesh_vertex_slice(&mesh_data.mesh_asset_id)
                else {
                    continue;
                };

                // Set mesh-specific bind group and vertex buffer
                render_pass.set_bind_group(1, mesh_bind_group, &[]);
                render_pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));

                // Draw based on indexed vs non-indexed mesh
                match &gpu_mesh.buffer_info {
                    bevy::render::mesh::RenderMeshBufferInfo::Indexed {
                        count,
                        index_format,
                    } => {
                        let Some(index_slice) =
                            mesh_allocator.mesh_index_slice(&mesh_data.mesh_asset_id)
                        else {
                            continue;
                        };

                        render_pass.set_index_buffer(
                            index_slice.buffer.slice(..),
                            0,
                            *index_format,
                        );

                        render_pass.draw_indexed(
                            index_slice.range.start..(index_slice.range.start + count),
                            vertex_slice.range.start as i32,
                            0..1,
                        );
                    }
                    bevy::render::mesh::RenderMeshBufferInfo::NonIndexed => {
                        render_pass.draw(vertex_slice.range.clone(), 0..1);
                    }
                }
            }
        } else {
            // Fallback: render test cube if no extracted meshes
            render_pass.set_bind_group(1, mesh_bind_group, &[]);
            render_pass.set_vertex_buffer(0, geometry_pipeline.vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                geometry_pipeline.index_buffer.slice(..),
                0,
                IndexFormat::Uint32,
            );
            render_pass.draw_indexed(0..geometry_pipeline.index_count, 0, 0..1);
        }

        Ok(())
    }
}
