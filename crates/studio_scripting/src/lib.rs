use bevy::prelude::*;
use bevy_mod_imgui::prelude::*;
use rand::Rng;
use studio_physics::{ClearBodiesEvent, PhysicsState, SpawnCubeEvent};

pub struct ScriptingPlugin;

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_mod_imgui::ImguiPlugin::default())
            .add_systems(Update, imgui_ui);
    }
}

fn imgui_ui(
    mut context: NonSendMut<ImguiContext>,
    physics: Res<PhysicsState>,
    mut spawn_events: MessageWriter<SpawnCubeEvent>,
    mut clear_events: MessageWriter<ClearBodiesEvent>,
) {
    let ui = context.ui();

    // Enable docking
    ui.dockspace_over_main_viewport();

    // Scene control window
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
}
