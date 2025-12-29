//! Point light support for deferred rendering.
//!
//! This module provides point lights that illuminate nearby surfaces.
//! Point lights are added as Bevy components and extracted to the render world
//! where they're passed to the deferred lighting shader.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Spawn a point light
//! commands.spawn((
//!     DeferredPointLight {
//!         color: Color::srgb(1.0, 0.5, 0.2),
//!         intensity: 5.0,
//!         radius: 10.0,
//!     },
//!     Transform::from_xyz(5.0, 3.0, 5.0),
//! ));
//! ```

use bevy::prelude::*;
use bevy::render::{
    render_resource::{Buffer, BufferInitDescriptor, BufferUsages, ShaderType},
    renderer::RenderDevice,
    Extract,
};

/// Maximum number of point lights supported by the shader.
/// This must match MAX_POINT_LIGHTS in deferred_lighting.wgsl
/// Using storage buffer allows much higher counts than uniform buffers.
pub const MAX_POINT_LIGHTS: usize = 256;

/// Point light component for deferred rendering.
///
/// Add this component along with a `Transform` to create a point light.
/// The light will illuminate nearby surfaces based on distance.
#[derive(Component, Clone, Debug)]
pub struct DeferredPointLight {
    /// Light color (RGB).
    pub color: Color,
    /// Light intensity multiplier.
    pub intensity: f32,
    /// Maximum radius of effect. Light falls off to zero at this distance.
    pub radius: f32,
}

impl Default for DeferredPointLight {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            intensity: 1.0,
            radius: 10.0,
        }
    }
}

impl DeferredPointLight {
    /// Create a new point light with the given color.
    pub fn new(color: Color, intensity: f32, radius: f32) -> Self {
        Self {
            color,
            intensity,
            radius,
        }
    }
    
    /// Create a colored light with default intensity and radius.
    pub fn colored(color: Color) -> Self {
        Self {
            color,
            intensity: 1.0,
            radius: 10.0,
        }
    }
}

/// Extracted point light data in render world.
#[derive(Component, Clone)]
pub struct ExtractedPointLight {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
}

/// GPU-side point light data.
/// Must match the struct in deferred_lighting.wgsl
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable, ShaderType)]
pub struct GpuPointLight {
    /// World position of the light (xyz), w unused (padding)
    pub position: [f32; 4],
    /// Light color (rgb), w = intensity
    pub color_intensity: [f32; 4],
    /// x = radius, yzw = padding
    pub radius_padding: [f32; 4],
}

impl Default for GpuPointLight {
    fn default() -> Self {
        Self {
            position: [0.0; 4],
            color_intensity: [0.0; 4],
            radius_padding: [0.0; 4],
        }
    }
}

/// Storage buffer header containing point light count.
/// Stored separately for alignment and to allow dynamic light counts.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLightsHeader {
    /// Number of active point lights (x), padding (yzw)
    pub count: [u32; 4],
}

/// Storage buffer containing all point lights for the lighting pass.
/// Using storage buffer instead of uniform allows:
/// - Much larger light counts (256+ vs ~32 for uniform)
/// - Dynamic array access in shader
/// - Better performance for sparse light culling
#[repr(C)]
#[derive(Clone)]
pub struct PointLightsStorage {
    /// Header with light count
    pub header: PointLightsHeader,
    /// Array of point lights (only count lights are valid)
    pub lights: Vec<GpuPointLight>,
}

impl Default for PointLightsStorage {
    fn default() -> Self {
        Self {
            header: PointLightsHeader { count: [0; 4] },
            lights: Vec::new(),
        }
    }
}

impl PointLightsStorage {
    /// Convert to bytes for GPU upload.
    /// Layout: [header (16 bytes)] [lights (48 bytes each)]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16 + self.lights.len() * std::mem::size_of::<GpuPointLight>());
        bytes.extend_from_slice(bytemuck::bytes_of(&self.header));
        for light in &self.lights {
            bytes.extend_from_slice(bytemuck::bytes_of(light));
        }
        // Pad to at least 256 lights worth of space for consistent buffer size
        // This prevents buffer recreation on every frame when light count changes
        let min_lights = MAX_POINT_LIGHTS;
        let current_lights = self.lights.len();
        if current_lights < min_lights {
            let padding = (min_lights - current_lights) * std::mem::size_of::<GpuPointLight>();
            bytes.extend(std::iter::repeat(0u8).take(padding));
        }
        bytes
    }
}

/// Resource holding the point lights buffer for the current frame.
#[derive(Resource)]
pub struct PointLightsBuffer {
    pub buffer: Buffer,
    pub count: u32,
}

/// Resource to store extracted point lights for the current frame.
/// This avoids spawning entities which accumulate over time.
#[derive(Resource, Default)]
pub struct ExtractedPointLights {
    pub lights: Vec<ExtractedPointLight>,
}

/// System to extract point lights from main world to render world.
pub fn extract_point_lights(
    mut commands: Commands,
    lights_query: Extract<Query<(&GlobalTransform, &DeferredPointLight), With<DeferredPointLight>>>,
) {
    let mut extracted = ExtractedPointLights::default();
    
    for (transform, light) in lights_query.iter() {
        let position = transform.translation();
        let color_linear = light.color.to_linear();
        
        extracted.lights.push(ExtractedPointLight {
            position,
            color: Vec3::new(color_linear.red, color_linear.green, color_linear.blue),
            intensity: light.intensity,
            radius: light.radius,
        });
    }
    
    commands.insert_resource(extracted);
}

/// System to prepare point lights storage buffer.
pub fn prepare_point_lights(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    extracted_lights: Option<Res<ExtractedPointLights>>,
) {
    let mut storage = PointLightsStorage::default();
    let mut count = 0u32;
    
    if let Some(extracted) = extracted_lights {
        for light in extracted.lights.iter() {
            if count >= MAX_POINT_LIGHTS as u32 {
                warn_once!("Too many point lights ({} > {}), extras ignored", 
                      extracted.lights.len(), MAX_POINT_LIGHTS);
                break;
            }
            
            storage.lights.push(GpuPointLight {
                position: [light.position.x, light.position.y, light.position.z, 0.0],
                color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
                radius_padding: [light.radius, 0.0, 0.0, 0.0],
            });
            
            count += 1;
        }
    }
    
    storage.header.count[0] = count;
    
    // Create storage buffer (read-only in shader)
    let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("point_lights_storage"),
        contents: &storage.to_bytes(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });
    
    commands.insert_resource(PointLightsBuffer { buffer, count });
    
    if count > 0 {
        debug!("Prepared {} point lights for lighting pass (storage buffer)", count);
    }
}
