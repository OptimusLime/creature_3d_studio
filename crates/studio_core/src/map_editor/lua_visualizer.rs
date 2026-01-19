//! Lua visualizer plugin for the map editor.
//!
//! **DEPRECATED**: Use `LuaLayerPlugin` instead, which manages both renderers
//! and visualizers with multi-instance support.
//!
//! Manages the visualizer layer with hot-reload support.
//! The visualizer implements `GeneratorListener` to receive step events.

use super::generator::GeneratorListeners;
use super::hot_reload::{check_hot_reload, setup_hot_reload, HotReloadConfig, HotReloadFlag};
use super::render::{LuaVisualizer, RenderLayerStack, SharedVisualizer, VISUALIZER_LUA_PATH};
use bevy::prelude::*;

/// Marker type for visualizer hot-reload.
pub struct VisualizerMarker;

/// Type alias for cleaner API.
pub type VisualizerReloadFlag = HotReloadFlag<VisualizerMarker>;

/// Resource holding the shared visualizer for both rendering and listening.
#[derive(Resource, Clone)]
pub struct VisualizerState {
    pub shared: SharedVisualizer,
}

/// Plugin that enables hot-reload for Lua visualizers.
pub struct LuaVisualizerPlugin {
    /// Path to the visualizer directory to watch.
    pub watch_path: String,
    /// Path to the Lua file.
    pub lua_path: String,
}

impl Default for LuaVisualizerPlugin {
    fn default() -> Self {
        let lua_path = VISUALIZER_LUA_PATH.to_string();
        let watch_path = std::path::Path::new(&lua_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "assets/map_editor/visualizers".to_string());

        Self {
            watch_path,
            lua_path,
        }
    }
}

impl Plugin for LuaVisualizerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HotReloadConfig::<VisualizerMarker>::new(
            &self.watch_path,
            &self.lua_path,
        ));
        app.insert_resource(HotReloadFlag::<VisualizerMarker>::default());

        app.add_systems(
            Startup,
            (setup_visualizer, setup_hot_reload::<VisualizerMarker>).chain(),
        );
        app.add_systems(
            Update,
            (check_hot_reload::<VisualizerMarker>, reload_visualizer).chain(),
        );
    }
}

/// Setup the initial visualizer layer and register as listener.
fn setup_visualizer(
    mut commands: Commands,
    mut render_stack: ResMut<RenderLayerStack>,
    mut listeners: ResMut<GeneratorListeners>,
    config: Res<HotReloadConfig<VisualizerMarker>>,
) {
    // Create the shared visualizer
    let visualizer = LuaVisualizer::new("visualizer", &config.lua_path);
    let shared = SharedVisualizer::new(visualizer);

    // Store in resource for reloading
    commands.insert_resource(VisualizerState {
        shared: shared.clone(),
    });

    // Add to render stack
    render_stack.add_layer(Box::new(shared.clone()));
    info!("Added visualizer layer to render stack");

    // Register as generator listener
    listeners.add(Box::new(shared));
    info!("Registered visualizer as generator listener");
}

/// Reload the Lua visualizer when flag is set.
fn reload_visualizer(
    mut reload_flag: ResMut<HotReloadFlag<VisualizerMarker>>,
    vis_state: Option<Res<VisualizerState>>,
) {
    if !reload_flag.needs_reload {
        return;
    }
    reload_flag.needs_reload = false;

    let Some(vis_state) = vis_state else {
        error!("VisualizerState not found");
        return;
    };

    info!("Reloading Lua visualizer...");

    // Reload the visualizer in place (it's shared, so render stack sees the change)
    let mut vis = vis_state.shared.lock();
    if let Err(e) = vis.reload() {
        error!("Failed to reload visualizer: {}", e);
    } else {
        info!("Lua visualizer reloaded successfully");
    }
}
