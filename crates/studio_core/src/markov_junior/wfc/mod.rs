//! Wave Function Collapse (WFC) nodes for MarkovJunior.
//!
//! This module implements WFC for procedural generation:
//! - `Wave`: Tracks possibility state and entropy for each cell
//! - `WfcNode`: Base implementation with propagation, observation, and search
//! - `OverlapNode`: Extracts NxN patterns from sample images
//! - `TileNode`: Uses pre-defined tilesets with neighbor constraints
//!
//! C# Reference: WaveFunctionCollapse.cs, OverlapNode.cs, TileNode.cs

pub mod overlap_node;
pub mod tile_node;
pub mod wave;
pub mod wfc_node;

pub use overlap_node::OverlapNode;
pub use tile_node::TileNode;
pub use wave::Wave;
pub use wfc_node::{WfcNode, WfcState};
