//! Phase 1 Screenshot Test: Black void with 3D camera.
//!
//! This test verifies black background and 3D camera setup.
//! Run with: `cargo run --example p1_black_void_test`
//!
//! Expected output: `screenshots/p1_black_void.png` - solid black image.

use bevy::prelude::*;
use bevy::app::AppExit;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p1_black_void.png";

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");
    
    println!("Running Phase 1 Screenshot Test: Black Void...");
    println!("Expected output: {} (solid black)", SCREENSHOT_PATH);
    
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 1: Black Void".into(),
                ..default()
            }),
            ..default()
        }))
        // Black clear color - RGB(0,0,0)
        .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
        .insert_resource(FrameCount(0))
        .add_systems(Startup, setup_camera)
        .add_systems(Update, capture_and_exit)
        .run();
    
    if Path::new(SCREENSHOT_PATH).exists() {
        println!("SUCCESS: Screenshot saved to {}", SCREENSHOT_PATH);
    } else {
        println!("FAILED: Screenshot was not created at {}", SCREENSHOT_PATH);
        std::process::exit(1);
    }
}

#[derive(Resource)]
struct FrameCount(u32);

fn setup_camera(mut commands: Commands) {
    // 3D camera at position (0, 5, 10) looking at origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;
    
    if frame_count.0 == 5 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }
    
    if frame_count.0 >= 15 {
        println!("Exiting after {} frames", frame_count.0);
        exit.write(AppExit::Success);
    }
}
