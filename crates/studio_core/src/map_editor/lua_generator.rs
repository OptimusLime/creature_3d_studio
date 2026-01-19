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

use super::generator::{CurrentStepInfo, GeneratorListeners, StepInfo};
use super::material::MaterialPalette;
use super::playback::PlaybackState;
use super::voxel_buffer_2d::VoxelBuffer2D;
use bevy::prelude::*;
use mlua::{Function, Lua, Result as LuaResult, Table, UserData, UserDataMethods, Value};
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

/// Shared buffer for Lua to write to.
/// We use Arc<Mutex> so the UserData can access it.
/// Also tracks the last write for step info generation.
#[derive(Clone)]
struct SharedBuffer {
    data: Arc<Mutex<Vec<u32>>>,
    last_write: Arc<Mutex<LastWrite>>,
    width: usize,
    height: usize,
}

impl SharedBuffer {
    fn new(width: usize, height: usize) -> Self {
        Self {
            data: Arc::new(Mutex::new(vec![0; width * height])),
            last_write: Arc::new(Mutex::new(LastWrite::default())),
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

    // Create Lua state
    let lua = Lua::new();
    world.insert_non_send_resource(LuaGeneratorState {
        lua,
        generator: None,
        initialized: false,
    });

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
                    // Emit step info to current_step and listeners
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
                    // Emit step info to current_step and listeners
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
                // Emit step info to current_step and listeners
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
