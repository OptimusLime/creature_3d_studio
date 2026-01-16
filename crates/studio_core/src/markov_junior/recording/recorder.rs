//! Simulation recorder for capturing grid states over time.

use super::archive::SimulationArchive;
use super::grid_type::GridType;
use super::traits::RecordableGrid;

/// Records simulation frames from any RecordableGrid.
///
/// Usage:
/// ```ignore
/// let mut recorder = SimulationRecorder::new(&model.grid);
/// recorder.record_frame(&model.grid);  // Record initial state
///
/// while model.step() {
///     recorder.record_frame(&model.grid);
/// }
///
/// let archive = recorder.into_archive();
/// archive.save("simulation.mjsim")?;
/// ```
pub struct SimulationRecorder {
    grid_type: GridType,
    palette: String,
    frames: Vec<Vec<u8>>,
    bytes_per_frame: usize,
}

impl SimulationRecorder {
    /// Create a new recorder for the given grid.
    ///
    /// Captures the grid type and palette from the grid.
    pub fn new<G: RecordableGrid>(grid: &G) -> Self {
        let grid_type = grid.grid_type();
        let palette = grid.palette();
        let bytes_per_frame = grid.bytes_per_frame();

        Self {
            grid_type,
            palette,
            frames: Vec::new(),
            bytes_per_frame,
        }
    }

    /// Create a new recorder with pre-allocated capacity.
    ///
    /// Use this when you know approximately how many frames you'll record.
    pub fn with_capacity<G: RecordableGrid>(grid: &G, frame_capacity: usize) -> Self {
        let grid_type = grid.grid_type();
        let palette = grid.palette();
        let bytes_per_frame = grid.bytes_per_frame();

        Self {
            grid_type,
            palette,
            frames: Vec::with_capacity(frame_capacity),
            bytes_per_frame,
        }
    }

    /// Record the current state of the grid as a new frame.
    ///
    /// Call this at each step of the simulation you want to capture.
    pub fn record_frame<G: RecordableGrid>(&mut self, grid: &G) {
        let state = grid.state_to_bytes();
        debug_assert_eq!(
            state.len(),
            self.bytes_per_frame,
            "Grid state size mismatch"
        );
        self.frames.push(state);
    }

    /// Number of frames recorded so far.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Total bytes used for frame storage.
    pub fn total_bytes(&self) -> usize {
        self.frames.len() * self.bytes_per_frame
    }

    /// Get the grid type.
    pub fn grid_type(&self) -> GridType {
        self.grid_type
    }

    /// Get the palette string.
    pub fn palette(&self) -> &str {
        &self.palette
    }

    /// Convert the recorder into a SimulationArchive.
    ///
    /// This consumes the recorder. The archive can then be saved to disk.
    pub fn into_archive(self) -> SimulationArchive {
        SimulationArchive::new(self.grid_type, self.palette, self.frames)
    }

    /// Clear all recorded frames, keeping the grid configuration.
    pub fn clear(&mut self) {
        self.frames.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock grid for testing
    struct MockGrid {
        width: u32,
        height: u32,
        state: Vec<u8>,
    }

    impl MockGrid {
        fn new(width: u32, height: u32) -> Self {
            let size = (width * height) as usize;
            Self {
                width,
                height,
                state: vec![0; size],
            }
        }

        fn set(&mut self, x: u32, y: u32, value: u8) {
            let idx = (x + y * self.width) as usize;
            self.state[idx] = value;
        }
    }

    impl RecordableGrid for MockGrid {
        fn grid_type(&self) -> GridType {
            GridType::Cartesian2D {
                width: self.width,
                height: self.height,
            }
        }

        fn palette(&self) -> String {
            "BW".to_string()
        }

        fn state_to_bytes(&self) -> Vec<u8> {
            self.state.clone()
        }

        fn state_from_bytes(&mut self, bytes: &[u8]) -> bool {
            if bytes.len() != self.state.len() {
                return false;
            }
            self.state.copy_from_slice(bytes);
            true
        }
    }

    #[test]
    fn test_recorder_basic() {
        let mut grid = MockGrid::new(10, 10);
        let mut recorder = SimulationRecorder::new(&grid);

        // Record initial state
        recorder.record_frame(&grid);
        assert_eq!(recorder.frame_count(), 1);

        // Modify grid and record again
        grid.set(5, 5, 1);
        recorder.record_frame(&grid);
        assert_eq!(recorder.frame_count(), 2);

        // Check archive
        let archive = recorder.into_archive();
        assert_eq!(archive.frame_count(), 2);
    }
}
