//! Screenshot Sequence Capture System
//!
//! Captures a series of screenshots at specific times in the day/night cycle.
//! Useful for visualizing the full cycle progression in a single folder.
//!
//! # Usage
//!
//! ```ignore
//! use studio_core::{ScreenshotSequence, VoxelWorldApp};
//!
//! VoxelWorldApp::new("Day Night Demo")
//!     .with_day_night_cycle(DayNightCycle::dark_world())
//!     .with_screenshot_sequence(ScreenshotSequence::evenly_spaced(
//!         "screenshots/day_night_cycle",
//!         24,
//!     ))
//!     .run();
//! ```

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::Path;

use crate::day_night::DayNightCycle;

/// Configuration for capturing a screenshot sequence.
#[derive(Resource, Clone)]
pub struct ScreenshotSequence {
    /// Output directory for screenshots.
    pub output_dir: String,

    /// Times to capture (0.0 - 1.0 in cycle time).
    /// If empty, captures at evenly spaced intervals based on frame_count.
    pub capture_times: Vec<f32>,

    /// Current capture index.
    pub current_frame: usize,

    /// Whether the sequence is active.
    pub active: bool,

    /// Frames to wait between captures (for GPU/scene to settle).
    pub settle_frames: u32,

    /// Current settle frame counter.
    pub settle_counter: u32,

    /// State machine: waiting for time set, settling, capturing.
    pub state: SequenceState,
}

/// State machine for screenshot sequence capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SequenceState {
    /// Waiting to set cycle time and start settling.
    #[default]
    WaitingToSetTime,
    /// Settling (waiting for scene to stabilize).
    Settling,
    /// Capture screenshot this frame.
    Capturing,
    /// Screenshot was captured, move to next.
    Advancing,
}

impl ScreenshotSequence {
    /// Create a sequence with evenly spaced captures.
    ///
    /// # Arguments
    /// * `output_dir` - Directory to save screenshots
    /// * `count` - Number of screenshots to capture (evenly distributed across 0.0-1.0)
    pub fn evenly_spaced(output_dir: impl Into<String>, count: usize) -> Self {
        let times: Vec<f32> = (0..count).map(|i| i as f32 / count as f32).collect();

        Self {
            output_dir: output_dir.into(),
            capture_times: times,
            current_frame: 0,
            active: true,
            settle_frames: 3,
            settle_counter: 0,
            state: SequenceState::WaitingToSetTime,
        }
    }

    /// Create a sequence capturing at specific times.
    ///
    /// # Arguments
    /// * `output_dir` - Directory to save screenshots
    /// * `times` - Specific cycle times (0.0 - 1.0) to capture
    pub fn at_times(output_dir: impl Into<String>, times: Vec<f32>) -> Self {
        Self {
            output_dir: output_dir.into(),
            capture_times: times,
            current_frame: 0,
            active: true,
            settle_frames: 3,
            settle_counter: 0,
            state: SequenceState::WaitingToSetTime,
        }
    }

    /// Create a sequence with key times for dark world cycle.
    ///
    /// Captures at significant transition points:
    /// - Deep night, purple moon high, dawn transition, orange moon rising,
    /// - Orange moon high, dusk transition, purple moon rising, pre-midnight
    pub fn dark_world_highlights(output_dir: impl Into<String>) -> Self {
        Self::at_times(
            output_dir,
            vec![
                0.0,  // Deep night
                0.15, // Purple moon high
                0.3,  // Dawn transition
                0.45, // Orange moon rising
                0.5,  // Orange moon high
                0.65, // Dusk transition
                0.85, // Purple moon rising
                0.95, // Pre-midnight
            ],
        )
    }

    /// Set number of settle frames (default: 3).
    pub fn with_settle_frames(mut self, frames: u32) -> Self {
        self.settle_frames = frames;
        self
    }

    /// Get the target time for the current capture.
    pub fn current_target_time(&self) -> Option<f32> {
        self.capture_times.get(self.current_frame).copied()
    }

    /// Get the filename for the current capture.
    pub fn current_filename(&self) -> Option<String> {
        self.current_target_time().map(|time| {
            format!(
                "{}/cycle_{:03}_{:.2}.png",
                self.output_dir, self.current_frame, time
            )
        })
    }

    /// Check if the sequence is complete.
    pub fn is_complete(&self) -> bool {
        self.current_frame >= self.capture_times.len()
    }
}

/// System to capture screenshot sequence at cycle times.
///
/// This system:
/// 1. Pauses the day/night cycle
/// 2. Sets cycle time to target
/// 3. Waits for settle frames
/// 4. Captures screenshot
/// 5. Advances to next capture time
/// 6. Exits when complete
#[allow(deprecated)]
pub fn capture_screenshot_sequence(
    mut commands: Commands,
    mut sequence: ResMut<ScreenshotSequence>,
    mut cycle: ResMut<DayNightCycle>,
    mut app_exit: EventWriter<bevy::app::AppExit>,
) {
    if !sequence.active || sequence.is_complete() {
        if sequence.active {
            sequence.active = false;
            println!(
                "Screenshot sequence complete: {} frames in {}",
                sequence.capture_times.len(),
                sequence.output_dir
            );
            app_exit.write(bevy::app::AppExit::Success);
        }
        return;
    }

    match sequence.state {
        SequenceState::WaitingToSetTime => {
            // Get target time and set cycle
            if let Some(target_time) = sequence.current_target_time() {
                cycle.paused = true;
                cycle.set_time(target_time);
                sequence.settle_counter = 0;
                sequence.state = SequenceState::Settling;
            } else {
                // No more times, complete
                sequence.active = false;
            }
        }

        SequenceState::Settling => {
            sequence.settle_counter += 1;
            if sequence.settle_counter >= sequence.settle_frames {
                sequence.state = SequenceState::Capturing;
            }
        }

        SequenceState::Capturing => {
            if let Some(filename) = sequence.current_filename() {
                // Ensure directory exists
                if let Some(parent) = Path::new(&filename).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                let time = sequence.current_target_time().unwrap_or(0.0);
                println!(
                    "Capturing cycle screenshot: {} (time={:.2})",
                    filename, time
                );

                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(filename));

                sequence.state = SequenceState::Advancing;
            }
        }

        SequenceState::Advancing => {
            // Screenshot was queued last frame, advance to next
            sequence.current_frame += 1;
            sequence.state = SequenceState::WaitingToSetTime;
        }
    }
}

/// Plugin for screenshot sequence functionality.
pub struct ScreenshotSequencePlugin;

impl Plugin for ScreenshotSequencePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            capture_screenshot_sequence.run_if(resource_exists::<ScreenshotSequence>),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evenly_spaced_times() {
        let seq = ScreenshotSequence::evenly_spaced("test", 4);
        assert_eq!(seq.capture_times, vec![0.0, 0.25, 0.5, 0.75]);
    }

    #[test]
    fn test_filename_format() {
        let seq = ScreenshotSequence::at_times("screenshots/test", vec![0.0, 0.5, 1.0]);
        assert_eq!(
            seq.current_filename(),
            Some("screenshots/test/cycle_000_0.00.png".to_string())
        );
    }

    #[test]
    fn test_dark_world_highlights() {
        let seq = ScreenshotSequence::dark_world_highlights("test");
        assert_eq!(seq.capture_times.len(), 8);
        assert_eq!(seq.capture_times[0], 0.0);
        assert_eq!(seq.capture_times[4], 0.5);
    }
}
