use bevy::prelude::*;
use studio_core::{CorePlugin, OrbitCameraBundle};
use studio_physics::PhysicsPlugin;
use studio_scripting::ScriptingPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(PhysicsPlugin)
        .add_plugins(ScriptingPlugin)
        // Black void background
        .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // Orbit camera looking at origin from distance 15
    commands.spawn(OrbitCameraBundle::new(15.0, Vec3::ZERO));
}
