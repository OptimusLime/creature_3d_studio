//! Shadow pass render graph node.
//!
//! This node renders the scene from the light's perspective to create
//! a shadow depth map used in the lighting pass.

use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    mesh::allocator::MeshAllocator,
    mesh::RenderMesh,
    render_asset::RenderAssets,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroup, BindGroupEntry, Buffer, BufferInitDescriptor, BufferUsages,
        IndexFormat, LoadOp, Operations, PipelineCache,
        RenderPassDepthStencilAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::{RenderContext, RenderDevice},
    view::ViewTarget,
};

use super::gbuffer_geometry::{GBufferGeometryPipeline, GBufferMeshUniform};
use super::shadow::{ShadowConfig, ShadowPipeline, ShadowViewUniform, ViewShadowTextures};

/// Per-view shadow uniforms (light-space matrices).
#[derive(Component)]
pub struct ViewShadowUniforms {
    #[allow(dead_code)]
    pub buffer: Buffer,
    pub bind_group: BindGroup,
    /// Cached light-space view-projection for use in lighting pass.
    pub light_view_proj: Mat4,
}

/// Pre-built shadow mesh bind groups, prepared during the Prepare phase.
/// This avoids lifetime issues with creating bind groups during the render pass.
#[derive(Resource, Default)]
pub struct ShadowMeshBindGroups {
    /// Bind groups for each mesh, keyed by index matching meshes_to_render order.
    pub bind_groups: Vec<BindGroup>,
    /// Fallback bind group for test cube (identity transform).
    pub fallback: Option<BindGroup>,
}

/// Render graph node that renders the scene to a shadow depth map.
#[derive(Default)]
pub struct ShadowPassNode;

impl ViewNode for ShadowPassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        &'static ViewShadowTextures,
        &'static ViewShadowUniforms,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (_camera, _target, shadow_textures, shadow_uniforms): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // Get shadow pipeline
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(shadow_pipeline) = world.get_resource::<ShadowPipeline>() else {
            return Ok(());
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(shadow_pipeline.pipeline_id) else {
            return Ok(());
        };
        
        // Get geometry pipeline for mesh data
        let Some(geometry_pipeline) = world.get_resource::<GBufferGeometryPipeline>() else {
            return Ok(());
        };
        
        // Get pre-built shadow mesh bind groups
        let Some(shadow_bind_groups) = world.get_resource::<ShadowMeshBindGroups>() else {
            return Ok(());
        };
        
        // Begin shadow depth pass (depth-only, no color attachments)
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("shadow_pass"),
            color_attachments: &[],  // No color output
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &shadow_textures.depth.default_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),  // Clear to far (standard depth, not reverse-Z)
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        
        // Set pipeline and view bind group
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &shadow_uniforms.bind_group, &[]);
        
        // Get mesh resources
        let mesh_allocator = world.resource::<MeshAllocator>();
        let render_meshes = world.resource::<RenderAssets<RenderMesh>>();
        
        // Render all extracted meshes
        let mesh_count = geometry_pipeline.meshes_to_render.len();
        let bind_group_count = shadow_bind_groups.bind_groups.len();
        
        if mesh_count > 0 && bind_group_count == mesh_count {
            for (idx, mesh_data) in geometry_pipeline.meshes_to_render.iter().enumerate() {
                // Look up GPU mesh data
                let Some(gpu_mesh) = render_meshes.get(mesh_data.mesh_asset_id) else {
                    continue;
                };
                
                // Get vertex buffer from allocator
                let Some(vertex_slice) = mesh_allocator.mesh_vertex_slice(&mesh_data.mesh_asset_id) else {
                    continue;
                };
                
                // Use pre-built bind group
                render_pass.set_bind_group(1, &shadow_bind_groups.bind_groups[idx], &[]);
                render_pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));
                
                // Draw based on indexed vs non-indexed
                match &gpu_mesh.buffer_info {
                    bevy::render::mesh::RenderMeshBufferInfo::Indexed { count, index_format } => {
                        let Some(index_slice) = mesh_allocator.mesh_index_slice(&mesh_data.mesh_asset_id) else {
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
        } else if let Some(fallback_bind_group) = &shadow_bind_groups.fallback {
            // Fallback: render test cube
            render_pass.set_bind_group(1, fallback_bind_group, &[]);
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

/// System to prepare shadow view uniforms for each deferred camera.
pub fn prepare_shadow_view_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    shadow_config: Option<Res<ShadowConfig>>,
    shadow_pipeline: Option<Res<ShadowPipeline>>,
    cameras: Query<Entity, With<super::DeferredCamera>>,
) {
    let Some(shadow_config) = shadow_config else {
        return;
    };
    let Some(shadow_pipeline) = shadow_pipeline else {
        return;
    };
    
    // Calculate scene center (for now, use origin - could be based on camera target)
    let scene_center = Vec3::ZERO;
    
    // Calculate light-space view-projection
    let light_view_proj = shadow_config.light_view_projection(scene_center);
    
    let shadow_uniform = ShadowViewUniform {
        light_view_proj: light_view_proj.to_cols_array_2d(),
    };
    
    for entity in cameras.iter() {
        // Create uniform buffer
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("shadow_view_uniform"),
            contents: bytemuck::bytes_of(&shadow_uniform),
            usage: BufferUsages::UNIFORM,
        });
        
        // Create bind group
        let bind_group = render_device.create_bind_group(
            Some("shadow_view_bind_group"),
            &shadow_pipeline.view_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        );
        
        commands.entity(entity).insert(ViewShadowUniforms {
            buffer,
            bind_group,
            light_view_proj,
        });
    }
}

/// System to prepare shadow mesh bind groups during the Prepare phase.
/// This creates bind groups ahead of time to avoid lifetime issues during rendering.
pub fn prepare_shadow_mesh_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    shadow_pipeline: Option<Res<ShadowPipeline>>,
    geometry_pipeline: Option<Res<GBufferGeometryPipeline>>,
) {
    let Some(shadow_pipeline) = shadow_pipeline else {
        return;
    };
    let Some(geometry_pipeline) = geometry_pipeline else {
        return;
    };
    
    let mut bind_groups = Vec::with_capacity(geometry_pipeline.meshes_to_render.len());
    
    // Create bind groups for each mesh
    for mesh_data in &geometry_pipeline.meshes_to_render {
        let mesh_uniform = GBufferMeshUniform {
            world_from_local: mesh_data.transform.to_cols_array_2d(),
            local_from_world: mesh_data.transform.inverse().to_cols_array_2d(),
        };
        
        let mesh_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("shadow_mesh_uniform"),
            contents: bytemuck::bytes_of(&mesh_uniform),
            usage: BufferUsages::UNIFORM,
        });
        
        let bind_group = render_device.create_bind_group(
            Some("shadow_mesh_bind_group"),
            &shadow_pipeline.mesh_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: mesh_buffer.as_entire_binding(),
            }],
        );
        
        bind_groups.push(bind_group);
    }
    
    // Create fallback bind group for test cube (identity transform)
    let fallback_uniform = GBufferMeshUniform {
        world_from_local: Mat4::IDENTITY.to_cols_array_2d(),
        local_from_world: Mat4::IDENTITY.to_cols_array_2d(),
    };
    
    let fallback_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("shadow_fallback_mesh_uniform"),
        contents: bytemuck::bytes_of(&fallback_uniform),
        usage: BufferUsages::UNIFORM,
    });
    
    let fallback = render_device.create_bind_group(
        Some("shadow_fallback_mesh_bind_group"),
        &shadow_pipeline.mesh_layout,
        &[BindGroupEntry {
            binding: 0,
            resource: fallback_buffer.as_entire_binding(),
        }],
    );
    
    commands.insert_resource(ShadowMeshBindGroups {
        bind_groups,
        fallback: Some(fallback),
    });
}
