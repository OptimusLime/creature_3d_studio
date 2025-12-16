use bevy::prelude::*;
use bevy_mod_imgui::prelude::{Condition, ImguiContext, Ui};
use mlua::{Function, Lua, Result as LuaResult};
use notify::{recommended_watcher, Event, RecommendedWatcher, RecursiveMode, Watcher};
use rand::Rng;
use std::cell::Cell;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use studio_physics::{PhysicsState, UiActions};

const SCRIPT_PATH: &str = "assets/scripts/ui/main.lua";

// Thread-local pointers, only valid during on_draw callback
thread_local! {
    static UI_PTR: Cell<*const Ui> = const { Cell::new(std::ptr::null()) };
    static ACTIONS_PTR: Cell<*mut UiActions> = const { Cell::new(std::ptr::null_mut()) };
}

/// Execute a closure with access to the current imgui::Ui.
/// Returns an error if called outside of a UI frame (i.e., not during on_draw).
fn with_ui<R>(f: impl FnOnce(&Ui) -> R) -> LuaResult<R> {
    UI_PTR.with(|cell| {
        let ptr = cell.get();
        if ptr.is_null() {
            return Err(mlua::Error::RuntimeError(
                "imgui.* called outside UI frame".into(),
            ));
        }
        // SAFETY: The pointer is only set during the on_draw callback,
        // and cleared immediately after. The Ui reference is valid for
        // the duration of that callback.
        Ok(f(unsafe { &*ptr }))
    })
}

/// Execute a closure with mutable access to UiActions.
/// Returns an error if called outside of a UI frame.
fn with_actions<R>(f: impl FnOnce(&mut UiActions) -> R) -> LuaResult<R> {
    ACTIONS_PTR.with(|cell| {
        let ptr = cell.get();
        if ptr.is_null() {
            return Err(mlua::Error::RuntimeError(
                "tools.* called outside UI frame".into(),
            ));
        }
        // SAFETY: Same as UI_PTR - only valid during on_draw callback
        Ok(f(unsafe { &mut *ptr }))
    })
}

pub struct ScriptingPlugin;

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_mod_imgui::ImguiPlugin::default())
            .add_systems(Startup, (setup_lua, setup_file_watcher))
            .add_systems(Update, (check_hot_reload, imgui_ui).chain());
    }
}

/// Holds file watcher and change notification receiver
struct ScriptWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event, notify::Error>>,
}

fn setup_file_watcher(world: &mut World) {
    let (tx, rx) = channel();

    let mut watcher = match recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create file watcher: {:?}", e);
            return;
        }
    };

    // Watch the scripts directory
    if let Err(e) = watcher.watch(Path::new("assets/scripts"), RecursiveMode::Recursive) {
        error!("Failed to watch scripts directory: {:?}", e);
        return;
    }

    info!("Hot reload enabled for {}", SCRIPT_PATH);

    world.insert_non_send_resource(ScriptWatcher {
        _watcher: watcher,
        receiver: rx,
    });
}

fn check_hot_reload(
    watcher: Option<NonSend<ScriptWatcher>>,
    mut vm: Option<NonSendMut<LuaVm>>,
) {
    let Some(watcher) = watcher else { return };
    let Some(ref mut vm) = vm else { return };

    // Check for file change events (non-blocking)
    let mut should_reload = false;
    while let Ok(event) = watcher.receiver.try_recv() {
        if let Ok(event) = event {
            // Check if any modified path is our script
            for path in &event.paths {
                if path.ends_with("main.lua") {
                    should_reload = true;
                }
            }
        }
    }

    if should_reload {
        info!("Reloading Lua script...");
        reload_lua_vm(vm);
    }
}

fn reload_lua_vm(vm: &mut LuaVm) {
    // Create fresh Lua VM
    let lua = Lua::new();

    if let Err(e) = register_lua_api(&lua) {
        vm.last_error = Some(format!("Failed to register API: {:?}", e));
        error!("Hot reload failed: {:?}", e);
        return;
    }

    vm.lua = lua;
    vm.draw_fn = None;

    // Reload the script
    if let Err(e) = load_ui_script(vm, SCRIPT_PATH) {
        vm.last_error = Some(format!("{:?}", e));
        error!("Hot reload failed: {:?}", e);
    } else {
        vm.last_error = None;
        info!("Hot reload successful");
    }
}

/// Holds the Lua VM and script state (non-send because Lua is !Send)
struct LuaVm {
    lua: Lua,
    draw_fn: Option<mlua::RegistryKey>,
    last_error: Option<String>,
}

fn setup_lua(world: &mut World) {
    let lua = Lua::new();

    // Register tools.print
    if let Err(e) = register_lua_api(&lua) {
        error!("Failed to register Lua API: {:?}", e);
    }

    let mut vm = LuaVm {
        lua,
        draw_fn: None,
        last_error: None,
    };

    // Load the main UI script
    if let Err(e) = load_ui_script(&mut vm, SCRIPT_PATH) {
        vm.last_error = Some(format!("{:?}", e));
        error!("Failed to load Lua script: {:?}", e);
    }

    world.insert_non_send_resource(vm);
}

fn register_lua_api(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    // Create tools table
    let tools = lua.create_table()?;
    tools.set(
        "print",
        lua.create_function(|_, msg: String| {
            info!("[lua] {}", msg);
            Ok(())
        })?,
    )?;

    // tools.spawn_cube(x, y, z) - queue a cube spawn at position
    tools.set(
        "spawn_cube",
        lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
            with_actions(|actions| actions.spawn_cube(Vec3::new(x, y, z)))?;
            Ok(())
        })?,
    )?;

    // tools.clear() - queue clearing all dynamic bodies
    tools.set(
        "clear",
        lua.create_function(|_, ()| {
            with_actions(|actions| actions.clear())?;
            Ok(())
        })?,
    )?;

    globals.set("tools", tools)?;

    // Create imgui table with UI functions
    let imgui_table = lua.create_table()?;

    // imgui.text(str) - display text
    imgui_table.set(
        "text",
        lua.create_function(|_, text: String| {
            with_ui(|ui| ui.text(&text))?;
            Ok(())
        })?,
    )?;

    // imgui.button(label) -> bool - display button, returns true if clicked
    imgui_table.set(
        "button",
        lua.create_function(|_, label: String| with_ui(|ui| ui.button(&label)))?,
    )?;

    // imgui.window(title, fn) - create a window and call fn inside it
    // Position offset from Scene window so they don't overlap
    imgui_table.set(
        "window",
        lua.create_function(|_, (title, callback): (String, Function)| {
            with_ui(|ui| {
                ui.window(&title)
                    .position([340.0, 40.0], Condition::FirstUseEver)
                    .size([300.0, 200.0], Condition::FirstUseEver)
                    .build(|| {
                        // Call the Lua callback inside the window context
                        if let Err(e) = callback.call::<()>(()) {
                            ui.text_colored([1.0, 0.3, 0.3, 1.0], format!("Error: {:?}", e));
                        }
                    });
            })?;
            Ok(())
        })?,
    )?;

    // imgui.separator() - draw a horizontal separator
    imgui_table.set(
        "separator",
        lua.create_function(|_, ()| {
            with_ui(|ui| ui.separator())?;
            Ok(())
        })?,
    )?;

    // imgui.same_line() - next widget on same line
    imgui_table.set(
        "same_line",
        lua.create_function(|_, ()| {
            with_ui(|ui| ui.same_line())?;
            Ok(())
        })?,
    )?;

    globals.set("imgui", imgui_table)?;

    Ok(())
}

fn load_ui_script(vm: &mut LuaVm, path: &str) -> LuaResult<()> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| mlua::Error::RuntimeError(format!("Failed to read {}: {}", path, e)))?;

    // Execute the script (which should define on_draw function)
    vm.lua.load(&src).set_name(path).exec()?;

    // Get the on_draw function if it exists
    let globals = vm.lua.globals();
    if let Ok(draw_fn) = globals.get::<Function>("on_draw") {
        let key = vm.lua.create_registry_value(draw_fn)?;
        vm.draw_fn = Some(key);
    }

    Ok(())
}

fn imgui_ui(
    mut context: NonSendMut<ImguiContext>,
    physics: Res<PhysicsState>,
    mut actions: ResMut<UiActions>,
    mut vm: Option<NonSendMut<LuaVm>>,
) {
    let ui = context.ui();

    // Enable docking
    ui.dockspace_over_main_viewport();

    // Scene control window (Rust-side) - positioned at top-left
    ui.window("Scene")
        .position([20.0, 40.0], Condition::FirstUseEver)
        .size([300.0, 150.0], Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("Dynamic bodies: {}", physics.dynamic_body_count()));
            ui.separator();

            if ui.button("Spawn Cube") {
                let mut rng = rand::thread_rng();
                let x = rng.gen_range(-3.0..3.0);
                let z = rng.gen_range(-3.0..3.0);
                let y = rng.gen_range(3.0..8.0);
                actions.spawn_cube(Vec3::new(x, y, z));
            }

            ui.same_line();

            if ui.button("Clear All") {
                actions.clear();
            }
        });

    // Call Lua on_draw with UI and Actions pointers set
    if let Some(ref mut vm) = vm {
        if let Some(ref key) = vm.draw_fn {
            // Set pointers for the duration of the Lua callback
            UI_PTR.with(|cell| cell.set(ui as *const Ui));
            ACTIONS_PTR.with(|cell| cell.set(actions.as_mut() as *mut UiActions));

            let result: LuaResult<()> = (|| {
                let draw_fn: Function = vm.lua.registry_value(key)?;
                draw_fn.call::<()>(())?;
                Ok(())
            })();

            // Clear pointers immediately after
            UI_PTR.with(|cell| cell.set(std::ptr::null()));
            ACTIONS_PTR.with(|cell| cell.set(std::ptr::null_mut()));

            if let Err(e) = result {
                vm.last_error = Some(format!("{:?}", e));
            } else {
                vm.last_error = None;
            }
        }

        // Show Lua errors if any
        if let Some(ref err) = vm.last_error {
            ui.window("Lua Error")
                .size([400.0, 150.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text_wrapped(err);
                });
        }
    }
}
