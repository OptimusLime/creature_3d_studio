//! Phase 0 Screenshot Test: Capture a solid magenta screenshot.
//!
//! This test verifies that the screenshot infrastructure works.
//! Run with: `cargo run --example p0_screenshot_test`
//!
//! Expected output: `screenshots/p0_test.png` containing solid magenta (#FF00FF).

use bevy::prelude::*;
use bevy::app::AppExit;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;

const SCREENSHOT_DIR: &str = "screenshots";
const SCREENSHOT_PATH: &str = "screenshots/p0_test.png";

fn main() {
    // Ensure screenshots directory exists
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");
    
    println!("Running Phase 0 Screenshot Test...");
    println!("Expected output: {} (solid magenta)", SCREENSHOT_PATH);
    
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Phase 0: Screenshot Test".into(),
                ..default()
            }),
            ..default()
        }))
        // Magenta clear color (#FF00FF)
        .insert_resource(ClearColor(Color::srgb(1.0, 0.0, 1.0)))
        .insert_resource(FrameCount(0))
        .add_systems(Startup, setup_camera)
        .add_systems(Update, capture_and_exit)
        .run();
    
    // Verify the file was created
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
    // Simple 2D camera - we just want to see the clear color
    commands.spawn(Camera2d);
}

#[allow(deprecated)]
fn capture_and_exit(
    mut commands: Commands,
    mut frame_count: ResMut<FrameCount>,
    mut exit: EventWriter<AppExit>,
) {
    frame_count.0 += 1;
    
    // Wait a few frames for everything to initialize, then capture
    if frame_count.0 == 5 {
        println!("Capturing screenshot at frame {}...", frame_count.0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(SCREENSHOT_PATH));
    }
    
    // Exit after screenshot is captured (give it a few more frames)
    if frame_count.0 >= 15 {
        println!("Exiting after {} frames", frame_count.0);
        exit.write(AppExit::Success);
    }
}
