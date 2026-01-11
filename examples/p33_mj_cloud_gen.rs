//! Phase 33: MarkovJunior Cloud Texture Generator
//!
//! Generates cloud texture using MarkovJunior and saves as PNG.
//! Applies Gaussian blur to soften the binary MJ output into smooth cloud edges.
//!
//! Run: cargo run --example p33_mj_cloud_gen
//!
//! Output: assets/textures/generated/mj_clouds_001.png

use image::{ImageBuffer, Rgba, RgbaImage};
use std::path::Path;
use studio_core::markov_junior::{
    render::{render_2d, save_png},
    Model,
};

const OUTPUT_DIR: &str = "assets/textures/generated";
const OUTPUT_FILE: &str = "mj_clouds_001.png";

// 2K x 1K for sky dome (2:1 aspect ratio)
const TEXTURE_WIDTH: usize = 2048;
const TEXTURE_HEIGHT: usize = 1024;

// MJ runs at higher resolution for finer detail, less blocky
// 256x128 MJ grid -> 8x scale = 2048x1024 output
const MJ_WIDTH: usize = 256;
const MJ_HEIGHT: usize = 128;
const PIXEL_SCALE: u32 = 8;

// Larger blur radius for softer, more natural cloud edges
const BLUR_RADIUS: u32 = 16;

fn main() {
    println!("==============================================");
    println!("  Phase 33: MJ Cloud Texture Generator");
    println!("==============================================");
    println!();

    // Ensure output directory exists
    std::fs::create_dir_all(OUTPUT_DIR).expect("Failed to create output directory");

    // Load the cloud model
    let xml_path = Path::new("MarkovJunior/models/CloudTexture.xml");
    println!("Loading: {}", xml_path.display());

    let mut model = Model::load_with_size(xml_path, MJ_WIDTH, MJ_HEIGHT, 1)
        .expect("Failed to load CloudTexture.xml");

    // Run to completion with seed 42
    println!("MJ grid size: {}x{}", MJ_WIDTH, MJ_HEIGHT);
    println!(
        "Output size: {}x{} ({}x scale)",
        TEXTURE_WIDTH, TEXTURE_HEIGHT, PIXEL_SCALE
    );
    println!("Running MarkovJunior...");
    model.reset(42);

    let max_steps = 100_000;
    let mut steps = 0;
    while model.step() && steps < max_steps {
        steps += 1;
    }
    println!("Completed in {} steps", steps);

    // Get the result grid
    let grid = model.grid();
    println!("Grid size: {}x{}x{}", grid.mx, grid.my, grid.mz);
    println!("Non-zero cells: {}", grid.count_nonzero());

    // Define colors: B=transparent, W=white cloud
    // Using full alpha here - we'll control transparency via blur
    let colors: Vec<[u8; 4]> = vec![
        [0, 0, 0, 0],         // B: fully transparent
        [255, 255, 255, 255], // W: fully opaque white
    ];

    // Render to image with upscaling (transparent background for clouds)
    println!("Rendering and upscaling...");
    let img = render_2d(grid, &colors, PIXEL_SCALE, Some([0, 0, 0, 0]));

    // Debug: check pre-blur alpha range
    let mut min_alpha = 255u8;
    let mut max_alpha = 0u8;
    let mut nonzero_count = 0usize;
    for pixel in img.pixels() {
        let a = pixel[3];
        if a > 0 {
            nonzero_count += 1;
        }
        if a < min_alpha {
            min_alpha = a;
        }
        if a > max_alpha {
            max_alpha = a;
        }
    }
    println!(
        "  Pre-blur: alpha range [{}, {}], non-zero pixels: {}",
        min_alpha, max_alpha, nonzero_count
    );

    // Apply Gaussian blur to soften edges
    println!("Applying blur (radius={}) to soften edges...", BLUR_RADIUS);
    let blurred = gaussian_blur_rgba(&img, BLUR_RADIUS);

    // Debug: check blur output alpha range
    let mut min_alpha = 255u8;
    let mut max_alpha = 0u8;
    let mut nonzero_count = 0usize;
    for pixel in blurred.pixels() {
        let a = pixel[3];
        if a > 0 {
            nonzero_count += 1;
        }
        if a < min_alpha {
            min_alpha = a;
        }
        if a > max_alpha {
            max_alpha = a;
        }
    }
    println!(
        "  Blur output: alpha range [{}, {}], non-zero pixels: {}",
        min_alpha, max_alpha, nonzero_count
    );

    // Threshold to remove blur haze, then scale remaining alpha
    // threshold=64 (25%) keeps more soft edges for wispy look
    // factor=0.7 makes clouds semi-transparent
    println!("Applying threshold and alpha adjustment...");
    let final_img = threshold_and_adjust_alpha(&blurred, 64, 0.7);

    // Debug: check final output alpha range
    let mut min_alpha = 255u8;
    let mut max_alpha = 0u8;
    let mut nonzero_count = 0usize;
    for pixel in final_img.pixels() {
        let a = pixel[3];
        if a > 0 {
            nonzero_count += 1;
        }
        if a < min_alpha {
            min_alpha = a;
        }
        if a > max_alpha {
            max_alpha = a;
        }
    }
    println!(
        "  Final output: alpha range [{}, {}], non-zero pixels: {}",
        min_alpha, max_alpha, nonzero_count
    );

    // Save PNG
    let output_path = Path::new(OUTPUT_DIR).join(OUTPUT_FILE);
    save_png(&final_img, &output_path).expect("Failed to save PNG");

    println!();
    println!("==============================================");
    println!("  Generation complete!");
    println!("==============================================");
    println!("Output: {}", output_path.display());
    println!("Size: {}x{}", final_img.width(), final_img.height());
}

/// Apply Gaussian blur to an RGBA image
fn gaussian_blur_rgba(img: &RgbaImage, radius: u32) -> RgbaImage {
    let width = img.width();
    let height = img.height();

    // Generate 1D Gaussian kernel
    let kernel = gaussian_kernel(radius);
    let kernel_size = kernel.len();
    let half = (kernel_size / 2) as i32;

    // Horizontal pass
    let mut temp: RgbaImage = ImageBuffer::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let mut r_sum = 0.0f32;
            let mut g_sum = 0.0f32;
            let mut b_sum = 0.0f32;
            let mut a_sum = 0.0f32;
            let mut weight_sum = 0.0f32;

            for (i, &weight) in kernel.iter().enumerate() {
                let sx = (x as i32 + i as i32 - half).clamp(0, width as i32 - 1) as u32;
                let pixel = img.get_pixel(sx, y);
                r_sum += pixel[0] as f32 * weight;
                g_sum += pixel[1] as f32 * weight;
                b_sum += pixel[2] as f32 * weight;
                a_sum += pixel[3] as f32 * weight;
                weight_sum += weight;
            }

            temp.put_pixel(
                x,
                y,
                Rgba([
                    (r_sum / weight_sum) as u8,
                    (g_sum / weight_sum) as u8,
                    (b_sum / weight_sum) as u8,
                    (a_sum / weight_sum) as u8,
                ]),
            );
        }
    }

    // Vertical pass
    let mut result: RgbaImage = ImageBuffer::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let mut r_sum = 0.0f32;
            let mut g_sum = 0.0f32;
            let mut b_sum = 0.0f32;
            let mut a_sum = 0.0f32;
            let mut weight_sum = 0.0f32;

            for (i, &weight) in kernel.iter().enumerate() {
                let sy = (y as i32 + i as i32 - half).clamp(0, height as i32 - 1) as u32;
                let pixel = temp.get_pixel(x, sy);
                r_sum += pixel[0] as f32 * weight;
                g_sum += pixel[1] as f32 * weight;
                b_sum += pixel[2] as f32 * weight;
                a_sum += pixel[3] as f32 * weight;
                weight_sum += weight;
            }

            result.put_pixel(
                x,
                y,
                Rgba([
                    (r_sum / weight_sum) as u8,
                    (g_sum / weight_sum) as u8,
                    (b_sum / weight_sum) as u8,
                    (a_sum / weight_sum) as u8,
                ]),
            );
        }
    }

    result
}

/// Generate 1D Gaussian kernel
fn gaussian_kernel(radius: u32) -> Vec<f32> {
    let size = (radius * 2 + 1) as usize;
    let sigma = radius as f32 / 2.0;
    let mut kernel = vec![0.0f32; size];
    let center = radius as f32;

    for i in 0..size {
        let x = i as f32 - center;
        kernel[i] = (-x * x / (2.0 * sigma * sigma)).exp();
    }

    // Normalize
    let sum: f32 = kernel.iter().sum();
    for k in &mut kernel {
        *k /= sum;
    }

    kernel
}

/// Apply threshold and rescale alpha to remove blur haze while keeping soft edges
/// threshold: alpha values below this are zeroed (0-255 scale)
/// rescale: remaining alpha is rescaled to 0-255 range, then multiplied by factor
fn threshold_and_adjust_alpha(img: &RgbaImage, threshold: u8, factor: f32) -> RgbaImage {
    let mut result = img.clone();
    let threshold_f = threshold as f32;
    let max_alpha = 255.0 - threshold_f;

    for pixel in result.pixels_mut() {
        let alpha = pixel[3];
        if alpha < threshold {
            // Below threshold: fully transparent
            pixel[3] = 0;
        } else {
            // Above threshold: rescale to 0-255 range then apply factor
            let rescaled = ((alpha as f32 - threshold_f) / max_alpha) * 255.0;
            pixel[3] = (rescaled * factor).min(255.0) as u8;
        }
    }
    result
}
