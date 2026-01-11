//! Phase 35: MarkovJunior Moon Texture Generator
//!
//! Generates stylized moon textures using MarkovJunior.
//! Creates cratered moon appearance with circles-in-circles approach.
//!
//! Run: cargo run --example p35_mj_moon_gen
//!
//! Output:
//!   - assets/textures/generated/mj_moon_purple.png (cratered purple moon)
//!   - assets/textures/generated/mj_moon_orange.png (cratered orange moon)

use image::{ImageBuffer, Rgba, RgbaImage};
use std::path::Path;
use studio_core::markov_junior::Model;

const OUTPUT_DIR: &str = "assets/textures/generated";

// Moon texture size (square, will be rendered as disc in shader)
const MOON_SIZE: usize = 256;

fn main() {
    println!("==============================================");
    println!("  MarkovJunior Moon Texture Generator");
    println!("==============================================");
    println!();

    // Ensure output directory exists
    std::fs::create_dir_all(OUTPUT_DIR).expect("Failed to create output directory");

    // Generate purple moon (cratered style)
    println!("Generating purple moon (cratered)...");
    generate_moon(
        "MarkovJunior/models/MoonCratered.xml",
        "mj_moon_purple.png",
        42,
        MoonPalette::Purple,
    );

    // Generate orange moon (cratered style, different seed)
    println!();
    println!("Generating orange moon (cratered)...");
    generate_moon(
        "MarkovJunior/models/MoonCratered.xml",
        "mj_moon_orange.png",
        789, // Different seed for different crater pattern
        MoonPalette::Orange,
    );

    println!();
    println!("==============================================");
    println!("  Generation complete!");
    println!("==============================================");
}

#[derive(Clone, Copy)]
enum MoonPalette {
    Purple,
    Orange,
}

fn generate_moon(model_path: &str, output_name: &str, seed: u64, palette: MoonPalette) {
    let xml_path = Path::new(model_path);
    println!("  Loading: {}", xml_path.display());

    let mut model = Model::load_with_size(xml_path, MOON_SIZE, MOON_SIZE, 1)
        .expect(&format!("Failed to load {}", model_path));

    println!("  Running MarkovJunior (seed={})...", seed);
    model.reset(seed);

    let max_steps = 100_000;
    let mut steps = 0;
    while model.step() && steps < max_steps {
        steps += 1;
    }
    println!("  Completed in {} steps", steps);

    let grid = model.grid();
    println!("  Grid size: {}x{}", grid.mx, grid.my);
    println!("  Non-zero cells: {}", grid.count_nonzero());

    // Get color palette based on moon type
    let colors = get_moon_colors(palette);

    // Render to image
    let img = render_moon_texture(grid, &colors);

    // Apply circular mask to make it a disc
    let masked = apply_circular_mask(&img);

    // Apply glow effect around edges
    let with_glow = apply_moon_glow(&masked, palette);

    // Save
    let output_path = Path::new(OUTPUT_DIR).join(output_name);
    with_glow.save(&output_path).expect("Failed to save PNG");
    println!("  Saved: {}", output_path.display());
}

/// Get color palette for moon rendering
/// Values in MJ: B=0 (background), W=1 (moon surface), C=2 (crater)
fn get_moon_colors(palette: MoonPalette) -> Vec<[u8; 4]> {
    match palette {
        MoonPalette::Purple => vec![
            [0, 0, 0, 0],         // B: transparent background
            [170, 150, 190, 255], // W: moon surface (muted purple-gray)
            [90, 60, 110, 255],   // C: crater (much darker for visibility)
        ],
        MoonPalette::Orange => vec![
            [0, 0, 0, 0],         // B: transparent background
            [210, 180, 150, 255], // W: moon surface (muted orange-tan)
            [120, 80, 50, 255],   // C: crater (much darker for visibility)
        ],
    }
}

/// Render MJ grid to RGBA image
fn render_moon_texture(grid: &studio_core::markov_junior::MjGrid, colors: &[[u8; 4]]) -> RgbaImage {
    let width = grid.mx as u32;
    let height = grid.my as u32;
    let mut img: RgbaImage = ImageBuffer::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let value = grid.get(x as usize, y as usize, 0).unwrap_or(0) as usize;
            let color = colors.get(value).unwrap_or(&[0, 0, 0, 0]);
            img.put_pixel(x, y, Rgba(*color));
        }
    }

    img
}

/// Apply circular mask to create moon disc shape
fn apply_circular_mask(img: &RgbaImage) -> RgbaImage {
    let width = img.width();
    let height = img.height();
    let mut result = img.clone();

    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let radius = cx.min(cy) * 0.95; // Slightly smaller to leave edge room

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist > radius {
                // Outside circle - fully transparent
                result.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else if dist > radius - 3.0 {
                // Edge - soft falloff
                let t = (radius - dist) / 3.0;
                let pixel = img.get_pixel(x, y);
                let new_alpha = (pixel[3] as f32 * t) as u8;
                result.put_pixel(x, y, Rgba([pixel[0], pixel[1], pixel[2], new_alpha]));
            }
        }
    }

    result
}

/// Apply subtle glow around moon edges
fn apply_moon_glow(img: &RgbaImage, palette: MoonPalette) -> RgbaImage {
    let width = img.width();
    let height = img.height();
    let mut result: RgbaImage = ImageBuffer::new(width, height);

    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let radius = cx.min(cy) * 0.95;

    // Glow color based on moon type
    let glow_color: [u8; 3] = match palette {
        MoonPalette::Purple => [180, 140, 200],
        MoonPalette::Orange => [255, 180, 100],
    };

    // First pass: add glow layer
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            // Glow extends beyond the moon disc
            let glow_radius = radius * 1.15;
            if dist > radius && dist < glow_radius {
                let t = 1.0 - (dist - radius) / (glow_radius - radius);
                let glow_alpha = (t * t * 80.0) as u8; // Quadratic falloff
                result.put_pixel(
                    x,
                    y,
                    Rgba([glow_color[0], glow_color[1], glow_color[2], glow_alpha]),
                );
            }
        }
    }

    // Second pass: composite original moon on top
    for y in 0..height {
        for x in 0..width {
            let src = img.get_pixel(x, y);
            if src[3] > 0 {
                // Moon pixel - blend over glow
                let dst = result.get_pixel(x, y);
                let blended = alpha_blend(*src, *dst);
                result.put_pixel(x, y, blended);
            }
        }
    }

    result
}

/// Alpha blend src over dst
fn alpha_blend(src: Rgba<u8>, dst: Rgba<u8>) -> Rgba<u8> {
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);

    if out_a < 0.001 {
        return Rgba([0, 0, 0, 0]);
    }

    let blend = |s: u8, d: u8| -> u8 {
        let sf = s as f32 / 255.0;
        let df = d as f32 / 255.0;
        let out = (sf * sa + df * da * (1.0 - sa)) / out_a;
        (out * 255.0) as u8
    };

    Rgba([
        blend(src[0], dst[0]),
        blend(src[1], dst[1]),
        blend(src[2], dst[2]),
        (out_a * 255.0) as u8,
    ])
}
