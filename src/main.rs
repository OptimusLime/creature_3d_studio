use bevy::prelude::*;
use studio_core::{
    CorePlugin, DeferredPointLight, DeferredRenderingPlugin, OrbitCameraBundle, VoxelMaterialPlugin,
};
use studio_physics::PhysicsPlugin;
use studio_scripting::ScriptingPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(VoxelMaterialPlugin) // Required for voxel mesh materials
        .add_plugins(DeferredRenderingPlugin) // Required for deferred lighting
        .add_plugins(PhysicsPlugin)
        .add_plugins(ScriptingPlugin)
        // Dark background
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.08)))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // Orbit camera looking at origin from distance 20
    commands.spawn(OrbitCameraBundle::new(20.0, Vec3::ZERO));

    // Add a deferred point light for the generated voxels
    // Position above and to the side for good lighting angle
    commands.spawn((
        DeferredPointLight {
            color: Color::srgb(1.0, 0.95, 0.9),
            intensity: 60.0,
            radius: 40.0,
        },
        Transform::from_xyz(12.0, 18.0, 12.0),
    ));
}
