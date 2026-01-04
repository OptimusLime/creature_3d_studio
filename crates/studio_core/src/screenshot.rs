//! Screenshot capture utilities for testing and verification.
//!
//! This module provides functionality to capture screenshots from Bevy
//! for use in automated tests and visual verification.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

/// Spawns a screenshot capture command that will save to the given path.
///
/// This uses Bevy's built-in screenshot system which captures the primary window.
/// The screenshot is saved asynchronously after the current frame renders.
///
/// # Example
/// ```ignore
/// fn capture_system(mut commands: Commands) {
///     capture_screenshot(&mut commands, "screenshots/test.png");
/// }
/// ```
pub fn capture_screenshot(commands: &mut Commands, path: impl Into<String>) {
    let path_string = path.into();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path_string));
}

/// Resource to request a screenshot be taken on the next frame.
#[derive(Resource, Default)]
pub struct ScreenshotRequest {
    /// Path where the screenshot should be saved.
    pub path: Option<String>,
    /// Whether a screenshot has been requested but not yet captured.
    pub pending: bool,
}

impl ScreenshotRequest {
    /// Request a screenshot to be saved at the given path.
    pub fn request(&mut self, path: impl Into<String>) {
        self.path = Some(path.into());
        self.pending = true;
    }

    /// Clear the pending request (called after capture is initiated).
    pub fn clear(&mut self) {
        self.pending = false;
    }
}

/// System that processes screenshot requests.
///
/// Add this to your app's Update schedule to enable screenshot requests.
pub fn process_screenshot_requests(mut commands: Commands, mut request: ResMut<ScreenshotRequest>) {
    if request.pending {
        if let Some(path) = request.path.take() {
            capture_screenshot(&mut commands, path);
        }
        request.clear();
    }
}

/// Plugin that adds screenshot capture functionality.
pub struct ScreenshotPlugin;

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenshotRequest>()
            .add_systems(Update, process_screenshot_requests);
    }
}
