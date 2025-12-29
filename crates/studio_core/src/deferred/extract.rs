//! Extraction systems for deferred rendering.
//!
//! This module extracts mesh data from the main world to the render world
//! for our custom G-buffer pipeline.

use bevy::prelude::*;
use bevy::render::{
    extract_component::ExtractComponent,
    render_resource::{BindGroupEntry, BufferInitDescriptor, BufferUsages},
    renderer::RenderDevice,
    sync_world::RenderEntity,
    Extract,
};
use bytemuck;

use crate::voxel_mesh::VoxelMaterial;
use super::gbuffer_geometry::{GBufferGeometryPipeline, GBufferMeshDrawData, GBufferMeshUniform};

/// Marker component for entities that should be rendered through the deferred pipeline.
///
/// Add this component to entities with `Mesh3d` to render them in the G-buffer pass.
///
/// # Example
///
/// ```rust,ignore
/// commands.spawn((
///     Mesh3d(mesh_handle),
///     MeshMaterial3d(material_handle),
///     Transform::from_xyz(0.0, 0.0, 0.0),
///     DeferredRenderable,
/// ));
/// ```
#[derive(Component, Default, Clone, ExtractComponent)]
pub struct DeferredRenderable;

/// Extracted mesh data for deferred rendering in the render world.
///
/// This contains all the information needed to render a mesh in the G-buffer pass.
#[derive(Component)]
pub struct ExtractedDeferredMesh {
    /// Handle to the mesh asset
    pub mesh: AssetId<Mesh>,
    /// World transform matrix
    pub transform: Mat4,
    /// Inverse transpose of transform for normals
    pub inverse_transpose: Mat4,
}

/// System to extract deferred renderable meshes to the render world.
///
/// This runs during the Extract phase and copies mesh data from main world
/// entities to render world entities.
pub fn extract_deferred_meshes(
    mut commands: Commands,
    meshes_query: Extract<
        Query<
            (RenderEntity, &GlobalTransform, &Mesh3d, &ViewVisibility),
            (With<DeferredRenderable>, With<MeshMaterial3d<VoxelMaterial>>),
        >,
    >,
) {
    for (render_entity, transform, mesh, visibility) in meshes_query.iter() {
        // Skip invisible meshes
        if !visibility.get() {
            continue;
        }

        let world_transform = transform.to_matrix();
        let inverse_transpose = world_transform.inverse().transpose();

        commands.entity(render_entity).insert(ExtractedDeferredMesh {
            mesh: mesh.0.id(),
            transform: world_transform,
            inverse_transpose,
        });
    }
}

/// System to prepare mesh draw data for the G-buffer pass.
///
/// This runs during the Prepare phase and collects all extracted meshes
/// into the GBufferGeometryPipeline resource for rendering.
/// It also creates per-mesh bind groups with transform uniforms.
pub fn prepare_deferred_meshes(
    render_device: Res<RenderDevice>,
    extracted_meshes: Query<&ExtractedDeferredMesh>,
    mut pipeline: ResMut<GBufferGeometryPipeline>,
) {
    // Clear previous frame's meshes
    pipeline.meshes_to_render.clear();

    // Collect all extracted meshes and create bind groups
    for mesh in extracted_meshes.iter() {
        // Create uniform data for this mesh
        let mesh_uniform = GBufferMeshUniform {
            world_from_local: mesh.transform.to_cols_array_2d(),
            local_from_world: mesh.transform.inverse().to_cols_array_2d(),
        };

        // Create buffer for this mesh's uniform
        let uniform_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("gbuffer_per_mesh_uniform"),
            contents: bytemuck::bytes_of(&mesh_uniform),
            usage: BufferUsages::UNIFORM,
        });

        // Create bind group for this mesh
        let bind_group = render_device.create_bind_group(
            Some("gbuffer_per_mesh_bind_group"),
            &pipeline.mesh_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        );

        pipeline.meshes_to_render.push(GBufferMeshDrawData {
            mesh_asset_id: mesh.mesh,
            transform: mesh.transform,
            inverse_transpose: mesh.inverse_transpose,
            bind_group: Some(bind_group),
        });
    }

    if !pipeline.meshes_to_render.is_empty() {
        debug!(
            "Prepared {} meshes for G-buffer rendering",
            pipeline.meshes_to_render.len()
        );
    }
}
