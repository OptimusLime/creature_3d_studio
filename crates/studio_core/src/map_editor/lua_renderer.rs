//! Lua renderer hot-reload plugin for the map editor.
//!
//! Watches the renderer Lua file and reloads it when changed.
//! Works with the `RenderLayerStack` to update the Lua-based render layer.

use super::render::{LuaRenderLayer, RenderLayerStack, RENDERER_LUA_PATH};
use bevy::prelude::*;
use notify::{recommended_watcher, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};

/// Plugin that enables hot-reload for Lua renderers.
pub struct LuaRendererPlugin {
    /// Path to the renderer directory to watch.
    pub path: String,
}

impl Default for LuaRendererPlugin {
    fn default() -> Self {
        Self {
            path: "assets/map_editor/renderers".to_string(),
        }
    }
}

impl Plugin for LuaRendererPlugin {
    fn build(&self, app: &mut App) {
        let path = self.path.clone();

        app.insert_resource(LuaRendererConfig { path: path.clone() });
        app.insert_resource(RendererReloadFlag { needs_reload: true }); // Load on first frame

        app.add_systems(Startup, setup_renderer_watcher);
        app.add_systems(Update, (check_renderer_reload, reload_renderer).chain());
    }
}

/// Configuration for the Lua renderer.
#[derive(Resource)]
pub struct LuaRendererConfig {
    pub path: String,
}

/// Flag to trigger renderer reload.
#[derive(Resource)]
pub struct RendererReloadFlag {
    pub needs_reload: bool,
}

/// Resource holding the file watcher for renderers.
struct RendererWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event, notify::Error>>,
}

/// Setup the file watcher for the renderer directory and add initial base layer.
fn setup_renderer_watcher(world: &mut World) {
    // Get config path first, clone to avoid borrow issues
    let watch_path_str = world.resource::<LuaRendererConfig>().path.clone();
    let watch_path = Path::new(&watch_path_str);

    // Add initial base layer to render stack (will be reloaded from Lua on first update)
    let base_layer = LuaRenderLayer::new("base", RENDERER_LUA_PATH);
    if let Some(mut stack) = world.get_resource_mut::<RenderLayerStack>() {
        stack.add_layer(Box::new(base_layer));
        info!("Added base render layer to stack");
    }

    let (tx, rx) = channel();

    let mut watcher = match recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create renderer file watcher: {:?}", e);
            return;
        }
    };

    if let Err(e) = watcher.watch(watch_path, RecursiveMode::NonRecursive) {
        error!(
            "Failed to watch renderer directory {:?}: {:?}",
            watch_path, e
        );
        return;
    }

    info!("Hot reload enabled for renderers at {}", watch_path_str);

    world.insert_non_send_resource(RendererWatcher {
        _watcher: watcher,
        receiver: rx,
    });
}

/// Check for file changes and set reload flag.
fn check_renderer_reload(
    watcher: Option<NonSend<RendererWatcher>>,
    mut reload_flag: ResMut<RendererReloadFlag>,
) {
    let Some(watcher) = watcher else { return };

    while let Ok(event) = watcher.receiver.try_recv() {
        if let Ok(event) = event {
            for path in &event.paths {
                // Check if it's a Lua file in the renderers directory
                if path.extension().map(|e| e == "lua").unwrap_or(false) {
                    info!(
                        "Detected change in renderer {:?}, scheduling reload...",
                        path.file_name()
                    );
                    reload_flag.needs_reload = true;
                }
            }
        }
    }
}

/// Reload the Lua renderer when flag is set.
fn reload_renderer(
    mut reload_flag: ResMut<RendererReloadFlag>,
    mut render_stack: ResMut<RenderLayerStack>,
) {
    if !reload_flag.needs_reload {
        return;
    }
    reload_flag.needs_reload = false;

    info!("Reloading Lua renderer...");

    // Create and reload the layer, then replace it in the stack
    let mut lua_layer = LuaRenderLayer::new("base", RENDERER_LUA_PATH);
    if let Err(e) = lua_layer.reload() {
        error!("Failed to reload Lua renderer: {}", e);
    } else {
        info!("Lua renderer reloaded successfully");
    }
    render_stack.replace_layer(Box::new(lua_layer));
}
