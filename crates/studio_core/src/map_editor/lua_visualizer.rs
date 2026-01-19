//! Lua-based visualizer plugin for the map editor.
//!
//! Provides hot-reload support for visualizer scripts that overlay
//! generation progress on the render output.
//!
//! # Usage
//!
//! ```ignore
//! app.add_plugins(LuaVisualizerPlugin::default());
//! ```

use super::render::{LuaVisualizer, RenderLayerStack, VISUALIZER_LUA_PATH};
use bevy::prelude::*;
use notify::{recommended_watcher, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};

/// Plugin that manages the Lua visualizer with hot-reload support.
pub struct LuaVisualizerPlugin {
    /// Path to the visualizer Lua file.
    pub path: String,
}

impl Default for LuaVisualizerPlugin {
    fn default() -> Self {
        Self {
            path: VISUALIZER_LUA_PATH.to_string(),
        }
    }
}

impl Plugin for LuaVisualizerPlugin {
    fn build(&self, app: &mut App) {
        let path = self.path.clone();

        app.insert_resource(VisualizerConfig { path: path.clone() });
        app.insert_resource(VisualizerReloadFlag { needs_reload: true });

        app.add_systems(Startup, setup_visualizer);
        app.add_systems(Update, (check_visualizer_reload, reload_visualizer).chain());
    }
}

/// Configuration for the visualizer.
#[derive(Resource)]
struct VisualizerConfig {
    path: String,
}

/// Flag to trigger visualizer reload.
#[derive(Resource)]
pub struct VisualizerReloadFlag {
    pub needs_reload: bool,
}

/// The visualizer state (stored separately for hot-reload).
#[derive(Resource)]
pub struct VisualizerState {
    pub visualizer: LuaVisualizer,
}

/// File watcher for visualizer hot-reload.
struct VisualizerWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event, notify::Error>>,
}

/// Setup the visualizer and file watcher.
fn setup_visualizer(world: &mut World) {
    let config = world.resource::<VisualizerConfig>();
    let path = config.path.clone();

    // Create the visualizer and add it to the render stack
    let visualizer = LuaVisualizer::new("visualizer", &path);

    // Store the visualizer state (for potential future use)
    world.insert_resource(VisualizerState {
        visualizer: LuaVisualizer::new("visualizer", &path),
    });

    // Add visualizer to render stack (after base layer)
    if let Some(mut stack) = world.get_resource_mut::<RenderLayerStack>() {
        stack.add_layer(Box::new(visualizer));
        info!("Added visualizer layer to render stack");
    } else {
        error!("RenderLayerStack not found - visualizer not added");
    }

    // Setup file watcher
    let watch_dir = Path::new(&path)
        .parent()
        .unwrap_or(Path::new("assets/map_editor/visualizers"));

    let (tx, rx) = channel();

    let mut watcher = match recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create visualizer file watcher: {:?}", e);
            return;
        }
    };

    if let Err(e) = watcher.watch(watch_dir, RecursiveMode::NonRecursive) {
        error!(
            "Failed to watch visualizer directory {:?}: {:?}",
            watch_dir, e
        );
        return;
    }

    info!("Hot reload enabled for visualizer at {}", path);

    world.insert_non_send_resource(VisualizerWatcher {
        _watcher: watcher,
        receiver: rx,
    });
}

/// Check for file changes and set reload flag.
fn check_visualizer_reload(
    watcher: Option<NonSend<VisualizerWatcher>>,
    mut reload_flag: ResMut<VisualizerReloadFlag>,
) {
    let Some(watcher) = watcher else { return };

    while let Ok(event) = watcher.receiver.try_recv() {
        if let Ok(event) = event {
            for path in &event.paths {
                if path.extension().map(|e| e == "lua").unwrap_or(false) {
                    info!(
                        "Detected change in visualizer file {:?}, scheduling reload...",
                        path.file_name()
                    );
                    reload_flag.needs_reload = true;
                }
            }
        }
    }
}

/// Reload the visualizer from Lua file.
fn reload_visualizer(
    config: Res<VisualizerConfig>,
    mut reload_flag: ResMut<VisualizerReloadFlag>,
    mut render_stack: ResMut<RenderLayerStack>,
) {
    if !reload_flag.needs_reload {
        return;
    }
    reload_flag.needs_reload = false;

    info!("Reloading Lua visualizer...");

    // Create and reload the visualizer, then replace it in the stack
    let mut visualizer = LuaVisualizer::new("visualizer", &config.path);
    if let Err(e) = visualizer.reload() {
        error!("Failed to reload visualizer: {}", e);
    } else {
        info!("Lua visualizer reloaded successfully");
    }
    render_stack.replace_layer(Box::new(visualizer));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visualizer_config() {
        let config = VisualizerConfig {
            path: "test.lua".to_string(),
        };
        assert_eq!(config.path, "test.lua");
    }
}
