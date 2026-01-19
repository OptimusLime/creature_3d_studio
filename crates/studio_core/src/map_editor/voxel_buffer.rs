//! Unified VoxelBuffer - THE authoritative storage for material IDs.
//!
//! This module provides a single buffer implementation that:
//! - Supports both 2D (depth=1) and 3D modes
//! - Uses interior mutability for safe sharing between systems
//! - Can be cloned cheaply (Arc clone)
//! - Can be passed to Lua as UserData
//! - Can be read directly for rendering
//! - Tracks last write and pending step info for generator integration
//!
//! # Why One Buffer?
//!
//! Previously we had three separate implementations:
//! - `SharedBuffer` (Arc<Mutex>) for Lua writing
//! - `VoxelBuffer2D` (Vec<u32>) for rendering
//! - `MjGrid` (Vec<u8>) for Markov Jr.
//!
//! This caused unnecessary copies and complexity. Now:
//! - Generators write directly to VoxelBuffer
//! - Renderer reads directly from VoxelBuffer
//! - MJ does one batch copy (Rust function, not N Lua calls)
//!
//! # Example
//!
//! ```ignore
//! let buffer = VoxelBuffer::new_2d(32, 32);
//!
//! // Write from any thread/system
//! buffer.set_2d(10, 10, 5);
//!
//! // Read for rendering
//! let mat_id = buffer.get_2d(10, 10);
//!
//! // Clone is cheap (Arc clone)
//! let buffer2 = buffer.clone();
//! ```

use bevy::prelude::*;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Read-only trait for voxel data access.
///
/// Implemented by `VoxelBuffer` and can be implemented by other types
/// (e.g., MjGridView for zero-copy MJ rendering in special cases).
pub trait VoxelGrid: Send + Sync {
    /// Width of the grid in cells.
    fn width(&self) -> usize;

    /// Height of the grid in cells.
    fn height(&self) -> usize;

    /// Depth of the grid in cells (1 for 2D).
    fn depth(&self) -> usize;

    /// Get the material ID at position (x, y, z).
    /// Returns 0 if coordinates are out of bounds.
    fn get(&self, x: usize, y: usize, z: usize) -> u32;

    /// Get the material ID at position (x, y) with z=0.
    /// Convenience method for 2D access.
    fn get_2d(&self, x: usize, y: usize) -> u32 {
        self.get(x, y, 0)
    }
}

/// Information about the last voxel write for step tracking.
#[derive(Clone, Default)]
pub struct LastWrite {
    pub x: usize,
    pub y: usize,
    pub material_id: u32,
    pub written: bool,
}

/// Step info emitted from Lua, pending collection by Rust.
#[derive(Clone, Default)]
pub struct PendingStepInfo {
    pub path: String,
    pub step_number: usize,
    pub x: usize,
    pub y: usize,
    pub material_id: u32,
    pub completed: bool,
    pub rule_name: Option<String>,
    pub affected_cells: Option<usize>,
}

/// Unified voxel buffer - THE authoritative storage for material IDs.
///
/// Uses interior mutability via `Arc<RwLock<...>>` for safe sharing.
/// Clone is cheap (Arc clone). Supports both 2D and 3D.
/// Also tracks last write and pending step info for generator integration.
#[derive(Resource, Clone)]
pub struct VoxelBuffer {
    data: Arc<RwLock<Vec<u32>>>,
    last_write: Arc<Mutex<LastWrite>>,
    pending_steps: Arc<Mutex<Vec<PendingStepInfo>>>,
    width: usize,
    height: usize,
    depth: usize,
}

impl VoxelBuffer {
    /// Create a 2D buffer (depth = 1).
    pub fn new_2d(width: usize, height: usize) -> Self {
        Self {
            data: Arc::new(RwLock::new(vec![0; width * height])),
            last_write: Arc::new(Mutex::new(LastWrite::default())),
            pending_steps: Arc::new(Mutex::new(Vec::new())),
            width,
            height,
            depth: 1,
        }
    }

    /// Create a 3D buffer.
    pub fn new_3d(width: usize, height: usize, depth: usize) -> Self {
        Self {
            data: Arc::new(RwLock::new(vec![0; width * height * depth])),
            last_write: Arc::new(Mutex::new(LastWrite::default())),
            pending_steps: Arc::new(Mutex::new(Vec::new())),
            width,
            height,
            depth,
        }
    }

    /// Width of the buffer.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Height of the buffer.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Depth of the buffer (1 for 2D).
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Calculate linear index from 3D coordinates.
    #[inline]
    fn index(&self, x: usize, y: usize, z: usize) -> Option<usize> {
        if x < self.width && y < self.height && z < self.depth {
            Some(z * self.width * self.height + y * self.width + x)
        } else {
            None
        }
    }

    /// Write a voxel at (x, y, z). Thread-safe.
    /// Does nothing if coordinates are out of bounds.
    /// Also tracks this as the last write for step info.
    pub fn set(&self, x: usize, y: usize, z: usize, material_id: u32) {
        if let Some(idx) = self.index(x, y, z) {
            if let Ok(mut data) = self.data.write() {
                data[idx] = material_id;
            }
            // Track as last write
            if let Ok(mut last) = self.last_write.lock() {
                last.x = x;
                last.y = y;
                last.material_id = material_id;
                last.written = true;
            }
        }
    }

    /// Write a voxel at (x, y) with z=0. Thread-safe.
    /// Convenience method for 2D access.
    pub fn set_2d(&self, x: usize, y: usize, material_id: u32) {
        self.set(x, y, 0, material_id);
    }

    /// Read a voxel at (x, y, z). Thread-safe.
    /// Returns 0 if coordinates are out of bounds.
    pub fn get(&self, x: usize, y: usize, z: usize) -> u32 {
        if let Some(idx) = self.index(x, y, z) {
            if let Ok(data) = self.data.read() {
                return data[idx];
            }
        }
        0
    }

    /// Read a voxel at (x, y) with z=0. Thread-safe.
    /// Convenience method for 2D access.
    pub fn get_2d(&self, x: usize, y: usize) -> u32 {
        self.get(x, y, 0)
    }

    /// Clear all voxels to 0 and reset tracking state.
    pub fn clear(&self) {
        if let Ok(mut data) = self.data.write() {
            data.fill(0);
        }
        if let Ok(mut last) = self.last_write.lock() {
            *last = LastWrite::default();
        }
        if let Ok(mut pending) = self.pending_steps.lock() {
            pending.clear();
        }
    }

    /// Take the last write info, clearing the written flag.
    pub fn take_last_write(&self) -> Option<(usize, usize, u32)> {
        if let Ok(mut last) = self.last_write.lock() {
            if last.written {
                last.written = false;
                return Some((last.x, last.y, last.material_id));
            }
        }
        None
    }

    /// Emit a step info (called from generators).
    pub fn emit_step(&self, info: PendingStepInfo) {
        if let Ok(mut pending) = self.pending_steps.lock() {
            pending.push(info);
        }
    }

    /// Take all pending step infos.
    pub fn take_pending_steps(&self) -> Vec<PendingStepInfo> {
        if let Ok(mut pending) = self.pending_steps.lock() {
            std::mem::take(&mut *pending)
        } else {
            Vec::new()
        }
    }

    /// Get read access to underlying data (for batch operations).
    pub fn read(&self) -> Option<RwLockReadGuard<Vec<u32>>> {
        self.data.read().ok()
    }

    /// Get write access to underlying data (for batch operations).
    pub fn write(&self) -> Option<RwLockWriteGuard<Vec<u32>>> {
        self.data.write().ok()
    }

    /// Total number of cells in the buffer.
    pub fn cell_count(&self) -> usize {
        self.width * self.height * self.depth
    }

    /// Copy data from an MjGrid with value-to-material translation.
    /// This is the batch copy that replaces N per-pixel Lua calls.
    ///
    /// # Arguments
    /// * `mj_data` - Raw MjGrid state data (flattened, z=0 slice for 2D)
    /// * `mj_width` - Width of the MjGrid
    /// * `mj_height` - Height of the MjGrid
    /// * `value_to_material` - Translation table: mj_value -> material_id
    pub fn copy_from_mj_grid(
        &self,
        mj_data: &[u8],
        mj_width: usize,
        mj_height: usize,
        value_to_material: &[u32],
    ) {
        if let Ok(mut data) = self.data.write() {
            let mx = mj_width.min(self.width);
            let my = mj_height.min(self.height);
            for y in 0..my {
                for x in 0..mx {
                    let mj_idx = y * mj_width + x;
                    let val = mj_data.get(mj_idx).copied().unwrap_or(0) as usize;
                    let mat_id = value_to_material.get(val).copied().unwrap_or(0);
                    let idx = y * self.width + x;
                    data[idx] = mat_id;
                }
            }
        }
    }
}

impl VoxelGrid for VoxelBuffer {
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn depth(&self) -> usize {
        self.depth
    }

    fn get(&self, x: usize, y: usize, z: usize) -> u32 {
        VoxelBuffer::get(self, x, y, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_2d_buffer() {
        let buf = VoxelBuffer::new_2d(4, 4);
        assert_eq!(buf.width(), 4);
        assert_eq!(buf.height(), 4);
        assert_eq!(buf.depth(), 1);
        assert_eq!(buf.cell_count(), 16);
    }

    #[test]
    fn test_new_3d_buffer() {
        let buf = VoxelBuffer::new_3d(4, 4, 8);
        assert_eq!(buf.width(), 4);
        assert_eq!(buf.height(), 4);
        assert_eq!(buf.depth(), 8);
        assert_eq!(buf.cell_count(), 128);
    }

    #[test]
    fn test_set_get_2d() {
        let buf = VoxelBuffer::new_2d(4, 4);
        buf.set_2d(1, 2, 42);
        assert_eq!(buf.get_2d(1, 2), 42);
        assert_eq!(buf.get_2d(0, 0), 0);
    }

    #[test]
    fn test_set_get_3d() {
        let buf = VoxelBuffer::new_3d(4, 4, 4);
        buf.set(1, 2, 3, 99);
        assert_eq!(buf.get(1, 2, 3), 99);
        assert_eq!(buf.get(0, 0, 0), 0);
    }

    #[test]
    fn test_out_of_bounds() {
        let buf = VoxelBuffer::new_2d(4, 4);
        buf.set_2d(10, 10, 99); // Should do nothing
        assert_eq!(buf.get_2d(10, 10), 0);
    }

    #[test]
    fn test_clear() {
        let buf = VoxelBuffer::new_2d(4, 4);
        buf.set_2d(1, 1, 5);
        buf.set_2d(2, 2, 10);
        buf.clear();
        assert_eq!(buf.get_2d(1, 1), 0);
        assert_eq!(buf.get_2d(2, 2), 0);
    }

    #[test]
    fn test_clone_shares_data() {
        let buf1 = VoxelBuffer::new_2d(4, 4);
        let buf2 = buf1.clone();

        buf1.set_2d(1, 1, 42);
        assert_eq!(buf2.get_2d(1, 1), 42); // Clone sees the write
    }

    #[test]
    fn test_voxel_grid_trait() {
        let buf = VoxelBuffer::new_2d(4, 4);
        buf.set_2d(1, 2, 42);

        // Access via trait object
        let grid: &dyn VoxelGrid = &buf;
        assert_eq!(grid.width(), 4);
        assert_eq!(grid.height(), 4);
        assert_eq!(grid.depth(), 1);
        assert_eq!(grid.get(1, 2, 0), 42);
        assert_eq!(grid.get_2d(1, 2), 42);
    }

    #[test]
    fn test_copy_from_mj_grid() {
        let buf = VoxelBuffer::new_2d(4, 4);

        // Simulated MJ grid data: 4x4, values 0 and 1
        let mj_data: Vec<u8> = vec![
            0, 1, 0, 1, // row 0
            1, 0, 1, 0, // row 1
            0, 1, 0, 1, // row 2
            1, 0, 1, 0, // row 3
        ];

        // Translation: 0 -> material 10, 1 -> material 20
        let value_to_material = vec![10, 20];

        buf.copy_from_mj_grid(&mj_data, 4, 4, &value_to_material);

        assert_eq!(buf.get_2d(0, 0), 10); // 0 -> 10
        assert_eq!(buf.get_2d(1, 0), 20); // 1 -> 20
        assert_eq!(buf.get_2d(0, 1), 20); // 1 -> 20
        assert_eq!(buf.get_2d(1, 1), 10); // 0 -> 10
    }

    #[test]
    fn test_batch_write() {
        let buf = VoxelBuffer::new_2d(4, 4);

        // Use write() for batch operations
        {
            let mut data = buf.write().unwrap();
            for i in 0..16 {
                data[i] = i as u32;
            }
        }

        assert_eq!(buf.get_2d(0, 0), 0);
        assert_eq!(buf.get_2d(1, 0), 1);
        assert_eq!(buf.get_2d(0, 1), 4);
        assert_eq!(buf.get_2d(3, 3), 15);
    }

    #[test]
    fn test_last_write_tracking() {
        let buf = VoxelBuffer::new_2d(4, 4);

        // No writes yet
        assert!(buf.take_last_write().is_none());

        // Write and check
        buf.set_2d(1, 2, 42);
        let last = buf.take_last_write();
        assert_eq!(last, Some((1, 2, 42)));

        // Take clears the flag
        assert!(buf.take_last_write().is_none());
    }

    #[test]
    fn test_pending_steps() {
        let buf = VoxelBuffer::new_2d(4, 4);

        // Emit some step info
        buf.emit_step(PendingStepInfo {
            path: "root.step_1".to_string(),
            step_number: 1,
            x: 5,
            y: 6,
            material_id: 10,
            completed: false,
            rule_name: Some("WB=WW".to_string()),
            affected_cells: Some(3),
        });

        buf.emit_step(PendingStepInfo {
            path: "root.step_2".to_string(),
            step_number: 2,
            ..Default::default()
        });

        // Take all pending
        let pending = buf.take_pending_steps();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].path, "root.step_1");
        assert_eq!(pending[1].path, "root.step_2");

        // Take again is empty
        assert!(buf.take_pending_steps().is_empty());
    }

    #[test]
    fn test_clear_resets_tracking() {
        let buf = VoxelBuffer::new_2d(4, 4);

        buf.set_2d(1, 1, 5);
        buf.emit_step(PendingStepInfo::default());

        buf.clear();

        // Everything cleared
        assert_eq!(buf.get_2d(1, 1), 0);
        assert!(buf.take_last_write().is_none());
        assert!(buf.take_pending_steps().is_empty());
    }
}
