//! Lua API for MarkovJunior procedural generation.
//!
//! This module exposes MarkovJunior functionality to Lua scripts, enabling:
//! - Loading models from XML files
//! - Creating models programmatically
//! - Running generation with seeds
//! - Accessing grid results
//!
//! # Example
//!
//! ```lua
//! -- Load an existing XML model
//! local maze = mj.load_model("MarkovJunior/models/MazeBacktracker.xml")
//! maze:run(12345)
//! local grid = maze:grid()
//! print("Generated " .. grid:count_nonzero() .. " cells")
//!
//! -- Create a model programmatically
//! local model = mj.create_model({
//!     values = "BW",
//!     size = {10, 10, 1},
//!     origin = true
//! })
//! model:one("WB", "WW")  -- Growth rule
//! model:run(42)
//! ```
//!
//! # API Reference
//!
//! ## Module: `mj`
//!
//! - `mj.load_model(path)` - Load model from XML file, returns MjModel
//! - `mj.create_model(config)` - Create model programmatically, returns MjModelBuilder
//!
//! ## MjModel (from load_model)
//!
//! - `model:run(seed, [max_steps])` - Run model, returns step count
//! - `model:step()` - Execute single step, returns true if progress made
//! - `model:reset(seed)` - Reset for new run
//! - `model:grid()` - Get current grid state as MjLuaGrid
//! - `model:is_running()` - Check if model is still running
//! - `model:counter()` - Get current step count
//!
//! ## MjLuaGrid
//!
//! - `grid:get(x, y, z)` - Get value at position (0-indexed)
//! - `grid:count_nonzero()` - Count non-zero cells
//! - `grid:count_value(char)` - Count cells with specific value
//! - `grid:size()` - Returns {mx, my, mz}
//! - `grid:to_table()` - Convert to nested Lua table

use mlua::{Lua, ObjectLike, Result as LuaResult, UserData, UserDataMethods};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use super::model::Model;
use super::MjGrid;

/// Register the MarkovJunior Lua API.
///
/// This creates a global `mj` table with functions for loading and creating models.
///
/// # Example
///
/// ```ignore
/// use mlua::Lua;
/// use studio_core::markov_junior::lua_api::register_markov_junior_api;
///
/// let lua = Lua::new();
/// register_markov_junior_api(&lua).unwrap();
///
/// lua.load(r#"
///     local model = mj.load_model("MarkovJunior/models/Basic.xml")
///     model:run(42)
/// "#).exec().unwrap();
/// ```
pub fn register_markov_junior_api(lua: &Lua) -> LuaResult<()> {
    let mj = lua.create_table()?;

    // mj.load_model(path) -> MjLuaModel
    mj.set(
        "load_model",
        lua.create_function(|_, path: String| {
            let model = Model::load(&path).map_err(|e| {
                mlua::Error::RuntimeError(format!("Failed to load model '{}': {}", path, e))
            })?;

            Ok(MjLuaModel {
                inner: Rc::new(RefCell::new(model)),
                path: Rc::new(RefCell::new("root".to_string())),
                ctx: Rc::new(RefCell::new(None)),
            })
        })?,
    )?;

    // mj.create_model(config) -> MjLuaModelBuilder
    // config = { values = "BW", size = {mx, my, mz}, origin = bool }
    mj.set(
        "create_model",
        lua.create_function(|_, config: mlua::Table| {
            // Extract values string (required)
            let values: String = config.get("values").map_err(|_| {
                mlua::Error::RuntimeError("create_model requires 'values' string".into())
            })?;

            // Extract size (required)
            let size: mlua::Table = config.get("size").map_err(|_| {
                mlua::Error::RuntimeError("create_model requires 'size' table {mx, my, mz}".into())
            })?;

            let mx: usize = size.get(1).unwrap_or(16);
            let my: usize = size.get(2).unwrap_or(16);
            let mz: usize = size.get(3).unwrap_or(1);

            // Extract origin (optional, default false)
            let origin: bool = config.get("origin").unwrap_or(false);

            // Create the builder
            let grid = MjGrid::try_with_values(mx, my, mz, &values)
                .map_err(|e| mlua::Error::RuntimeError(format!("Invalid values string: {}", e)))?;

            Ok(MjLuaModelBuilder {
                grid,
                origin,
                rules: Vec::new(),
                node_type: NodeType::Markov, // Default to markov
            })
        })?,
    )?;

    // mj.list_models_with_refs() -> table of models with reference images
    // Returns models that have corresponding images in assets/reference_images/mj/
    mj.set(
        "list_models_with_refs",
        lua.create_function(|lua, ()| {
            let models_dir = PathBuf::from("MarkovJunior/models");
            let refs_dir = PathBuf::from("assets/reference_images/mj");

            let result = lua.create_table()?;

            // Check if directories exist
            if !models_dir.exists() || !refs_dir.exists() {
                return Ok(result);
            }

            // Get list of reference images (strip extension to get base name)
            let mut ref_names: std::collections::HashSet<String> = std::collections::HashSet::new();
            if let Ok(entries) = std::fs::read_dir(&refs_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        if ext == "gif" || ext == "png" {
                            if let Some(stem) = path.file_stem() {
                                ref_names.insert(stem.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }

            // Load models.xml to get size info
            let models_xml_path = PathBuf::from("MarkovJunior/models.xml");
            let model_configs = parse_models_xml(&models_xml_path);

            // Scan models directory for matching XMLs
            let mut idx = 1;
            if let Ok(entries) = std::fs::read_dir(&models_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "xml").unwrap_or(false) {
                        if let Some(stem) = path.file_stem() {
                            let name = stem.to_string_lossy().to_string();
                            if ref_names.contains(&name) {
                                // Find matching reference image
                                let ref_path = if refs_dir.join(format!("{}.png", name)).exists() {
                                    refs_dir.join(format!("{}.png", name))
                                } else {
                                    refs_dir.join(format!("{}.gif", name))
                                };

                                // Look up size from models.xml
                                let (size, is_3d) =
                                    model_configs.get(&name).cloned().unwrap_or((60, false));

                                let entry_table = lua.create_table()?;
                                entry_table.set("name", name.clone())?;
                                entry_table.set("xml_path", path.display().to_string())?;
                                entry_table.set("ref_path", ref_path.display().to_string())?;
                                entry_table.set("size", size)?;
                                entry_table.set("is_3d", is_3d)?;

                                result.set(idx, entry_table)?;
                                idx += 1;
                            }
                        }
                    }
                }
            }

            Ok(result)
        })?,
    )?;

    // mj.load_model_xml(path, options) -> MjLuaModel
    // Load model from XML with optional size override
    // options = { size = 60 } or { mx = 32, my = 32, mz = 1 }
    mj.set(
        "load_model_xml",
        lua.create_function(|_, (path, options): (String, Option<mlua::Table>)| {
            let (mx, my, mz) = if let Some(opts) = options {
                // Check for 'size' (square grid)
                if let Ok(size) = opts.get::<usize>("size") {
                    let mz: usize = opts.get("mz").unwrap_or(1);
                    (size, size, mz)
                } else {
                    // Check for individual dimensions
                    let mx: usize = opts.get("mx").unwrap_or(16);
                    let my: usize = opts.get("my").unwrap_or(16);
                    let mz: usize = opts.get("mz").unwrap_or(1);
                    (mx, my, mz)
                }
            } else {
                (16, 16, 1)
            };

            let model = Model::load_with_size(&path, mx, my, mz).map_err(|e| {
                mlua::Error::RuntimeError(format!("Failed to load model '{}': {}", path, e))
            })?;

            Ok(MjLuaModel {
                inner: Rc::new(RefCell::new(model)),
                path: Rc::new(RefCell::new("root".to_string())),
                ctx: Rc::new(RefCell::new(None)),
            })
        })?,
    )?;

    lua.globals().set("mj", mj)?;

    Ok(())
}

/// Parse models.xml to extract size information for each model.
/// Returns a map of model name -> (size, is_3d).
fn parse_models_xml(path: &PathBuf) -> std::collections::HashMap<String, (usize, bool)> {
    let mut result = std::collections::HashMap::new();

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return result,
    };

    // Simple XML parsing - look for <model name="..." size="..." d="3"?>
    for line in content.lines() {
        if !line.contains("<model") {
            continue;
        }

        // Extract name
        let name = if let Some(start) = line.find("name=\"") {
            let rest = &line[start + 6..];
            if let Some(end) = rest.find('"') {
                rest[..end].to_string()
            } else {
                continue;
            }
        } else {
            continue;
        };

        // Extract size
        let size = if let Some(start) = line.find("size=\"") {
            let rest = &line[start + 6..];
            if let Some(end) = rest.find('"') {
                rest[..end].parse().unwrap_or(60)
            } else {
                60
            }
        } else {
            60
        };

        // Check if 3D (has d="3")
        let is_3d = line.contains("d=\"3\"");

        // Only store if not already present (first entry wins for 2D)
        // We prefer 2D configurations for verification
        if !result.contains_key(&name) && !is_3d {
            result.insert(name, (size, is_3d));
        }
    }

    result
}

/// Wrapper around Model for Lua userdata.
///
/// Uses Rc<RefCell<>> to allow shared ownership between Lua values.
/// Supports scene tree integration via _path and _type fields.
#[derive(Clone)]
struct MjLuaModel {
    inner: Rc<RefCell<Model>>,
    /// Scene tree path (e.g., "root.step_1")
    path: Rc<RefCell<String>>,
    /// Generator context for emitting step info (optional)
    ctx: Rc<RefCell<Option<mlua::RegistryKey>>>,
}

impl UserData for MjLuaModel {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        // _type field for scene tree integration
        fields.add_field_method_get("_type", |_, _| Ok("MjModel"));

        // _path field for scene tree integration
        fields.add_field_method_get("_path", |_, this| Ok(this.path.borrow().clone()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // _set_path(path) - Set scene tree path (called by parent generator)
        methods.add_method("_set_path", |_, this, path: String| {
            *this.path.borrow_mut() = path;
            Ok(())
        });

        // _set_context(ctx) - Set generator context for emitting step info
        methods.add_method("_set_context", |lua, this, ctx: mlua::AnyUserData| {
            // Store a registry key to the context
            let key = lua.create_registry_value(ctx)?;
            *this.ctx.borrow_mut() = Some(key);
            Ok(())
        });

        // is_done() -> bool - For scene tree compatibility
        methods.add_method("is_done", |_, this, ()| {
            Ok(!this.inner.borrow().is_running())
        });

        // get_structure() -> table - For scene tree introspection
        methods.add_method("get_structure", |lua, this, ()| {
            let table = lua.create_table()?;
            table.set("type", "MjModel")?;
            table.set("path", this.path.borrow().clone())?;
            table.set("model", this.inner.borrow().name.clone())?;
            table.set("step", this.inner.borrow().counter())?;
            table.set("running", this.inner.borrow().is_running())?;
            Ok(table)
        });

        // model:run(seed, [max_steps]) -> steps
        methods.add_method("run", |_, this, args: (u64, Option<usize>)| {
            let (seed, max_steps) = args;
            let max_steps = max_steps.unwrap_or(0);
            let steps = this.inner.borrow_mut().run(seed, max_steps);
            Ok(steps)
        });

        // model:step() -> bool
        // After stepping, emit step info if context is available
        methods.add_method("step", |lua, this, ()| {
            let result = this.inner.borrow_mut().step();

            // Emit step info if we have a context
            if let Some(ref key) = *this.ctx.borrow() {
                if let Ok(ctx) = lua.registry_value::<mlua::AnyUserData>(key) {
                    // Get step info from the model
                    let model = this.inner.borrow();
                    let change_count = model.last_step_change_count();
                    let step_num = model.counter();
                    let path = this.path.borrow().clone();

                    // Get position of first changed cell (for x, y fields)
                    let (x, y) = if let Some(&(cx, cy, _)) = model.last_step_changes().first() {
                        (cx as usize, cy as usize)
                    } else {
                        (0, 0)
                    };

                    // Get the material at the changed position
                    let material_id: u32 = if change_count > 0 {
                        let (cx, cy, cz) = model.last_step_changes()[0];
                        model
                            .grid()
                            .get(cx as usize, cy as usize, cz as usize)
                            .unwrap_or(0) as u32
                    } else {
                        0
                    };

                    let completed = !model.is_running();
                    drop(model); // Release borrow before calling Lua

                    // Create info table
                    if let Ok(info) = lua.create_table() {
                        let _ = info.set("step_number", step_num);
                        let _ = info.set("x", x);
                        let _ = info.set("y", y);
                        let _ = info.set("material_id", material_id);
                        let _ = info.set("completed", completed);
                        let _ = info.set("affected_cells", change_count);
                        // rule_name not available from interpreter architecture

                        // Call ctx:emit_step(path, info)
                        if let Ok(emit_fn) = ctx.get::<mlua::Function>("emit_step") {
                            let _ = emit_fn.call::<()>((ctx.clone(), path, info));
                        }
                    }
                }
            }

            Ok(result)
        });

        // model:reset(seed)
        methods.add_method("reset", |_, this, seed: u64| {
            this.inner.borrow_mut().reset(seed);
            Ok(())
        });

        // model:grid() -> MjLuaGrid
        methods.add_method("grid", |_, this, ()| {
            // Clone the grid for Lua access (safe copy)
            let grid = this.inner.borrow().grid().clone();
            Ok(MjLuaGrid { inner: grid })
        });

        // model:is_running() -> bool
        methods.add_method("is_running", |_, this, ()| {
            Ok(this.inner.borrow().is_running())
        });

        // model:counter() -> usize
        methods.add_method("counter", |_, this, ()| Ok(this.inner.borrow().counter()));

        // model:name() -> string
        methods.add_method("name", |_, this, ()| Ok(this.inner.borrow().name.clone()));

        // model:run_animated(config) -> steps
        // config = { seed, max_steps, on_step, on_complete }
        // on_step(grid, step) - called after each successful step
        // on_complete(grid, steps) - called when model finishes
        //
        // C# Reference: Interpreter.cs lines 52-82 uses IEnumerable with yield.
        // Rust uses callbacks instead (documented deviation).
        methods.add_method("run_animated", |_lua, this, config: mlua::Table| {
            let seed: u64 = config.get("seed").map_err(|_| {
                mlua::Error::RuntimeError("run_animated requires 'seed' parameter".into())
            })?;
            let max_steps: usize = config.get("max_steps").unwrap_or(0);
            let on_step: Option<mlua::Function> = config.get("on_step").ok();
            let on_complete: Option<mlua::Function> = config.get("on_complete").ok();

            // Reset the model with the seed
            this.inner.borrow_mut().reset(seed);

            let mut step_count = 0;

            // Step through execution, calling on_step after each successful step
            loop {
                let made_progress = this.inner.borrow_mut().step();

                if made_progress {
                    step_count += 1;

                    // Call on_step callback if provided
                    if let Some(ref callback) = on_step {
                        // Clone grid for safe Lua access
                        let grid = this.inner.borrow().grid().clone();
                        let lua_grid = MjLuaGrid { inner: grid };
                        callback.call::<()>((lua_grid, step_count))?;
                    }

                    // Check max_steps limit
                    if max_steps > 0 && step_count >= max_steps {
                        break;
                    }
                } else {
                    // Model completed naturally
                    break;
                }
            }

            // Call on_complete callback if provided
            if let Some(callback) = on_complete {
                let grid = this.inner.borrow().grid().clone();
                let lua_grid = MjLuaGrid { inner: grid };
                callback.call::<()>((lua_grid, step_count))?;
            }

            Ok(step_count)
        });

        // model:changes() -> array of {x, y, z} tables
        // Returns all positions that have changed during execution.
        //
        // C# Reference: Interpreter.cs line 16: public List<(int, int, int)> changes;
        methods.add_method("changes", |lua, this, ()| {
            let model = this.inner.borrow();
            let changes = model.interpreter.changes();
            let table = lua.create_table()?;
            for (i, &(x, y, z)) in changes.iter().enumerate() {
                let pos = lua.create_table()?;
                pos.set("x", x)?;
                pos.set("y", y)?;
                pos.set("z", z)?;
                table.set(i + 1, pos)?; // Lua 1-indexed
            }
            Ok(table)
        });

        // model:last_changes() -> array of {x, y, z} tables
        // Returns positions changed in the most recent step only.
        // Uses the 'first' array to determine the boundary.
        methods.add_method("last_changes", |lua, this, ()| {
            let model = this.inner.borrow();
            let changes = model.interpreter.changes();
            let first = model.interpreter.first();

            // Get the start index for the last step's changes
            let start_idx = if first.len() >= 2 {
                first[first.len() - 2]
            } else {
                0
            };

            let table = lua.create_table()?;
            for (i, &(x, y, z)) in changes.iter().skip(start_idx).enumerate() {
                let pos = lua.create_table()?;
                pos.set("x", x)?;
                pos.set("y", y)?;
                pos.set("z", z)?;
                table.set(i + 1, pos)?; // Lua 1-indexed
            }
            Ok(table)
        });
    }
}

/// Grid wrapper for Lua access.
///
/// This is a copy of the grid state, safe for Lua to hold.
struct MjLuaGrid {
    inner: MjGrid,
}

impl UserData for MjLuaGrid {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // grid:get(x, y, z) -> value or nil
        methods.add_method("get", |_, this, (x, y, z): (usize, usize, usize)| {
            Ok(this.inner.get(x, y, z))
        });

        // grid:count_nonzero() -> count
        methods.add_method("count_nonzero", |_, this, ()| {
            Ok(this.inner.count_nonzero())
        });

        // grid:count_value(char) -> count
        methods.add_method("count_value", |_, this, ch: String| {
            let ch = ch.chars().next().ok_or_else(|| {
                mlua::Error::RuntimeError("count_value requires a single character".into())
            })?;

            let value = this.inner.values.get(&ch).ok_or_else(|| {
                mlua::Error::RuntimeError(format!("Unknown value character: '{}'", ch))
            })?;

            let count = this.inner.state.iter().filter(|&&v| v == *value).count();
            Ok(count)
        });

        // grid:size() -> {mx, my, mz}
        methods.add_method("size", |lua, this, ()| {
            let table = lua.create_table()?;
            table.set(1, this.inner.mx)?;
            table.set(2, this.inner.my)?;
            table.set(3, this.inner.mz)?;
            Ok(table)
        });

        // grid:values() -> string of value characters
        methods.add_method("values", |_, this, ()| {
            let values: String = this.inner.characters.iter().collect();
            Ok(values)
        });

        // grid:to_table() -> nested table [z][y][x]
        methods.add_method("to_table", |lua, this, ()| {
            let table = lua.create_table()?;

            for z in 0..this.inner.mz {
                let z_table = lua.create_table()?;
                for y in 0..this.inner.my {
                    let y_table = lua.create_table()?;
                    for x in 0..this.inner.mx {
                        let idx = x + y * this.inner.mx + z * this.inner.mx * this.inner.my;
                        y_table.set(x + 1, this.inner.state[idx])?; // Lua 1-indexed
                    }
                    z_table.set(y + 1, y_table)?;
                }
                table.set(z + 1, z_table)?;
            }

            Ok(table)
        });

        // grid:to_voxels() -> array of {x, y, z, r, g, b, e} tables
        // Converts non-zero grid cells to voxel data using default palette
        methods.add_method("to_voxels", |lua, this, ()| {
            use super::voxel_bridge::MjPalette;

            let palette = MjPalette::default();
            let result = lua.create_table()?;
            let mut idx = 1;

            // Center offset so grid is centered at origin
            let offset_x = (this.inner.mx / 2) as i32;
            let offset_y = (this.inner.my / 2) as i32;
            let offset_z = (this.inner.mz / 2) as i32;

            for (x, y, z, value) in this.inner.iter_nonzero() {
                if let Some(voxel) = palette.get(value) {
                    let voxel_table = lua.create_table()?;
                    voxel_table.set("x", x as i32 - offset_x)?;
                    voxel_table.set("y", y as i32 - offset_y)?;
                    voxel_table.set("z", z as i32 - offset_z)?;
                    voxel_table.set("r", voxel.color[0])?;
                    voxel_table.set("g", voxel.color[1])?;
                    voxel_table.set("b", voxel.color[2])?;
                    voxel_table.set("e", voxel.emission)?;
                    result.set(idx, voxel_table)?;
                    idx += 1;
                }
            }

            Ok(result)
        });

        // grid:to_voxel_world() -> VoxelWorld userdata (for direct scene integration)
        // Returns a lightweight reference that can be passed to scene.set_voxel_world()
        // Uses MjPalette::from_grid() to map characters to proper palette.xml colors.
        methods.add_method("to_voxel_world", |_, this, ()| {
            use super::voxel_bridge::MjPalette;

            // Use from_grid to get proper character->color mapping from palette.xml
            let palette = MjPalette::from_grid(&this.inner);
            let world = this.inner.to_voxel_world(&palette);
            Ok(MjLuaVoxelWorld { inner: world })
        });

        // grid:render_to_png(path, [pixel_size]) -> bool
        // Renders the grid to a PNG file at the given path.
        // Uses proper colors from palette.xml based on grid's character set.
        // pixel_size defaults to 4 for 2D grids, 8 for 3D isometric.
        //
        // C# Reference: Graphics.cs Render() dispatches to BitmapRender or IsometricRender
        methods.add_method(
            "render_to_png",
            |_, this, (path, pixel_size): (String, Option<u32>)| {
                use super::render::render_to_png;
                use std::path::Path;

                // Default pixel size: 4 for 2D, 8 for 3D
                let pixel_size = pixel_size.unwrap_or(if this.inner.mz == 1 { 4 } else { 8 });

                let path = Path::new(&path);
                render_to_png(&this.inner, path, pixel_size).map_err(|e| {
                    mlua::Error::RuntimeError(format!(
                        "Failed to save PNG '{}': {}",
                        path.display(),
                        e
                    ))
                })?;

                Ok(true)
            },
        );

        // grid:render_to_rgba([pixel_size]) -> {data: bytes, width: int, height: int}
        // Renders the grid to RGBA bytes (for ImGui display).
        // Returns a table with data (string of bytes), width, and height.
        methods.add_method("render_to_rgba", |lua, this, pixel_size: Option<u32>| {
            use super::render::{colors_for_grid, render_2d, render_3d_isometric};

            // Default pixel size: 4 for 2D, 8 for 3D
            let pixel_size = pixel_size.unwrap_or(if this.inner.mz == 1 { 4 } else { 8 });

            let colors = colors_for_grid(&this.inner);
            let img = if this.inner.mz == 1 {
                render_2d(&this.inner, &colors, pixel_size, None)
            } else {
                render_3d_isometric(&this.inner, &colors, pixel_size)
            };

            let width = img.width();
            let height = img.height();
            let data: Vec<u8> = img.into_raw();

            let result = lua.create_table()?;
            // Convert to Lua string (binary-safe)
            result.set("data", lua.create_string(&data)?)?;
            result.set("width", width)?;
            result.set("height", height)?;

            Ok(result)
        });
    }
}

/// VoxelWorld wrapper for Lua.
/// This can be passed directly to scene functions.
pub struct MjLuaVoxelWorld {
    inner: crate::voxel::VoxelWorld,
}

impl MjLuaVoxelWorld {
    /// Extract the inner VoxelWorld, consuming this wrapper.
    pub fn into_inner(self) -> crate::voxel::VoxelWorld {
        self.inner
    }
}

impl UserData for MjLuaVoxelWorld {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // world:voxel_count() -> count
        methods.add_method("voxel_count", |_, this, ()| {
            Ok(this.inner.total_voxel_count())
        });

        // world:chunk_count() -> count
        methods.add_method("chunk_count", |_, this, ()| Ok(this.inner.chunk_count()));
    }
}

/// Node type for programmatic model building.
#[derive(Debug, Clone, Copy, PartialEq)]
enum NodeType {
    Markov,
    Sequence,
    One,
    All,
}

/// Rule definition for programmatic model building.
#[derive(Debug, Clone)]
struct RuleDef {
    input: String,
    output: String,
}

/// Builder for creating models programmatically.
///
/// This allows defining rules in Lua before creating the final model.
struct MjLuaModelBuilder {
    grid: MjGrid,
    origin: bool,
    rules: Vec<RuleDef>,
    node_type: NodeType,
}

impl UserData for MjLuaModelBuilder {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // builder:one(input, output) - add a OneNode rule
        methods.add_method_mut("one", |_, this, (input, output): (String, String)| {
            this.rules.push(RuleDef { input, output });
            // For simple API, first rule call sets node type
            if this.rules.len() == 1 {
                this.node_type = NodeType::One;
            }
            Ok(())
        });

        // builder:all(input, output) - add an AllNode rule
        methods.add_method_mut("all", |_, this, (input, output): (String, String)| {
            this.rules.push(RuleDef { input, output });
            if this.rules.len() == 1 {
                this.node_type = NodeType::All;
            }
            Ok(())
        });

        // builder:run(seed, [max_steps]) -> MjLuaGrid
        // Builds the model, runs it, and returns the grid
        methods.add_method_mut("run", |_, this, args: (u64, Option<usize>)| {
            let (seed, max_steps) = args;
            let max_steps = max_steps.unwrap_or(0);

            // Build the model from rules
            let model = build_model_from_builder(this)?;
            let model_rc = Rc::new(RefCell::new(model));

            // Run it
            model_rc.borrow_mut().run(seed, max_steps);

            // Return the grid
            let grid = model_rc.borrow().grid().clone();
            Ok(MjLuaGrid { inner: grid })
        });

        // builder:build() -> MjLuaModel
        // Build the model without running it
        methods.add_method_mut("build", |_, this, ()| {
            let model = build_model_from_builder(this)?;
            Ok(MjLuaModel {
                inner: Rc::new(RefCell::new(model)),
                path: Rc::new(RefCell::new("root".to_string())),
                ctx: Rc::new(RefCell::new(None)),
            })
        });

        // builder:grid() -> MjLuaGrid (returns the initial grid for inspection)
        methods.add_method("grid", |_, this, ()| {
            Ok(MjLuaGrid {
                inner: this.grid.clone(),
            })
        });
    }
}

/// Build a Model from the builder state.
fn build_model_from_builder(builder: &MjLuaModelBuilder) -> LuaResult<Model> {
    use super::all_node::AllNode;
    use super::interpreter::Interpreter;
    use super::node::MarkovNode;
    use super::one_node::OneNode;
    use super::rule::MjRule;

    if builder.rules.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "Model has no rules defined. Call :one() or :all() first.".into(),
        ));
    }

    // Parse all rules
    let mut parsed_rules = Vec::new();
    for rule_def in &builder.rules {
        let rule =
            MjRule::parse(&rule_def.input, &rule_def.output, &builder.grid).map_err(|e| {
                mlua::Error::RuntimeError(format!(
                    "Invalid rule '{}' -> '{}': {}",
                    rule_def.input, rule_def.output, e
                ))
            })?;
        parsed_rules.push(rule);
    }

    let grid_size = builder.grid.state.len();

    // Create the appropriate node type
    let root: Box<dyn super::node::Node> = match builder.node_type {
        NodeType::One | NodeType::Markov => {
            // Wrap OneNode in MarkovNode for looping behavior
            let one = OneNode::new(parsed_rules, grid_size);
            Box::new(MarkovNode::new(vec![Box::new(one)]))
        }
        NodeType::All => {
            let all = AllNode::new(parsed_rules, grid_size);
            Box::new(MarkovNode::new(vec![Box::new(all)]))
        }
        NodeType::Sequence => {
            // For sequence, each rule gets its own OneNode
            let nodes: Vec<Box<dyn super::node::Node>> = parsed_rules
                .into_iter()
                .map(|r| -> Box<dyn super::node::Node> {
                    let one = OneNode::new(vec![r], grid_size);
                    Box::new(MarkovNode::new(vec![Box::new(one)]))
                })
                .collect();
            Box::new(super::node::SequenceNode::new(nodes))
        }
    };

    // Create interpreter
    let interpreter = if builder.origin {
        Interpreter::with_origin(root, builder.grid.clone())
    } else {
        Interpreter::new(root, builder.grid.clone())
    };

    // Wrap in Model (we need to create it directly since we have an interpreter)
    Ok(Model {
        name: "lua_model".to_string(),
        interpreter,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that mj table is created with expected functions.
    #[test]
    fn test_register_creates_mj_table() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let result: bool = lua
            .load("return mj ~= nil and mj.load_model ~= nil and mj.create_model ~= nil")
            .eval()
            .unwrap();
        assert!(result, "mj table should have load_model and create_model");
    }

    /// Test loading a model from XML.
    #[test]
    fn test_load_model_basic() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        // Get the path to Basic.xml
        let models_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MarkovJunior/models/Basic.xml");

        let script = format!(
            r#"
            local model = mj.load_model("{}")
            model:run(42)
            return model:grid():count_nonzero()
            "#,
            models_path.display().to_string().replace('\\', "/")
        );

        let count: usize = lua.load(&script).eval().unwrap();
        // Basic.xml converts all cells to W (value 1), so all 256 should be non-zero
        assert_eq!(count, 256, "Basic.xml should fill entire 16x16 grid");
    }

    /// Test model step-by-step execution.
    #[test]
    fn test_model_step_by_step() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let models_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MarkovJunior/models/Basic.xml");

        let script = format!(
            r#"
            local model = mj.load_model("{}")
            model:reset(123)
            local steps = 0
            while model:step() and steps < 10 do
                steps = steps + 1
            end
            return steps
            "#,
            models_path.display().to_string().replace('\\', "/")
        );

        let steps: usize = lua.load(&script).eval().unwrap();
        assert_eq!(steps, 10, "Should execute exactly 10 steps");
    }

    /// Test grid access methods.
    #[test]
    fn test_grid_methods() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let models_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MarkovJunior/models/Basic.xml");

        let script = format!(
            r#"
            local model = mj.load_model("{}")
            model:run(42, 10)  -- Run only 10 steps
            local grid = model:grid()
            local size = grid:size()
            local w_count = grid:count_value("W")
            return {{size[1], size[2], size[3], w_count}}
            "#,
            models_path.display().to_string().replace('\\', "/")
        );

        let result: mlua::Table = lua.load(&script).eval().unwrap();
        let mx: usize = result.get(1).unwrap();
        let my: usize = result.get(2).unwrap();
        let mz: usize = result.get(3).unwrap();
        let w_count: usize = result.get(4).unwrap();

        assert_eq!(mx, 16);
        assert_eq!(my, 16);
        assert_eq!(mz, 1);
        assert_eq!(w_count, 10, "Should have 10 W cells after 10 steps");
    }

    /// Test create_model with simple rules.
    #[test]
    fn test_create_model_basic() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local model = mj.create_model({
                values = "BW",
                size = {5, 5, 1}
            })
            model:one("B", "W")
            local grid = model:run(42)
            return grid:count_value("W")
        "#;

        let count: usize = lua.load(script).eval().unwrap();
        assert_eq!(count, 25, "All 25 cells should be W");
    }

    /// Test create_model with origin.
    #[test]
    fn test_create_model_with_origin() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local model = mj.create_model({
                values = "BW",
                size = {5, 5, 1},
                origin = true
            })
            model:one("WB", "WW")  -- Growth from center
            local grid = model:run(42)
            return grid:count_value("W")
        "#;

        let count: usize = lua.load(script).eval().unwrap();
        // With origin, center starts as W (value 1) and grows
        assert!(count > 1, "Should have grown from origin");
        assert!(count <= 25, "Cannot exceed grid size");
    }

    /// Test model:build() returns a runnable model.
    #[test]
    fn test_create_model_build() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local builder = mj.create_model({
                values = "BW",
                size = {3, 3, 1}
            })
            builder:one("B", "W")
            local model = builder:build()
            model:run(42)
            return model:grid():count_nonzero()
        "#;

        let count: usize = lua.load(script).eval().unwrap();
        assert_eq!(count, 9, "All 9 cells should be non-zero (W)");
    }

    /// Test error handling for invalid model path.
    #[test]
    fn test_load_model_error() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let result: Result<(), _> = lua.load(r#"mj.load_model("nonexistent_model.xml")"#).exec();

        assert!(result.is_err(), "Should error on missing file");
    }

    /// Test error handling for model with no rules.
    #[test]
    fn test_create_model_no_rules_error() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let result: Result<(), _> = lua
            .load(
                r#"
                local model = mj.create_model({ values = "BW", size = {5, 5, 1} })
                model:run(42)  -- No rules defined!
            "#,
            )
            .exec();

        assert!(result.is_err(), "Should error when no rules defined");
    }

    /// Test grid:to_table() produces correct structure.
    #[test]
    fn test_grid_to_table() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local model = mj.create_model({
                values = "BW",
                size = {2, 2, 1}
            })
            model:one("B", "W")
            local grid = model:run(42)
            local t = grid:to_table()
            -- t[z][y][x] with 1-indexing
            return t[1][1][1] + t[1][1][2] + t[1][2][1] + t[1][2][2]
        "#;

        let sum: u8 = lua.load(script).eval().unwrap();
        // All cells are W (value 1), so sum should be 4
        assert_eq!(sum, 4, "All cells should be value 1 (W)");
    }

    /// Test count_value with different characters.
    #[test]
    fn test_count_value_multiple_chars() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local model = mj.create_model({
                values = "BRW",
                size = {6, 1, 1}
            })
            -- Convert first 2 B's to R, then run out of matches
            model:one("BB", "RR")
            local grid = model:run(42)
            local b = grid:count_value("B")
            local r = grid:count_value("R")
            return {b, r}
        "#;

        let result: mlua::Table = lua.load(script).eval().unwrap();
        let b_count: usize = result.get(1).unwrap();
        let r_count: usize = result.get(2).unwrap();

        // 6 cells, BB->RR converts pairs, leaving some pattern
        assert!(r_count > 0, "Should have some R cells");
        assert_eq!(b_count + r_count, 6, "Total should be 6");
    }

    /// Test grid:to_voxels() returns array of voxel data.
    #[test]
    fn test_grid_to_voxels() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local model = mj.create_model({
                values = "BW",
                size = {3, 3, 1}
            })
            model:one("B", "W")
            local grid = model:run(42)
            local voxels = grid:to_voxels()
            
            -- Check structure
            local count = #voxels
            local first = voxels[1]
            return {count, first.r, first.g, first.b}
        "#;

        let result: mlua::Table = lua.load(script).eval().unwrap();
        let count: usize = result.get(1).unwrap();
        let r: u8 = result.get(2).unwrap();
        let g: u8 = result.get(3).unwrap();
        let b: u8 = result.get(4).unwrap();

        // All 9 cells should be W (value 1), which maps to white in default palette
        assert_eq!(count, 9, "Should have 9 voxels");
        assert_eq!(r, 255, "White voxel should have r=255");
        assert_eq!(g, 255, "White voxel should have g=255");
        assert_eq!(b, 255, "White voxel should have b=255");
    }

    /// Test grid:to_voxels() includes position data.
    #[test]
    fn test_grid_to_voxels_positions() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local model = mj.create_model({
                values = "BW",
                size = {3, 1, 1}
            })
            model:one("B", "W")
            local grid = model:run(42)
            local voxels = grid:to_voxels()
            
            -- Collect all x positions
            local xs = {}
            for i, v in ipairs(voxels) do
                xs[i] = v.x
            end
            table.sort(xs)
            return xs
        "#;

        let result: mlua::Table = lua.load(script).eval().unwrap();
        // 3x1x1 grid centered at origin: positions should be -1, 0, 1
        let x1: i32 = result.get(1).unwrap();
        let x2: i32 = result.get(2).unwrap();
        let x3: i32 = result.get(3).unwrap();

        assert_eq!(x1, -1);
        assert_eq!(x2, 0);
        assert_eq!(x3, 1);
    }

    /// Test grid:to_voxel_world() returns a VoxelWorld.
    #[test]
    fn test_grid_to_voxel_world() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local model = mj.create_model({
                values = "BW",
                size = {5, 5, 1}
            })
            model:one("B", "W")
            local grid = model:run(42)
            local world = grid:to_voxel_world()
            return world:voxel_count()
        "#;

        let count: usize = lua.load(script).eval().unwrap();
        assert_eq!(count, 25, "VoxelWorld should contain 25 voxels");
    }

    /// Test verification from HANDOFF.md: basic model creation and counting.
    #[test]
    fn test_handoff_verification() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        // This is the verification test from HANDOFF.md
        let script = r#"
            local model = mj.create_model({ values = "BW", size = {10, 10, 1} })
            model:one("B", "W")
            local grid = model:run(42, 50)
            assert(grid:count_value("W") > 40, "Should have many white cells")
            return grid:count_value("W")
        "#;

        let count: usize = lua.load(script).eval().unwrap();
        assert!(count > 40, "Should have more than 40 white cells");
        assert_eq!(
            count, 50,
            "With 50 step limit, should have exactly 50 W cells"
        );
    }

    // ========================================================================
    // Phase 2.2: Execution Callbacks Tests
    // ========================================================================

    /// Test run_animated calls on_step for each step.
    #[test]
    fn test_run_animated_calls_on_step() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local builder = mj.create_model({
                values = "BW",
                size = {5, 1, 1}
            })
            builder:one("B", "W")
            local model = builder:build()
            
            local steps_seen = 0
            local grids_received = 0
            
            local total = model:run_animated({
                seed = 42,
                max_steps = 5,
                on_step = function(grid, step)
                    steps_seen = steps_seen + 1
                    if grid then grids_received = grids_received + 1 end
                end
            })
            
            return {steps_seen, grids_received, total}
        "#;

        let result: mlua::Table = lua.load(script).eval().unwrap();
        let steps_seen: usize = result.get(1).unwrap();
        let grids_received: usize = result.get(2).unwrap();
        let total: usize = result.get(3).unwrap();

        assert_eq!(steps_seen, 5, "on_step should be called 5 times");
        assert_eq!(grids_received, 5, "Should receive grid each time");
        assert_eq!(total, 5, "run_animated should return step count");
    }

    /// Test run_animated calls on_complete when model finishes.
    #[test]
    fn test_run_animated_calls_on_complete() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local builder = mj.create_model({
                values = "BW",
                size = {3, 1, 1}
            })
            builder:one("B", "W")
            local model = builder:build()
            
            local completed = false
            local final_steps = 0
            local final_grid_count = 0
            
            model:run_animated({
                seed = 42,
                on_complete = function(grid, steps)
                    completed = true
                    final_steps = steps
                    final_grid_count = grid:count_nonzero()
                end
            })
            
            return {completed, final_steps, final_grid_count}
        "#;

        let result: mlua::Table = lua.load(script).eval().unwrap();
        let completed: bool = result.get(1).unwrap();
        let final_steps: usize = result.get(2).unwrap();
        let final_grid_count: usize = result.get(3).unwrap();

        assert!(completed, "on_complete should be called");
        assert_eq!(final_steps, 3, "Should report 3 steps for 3x1 grid");
        assert_eq!(final_grid_count, 3, "Grid should have 3 non-zero cells");
    }

    /// Test run_animated with no callbacks still works.
    #[test]
    fn test_run_animated_no_callbacks() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local builder = mj.create_model({
                values = "BW",
                size = {4, 1, 1}
            })
            builder:one("B", "W")
            local model = builder:build()
            
            local steps = model:run_animated({
                seed = 42
            })
            
            return steps
        "#;

        let steps: usize = lua.load(script).eval().unwrap();
        assert_eq!(steps, 4, "Should run to completion (4 steps)");
    }

    /// Test run_animated respects max_steps limit.
    #[test]
    fn test_run_animated_max_steps() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local builder = mj.create_model({
                values = "BW",
                size = {10, 1, 1}
            })
            builder:one("B", "W")
            local model = builder:build()
            
            local steps = model:run_animated({
                seed = 42,
                max_steps = 3
            })
            
            return steps
        "#;

        let steps: usize = lua.load(script).eval().unwrap();
        assert_eq!(steps, 3, "Should stop at max_steps=3");
    }

    /// Test run_animated with on_step can access grid values.
    #[test]
    fn test_run_animated_on_step_grid_access() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local builder = mj.create_model({
                values = "BW",
                size = {5, 1, 1}
            })
            builder:one("B", "W")
            local model = builder:build()
            
            local w_counts = {}
            
            model:run_animated({
                seed = 42,
                on_step = function(grid, step)
                    w_counts[step] = grid:count_value("W")
                end
            })
            
            return w_counts
        "#;

        let result: mlua::Table = lua.load(script).eval().unwrap();
        // After each step, W count should increase by 1
        let w1: usize = result.get(1).unwrap();
        let w2: usize = result.get(2).unwrap();
        let w3: usize = result.get(3).unwrap();
        let w4: usize = result.get(4).unwrap();
        let w5: usize = result.get(5).unwrap();

        assert_eq!(w1, 1, "After step 1, should have 1 W");
        assert_eq!(w2, 2, "After step 2, should have 2 W");
        assert_eq!(w3, 3, "After step 3, should have 3 W");
        assert_eq!(w4, 4, "After step 4, should have 4 W");
        assert_eq!(w5, 5, "After step 5, should have 5 W");
    }

    /// Test model:changes() returns all changed positions.
    #[test]
    fn test_changes_returns_positions() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local builder = mj.create_model({
                values = "BW",
                size = {3, 1, 1}
            })
            builder:one("B", "W")
            local model = builder:build()
            
            model:reset(42)
            model:step()
            model:step()
            
            local changes = model:changes()
            return {#changes, changes[1].x ~= nil, changes[1].y ~= nil, changes[1].z ~= nil}
        "#;

        let result: mlua::Table = lua.load(script).eval().unwrap();
        let count: usize = result.get(1).unwrap();
        let has_x: bool = result.get(2).unwrap();
        let has_y: bool = result.get(3).unwrap();
        let has_z: bool = result.get(4).unwrap();

        assert_eq!(count, 2, "Should have 2 changes after 2 steps");
        assert!(has_x, "Change should have x");
        assert!(has_y, "Change should have y");
        assert!(has_z, "Change should have z");
    }

    /// Test model:last_changes() returns only the most recent step's changes.
    #[test]
    fn test_last_changes_returns_recent_only() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local builder = mj.create_model({
                values = "BW",
                size = {5, 1, 1}
            })
            builder:one("B", "W")
            local model = builder:build()
            
            model:reset(42)
            model:step()  -- 1 change
            model:step()  -- 1 change
            model:step()  -- 1 change
            
            local all_changes = model:changes()
            local last_changes = model:last_changes()
            
            return {#all_changes, #last_changes}
        "#;

        let result: mlua::Table = lua.load(script).eval().unwrap();
        let all_count: usize = result.get(1).unwrap();
        let last_count: usize = result.get(2).unwrap();

        assert_eq!(all_count, 3, "Should have 3 total changes");
        assert_eq!(last_count, 1, "Should have 1 change from last step");
    }

    /// Test changes() with AllNode (multiple changes per step).
    #[test]
    fn test_changes_with_all_node() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let script = r#"
            local builder = mj.create_model({
                values = "BW",
                size = {4, 1, 1}
            })
            builder:all("B", "W")  -- AllNode changes all B's at once
            local model = builder:build()
            
            model:reset(42)
            model:step()  -- Should change all 4 cells at once
            
            local changes = model:changes()
            return #changes
        "#;

        let count: usize = lua.load(script).eval().unwrap();
        // AllNode should change all 4 cells in one step
        assert_eq!(count, 4, "AllNode should change all 4 cells in one step");
    }

    /// Test run_animated requires seed parameter.
    #[test]
    fn test_run_animated_requires_seed() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let result: Result<(), _> = lua
            .load(
                r#"
                local model = mj.create_model({ values = "BW", size = {3, 1, 1} })
                model:one("B", "W")
                local built = model:build()
                built:run_animated({})  -- Missing seed!
            "#,
            )
            .exec();

        assert!(result.is_err(), "Should error when seed is missing");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("seed"),
            "Error should mention 'seed': {}",
            err_msg
        );
    }

    /// Test HANDOFF.md Phase 2.2 verification: run_animated with callbacks.
    #[test]
    fn test_handoff_phase_2_2_verification() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        // This is the verification test from HANDOFF.md Phase 2.2
        let script = r#"
            local builder = mj.create_model({ values = "BW", size = {5, 1, 1} })
            builder:one("B", "W")
            local model = builder:build()
            
            local steps_seen = 0
            model:run_animated({
                seed = 42,
                max_steps = 5,
                on_step = function(grid, step) 
                    steps_seen = steps_seen + 1 
                end
            })
            assert(steps_seen == 5, "Should call on_step 5 times")
            
            -- Also test changes
            model:reset(42)
            model:step()
            local changes = model:changes()
            assert(#changes > 0, "Should have changes after step")
            assert(changes[1].x ~= nil, "Change should have x")
            
            return true
        "#;

        let result: bool = lua.load(script).eval().unwrap();
        assert!(result, "HANDOFF.md Phase 2.2 verification should pass");
    }

    // ========================================================================
    // Phase 3.3: PNG Rendering Tests
    // ========================================================================

    /// Test grid:render_to_png() saves a PNG file.
    #[test]
    fn test_grid_render_to_png() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("screenshots");

        let output_path = output_dir.join("test_lua_render_to_png.png");
        // Clean up from previous runs
        let _ = std::fs::remove_file(&output_path);

        let script = format!(
            r#"
            local model = mj.create_model({{
                values = "BW",
                size = {{8, 8, 1}}
            }})
            model:one("B", "W")
            local grid = model:run(42, 32)
            return grid:render_to_png("{}")
            "#,
            output_path.display().to_string().replace('\\', "/")
        );

        let result: bool = lua.load(&script).eval().unwrap();
        assert!(result, "render_to_png should return true");
        assert!(output_path.exists(), "PNG file should be created");

        // Verify file has content
        let metadata = std::fs::metadata(&output_path).unwrap();
        assert!(metadata.len() > 100, "PNG file should have content");
    }

    /// Test grid:render_to_png() with 3D grid produces isometric output.
    #[test]
    fn test_grid_render_to_png_3d() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("screenshots");

        let output_path = output_dir.join("test_lua_render_to_png_3d.png");
        let _ = std::fs::remove_file(&output_path);

        let script = format!(
            r#"
            local model = mj.create_model({{
                values = "BW",
                size = {{8, 8, 8}},
                origin = true
            }})
            model:one("WB", "WW")
            local grid = model:run(42, 200)
            return grid:render_to_png("{}", 6)
            "#,
            output_path.display().to_string().replace('\\', "/")
        );

        let result: bool = lua.load(&script).eval().unwrap();
        assert!(result, "render_to_png should return true for 3D");
        assert!(output_path.exists(), "3D PNG file should be created");
    }

    /// Test grid:render_to_png() with custom pixel size.
    #[test]
    fn test_grid_render_to_png_custom_size() {
        let lua = Lua::new();
        register_markov_junior_api(&lua).unwrap();

        let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("screenshots");

        let output_path = output_dir.join("test_lua_render_to_png_large.png");
        let _ = std::fs::remove_file(&output_path);

        let script = format!(
            r#"
            local model = mj.create_model({{
                values = "BW",
                size = {{4, 4, 1}}
            }})
            model:one("B", "W")
            local grid = model:run(42)
            return grid:render_to_png("{}", 16)  -- Large pixels
            "#,
            output_path.display().to_string().replace('\\', "/")
        );

        let result: bool = lua.load(&script).eval().unwrap();
        assert!(result, "render_to_png should work with custom pixel size");
        assert!(output_path.exists(), "Large pixel PNG should be created");
    }
}
