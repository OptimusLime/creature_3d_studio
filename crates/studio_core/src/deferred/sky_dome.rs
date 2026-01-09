//! Sky dome configuration for procedural sky rendering.
//!
//! This module defines the configuration resource for the sky dome pass,
//! which renders a procedural sky where no geometry exists (depth > 999.0).
//!
//! ## Phase 1: Facade
//! Currently outputs a constant purple color to prove the pipeline works.
//! Future phases will add gradient, moons, and atmospheric effects.

use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;

/// Configuration for the sky dome rendering pass.
///
/// Controls the appearance of the procedural sky rendered behind all geometry.
/// The sky dome runs after bloom and before transparent passes.
#[derive(Resource, Clone, Debug, ExtractResource)]
pub struct SkyDomeConfig {
    /// Whether sky dome rendering is enabled.
    /// When disabled, sky pixels show the fog color from the lighting pass.
    pub enabled: bool,
    // Phase 2+ will add:
    // pub horizon_color: Color,
    // pub zenith_color: Color,
    // pub horizon_blend_power: f32,
}

impl Default for SkyDomeConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}
