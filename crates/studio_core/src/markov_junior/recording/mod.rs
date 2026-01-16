//! Simulation recording and video export for MarkovJunior.
//!
//! This module provides infrastructure for:
//! - Recording simulation states to binary archives (.mjsim)
//! - Loading and playing back recorded simulations
//! - Exporting simulations to video (MP4)
//!
//! Supports all MarkovJunior grid types:
//! - Cartesian 2D (standard rectangular grids)
//! - Cartesian 3D (voxel grids)
//! - Polar 2D (ring/disc grids)
//! - Polar 3D (spherical grids, future)
//!
//! # Example
//!
//! ```ignore
//! use studio_core::markov_junior::recording::*;
//!
//! // Record a simulation
//! let mut recorder = SimulationRecorder::new(&model.grid);
//! recorder.record_frame(&model.grid);
//!
//! while model.step() {
//!     recorder.record_frame(&model.grid);
//! }
//!
//! // Save to archive
//! let archive = recorder.into_archive();
//! archive.save("simulation.mjsim").unwrap();
//!
//! // Export to video
//! let archive = SimulationArchive::load("simulation.mjsim").unwrap();
//! let exporter = VideoExporter::new(archive, colors, 512);
//! exporter.export_mp4("simulation.mp4", 10.0, 30).unwrap();
//! ```

mod archive;
mod grid_type;
mod recorder;
#[cfg(test)]
mod tests;
mod traits;
mod video;

pub use archive::{ArchiveError, SimulationArchive};
pub use grid_type::{GridType, GridTypeId};
pub use recorder::SimulationRecorder;
pub use traits::{
    default_colors_for_palette, RecordableGrid, Renderable2D, Renderable3D, VoxelData,
};
pub use video::{VideoError, VideoExporter};
