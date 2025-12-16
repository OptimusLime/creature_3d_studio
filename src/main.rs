use bevy::prelude::*;
use studio_core::CorePlugin;
use studio_physics::PhysicsPlugin;
use studio_scripting::ScriptingPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(PhysicsPlugin)
        .add_plugins(ScriptingPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
