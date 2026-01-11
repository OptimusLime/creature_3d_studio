//! Sky dome configuration for SEUS-style atmospheric scattering.
//!
//! This module defines the configuration resource for the sky dome pass,
//! which renders a procedural sky with physically-based atmospheric scattering
//! inspired by Sonic Ether's Unbelievable Shaders (SEUS).
//!
//! ## Features
//! - Rayleigh/Mie scattering for realistic sky colors
//! - Dynamic sun with corona and proper horizon reddening
//! - Dual moon system with ray-traced sphere rendering
//! - Procedural starfield with twilight fade

use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;

/// Configuration for a moon in the sky dome.
#[derive(Clone, Debug)]
pub struct MoonAppearance {
    /// Angular size of the moon disc in radians
    /// Game moons should be HUGE for visual impact:
    /// - 0.3 radians = ~17 degrees (impressive)
    /// - 0.5 radians = ~29 degrees (massive, dramatic)
    /// - Earth's moon is only 0.009 radians (~0.5 degrees) - boring!
    pub size: f32,
    /// Core color of the moon (bright, saturated)
    pub color: Color,
    /// Glow intensity (1.0+ for dramatic effect)
    pub glow_intensity: f32,
    /// Glow falloff (lower = softer, more atmospheric glow)
    pub glow_falloff: f32,
    /// Limb darkening strength (0.0 = uniform, 1.0 = dark edges)
    pub limb_darkening: f32,
    /// Surface detail intensity (procedural crater/texture noise)
    pub surface_detail: f32,
}

impl Default for MoonAppearance {
    fn default() -> Self {
        Self {
            size: 0.10, // ~6 degrees - fantasy-sized but not overwhelming
            color: Color::WHITE,
            glow_intensity: 0.2, // Subtle eerie glow
            glow_falloff: 2.0,
            limb_darkening: 0.4,
            surface_detail: 0.3,
        }
    }
}

impl MoonAppearance {
    /// Purple moon preset - strong emissive bloom like lights
    pub fn purple() -> Self {
        Self {
            size: 0.12,                         // ~7 degrees - noticeable but not overwhelming
            color: Color::srgb(0.8, 0.6, 0.95), // Brighter purple for bloom
            glow_intensity: 1.2,                // STRONG emissive glow - bloom out!
            glow_falloff: 1.2,                  // Soft falloff for wide bloom
            limb_darkening: 0.3,                // Less darkening, more glow
            surface_detail: 0.2,
        }
    }

    /// Orange moon preset - warm bloom
    pub fn orange() -> Self {
        Self {
            size: 0.08,                         // ~5 degrees - smaller secondary moon
            color: Color::srgb(0.95, 0.7, 0.4), // Brighter amber for bloom
            glow_intensity: 1.0,                // Strong emissive glow
            glow_falloff: 1.3,                  // Soft falloff for bloom
            limb_darkening: 0.25,
            surface_detail: 0.2,
        }
    }
}

/// Configuration for the sun in the sky dome.
#[derive(Clone, Debug)]
pub struct SunAppearance {
    /// Angular size of the sun disc in radians (Earth's sun is ~0.009)
    /// For fantasy settings, 0.02-0.05 creates a more dramatic sun
    pub size: f32,
    /// Sun color (typically warm white to yellow)
    pub color: Color,
    /// Sun intensity (controls brightness and corona strength)
    pub intensity: f32,
}

impl Default for SunAppearance {
    fn default() -> Self {
        Self {
            size: 0.03,                          // ~1.7 degrees - slightly larger than real
            color: Color::srgb(1.0, 0.95, 0.85), // Warm white
            intensity: 1.0,
        }
    }
}

/// Configuration for the sky dome rendering pass.
///
/// Controls the appearance of the procedural sky rendered behind all geometry.
/// Uses SEUS-style atmospheric scattering for realistic sky colors.
#[derive(Resource, Clone, Debug, ExtractResource)]
pub struct SkyDomeConfig {
    /// Whether sky dome rendering is enabled.
    /// When disabled, sky pixels show the fog color from the lighting pass.
    pub enabled: bool,

    /// Color at the horizon (used as base tint, atmospheric scattering adds detail)
    pub horizon_color: Color,

    /// Color at the zenith (used as base tint)
    pub zenith_color: Color,

    /// Controls the gradient blend curve.
    /// Higher values = sharper transition near horizon.
    pub horizon_blend_power: f32,

    /// Time of day (0.0 - 1.0).
    /// Controls sun position and sky coloring.
    /// 0.0 = midnight, 0.25 = sunrise, 0.5 = noon, 0.75 = sunset
    pub time_of_day: f32,

    /// Moon 1 orbital time (0.0 - 1.0).
    /// Independent of time_of_day for separate control.
    /// 0.0 = rising in east, 0.25 = zenith, 0.5 = setting in west, 0.75 = below horizon
    pub moon1_time: f32,

    /// Moon 2 orbital time (0.0 - 1.0).
    /// Independent of time_of_day for separate control.
    pub moon2_time: f32,

    /// Sun appearance settings
    pub sun: SunAppearance,

    /// Moon 1 (purple) appearance settings
    pub moon1: MoonAppearance,

    /// Moon 2 (orange) appearance settings
    pub moon2: MoonAppearance,

    /// Whether to render moons
    pub moons_enabled: bool,

    /// Path to cloud texture (relative to assets folder).
    /// If None, uses procedural placeholder pattern.
    /// Set to "textures/generated/mj_clouds_001.png" to use MJ-generated clouds.
    pub cloud_texture_path: Option<String>,

    /// Path to moon 1 texture (relative to assets folder).
    /// If None, uses fallback texture.
    pub moon1_texture_path: Option<String>,

    /// Path to moon 2 texture (relative to assets folder).
    /// If None, uses fallback texture.
    pub moon2_texture_path: Option<String>,
}

impl Default for SkyDomeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            // These colors are used as base tints; atmospheric scattering adds detail
            horizon_color: Color::srgb(0.1, 0.05, 0.15),
            zenith_color: Color::srgb(0.02, 0.01, 0.03),
            horizon_blend_power: 1.5,
            // Default: night time with moons visible
            time_of_day: 0.1,
            // Moon orbital times (independent control)
            moon1_time: 0.25, // Purple moon at zenith
            moon2_time: 0.15, // Orange moon rising
            sun: SunAppearance::default(),
            moon1: MoonAppearance::purple(),
            moon2: MoonAppearance::orange(),
            moons_enabled: true,
            // Default to MJ-generated cloud texture
            cloud_texture_path: Some("textures/generated/mj_clouds_001.png".to_string()),
            // Default to MJ-generated moon textures
            moon1_texture_path: Some("textures/generated/mj_moon_purple.png".to_string()),
            moon2_texture_path: Some("textures/generated/mj_moon_orange.png".to_string()),
        }
    }
}

impl SkyDomeConfig {
    /// Create a midnight configuration (deep night, moons high)
    pub fn midnight() -> Self {
        Self {
            time_of_day: 0.0,
            ..Default::default()
        }
    }

    /// Create a sunrise configuration (golden hour, stars fading)
    pub fn sunrise() -> Self {
        Self {
            time_of_day: 0.25,
            sun: SunAppearance {
                intensity: 0.8,
                color: Color::srgb(1.0, 0.8, 0.5),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Create a noon configuration (bright day, no stars)
    pub fn noon() -> Self {
        Self {
            time_of_day: 0.5,
            moons_enabled: false, // Moons not visible during day
            ..Default::default()
        }
    }

    /// Create a sunset configuration (golden hour)
    pub fn sunset() -> Self {
        Self {
            time_of_day: 0.75,
            sun: SunAppearance {
                intensity: 0.9,
                color: Color::srgb(1.0, 0.6, 0.3),
                ..Default::default()
            },
            ..Default::default()
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

    /// Set sun intensity
    pub fn with_sun_intensity(mut self, intensity: f32) -> Self {
        self.sun.intensity = intensity;
        self
    }

    /// Set cloud texture path (relative to assets folder)
    pub fn with_cloud_texture(mut self, path: Option<String>) -> Self {
        self.cloud_texture_path = path;
        self
    }
}
