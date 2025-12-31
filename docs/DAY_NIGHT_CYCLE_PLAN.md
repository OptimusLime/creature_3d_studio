# Day/Night Cycle System Design

## Overview

A robust, configurable day/night cycle system for dual-moon lighting with:
- **Independent moon cycles** - Each moon can have different orbital periods and phases
- **LUT-based color grading** - Lookup tables for time-of-day color transformations
- **Keyframe interpolation** - Smooth transitions between dawn/day/dusk/night
- **Screenshot sequence capture** - Generate image sequences at specific cycle points

## Architecture

```
                        ┌─────────────────────────────────────────────────┐
                        │              DayNightCycleConfig                │
                        │                                                 │
                        │  moon1_cycle: MoonCycleConfig                   │
                        │  moon2_cycle: MoonCycleConfig                   │
                        │  color_lut: ColorLutConfig                      │
                        │  cycle_speed: f32                               │
                        └─────────────────────────────────────────────────┘
                                             │
                                             ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              DayNightCycle (Resource)                            │
│                                                                                  │
│  time: f32 (0.0 - 1.0)          ◄── current time in cycle                       │
│  paused: bool                    ◄── freeze time for screenshots                │
│  speed: f32                      ◄── 1.0 = real-time, >1 = fast forward         │
│                                                                                  │
│  ┌─────────────────────┐    ┌─────────────────────┐                             │
│  │  Moon1State         │    │  Moon2State         │                             │
│  │  direction: Vec3    │    │  direction: Vec3    │                             │
│  │  color: Vec3        │    │  color: Vec3        │                             │
│  │  intensity: f32     │    │  intensity: f32     │                             │
│  │  phase: f32         │    │  phase: f32         │                             │
│  └─────────────────────┘    └─────────────────────┘                             │
│                                                                                  │
│  ambient_color: Vec3            ◄── interpolated from LUT                       │
│  fog_color: Vec3                ◄── interpolated from LUT                       │
│  exposure: f32                  ◄── interpolated from LUT                       │
│  color_grading: ColorGrading    ◄── interpolated from LUT                       │
└─────────────────────────────────────────────────────────────────────────────────┘
                                             │
                                             ▼
                             ┌───────────────────────────────┐
                             │   update_day_night_cycle()    │
                             │   system (PreUpdate)          │
                             └───────────────────────────────┘
                                             │
                                             ▼
                             ┌───────────────────────────────┐
                             │   apply_cycle_to_moons()      │
                             │   Updates MoonConfig resource │
                             └───────────────────────────────┘
                                             │
                                             ▼
                             ┌───────────────────────────────┐
                             │   Shader receives updated     │
                             │   DirectionalShadowUniforms   │
                             └───────────────────────────────┘
```

## Moon Cycle Configuration

Each moon has independent configuration for its orbital path:

```rust
/// Configuration for a single moon's cycle
#[derive(Clone, Debug)]
pub struct MoonCycleConfig {
    /// Period multiplier relative to main cycle (1.0 = one orbit per day)
    /// Example: 0.5 = moon takes 2 days to orbit
    pub period: f32,
    
    /// Phase offset (0.0 - 1.0) - when this moon reaches zenith
    /// 0.0 = zenith at midnight, 0.5 = zenith at noon
    pub phase_offset: f32,
    
    /// Orbital inclination (degrees) - tilt of orbit plane
    /// 0 = equatorial orbit, 45 = tilted
    pub inclination: f32,
    
    /// Base color at zenith (when moon is highest)
    pub zenith_color: Vec3,
    
    /// Color near horizon (sunrise/sunset tint)
    pub horizon_color: Vec3,
    
    /// Intensity at zenith
    pub zenith_intensity: f32,
    
    /// Intensity at horizon (typically lower due to atmospheric scattering)
    pub horizon_intensity: f32,
    
    /// Height at which moon "sets" (below this = not visible)
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

/// Pre-configured moon cycle presets
impl MoonCycleConfig {
    /// Purple moon - slow orbit, high intensity
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
    
    /// Orange moon - faster orbit, lower intensity
    pub fn orange_moon() -> Self {
        Self {
            period: 0.8, // Faster than purple moon
            phase_offset: 0.5, // Offset by half cycle (when purple is setting, orange is rising)
            inclination: 15.0,
            zenith_color: Vec3::new(1.0, 0.5, 0.15),  // Warm orange
            horizon_color: Vec3::new(1.0, 0.3, 0.05), // Deep orange/red near horizon
            zenith_intensity: 0.5,
            horizon_intensity: 0.1,
            set_height: -0.1,
        }
    }
}
```

## LUT System for Color Grading

The LUT (Look-Up Table) system allows defining color transformations at specific times of day, with smooth interpolation between them.

```rust
/// A keyframe in the color grading timeline
#[derive(Clone, Debug)]
pub struct ColorKeyframe {
    /// Time in cycle (0.0 - 1.0)
    pub time: f32,
    
    /// Ambient light color
    pub ambient_color: Vec3,
    
    /// Ambient light intensity
    pub ambient_intensity: f32,
    
    /// Fog color
    pub fog_color: Vec3,
    
    /// Fog density (0.0 = no fog, 1.0 = full)
    pub fog_density: f32,
    
    /// Exposure adjustment (for tone mapping)
    pub exposure: f32,
    
    /// Color tint applied to final image (multiply)
    pub color_tint: Vec3,
    
    /// Saturation multiplier
    pub saturation: f32,
    
    /// Contrast adjustment
    pub contrast: f32,
}

/// Configuration for time-of-day color grading
#[derive(Clone, Debug)]
pub struct ColorLutConfig {
    /// Keyframes defining colors at specific times
    /// Must be sorted by time
    pub keyframes: Vec<ColorKeyframe>,
    
    /// Interpolation mode
    pub interpolation: InterpolationMode,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum InterpolationMode {
    #[default]
    Linear,
    CatmullRom,  // Smooth spline interpolation
    Step,        // No interpolation, snap to nearest
}

impl Default for ColorLutConfig {
    fn default() -> Self {
        Self::dark_world()
    }
}

impl ColorLutConfig {
    /// Dark fantasy dual-moon color grading
    /// Primarily night-time with brief twilight transitions
    pub fn dark_world() -> Self {
        Self {
            interpolation: InterpolationMode::CatmullRom,
            keyframes: vec![
                // Deep Night (0.0) - Both moons potentially visible
                ColorKeyframe {
                    time: 0.0,
                    ambient_color: Vec3::new(0.02, 0.01, 0.04),  // Very dark purple
                    ambient_intensity: 0.05,
                    fog_color: Vec3::new(0.02, 0.01, 0.03),
                    fog_density: 0.6,
                    exposure: 1.0,
                    color_tint: Vec3::ONE,
                    saturation: 1.1,  // Slight boost for vibrant moon colors
                    contrast: 1.1,
                },
                // Pre-Dawn (0.2) - Purple moon setting, hint of orange on horizon
                ColorKeyframe {
                    time: 0.2,
                    ambient_color: Vec3::new(0.04, 0.02, 0.03),
                    ambient_intensity: 0.08,
                    fog_color: Vec3::new(0.05, 0.02, 0.03),  // Slight purple-pink
                    fog_density: 0.5,
                    exposure: 0.9,
                    color_tint: Vec3::new(1.0, 0.95, 0.9),  // Warm shift
                    saturation: 1.2,
                    contrast: 1.0,
                },
                // Dawn/Dusk Peak (0.3) - Maximum color transition
                ColorKeyframe {
                    time: 0.3,
                    ambient_color: Vec3::new(0.15, 0.05, 0.08),  // Purple-pink ambient
                    ambient_intensity: 0.15,
                    fog_color: Vec3::new(0.2, 0.08, 0.1),  // Pink fog
                    fog_density: 0.7,  // Thicker fog at transition
                    exposure: 1.1,
                    color_tint: Vec3::new(1.0, 0.8, 0.7),  // Strong warm tint
                    saturation: 1.3,  // Vibrant colors at dawn/dusk
                    contrast: 0.95,
                },
                // Twilight (0.4) - Orange moon rising
                ColorKeyframe {
                    time: 0.4,
                    ambient_color: Vec3::new(0.08, 0.04, 0.02),
                    ambient_intensity: 0.1,
                    fog_color: Vec3::new(0.06, 0.03, 0.02),  // Orange-brown
                    fog_density: 0.5,
                    exposure: 1.0,
                    color_tint: Vec3::new(1.0, 0.9, 0.85),
                    saturation: 1.15,
                    contrast: 1.05,
                },
                // Night (0.5) - Orange moon at zenith
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
                // Second Transition (0.7) - Orange setting, purple rising
                ColorKeyframe {
                    time: 0.7,
                    ambient_color: Vec3::new(0.12, 0.04, 0.1),  // Mixed purple-orange
                    ambient_intensity: 0.12,
                    fog_color: Vec3::new(0.15, 0.05, 0.1),
                    fog_density: 0.65,
                    exposure: 1.05,
                    color_tint: Vec3::new(0.95, 0.85, 1.0),  // Cool shift
                    saturation: 1.25,
                    contrast: 0.98,
                },
                // Late Night (0.85) - Purple moon at zenith
                ColorKeyframe {
                    time: 0.85,
                    ambient_color: Vec3::new(0.02, 0.01, 0.05),  // Deep purple
                    ambient_intensity: 0.05,
                    fog_color: Vec3::new(0.02, 0.01, 0.04),
                    fog_density: 0.6,
                    exposure: 1.0,
                    color_tint: Vec3::new(0.95, 0.9, 1.0),  // Subtle purple tint
                    saturation: 1.1,
                    contrast: 1.1,
                },
                // Wrap back to start (1.0 = 0.0)
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
    
    /// Simple day/night cycle (for debugging)
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
                    exposure: 1.0,
                    color_tint: Vec3::ONE,
                    saturation: 1.0,
                    contrast: 1.0,
                },
                ColorKeyframe {
                    time: 0.5,
                    ambient_color: Vec3::new(0.1, 0.05, 0.02),
                    ambient_intensity: 0.1,
                    fog_color: Vec3::new(0.08, 0.04, 0.02),
                    fog_density: 0.4,
                    exposure: 1.1,
                    color_tint: Vec3::new(1.0, 0.9, 0.8),
                    saturation: 1.2,
                    contrast: 1.0,
                },
                ColorKeyframe {
                    time: 1.0,
                    ambient_color: Vec3::new(0.02, 0.01, 0.03),
                    ambient_intensity: 0.05,
                    fog_color: Vec3::new(0.02, 0.01, 0.03),
                    fog_density: 0.6,
                    exposure: 1.0,
                    color_tint: Vec3::ONE,
                    saturation: 1.0,
                    contrast: 1.0,
                },
            ],
        }
    }
}
```

## Moon Position Calculation

```rust
impl MoonCycleConfig {
    /// Calculate moon direction at a given cycle time
    /// 
    /// Returns (direction, height) where:
    /// - direction: normalized Vec3 pointing TO the moon
    /// - height: -1 to 1 (negative = below horizon)
    pub fn calculate_position(&self, cycle_time: f32) -> (Vec3, f32) {
        // Adjust time by period and phase offset
        let moon_time = (cycle_time / self.period + self.phase_offset).fract();
        
        // Convert to radians for orbital position
        let angle = moon_time * std::f32::consts::TAU;
        
        // Calculate position on inclined orbit
        let incline_rad = self.inclination.to_radians();
        
        // Orbital position (x-z plane rotated by inclination around x-axis)
        let x = angle.cos();
        let y_base = angle.sin();
        let y = y_base * incline_rad.cos();
        let z = y_base * incline_rad.sin();
        
        // Height is the y component (positive = above horizon)
        let height = y;
        
        // Direction FROM moon TO scene (light direction)
        let direction = Vec3::new(-x, -y, -z).normalize();
        
        (direction, height)
    }
    
    /// Calculate moon color at given height
    pub fn calculate_color(&self, height: f32) -> Vec3 {
        // Interpolate between horizon and zenith colors based on height
        let t = ((height + 1.0) / 2.0).clamp(0.0, 1.0);
        self.horizon_color.lerp(self.zenith_color, t)
    }
    
    /// Calculate moon intensity at given height
    pub fn calculate_intensity(&self, height: f32) -> f32 {
        if height < self.set_height {
            return 0.0;  // Moon has set
        }
        
        // Smooth fade near horizon
        let fade = ((height - self.set_height) / (0.3 - self.set_height)).clamp(0.0, 1.0);
        let base_intensity = self.horizon_intensity.lerp(&self.zenith_intensity, 
            ((height + 1.0) / 2.0).clamp(0.0, 1.0));
        
        base_intensity * fade
    }
}
```

## LUT Interpolation

```rust
impl ColorLutConfig {
    /// Sample the LUT at a given time, interpolating between keyframes
    pub fn sample(&self, time: f32) -> ColorKeyframe {
        let time = time.fract();  // Wrap to 0-1
        
        // Find surrounding keyframes
        let mut prev_idx = 0;
        let mut next_idx = 0;
        
        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.time <= time {
                prev_idx = i;
            }
            if kf.time >= time && next_idx == 0 {
                next_idx = i;
                break;
            }
        }
        
        // Handle wraparound
        if next_idx == 0 {
            next_idx = 0;  // Wrap to first keyframe
        }
        
        let prev = &self.keyframes[prev_idx];
        let next = &self.keyframes[next_idx];
        
        // Calculate interpolation factor
        let span = if next.time >= prev.time {
            next.time - prev.time
        } else {
            (1.0 - prev.time) + next.time  // Wraparound
        };
        
        let t = if span > 0.0 {
            let elapsed = if time >= prev.time {
                time - prev.time
            } else {
                (1.0 - prev.time) + time
            };
            elapsed / span
        } else {
            0.0
        };
        
        // Interpolate based on mode
        let t = match self.interpolation {
            InterpolationMode::Linear => t,
            InterpolationMode::CatmullRom => smoothstep(t),
            InterpolationMode::Step => if t < 0.5 { 0.0 } else { 1.0 },
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

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
```

## Main DayNightCycle Resource

```rust
/// Main day/night cycle resource
#[derive(Resource)]
pub struct DayNightCycle {
    /// Current time in cycle (0.0 - 1.0)
    pub time: f32,
    
    /// Whether the cycle is paused
    pub paused: bool,
    
    /// Speed multiplier (1.0 = one cycle per real-time day, higher = faster)
    pub speed: f32,
    
    /// Configuration for moon 1 (purple)
    pub moon1_config: MoonCycleConfig,
    
    /// Configuration for moon 2 (orange)
    pub moon2_config: MoonCycleConfig,
    
    /// Color grading LUT
    pub color_lut: ColorLutConfig,
    
    // Cached computed values (updated by system)
    
    /// Current moon 1 state
    pub moon1_direction: Vec3,
    pub moon1_color: Vec3,
    pub moon1_intensity: f32,
    
    /// Current moon 2 state
    pub moon2_direction: Vec3,
    pub moon2_color: Vec3,
    pub moon2_intensity: f32,
    
    /// Current ambient light
    pub ambient_color: Vec3,
    pub ambient_intensity: f32,
    
    /// Current fog settings
    pub fog_color: Vec3,
    pub fog_density: f32,
    
    /// Current color grading
    pub exposure: f32,
    pub color_tint: Vec3,
    pub saturation: f32,
    pub contrast: f32,
}

impl Default for DayNightCycle {
    fn default() -> Self {
        Self::dark_world()
    }
}

impl DayNightCycle {
    /// Dark fantasy preset
    pub fn dark_world() -> Self {
        Self {
            time: 0.0,
            paused: false,
            speed: 0.01,  // Very slow - about 100 seconds per cycle
            
            moon1_config: MoonCycleConfig::purple_moon(),
            moon2_config: MoonCycleConfig::orange_moon(),
            color_lut: ColorLutConfig::dark_world(),
            
            // Initialize cached values (will be updated by system)
            moon1_direction: Vec3::NEG_Y,
            moon1_color: Vec3::ONE,
            moon1_intensity: 0.5,
            moon2_direction: Vec3::NEG_Y,
            moon2_color: Vec3::ONE,
            moon2_intensity: 0.5,
            ambient_color: Vec3::ZERO,
            ambient_intensity: 0.05,
            fog_color: Vec3::ZERO,
            fog_density: 0.5,
            exposure: 1.0,
            color_tint: Vec3::ONE,
            saturation: 1.0,
            contrast: 1.0,
        }
    }
    
    /// Set time directly (for screenshots)
    pub fn set_time(&mut self, time: f32) {
        self.time = time.fract();
    }
    
    /// Update the cycle (called by system)
    pub fn update(&mut self, delta_seconds: f32) {
        if !self.paused {
            self.time = (self.time + delta_seconds * self.speed).fract();
        }
        
        // Update moon positions
        let (dir1, height1) = self.moon1_config.calculate_position(self.time);
        self.moon1_direction = dir1;
        self.moon1_color = self.moon1_config.calculate_color(height1);
        self.moon1_intensity = self.moon1_config.calculate_intensity(height1);
        
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
```

## Systems

```rust
/// Update the day/night cycle
pub fn update_day_night_cycle(
    time: Res<Time>,
    mut cycle: ResMut<DayNightCycle>,
) {
    cycle.update(time.delta_secs());
}

/// Apply cycle state to MoonConfig for rendering
pub fn apply_cycle_to_moon_config(
    cycle: Res<DayNightCycle>,
    mut moon_config: ResMut<MoonConfig>,
) {
    if !cycle.is_changed() {
        return;
    }
    
    moon_config.moon1_direction = cycle.moon1_direction;
    moon_config.moon1_color = cycle.moon1_color;
    moon_config.moon1_intensity = cycle.moon1_intensity;
    
    moon_config.moon2_direction = cycle.moon2_direction;
    moon_config.moon2_color = cycle.moon2_color;
    moon_config.moon2_intensity = cycle.moon2_intensity;
}

/// Apply cycle state to DeferredLightingConfig
pub fn apply_cycle_to_lighting(
    cycle: Res<DayNightCycle>,
    mut lighting: ResMut<DeferredLightingConfig>,
) {
    if !cycle.is_changed() {
        return;
    }
    
    lighting.ambient_color = Color::rgb(
        cycle.ambient_color.x,
        cycle.ambient_color.y,
        cycle.ambient_color.z,
    );
    lighting.ambient_intensity = cycle.ambient_intensity;
    lighting.fog_color = Color::rgb(
        cycle.fog_color.x,
        cycle.fog_color.y,
        cycle.fog_color.z,
    );
}
```

## Shader Uniforms Extension

Add to `DirectionalShadowUniforms` or create new `ColorGradingUniforms`:

```rust
/// Color grading uniforms for post-processing
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorGradingUniforms {
    /// Exposure multiplier
    pub exposure: f32,
    /// Saturation multiplier
    pub saturation: f32,
    /// Contrast multiplier
    pub contrast: f32,
    /// Padding
    pub _pad0: f32,
    /// Color tint (RGB) + unused (A)
    pub color_tint: [f32; 4],
}
```

Apply in `bloom_composite.wgsl`:

```wgsl
// Before tone mapping
color *= grading.exposure;

// Saturation adjustment
let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
color = mix(vec3<f32>(luminance), color, grading.saturation);

// Contrast adjustment (around mid-gray)
color = (color - 0.5) * grading.contrast + 0.5;

// Color tint
color *= grading.color_tint.rgb;
```

## Screenshot Sequence Capture

```rust
/// Configuration for capturing a screenshot sequence
#[derive(Resource)]
pub struct ScreenshotSequence {
    /// Output directory
    pub output_dir: String,
    
    /// Number of frames to capture
    pub frame_count: usize,
    
    /// Times to capture (0.0 - 1.0)
    /// If empty, captures at evenly spaced intervals
    pub capture_times: Vec<f32>,
    
    /// Current capture index
    pub current_frame: usize,
    
    /// Whether sequence is active
    pub active: bool,
    
    /// Frames to wait between captures (for GPU to settle)
    pub settle_frames: u32,
    pub settle_counter: u32,
}

impl ScreenshotSequence {
    /// Create a new sequence with evenly spaced captures
    pub fn evenly_spaced(output_dir: &str, count: usize) -> Self {
        let times: Vec<f32> = (0..count)
            .map(|i| i as f32 / count as f32)
            .collect();
        
        Self {
            output_dir: output_dir.to_string(),
            frame_count: count,
            capture_times: times,
            current_frame: 0,
            active: true,
            settle_frames: 2,
            settle_counter: 0,
        }
    }
    
    /// Create a sequence capturing specific times of day
    pub fn at_times(output_dir: &str, times: Vec<f32>) -> Self {
        Self {
            output_dir: output_dir.to_string(),
            frame_count: times.len(),
            capture_times: times,
            current_frame: 0,
            active: true,
            settle_frames: 2,
            settle_counter: 0,
        }
    }
    
    /// Key times for dark world cycle
    pub fn dark_world_highlights(output_dir: &str) -> Self {
        Self::at_times(output_dir, vec![
            0.0,   // Deep night (both moons?)
            0.15,  // Purple moon high
            0.3,   // Dawn transition
            0.45,  // Orange moon rising
            0.5,   // Orange moon high
            0.65,  // Dusk transition
            0.85,  // Purple moon rising
            0.95,  // Pre-midnight
        ])
    }
}

/// System to capture screenshot sequence
pub fn capture_screenshot_sequence(
    mut sequence: ResMut<ScreenshotSequence>,
    mut cycle: ResMut<DayNightCycle>,
    // ... screenshot capture resources
) {
    if !sequence.active {
        return;
    }
    
    if sequence.current_frame >= sequence.frame_count {
        sequence.active = false;
        info!("Screenshot sequence complete: {} frames", sequence.frame_count);
        return;
    }
    
    // Wait for settle frames
    if sequence.settle_counter < sequence.settle_frames {
        sequence.settle_counter += 1;
        return;
    }
    
    // Get target time
    let target_time = sequence.capture_times[sequence.current_frame];
    
    // Set cycle time and pause
    cycle.set_time(target_time);
    cycle.paused = true;
    
    // Capture screenshot
    let filename = format!(
        "{}/cycle_{:03}_{:.2}.png",
        sequence.output_dir,
        sequence.current_frame,
        target_time
    );
    
    // Trigger screenshot capture...
    
    sequence.current_frame += 1;
    sequence.settle_counter = 0;
}
```

## Files to Create/Modify

### New Files

| File | Purpose |
|------|---------|
| `crates/studio_core/src/day_night.rs` | DayNightCycle, MoonCycleConfig, ColorLutConfig |
| `crates/studio_core/src/screenshot_sequence.rs` | ScreenshotSequence resource and system |
| `examples/p21_day_night_cycle.rs` | Day/night cycle demo with sequence capture |

### Modified Files

| File | Changes |
|------|---------|
| `crates/studio_core/src/lib.rs` | Export new modules |
| `crates/studio_core/src/deferred/shadow.rs` | Add color grading uniforms |
| `crates/studio_core/src/deferred/lighting_node.rs` | Pass color grading to shader |
| `assets/shaders/deferred_lighting.wgsl` | Use dynamic ambient/fog from uniforms |
| `assets/shaders/bloom_composite.wgsl` | Add exposure/saturation/contrast/tint |

## Example Usage

```rust
// examples/p21_day_night_cycle.rs

use bevy::prelude::*;
use studio_core::{
    DayNightCycle, MoonCycleConfig, ColorLutConfig,
    ScreenshotSequence, VoxelWorldApp, WorldSource,
};

fn main() {
    VoxelWorldApp::new("Phase 21: Day/Night Cycle")
        .with_world(WorldSource::File("assets/worlds/island.voxworld"))
        .with_day_night_cycle(DayNightCycle {
            speed: 0.1,  // Fast for demo (10 seconds per cycle)
            moon1_config: MoonCycleConfig::purple_moon(),
            moon2_config: MoonCycleConfig::orange_moon(),
            color_lut: ColorLutConfig::dark_world(),
            ..default()
        })
        .with_screenshot_sequence(
            ScreenshotSequence::evenly_spaced("screenshots/day_night_cycle", 24)
        )
        .with_camera_position(
            Vec3::new(30.0, 25.0, 30.0),
            Vec3::new(8.0, 4.0, 8.0),
        )
        .run();
}
```

## Verification

### Test Command
```bash
cargo run --example p21_day_night_cycle
```

### Expected Output
```
screenshots/day_night_cycle/
  cycle_000_0.00.png   # Deep night
  cycle_001_0.04.png   
  cycle_002_0.08.png
  ...
  cycle_023_0.96.png
```

### Pass Criteria

1. **Moon Movement**: Moons visibly change position through the cycle
2. **Color Transitions**: Smooth color changes between keyframes (no harsh jumps)
3. **Independent Moons**: Purple and orange moons have different positions/intensities at any given time
4. **Fog Color**: Fog color changes with time of day
5. **Shadows Update**: Shadow directions track moon positions
6. **Screenshot Sequence**: All frames captured to output directory
7. **Named Times**: Key times (dawn, dusk, midnight) show expected aesthetics

## Implementation Order

1. **Core types** (`day_night.rs`):
   - `MoonCycleConfig` with position/color/intensity calculation
   - `ColorKeyframe` and `ColorLutConfig` with interpolation
   - `DayNightCycle` resource with update logic

2. **Systems**:
   - `update_day_night_cycle` - advances time
   - `apply_cycle_to_moon_config` - syncs to MoonConfig
   - `apply_cycle_to_lighting` - syncs ambient/fog

3. **Shader updates**:
   - Make ambient/fog uniforms instead of constants
   - Add color grading uniforms to bloom composite

4. **Screenshot sequence** (`screenshot_sequence.rs`):
   - `ScreenshotSequence` resource
   - `capture_screenshot_sequence` system

5. **Example and testing**:
   - `p21_day_night_cycle.rs` example
   - Verify smooth transitions
   - Capture sequence and review

## Timeline Estimate

| Task | Estimate |
|------|----------|
| Core types and MoonCycleConfig | 2 hours |
| ColorLutConfig with interpolation | 1.5 hours |
| DayNightCycle resource | 1 hour |
| Systems (update, apply) | 1 hour |
| Shader uniform updates | 1.5 hours |
| Color grading in bloom_composite | 1 hour |
| Screenshot sequence system | 1.5 hours |
| Example and testing | 1 hour |
| **Total** | **~10.5 hours** |

---

## Notes

### Why LUT over Procedural?

LUTs (keyframe tables) offer several advantages:
1. **Artist-friendly**: Can define exact colors at exact times
2. **Non-physical**: Dark fantasy isn't realistic - we want specific moods
3. **Predictable**: Same time = same look, always
4. **Debuggable**: Can visualize the LUT as a timeline

### Why Catmull-Rom Interpolation?

- Linear interpolation can look "robotic" with sudden changes in derivative
- Catmull-Rom splines pass through all control points with smooth tangents
- Better for color transitions that should feel natural

### Performance Considerations

- Moon position calculation: ~10 trig operations per frame (negligible)
- LUT sampling: Linear search through keyframes (max ~10)
- No per-pixel shader work changes
- Screenshot capture: GPU texture readback is the bottleneck

### Future Extensions

- **LUT texture**: Store gradient as 1D texture for GPU sampling
- **Per-channel curves**: Separate R/G/B LUT curves
- **Weather integration**: Fog/cloud parameters in keyframes
- **Audio reactivity**: Pulse colors with music
