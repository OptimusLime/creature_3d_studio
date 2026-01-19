//! Lua renderer plugin for the map editor.
//!
//! **DEPRECATED**: Use `LuaLayerPlugin` instead, which manages both renderers
//! and visualizers with multi-instance support.
//!
//! Manages the base render layer with hot-reload support.

use super::hot_reload::{check_hot_reload, setup_hot_reload, HotReloadConfig, HotReloadFlag};
use super::render::{LuaRenderLayer, RenderLayerStack, RENDERER_LUA_PATH};
use bevy::prelude::*;

/// Marker type for renderer hot-reload.
pub struct RendererMarker;

/// Type alias for cleaner API.
pub type RendererReloadFlag = HotReloadFlag<RendererMarker>;

/// Plugin that enables hot-reload for Lua renderers.
pub struct LuaRendererPlugin {
    /// Path to the renderer directory to watch.
    pub watch_path: String,
    /// Path to the Lua file.
    pub lua_path: String,
}

impl Default for LuaRendererPlugin {
    fn default() -> Self {
        Self {
            watch_path: "assets/map_editor/renderers".to_string(),
            lua_path: RENDERER_LUA_PATH.to_string(),
        }
    }
}

impl Plugin for LuaRendererPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HotReloadConfig::<RendererMarker>::new(
            &self.watch_path,
            &self.lua_path,
        ));
        app.insert_resource(HotReloadFlag::<RendererMarker>::default());

        app.add_systems(
            Startup,
            (setup_renderer, setup_hot_reload::<RendererMarker>).chain(),
        );
        app.add_systems(
            Update,
            (check_hot_reload::<RendererMarker>, reload_renderer).chain(),
        );
    }
}

/// Setup the initial base layer in the render stack.
fn setup_renderer(
    mut render_stack: ResMut<RenderLayerStack>,
    config: Res<HotReloadConfig<RendererMarker>>,
) {
    let base_layer = LuaRenderLayer::new("base", &config.lua_path);
    render_stack.add_layer(Box::new(base_layer));
    info!("Added base render layer to stack");
}

/// Reload the Lua renderer when flag is set.
fn reload_renderer(
    config: Res<HotReloadConfig<RendererMarker>>,
    mut reload_flag: ResMut<HotReloadFlag<RendererMarker>>,
    mut render_stack: ResMut<RenderLayerStack>,
) {
    if !reload_flag.needs_reload {
        return;
    }
    reload_flag.needs_reload = false;

    info!("Reloading Lua renderer...");

    let mut lua_layer = LuaRenderLayer::new("base", &config.lua_path);
    if let Err(e) = lua_layer.reload() {
        error!("Failed to reload Lua renderer: {}", e);
    } else {
        info!("Lua renderer reloaded successfully");
    }
    render_stack.replace_layer(Box::new(lua_layer));
}
