//! VoxelWorldPlugin - A unified plugin for setting up and running voxel world examples/tests.
//!
//! This plugin eliminates boilerplate across examples by providing:
//! - Configurable window setup
//! - World loading (from file, lua script, or builder function)
//! - Scene spawning with camera, lights
//! - Screenshot capture and auto-exit for testing
//! - Support for both forward and deferred rendering
//! - HDR and Bloom camera options
//!
//! # Example - Minimal Test
//!
//! ```ignore
//! use studio_core::{VoxelWorldApp, WorldSource};
//!
//! fn main() {
//!     VoxelWorldApp::new("Test Scene")
//!         .with_world(WorldSource::File("worlds/island.voxworld"))
//!         .with_screenshot("screenshots/test.png")
//!         .run();
//! }
//! ```
//!
//! # Example - Custom World Builder
//!
//! ```ignore
//! use studio_core::{VoxelWorldApp, WorldSource, Voxel};
//!
//! fn main() {
//!     VoxelWorldApp::new("Custom Scene")
//!         .with_world(WorldSource::Builder(Box::new(|world| {
//!             for x in 0..10 {
//!                 world.set_voxel(x, 0, 0, Voxel::solid(255, 0, 0));
//!             }
//!         })))
//!         .run();
//! }
//! ```

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::render::view::Hdr;
use bevy::window::WindowPlugin;
use std::path::Path;

use crate::day_night::{DayNightCycle, DayNightCyclePlugin};
use crate::deferred::{DeferredCamera, DeferredRenderingPlugin, MoonConfig};
use crate::scene_utils::{spawn_world_with_lights_config, CameraPreset, WorldSpawnConfig};
use crate::screenshot_sequence::{ScreenshotSequence, ScreenshotSequencePlugin};
use crate::voxel::VoxelWorld;
use crate::voxel_mesh::VoxelMaterialPlugin;
use crate::world_io::load_world;
use crate::creature_script::load_creature_script;

/// Source of voxel world data.
pub enum WorldSource {
    /// Load from a .voxworld file
    File(String),
    /// Load from a Lua script
    LuaScript(String),
    /// Build programmatically
    Builder(Box<dyn FnOnce(&mut VoxelWorld) + Send + Sync>),
    /// Pre-built world
    World(VoxelWorld),
    /// Empty world (user will populate via systems)
    Empty,
}

/// Bloom configuration for HDR rendering.
#[derive(Clone)]
pub struct BloomConfig {
    pub intensity: f32,
    pub low_frequency_boost: f32,
    pub threshold: f32,
    pub threshold_softness: f32,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            low_frequency_boost: 0.7,
            threshold: 1.0,
            threshold_softness: 0.5,
        }
    }
}

/// Camera configuration for the scene.
#[derive(Clone)]
pub enum CameraConfig {
    /// Automatically frame the world bounds
    /// - `angle`: Horizontal angle in degrees (0 = +X, 90 = +Z)
    /// - `elevation`: Vertical angle above horizon in degrees
    /// - `zoom`: Zoom multiplier (1.0 = tight fit, <1.0 = zoomed in, >1.0 = zoomed out)
    AutoFrame { angle: f32, elevation: f32, zoom: f32 },
    /// Use a camera preset
    Preset(CameraPreset),
    /// Custom position and target
    Custom { position: Vec3, look_at: Vec3 },
    /// No camera (user will spawn their own)
    None,
}

impl Default for CameraConfig {
    fn default() -> Self {
        CameraConfig::AutoFrame {
            angle: 45.0,
            elevation: 30.0,
            zoom: 1.0,
        }
    }
}

/// Screenshot configuration.
#[derive(Clone)]
pub struct ScreenshotConfig {
    /// Path to save screenshot
    pub path: String,
    /// Frame to capture on
    pub capture_frame: u32,
    /// Frame to exit on (after capture)
    pub exit_frame: u32,
    /// Verify file exists after run
    pub verify: bool,
}

impl ScreenshotConfig {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            capture_frame: 15,
            exit_frame: 25,
            verify: true,
        }
    }

    pub fn with_timing(mut self, capture: u32, exit: u32) -> Self {
        self.capture_frame = capture;
        self.exit_frame = exit;
        self
    }
}

use crate::debug_screenshot::{DebugModes, DebugScreenshotConfig, DebugScreenshotPlugin, DebugScreenshotState};

/// Configuration for the VoxelWorldApp.
#[derive(Default)]
pub struct VoxelWorldConfig {
    /// Window title
    pub title: String,
    /// Window resolution
    pub resolution: (u32, u32),
    /// Clear color (background)
    pub clear_color: Color,
    /// Enable deferred rendering pipeline
    pub use_deferred: bool,
    /// Enable greedy meshing
    pub use_greedy_meshing: bool,
    /// Enable cross-chunk face culling
    pub use_cross_chunk_culling: bool,
    /// Camera configuration
    pub camera: CameraConfig,
    /// Screenshot configuration (if any)
    pub screenshot: Option<ScreenshotConfig>,
    /// Debug screenshot configuration (multi-capture with debug modes)
    pub debug_screenshots: Option<DebugScreenshotConfig>,
    /// Spawn point lights from emissive voxels
    pub spawn_emissive_lights: bool,
    /// Add a shadow-casting light
    pub shadow_light: Option<Vec3>,
    /// Enable HDR rendering
    pub use_hdr: bool,
    /// Bloom configuration (requires HDR)
    pub bloom: Option<BloomConfig>,
    /// Moon configuration for dual moon lighting
    pub moon_config: Option<MoonConfig>,
    /// Day/night cycle configuration
    pub day_night_cycle: Option<DayNightCycle>,
    /// Screenshot sequence configuration
    pub screenshot_sequence: Option<ScreenshotSequence>,
    /// Interactive mode - disables auto-exit, enables benchmark
    pub interactive: bool,
}

/// Stored update systems for interactive mode.
type UpdateSystemFn = Box<dyn FnOnce(&mut App) + Send + Sync>;

/// Builder for creating a VoxelWorld app with minimal boilerplate.
pub struct VoxelWorldApp {
    config: VoxelWorldConfig,
    world_source: WorldSource,
    setup_callback: Option<Box<dyn FnOnce(&mut Commands, &VoxelWorld) + Send + Sync>>,
    /// Whether deferred bloom is enabled (default: true)
    deferred_bloom_enabled: bool,
    /// Custom update systems to add (for interactive mode)
    update_systems: Vec<UpdateSystemFn>,
    /// Custom plugins to add
    custom_plugins: Vec<Box<dyn FnOnce(&mut App) + Send + Sync>>,
}

impl VoxelWorldApp {
    /// Create a new VoxelWorldApp with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            config: VoxelWorldConfig {
                title: title.into(),
                resolution: (800, 600),
                clear_color: Color::srgb(0.102, 0.039, 0.180), // Purple fog
                use_deferred: true,
                use_greedy_meshing: true,
                use_cross_chunk_culling: true,
                camera: CameraConfig::default(),
                screenshot: None,
                debug_screenshots: None,
                spawn_emissive_lights: true,
                shadow_light: None,
                use_hdr: false,
                bloom: None,
                moon_config: None,
                day_night_cycle: None,
                screenshot_sequence: None,
                interactive: false,
            },
            world_source: WorldSource::Empty,
            setup_callback: None,
            deferred_bloom_enabled: true,
            update_systems: Vec::new(),
            custom_plugins: Vec::new(),
        }
    }

    /// Set the world source.
    pub fn with_world(mut self, source: WorldSource) -> Self {
        self.world_source = source;
        self
    }

    /// Load world from a file.
    pub fn with_world_file(self, path: impl Into<String>) -> Self {
        self.with_world(WorldSource::File(path.into()))
    }

    /// Load world from a Lua script.
    pub fn with_lua_script(self, path: impl Into<String>) -> Self {
        self.with_world(WorldSource::LuaScript(path.into()))
    }

    /// Build world programmatically.
    pub fn with_world_builder<F>(self, builder: F) -> Self
    where
        F: FnOnce(&mut VoxelWorld) + Send + Sync + 'static,
    {
        self.with_world(WorldSource::Builder(Box::new(builder)))
    }

    /// Set window resolution.
    pub fn with_resolution(mut self, width: u32, height: u32) -> Self {
        self.config.resolution = (width, height);
        self
    }

    /// Set clear color.
    pub fn with_clear_color(mut self, color: Color) -> Self {
        self.config.clear_color = color;
        self
    }

    /// Configure screenshot capture.
    pub fn with_screenshot(mut self, path: impl Into<String>) -> Self {
        self.config.screenshot = Some(ScreenshotConfig::new(path));
        self
    }

    /// Configure screenshot with custom timing.
    pub fn with_screenshot_timed(
        mut self,
        path: impl Into<String>,
        capture_frame: u32,
        exit_frame: u32,
    ) -> Self {
        self.config.screenshot = Some(
            ScreenshotConfig::new(path)
                .with_timing(capture_frame, exit_frame),
        );
        self
    }

    /// Set camera configuration.
    pub fn with_camera(mut self, camera: CameraConfig) -> Self {
        self.config.camera = camera;
        self
    }

    /// Position camera with auto-framing.
    pub fn with_camera_angle(mut self, angle: f32, elevation: f32) -> Self {
        self.config.camera = CameraConfig::AutoFrame { angle, elevation, zoom: 1.0 };
        self
    }

    /// Set zoom level for auto-framing camera.
    /// - `zoom < 1.0`: Zoom in (closer to subject)
    /// - `zoom = 1.0`: Default tight framing
    /// - `zoom > 1.0`: Zoom out (further from subject)
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        if let CameraConfig::AutoFrame { angle, elevation, .. } = self.config.camera {
            self.config.camera = CameraConfig::AutoFrame { angle, elevation, zoom };
        }
        self
    }

    /// Position camera with preset.
    pub fn with_camera_preset(mut self, preset: CameraPreset) -> Self {
        self.config.camera = CameraConfig::Preset(preset);
        self
    }

    /// Position camera explicitly.
    pub fn with_camera_position(mut self, position: Vec3, look_at: Vec3) -> Self {
        self.config.camera = CameraConfig::Custom { position, look_at };
        self
    }

    /// Disable automatic camera spawning.
    pub fn without_camera(mut self) -> Self {
        self.config.camera = CameraConfig::None;
        self
    }

    /// Enable/disable deferred rendering.
    pub fn with_deferred(mut self, enabled: bool) -> Self {
        self.config.use_deferred = enabled;
        self
    }

    /// Enable/disable greedy meshing.
    pub fn with_greedy_meshing(mut self, enabled: bool) -> Self {
        self.config.use_greedy_meshing = enabled;
        self
    }

    /// Enable/disable emissive light spawning.
    pub fn with_emissive_lights(mut self, enabled: bool) -> Self {
        self.config.spawn_emissive_lights = enabled;
        self
    }

    /// Add a shadow-casting light at position.
    pub fn with_shadow_light(mut self, position: Vec3) -> Self {
        self.config.shadow_light = Some(position);
        self
    }

    /// Enable HDR rendering.
    pub fn with_hdr(mut self, enabled: bool) -> Self {
        self.config.use_hdr = enabled;
        self
    }

    /// Enable bloom with default settings (requires HDR).
    pub fn with_bloom(mut self) -> Self {
        self.config.use_hdr = true;
        self.config.bloom = Some(BloomConfig::default());
        self
    }

    /// Enable bloom with custom settings (requires HDR).
    pub fn with_bloom_config(mut self, config: BloomConfig) -> Self {
        self.config.use_hdr = true;
        self.config.bloom = Some(config);
        self
    }

    /// Disable deferred bloom (useful for clean debug output).
    pub fn without_deferred_bloom(self) -> Self {
        self.with_deferred_bloom_enabled(false)
    }

    /// Enable/disable deferred bloom.
    pub fn with_deferred_bloom_enabled(mut self, enabled: bool) -> Self {
        self.deferred_bloom_enabled = enabled;
        self
    }

    /// Enable/disable cross-chunk face culling.
    pub fn with_cross_chunk_culling(mut self, enabled: bool) -> Self {
        self.config.use_cross_chunk_culling = enabled;
        self
    }

    /// Set custom moon configuration for dual moon lighting.
    pub fn with_moon_config(mut self, config: MoonConfig) -> Self {
        self.config.moon_config = Some(config);
        self
    }

    /// Configure debug screenshots (multi-capture with debug modes).
    /// This replaces the simple screenshot configuration.
    pub fn with_debug_screenshots(mut self, config: DebugScreenshotConfig) -> Self {
        self.config.debug_screenshots = Some(config);
        // Clear simple screenshot config when using debug screenshots
        self.config.screenshot = None;
        self
    }

    /// Enable day/night cycle with the given configuration.
    ///
    /// This enables automatic moon position/color cycling over time.
    /// The cycle updates each frame and syncs to MoonConfig.
    pub fn with_day_night_cycle(mut self, cycle: DayNightCycle) -> Self {
        self.config.day_night_cycle = Some(cycle);
        self
    }

    /// Enable screenshot sequence capture.
    ///
    /// This will capture screenshots at specific times in the day/night cycle.
    /// The app will auto-exit when the sequence is complete.
    ///
    /// NOTE: This requires a day/night cycle to be configured.
    pub fn with_screenshot_sequence(mut self, sequence: ScreenshotSequence) -> Self {
        self.config.screenshot_sequence = Some(sequence);
        // Disable simple screenshot when using sequence
        self.config.screenshot = None;
        self
    }

    /// Enable interactive mode (no auto-exit, adds benchmark FPS display).
    ///
    /// Use this for examples where the user controls the camera or player.
    pub fn with_interactive(mut self) -> Self {
        self.config.interactive = true;
        self.config.screenshot = None; // No screenshots in interactive mode
        self
    }

    /// Add custom update systems for interactive mode.
    ///
    /// The closure receives the App and can add any systems needed.
    ///
    /// # Example
    /// ```ignore
    /// .with_update_systems(|app| {
    ///     app.add_systems(Update, (player_input, physics_step).chain());
    /// })
    /// ```
    pub fn with_update_systems<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut App) + Send + Sync + 'static,
    {
        self.update_systems.push(Box::new(f));
        self
    }

    /// Add a custom plugin.
    ///
    /// # Example
    /// ```ignore
    /// .with_plugin(|app| {
    ///     app.add_plugins(MyCustomPlugin);
    /// })
    /// ```
    pub fn with_plugin<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut App) + Send + Sync + 'static,
    {
        self.custom_plugins.push(Box::new(f));
        self
    }

    /// Insert a resource into the app.
    ///
    /// # Example
    /// ```ignore
    /// .with_resource(MyConfig::default())
    /// ```
    pub fn with_resource<R: Resource>(mut self, resource: R) -> Self {
        self.custom_plugins.push(Box::new(move |app: &mut App| {
            app.insert_resource(resource);
        }));
        self
    }

    /// Add a custom setup callback.
    pub fn with_setup<F>(mut self, callback: F) -> Self
    where
        F: FnOnce(&mut Commands, &VoxelWorld) + Send + Sync + 'static,
    {
        self.setup_callback = Some(Box::new(callback));
        self
    }

    /// Run the application.
    pub fn run(self) {
        // Ensure screenshots directory exists if needed
        if let Some(ref ss) = self.config.screenshot {
            if let Some(parent) = Path::new(&ss.path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        let title = self.config.title.clone();
        let resolution = self.config.resolution;
        let clear_color = self.config.clear_color;
        let use_deferred = self.config.use_deferred;
        let interactive = self.config.interactive;
        let screenshot_path = self.config.screenshot.as_ref().map(|s| s.path.clone());

        let mut app = App::new();

        // Core plugins
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: resolution.into(),
                title: title.clone(),
                ..default()
            }),
            ..default()
        }));

        // Voxel material plugin (always needed)
        app.add_plugins(VoxelMaterialPlugin);

        // Deferred rendering plugin (optional)
        if use_deferred {
            app.add_plugins(DeferredRenderingPlugin);
        }

        // Benchmark plugin for interactive mode
        if interactive {
            use crate::benchmark::BenchmarkPlugin;
            app.add_plugins(BenchmarkPlugin);
        }

        // Override moon config if specified (must happen after DeferredRenderingPlugin)
        if let Some(moon_config) = self.config.moon_config.clone() {
            app.insert_resource(moon_config);
        }

        // Add day/night cycle if configured
        if let Some(day_night_cycle) = self.config.day_night_cycle.clone() {
            app.add_plugins(DayNightCyclePlugin);
            app.insert_resource(day_night_cycle);
        }

        // Add screenshot sequence if configured
        if let Some(screenshot_sequence) = self.config.screenshot_sequence.clone() {
            app.add_plugins(ScreenshotSequencePlugin);
            app.insert_resource(screenshot_sequence);
        }

        // Override deferred bloom config if disabled (must happen after DeferredRenderingPlugin)
        if use_deferred && !self.deferred_bloom_enabled {
            use crate::deferred::BloomConfig as DeferredBloomConfig;
            app.insert_resource(DeferredBloomConfig::disabled());
        }

        // Debug screenshot plugin (if configured)
        let has_debug_screenshots = self.config.debug_screenshots.is_some();
        if let Some(debug_config) = self.config.debug_screenshots.clone() {
            app.add_plugins(DebugScreenshotPlugin);
            app.insert_resource(DebugScreenshotState::new(debug_config));
        } else {
            // Initialize default debug modes even without debug screenshots
            app.init_resource::<DebugModes>();
        }

        // Add custom plugins
        for plugin_fn in self.custom_plugins {
            plugin_fn(&mut app);
        }

        // Resources
        app.insert_resource(ClearColor(clear_color));
        app.insert_resource(VoxelWorldAppConfig(self.config));
        app.insert_resource(VoxelWorldSource(self.world_source));
        
        if let Some(callback) = self.setup_callback {
            app.insert_resource(SetupCallback(Some(callback)));
        } else {
            app.insert_resource(SetupCallback(None));
        }

        // Frame counter for screenshot timing
        app.insert_resource(FrameCounter(0));

        // Systems
        app.add_systems(Startup, setup_world);
        
        // Only add screenshot/exit system if NOT in interactive mode and not using debug screenshots
        if !interactive && !has_debug_screenshots {
            app.add_systems(Update, screenshot_and_exit);
        }

        // Add custom update systems
        for system_fn in self.update_systems {
            system_fn(&mut app);
        }

        // Run
        app.run();

        // Verify screenshot if requested (only in non-interactive mode)
        if !interactive {
            if let Some(ref ss_config) = screenshot_path {
                if Path::new(ss_config).exists() {
                    println!("SUCCESS: Screenshot saved to {}", ss_config);
                } else {
                    println!("FAILED: Screenshot was not created at {}", ss_config);
                    std::process::exit(1);
                }
            }
        }
    }
}

// Internal resources

#[derive(Resource)]
struct VoxelWorldAppConfig(VoxelWorldConfig);

#[derive(Resource)]
struct VoxelWorldSource(WorldSource);

#[derive(Resource)]
struct SetupCallback(Option<Box<dyn FnOnce(&mut Commands, &VoxelWorld) + Send + Sync>>);

#[derive(Resource)]
struct FrameCounter(u32);

#[derive(Resource)]
#[allow(dead_code)]
struct LoadedWorld(VoxelWorld);

// Systems

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::voxel_mesh::VoxelMaterial>>,
    config: Res<VoxelWorldAppConfig>,
    mut world_source: ResMut<VoxelWorldSource>,
    mut setup_callback: ResMut<SetupCallback>,
) {
    let config = &config.0;

    // Load world from source
    let world = match std::mem::replace(&mut world_source.0, WorldSource::Empty) {
        WorldSource::File(path) => {
            println!("Loading world from: {}", path);
            match load_world(&path) {
                Ok(w) => {
                    println!("Loaded {} chunks, {} voxels", w.chunk_count(), w.total_voxel_count());
                    w
                }
                Err(e) => {
                    eprintln!("ERROR: Failed to load world: {}", e);
                    std::process::exit(1);
                }
            }
        }
        WorldSource::LuaScript(path) => {
            println!("Loading Lua script: {}", path);
            match load_creature_script(&path) {
                Ok(chunk) => {
                    // Convert single chunk to world
                    let mut world = VoxelWorld::new();
                    for (x, y, z, voxel) in chunk.iter() {
                        world.set_voxel(x as i32, y as i32, z as i32, voxel);
                    }
                    println!("Loaded {} voxels from script", world.total_voxel_count());
                    world
                }
                Err(e) => {
                    eprintln!("ERROR: Failed to load script: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        WorldSource::Builder(builder) => {
            let mut world = VoxelWorld::new();
            builder(&mut world);
            println!("Built world: {} chunks, {} voxels", world.chunk_count(), world.total_voxel_count());
            world
        }
        WorldSource::World(world) => world,
        WorldSource::Empty => VoxelWorld::new(),
    };

    // Spawn shadow light first (if configured) with PrimaryShadowCaster marker
    if let Some(light_pos) = config.shadow_light {
        use crate::deferred::{DeferredPointLight, PrimaryShadowCaster};
        commands.spawn((
            DeferredPointLight {
                color: Color::srgb(1.0, 0.9, 0.7),
                intensity: 40.0,
                radius: 25.0,
            },
            Transform::from_translation(light_pos),
            PrimaryShadowCaster, // This light ALWAYS casts shadows
        ));
        println!("Added primary shadow-casting light at {:?}", light_pos);
    }

    // Spawn world meshes and lights
    if world.chunk_count() > 0 {
        let mut spawn_config = WorldSpawnConfig {
            use_greedy_meshing: config.use_greedy_meshing,
            use_cross_chunk_culling: config.use_cross_chunk_culling,
            ..default()
        };

        // Disable lights if not wanted
        if !config.spawn_emissive_lights {
            spawn_config.light_config.min_emission = 255; // Only max emission will spawn lights (essentially disabled)
        }

        let result = spawn_world_with_lights_config(
            &mut commands,
            &mut meshes,
            &mut materials,
            &world,
            &spawn_config,
        );

        println!(
            "Spawned {} chunk meshes + {} lights",
            result.chunk_entities.len(),
            result.light_entities.len()
        );
    }

    // Spawn camera
    let camera_transform = match &config.camera {
        CameraConfig::AutoFrame { angle, elevation, zoom } => {
            // Use actual voxel bounds for tight framing, not chunk bounds
            if let Some((min_world, max_world)) = world.voxel_bounds() {
                // Apply zoom: base padding of 1.3 * zoom factor
                // zoom < 1.0 = closer, zoom > 1.0 = further
                let padding = 1.3 * zoom;
                let framing = crate::scene_utils::compute_camera_framing(
                    min_world,
                    max_world,
                    *angle,
                    *elevation,
                    padding,
                );
                println!("Camera auto-framed at {:?} (bounds: {:?} to {:?}, zoom: {})", 
                    framing.position, min_world, max_world, zoom);
                Some(Transform::from_translation(framing.position).looking_at(framing.look_at, Vec3::Y))
            } else {
                // Empty world, spawn default camera
                Some(Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y))
            }
        }
        CameraConfig::Preset(preset) => {
            Some(Transform::from_translation(preset.position).looking_at(preset.look_at, Vec3::Y))
        }
        CameraConfig::Custom { position, look_at } => {
            Some(Transform::from_translation(*position).looking_at(*look_at, Vec3::Y))
        }
        CameraConfig::None => None,
    };

    if let Some(transform) = camera_transform {
        // Build camera entity based on rendering mode
        if config.use_deferred {
            // Deferred rendering camera
            commands.spawn((
                Camera3d::default(),
                Tonemapping::TonyMcMapface,
                transform,
                DeferredCamera,
            ));
        } else if let Some(ref bloom_config) = config.bloom {
            // Forward rendering with HDR + Bloom
            commands.spawn((
                Camera3d::default(),
                Hdr,
                Tonemapping::TonyMcMapface,
                transform,
                Bloom {
                    intensity: bloom_config.intensity,
                    low_frequency_boost: bloom_config.low_frequency_boost,
                    low_frequency_boost_curvature: 0.95,
                    high_pass_frequency: 1.0,
                    prefilter: BloomPrefilter {
                        threshold: bloom_config.threshold,
                        threshold_softness: bloom_config.threshold_softness,
                    },
                    composite_mode: BloomCompositeMode::Additive,
                    ..default()
                },
            ));
        } else if config.use_hdr {
            // Forward rendering with HDR only
            commands.spawn((
                Camera3d::default(),
                Hdr,
                Tonemapping::TonyMcMapface,
                transform,
            ));
        } else {
            // Basic forward rendering
            commands.spawn((
                Camera3d::default(),
                Tonemapping::TonyMcMapface,
                transform,
            ));
        }
    }

    // Add directional light (for forward pass reference)
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Run setup callback if provided
    if let Some(callback) = setup_callback.0.take() {
        callback(&mut commands, &world);
    }

    // Store world for potential later use
    commands.insert_resource(LoadedWorld(world));

    println!("World setup complete.");
}

#[allow(deprecated)]
fn screenshot_and_exit(
    mut commands: Commands,
    mut frame_counter: ResMut<FrameCounter>,
    config: Res<VoxelWorldAppConfig>,
    mut exit: EventWriter<AppExit>,
) {
    frame_counter.0 += 1;

    let Some(ss_config) = &config.0.screenshot else {
        return;
    };

    if frame_counter.0 == ss_config.capture_frame {
        println!("Capturing screenshot at frame {}...", frame_counter.0);
        let path = ss_config.path.clone();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }

    if frame_counter.0 >= ss_config.exit_frame {
        println!("Exiting after {} frames", frame_counter.0);
        exit.write(AppExit::Success);
    }
}
