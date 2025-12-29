//! Prepare phase systems for deferred rendering.
//!
//! These systems run in the Render schedule's Prepare phase to set up
//! GPU resources needed for the G-buffer and lighting passes.

use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    render_resource::Extent3d,
    renderer::RenderDevice,
    texture::TextureCache,
};

use super::gbuffer::{DeferredCamera, ViewGBufferTextures};

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
            
            info!(
                "Created G-buffer textures for camera {:?} at {}x{}",
                entity, size.width, size.height
            );
            
            commands.entity(entity).insert(textures);
        }
    }
}
