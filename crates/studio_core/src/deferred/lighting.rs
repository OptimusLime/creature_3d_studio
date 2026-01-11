//! Deferred lighting configuration and resources.

use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;

/// Configuration for deferred lighting.
#[derive(Resource, Clone, ExtractResource)]
pub struct DeferredLightingConfig {
    /// Ambient light color and intensity
    pub ambient_color: Color,
    pub ambient_intensity: f32,

    /// Directional light (sun) direction and color
    pub sun_direction: Vec3,
    pub sun_color: Color,
    pub sun_intensity: f32,

    /// Distance fog configuration (Bonsai-style)
    pub fog_color: Color,
    pub fog_start: f32,
    pub fog_end: f32,

    /// Height fog configuration
    /// Height fog creates ground-hugging mist that buildings emerge from
    pub height_fog_density: f32,
    pub height_fog_base: f32,
    pub height_fog_falloff: f32,
}

impl Default for DeferredLightingConfig {
    fn default() -> Self {
        Self {
            // Dark ambient for that 80s horror vibe
            ambient_color: Color::srgb(0.1, 0.05, 0.15),
            ambient_intensity: 0.1,

            // Pale moonlight
            sun_direction: Vec3::new(0.5, -1.0, 0.3).normalize(),
            sun_color: Color::srgb(0.8, 0.85, 1.0),
            sun_intensity: 0.3,

            // Deep purple fog (#1a0a2e)
            fog_color: Color::srgb(0.102, 0.039, 0.180),
            fog_start: 10.0,
            fog_end: 100.0,

            // Height fog - ground mist
            // density: how thick the fog is (0.0 = none, 1.0 = very heavy)
            // base: Y level where fog is densest
            // falloff: how quickly fog thins with height (higher = sharper falloff)
            height_fog_density: 0.6,
            height_fog_base: 5.0,
            height_fog_falloff: 0.05,
        }
    }
}
