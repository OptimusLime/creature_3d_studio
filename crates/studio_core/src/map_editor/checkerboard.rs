//! Checkerboard generator for the map editor.
//!
//! A simple pattern generator used for M1 verification.

use super::{PlaybackState, VoxelBuffer2D};
use bevy::prelude::*;

/// State for checkerboard generation.
#[derive(Resource)]
pub struct CheckerboardState {
    /// Material ID for even cells (x + y) % 2 == 0.
    pub material_a: u32,
    /// Material ID for odd cells (x + y) % 2 == 1.
    pub material_b: u32,
    /// Flag indicating the checkerboard needs to be regenerated.
    pub needs_regenerate: bool,
}

impl Default for CheckerboardState {
    fn default() -> Self {
        Self {
            material_a: 1, // stone
            material_b: 2, // dirt
            needs_regenerate: true,
        }
    }
}

impl CheckerboardState {
    /// Create a new checkerboard state with the given materials.
    pub fn new(material_a: u32, material_b: u32) -> Self {
        Self {
            material_a,
            material_b,
            needs_regenerate: true,
        }
    }

    /// Request regeneration of the checkerboard.
    pub fn request_regenerate(&mut self) {
        self.needs_regenerate = true;
    }

    /// Set the primary material (material_a).
    pub fn set_material_a(&mut self, material_id: u32) {
        if self.material_a != material_id {
            self.material_a = material_id;
            self.needs_regenerate = true;
        }
    }

    /// Set the secondary material (material_b).
    pub fn set_material_b(&mut self, material_id: u32) {
        if self.material_b != material_id {
            self.material_b = material_id;
            self.needs_regenerate = true;
        }
    }
}

/// Step one cell in checkerboard generation.
///
/// Returns true if generation is complete.
pub fn step_checkerboard(
    buffer: &mut VoxelBuffer2D,
    checker_state: &CheckerboardState,
    playback: &mut PlaybackState,
) -> bool {
    let total_cells = buffer.cell_count();
    if playback.step_index >= total_cells {
        playback.complete();
        return true;
    }

    let x = playback.step_index % buffer.width;
    let y = playback.step_index / buffer.width;

    let mat_id = if (x + y) % 2 == 0 {
        checker_state.material_a
    } else {
        checker_state.material_b
    };
    buffer.set(x, y, mat_id);

    playback.step();

    if playback.step_index >= total_cells {
        playback.complete();
        true
    } else {
        false
    }
}

/// Fill the entire buffer with a checkerboard pattern immediately.
pub fn fill_checkerboard(buffer: &mut VoxelBuffer2D, checker_state: &CheckerboardState) {
    for y in 0..buffer.height {
        for x in 0..buffer.width {
            let mat_id = if (x + y) % 2 == 0 {
                checker_state.material_a
            } else {
                checker_state.material_b
            };
            buffer.set(x, y, mat_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkerboard_pattern() {
        let mut buffer = VoxelBuffer2D::new(4, 4);
        let state = CheckerboardState::new(1, 2);
        fill_checkerboard(&mut buffer, &state);

        // Check corners
        assert_eq!(buffer.get(0, 0), 1); // even
        assert_eq!(buffer.get(1, 0), 2); // odd
        assert_eq!(buffer.get(0, 1), 2); // odd
        assert_eq!(buffer.get(1, 1), 1); // even
    }

    #[test]
    fn test_step_checkerboard() {
        let mut buffer = VoxelBuffer2D::new(2, 2);
        let state = CheckerboardState::new(1, 2);
        let mut playback = PlaybackState::default();

        // Step through all 4 cells
        assert!(!step_checkerboard(&mut buffer, &state, &mut playback));
        assert!(!step_checkerboard(&mut buffer, &state, &mut playback));
        assert!(!step_checkerboard(&mut buffer, &state, &mut playback));
        assert!(step_checkerboard(&mut buffer, &state, &mut playback)); // Complete

        assert!(playback.completed);
        assert_eq!(playback.step_index, 4);
    }
}
