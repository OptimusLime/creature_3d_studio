use bevy::prelude::*;
use bevy_mod_imgui::prelude::*;
use mlua::{Function, Lua, Result as LuaResult};
use rand::Rng;
use studio_physics::{ClearBodiesEvent, PhysicsState, SpawnCubeEvent};

pub struct ScriptingPlugin;

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_mod_imgui::ImguiPlugin::default())
            .add_systems(Startup, setup_lua)
            .add_systems(Update, (imgui_ui, lua_on_draw));
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

fn lua_on_draw(mut vm: NonSendMut<LuaVm>) {
    // Call on_draw if it exists
    if let Some(ref key) = vm.draw_fn {
        let result: LuaResult<()> = (|| {
            let draw_fn: Function = vm.lua.registry_value(key)?;
            draw_fn.call::<()>(())?;
            Ok(())
        })();

        if let Err(e) = result {
            vm.last_error = Some(format!("{:?}", e));
        } else {
            vm.last_error = None;
        }
    }
}

fn imgui_ui(
    mut context: NonSendMut<ImguiContext>,
    physics: Res<PhysicsState>,
    mut spawn_events: MessageWriter<SpawnCubeEvent>,
    mut clear_events: MessageWriter<ClearBodiesEvent>,
    vm: Option<NonSend<LuaVm>>,
) {
    let ui = context.ui();

    // Enable docking
    ui.dockspace_over_main_viewport();

    // Scene control window (Rust-side)
    ui.window("Scene")
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

    // Show Lua errors if any
    if let Some(ref vm) = vm {
        if let Some(ref err) = vm.last_error {
            ui.window("Lua Error")
                .size([400.0, 150.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text_wrapped(err);
                });
        }
    }
}
