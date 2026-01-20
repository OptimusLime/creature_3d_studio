//! Map Editor 2D Application Builder
//!
//! Provides a fluent builder API for creating map editor applications,
//! similar to `VoxelWorldApp` for 3D voxel scenes.
//!
//! # CLI Arguments
//!
//! The app supports command-line arguments:
//! - `--screenshot <path>` - Take a screenshot and save to path
//! - `--capture-frame <N>` - Frame number to capture screenshot (default: 30)
//! - `--exit-frame <N>` - Exit after N frames (optional, runs forever if not set)
//!
//! # Examples
//!
//! Interactive mode (runs forever):
//! ```bash
//! cargo run --example p_map_editor_2d
//! ```
//!
//! Screenshot mode (captures and exits):
//! ```bash
//! cargo run --example p_map_editor_2d -- --screenshot screenshots/test.png --exit-frame 45
//! ```

use super::{
    asset::{
        AssetFileWatcher, AssetStoreResource, BlobStore, DatabaseStore, EmbeddingService,
        InMemoryBlobStore,
    },
    checkerboard::{fill_checkerboard, step_checkerboard, CheckerboardState},
    imgui_screenshot::{AutoExitConfig, ImguiScreenshotConfig, ImguiScreenshotPlugin},
    lua_generator::LuaGeneratorPlugin,
    lua_layer_registry::LuaLayerPlugin,
    lua_materials::{LuaMaterialsPlugin, MaterialsLoadSet},
    material::MaterialPalette,
    mcp_server::McpServerPlugin,
    playback::PlaybackState,
    render::{FrameCapture, RenderSurfaceManager, SurfaceLayout},
    ui::{AssetBrowser, BrowserAction},
    voxel_buffer::VoxelBuffer,
};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use bevy_mod_imgui::prelude::{Condition, ImguiContext};
use std::path::Path;
use std::sync::Arc;

/// Default grid dimensions.
pub const DEFAULT_GRID_WIDTH: usize = 32;
pub const DEFAULT_GRID_HEIGHT: usize = 32;

/// Default frame to capture screenshot.
pub const DEFAULT_CAPTURE_FRAME: u32 = 30;

/// Scale factor for displaying the grid (pixels per cell).
const CANVAS_SCALE: f32 = 10.0;

/// Cell size for ImGui display.
const CELL_SIZE: f32 = 16.0;

/// Asset storage backend mode.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum AssetBackend {
    /// Use SQLite database (default). Path can be configured.
    #[default]
    Database,
    /// Use in-memory storage (no persistence, useful for testing).
    InMemory,
}

/// Default watch directory for auto-import.
pub const DEFAULT_WATCH_DIR: &str = "assets/incoming";

/// Configuration for the Map Editor 2D application.
#[derive(Default)]
pub struct MapEditor2DConfig {
    /// Window title.
    pub title: String,
    /// Window resolution (width, height).
    pub resolution: (u32, u32),
    /// Background clear color.
    pub clear_color: Color,
    /// Screenshot path (if any).
    pub screenshot_path: Option<String>,
    /// Frame to capture screenshot on.
    pub capture_frame: u32,
    /// Frame to exit on (None = run forever).
    pub exit_frame: Option<u32>,
    /// Grid dimensions.
    pub grid_size: (usize, usize),
    /// Asset storage backend mode.
    pub asset_backend: AssetBackend,
    /// Path to asset database (only used when backend is Database).
    pub asset_db_path: Option<String>,
    /// Directory to watch for auto-import (None = no watching).
    pub watch_dir: Option<String>,
}

/// Fluent builder for Map Editor 2D applications.
///
/// # Example
///
/// ```ignore
/// use studio_core::map_editor::MapEditor2DApp;
///
/// fn main() {
///     MapEditor2DApp::new("Map Editor 2D")
///         .with_resolution(1024, 768)
///         .with_cli_args() // Parse --screenshot, --capture-frame, --exit-frame
///         .run();
/// }
/// ```
pub struct MapEditor2DApp {
    config: MapEditor2DConfig,
}

impl MapEditor2DApp {
    /// Create a new Map Editor 2D application with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            config: MapEditor2DConfig {
                title: title.into(),
                resolution: (1024, 768),
                clear_color: Color::srgb(0.15, 0.15, 0.15),
                screenshot_path: None,
                capture_frame: DEFAULT_CAPTURE_FRAME,
                exit_frame: None,
                grid_size: (DEFAULT_GRID_WIDTH, DEFAULT_GRID_HEIGHT),
                asset_backend: AssetBackend::Database,
                asset_db_path: None,
                watch_dir: Some(DEFAULT_WATCH_DIR.to_string()),
            },
        }
    }

    /// Set the window resolution.
    pub fn with_resolution(mut self, width: u32, height: u32) -> Self {
        self.config.resolution = (width, height);
        self
    }

    /// Set the background clear color.
    pub fn with_clear_color(mut self, color: Color) -> Self {
        self.config.clear_color = color;
        self
    }

    /// Configure screenshot capture (path and frame).
    pub fn with_screenshot(mut self, path: impl Into<String>, capture_frame: u32) -> Self {
        self.config.screenshot_path = Some(path.into());
        self.config.capture_frame = capture_frame;
        self
    }

    /// Configure automatic exit after N frames.
    pub fn with_exit_frame(mut self, exit_frame: u32) -> Self {
        self.config.exit_frame = Some(exit_frame);
        self
    }

    /// Set the grid size.
    pub fn with_grid_size(mut self, width: usize, height: usize) -> Self {
        self.config.grid_size = (width, height);
        self
    }

    /// Parse command-line arguments to configure the app.
    ///
    /// Supported args:
    /// - `--screenshot <path>` - Take a screenshot and save to path
    /// - `--capture-frame <N>` - Frame number to capture screenshot (default: 30)
    /// - `--exit-frame <N>` - Exit after N frames
    /// - `--asset-db <path>` - Path to asset database (default: "assets.db")
    /// - `--no-persist` - Use in-memory storage (assets lost on restart)
    /// - `--watch-dir <path>` - Watch directory for auto-import (default: "assets/incoming")
    /// - `--no-watch` - Disable file watching
    pub fn with_cli_args(mut self) -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut i = 1; // Skip program name

        while i < args.len() {
            match args[i].as_str() {
                "--screenshot" => {
                    if i + 1 < args.len() {
                        self.config.screenshot_path = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Warning: --screenshot requires a path argument");
                        i += 1;
                    }
                }
                "--capture-frame" => {
                    if i + 1 < args.len() {
                        if let Ok(frame) = args[i + 1].parse() {
                            self.config.capture_frame = frame;
                        } else {
                            eprintln!("Warning: --capture-frame requires a number");
                        }
                        i += 2;
                    } else {
                        eprintln!("Warning: --capture-frame requires a number argument");
                        i += 1;
                    }
                }
                "--exit-frame" => {
                    if i + 1 < args.len() {
                        if let Ok(frame) = args[i + 1].parse() {
                            self.config.exit_frame = Some(frame);
                        } else {
                            eprintln!("Warning: --exit-frame requires a number");
                        }
                        i += 2;
                    } else {
                        eprintln!("Warning: --exit-frame requires a number argument");
                        i += 1;
                    }
                }
                "--asset-db" => {
                    if i + 1 < args.len() {
                        self.config.asset_db_path = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Warning: --asset-db requires a path argument");
                        i += 1;
                    }
                }
                "--no-persist" => {
                    self.config.asset_backend = AssetBackend::InMemory;
                    i += 1;
                }
                "--watch-dir" => {
                    if i + 1 < args.len() {
                        self.config.watch_dir = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Warning: --watch-dir requires a path argument");
                        i += 1;
                    }
                }
                "--no-watch" => {
                    // Explicitly disable watching by setting to empty
                    self.config.watch_dir = None;
                    i += 1;
                }
                _ => {
                    i += 1; // Skip unknown args
                }
            }
        }

        // Default watch directory if not explicitly disabled
        if self.config.watch_dir.is_none() && self.config.asset_backend == AssetBackend::Database {
            self.config.watch_dir = Some(DEFAULT_WATCH_DIR.to_string());
        }

        self
    }

    /// Set the asset database path programmatically.
    pub fn with_asset_db(mut self, path: impl Into<String>) -> Self {
        self.config.asset_db_path = Some(path.into());
        self
    }

    /// Use in-memory asset storage (no persistence).
    pub fn with_no_persist(mut self) -> Self {
        self.config.asset_backend = AssetBackend::InMemory;
        self
    }

    /// Set the watch directory for auto-import.
    pub fn with_watch_dir(mut self, path: impl Into<String>) -> Self {
        self.config.watch_dir = Some(path.into());
        self
    }

    /// Disable file watching.
    pub fn with_no_watch(mut self) -> Self {
        self.config.watch_dir = None;
        self
    }

    /// Run the application.
    pub fn run(self) {
        // Ensure screenshots directory exists if needed
        if let Some(ref path) = self.config.screenshot_path {
            if let Some(parent) = Path::new(path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        let title = self.config.title.clone();
        let resolution = self.config.resolution;
        let clear_color = self.config.clear_color;
        let grid_size = self.config.grid_size;
        let asset_backend = self.config.asset_backend.clone();
        let asset_db_path = self
            .config
            .asset_db_path
            .unwrap_or_else(|| "assets.db".to_string());

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

        // ImGui plugin
        app.add_plugins(bevy_mod_imgui::ImguiPlugin::default());

        // ImGui screenshot plugin (captures imgui panels in screenshots)
        app.add_plugins(ImguiScreenshotPlugin);

        // Screenshot config (if path specified) - uses ImGui-aware screenshot
        if let Some(ref path) = self.config.screenshot_path {
            app.insert_resource(ImguiScreenshotConfig::new(
                path.clone(),
                self.config.capture_frame,
            ));
        }

        // Auto-exit config (if exit frame specified)
        if let Some(exit_frame) = self.config.exit_frame {
            app.insert_resource(AutoExitConfig::new(exit_frame));
        }

        // Lua materials plugin (loads materials from assets/map_editor/materials.lua)
        app.add_plugins(LuaMaterialsPlugin::default());

        // Resources (must be inserted BEFORE LuaGeneratorPlugin which uses VoxelBuffer)
        app.insert_resource(ClearColor(clear_color));
        app.insert_resource(VoxelBuffer::new_2d(grid_size.0, grid_size.1));
        app.insert_resource(MaterialPalette::default()); // Will be replaced by Lua materials on first frame
        app.insert_resource(CheckerboardState::default()); // Kept for fallback
        app.insert_resource(PlaybackState::default());
        app.insert_resource(GridConfig {
            width: grid_size.0,
            height: grid_size.1,
        });
        app.insert_resource(SearchState::default());
        app.insert_resource(AssetBrowserState::default());

        // Render surface manager - single source of truth for all render surfaces
        // M10.4: Foundation for multi-surface rendering
        // Currently using single "grid" surface with all layers (base + visualizer)
        // M10.8 will add "mj_structure" surface for Markov Jr. node tree visualization
        let mut surface_manager = RenderSurfaceManager::new();
        surface_manager.add_surface("grid", grid_size.0, grid_size.1);
        surface_manager.set_layout(SurfaceLayout::Single("grid".to_string()));
        app.insert_resource(surface_manager);

        // Frame capture for video export
        app.insert_resource(FrameCapture::new(30));
        app.insert_resource(RecordingState {
            export_path: "generation.mp4".to_string(),
            last_result: String::new(),
        });

        // Asset store - switchable backend via config
        // We need Arc<dyn BlobStore> for sharing with file watcher
        let store_arc: Arc<dyn BlobStore>;
        let embedding_service: Option<Arc<EmbeddingService>>;

        match asset_backend {
            AssetBackend::Database => {
                let db_path = std::path::Path::new(&asset_db_path);
                match DatabaseStore::open(db_path) {
                    Ok(store) => {
                        info!("Opened asset database at {:?}", db_path);
                        let store = Arc::new(store);
                        // EmbeddingService needs Arc<DatabaseStore> for set_embedding
                        let service = Arc::new(EmbeddingService::new(Arc::clone(&store)));
                        embedding_service = Some(service);
                        store_arc = store;
                    }
                    Err(e) => {
                        error!(
                            "Failed to open asset database: {}. Using in-memory fallback.",
                            e
                        );
                        store_arc = Arc::new(InMemoryBlobStore::new());
                        embedding_service = None;
                    }
                }
            }
            AssetBackend::InMemory => {
                info!("Using in-memory asset store (no persistence)");
                store_arc = Arc::new(InMemoryBlobStore::new());
                embedding_service = None;
            }
        }

        // Insert asset store resource
        app.insert_resource(AssetStoreResource::from_dyn(Arc::clone(&store_arc)));

        // Insert embedding service if available
        if let Some(ref service) = embedding_service {
            app.insert_resource((**service).clone());
        }

        // File watcher for auto-import (M14)
        if let Some(ref watch_dir) = self.config.watch_dir {
            let watch_path = std::path::Path::new(watch_dir);
            match AssetFileWatcher::new(
                watch_path,
                Arc::clone(&store_arc),
                embedding_service.clone(),
            ) {
                Ok(watcher) => {
                    info!("Asset file watcher started on {}", watch_dir);
                    app.insert_non_send_resource(watcher);
                    app.add_systems(Update, process_file_watcher_events);
                }
                Err(e) => {
                    error!("Failed to start file watcher: {}. Auto-import disabled.", e);
                }
            }
        }

        // Lua layer plugin (manages all render layers and visualizers with hot-reload)
        // Replaces LuaRendererPlugin and LuaVisualizerPlugin
        app.add_plugins(LuaLayerPlugin::default());

        // Lua generator plugin (loads generator from assets/map_editor/generator.lua)
        // Must be added AFTER VoxelBuffer resource is inserted
        app.add_plugins(LuaGeneratorPlugin::default());

        // MCP server plugin (HTTP API for external AI)
        app.add_plugins(McpServerPlugin::default());

        // Systems
        app.add_systems(Startup, setup);

        // Map editor systems run AFTER materials are loaded (MaterialsLoadSet)
        // Note: LuaGeneratorPlugin handles generation internally, so we just need
        // to update the canvas texture and render UI
        // ImguiScreenshotPlugin handles screenshot capture and auto-exit internally
        app.add_systems(
            Update,
            (update_canvas_texture_system, render_ui_system)
                .chain()
                .after(MaterialsLoadSet),
        );

        // Run
        app.run();

        // Verify screenshot if configured
        if let Some(ref path) = self.config.screenshot_path {
            if Path::new(path).exists() {
                println!("SUCCESS: Screenshot saved to {}", path);
            } else {
                println!("WARNING: Screenshot was not created at {}", path);
            }
        }
    }
}

// =============================================================================
// Internal Resources and Components
// =============================================================================

#[derive(Resource)]
struct GridConfig {
    width: usize,
    height: usize,
}

/// Handle to the render texture.
#[derive(Resource)]
struct CanvasTexture {
    handle: Handle<Image>,
}

/// Marker for the canvas sprite entity.
#[derive(Component)]
struct CanvasSprite;

/// Search state for the UI.
#[derive(Resource, Default)]
struct SearchState {
    /// Current search query.
    query: String,
    /// Cached search results.
    results: Vec<SearchResult>,
}

/// A search result entry.
struct SearchResult {
    asset_type: String,
    name: String,
    id: u32,
    tags: Vec<String>,
}

/// Asset browser state for the database-backed asset panel.
#[derive(Resource)]
struct AssetBrowserState {
    /// The browser UI state.
    browser: AssetBrowser,
    /// Whether the browser window is open.
    is_open: bool,
}

impl Default for AssetBrowserState {
    fn default() -> Self {
        Self {
            browser: AssetBrowser::new(),
            is_open: true, // Open by default so it's visible in screenshots
        }
    }
}

// =============================================================================
// Systems
// =============================================================================

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, grid: Res<GridConfig>) {
    commands.spawn(Camera2d);

    // Create the canvas texture
    let size = Extent3d {
        width: grid.width as u32,
        height: grid.height as u32,
        depth_or_array_layers: 1,
    };

    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[30, 30, 30, 255], // Dark gray initially
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());

    let handle = images.add(image);

    // Spawn a sprite to display the canvas texture (scaled up for visibility)
    commands.spawn((
        Sprite {
            image: handle.clone(),
            custom_size: Some(Vec2::new(
                grid.width as f32 * CANVAS_SCALE,
                grid.height as f32 * CANVAS_SCALE,
            )),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        CanvasSprite,
    ));

    commands.insert_resource(CanvasTexture { handle });
}

/// Handle regeneration requests (when active palette changes).
/// NOTE: This is currently unused - LuaGeneratorPlugin handles generation.
/// Kept for potential fallback if no generator.lua exists.
#[allow(dead_code)]
fn generate_checkerboard_system(
    buffer: Res<VoxelBuffer>,
    mut state: ResMut<CheckerboardState>,
    mut playback: ResMut<PlaybackState>,
    mut palette: ResMut<MaterialPalette>,
) {
    // Regenerate when palette changes
    if !palette.changed && !state.needs_regenerate {
        return;
    }

    // Use first two materials from active palette
    if palette.active.len() >= 2 {
        state.material_a = palette.active[0];
        state.material_b = palette.active[1];
    } else if palette.active.len() == 1 {
        // Only one material - use it for both
        state.material_a = palette.active[0];
        state.material_b = palette.active[0];
    } else {
        // No active materials - nothing to do
        return;
    }

    palette.clear_changed();
    state.needs_regenerate = false;

    info!(
        "Regenerating checkerboard with active palette: {:?} (using {} + {})",
        palette.active, state.material_a, state.material_b
    );

    // Clear buffer and reset playback
    buffer.clear();
    playback.reset();

    // Fill immediately with current material_a and material_b
    fill_checkerboard(&buffer, &state);
    playback.step_index = buffer.cell_count();
    playback.complete();

    info!("Checkerboard regeneration complete");
}

/// Update playback - advance generation based on speed.
/// NOTE: This is currently unused - LuaGeneratorPlugin handles playback.
#[allow(dead_code)]
fn update_playback_system(
    time: Res<Time>,
    buffer: Res<VoxelBuffer>,
    checker_state: Res<CheckerboardState>,
    mut playback: ResMut<PlaybackState>,
) {
    // Handle reset request (when materials change)
    if checker_state.needs_regenerate {
        return; // Will be handled by generate_checkerboard_system
    }

    if playback.completed || !playback.playing {
        return;
    }

    playback.accumulator += time.delta_secs() * playback.speed;

    while playback.accumulator >= 1.0 && !playback.completed {
        playback.accumulator -= 1.0;
        step_checkerboard(&buffer, &checker_state, &mut playback);
    }
}

/// Update the canvas texture from the voxel buffer.
fn update_canvas_texture_system(
    buffer: Res<VoxelBuffer>,
    palette: Res<MaterialPalette>,
    canvas: Res<CanvasTexture>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(image) = images.get_mut(&canvas.handle) else {
        return;
    };

    // Build new pixel data
    let mut new_data = vec![0u8; buffer.width() * buffer.height() * 4];

    for y in 0..buffer.height() {
        for x in 0..buffer.width() {
            let mat_id = buffer.get_2d(x, y);
            let color = if mat_id == 0 {
                [30u8, 30, 30, 255] // Empty = dark gray
            } else if let Some(mat) = palette.get_by_id(mat_id) {
                [
                    (mat.color[0] * 255.0) as u8,
                    (mat.color[1] * 255.0) as u8,
                    (mat.color[2] * 255.0) as u8,
                    255,
                ]
            } else {
                [255u8, 0, 255, 255] // Unknown = magenta
            };

            let idx = (y * buffer.width() + x) * 4;
            new_data[idx..idx + 4].copy_from_slice(&color);
        }
    }

    // Replace the image data entirely - this should trigger Bevy to re-upload to GPU
    image.data = Some(new_data);
}

/// Recording state for video export UI.
#[derive(Resource, Default)]
struct RecordingState {
    /// Path for video export.
    export_path: String,
    /// Last export result message.
    last_result: String,
}

/// Render ImGui UI.
fn render_ui_system(
    mut context: NonSendMut<ImguiContext>,
    mut palette: ResMut<MaterialPalette>,
    mut playback: ResMut<PlaybackState>,
    buffer: Res<VoxelBuffer>,
    grid: Res<GridConfig>,
    mut gen_reload: ResMut<super::lua_generator::GeneratorReloadFlag>,
    mut search_state: ResMut<SearchState>,
    mut frame_capture: ResMut<FrameCapture>,
    _surface_manager: Res<RenderSurfaceManager>,
    mut recording_state: ResMut<RecordingState>,
    mut browser_state: ResMut<AssetBrowserState>,
    asset_store: Res<AssetStoreResource>,
) {
    // NOTE: The canvas is displayed as a Bevy Sprite, not an ImGui Image.
    // ImGui's Image widget doesn't update when texture data changes.

    let ui = context.ui();

    // === Available Materials Panel ===
    ui.window("Available")
        .size([200.0, 200.0], Condition::FirstUseEver)
        .position([20.0, 20.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Click [+] to add to palette");
            ui.separator();

            // Collect actions to avoid borrow issues
            let mut add_id = None;

            for mat in palette.available.iter() {
                let is_active = palette.is_active(mat.id);
                let color = [mat.color[0], mat.color[1], mat.color[2], 1.0];

                // [+] button (disabled if already active)
                if is_active {
                    ui.text_disabled("[+]");
                } else {
                    let _color_token =
                        ui.push_style_color(imgui::StyleColor::Button, [0.2, 0.5, 0.2, 1.0]);
                    if ui.button(format!("[+]##{}", mat.id)) {
                        add_id = Some(mat.id);
                    }
                }

                ui.same_line();

                // Color swatch + name
                let _color_token = ui.push_style_color(imgui::StyleColor::Button, color);
                ui.button_with_size(&mat.name, [120.0, 20.0]);
            }

            if let Some(id) = add_id {
                palette.add_to_active(id);
                info!("Added material {} to active palette", id);
            }
        });

    // === Active Palette Panel ===
    ui.window("Active Palette")
        .size([200.0, 200.0], Condition::FirstUseEver)
        .position([20.0, 240.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Click [x] to remove");
            ui.separator();

            // Collect actions to avoid borrow issues
            let mut remove_id = None;

            // Get active materials for display
            let active_mats: Vec<_> = palette
                .active
                .iter()
                .filter_map(|&id| palette.get_by_id(id).map(|m| (id, m.name.clone(), m.color)))
                .collect();

            for (id, name, color) in active_mats {
                // [x] button
                let _color_token =
                    ui.push_style_color(imgui::StyleColor::Button, [0.5, 0.2, 0.2, 1.0]);
                if ui.button(format!("[x]##{}", id)) {
                    remove_id = Some(id);
                }

                ui.same_line();

                // Color swatch + name
                let _color_token2 = ui.push_style_color(
                    imgui::StyleColor::Button,
                    [color[0], color[1], color[2], 1.0],
                );
                ui.button_with_size(&name, [120.0, 20.0]);
            }

            if let Some(id) = remove_id {
                palette.remove_from_active(id);
                info!("Removed material {} from active palette", id);
            }

            ui.separator();
            ui.text(format!("Generator uses: {:?}", palette.active));
        });

    // === Search Panel ===
    ui.window("Search")
        .size([200.0, 200.0], Condition::FirstUseEver)
        .position([20.0, 460.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Search assets by name");
            ui.separator();

            // Search input
            if ui.input_text("##search", &mut search_state.query).build() {
                // Query changed, update results
                let query = search_state.query.to_lowercase();

                if query.is_empty() {
                    search_state.results.clear();
                } else {
                    // Search materials using InMemoryStore::search (searches name + tags)
                    search_state.results = palette
                        .search(&query)
                        .into_iter()
                        .map(|mat| SearchResult {
                            asset_type: "material".to_string(),
                            name: mat.name.clone(),
                            id: mat.id,
                            tags: mat.tags.clone(),
                        })
                        .collect();
                }
            }

            ui.separator();

            // Results list
            if search_state.results.is_empty() {
                if !search_state.query.is_empty() {
                    ui.text_disabled("No results");
                }
            } else {
                ui.text(format!("{} result(s):", search_state.results.len()));

                // Collect click actions
                let mut add_id = None;

                for result in &search_state.results {
                    // Get color from palette if it's a material
                    let color = if result.asset_type == "material" {
                        palette
                            .get_by_id(result.id)
                            .map(|m| [m.color[0], m.color[1], m.color[2], 1.0])
                            .unwrap_or([0.5, 0.5, 0.5, 1.0])
                    } else {
                        [0.5, 0.5, 0.5, 1.0]
                    };

                    // Clickable result
                    let _color_token = ui.push_style_color(imgui::StyleColor::Button, color);
                    if ui.button_with_size(format!("{}", result.name), [180.0, 20.0]) {
                        // Add to active palette if material
                        if result.asset_type == "material" {
                            add_id = Some(result.id);
                        }
                    }
                    // Show tags as tooltip on hover
                    if !result.tags.is_empty() && ui.is_item_hovered() {
                        ui.tooltip_text(format!("Tags: {}", result.tags.join(", ")));
                    }
                }

                if let Some(id) = add_id {
                    if !palette.is_active(id) {
                        palette.add_to_active(id);
                        info!("Added material {} from search to active palette", id);
                    }
                }
            }
        });

    // === Canvas info window ===
    // NOTE: The actual checkerboard is displayed as a Bevy Sprite in the center of the screen.
    // ImGui's Image widget doesn't update when texture data changes, so we use a Sprite instead.
    ui.window("Canvas")
        .size([200.0, 80.0], Condition::FirstUseEver)
        .position([220.0, 20.0], Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("Grid: {}x{}", grid.width, grid.height));
            ui.text("(View canvas in center of screen)");
        });

    // === Playback Controls (Bottom) ===
    ui.window("Playback")
        .size([400.0, 180.0], Condition::FirstUseEver)
        .position(
            [220.0, grid.height as f32 * CELL_SIZE + 80.0],
            Condition::FirstUseEver,
        )
        .build(|| {
            // Play/Pause button
            let play_label = if playback.playing { "Pause" } else { "Play" };
            if ui.button(play_label) {
                playback.toggle_play();
            }

            ui.same_line();

            // Step button - temporarily pause, let one step run, then pause again
            if ui.button("Step") && !playback.completed {
                // Enable playing for one frame, accumulator will handle single step
                playback.playing = true;
                playback.accumulator = 1.0; // Trigger one step
            }

            ui.same_line();

            // Reset button - triggers generator reload which resets everything
            if ui.button("Reset") {
                gen_reload.needs_reload = true;
            }

            // Speed slider
            let mut speed = playback.speed;
            if ui.slider("Speed", 1.0f32, 1000.0f32, &mut speed) {
                playback.set_speed(speed);
            }
            ui.text(format!("{:.0} cells/sec", playback.speed));

            // Progress display
            let total_cells = buffer.cell_count();
            let progress = playback.step_index as f32 / total_cells as f32 * 100.0;
            ui.text(format!(
                "Progress: {}/{} ({:.1}%)",
                playback.step_index, total_cells, progress
            ));

            if playback.completed {
                ui.text_colored([0.0, 1.0, 0.0, 1.0], "Generation complete!");
            }

            ui.separator();

            // === Recording Controls ===
            ui.text("Recording:");

            let is_recording = frame_capture.is_recording();

            // Record button
            if is_recording {
                let _color_token =
                    ui.push_style_color(imgui::StyleColor::Button, [0.8, 0.2, 0.2, 1.0]);
                if ui.button("Stop Recording") {
                    frame_capture.stop();
                    recording_state.last_result =
                        format!("Recorded {} frames", frame_capture.frame_count());
                }
            } else {
                let _color_token =
                    ui.push_style_color(imgui::StyleColor::Button, [0.2, 0.5, 0.2, 1.0]);
                if ui.button("Start Recording") {
                    frame_capture.clear();
                    frame_capture.start();
                    recording_state.last_result.clear();
                }
            }

            ui.same_line();
            ui.text(format!("Frames: {}", frame_capture.frame_count()));

            // Export path input
            ui.input_text("Export path", &mut recording_state.export_path)
                .build();

            // Export button
            if !frame_capture.is_recording() && frame_capture.frame_count() > 0 {
                if ui.button("Export Video") {
                    let path = std::path::Path::new(&recording_state.export_path);
                    match frame_capture.export_video(path, "libx264") {
                        Ok(_) => {
                            recording_state.last_result =
                                format!("Exported to {}", recording_state.export_path);
                        }
                        Err(e) => {
                            recording_state.last_result = format!("Export failed: {}", e);
                        }
                    }
                }

                ui.same_line();

                // Export PNGs button
                if ui.button("Export PNGs") {
                    let dir = std::path::Path::new(&recording_state.export_path)
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .join("frames");
                    match frame_capture.export_pngs(&dir) {
                        Ok(count) => {
                            recording_state.last_result =
                                format!("Exported {} PNGs to {:?}", count, dir);
                        }
                        Err(e) => {
                            recording_state.last_result = format!("Export failed: {}", e);
                        }
                    }
                }
            }

            // Status message
            if !recording_state.last_result.is_empty() {
                ui.text_colored([0.7, 0.7, 1.0, 1.0], &recording_state.last_result);
            }
        });

    // === Asset Browser Panel ===
    // Toggle button in main menu area
    ui.window("Browser Toggle")
        .size([120.0, 40.0], Condition::FirstUseEver)
        .position([450.0, 20.0], Condition::FirstUseEver)
        .build(|| {
            let label = if browser_state.is_open {
                "Hide Browser"
            } else {
                "Show Browser"
            };
            if ui.button(label) {
                browser_state.is_open = !browser_state.is_open;
            }
        });

    // Browser window
    if browser_state.is_open {
        let mut opened = browser_state.is_open;
        ui.window("Asset Browser")
            .size([400.0, 500.0], Condition::FirstUseEver)
            .position([600.0, 20.0], Condition::FirstUseEver)
            .opened(&mut opened)
            .build(|| {
                // Render browser and handle actions
                if let Some(action) = browser_state.browser.render(ui, asset_store.store()) {
                    match action {
                        BrowserAction::Load(key) => {
                            info!("Browser: Load asset {:?}", key);
                            // TODO: Implement loading asset into editor
                        }
                        BrowserAction::Edit(key) => {
                            info!("Browser: Edit asset {:?}", key);
                            // TODO: Implement editing asset
                        }
                        BrowserAction::Delete(key) => {
                            info!("Browser: Delete asset {:?}", key);
                            // TODO: Implement deleting asset
                            if let Err(e) = asset_store.delete(&key) {
                                error!("Failed to delete asset: {}", e);
                            } else {
                                browser_state.browser.mark_dirty();
                            }
                        }
                    }
                }
            });
        browser_state.is_open = opened;
    }
}

/// System to process file watcher events (M14: File Watcher Auto-Import).
///
/// Checks for pending file events and imports/removes assets accordingly.
/// The watcher is NonSend because notify uses threads internally.
fn process_file_watcher_events(
    watcher: Option<NonSend<AssetFileWatcher>>,
    mut browser_state: ResMut<AssetBrowserState>,
) {
    let Some(watcher) = watcher else { return };

    let count = watcher.process_events();
    if count > 0 {
        // Mark browser dirty so it refreshes the tree
        browser_state.browser.mark_dirty();
    }
}
