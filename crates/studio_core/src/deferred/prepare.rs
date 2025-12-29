//! Prepare phase systems for deferred rendering.
//!
//! These systems run in the Render schedule's Prepare phase to set up
//! GPU resources needed for the G-buffer and lighting passes.

use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    render_resource::{
        BindGroup, BindGroupEntry, Buffer, BufferInitDescriptor, BufferUsages, Extent3d,
    },
    renderer::RenderDevice,
    texture::TextureCache,
    view::ExtractedView,
};

use super::gbuffer::{DeferredCamera, ViewGBufferTextures};
use super::gbuffer_geometry::{GBufferGeometryPipeline, GBufferViewUniform};

/// Per-view uniform buffer and bind group for G-buffer rendering.
///
/// This component is attached to camera entities and contains the view
/// matrices extracted from the actual camera transform.
#[derive(Component)]
pub struct ViewGBufferUniforms {
    #[allow(dead_code)]
    pub buffer: Buffer, // Kept alive to back the bind group
    pub bind_group: BindGroup,
}

/// System to create/resize G-buffer textures for deferred cameras.
///
/// Runs in the Prepare phase of the Render schedule.
pub fn prepare_gbuffer_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    cameras: Query<(Entity, &ExtractedCamera), With<DeferredCamera>>,
    existing: Query<&ViewGBufferTextures>,
) {
    for (entity, camera) in cameras.iter() {
        let Some(physical_size) = camera.physical_viewport_size else {
            continue;
        };

        let size = Extent3d {
            width: physical_size.x,
            height: physical_size.y,
            depth_or_array_layers: 1,
        };

        // Check if we need to recreate (size changed or doesn't exist)
        let needs_create = match existing.get(entity) {
            Ok(existing_textures) => existing_textures.size != size,
            Err(_) => true,
        };

        if needs_create {
            let textures = ViewGBufferTextures::new(&render_device, &mut texture_cache, size);
            
            commands.entity(entity).insert(textures);
        }
    }
}

/// System to prepare view uniforms for each deferred camera.
///
/// This reads the actual camera transform from ExtractedView and creates
/// per-view uniform buffers with the correct view/projection matrices.
pub fn prepare_gbuffer_view_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline: Option<Res<GBufferGeometryPipeline>>,
    cameras: Query<(Entity, &ExtractedCamera, &ExtractedView), With<DeferredCamera>>,
) {
    let Some(pipeline) = pipeline else {
        return;
    };

    for (entity, camera, view) in cameras.iter() {
        let Some(viewport_size) = camera.physical_viewport_size else {
            continue;
        };

        // Get view matrix from extracted view (world_from_view gives us camera transform)
        let world_from_view = view.world_from_view.to_matrix();
        let view_from_world = world_from_view.inverse();
        
        // Get projection matrix
        let clip_from_view = view.clip_from_view;
        
        // Compute combined view-projection
        let clip_from_world = view.clip_from_world.unwrap_or(clip_from_view * view_from_world);
        
        // Extract camera world position from the transform
        let camera_position = view.world_from_view.translation();

        let view_uniform = GBufferViewUniform {
            view_proj: clip_from_world.to_cols_array_2d(),
            inverse_view_proj: clip_from_world.inverse().to_cols_array_2d(),
            view: view_from_world.to_cols_array_2d(),
            inverse_view: world_from_view.to_cols_array_2d(),
            projection: clip_from_view.to_cols_array_2d(),
            inverse_projection: clip_from_view.inverse().to_cols_array_2d(),
            world_position: camera_position.to_array(),
            _padding: 0.0,
            viewport: [0.0, 0.0, viewport_size.x as f32, viewport_size.y as f32],
        };

        // Create buffer with uniform data
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("gbuffer_view_uniform"),
            contents: bytemuck::bytes_of(&view_uniform),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Create bind group
        let bind_group = render_device.create_bind_group(
            Some("gbuffer_view_bind_group"),
            &pipeline.view_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        );

        commands.entity(entity).insert(ViewGBufferUniforms { buffer, bind_group });
    }
}
