use bevy::prelude::*;
use bevy_mod_imgui::prelude::{Condition, ImguiContext, Ui};
use mlua::{Function, Lua, Result as LuaResult};
use rand::Rng;
use std::cell::Cell;
use studio_physics::{ClearBodiesEvent, PhysicsState, SpawnCubeEvent};

// Thread-local pointer to the current imgui::Ui, only valid during on_draw callback
thread_local! {
    static UI_PTR: Cell<*const Ui> = const { Cell::new(std::ptr::null()) };
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

pub struct ScriptingPlugin;

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_mod_imgui::ImguiPlugin::default())
            .add_systems(Startup, setup_lua)
            .add_systems(Update, imgui_ui);
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
    if let Err(e) = load_ui_script(&mut vm, "assets/scripts/ui/main.lua") {
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
    mut spawn_events: MessageWriter<SpawnCubeEvent>,
    mut clear_events: MessageWriter<ClearBodiesEvent>,
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
                spawn_events.write(SpawnCubeEvent(Vec3::new(x, y, z)));
            }

            ui.same_line();

            if ui.button("Clear All") {
                clear_events.write(ClearBodiesEvent);
            }
        });

    // Call Lua on_draw with UI pointer set
    if let Some(ref mut vm) = vm {
        if let Some(ref key) = vm.draw_fn {
            // Set the UI pointer for the duration of the Lua callback
            UI_PTR.with(|cell| cell.set(ui as *const Ui));

            let result: LuaResult<()> = (|| {
                let draw_fn: Function = vm.lua.registry_value(key)?;
                draw_fn.call::<()>(())?;
                Ok(())
            })();

            // Clear the UI pointer immediately after
            UI_PTR.with(|cell| cell.set(std::ptr::null()));

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
