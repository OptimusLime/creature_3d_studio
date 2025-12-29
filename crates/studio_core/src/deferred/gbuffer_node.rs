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
    render_resource::{LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp},
    renderer::RenderContext,
    view::ViewTarget,
};

use super::gbuffer::ViewGBufferTextures;

/// Render graph node that renders geometry to G-buffer textures.
///
/// This node creates a render pass with 3 color attachments (MRT):
/// 1. gColor - albedo + emission
/// 2. gNormal - world-space normal
/// 3. gPosition - world position + depth
///
/// For now, this is a simple pass that clears the G-buffer.
/// We'll add actual geometry rendering when we have a custom draw function.
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
        _world: &'w World,
    ) -> Result<(), NodeRunError> {
        // Create MRT color attachments
        let color_attachments = [
            // gColor: RGB = albedo, A = emission
            Some(RenderPassColorAttachment {
                view: &gbuffer.color.default_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0, // No emission
                    }),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            }),
            // gNormal: RGB = world normal (0.5, 0.5, 1.0 = up)
            Some(RenderPassColorAttachment {
                view: &gbuffer.normal.default_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color {
                        r: 0.5, // Neutral normal X
                        g: 0.5, // Neutral normal Y
                        b: 1.0, // Normal Z (up)
                        a: 1.0,
                    }),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            }),
            // gPosition: XYZ = world pos, W = depth
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

        // Begin render pass - no depth attachment since we store depth in gPosition.w
        // When we add geometry rendering, we'll create our own depth texture with sample_count=1
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("gbuffer_pass"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Set viewport if camera has one
        if let Some(viewport) = &camera.viewport {
            render_pass.set_camera_viewport(viewport);
        }

        // TODO: Render geometry here
        // For now we just clear - geometry rendering will come when we add
        // a custom material/draw function that outputs to MRT

        // The render pass automatically ends when dropped
        drop(render_pass);

        Ok(())
    }
}
