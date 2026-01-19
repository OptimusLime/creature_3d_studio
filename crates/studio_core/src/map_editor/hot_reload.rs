//! Generic hot-reload infrastructure for Lua assets.
//!
//! Provides file watching and reload triggering for any Lua-based asset.
//! Individual plugins use this to avoid duplicating watcher boilerplate.
//!
//! # Usage
//!
//! ```ignore
//! // In your plugin's build():
//! app.insert_resource(HotReloadConfig::<MyMarker>::new("assets/path"));
//! app.insert_resource(HotReloadFlag::<MyMarker>::default());
//! app.add_systems(Startup, setup_hot_reload::<MyMarker>);
//! app.add_systems(Update, check_hot_reload::<MyMarker>);
//! ```

use bevy::prelude::*;
use notify::{recommended_watcher, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::marker::PhantomData;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};

/// Configuration for a hot-reloadable Lua asset.
/// The type parameter `T` is a marker type to distinguish different asset types.
#[derive(Resource)]
pub struct HotReloadConfig<T: Send + Sync + 'static> {
    /// Directory or file path to watch.
    pub watch_path: String,
    /// Specific Lua file path (for single-file assets).
    pub lua_path: String,
    _marker: PhantomData<T>,
}

impl<T: Send + Sync + 'static> HotReloadConfig<T> {
    /// Create a new config watching a directory, with a specific Lua file.
    pub fn new(watch_path: impl Into<String>, lua_path: impl Into<String>) -> Self {
        Self {
            watch_path: watch_path.into(),
            lua_path: lua_path.into(),
            _marker: PhantomData,
        }
    }

    /// Create a config where watch path is derived from lua path's parent.
    pub fn from_lua_path(lua_path: impl Into<String>) -> Self {
        let lua_path = lua_path.into();
        let watch_path = Path::new(&lua_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        Self {
            watch_path,
            lua_path,
            _marker: PhantomData,
        }
    }
}

/// Flag to trigger reload for a hot-reloadable asset.
#[derive(Resource)]
pub struct HotReloadFlag<T: Send + Sync + 'static> {
    pub needs_reload: bool,
    _marker: PhantomData<T>,
}

impl<T: Send + Sync + 'static> Default for HotReloadFlag<T> {
    fn default() -> Self {
        Self {
            needs_reload: true, // Load on first frame
            _marker: PhantomData,
        }
    }
}

impl<T: Send + Sync + 'static> HotReloadFlag<T> {
    pub fn new(needs_reload: bool) -> Self {
        Self {
            needs_reload,
            _marker: PhantomData,
        }
    }
}

/// File watcher for a hot-reloadable asset (non-send due to mpsc Receiver).
pub struct HotReloadWatcher<T: Send + Sync + 'static> {
    _watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event, notify::Error>>,
    _marker: PhantomData<T>,
}

/// Setup the file watcher for a hot-reloadable asset.
/// Call this in Startup after inserting HotReloadConfig and HotReloadFlag.
pub fn setup_hot_reload<T: Send + Sync + 'static>(world: &mut World) {
    let config = world.resource::<HotReloadConfig<T>>();
    let watch_path_str = config.watch_path.clone();
    let watch_path = Path::new(&watch_path_str);

    let (tx, rx) = channel();

    let mut watcher = match recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            error!(
                "Failed to create file watcher for {:?}: {:?}",
                watch_path, e
            );
            return;
        }
    };

    if let Err(e) = watcher.watch(watch_path, RecursiveMode::NonRecursive) {
        error!("Failed to watch directory {:?}: {:?}", watch_path, e);
        return;
    }

    info!("Hot reload enabled for {}", watch_path_str);

    world.insert_non_send_resource(HotReloadWatcher::<T> {
        _watcher: watcher,
        receiver: rx,
        _marker: PhantomData,
    });
}

/// Check for file changes and set reload flag.
/// Call this in Update before your reload system.
pub fn check_hot_reload<T: Send + Sync + 'static>(
    watcher: Option<NonSend<HotReloadWatcher<T>>>,
    mut reload_flag: ResMut<HotReloadFlag<T>>,
) {
    let Some(watcher) = watcher else { return };

    while let Ok(event) = watcher.receiver.try_recv() {
        if let Ok(event) = event {
            for path in &event.paths {
                if path.extension().map(|e| e == "lua").unwrap_or(false) {
                    info!(
                        "Detected change in {:?}, scheduling reload...",
                        path.file_name()
                    );
                    reload_flag.needs_reload = true;
                }
            }
        }
    }
}

/// Trait for Lua assets that can be reloaded.
/// Implement this for your layer type to use with the reload system.
pub trait ReloadableLuaAsset: Sized {
    /// Create a new instance from a path.
    fn new_from_path(name: &str, path: &str) -> Self;

    /// Reload the Lua script.
    fn reload(&mut self) -> Result<(), String>;

    /// Get the layer name.
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestMarker;

    #[test]
    fn test_config_from_lua_path() {
        let config = HotReloadConfig::<TestMarker>::from_lua_path("assets/foo/bar.lua");
        assert_eq!(config.lua_path, "assets/foo/bar.lua");
        assert_eq!(config.watch_path, "assets/foo");
    }

    #[test]
    fn test_reload_flag_default() {
        let flag = HotReloadFlag::<TestMarker>::default();
        assert!(flag.needs_reload); // Should be true to trigger initial load
    }
}
