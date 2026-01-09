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
    // ORDERED BY ALGORITHM PHASE - see docs/GTAO_VERIFICATION.md
    //
    // IMPORTANT: GTAO debug modes write to the GTAO texture, which is then sampled
    // by the lighting shader. To SEE the GTAO debug output, we must ALSO set
    // lighting_debug_mode = 5 (AO only) so the lighting shader passes through
    // the GTAO texture values directly.
    let debug_config = DebugScreenshotConfig::new("screenshots/gtao_test")
        .with_base_wait_frames(15) // Wait for scene to stabilize
        // ============================================================
        // PHASE 1: G-Buffer Inputs (pre-requisite)
        // ============================================================
        .with_capture("p1_gbuffer_depth", DebugCapture::lighting_debug(2))
        .with_capture("p1_gbuffer_normals", DebugCapture::lighting_debug(1))
        // ============================================================
        // PHASE 2: Depth Linearization & MIP Chain
        // ============================================================
        .with_capture(
            "p2_depth_mip0",
            DebugCapture {
                name: "p2_depth_mip0".to_string(),
                gtao_debug_mode: 11, // Viewspace linear depth MIP 0
                lighting_debug_mode: 5,
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        .with_capture(
            "p2_depth_mip1",
            DebugCapture {
                name: "p2_depth_mip1".to_string(),
                gtao_debug_mode: 12, // Depth MIP level 1
                lighting_debug_mode: 5,
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        .with_capture(
            "p2_depth_mip2",
            DebugCapture {
                name: "p2_depth_mip2".to_string(),
                gtao_debug_mode: 13, // Depth MIP level 2
                lighting_debug_mode: 5,
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        .with_capture(
            "p2_depth_log",
            DebugCapture {
                name: "p2_depth_log".to_string(),
                gtao_debug_mode: 16, // Log-scale depth (full range visible)
                lighting_debug_mode: 5,
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        // ============================================================
        // PHASE 3: Edge Detection
        // ============================================================
        .with_capture(
            "p3_edges_packed",
            DebugCapture {
                name: "p3_edges_packed".to_string(),
                gtao_debug_mode: 40, // Packed edges (raw output)
                lighting_debug_mode: 5,
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        .with_capture(
            "p3_edges_inverted",
            DebugCapture {
                name: "p3_edges_inverted".to_string(),
                gtao_debug_mode: 44, // Inverted edges (edges = bright)
                lighting_debug_mode: 5,
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        // ============================================================
        // PHASE 4: View-Space Normals
        // ============================================================
        .with_capture(
            "p4_normal_z",
            DebugCapture {
                name: "p4_normal_z".to_string(),
                gtao_debug_mode: 20, // View-space normal.z
                lighting_debug_mode: 5,
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        .with_capture(
            "p4_normal_xy",
            DebugCapture {
                name: "p4_normal_xy".to_string(),
                gtao_debug_mode: 21, // View-space normal.xy
                lighting_debug_mode: 5,
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        // ============================================================
        // PHASE 5: Screen-Space Radius
        // ============================================================
        .with_capture(
            "p5_radius",
            DebugCapture {
                name: "p5_radius".to_string(),
                gtao_debug_mode: 30, // Screenspace radius
                lighting_debug_mode: 5,
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        // ============================================================
        // PHASE 6: Raw GTAO Output (before denoise)
        // ============================================================
        .with_capture(
            "p6_gtao_raw",
            DebugCapture {
                name: "p6_gtao_raw".to_string(),
                gtao_debug_mode: 50, // Raw GTAO before denoise
                lighting_debug_mode: 5,
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        // ============================================================
        // PHASE 7: Denoised GTAO Output
        // ============================================================
        .with_capture(
            "p7_ao_denoised",
            DebugCapture {
                name: "p7_ao_denoised".to_string(),
                gtao_debug_mode: 0,     // Normal GTAO (denoised)
                lighting_debug_mode: 5, // View as grayscale
                denoise_debug_mode: 0,
                wait_frames: 5,
            },
        )
        .with_capture("p7_denoise_diff", DebugCapture::denoise_debug(4))
        // ============================================================
        // PHASE 8: Final Composited Render
        // ============================================================
        .with_capture("p8_render", DebugCapture::default());

    println!(
        "Capturing {} debug screenshots to screenshots/gtao_test/",
        debug_config.captures.len()
    );
    println!();

    // Camera positioned to see all test geometries
    // NOTE: Bloom is disabled for clean debug output
    let app = VoxelWorldApp::new("GTAO Verification Test")
        .with_world_file(world_path)
        .with_resolution(1024, 768)
        .with_deferred(true)
        .without_deferred_bloom() // Disable bloom for clean debug output
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
