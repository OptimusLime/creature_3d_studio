//! VoxelWorldPlugin - A unified plugin for setting up and running voxel world examples/tests.
//!
//! This plugin eliminates boilerplate across examples by providing:
//! - Configurable window setup
//! - World loading (from file, lua script, or builder function)
//! - Scene spawning with camera, lights
//! - Screenshot capture and auto-exit for testing
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
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::WindowPlugin;
use std::path::Path;

use crate::deferred::{DeferredCamera, DeferredRenderingPlugin};
use crate::scene_utils::{spawn_world_with_lights_config, CameraPreset, WorldSpawnConfig};
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

/// Camera configuration for the scene.
#[derive(Clone)]
pub enum CameraConfig {
    /// Automatically frame the world bounds
    AutoFrame { angle: f32, elevation: f32 },
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
    /// Camera configuration
    pub camera: CameraConfig,
    /// Screenshot configuration (if any)
    pub screenshot: Option<ScreenshotConfig>,
    /// Spawn point lights from emissive voxels
    pub spawn_emissive_lights: bool,
    /// Add a shadow-casting light
    pub shadow_light: Option<Vec3>,
}

/// Builder for creating a VoxelWorld app with minimal boilerplate.
pub struct VoxelWorldApp {
    config: VoxelWorldConfig,
    world_source: WorldSource,
    setup_callback: Option<Box<dyn FnOnce(&mut Commands, &VoxelWorld) + Send + Sync>>,
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
                camera: CameraConfig::default(),
                screenshot: None,
                spawn_emissive_lights: true,
                shadow_light: None,
            },
            world_source: WorldSource::Empty,
            setup_callback: None,
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
        self.config.camera = CameraConfig::AutoFrame { angle, elevation };
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
        app.add_systems(Update, screenshot_and_exit);

        // Run
        app.run();

        // Verify screenshot if requested
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

    // Spawn shadow light first (if configured)
    if let Some(light_pos) = config.shadow_light {
        use crate::deferred::DeferredPointLight;
        commands.spawn((
            DeferredPointLight {
                color: Color::srgb(1.0, 0.9, 0.7),
                intensity: 40.0,
                radius: 25.0,
            },
            Transform::from_translation(light_pos),
        ));
        println!("Added shadow-casting light at {:?}", light_pos);
    }

    // Spawn world meshes and lights
    if world.chunk_count() > 0 {
        let mut spawn_config = WorldSpawnConfig {
            use_greedy_meshing: config.use_greedy_meshing,
            use_cross_chunk_culling: true,
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
    match &config.camera {
        CameraConfig::AutoFrame { angle, elevation } => {
            if let Some((min, max)) = world.chunk_bounds() {
                // Calculate world bounds
                let min_world = Vec3::new(
                    (min.x * 32) as f32,
                    (min.y * 32) as f32,
                    (min.z * 32) as f32,
                );
                let max_world = Vec3::new(
                    ((max.x + 1) * 32) as f32,
                    ((max.y + 1) * 32) as f32,
                    ((max.z + 1) * 32) as f32,
                );
                let framing = crate::scene_utils::compute_camera_framing(
                    min_world,
                    max_world,
                    *angle,
                    *elevation,
                    1.0,
                );

                commands.spawn((
                    Camera3d::default(),
                    Tonemapping::TonyMcMapface,
                    Transform::from_translation(framing.position).looking_at(framing.look_at, Vec3::Y),
                    DeferredCamera,
                ));
                println!("Camera auto-framed at {:?}", framing.position);
            } else {
                // Empty world, spawn default camera
                commands.spawn((
                    Camera3d::default(),
                    Tonemapping::TonyMcMapface,
                    Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
                    DeferredCamera,
                ));
            }
        }
        CameraConfig::Preset(preset) => {
            commands.spawn((
                Camera3d::default(),
                Tonemapping::TonyMcMapface,
                Transform::from_translation(preset.position).looking_at(preset.look_at, Vec3::Y),
                DeferredCamera,
            ));
        }
        CameraConfig::Custom { position, look_at } => {
            commands.spawn((
                Camera3d::default(),
                Tonemapping::TonyMcMapface,
                Transform::from_translation(*position).looking_at(*look_at, Vec3::Y),
                DeferredCamera,
            ));
        }
        CameraConfig::None => {
            // User will spawn camera
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
