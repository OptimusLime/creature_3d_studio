//! Point light shadow pass render graph node.
//!
//! This node renders the scene from each shadow-casting point light's perspective,
//! creating cube shadow maps that are sampled in the lighting pass.
//!
//! For each shadow-casting light, we render 6 passes (one per cube face).

use bevy::render::{
    camera::ExtractedCamera,
    mesh::allocator::MeshAllocator,
    mesh::RenderMesh,
    render_asset::RenderAssets,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        IndexFormat, LoadOp, Operations, PipelineCache, RenderPassDepthStencilAttachment,
        RenderPassDescriptor, StoreOp,
    },
    renderer::RenderContext,
    view::ViewTarget,
};
use bevy::{log::info_once, prelude::*};

use super::gbuffer_geometry::GBufferGeometryPipeline;
use super::point_light_shadow::{
    PointShadowBindGroups, PointShadowPipeline, ShadowCastingLights, ViewPointShadowTextures,
    POINT_SHADOW_MAP_SIZE,
};

/// Render graph node for point light shadow passes.
#[derive(Default)]
pub struct PointShadowPassNode;

impl ViewNode for PointShadowPassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        &'static ViewPointShadowTextures,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (_camera, _target, shadow_textures): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // Get required resources
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) = world.get_resource::<PointShadowPipeline>() else {
            return Ok(());
        };
        let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
            return Ok(());
        };
        let Some(geometry_pipeline) = world.get_resource::<GBufferGeometryPipeline>() else {
            return Ok(());
        };
        let Some(bind_groups) = world.get_resource::<PointShadowBindGroups>() else {
            return Ok(());
        };
        let Some(shadow_lights) = world.get_resource::<ShadowCastingLights>() else {
            return Ok(());
        };

        // Skip if no shadow-casting lights
        if shadow_lights.lights.is_empty() {
            return Ok(());
        }

        // Debug: Log that we're rendering shadows
        info_once!(
            "Point shadow pass: rendering {} lights",
            shadow_lights.lights.len()
        );

        // Get mesh resources
        let mesh_allocator = world.resource::<MeshAllocator>();
        let render_meshes = world.resource::<RenderAssets<RenderMesh>>();

        // Render each shadow-casting light
        for (light_idx, _light) in shadow_lights.lights.iter().enumerate() {
            // Render each cube face
            for face_idx in 0..6 {
                // Get the face view for this light and face
                let Some(face_view) = shadow_textures.get_face_view(light_idx, face_idx) else {
                    continue;
                };

                // Get the view bind group for this light/face
                let view_bind_group_idx = light_idx * 6 + face_idx;
                let Some(view_bind_group) = bind_groups.view_bind_groups.get(view_bind_group_idx)
                else {
                    continue;
                };

                // Begin shadow depth pass for this face
                // Clear to 1.0 (far plane) - geometry will write closer depths
                let clear_value = 1.0;
                let mut render_pass =
                    render_context.begin_tracked_render_pass(RenderPassDescriptor {
                        label: Some("point_shadow_pass"),
                        color_attachments: &[], // Depth-only
                        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                            view: face_view,
                            depth_ops: Some(Operations {
                                load: LoadOp::Clear(clear_value),
                                store: StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                // Set viewport to shadow map size
                render_pass.set_viewport(
                    0.0,
                    0.0,
                    POINT_SHADOW_MAP_SIZE as f32,
                    POINT_SHADOW_MAP_SIZE as f32,
                    0.0,
                    1.0,
                );

                // Set pipeline and view bind group
                render_pass.set_render_pipeline(render_pipeline);
                render_pass.set_bind_group(0, view_bind_group, &[]);

                // Render all meshes
                let mesh_count = geometry_pipeline.meshes_to_render.len();
                let bind_group_count = bind_groups.mesh_bind_groups.len();

                if mesh_count > 0 && bind_group_count == mesh_count {
                    for (idx, mesh_data) in geometry_pipeline.meshes_to_render.iter().enumerate() {
                        // Look up GPU mesh data
                        let Some(gpu_mesh) = render_meshes.get(mesh_data.mesh_asset_id) else {
                            continue;
                        };

                        // Get vertex buffer from allocator
                        let Some(vertex_slice) =
                            mesh_allocator.mesh_vertex_slice(&mesh_data.mesh_asset_id)
                        else {
                            continue;
                        };

                        // Use mesh bind group
                        render_pass.set_bind_group(1, &bind_groups.mesh_bind_groups[idx], &[]);
                        render_pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));

                        // Draw
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
                } else if let Some(fallback) = &bind_groups.fallback_mesh {
                    // Render test cube
                    render_pass.set_bind_group(1, fallback, &[]);
                    render_pass.set_vertex_buffer(0, geometry_pipeline.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        geometry_pipeline.index_buffer.slice(..),
                        0,
                        IndexFormat::Uint32,
                    );
                    render_pass.draw_indexed(0..geometry_pipeline.index_count, 0, 0..1);
                }
            }
        }

        Ok(())
    }
}
