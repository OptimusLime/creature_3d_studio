//! Lua-based material loading for the map editor.
//!
//! Loads materials from `assets/map_editor/materials.lua` and provides
//! hot-reload support via file watching.
//!
//! # Lua File Format
//!
//! ```lua
//! return {
//!     { id = 1, name = "stone", color = {0.5, 0.5, 0.5}, tags = {"natural", "terrain"} },
//!     { id = 2, name = "dirt",  color = {0.6, 0.4, 0.2}, tags = {"natural", "terrain"} },
//! }
//! ```
//!
//! The `tags` field is optional. Tags enable search by category (e.g., search "natural" finds all natural materials).

use super::material::{Material, MaterialPalette};
use bevy::prelude::*;
use mlua::{Lua, Result as LuaResult, Value};
use notify::{recommended_watcher, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};

/// System set for materials loading. Runs before other map editor systems.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MaterialsLoadSet;

/// Default path to the materials Lua file.
pub const MATERIALS_LUA_PATH: &str = "assets/map_editor/materials.lua";

/// Plugin that loads materials from Lua and provides hot-reload support.
pub struct LuaMaterialsPlugin {
    /// Path to the materials.lua file.
    pub path: String,
}

impl Default for LuaMaterialsPlugin {
    fn default() -> Self {
        Self {
            path: MATERIALS_LUA_PATH.to_string(),
        }
    }
}

impl LuaMaterialsPlugin {
    /// Create a new plugin with a custom path.
    pub fn with_path(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl Plugin for LuaMaterialsPlugin {
    fn build(&self, app: &mut App) {
        let path = self.path.clone();

        // Insert config resource
        app.insert_resource(LuaMaterialsConfig { path: path.clone() });

        // Insert flag for reload requests
        app.insert_resource(MaterialsReloadFlag { needs_reload: true }); // Start with reload to load initial materials

        // Setup systems
        app.add_systems(Startup, setup_materials_watcher);

        // Materials loading runs in MaterialsLoadSet, which other systems can depend on
        app.add_systems(
            Update,
            (check_materials_reload, reload_materials_from_lua)
                .chain()
                .in_set(MaterialsLoadSet),
        );
    }
}

/// Configuration for Lua materials loading.
#[derive(Resource)]
struct LuaMaterialsConfig {
    path: String,
}

/// Flag to trigger material reload.
#[derive(Resource)]
pub struct MaterialsReloadFlag {
    /// Set to true to trigger a reload on the next frame.
    pub needs_reload: bool,
}

/// Resource holding the file watcher (non-send because notify uses threads).
struct MaterialsWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event, notify::Error>>,
}

/// Setup the file watcher for materials.lua.
fn setup_materials_watcher(world: &mut World) {
    let config = world.resource::<LuaMaterialsConfig>();
    let watch_path = Path::new(&config.path)
        .parent()
        .unwrap_or(Path::new("assets/map_editor"));

    let (tx, rx) = channel();

    let mut watcher = match recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create materials file watcher: {:?}", e);
            return;
        }
    };

    if let Err(e) = watcher.watch(watch_path, RecursiveMode::NonRecursive) {
        error!(
            "Failed to watch materials directory {:?}: {:?}",
            watch_path, e
        );
        return;
    }

    info!(
        "Hot reload enabled for materials at {}",
        world.resource::<LuaMaterialsConfig>().path
    );

    world.insert_non_send_resource(MaterialsWatcher {
        _watcher: watcher,
        receiver: rx,
    });
}

/// Check for file changes and set reload flag.
fn check_materials_reload(
    watcher: Option<NonSend<MaterialsWatcher>>,
    config: Res<LuaMaterialsConfig>,
    mut reload_flag: ResMut<MaterialsReloadFlag>,
) {
    let Some(watcher) = watcher else { return };

    // Drain all events and check if any are for our file
    while let Ok(event) = watcher.receiver.try_recv() {
        if let Ok(event) = event {
            for path in &event.paths {
                // Check if this is our materials file
                if path
                    .file_name()
                    .map(|n| n == "materials.lua")
                    .unwrap_or(false)
                {
                    info!("Detected change in materials.lua, scheduling reload...");
                    reload_flag.needs_reload = true;
                }
            }
        }
    }

    // Also check on first run if file exists
    if reload_flag.needs_reload && !Path::new(&config.path).exists() {
        warn!(
            "Materials file not found at {}, using defaults",
            config.path
        );
    }
}

/// Reload materials from Lua file when flag is set.
fn reload_materials_from_lua(
    config: Res<LuaMaterialsConfig>,
    mut reload_flag: ResMut<MaterialsReloadFlag>,
    mut palette: ResMut<MaterialPalette>,
) {
    if !reload_flag.needs_reload {
        return;
    }
    reload_flag.needs_reload = false;

    match load_materials_from_lua(&config.path) {
        Ok(materials) => {
            let count = materials.len();

            // Update available materials, preserving active palette where possible
            palette.set_available(materials);

            info!(
                "Loaded {} materials from {}, active palette: {:?}",
                count, config.path, palette.active
            );
        }
        Err(e) => {
            error!("Failed to load materials from {}: {}", config.path, e);
            // Keep existing materials on error
        }
    }
}

/// Load materials from a Lua file.
///
/// Returns a vector of materials, or an error if loading fails.
pub fn load_materials_from_lua(path: &str) -> Result<Vec<Material>, String> {
    let src =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))?;

    let lua = Lua::new();
    parse_materials_lua(&lua, &src).map_err(|e| format!("Lua error: {:?}", e))
}

/// Parse materials from Lua source code.
fn parse_materials_lua(lua: &Lua, src: &str) -> LuaResult<Vec<Material>> {
    let value: Value = lua.load(src).eval()?;

    let table = value
        .as_table()
        .ok_or_else(|| mlua::Error::RuntimeError("Materials must be a table".into()))?;

    let mut materials = Vec::new();

    for pair in table.pairs::<i64, Value>() {
        let (_, entry) = pair?;
        let entry_table = entry
            .as_table()
            .ok_or_else(|| mlua::Error::RuntimeError("Each material must be a table".into()))?;

        let id: u32 = entry_table.get("id")?;
        let name: String = entry_table.get("name")?;

        let color_value: Value = entry_table.get("color")?;
        let color_table = color_value
            .as_table()
            .ok_or_else(|| mlua::Error::RuntimeError("color must be a table {r, g, b}".into()))?;

        let r: f32 = color_table.get(1)?;
        let g: f32 = color_table.get(2)?;
        let b: f32 = color_table.get(3)?;

        // Parse optional tags
        let tags: Vec<String> = match entry_table.get::<Value>("tags") {
            Ok(Value::Table(tags_table)) => {
                let mut tags = Vec::new();
                for pair in tags_table.pairs::<i64, String>() {
                    if let Ok((_, tag)) = pair {
                        tags.push(tag);
                    }
                }
                tags
            }
            _ => Vec::new(), // No tags or invalid format - use empty vec
        };

        // Parse optional mj_char (MarkovJunior palette character binding)
        let mj_char: Option<char> = match entry_table.get::<String>("mj_char") {
            Ok(s) if !s.is_empty() => s.chars().next(),
            _ => None,
        };

        let material = if let Some(ch) = mj_char {
            Material::with_mj_char(id, name, [r, g, b], tags, ch)
        } else {
            Material::with_tags(id, name, [r, g, b], tags)
        };
        materials.push(material);
    }

    // Sort by ID for consistent ordering
    materials.sort_by_key(|m| m.id);

    Ok(materials)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_materials_lua() {
        let lua = Lua::new();
        let src = r#"
            return {
                { id = 1, name = "stone", color = {0.5, 0.5, 0.5} },
                { id = 2, name = "dirt", color = {0.6, 0.4, 0.2} },
            }
        "#;

        let materials = parse_materials_lua(&lua, src).expect("Should parse");
        assert_eq!(materials.len(), 2);
        assert_eq!(materials[0].id, 1);
        assert_eq!(materials[0].name, "stone");
        assert_eq!(materials[0].color, [0.5, 0.5, 0.5]);
        assert!(materials[0].tags.is_empty()); // No tags
        assert_eq!(materials[1].id, 2);
        assert_eq!(materials[1].name, "dirt");
    }

    #[test]
    fn test_parse_materials_with_tags() {
        let lua = Lua::new();
        let src = r#"
            return {
                { id = 1, name = "stone", color = {0.5, 0.5, 0.5}, tags = {"natural", "terrain"} },
                { id = 2, name = "metal", color = {0.7, 0.7, 0.8}, tags = {"industrial"} },
            }
        "#;

        let materials = parse_materials_lua(&lua, src).expect("Should parse");
        assert_eq!(materials.len(), 2);
        assert_eq!(materials[0].tags, vec!["natural", "terrain"]);
        assert_eq!(materials[1].tags, vec!["industrial"]);
    }

    #[test]
    fn test_parse_materials_with_extra_fields() {
        let lua = Lua::new();
        let src = r#"
            return {
                { id = 1, name = "stone", color = {0.5, 0.5, 0.5}, roughness = 0.7 },
            }
        "#;

        let materials = parse_materials_lua(&lua, src).expect("Should parse despite extra fields");
        assert_eq!(materials.len(), 1);
        assert_eq!(materials[0].name, "stone");
    }

    #[test]
    fn test_parse_materials_invalid_format() {
        let lua = Lua::new();
        let src = r#"return "not a table""#;

        let result = parse_materials_lua(&lua, src);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_materials_missing_field() {
        let lua = Lua::new();
        let src = r#"
            return {
                { id = 1, name = "stone" }, -- missing color
            }
        "#;

        let result = parse_materials_lua(&lua, src);
        assert!(result.is_err());
    }
}
