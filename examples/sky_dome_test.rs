//! Minimal sky dome test - automated verification
//!
//! This example:
//! 1. Creates a minimal scene with just a camera and sky dome
//! 2. Waits a few frames for everything to initialize
//! 3. Takes a screenshot
//! 4. Exits
//!
//! Run with: cargo run --example sky_dome_test
//! Then run: python3 scripts/verify_sky_dome.py

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::WindowResolution;
use studio_core::deferred::{DeferredCamera, DeferredRenderingPlugin, SkyDomeConfig};

const SCREENSHOT_PATH: &str = "test_output/sky_dome_test.png";
const FRAMES_BEFORE_SCREENSHOT: u32 = 10;

fn main() {
    // Ensure output directory exists
    std::fs::create_dir_all("test_output").ok();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Sky Dome Test".into(),
                        resolution: WindowResolution::new(800, 600),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(DeferredRenderingPlugin)
        // Configure sky dome - enabled with cloud texture
        .insert_resource(SkyDomeConfig {
            enabled: true,
            // Use obvious colors for testing
            horizon_color: Color::srgb(1.0, 0.5, 0.0), // Orange
            zenith_color: Color::srgb(0.0, 0.0, 0.5),  // Dark blue
            horizon_blend_power: 1.5,
            time_of_day: 0.5, // Noon
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

fn setup(mut commands: Commands) {
    println!("=== Sky Dome Automated Test ===");
    println!(
        "Will capture screenshot after {} frames",
        FRAMES_BEFORE_SCREENSHOT
    );
    println!("Output: {}", SCREENSHOT_PATH);

    // Camera looking at empty sky (no geometry)
    // Position camera to look upward so we see mostly sky
    commands.spawn((
        Camera3d::default(),
        DeferredCamera,
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Z),
        Projection::Perspective(PerspectiveProjection {
            fov: 90.0_f32.to_radians(),
            near: 0.1,
            far: 1000.0,
            ..default()
        }),
    ));

    // NO geometry - we want to see only sky
    // The sky dome pass should fill everything since depth will be at far plane
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
            println!("Screenshot requested: {}", SCREENSHOT_PATH);
        }
    }

    if frame_count.0 > FRAMES_BEFORE_SCREENSHOT + 5 {
        println!("Test complete. Exiting.");
        app_exit.write(AppExit::Success);
    }
}
