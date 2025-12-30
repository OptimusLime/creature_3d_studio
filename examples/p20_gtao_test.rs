//! Phase 20: GTAO (Ground Truth Ambient Occlusion) Test
//!
//! Dedicated test for verifying GTAO implementation based on Intel's XeGTAO.
//! Uses a specially designed test scene with known geometries for AO verification.
//!
//! Run with: `cargo run --example p20_gtao_test`
//!
//! This test captures multiple debug screenshots to verify each stage of the
//! GTAO pipeline:
//! - render.png: Final rendered scene with GTAO applied
//! - ao_only.png: Raw GTAO output (grayscale)
//! - gtao_depth.png: Linear viewspace depth
//! - gtao_normal.png: View-space normals
//! - gtao_edges.png: Packed edges for denoiser
//! - gtao_radius.png: Screenspace sample radius
//!
//! All screenshots are saved to: screenshots/gtao_test/
//!
//! Verification criteria:
//! 1. Flat ground: AO value > 0.95 (no false occlusion)
//! 2. Corner crease: AO value 0.2-0.6 (proper occlusion)
//! 3. Stairs: Smooth gradient, no banding
//! 4. Floating cube: Visible shadow on ground beneath
//! 5. Thin pillar: No excessive darkening of surrounding area

use bevy::prelude::*;
use studio_core::{DebugCapture, DebugScreenshotConfig, VoxelWorldApp};

fn main() {
    println!("==============================================");
    println!("  Phase 20: GTAO Verification Test");
    println!("==============================================");
    println!();

    // Generate test world if it doesn't exist
    let world_path = "assets/worlds/gtao_test.voxworld";
    if !std::path::Path::new(world_path).exists() {
        println!("Test world not found. Run: cargo run --example generate_test_worlds");
        println!("Then re-run this test.");
        std::process::exit(1);
    }

    // Configure debug screenshots for systematic verification
    // 
    // IMPORTANT: GTAO debug modes write to the GTAO texture, which is then sampled
    // by the lighting shader. To SEE the GTAO debug output, we must ALSO set
    // lighting_debug_mode = 5 (AO only) so the lighting shader passes through
    // the GTAO texture values directly.
    let debug_config = DebugScreenshotConfig::new("screenshots/gtao_test")
        .with_base_wait_frames(15) // Wait for scene to stabilize
        // Normal render - final output with GTAO
        .with_capture("render", DebugCapture::default())
        // GTAO intermediates - set BOTH gtao debug mode AND lighting=5 to view raw output
        .with_capture("ao_only", DebugCapture::lighting_debug(5))  // Normal GTAO (mode 0) viewed as grayscale
        .with_capture("gtao_depth", DebugCapture {
            name: "gtao_depth".to_string(),
            gtao_debug_mode: 11,      // Linear viewspace depth
            lighting_debug_mode: 5,   // Pass through as grayscale
            wait_frames: 5,
        })
        .with_capture("gtao_normal", DebugCapture {
            name: "gtao_normal".to_string(),
            gtao_debug_mode: 20,      // View-space normal.z
            lighting_debug_mode: 5,   // Pass through as grayscale
            wait_frames: 5,
        })
        .with_capture("gtao_edges", DebugCapture {
            name: "gtao_edges".to_string(),
            gtao_debug_mode: 40,      // Packed edges
            lighting_debug_mode: 5,   // Pass through as grayscale
            wait_frames: 5,
        })
        .with_capture("gtao_radius", DebugCapture {
            name: "gtao_radius".to_string(),
            gtao_debug_mode: 30,      // Screenspace radius
            lighting_debug_mode: 5,   // Pass through as grayscale
            wait_frames: 5,
        })
        // G-buffer debug (uses lighting debug modes directly)
        .with_capture("gbuffer_normals", DebugCapture::lighting_debug(1))
        .with_capture("gbuffer_depth", DebugCapture::lighting_debug(2));

    println!("Capturing {} debug screenshots to screenshots/gtao_test/", debug_config.captures.len());
    println!();

    // Camera positioned to see all test geometries
    let app = VoxelWorldApp::new("GTAO Verification Test")
        .with_world_file(world_path)
        .with_resolution(1024, 768)
        .with_deferred(true)
        .with_clear_color(Color::srgb(0.1, 0.1, 0.15))
        .with_camera_angle(35.0, 25.0)
        .with_zoom(0.7)
        .with_shadow_light(Vec3::new(8.0, 15.0, 8.0))
        .with_debug_screenshots(debug_config);

    app.run();

    println!();
    println!("==============================================");
    println!("  Verification Checklist");
    println!("==============================================");
    println!();
    println!("Check screenshots/gtao_test/ for the following:");
    println!();
    println!("  render.png:");
    println!("    [ ] Scene renders without crashes");
    println!("    [ ] Corners are darker than flat surfaces");
    println!("    [ ] No excessive noise or artifacts");
    println!();
    println!("  ao_only.png:");
    println!("    [ ] Flat surfaces are white/bright (AO > 0.9)");
    println!("    [ ] Corners show darkening (AO 0.3-0.7)");
    println!("    [ ] No patchy noise patterns");
    println!("    [ ] Smooth gradients, no banding");
    println!();
    println!("  gtao_depth.png:");
    println!("    [ ] Smooth depth gradient (near=dark, far=bright)");
    println!("    [ ] No NaN/Inf artifacts (magenta pixels)");
    println!();
    println!("  gtao_normal.png:");
    println!("    [ ] Surfaces facing camera are bright");
    println!("    [ ] Side surfaces are medium gray");
    println!();
    println!("  gtao_edges.png:");
    println!("    [ ] Sharp depth edges are visible");
    println!("    [ ] Smooth surfaces show consistent values");
    println!();
}
