//! P34 with automatic screenshot for debugging
//!
//! Same as p34_sky_terrain_test but takes a screenshot and exits

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::WindowResolution;
use studio_core::deferred::{DeferredCamera, DeferredRenderingPlugin, SkyDomeConfig};

const SCREENSHOT_PATH: &str = "test_output/p34_test.png";
const FRAMES_BEFORE_SCREENSHOT: u32 = 30; // More frames to let terrain render

fn main() {
    std::fs::create_dir_all("test_output").ok();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "P34 Screenshot Test".into(),
                        resolution: WindowResolution::new(800, 600),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(DeferredRenderingPlugin)
        .insert_resource(SkyDomeConfig {
            enabled: true,
            horizon_color: Color::srgb(0.8, 0.4, 0.1), // Orange
            zenith_color: Color::srgb(0.0, 0.0, 0.3),  // Dark blue
            horizon_blend_power: 1.5,
            time_of_day: 0.5,
            moons_enabled: false,
            cloud_texture_path: Some("textures/generated/mj_clouds_001.png".to_string()),
            ..default()
        })
        .init_resource::<FrameCount>()
        .add_systems(Startup, setup)
        .add_systems(Update, take_screenshot_and_exit)
        .run();
}

#[derive(Resource, Default)]
struct FrameCount(u32);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    println!("=== P34 Screenshot Test ===");

    // Camera looking at horizon (to see both terrain and sky)
    commands.spawn((
        Camera3d::default(),
        DeferredCamera,
        Transform::from_xyz(0.0, 20.0, 50.0).looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::Y),
        Projection::Perspective(PerspectiveProjection {
            fov: 70.0_f32.to_radians(),
            near: 0.1,
            far: 1000.0,
            ..default()
        }),
    ));

    // Create a simple ground plane using a cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(50.0, 1.0, 50.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.5, 0.2), // Green
            ..default()
        })),
        Transform::from_xyz(0.0, -1.0, 0.0),
        studio_core::deferred::DeferredRenderable,
    ));

    // Add a point light
    commands.spawn((
        PointLight {
            color: Color::WHITE,
            intensity: 100000.0,
            range: 100.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 30.0, 10.0),
    ));

    println!(
        "Setup complete. Taking screenshot after {} frames.",
        FRAMES_BEFORE_SCREENSHOT
    );
}

fn take_screenshot_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut app_exit: EventWriter<AppExit>,
    main_window: Query<Entity, With<bevy::window::PrimaryWindow>>,
) {
    frame_count.0 += 1;

    if frame_count.0 == FRAMES_BEFORE_SCREENSHOT {
        println!("Taking screenshot...");
        let path = std::path::PathBuf::from(SCREENSHOT_PATH);
        if let Ok(window) = main_window.single() {
            commands.entity(window).observe(save_to_disk(path));
            commands.entity(window).insert(Screenshot::window(window));
        }
    }

    if frame_count.0 > FRAMES_BEFORE_SCREENSHOT + 5 {
        println!("Done.");
        app_exit.write(AppExit::Success);
    }
}
