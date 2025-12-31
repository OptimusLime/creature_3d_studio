//! Day/Night Cycle System
//!
//! Provides configurable day/night cycles with:
//! - Independent dual-moon orbital configurations
//! - Time-based position, color, and intensity interpolation
//! - LUT-based color grading (Phase 4+)
//!
//! # Example
//!
//! ```ignore
//! use studio_core::{DayNightCycle, VoxelWorldApp};
//!
//! VoxelWorldApp::new("Day Night Demo")
//!     .with_day_night_cycle(DayNightCycle::dark_world())
//!     .run();
//! ```

use bevy::prelude::*;
use std::f32::consts::TAU;

/// Configuration for a single moon's orbital cycle.
///
/// Each moon can have independent orbital parameters, allowing
/// for complex dual-moon lighting scenarios.
#[derive(Clone, Debug)]
pub struct MoonCycleConfig {
    /// Period multiplier relative to main cycle (1.0 = one orbit per cycle).
    /// Example: 0.8 = moon completes orbit in 80% of the time.
    pub period: f32,

    /// Phase offset (0.0 - 1.0) - when this moon reaches zenith.
    /// 0.0 = zenith at cycle start, 0.5 = zenith at cycle midpoint.
    pub phase_offset: f32,

    /// Orbital inclination in degrees - tilt of orbit plane.
    /// 0 = equatorial orbit, 45 = tilted 45 degrees.
    pub inclination: f32,

    /// Base color at zenith (when moon is highest).
    pub zenith_color: Vec3,

    /// Color near horizon (tinted by "atmospheric scattering").
    pub horizon_color: Vec3,

    /// Intensity multiplier at zenith.
    pub zenith_intensity: f32,

    /// Intensity multiplier at horizon.
    pub horizon_intensity: f32,

    /// Height at which moon "sets" (below this = not visible).
    /// Typically a small negative value like -0.1.
    pub set_height: f32,
}

impl Default for MoonCycleConfig {
    fn default() -> Self {
        Self {
            period: 1.0,
            phase_offset: 0.0,
            inclination: 23.5,
            zenith_color: Vec3::new(0.9, 0.9, 1.0),
            horizon_color: Vec3::new(1.0, 0.5, 0.3),
            zenith_intensity: 0.5,
            horizon_intensity: 0.1,
            set_height: -0.1,
        }
    }
}

impl MoonCycleConfig {
    /// Purple moon preset - slower orbit, deep purple color.
    pub fn purple_moon() -> Self {
        Self {
            period: 1.0,
            phase_offset: 0.0, // Rises at start of cycle
            inclination: 30.0,
            zenith_color: Vec3::new(0.5, 0.2, 0.9),  // Deep purple
            horizon_color: Vec3::new(0.8, 0.3, 0.5), // Pink-purple near horizon
            zenith_intensity: 0.6,
            horizon_intensity: 0.15,
            set_height: -0.15,
        }
    }

    /// Orange moon preset - faster orbit, warm orange color.
    pub fn orange_moon() -> Self {
        Self {
            period: 0.8, // Faster than purple moon
            phase_offset: 0.5, // Offset by half cycle
            inclination: 15.0,
            zenith_color: Vec3::new(1.0, 0.5, 0.15),  // Warm orange
            horizon_color: Vec3::new(1.0, 0.3, 0.05), // Deep orange/red near horizon
            zenith_intensity: 0.5,
            horizon_intensity: 0.1,
            set_height: -0.1,
        }
    }

    /// Calculate moon direction and height at a given cycle time.
    ///
    /// # Arguments
    /// * `cycle_time` - Time in the main cycle (0.0 - 1.0)
    ///
    /// # Returns
    /// * `(direction, height)` where:
    ///   - `direction`: normalized Vec3 pointing FROM scene TO moon (light direction is negated)
    ///   - `height`: -1.0 to 1.0 (negative = below horizon)
    pub fn calculate_position(&self, cycle_time: f32) -> (Vec3, f32) {
        // Adjust time by period and phase offset
        let moon_time = (cycle_time / self.period + self.phase_offset).fract();

        // Convert to radians for orbital position
        let angle = moon_time * TAU;

        // Calculate position on inclined orbit
        let incline_rad = self.inclination.to_radians();

        // Basic circular orbit in XY plane, then tilt around X axis
        let x = angle.cos();
        let y_base = angle.sin();
        let y = y_base * incline_rad.cos();
        let z = y_base * incline_rad.sin();

        // Height is the y component (positive = above horizon)
        let height = y;

        // Direction TO the moon (for lighting, we negate this)
        // Moon at position (x, y, z) means light comes FROM that direction
        let direction = Vec3::new(x, y, z).normalize();

        (direction, height)
    }

    /// Calculate moon color based on height above horizon.
    ///
    /// Interpolates between horizon_color (at height=-1) and zenith_color (at height=1).
    pub fn calculate_color(&self, height: f32) -> Vec3 {
        // Map height from [-1, 1] to [0, 1] for lerp
        let t = ((height + 1.0) / 2.0).clamp(0.0, 1.0);
        self.horizon_color.lerp(self.zenith_color, t)
    }

    /// Calculate moon intensity based on height above horizon.
    ///
    /// Returns 0.0 when moon is below set_height (has "set").
    pub fn calculate_intensity(&self, height: f32) -> f32 {
        if height < self.set_height {
            return 0.0; // Moon has set
        }

        // Smooth fade near horizon to avoid harsh cutoff
        let fade_start = self.set_height;
        let fade_end = 0.3; // Fully bright above this height
        let fade = ((height - fade_start) / (fade_end - fade_start)).clamp(0.0, 1.0);

        // Interpolate base intensity based on height
        let t = ((height + 1.0) / 2.0).clamp(0.0, 1.0);
        let base_intensity = self.horizon_intensity + (self.zenith_intensity - self.horizon_intensity) * t;

        base_intensity * fade
    }
}

// ============================================================================
// Color LUT System
// ============================================================================

/// A keyframe in the color grading timeline.
///
/// Defines lighting and color values at a specific point in the cycle.
#[derive(Clone, Debug)]
pub struct ColorKeyframe {
    /// Time in cycle (0.0 - 1.0).
    pub time: f32,

    /// Ambient light color.
    pub ambient_color: Vec3,
    /// Ambient light intensity.
    pub ambient_intensity: f32,

    /// Fog color.
    pub fog_color: Vec3,
    /// Fog density (0.0 = no fog, 1.0 = full fog at max distance).
    pub fog_density: f32,

    /// Exposure adjustment for tone mapping.
    pub exposure: f32,
    /// Color tint applied to final image (multiply).
    pub color_tint: Vec3,
    /// Saturation multiplier.
    pub saturation: f32,
    /// Contrast adjustment.
    pub contrast: f32,
}

impl Default for ColorKeyframe {
    fn default() -> Self {
        Self {
            time: 0.0,
            ambient_color: Vec3::new(0.02, 0.01, 0.03),
            ambient_intensity: 0.05,
            fog_color: Vec3::new(0.02, 0.01, 0.03),
            fog_density: 0.6,
            exposure: 1.0,
            color_tint: Vec3::ONE,
            saturation: 1.0,
            contrast: 1.0,
        }
    }
}

/// Interpolation mode for color LUT sampling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InterpolationMode {
    /// Linear interpolation between keyframes.
    #[default]
    Linear,
    /// Smooth Catmull-Rom spline interpolation.
    CatmullRom,
    /// No interpolation - snap to nearest keyframe.
    Step,
}

/// Configuration for time-of-day color grading using keyframes.
#[derive(Clone, Debug)]
pub struct ColorLutConfig {
    /// Keyframes defining colors at specific times (must be sorted by time).
    pub keyframes: Vec<ColorKeyframe>,
    /// Interpolation mode between keyframes.
    pub interpolation: InterpolationMode,
}

impl Default for ColorLutConfig {
    fn default() -> Self {
        Self::dark_world()
    }
}

impl ColorLutConfig {
    /// Dark fantasy dual-moon color grading.
    pub fn dark_world() -> Self {
        Self {
            interpolation: InterpolationMode::Linear,
            keyframes: vec![
                // Deep Night (0.0)
                ColorKeyframe {
                    time: 0.0,
                    ambient_color: Vec3::new(0.02, 0.01, 0.04),
                    ambient_intensity: 0.05,
                    fog_color: Vec3::new(0.02, 0.01, 0.03),
                    fog_density: 0.6,
                    exposure: 1.0,
                    color_tint: Vec3::ONE,
                    saturation: 1.1,
                    contrast: 1.1,
                },
                // Pre-Dawn (0.2)
                ColorKeyframe {
                    time: 0.2,
                    ambient_color: Vec3::new(0.04, 0.02, 0.03),
                    ambient_intensity: 0.08,
                    fog_color: Vec3::new(0.05, 0.02, 0.03),
                    fog_density: 0.5,
                    exposure: 0.9,
                    color_tint: Vec3::new(1.0, 0.95, 0.9),
                    saturation: 1.2,
                    contrast: 1.0,
                },
                // Dawn Peak (0.3)
                ColorKeyframe {
                    time: 0.3,
                    ambient_color: Vec3::new(0.15, 0.05, 0.08),
                    ambient_intensity: 0.15,
                    fog_color: Vec3::new(0.2, 0.08, 0.1),
                    fog_density: 0.7,
                    exposure: 1.1,
                    color_tint: Vec3::new(1.0, 0.8, 0.7),
                    saturation: 1.3,
                    contrast: 0.95,
                },
                // Twilight (0.4)
                ColorKeyframe {
                    time: 0.4,
                    ambient_color: Vec3::new(0.08, 0.04, 0.02),
                    ambient_intensity: 0.1,
                    fog_color: Vec3::new(0.06, 0.03, 0.02),
                    fog_density: 0.5,
                    exposure: 1.0,
                    color_tint: Vec3::new(1.0, 0.9, 0.85),
                    saturation: 1.15,
                    contrast: 1.05,
                },
                // Night (0.5)
                ColorKeyframe {
                    time: 0.5,
                    ambient_color: Vec3::new(0.03, 0.015, 0.01),
                    ambient_intensity: 0.05,
                    fog_color: Vec3::new(0.03, 0.015, 0.01),
                    fog_density: 0.6,
                    exposure: 1.0,
                    color_tint: Vec3::ONE,
                    saturation: 1.1,
                    contrast: 1.1,
                },
                // Second Transition (0.7)
                ColorKeyframe {
                    time: 0.7,
                    ambient_color: Vec3::new(0.12, 0.04, 0.1),
                    ambient_intensity: 0.12,
                    fog_color: Vec3::new(0.15, 0.05, 0.1),
                    fog_density: 0.65,
                    exposure: 1.05,
                    color_tint: Vec3::new(0.95, 0.85, 1.0),
                    saturation: 1.25,
                    contrast: 0.98,
                },
                // Late Night (0.85)
                ColorKeyframe {
                    time: 0.85,
                    ambient_color: Vec3::new(0.02, 0.01, 0.05),
                    ambient_intensity: 0.05,
                    fog_color: Vec3::new(0.02, 0.01, 0.04),
                    fog_density: 0.6,
                    exposure: 1.0,
                    color_tint: Vec3::new(0.95, 0.9, 1.0),
                    saturation: 1.1,
                    contrast: 1.1,
                },
                // Wrap to start (1.0)
                ColorKeyframe {
                    time: 1.0,
                    ambient_color: Vec3::new(0.02, 0.01, 0.04),
                    ambient_intensity: 0.05,
                    fog_color: Vec3::new(0.02, 0.01, 0.03),
                    fog_density: 0.6,
                    exposure: 1.0,
                    color_tint: Vec3::ONE,
                    saturation: 1.1,
                    contrast: 1.1,
                },
            ],
        }
    }

    /// Simple two-state LUT for testing.
    pub fn simple() -> Self {
        Self {
            interpolation: InterpolationMode::Linear,
            keyframes: vec![
                ColorKeyframe {
                    time: 0.0,
                    ambient_color: Vec3::new(0.02, 0.01, 0.03),
                    ambient_intensity: 0.05,
                    fog_color: Vec3::new(0.02, 0.01, 0.03),
                    fog_density: 0.6,
                    ..default()
                },
                ColorKeyframe {
                    time: 0.5,
                    ambient_color: Vec3::new(0.1, 0.05, 0.02),
                    ambient_intensity: 0.1,
                    fog_color: Vec3::new(0.08, 0.04, 0.02),
                    fog_density: 0.4,
                    color_tint: Vec3::new(1.0, 0.9, 0.8),
                    saturation: 1.2,
                    ..default()
                },
                ColorKeyframe {
                    time: 1.0,
                    ambient_color: Vec3::new(0.02, 0.01, 0.03),
                    ambient_intensity: 0.05,
                    fog_color: Vec3::new(0.02, 0.01, 0.03),
                    fog_density: 0.6,
                    ..default()
                },
            ],
        }
    }

    /// Sample the LUT at a given time, interpolating between keyframes.
    pub fn sample(&self, time: f32) -> ColorKeyframe {
        let time = time.fract(); // Wrap to 0-1

        if self.keyframes.is_empty() {
            return ColorKeyframe::default();
        }

        if self.keyframes.len() == 1 {
            return self.keyframes[0].clone();
        }

        // Find surrounding keyframes
        let mut prev_idx = 0;
        let mut next_idx = 0;

        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.time <= time {
                prev_idx = i;
            }
            if kf.time >= time {
                next_idx = i;
                break;
            }
            next_idx = i; // Handle wrap case
        }

        // Handle same keyframe case
        if prev_idx == next_idx {
            return self.keyframes[prev_idx].clone();
        }

        let prev = &self.keyframes[prev_idx];
        let next = &self.keyframes[next_idx];

        // Calculate interpolation factor
        let span = next.time - prev.time;
        let t = if span > 0.0 {
            (time - prev.time) / span
        } else {
            0.0
        };

        // Apply interpolation mode
        let t = match self.interpolation {
            InterpolationMode::Linear => t,
            InterpolationMode::CatmullRom => smoothstep(t),
            InterpolationMode::Step => {
                if t < 0.5 {
                    0.0
                } else {
                    1.0
                }
            }
        };

        // Interpolate all fields
        ColorKeyframe {
            time,
            ambient_color: prev.ambient_color.lerp(next.ambient_color, t),
            ambient_intensity: lerp(prev.ambient_intensity, next.ambient_intensity, t),
            fog_color: prev.fog_color.lerp(next.fog_color, t),
            fog_density: lerp(prev.fog_density, next.fog_density, t),
            exposure: lerp(prev.exposure, next.exposure, t),
            color_tint: prev.color_tint.lerp(next.color_tint, t),
            saturation: lerp(prev.saturation, next.saturation, t),
            contrast: lerp(prev.contrast, next.contrast, t),
        }
    }
}

/// Smoothstep function for smooth interpolation.
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Linear interpolation helper.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ============================================================================
// Main DayNightCycle Resource
// ============================================================================

/// Main day/night cycle resource.
///
/// Controls time progression and computes moon states each frame.
/// Insert this resource and add the day/night systems to enable cycling.
#[derive(Resource, Clone)]
pub struct DayNightCycle {
    /// Current time in cycle (0.0 - 1.0, wraps automatically).
    pub time: f32,

    /// Whether the cycle is paused (useful for screenshots).
    pub paused: bool,

    /// Speed multiplier. 1.0 = one cycle per second (very fast for testing).
    /// Typical values: 0.01 (100 sec/cycle), 0.001 (1000 sec/cycle).
    pub speed: f32,

    /// Configuration for moon 1 (typically purple).
    pub moon1_config: MoonCycleConfig,

    /// Configuration for moon 2 (typically orange).
    pub moon2_config: MoonCycleConfig,

    /// Color grading LUT configuration.
    pub color_lut: ColorLutConfig,

    // Cached computed values (updated each frame by update system)

    /// Current moon 1 direction (FROM scene TO moon, for lighting use -direction).
    pub moon1_direction: Vec3,
    /// Current moon 1 color.
    pub moon1_color: Vec3,
    /// Current moon 1 intensity.
    pub moon1_intensity: f32,

    /// Current moon 2 direction.
    pub moon2_direction: Vec3,
    /// Current moon 2 color.
    pub moon2_color: Vec3,
    /// Current moon 2 intensity.
    pub moon2_intensity: f32,

    // Cached color grading values (sampled from LUT each frame)

    /// Current ambient light color.
    pub ambient_color: Vec3,
    /// Current ambient light intensity.
    pub ambient_intensity: f32,
    /// Current fog color.
    pub fog_color: Vec3,
    /// Current fog density.
    pub fog_density: f32,
    /// Current exposure.
    pub exposure: f32,
    /// Current color tint.
    pub color_tint: Vec3,
    /// Current saturation.
    pub saturation: f32,
    /// Current contrast.
    pub contrast: f32,
}

impl Default for DayNightCycle {
    fn default() -> Self {
        Self::dark_world()
    }
}

impl DayNightCycle {
    /// Create a dark fantasy dual-moon cycle.
    pub fn dark_world() -> Self {
        let mut cycle = Self {
            time: 0.0,
            paused: false,
            speed: 0.1, // 10 seconds per cycle (fast for testing)

            moon1_config: MoonCycleConfig::purple_moon(),
            moon2_config: MoonCycleConfig::orange_moon(),
            color_lut: ColorLutConfig::dark_world(),

            // Initialize cached moon values (will be updated on first frame)
            moon1_direction: Vec3::Y,
            moon1_color: Vec3::ONE,
            moon1_intensity: 0.5,
            moon2_direction: Vec3::Y,
            moon2_color: Vec3::ONE,
            moon2_intensity: 0.5,

            // Initialize cached color grading values
            ambient_color: Vec3::new(0.02, 0.01, 0.03),
            ambient_intensity: 0.05,
            fog_color: Vec3::new(0.02, 0.01, 0.03),
            fog_density: 0.6,
            exposure: 1.0,
            color_tint: Vec3::ONE,
            saturation: 1.0,
            contrast: 1.0,
        };
        // Compute initial values
        cycle.update_cached_values();
        cycle
    }

    /// Set a custom color LUT.
    pub fn with_color_lut(mut self, lut: ColorLutConfig) -> Self {
        self.color_lut = lut;
        self.update_cached_values();
        self
    }

    /// Create a cycle with custom speed.
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Create a cycle starting at a specific time.
    pub fn with_time(mut self, time: f32) -> Self {
        self.time = time.fract();
        self.update_cached_values();
        self
    }

    /// Set time directly (for screenshot sequences).
    pub fn set_time(&mut self, time: f32) {
        self.time = time.fract();
        self.update_cached_values();
    }

    /// Advance time and update cached moon values.
    pub fn update(&mut self, delta_seconds: f32) {
        if !self.paused {
            self.time = (self.time + delta_seconds * self.speed).fract();
        }
        self.update_cached_values();
    }

    /// Update cached moon direction/color/intensity values and color grading.
    fn update_cached_values(&mut self) {
        // Moon 1
        let (dir1, height1) = self.moon1_config.calculate_position(self.time);
        self.moon1_direction = dir1;
        self.moon1_color = self.moon1_config.calculate_color(height1);
        self.moon1_intensity = self.moon1_config.calculate_intensity(height1);

        // Moon 2
        let (dir2, height2) = self.moon2_config.calculate_position(self.time);
        self.moon2_direction = dir2;
        self.moon2_color = self.moon2_config.calculate_color(height2);
        self.moon2_intensity = self.moon2_config.calculate_intensity(height2);

        // Sample color LUT
        let color = self.color_lut.sample(self.time);
        self.ambient_color = color.ambient_color;
        self.ambient_intensity = color.ambient_intensity;
        self.fog_color = color.fog_color;
        self.fog_density = color.fog_density;
        self.exposure = color.exposure;
        self.color_tint = color.color_tint;
        self.saturation = color.saturation;
        self.contrast = color.contrast;
    }
}

/// System to update the day/night cycle each frame.
pub fn update_day_night_cycle(time: Res<Time>, mut cycle: ResMut<DayNightCycle>) {
    cycle.update(time.delta_secs());
}

/// System to apply cycle state to MoonConfig for rendering.
///
/// This syncs the computed moon directions/colors/intensities to the
/// `MoonConfig` resource used by the deferred lighting shader.
pub fn apply_cycle_to_moon_config(
    cycle: Res<DayNightCycle>,
    mut moon_config: ResMut<crate::deferred::MoonConfig>,
) {
    // Only update if cycle changed
    if !cycle.is_changed() {
        return;
    }

    // Convert direction TO moon into direction FROM moon (light direction)
    // The shader expects direction FROM light TO scene
    moon_config.moon1_direction = -cycle.moon1_direction;
    moon_config.moon1_color = cycle.moon1_color;
    moon_config.moon1_intensity = cycle.moon1_intensity;

    moon_config.moon2_direction = -cycle.moon2_direction;
    moon_config.moon2_color = cycle.moon2_color;
    moon_config.moon2_intensity = cycle.moon2_intensity;
}

/// System to apply cycle color grading to BloomConfig.
///
/// This syncs exposure from the color LUT to the bloom post-processing.
pub fn apply_cycle_to_bloom(
    cycle: Res<DayNightCycle>,
    mut bloom_config: ResMut<crate::deferred::BloomConfig>,
) {
    // Only update if cycle changed
    if !cycle.is_changed() {
        return;
    }

    bloom_config.exposure = cycle.exposure;
}

/// Plugin that adds day/night cycle systems.
pub struct DayNightCyclePlugin;

impl Plugin for DayNightCyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (
                update_day_night_cycle,
                apply_cycle_to_moon_config,
                apply_cycle_to_bloom,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moon_position_varies_over_cycle() {
        let config = MoonCycleConfig::default();

        let (dir_0, height_0) = config.calculate_position(0.0);
        let (dir_25, height_25) = config.calculate_position(0.25);
        let (dir_50, height_50) = config.calculate_position(0.5);
        let (dir_75, height_75) = config.calculate_position(0.75);

        println!("t=0.00: dir={:?}, height={}", dir_0, height_0);
        println!("t=0.25: dir={:?}, height={}", dir_25, height_25);
        println!("t=0.50: dir={:?}, height={}", dir_50, height_50);
        println!("t=0.75: dir={:?}, height={}", dir_75, height_75);

        // Direction should vary over the cycle (moon moves)
        assert!(
            (dir_0 - dir_50).length() > 0.1,
            "Direction should change between 0.0 and 0.5: d0={:?}, d50={:?}",
            dir_0, dir_50
        );
        assert!(
            (dir_25 - dir_75).length() > 0.1,
            "Direction should change between 0.25 and 0.75: d25={:?}, d75={:?}",
            dir_25, dir_75
        );
    }

    #[test]
    fn test_moon_height_range() {
        let config = MoonCycleConfig::default();

        // Sample many points and verify height stays in expected range
        for i in 0..100 {
            let t = i as f32 / 100.0;
            let (_, height) = config.calculate_position(t);
            assert!(
                height >= -1.0 && height <= 1.0,
                "Height {} at time {} out of range",
                height,
                t
            );
        }
    }

    #[test]
    fn test_moon_intensity_zero_when_set() {
        let config = MoonCycleConfig {
            set_height: -0.1,
            ..Default::default()
        };

        // Well below horizon should be zero
        assert_eq!(config.calculate_intensity(-0.5), 0.0);

        // Above horizon should be positive
        assert!(config.calculate_intensity(0.5) > 0.0);
    }

    #[test]
    fn test_cycle_time_wraps() {
        let mut cycle = DayNightCycle::dark_world();

        cycle.time = 0.9;
        cycle.update(2.0); // With speed 0.1, this adds 0.2, going to 1.1 -> wraps to 0.1

        assert!(cycle.time >= 0.0 && cycle.time < 1.0, "Time should wrap to [0, 1)");
    }

    #[test]
    fn test_paused_cycle_does_not_advance() {
        let mut cycle = DayNightCycle::dark_world();
        cycle.paused = true;
        let initial_time = cycle.time;

        cycle.update(1.0);

        assert_eq!(cycle.time, initial_time, "Paused cycle should not advance");
    }

    #[test]
    fn test_lut_sample_interpolates() {
        let lut = ColorLutConfig::simple();

        // At time 0.0, should be first keyframe
        let kf0 = lut.sample(0.0);
        assert!((kf0.ambient_intensity - 0.05).abs() < 0.01);

        // At time 0.5, should be second keyframe
        let kf5 = lut.sample(0.5);
        assert!((kf5.ambient_intensity - 0.1).abs() < 0.01);

        // At time 0.25, should be between first and second
        let kf25 = lut.sample(0.25);
        assert!(kf25.ambient_intensity > 0.05 && kf25.ambient_intensity < 0.1);
    }

    #[test]
    fn test_lut_sample_wraps() {
        let lut = ColorLutConfig::simple();

        // Time 1.5 should wrap to 0.5
        let kf = lut.sample(1.5);
        assert!((kf.ambient_intensity - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_cycle_updates_lut_values() {
        let mut cycle = DayNightCycle::dark_world();
        cycle.set_time(0.0);
        let ambient_0 = cycle.ambient_color;

        cycle.set_time(0.3); // Dawn peak
        let ambient_30 = cycle.ambient_color;

        // Ambient color should be different at different times
        assert!(
            (ambient_0 - ambient_30).length() > 0.01,
            "Ambient color should change: {:?} vs {:?}",
            ambient_0,
            ambient_30
        );
    }
}
