//! Map Editor 2D - Phase 1 Foundation
//!
//! A 2D voxel map editor with ImGui UI, material selection, and playback controls.
//!
//! # Usage
//!
//! Interactive mode (runs forever, hot reload enabled):
//! ```bash
//! cargo run --example p_map_editor_2d
//! ```
//!
//! Screenshot mode (captures and exits):
//! ```bash
//! cargo run --example p_map_editor_2d -- --screenshot screenshots/p_map_editor_2d.png --exit-frame 45
//! ```
//!
//! # CLI Arguments
//!
//! - `--screenshot <path>` - Take a screenshot and save to path
//! - `--capture-frame <N>` - Frame to capture screenshot (default: 30)
//! - `--exit-frame <N>` - Exit after N frames (optional)

use studio_core::map_editor::MapEditor2DApp;

fn main() {
    println!("Map Editor 2D - Phase 1 Foundation");
    println!("Run with --screenshot <path> --exit-frame 45 for automated capture");
    println!("Run without args for interactive mode with hot reload");

    MapEditor2DApp::new("Map Editor 2D")
        .with_resolution(1024, 768)
        .with_cli_args()
        .run();
}
