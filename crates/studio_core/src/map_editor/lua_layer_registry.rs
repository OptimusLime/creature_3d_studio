//! Registry for Lua-defined render layers and visualizers.
//!
//! Provides multi-instance support using `AssetStore<LuaLayerDef>` for storage.
//! This aligns with Phase 4's database-backed store - just swap the backend.
//!
//! # Architecture
//!
//! - `LuaLayerDef`: Definition of a layer (name, type, path, tags)
//! - `LuaLayerRegistry`: Stores definitions + manages live instances
//! - Single file watcher for all layers, reloads by matching paths
//!
//! # Example
//!
//! ```ignore
//! let mut registry = LuaLayerRegistry::new();
//! registry.register(LuaLayerDef {
//!     name: "grid".into(),
//!     layer_type: LuaLayerType::Renderer,
//!     lua_path: "assets/map_editor/renderers/grid_2d.lua".into(),
//!     tags: vec!["base".into()],
//! });
//! ```

use super::asset::{Asset, AssetStore, InMemoryStore};
use super::generator::GeneratorListeners;
use super::render::{LuaRenderLayer, LuaVisualizer, RenderSurfaceManager, SharedVisualizer};
use bevy::prelude::*;
use notify::{recommended_watcher, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};

/// Type of Lua layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LuaLayerType {
    /// Base renderer layer (renders voxels to pixels).
    Renderer,
    /// Visualizer layer (overlay, receives generator events).
    Visualizer,
}

/// Definition of a Lua-defined layer.
#[derive(Clone, Debug)]
pub struct LuaLayerDef {
    /// Unique name for this layer.
    pub name: String,
    /// Type of layer (renderer or visualizer).
    pub layer_type: LuaLayerType,
    /// Path to the Lua file.
    pub lua_path: String,
    /// Tags for categorization and search.
    pub tags: Vec<String>,
}

impl LuaLayerDef {
    /// Create a new renderer layer definition.
    pub fn renderer(name: impl Into<String>, lua_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            layer_type: LuaLayerType::Renderer,
            lua_path: lua_path.into(),
            tags: Vec::new(),
        }
    }

    /// Create a new visualizer layer definition.
    pub fn visualizer(name: impl Into<String>, lua_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            layer_type: LuaLayerType::Visualizer,
            lua_path: lua_path.into(),
            tags: Vec::new(),
        }
    }

    /// Add tags to this definition.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

impl Asset for LuaLayerDef {
    fn name(&self) -> &str {
        &self.name
    }

    fn asset_type() -> &'static str {
        "lua_layer"
    }

    fn tags(&self) -> &[String] {
        &self.tags
    }
}

/// Live instance of a Lua layer.
enum LuaLayerInstance {
    /// A renderer layer (owned by RenderSurfaceManager).
    Renderer,
    /// A visualizer layer (shared between render stack and listener registry).
    Visualizer(SharedVisualizer),
}

/// Registry for Lua layers, using AssetStore for definitions.
#[derive(Resource)]
pub struct LuaLayerRegistry {
    /// Layer definitions (what layers exist).
    store: InMemoryStore<LuaLayerDef>,
    /// Live instances, keyed by layer name.
    instances: HashMap<String, LuaLayerInstance>,
    /// Flag indicating a reload is needed for specific layers.
    pending_reloads: Vec<String>,
}

impl Default for LuaLayerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LuaLayerRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            store: InMemoryStore::new(),
            instances: HashMap::new(),
            pending_reloads: Vec::new(),
        }
    }

    /// Register a new layer definition.
    /// Returns the layer name for convenience.
    pub fn register(&mut self, def: LuaLayerDef) -> String {
        let name = def.name.clone();
        self.store.set(def);
        name
    }

    /// Unregister a layer by name.
    /// Returns the definition if it existed.
    pub fn unregister(&mut self, name: &str) -> Option<LuaLayerDef> {
        // Find and remove from store
        let list = self.store.list();
        let idx = list.iter().position(|d| d.name == name)?;

        // Get the def before removing
        let def = list[idx].clone();

        // Remove from instances
        self.instances.remove(name);

        // Rebuild store without this item
        let remaining: Vec<_> = list.iter().filter(|d| d.name != name).cloned().collect();
        self.store.set_all(remaining);

        Some(def)
    }

    /// Get a layer definition by name.
    pub fn get(&self, name: &str) -> Option<&LuaLayerDef> {
        self.store.find(|d| d.name == name)
    }

    /// List all layer definitions.
    pub fn list(&self) -> &[LuaLayerDef] {
        self.store.list()
    }

    /// Search layers by name or tag.
    pub fn search(&self, query: &str) -> Vec<&LuaLayerDef> {
        self.store.search(query)
    }

    /// Get layers by type.
    pub fn layers_of_type(&self, layer_type: LuaLayerType) -> Vec<&LuaLayerDef> {
        self.store
            .list()
            .iter()
            .filter(|d| d.layer_type == layer_type)
            .collect()
    }

    /// Mark a layer for reload.
    pub fn mark_for_reload(&mut self, name: &str) {
        if !self.pending_reloads.contains(&name.to_string()) {
            self.pending_reloads.push(name.to_string());
        }
    }

    /// Mark all layers for reload.
    pub fn mark_all_for_reload(&mut self) {
        for def in self.store.list() {
            if !self.pending_reloads.contains(&def.name) {
                self.pending_reloads.push(def.name.clone());
            }
        }
    }

    /// Take pending reloads (clears the list).
    pub fn take_pending_reloads(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_reloads)
    }

    /// Check if there are pending reloads.
    pub fn has_pending_reloads(&self) -> bool {
        !self.pending_reloads.is_empty()
    }

    /// Find layer by Lua path (for hot-reload matching).
    pub fn find_by_path(&self, path: &Path) -> Option<&LuaLayerDef> {
        let path_str = path.to_string_lossy();
        self.store.find(|d| {
            // Match by filename or full path
            path_str.ends_with(&d.lua_path)
                || Path::new(&d.lua_path)
                    .file_name()
                    .map(|f| path.ends_with(f))
                    .unwrap_or(false)
        })
    }

    /// Store a visualizer instance for sharing.
    pub fn store_visualizer(&mut self, name: &str, vis: SharedVisualizer) {
        self.instances
            .insert(name.to_string(), LuaLayerInstance::Visualizer(vis));
    }

    /// Get a stored visualizer instance.
    pub fn get_visualizer(&self, name: &str) -> Option<&SharedVisualizer> {
        match self.instances.get(name) {
            Some(LuaLayerInstance::Visualizer(v)) => Some(v),
            _ => None,
        }
    }

    /// Mark that a renderer exists (for tracking, actual instance is in RenderSurfaceManager).
    pub fn mark_renderer_exists(&mut self, name: &str) {
        self.instances
            .insert(name.to_string(), LuaLayerInstance::Renderer);
    }
}

/// File watcher for all Lua layers (non-send due to mpsc Receiver).
pub struct LayerWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event, notify::Error>>,
}

/// Plugin that manages Lua layers with hot-reload support.
pub struct LuaLayerPlugin {
    /// Directory to watch for Lua file changes.
    pub watch_dir: String,
    /// Initial layers to register on startup.
    pub initial_layers: Vec<LuaLayerDef>,
}

impl Default for LuaLayerPlugin {
    fn default() -> Self {
        Self {
            watch_dir: "assets/map_editor".to_string(),
            initial_layers: vec![
                LuaLayerDef::renderer("base", "assets/map_editor/renderers/grid_2d.lua"),
                LuaLayerDef::visualizer(
                    "visualizer",
                    "assets/map_editor/visualizers/step_highlight.lua",
                ),
            ],
        }
    }
}

impl Plugin for LuaLayerPlugin {
    fn build(&self, app: &mut App) {
        // Create registry with initial layers
        let mut registry = LuaLayerRegistry::new();
        for def in &self.initial_layers {
            registry.register(def.clone());
        }
        // Mark all for initial load
        registry.mark_all_for_reload();

        app.insert_resource(registry);
        app.insert_resource(LayerWatchDir(self.watch_dir.clone()));

        app.add_systems(Startup, setup_layer_watcher);
        app.add_systems(Update, (check_layer_changes, process_layer_reloads).chain());
    }
}

/// Resource holding the watch directory path.
#[derive(Resource)]
struct LayerWatchDir(String);

/// Setup the file watcher for all Lua layers.
fn setup_layer_watcher(world: &mut World) {
    let watch_dir = world.resource::<LayerWatchDir>().0.clone();
    let watch_path = Path::new(&watch_dir);

    let (tx, rx) = channel();

    let mut watcher = match recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create layer watcher: {:?}", e);
            return;
        }
    };

    // Watch recursively to catch subdirectories (renderers/, visualizers/)
    if let Err(e) = watcher.watch(watch_path, RecursiveMode::Recursive) {
        error!("Failed to watch directory {:?}: {:?}", watch_path, e);
        return;
    }

    info!("Layer hot-reload enabled for {} (recursive)", watch_dir);

    world.insert_non_send_resource(LayerWatcher {
        _watcher: watcher,
        receiver: rx,
    });
}

/// Check for file changes and mark affected layers for reload.
fn check_layer_changes(
    watcher: Option<NonSend<LayerWatcher>>,
    mut registry: ResMut<LuaLayerRegistry>,
) {
    let Some(watcher) = watcher else { return };

    while let Ok(event) = watcher.receiver.try_recv() {
        if let Ok(event) = event {
            for path in &event.paths {
                if path.extension().map(|e| e == "lua").unwrap_or(false) {
                    // Find which layer this file belongs to
                    if let Some(def) = registry.find_by_path(path) {
                        info!(
                            "Detected change in {:?}, marking '{}' for reload",
                            path.file_name(),
                            def.name
                        );
                        let name = def.name.clone();
                        registry.mark_for_reload(&name);
                    }
                }
            }
        }
    }
}

/// Process pending layer reloads.
fn process_layer_reloads(
    mut registry: ResMut<LuaLayerRegistry>,
    mut surface_manager: ResMut<RenderSurfaceManager>,
    mut listeners: ResMut<GeneratorListeners>,
) {
    if !registry.has_pending_reloads() {
        return;
    }

    let pending = registry.take_pending_reloads();

    for name in pending {
        let Some(def) = registry.get(&name).cloned() else {
            warn!("Layer '{}' not found in registry", name);
            continue;
        };

        match def.layer_type {
            LuaLayerType::Renderer => {
                reload_renderer(&name, &def.lua_path, &mut surface_manager, &mut registry);
            }
            LuaLayerType::Visualizer => {
                reload_visualizer(
                    &name,
                    &def.lua_path,
                    &mut surface_manager,
                    &mut listeners,
                    &mut registry,
                );
            }
        }
    }
}

/// Reload or create a renderer layer.
fn reload_renderer(
    name: &str,
    lua_path: &str,
    surface_manager: &mut RenderSurfaceManager,
    registry: &mut LuaLayerRegistry,
) {
    info!("Reloading renderer '{}'...", name);

    let mut layer = LuaRenderLayer::new(name, lua_path);
    if let Err(e) = layer.reload() {
        error!("Failed to reload renderer '{}': {}", name, e);
        return;
    }

    // Add to the "grid" surface (default surface for all renderers)
    if let Some(surface) = surface_manager.get_surface_mut("grid") {
        if surface.has_layer(name) {
            surface.replace_layer(Box::new(layer));
        } else {
            surface.add_layer(Box::new(layer));
        }
    } else {
        error!("Surface 'grid' not found - cannot add renderer '{}'", name);
        return;
    }

    registry.mark_renderer_exists(name);
    info!("Renderer '{}' reloaded successfully", name);
}

/// Reload or create a visualizer layer.
fn reload_visualizer(
    name: &str,
    lua_path: &str,
    surface_manager: &mut RenderSurfaceManager,
    listeners: &mut GeneratorListeners,
    registry: &mut LuaLayerRegistry,
) {
    info!("Reloading visualizer '{}'...", name);

    // Check if we already have this visualizer
    if let Some(shared) = registry.get_visualizer(name) {
        // Reload in place
        let mut vis = shared.lock();
        if let Err(e) = vis.reload() {
            error!("Failed to reload visualizer '{}': {}", name, e);
        } else {
            info!("Visualizer '{}' reloaded successfully", name);
        }
    } else {
        // Create new visualizer
        let visualizer = LuaVisualizer::new(name, lua_path);
        let shared = SharedVisualizer::new(visualizer);

        // Add to the "grid" surface (overlays on top of base renderer)
        // M10.8 will add separate "mj_structure" surface for node tree visualization
        if let Some(surface) = surface_manager.get_surface_mut("grid") {
            if surface.has_layer(name) {
                surface.replace_layer(Box::new(shared.clone()));
            } else {
                surface.add_layer(Box::new(shared.clone()));
            }
        } else {
            error!(
                "Surface 'grid' not found - cannot add visualizer '{}'",
                name
            );
            return;
        }

        // Register as listener
        listeners.add(Box::new(shared.clone()));

        // Store for future reloads
        registry.store_visualizer(name, shared);

        info!("Visualizer '{}' created and registered", name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_def_renderer() {
        let def = LuaLayerDef::renderer("base", "path/to/file.lua");
        assert_eq!(def.name, "base");
        assert_eq!(def.layer_type, LuaLayerType::Renderer);
        assert_eq!(def.lua_path, "path/to/file.lua");
        assert!(def.tags.is_empty());
    }

    #[test]
    fn test_layer_def_visualizer_with_tags() {
        let def = LuaLayerDef::visualizer("highlight", "path/to/vis.lua")
            .with_tags(vec!["debug".into(), "overlay".into()]);
        assert_eq!(def.name, "highlight");
        assert_eq!(def.layer_type, LuaLayerType::Visualizer);
        assert_eq!(def.tags, vec!["debug", "overlay"]);
    }

    #[test]
    fn test_layer_def_asset_impl() {
        let def = LuaLayerDef::renderer("test", "test.lua");
        assert_eq!(def.name(), "test");
        assert_eq!(LuaLayerDef::asset_type(), "lua_layer");
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = LuaLayerRegistry::new();
        registry.register(LuaLayerDef::renderer("base", "base.lua"));
        registry.register(LuaLayerDef::visualizer("vis", "vis.lua"));

        assert_eq!(registry.list().len(), 2);
        assert!(registry.get("base").is_some());
        assert!(registry.get("vis").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = LuaLayerRegistry::new();
        registry.register(LuaLayerDef::renderer("base", "base.lua"));
        registry.register(LuaLayerDef::visualizer("vis", "vis.lua"));

        let removed = registry.unregister("base");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "base");
        assert_eq!(registry.list().len(), 1);
        assert!(registry.get("base").is_none());
    }

    #[test]
    fn test_registry_layers_of_type() {
        let mut registry = LuaLayerRegistry::new();
        registry.register(LuaLayerDef::renderer("r1", "r1.lua"));
        registry.register(LuaLayerDef::renderer("r2", "r2.lua"));
        registry.register(LuaLayerDef::visualizer("v1", "v1.lua"));

        let renderers = registry.layers_of_type(LuaLayerType::Renderer);
        assert_eq!(renderers.len(), 2);

        let visualizers = registry.layers_of_type(LuaLayerType::Visualizer);
        assert_eq!(visualizers.len(), 1);
    }

    #[test]
    fn test_registry_search() {
        let mut registry = LuaLayerRegistry::new();
        registry
            .register(LuaLayerDef::renderer("grid_2d", "grid.lua").with_tags(vec!["base".into()]));
        registry.register(
            LuaLayerDef::visualizer("step_highlight", "step.lua").with_tags(vec!["debug".into()]),
        );

        // Search by name
        let results = registry.search("grid");
        assert_eq!(results.len(), 1);

        // Search by tag
        let results = registry.search("debug");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "step_highlight");
    }

    #[test]
    fn test_registry_pending_reloads() {
        let mut registry = LuaLayerRegistry::new();
        registry.register(LuaLayerDef::renderer("base", "base.lua"));

        assert!(!registry.has_pending_reloads());

        registry.mark_for_reload("base");
        assert!(registry.has_pending_reloads());

        let pending = registry.take_pending_reloads();
        assert_eq!(pending, vec!["base"]);
        assert!(!registry.has_pending_reloads());
    }

    #[test]
    fn test_registry_mark_all_for_reload() {
        let mut registry = LuaLayerRegistry::new();
        registry.register(LuaLayerDef::renderer("r1", "r1.lua"));
        registry.register(LuaLayerDef::visualizer("v1", "v1.lua"));

        registry.mark_all_for_reload();
        let pending = registry.take_pending_reloads();
        assert_eq!(pending.len(), 2);
    }
}
