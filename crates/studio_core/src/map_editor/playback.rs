//! Playback state for step-by-step generation.
//!
//! Controls play/pause, stepping, and speed for watching generation progress.

use bevy::prelude::*;

/// Playback state for step-by-step generation.
#[derive(Resource)]
pub struct PlaybackState {
    /// Whether generation is currently playing (auto-advancing).
    pub playing: bool,
    /// Speed in cells per second (1.0 to 1000.0).
    pub speed: f32,
    /// Current step index (number of cells filled).
    pub step_index: usize,
    /// Time accumulator for sub-frame stepping.
    pub accumulator: f32,
    /// Whether generation has completed.
    pub completed: bool,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            playing: false,
            speed: 100.0,
            step_index: 0,
            accumulator: 0.0,
            completed: false,
        }
    }
}

impl PlaybackState {
    /// Create new playback state with the given speed.
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            ..Default::default()
        }
    }

    /// Toggle between playing and paused.
    pub fn toggle_play(&mut self) {
        self.playing = !self.playing;
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Start playing.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        self.playing = false;
        self.step_index = 0;
        self.accumulator = 0.0;
        self.completed = false;
    }

    /// Mark as completed.
    pub fn complete(&mut self) {
        self.completed = true;
        self.playing = false;
    }

    /// Advance by one step.
    pub fn step(&mut self) {
        if !self.completed {
            self.step_index += 1;
        }
    }

    /// Set the speed (clamped to 1.0-1000.0).
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(1.0, 1000.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let state = PlaybackState::default();
        assert!(!state.playing);
        assert_eq!(state.speed, 100.0);
        assert_eq!(state.step_index, 0);
        assert!(!state.completed);
    }

    #[test]
    fn test_toggle_play() {
        let mut state = PlaybackState::default();
        state.toggle_play();
        assert!(state.playing);
        state.toggle_play();
        assert!(!state.playing);
    }

    #[test]
    fn test_reset() {
        let mut state = PlaybackState::default();
        state.step_index = 50;
        state.completed = true;
        state.playing = true;
        state.reset();
        assert_eq!(state.step_index, 0);
        assert!(!state.completed);
        assert!(!state.playing);
    }
}
