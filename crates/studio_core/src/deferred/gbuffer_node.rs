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
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        IndexFormat, LoadOp, Operations, PipelineCache,
        RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
        StoreOp,
    },
    renderer::RenderContext,
    view::ViewTarget,
};

use super::gbuffer::ViewGBufferTextures;
use super::gbuffer_geometry::GBufferGeometryPipeline;

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
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, _target, gbuffer): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // Get the geometry pipeline
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(geometry_pipeline) = world.get_resource::<GBufferGeometryPipeline>() else {
            return Ok(());
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(geometry_pipeline.pipeline_id) else {
            return Ok(());
        };

        // Need bind groups
        let Some(view_bind_group) = &geometry_pipeline.view_bind_group else {
            return Ok(());
        };
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

        // Draw geometry
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, view_bind_group, &[]);
        render_pass.set_bind_group(1, mesh_bind_group, &[]);
        render_pass.set_vertex_buffer(0, geometry_pipeline.vertex_buffer.slice(..));
        render_pass.set_index_buffer(geometry_pipeline.index_buffer.slice(..), 0, IndexFormat::Uint32);
        render_pass.draw_indexed(0..geometry_pipeline.index_count, 0, 0..1);

        Ok(())
    }
}
