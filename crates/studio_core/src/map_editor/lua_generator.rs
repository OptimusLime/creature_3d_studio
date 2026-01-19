//! Lua-based terrain generator for the map editor.
//!
//! Loads and executes generator scripts from `assets/map_editor/generator.lua`.
//! Provides hot-reload support via file watching.
//!
//! # Generator Protocol
//!
//! The Lua generator must return a table with these methods:
//! - `init(ctx)` - Initialize generator state
//! - `step(ctx) -> bool` - Generate one cell, return true when done
//! - `reset()` - Reset to initial state
//!
//! # Context API
//!
//! The `ctx` object passed to Lua provides:
//! - `ctx.width`, `ctx.height` - Buffer dimensions
//! - `ctx.palette` - Array of active material IDs
//! - `ctx:set_voxel(x, y, material_id)` - Write to buffer
//! - `ctx:get_voxel(x, y) -> material_id` - Read from buffer

use super::generator::{
    ActiveGenerator, CurrentStepInfo, FillCondition, FillGenerator, Generator, GeneratorListeners,
    GeneratorStructure, ParallelGenerator, ScatterGenerator, SequentialGenerator, StepInfo,
    StepInfoRegistry,
};
use super::material::MaterialPalette;
use super::playback::PlaybackState;
use super::voxel_buffer_2d::VoxelBuffer2D;
use crate::markov_junior::register_markov_junior_api;
use bevy::prelude::*;
use mlua::{
    Function, Lua, ObjectLike, Result as LuaResult, Table, UserData, UserDataMethods, Value,
};
use notify::{recommended_watcher, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

/// Default path to the generator Lua file.
pub const GENERATOR_LUA_PATH: &str = "assets/map_editor/generator.lua";

/// Plugin that loads and runs the Lua generator with hot-reload support.
pub struct LuaGeneratorPlugin {
    /// Path to the generator.lua file.
    pub path: String,
}

impl Default for LuaGeneratorPlugin {
    fn default() -> Self {
        Self {
            path: GENERATOR_LUA_PATH.to_string(),
        }
    }
}

impl Plugin for LuaGeneratorPlugin {
    fn build(&self, app: &mut App) {
        let path = self.path.clone();

        app.insert_resource(LuaGeneratorConfig { path: path.clone() });
        app.insert_resource(GeneratorReloadFlag { needs_reload: true });
        app.insert_resource(CurrentStepInfo::default());
        app.insert_resource(GeneratorListeners::default());

        // The actual Lua state is non-send (mlua::Lua is not Send)
        app.add_systems(Startup, setup_generator);
        app.add_systems(
            Update,
            (check_generator_reload, reload_generator, run_generator_step).chain(),
        );
    }
}

/// Configuration for the Lua generator.
#[derive(Resource)]
pub struct LuaGeneratorConfig {
    pub path: String,
}

/// Flag to trigger generator reload.
#[derive(Resource)]
pub struct GeneratorReloadFlag {
    pub needs_reload: bool,
}

/// The loaded Lua generator state (non-send because Lua is not thread-safe).
struct LuaGeneratorState {
    lua: Lua,
    generator: Option<Table>,
    initialized: bool,
}

/// Information about the last voxel write for step tracking.
#[derive(Clone, Default)]
struct LastWrite {
    x: usize,
    y: usize,
    material_id: u32,
    written: bool,
}

/// Step info emitted from Lua, pending collection by Rust.
#[derive(Clone, Default)]
struct PendingStepInfo {
    path: String,
    step_number: usize,
    x: usize,
    y: usize,
    material_id: u32,
    completed: bool,
    rule_name: Option<String>,
    affected_cells: Option<usize>,
}

/// Shared buffer for Lua to write to.
/// We use Arc<Mutex> so the UserData can access it.
/// Also tracks the last write for step info generation.
#[derive(Clone)]
struct SharedBuffer {
    data: Arc<Mutex<Vec<u32>>>,
    last_write: Arc<Mutex<LastWrite>>,
    pending_steps: Arc<Mutex<Vec<PendingStepInfo>>>,
    width: usize,
    height: usize,
}

impl SharedBuffer {
    fn new(width: usize, height: usize) -> Self {
        Self {
            data: Arc::new(Mutex::new(vec![0; width * height])),
            last_write: Arc::new(Mutex::new(LastWrite::default())),
            pending_steps: Arc::new(Mutex::new(Vec::new())),
            width,
            height,
        }
    }

    fn set(&self, x: usize, y: usize, value: u32) {
        if x < self.width && y < self.height {
            let mut data = self.data.lock().unwrap();
            data[y * self.width + x] = value;

            // Track this as the last write
            let mut last = self.last_write.lock().unwrap();
            last.x = x;
            last.y = y;
            last.material_id = value;
            last.written = true;
        }
    }

    fn get(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            let data = self.data.lock().unwrap();
            data[y * self.width + x]
        } else {
            0
        }
    }

    fn copy_to_buffer(&self, buffer: &mut VoxelBuffer2D) {
        let data = self.data.lock().unwrap();
        for y in 0..self.height.min(buffer.height) {
            for x in 0..self.width.min(buffer.width) {
                buffer.set(x, y, data[y * self.width + x]);
            }
        }
    }

    fn clear(&self) {
        let mut data = self.data.lock().unwrap();
        data.fill(0);
        let mut last = self.last_write.lock().unwrap();
        *last = LastWrite::default();
        let mut pending = self.pending_steps.lock().unwrap();
        pending.clear();
    }

    /// Take the last write info, clearing the written flag.
    fn take_last_write(&self) -> Option<(usize, usize, u32)> {
        let mut last = self.last_write.lock().unwrap();
        if last.written {
            last.written = false;
            Some((last.x, last.y, last.material_id))
        } else {
            None
        }
    }

    /// Emit a step info (called from Lua via GeneratorContext).
    fn emit_step(&self, info: PendingStepInfo) {
        let mut pending = self.pending_steps.lock().unwrap();
        pending.push(info);
    }

    /// Take all pending step infos.
    fn take_pending_steps(&self) -> Vec<PendingStepInfo> {
        let mut pending = self.pending_steps.lock().unwrap();
        std::mem::take(&mut *pending)
    }
}

/// Context passed to Lua generator methods.
struct GeneratorContext {
    buffer: SharedBuffer,
    palette: Vec<u32>,
}

impl UserData for GeneratorContext {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, this| Ok(this.buffer.width));
        fields.add_field_method_get("height", |_, this| Ok(this.buffer.height));
        fields.add_field_method_get("palette", |lua, this| {
            // Convert palette to Lua table (1-indexed)
            let table = lua.create_table()?;
            for (i, &id) in this.palette.iter().enumerate() {
                table.set(i + 1, id)?;
            }
            Ok(table)
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("set_voxel", |_, this, (x, y, mat): (usize, usize, u32)| {
            this.buffer.set(x, y, mat);
            Ok(())
        });

        methods.add_method("get_voxel", |_, this, (x, y): (usize, usize)| {
            Ok(this.buffer.get(x, y))
        });

        // Emit step info with path for scene tree tracking
        methods.add_method("emit_step", |_, this, (path, info): (String, Table)| {
            let pending = PendingStepInfo {
                path,
                step_number: info.get("step_number").unwrap_or(0),
                x: info.get("x").unwrap_or(0),
                y: info.get("y").unwrap_or(0),
                material_id: info.get("material_id").unwrap_or(0),
                completed: info.get("completed").unwrap_or(false),
                rule_name: info.get("rule_name").ok(),
                affected_cells: info.get("affected_cells").ok(),
            };
            this.buffer.emit_step(pending);
            Ok(())
        });
    }
}

/// Resource holding the generator's shared buffer.
#[derive(Resource)]
pub struct GeneratorBuffer {
    pub(super) buffer: SharedBuffer,
}

/// Resource holding the file watcher.
struct GeneratorWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event, notify::Error>>,
}

/// Setup the Lua generator and file watcher.
fn setup_generator(world: &mut World) {
    let config = world.resource::<LuaGeneratorConfig>();
    let path = config.path.clone();

    // Create shared buffer matching the VoxelBuffer2D size
    let voxel_buffer = world.resource::<VoxelBuffer2D>();
    let shared_buffer = SharedBuffer::new(voxel_buffer.width, voxel_buffer.height);

    world.insert_resource(GeneratorBuffer {
        buffer: shared_buffer,
    });

    // Create Lua state and register MarkovJunior API
    let lua = Lua::new();

    // Set up package path to find lib modules
    if let Err(e) = lua
        .load(
            r#"
        package.path = package.path .. ";assets/map_editor/?.lua"
    "#,
        )
        .exec()
    {
        error!("Failed to set Lua package path: {:?}", e);
    }

    if let Err(e) = register_markov_junior_api(&lua) {
        error!("Failed to register MarkovJunior API: {:?}", e);
    } else {
        info!("MarkovJunior API registered in generator Lua context");
    }
    world.insert_non_send_resource(LuaGeneratorState {
        lua,
        generator: None,
        initialized: false,
    });

    // Insert StepInfoRegistry resource
    world.insert_resource(StepInfoRegistry::default());

    // Insert ActiveGenerator (non-send because Generator may not be Send)
    world.insert_non_send_resource(super::generator::ActiveGenerator::new());

    // Setup file watcher
    let watch_path = Path::new(&path)
        .parent()
        .unwrap_or(Path::new("assets/map_editor"));

    let (tx, rx) = channel();

    let mut watcher = match recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create generator file watcher: {:?}", e);
            return;
        }
    };

    if let Err(e) = watcher.watch(watch_path, RecursiveMode::NonRecursive) {
        error!(
            "Failed to watch generator directory {:?}: {:?}",
            watch_path, e
        );
        return;
    }

    info!("Hot reload enabled for generator at {}", path);

    world.insert_non_send_resource(GeneratorWatcher {
        _watcher: watcher,
        receiver: rx,
    });
}

/// Convert a Lua generator table to a Rust Generator for structure introspection.
///
/// This recursively converts composed generators (Sequential, Parallel) and their
/// children into Rust Generator implementations. The Rust generators mirror the
/// Lua structure purely for MCP introspection - the actual generation still uses Lua.
fn lua_table_to_rust_generator(lua: &Lua, table: &Table) -> Option<Box<dyn Generator>> {
    // Get the _type field to determine generator type
    let gen_type: String = match table.get("_type") {
        Ok(t) => t,
        Err(_) => {
            // Check if this is a userdata (like MjLuaModel)
            // For simple Lua generators without _type, create a placeholder
            return None;
        }
    };

    match gen_type.as_str() {
        "Sequential" => {
            let children = extract_children(lua, table);
            Some(Box::new(SequentialGenerator::new(children)))
        }
        "Parallel" => {
            let children = extract_children(lua, table);
            Some(Box::new(ParallelGenerator::new(children)))
        }
        "Scatter" => {
            let material: u32 = table.get("_material").unwrap_or(1);
            let target: u32 = table.get("_target").unwrap_or(0);
            let density: f64 = table.get("_density").unwrap_or(0.1);
            Some(Box::new(ScatterGenerator::new(material, target, density)))
        }
        "Fill" => {
            let material: u32 = table.get("_material").unwrap_or(1);
            let where_str: String = table.get("_where").unwrap_or_else(|_| "all".to_string());
            let condition = match where_str.as_str() {
                "empty" => FillCondition::Empty,
                "border" => FillCondition::Border,
                _ => FillCondition::All,
            };
            Some(Box::new(FillGenerator::new(material, condition)))
        }
        "MjModel" => {
            // MjModel is a Rust userdata, we need to create MjGenerator from it
            // For now, create a placeholder structure since we can't extract the Model
            // The actual Model is inside the Lua userdata
            None // TODO: Extract Model from MjLuaModel userdata
        }
        _ => {
            warn!("Unknown generator type: {}", gen_type);
            None
        }
    }
}

/// Extract children from a Lua generator table.
fn extract_children(lua: &Lua, table: &Table) -> Vec<(String, Box<dyn Generator>)> {
    let mut children = Vec::new();

    // Get _children table
    let children_table: Table = match table.get("_children") {
        Ok(t) => t,
        Err(_) => return children,
    };

    // Get _child_names for ordering (if available)
    let child_names: Vec<String> = table
        .get::<Table>("_child_names")
        .ok()
        .and_then(|t| {
            t.sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect::<Vec<_>>()
                .into()
        })
        .unwrap_or_default();

    // If we have ordered names, use them
    if !child_names.is_empty() {
        for name in child_names {
            if let Ok(child_value) = children_table.get::<Value>(name.clone()) {
                if let Some(child_gen) = value_to_generator(lua, &child_value) {
                    children.push((name, child_gen));
                }
            }
        }
    } else {
        // Fall back to iterating the table (unordered)
        for (name, value) in children_table.pairs::<String, Value>().flatten() {
            if let Some(child_gen) = value_to_generator(lua, &value) {
                children.push((name, child_gen));
            }
        }
    }

    children
}

/// Convert a Lua Value to a Rust Generator.
fn value_to_generator(_lua: &Lua, value: &Value) -> Option<Box<dyn Generator>> {
    match value {
        Value::Table(t) => lua_table_to_rust_generator(_lua, t),
        Value::UserData(ud) => {
            // Check if this is an MjLuaModel userdata
            // MjLuaModel has a _type field that returns "MjModel"
            if let Ok(type_name) = ud.get::<String>("_type") {
                if type_name == "MjModel" {
                    // Get the model name via the _path field (which contains model info)
                    let path: String = ud.get("_path").unwrap_or_else(|_| "root".to_string());
                    // For model name, we'd need to call a method, but fields are easier
                    // Just use "MjModel" as placeholder - the path will show location
                    return Some(Box::new(MjGeneratorPlaceholder::new(
                        path.split('.').last().unwrap_or("model").to_string(),
                    )));
                }
            }
            None
        }
        _ => None,
    }
}

/// Placeholder for MjGenerator when we can't extract the actual Model.
/// Used purely for structure introspection via MCP.
struct MjGeneratorPlaceholder {
    model_name: String,
    path: String,
}

impl MjGeneratorPlaceholder {
    fn new(model_name: String) -> Self {
        Self {
            model_name,
            path: "root".to_string(),
        }
    }
}

impl Generator for MjGeneratorPlaceholder {
    fn type_name(&self) -> &str {
        "MjModel"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn structure(&self) -> GeneratorStructure {
        GeneratorStructure::mj_model(&self.path, &self.model_name)
    }

    fn init(&mut self, _ctx: &mut super::generator::GeneratorContext) {}

    fn step(&mut self, _ctx: &mut super::generator::GeneratorContext) -> bool {
        true // Placeholder always "done"
    }

    fn reset(&mut self, _seed: u64) {}

    fn last_step_info(&self) -> Option<&StepInfo> {
        None
    }

    fn is_done(&self) -> bool {
        true
    }

    fn set_path(&mut self, path: String) {
        self.path = path;
    }
}

/// Check for file changes and set reload flag.
fn check_generator_reload(
    watcher: Option<NonSend<GeneratorWatcher>>,
    mut reload_flag: ResMut<GeneratorReloadFlag>,
) {
    let Some(watcher) = watcher else { return };

    while let Ok(event) = watcher.receiver.try_recv() {
        if let Ok(event) = event {
            for path in &event.paths {
                if path
                    .file_name()
                    .map(|n| n == "generator.lua")
                    .unwrap_or(false)
                {
                    info!("Detected change in generator.lua, scheduling reload...");
                    reload_flag.needs_reload = true;
                }
            }
        }
    }
}

/// Reload the generator from Lua file.
fn reload_generator(
    config: Res<LuaGeneratorConfig>,
    mut reload_flag: ResMut<GeneratorReloadFlag>,
    mut state: NonSendMut<LuaGeneratorState>,
    mut playback: ResMut<PlaybackState>,
    gen_buffer: Res<GeneratorBuffer>,
    mut active_generator: NonSendMut<ActiveGenerator>,
) {
    if !reload_flag.needs_reload {
        return;
    }
    reload_flag.needs_reload = false;

    // Load and parse the generator script
    let src = match std::fs::read_to_string(&config.path) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read generator file {}: {}", config.path, e);
            return;
        }
    };

    // Evaluate the script to get the Generator table
    let generator: Table = match state.lua.load(&src).eval() {
        Ok(g) => g,
        Err(e) => {
            error!("Failed to load generator script: {:?}", e);
            return;
        }
    };

    // Convert Lua generator to Rust generator for structure introspection
    if let Some(rust_gen) = lua_table_to_rust_generator(&state.lua, &generator) {
        active_generator.set(rust_gen);
        info!("Generator structure extracted for MCP introspection");
    } else {
        // Clear active generator if we couldn't convert
        active_generator.clear();
    }

    state.generator = Some(generator);
    state.initialized = false;

    // Restart playback and clear buffer (keeps playing if it was playing)
    playback.restart();
    gen_buffer.buffer.clear();

    info!("Generator reloaded from {}", config.path);
}

/// Run generator steps based on playback state.
fn run_generator_step(
    mut state: NonSendMut<LuaGeneratorState>,
    mut playback: ResMut<PlaybackState>,
    mut palette: ResMut<MaterialPalette>,
    gen_buffer: Res<GeneratorBuffer>,
    mut voxel_buffer: ResMut<VoxelBuffer2D>,
    mut current_step: ResMut<CurrentStepInfo>,
    mut listeners: ResMut<GeneratorListeners>,
    mut step_registry: ResMut<StepInfoRegistry>,
    time: Res<Time>,
) {
    // Check if generator is loaded
    if state.generator.is_none() {
        return;
    }

    // Create context for Lua
    let ctx = GeneratorContext {
        buffer: gen_buffer.buffer.clone(),
        palette: palette.active.clone(),
    };

    // Initialize if needed - run generator to completion immediately
    if !state.initialized {
        // Call init
        {
            let generator = state.generator.as_ref().unwrap();
            if let Err(e) = call_generator_method(&state.lua, generator, "init", &ctx) {
                error!("Generator init failed: {:?}", e);
                return;
            }
        }
        state.initialized = true;
        current_step.clear();
        listeners.notify_reset();

        // Run generator to completion immediately for initial display
        let max_steps = ctx.buffer.width * ctx.buffer.height;
        for _ in 0..max_steps {
            let generator = state.generator.as_ref().unwrap();
            match call_generator_step(&state.lua, generator, &ctx) {
                Ok(done) => {
                    // Process any pending step infos from Lua
                    process_pending_steps(
                        &gen_buffer.buffer,
                        &mut step_registry,
                        &mut current_step,
                        &mut listeners,
                        playback.step_index,
                    );

                    // Also handle legacy last_write for backwards compatibility
                    if let Some((x, y, material_id)) = gen_buffer.buffer.take_last_write() {
                        let info = StepInfo::new(playback.step_index, x, y, material_id, done);
                        current_step.update(info.clone());
                        listeners.notify_step(&info);
                    }
                    playback.step();
                    if done {
                        playback.complete();
                        break;
                    }
                }
                Err(e) => {
                    error!("Generator step failed during init: {:?}", e);
                    break;
                }
            }
        }
        info!(
            "Generator initialized with palette: {:?}, filled {} cells",
            palette.active, playback.step_index
        );
    }

    // Handle palette changes - reinitialize and fill immediately
    if palette.changed {
        // Reset and init
        {
            let generator = state.generator.as_ref().unwrap();
            if let Err(e) = call_generator_method(&state.lua, generator, "reset", &ctx) {
                error!("Generator reset failed: {:?}", e);
            }
            if let Err(e) = call_generator_method(&state.lua, generator, "init", &ctx) {
                error!("Generator init failed: {:?}", e);
            }
        }
        playback.reset();
        gen_buffer.buffer.clear();
        current_step.clear();
        step_registry.clear();
        listeners.notify_reset();

        // Recreate context with cleared buffer
        let ctx = GeneratorContext {
            buffer: gen_buffer.buffer.clone(),
            palette: palette.active.clone(),
        };

        // Run generator to completion immediately
        let max_steps = ctx.buffer.width * ctx.buffer.height;
        for _ in 0..max_steps {
            let generator = state.generator.as_ref().unwrap();
            match call_generator_step(&state.lua, generator, &ctx) {
                Ok(done) => {
                    // Process any pending step infos from Lua
                    process_pending_steps(
                        &gen_buffer.buffer,
                        &mut step_registry,
                        &mut current_step,
                        &mut listeners,
                        playback.step_index,
                    );

                    // Also handle legacy last_write for backwards compatibility
                    if let Some((x, y, material_id)) = gen_buffer.buffer.take_last_write() {
                        let info = StepInfo::new(playback.step_index, x, y, material_id, done);
                        current_step.update(info.clone());
                        listeners.notify_step(&info);
                    }
                    playback.step();
                    if done {
                        playback.complete();
                        break;
                    }
                }
                Err(e) => {
                    error!("Generator step failed during reinit: {:?}", e);
                    break;
                }
            }
        }

        palette.clear_changed();
        info!(
            "Generator reinitialized with new palette: {:?}, filled {} cells",
            palette.active, playback.step_index
        );
    }

    if playback.completed || !playback.playing {
        // Still copy buffer even when not playing (for initial state)
        gen_buffer.buffer.copy_to_buffer(&mut voxel_buffer);
        return;
    }

    // Run steps based on speed
    playback.accumulator += time.delta_secs() * playback.speed;

    while playback.accumulator >= 1.0 && !playback.completed {
        playback.accumulator -= 1.0;

        // Call step
        let generator = state.generator.as_ref().unwrap();
        match call_generator_step(&state.lua, generator, &ctx) {
            Ok(done) => {
                // Process any pending step infos from Lua
                process_pending_steps(
                    &gen_buffer.buffer,
                    &mut step_registry,
                    &mut current_step,
                    &mut listeners,
                    playback.step_index,
                );

                // Also handle legacy last_write for backwards compatibility
                if let Some((x, y, material_id)) = gen_buffer.buffer.take_last_write() {
                    let info = StepInfo::new(playback.step_index, x, y, material_id, done);
                    current_step.update(info.clone());
                    listeners.notify_step(&info);
                }
                playback.step();
                if done {
                    playback.complete();
                    info!("Generator completed");
                    break;
                }
            }
            Err(e) => {
                error!("Generator step failed: {:?}", e);
                playback.complete();
                break;
            }
        }
    }

    // Copy shared buffer to voxel buffer
    gen_buffer.buffer.copy_to_buffer(&mut voxel_buffer);
}

/// Call a generator method (init, reset).
fn call_generator_method(
    lua: &Lua,
    generator: &Table,
    method: &str,
    ctx: &GeneratorContext,
) -> LuaResult<()> {
    let func: Function = generator.get(method)?;
    let ctx_ud = lua.create_userdata(GeneratorContext {
        buffer: ctx.buffer.clone(),
        palette: ctx.palette.clone(),
    })?;
    func.call::<()>((generator.clone(), ctx_ud))?;
    Ok(())
}

/// Call the generator step method.
fn call_generator_step(lua: &Lua, generator: &Table, ctx: &GeneratorContext) -> LuaResult<bool> {
    let func: Function = generator.get("step")?;
    let ctx_ud = lua.create_userdata(GeneratorContext {
        buffer: ctx.buffer.clone(),
        palette: ctx.palette.clone(),
    })?;
    let result: Value = func.call((generator.clone(), ctx_ud))?;

    // Convert result to bool
    match result {
        Value::Boolean(b) => Ok(b),
        Value::Nil => Ok(false),
        _ => Ok(false),
    }
}

/// Process pending step infos from Lua and emit to registry and listeners.
fn process_pending_steps(
    buffer: &SharedBuffer,
    step_registry: &mut StepInfoRegistry,
    current_step: &mut CurrentStepInfo,
    listeners: &mut GeneratorListeners,
    base_step: usize,
) {
    for pending in buffer.take_pending_steps() {
        let info = StepInfo {
            path: pending.path.clone(),
            step_number: base_step + pending.step_number,
            x: pending.x,
            y: pending.y,
            material_id: pending.material_id,
            completed: pending.completed,
            rule_name: pending.rule_name,
            affected_cells: pending.affected_cells,
        };

        // Emit to registry (keyed by path)
        step_registry.emit(&pending.path, info.clone());

        // Also update current step and notify listeners
        current_step.update(info.clone());
        listeners.notify_step(&info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_buffer() {
        let buffer = SharedBuffer::new(4, 4);
        buffer.set(1, 2, 5);
        assert_eq!(buffer.get(1, 2), 5);
        assert_eq!(buffer.get(0, 0), 0);
    }

    #[test]
    fn test_generator_context() {
        let lua = Lua::new();
        let buffer = SharedBuffer::new(32, 32);
        let ctx = GeneratorContext {
            buffer: buffer.clone(),
            palette: vec![1, 2, 3],
        };

        // Test that we can create userdata
        let ud = lua.create_userdata(ctx).unwrap();
        assert!(ud.is::<GeneratorContext>());
    }
}
