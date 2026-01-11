//! Shadow pass render graph nodes.
//!
//! Renders the scene from each moon's perspective to create shadow depth maps.
//! Used by the lighting pass to determine shadow visibility.
//!
//! Supports dual directional shadows (two moons) for dark world lighting.

use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    mesh::allocator::MeshAllocator,
    mesh::RenderMesh,
    render_asset::RenderAssets,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroup, BindGroupEntry, Buffer, BufferInitDescriptor, BufferUsages, IndexFormat, LoadOp,
        Operations, PipelineCache, RenderPassDepthStencilAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::{RenderContext, RenderDevice},
    view::ViewTarget,
};

use super::gbuffer_geometry::{GBufferGeometryPipeline, GBufferMeshUniform};
use super::shadow::{
    DirectionalShadowUniforms, MoonConfig, ShadowPipeline, ShadowViewUniform,
    ViewDirectionalShadowTextures,
};

/// Per-view uniforms for dual moon shadow system.
#[derive(Component)]
pub struct ViewDirectionalShadowUniforms {
    /// GPU buffer containing DirectionalShadowUniforms.
    pub buffer: Buffer,
    /// Bind group for moon 1 shadow pass.
    pub moon1_bind_group: BindGroup,
    /// Bind group for moon 2 shadow pass.
    pub moon2_bind_group: BindGroup,
    /// Bind group for lighting pass (contains full uniform data).
    pub lighting_bind_group: BindGroup,
    /// Cached uniforms for reference.
    pub uniforms: DirectionalShadowUniforms,
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

/// Render graph node for Moon 1 (purple) shadow pass.
#[derive(Default)]
pub struct Moon1ShadowPassNode;

impl ViewNode for Moon1ShadowPassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        &'static ViewDirectionalShadowTextures,
        &'static ViewDirectionalShadowUniforms,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (_camera, _target, shadow_textures, shadow_uniforms): bevy::ecs::query::QueryItem<
            'w,
            '_,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        render_directional_shadow_pass(
            render_context,
            world,
            &shadow_textures.moon1.default_view,
            &shadow_uniforms.moon1_bind_group,
            "moon1_shadow_pass",
        )
    }
}

/// Render graph node for Moon 2 (orange) shadow pass.
#[derive(Default)]
pub struct Moon2ShadowPassNode;

impl ViewNode for Moon2ShadowPassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        &'static ViewDirectionalShadowTextures,
        &'static ViewDirectionalShadowUniforms,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (_camera, _target, shadow_textures, shadow_uniforms): bevy::ecs::query::QueryItem<
            'w,
            '_,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        render_directional_shadow_pass(
            render_context,
            world,
            &shadow_textures.moon2.default_view,
            &shadow_uniforms.moon2_bind_group,
            "moon2_shadow_pass",
        )
    }
}

/// Shared implementation for rendering a directional shadow pass.
fn render_directional_shadow_pass<'w>(
    render_context: &mut RenderContext<'w>,
    world: &'w World,
    depth_view: &bevy::render::render_resource::TextureView,
    view_bind_group: &BindGroup,
    pass_label: &'static str,
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

    // Begin shadow depth pass
    let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some(pass_label),
        color_attachments: &[],
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(Operations {
                load: LoadOp::Clear(1.0),
                store: StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    });

    render_pass.set_render_pipeline(pipeline);
    render_pass.set_bind_group(0, view_bind_group, &[]);

    // Get mesh resources
    let mesh_allocator = world.resource::<MeshAllocator>();
    let render_meshes = world.resource::<RenderAssets<RenderMesh>>();

    // Render all extracted meshes
    let mesh_count = geometry_pipeline.meshes_to_render.len();
    let bind_group_count = shadow_bind_groups.bind_groups.len();

    if mesh_count > 0 && bind_group_count == mesh_count {
        for (idx, mesh_data) in geometry_pipeline.meshes_to_render.iter().enumerate() {
            let Some(gpu_mesh) = render_meshes.get(mesh_data.mesh_asset_id) else {
                continue;
            };

            let Some(vertex_slice) = mesh_allocator.mesh_vertex_slice(&mesh_data.mesh_asset_id)
            else {
                continue;
            };

            render_pass.set_bind_group(1, &shadow_bind_groups.bind_groups[idx], &[]);
            render_pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));

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

                    render_pass.set_index_buffer(index_slice.buffer.slice(..), 0, *index_format);

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

/// System to prepare dual moon shadow uniforms for each deferred camera.
pub fn prepare_directional_shadow_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    moon_config: Option<Res<MoonConfig>>,
    shadow_pipeline: Option<Res<ShadowPipeline>>,
    debug_modes: Option<Res<crate::debug_screenshot::DebugModes>>,
    lighting_config: Option<Res<super::lighting::DeferredLightingConfig>>,
    cameras: Query<Entity, With<super::DeferredCamera>>,
) {
    let Some(moon_config) = moon_config else {
        return;
    };
    let Some(shadow_pipeline) = shadow_pipeline else {
        return;
    };

    let scene_center = Vec3::ZERO;

    // Get lighting debug mode from DebugModes resource (extracted from main world)
    let lighting_debug_mode = debug_modes.map(|dm| dm.lighting_debug_mode).unwrap_or(0);

    // Get height fog params from lighting config (or use defaults)
    let (height_fog_density, height_fog_base, height_fog_falloff) =
        if let Some(ref config) = lighting_config {
            (
                config.height_fog_density,
                config.height_fog_base,
                config.height_fog_falloff,
            )
        } else {
            (0.03, 0.0, 0.08) // Defaults
        };

    // Create full uniforms for lighting pass (includes debug mode in shadow_softness.z)
    let uniforms = DirectionalShadowUniforms::from_config(
        &moon_config,
        scene_center,
        lighting_debug_mode,
        height_fog_density,
        height_fog_base,
        height_fog_falloff,
    );

    // Create individual view uniforms for each moon's shadow pass
    let moon1_uniform = ShadowViewUniform {
        light_view_proj: uniforms.moon1_view_proj,
    };
    let moon2_uniform = ShadowViewUniform {
        light_view_proj: uniforms.moon2_view_proj,
    };

    for entity in cameras.iter() {
        // Buffer for full uniforms (used by lighting pass)
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("directional_shadow_uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: BufferUsages::UNIFORM,
        });

        // Buffer for moon 1 shadow pass
        let moon1_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("moon1_shadow_view_uniform"),
            contents: bytemuck::bytes_of(&moon1_uniform),
            usage: BufferUsages::UNIFORM,
        });

        // Buffer for moon 2 shadow pass
        let moon2_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("moon2_shadow_view_uniform"),
            contents: bytemuck::bytes_of(&moon2_uniform),
            usage: BufferUsages::UNIFORM,
        });

        // Bind groups for shadow passes (use view_layout from shadow pipeline)
        let moon1_bind_group = render_device.create_bind_group(
            Some("moon1_shadow_view_bind_group"),
            &shadow_pipeline.view_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: moon1_buffer.as_entire_binding(),
            }],
        );

        let moon2_bind_group = render_device.create_bind_group(
            Some("moon2_shadow_view_bind_group"),
            &shadow_pipeline.view_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: moon2_buffer.as_entire_binding(),
            }],
        );

        // Bind group for lighting pass (full uniforms) - uses same layout for now
        // TODO: Create dedicated layout for DirectionalShadowUniforms
        let lighting_bind_group = render_device.create_bind_group(
            Some("directional_shadow_lighting_bind_group"),
            &shadow_pipeline.view_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        );

        commands
            .entity(entity)
            .insert(ViewDirectionalShadowUniforms {
                buffer,
                moon1_bind_group,
                moon2_bind_group,
                lighting_bind_group,
                uniforms,
            });
    }
}
