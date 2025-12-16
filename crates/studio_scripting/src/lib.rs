use bevy::prelude::*;
use bevy_mod_imgui::prelude::*;

pub struct ScriptingPlugin;

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_mod_imgui::ImguiPlugin::default())
            .add_systems(Update, imgui_ui);
    }
}

fn imgui_ui(mut context: NonSendMut<ImguiContext>) {
    let ui = context.ui();

    // Enable docking
    ui.dockspace_over_main_viewport();

    // Show demo window
    ui.show_demo_window(&mut true);

    // Custom debug window
    ui.window("Debug")
        .size([300.0, 100.0], bevy_mod_imgui::prelude::Condition::FirstUseEver)
        .build(|| {
            ui.text("ImGui is working");
        });
}
