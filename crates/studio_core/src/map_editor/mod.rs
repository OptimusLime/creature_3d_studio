//! Map Editor Module
//!
//! Provides a 2D map editor with ImGui UI for voxel-based terrain editing.
//! This module contains all the core functionality that examples and applications use.
//!
//! # Architecture
//!
//! - `Asset` / `AssetStore<T>`: Generic asset storage with search (Phase 2)
//! - `VoxelBuffer2D`: 2D grid of material IDs
//! - `Material` / `MaterialPalette`: Material definitions and selection
//! - `PlaybackState`: Step-by-step generation playback controls
//! - `CheckerboardState`: Simple checkerboard pattern generator
//! - `ImguiScreenshotPlugin`: Screenshot capture that includes ImGui panels
//! - `MapEditor2DApp`: Fluent builder for creating map editor applications
//!
//! # Example
//!
//! ```ignore
//! use studio_core::map_editor::MapEditor2DApp;
//!
//! fn main() {
//!     MapEditor2DApp::new("Map Editor 2D")
//!         .with_screenshot("screenshots/map_editor.png")
//!         .with_resolution(1024, 768)
//!         .run();
//! }
//! ```

pub mod app;
pub mod asset;
pub mod checkerboard;
pub mod imgui_screenshot;
pub mod lua_generator;
pub mod lua_materials;
pub mod material;
pub mod mcp_server;
pub mod playback;
pub mod voxel_buffer_2d;

pub use app::MapEditor2DApp;
pub use asset::{Asset, AssetStore, InMemoryStore};
pub use checkerboard::CheckerboardState;
pub use imgui_screenshot::{AutoExitConfig, ImguiScreenshotConfig, ImguiScreenshotPlugin};
pub use lua_generator::{GeneratorReloadFlag, LuaGeneratorPlugin, GENERATOR_LUA_PATH};
pub use lua_materials::{
    LuaMaterialsPlugin, MaterialsLoadSet, MaterialsReloadFlag, MATERIALS_LUA_PATH,
};
pub use material::{Material, MaterialPalette};
pub use mcp_server::{McpServerPlugin, MCP_SERVER_PORT};
pub use playback::PlaybackState;
pub use voxel_buffer_2d::VoxelBuffer2D;
