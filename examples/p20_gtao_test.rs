//! Phase 20: GTAO (Ground Truth Ambient Occlusion) Test
//!
//! Dedicated test for verifying GTAO implementation based on Intel's XeGTAO.
//! Uses a specially designed test scene with known geometries for AO verification.
//!
//! Run with: `cargo run --example p20_gtao_test`
//!
//! Test modes (set via environment variable GTAO_MODE):
//! - full: Full render with AO applied to lighting (default)
//! - ao_only: AO buffer visualization (grayscale)
//! - depth: View-space depth visualization
//! - normals: View-space normals visualization
//!
//! Example: `GTAO_MODE=ao_only cargo run --example p20_gtao_test`
//!
//! Expected outputs in screenshots/:
//! - p20_gtao_full.png: Complete render
//! - p20_gtao_ao_only.png: AO buffer (white=unoccluded, dark=occluded)
//!
//! Verification criteria:
//! 1. Flat ground: AO value > 0.95 (no false occlusion)
//! 2. Corner crease: AO value 0.2-0.6 (proper occlusion)
//! 3. Stairs: Smooth gradient, no banding
//! 4. Floating cube: Visible shadow on ground beneath
//! 5. Thin pillar: No excessive darkening of surrounding area

use bevy::prelude::*;
use std::env;
use studio_core::VoxelWorldApp;

fn main() {
    // Determine test mode from environment
    let mode = env::var("GTAO_MODE").unwrap_or_else(|_| "full".to_string());
    
    println!("==============================================");
    println!("  Phase 20: GTAO Verification Test");
    println!("==============================================");
    println!("Mode: {}", mode);
    println!();
    
    // Generate test world if it doesn't exist
    let world_path = "assets/worlds/gtao_test.voxworld";
    if !std::path::Path::new(world_path).exists() {
        println!("Test world not found. Run: cargo run --example generate_test_worlds");
        println!("Then re-run this test.");
        std::process::exit(1);
    }
    
    let screenshot_path = format!("screenshots/p20_gtao_{}.png", mode);
    
    // Camera positioned to see all test geometries
    let app = VoxelWorldApp::new(format!("GTAO Test - {}", mode))
        .with_world_file(world_path)
        .with_resolution(1024, 768)
        .with_deferred(true)
        .with_clear_color(Color::srgb(0.1, 0.1, 0.15))
        .with_camera_angle(35.0, 25.0)
        .with_zoom(0.7)
        .with_shadow_light(Vec3::new(8.0, 15.0, 8.0))
        .with_screenshot(&screenshot_path);
    
    match mode.as_str() {
        "full" => {
            println!("Rendering full scene with GTAO applied...");
        }
        "ao_only" => {
            println!("AO-only mode: Set DEBUG_MODE=4 in gtao.wgsl for constant output test");
            println!("              Set DEBUG_MODE=0 for actual GTAO output");
        }
        "depth" => {
            println!("Depth mode: Set DEBUG_MODE=1 in gtao.wgsl");
        }
        "normals" => {
            println!("Normals mode: Set DEBUG_MODE=2 in gtao.wgsl");
        }
        _ => {
            println!("Unknown mode '{}', using 'full'", mode);
        }
    }
    
    app.run();
    
    println!();
    println!("Screenshot saved to: {}", screenshot_path);
    println!();
    println!("Verification checklist:");
    println!("  [ ] Pipeline runs without errors");
    println!("  [ ] Output texture is not all white (occlusion is computed)");
    println!("  [ ] Output texture is not all black (not over-occluded)");
    println!("  [ ] Flat surfaces have high visibility (> 0.9)");
    println!("  [ ] Corners/creases have lower visibility (0.3-0.7)");
}
