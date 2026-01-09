//! Sky dome configuration for procedural sky rendering.
//!
//! This module defines the configuration resource for the sky dome pass,
//! which renders a procedural sky where no geometry exists (depth > 999.0).
//!
//! ## Phases
//! - Phase 1: Constant purple output (facade)
//! - Phase 2: Horizon-to-zenith gradient
//! - Phase 3: Moon rendering with time-of-day control

use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;

/// Configuration for a moon in the sky dome.
#[derive(Clone, Debug)]
pub struct MoonAppearance {
    /// Angular size of the moon disc in radians (0.02 ~ 1 degree)
    pub size: f32,
    /// Core color of the moon
    pub color: Color,
    /// Glow intensity (0.0 = no glow, 1.0 = strong glow)
    pub glow_intensity: f32,
    /// Glow falloff (higher = sharper edge, lower = softer glow)
    pub glow_falloff: f32,
}

impl Default for MoonAppearance {
    fn default() -> Self {
        Self {
            size: 0.05, // ~3 degrees
            color: Color::WHITE,
            glow_intensity: 0.5,
            glow_falloff: 3.0,
        }
    }
}

impl MoonAppearance {
    /// Purple moon preset
    pub fn purple() -> Self {
        Self {
            size: 0.08,                        // Larger disc (~4.5 degrees)
            color: Color::srgb(0.7, 0.4, 1.0), // Brighter purple
            glow_intensity: 0.8,               // More glow
            glow_falloff: 2.0,                 // Softer falloff
        }
    }

    /// Orange moon preset
    pub fn orange() -> Self {
        Self {
            size: 0.06,                        // Larger disc (~3.4 degrees)
            color: Color::srgb(1.0, 0.6, 0.2), // Brighter orange
            glow_intensity: 0.7,               // More glow
            glow_falloff: 2.5,                 // Softer falloff
        }
    }
}

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

    /// Time of day for moon positioning (0.0 - 1.0).
    /// This controls where the moons appear in the sky.
    /// 0.0/1.0 = cycle start, 0.25 = quarter, 0.5 = half, etc.
    pub time_of_day: f32,

    /// Moon 1 (purple) appearance settings
    pub moon1: MoonAppearance,

    /// Moon 2 (orange) appearance settings
    pub moon2: MoonAppearance,

    /// Whether to render moons
    pub moons_enabled: bool,
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
            // Default time - moons visible
            time_of_day: 0.15,
            // Moon appearances
            moon1: MoonAppearance::purple(),
            moon2: MoonAppearance::orange(),
            moons_enabled: true,
        }
    }
}

impl SkyDomeConfig {
    /// Set time of day (0.0 - 1.0)
    pub fn with_time(mut self, time: f32) -> Self {
        self.time_of_day = time.clamp(0.0, 1.0);
        self
    }

    /// Enable or disable moons
    pub fn with_moons(mut self, enabled: bool) -> Self {
        self.moons_enabled = enabled;
        self
    }
}
