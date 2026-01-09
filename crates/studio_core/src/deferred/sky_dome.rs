//! Sky dome configuration for procedural sky rendering.
//!
//! This module defines the configuration resource for the sky dome pass,
//! which renders a procedural sky where no geometry exists (depth > 999.0).
//!
//! ## Phases
//! - Phase 1: Constant purple output (facade)
//! - Phase 2: Horizon-to-zenith gradient
//! - Phase 3+: Moons, atmospheric effects

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

    /// Color at the horizon (warm, slightly lighter)
    pub horizon_color: Color,

    /// Color at the zenith (cool, darker)
    pub zenith_color: Color,

    /// Controls the gradient blend curve.
    /// Higher values = sharper transition near horizon.
    /// 1.0 = linear, 2.0 = quadratic (more sky color visible)
    pub horizon_blend_power: f32,
}

impl Default for SkyDomeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            // Dark fantasy palette - mysterious night sky
            // Horizon: warmer purple with slight orange tint
            horizon_color: Color::srgb(0.25, 0.12, 0.30),
            // Zenith: deep dark purple, almost black
            zenith_color: Color::srgb(0.08, 0.04, 0.12),
            // Quadratic blend for more zenith color at top
            horizon_blend_power: 2.0,
        }
    }
}
