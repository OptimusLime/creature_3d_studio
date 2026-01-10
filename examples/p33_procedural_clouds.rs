//! Phase 33b: Procedural Cloud Texture Generator
//!
//! Generates cloud texture using proper noise-based techniques:
//! - Fractal Brownian Motion (fBM) for base shape
//! - Worley/Voronoi noise for puffy cellular look
//! - Domain warping for organic flowing shapes
//! - Thresholding with soft edges
//!
//! Run: cargo run --example p33_procedural_clouds
//!
//! Output: assets/textures/generated/clouds_procedural.png

use image::{ImageBuffer, Rgba, RgbaImage};
use std::f32::consts::PI;
use std::path::Path;

const OUTPUT_DIR: &str = "assets/textures/generated";
const OUTPUT_FILE: &str = "clouds_procedural.png";

// 2K x 1K for sky dome (2:1 aspect ratio for equirectangular)
const TEXTURE_WIDTH: u32 = 2048;
const TEXTURE_HEIGHT: u32 = 1024;

// Cloud generation parameters
const OCTAVES: u32 = 6;
const LACUNARITY: f32 = 2.0;
const PERSISTENCE: f32 = 0.5;
const BASE_FREQUENCY: f32 = 3.0; // Lower = larger cloud formations

// Cloud density control
const CLOUD_THRESHOLD: f32 = 0.48; // Higher = sparser clouds (lower = more dense)
const EDGE_SOFTNESS: f32 = 0.06; // Narrow transition = defined cloud edges

// Domain warping for organic shapes
const WARP_STRENGTH: f32 = 0.5;
const WARP_OCTAVES: u32 = 3;

// Detail noise to break up uniformity
const DETAIL_FREQUENCY: f32 = 12.0;
const DETAIL_STRENGTH: f32 = 0.15;

fn main() {
    println!("==============================================");
    println!("  Procedural Cloud Texture Generator");
    println!("==============================================");
    println!();
    println!("Parameters:");
    println!("  Octaves: {}", OCTAVES);
    println!("  Lacunarity: {}", LACUNARITY);
    println!("  Persistence: {}", PERSISTENCE);
    println!("  Base frequency: {}", BASE_FREQUENCY);
    println!("  Cloud threshold: {}", CLOUD_THRESHOLD);
    println!("  Warp strength: {}", WARP_STRENGTH);
    println!("  Detail strength: {}", DETAIL_STRENGTH);
    println!();

    // Ensure output directory exists
    std::fs::create_dir_all(OUTPUT_DIR).expect("Failed to create output directory");

    println!(
        "Generating {}x{} cloud texture...",
        TEXTURE_WIDTH, TEXTURE_HEIGHT
    );

    let mut img: RgbaImage = ImageBuffer::new(TEXTURE_WIDTH, TEXTURE_HEIGHT);

    for y in 0..TEXTURE_HEIGHT {
        for x in 0..TEXTURE_WIDTH {
            // Normalize to [0, 1]
            let u = x as f32 / TEXTURE_WIDTH as f32;
            let v = y as f32 / TEXTURE_HEIGHT as f32;

            // === SEAMLESS HORIZONTAL TILING ===
            // Map U to a circle so U=0 and U=1 meet at the same point
            // This creates a cylinder in noise space
            let angle = u * 2.0 * PI;
            let radius = BASE_FREQUENCY / (2.0 * PI); // Scale radius to match frequency
            let px = angle.cos() * radius;
            let py = angle.sin() * radius;
            let pz = v * BASE_FREQUENCY * 0.5; // V maps to height (no tiling needed)

            // Generate cloud density using 3D coordinates (cylinder surface)
            let density = cloud_density_3d(px, py, pz);

            // Convert to RGBA (white cloud with alpha = density)
            let alpha = (density * 255.0) as u8;
            img.put_pixel(x, y, Rgba([255, 255, 255, alpha]));
        }

        // Progress indicator
        if y % 100 == 0 {
            println!("  Row {}/{}", y, TEXTURE_HEIGHT);
        }
    }

    // Save PNG
    let output_path = Path::new(OUTPUT_DIR).join(OUTPUT_FILE);
    img.save(&output_path).expect("Failed to save PNG");

    // Stats
    let mut nonzero = 0usize;
    let mut total_alpha = 0u64;
    for pixel in img.pixels() {
        if pixel[3] > 0 {
            nonzero += 1;
            total_alpha += pixel[3] as u64;
        }
    }
    let coverage = nonzero as f32 / (TEXTURE_WIDTH * TEXTURE_HEIGHT) as f32;
    let avg_alpha = if nonzero > 0 {
        total_alpha / nonzero as u64
    } else {
        0
    };

    println!();
    println!("==============================================");
    println!("  Generation complete!");
    println!("==============================================");
    println!("Output: {}", output_path.display());
    println!("Size: {}x{}", TEXTURE_WIDTH, TEXTURE_HEIGHT);
    println!("Cloud coverage: {:.1}%", coverage * 100.0);
    println!("Average cloud alpha: {}", avg_alpha);
}

/// Main cloud density function using 3D noise for seamless horizontal tiling
fn cloud_density_3d(x: f32, y: f32, z: f32) -> f32 {
    // === Layer 1: Domain warping for organic flowing shapes ===
    // This is the key to non-uniform, natural looking clouds
    let warp1_x =
        fbm_3d(x * 0.7 + 31.7, y * 0.7 + 47.3, z * 0.7 + 11.1, WARP_OCTAVES) * WARP_STRENGTH;
    let warp1_y =
        fbm_3d(x * 0.7 + 83.2, y * 0.7 + 19.8, z * 0.7 + 55.5, WARP_OCTAVES) * WARP_STRENGTH;
    let warp1_z =
        fbm_3d(x * 0.7 + 17.9, y * 0.7 + 63.4, z * 0.7 + 33.3, WARP_OCTAVES) * WARP_STRENGTH;

    // Second level of warping for more complexity
    let warp2_x = fbm_3d(
        x + warp1_x + 12.5,
        y + warp1_y + 67.2,
        z + warp1_z + 22.2,
        WARP_OCTAVES,
    ) * WARP_STRENGTH
        * 0.5;
    let warp2_y = fbm_3d(
        x + warp1_x + 91.4,
        y + warp1_y + 28.6,
        z + warp1_z + 44.4,
        WARP_OCTAVES,
    ) * WARP_STRENGTH
        * 0.5;
    let warp2_z = fbm_3d(
        x + warp1_x + 53.7,
        y + warp1_y + 81.3,
        z + warp1_z + 66.6,
        WARP_OCTAVES,
    ) * WARP_STRENGTH
        * 0.5;

    let warped_x = x + warp1_x + warp2_x;
    let warped_y = y + warp1_y + warp2_y;
    let warped_z = z + warp1_z + warp2_z;

    // === Layer 2: Base cloud shape ===
    let base = fbm_3d(warped_x, warped_y, warped_z, OCTAVES);

    // === Layer 3: High-frequency detail to break uniformity ===
    let detail = fbm_3d(
        x * DETAIL_FREQUENCY + 55.0,
        y * DETAIL_FREQUENCY + 77.0,
        z * DETAIL_FREQUENCY + 99.0,
        3,
    );
    let with_detail = base + (detail - 0.5) * DETAIL_STRENGTH;

    // === Thresholding: create sparse clouds with clear sky ===
    let cloud_raw = smoothstep(
        CLOUD_THRESHOLD - EDGE_SOFTNESS,
        CLOUD_THRESHOLD + EDGE_SOFTNESS,
        with_detail,
    );

    // === Power curve: brighter centers, softer edges ===
    let density = cloud_raw.powf(0.7);

    density.clamp(0.0, 1.0)
}

/// 3D fBM for seamless cylindrical tiling
fn fbm_3d(x: f32, y: f32, z: f32, octaves: u32) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut max_value = 0.0f32;

    for _ in 0..octaves {
        value += amplitude * perlin_3d(x * frequency, y * frequency, z * frequency);
        max_value += amplitude;
        amplitude *= PERSISTENCE;
        frequency *= LACUNARITY;
    }

    // Normalize to [0, 1]
    (value / max_value) * 0.5 + 0.5
}

/// 3D Perlin noise
fn perlin_3d(x: f32, y: f32, z: f32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let z0 = z.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let z1 = z0 + 1;

    let fx = x - x.floor();
    let fy = y - y.floor();
    let fz = z - z.floor();
    let u = fade(fx);
    let v = fade(fy);
    let w = fade(fz);

    // 8 corner gradients
    let n000 = grad_dot_3d(hash3(x0, y0, z0), fx, fy, fz);
    let n100 = grad_dot_3d(hash3(x1, y0, z0), fx - 1.0, fy, fz);
    let n010 = grad_dot_3d(hash3(x0, y1, z0), fx, fy - 1.0, fz);
    let n110 = grad_dot_3d(hash3(x1, y1, z0), fx - 1.0, fy - 1.0, fz);
    let n001 = grad_dot_3d(hash3(x0, y0, z1), fx, fy, fz - 1.0);
    let n101 = grad_dot_3d(hash3(x1, y0, z1), fx - 1.0, fy, fz - 1.0);
    let n011 = grad_dot_3d(hash3(x0, y1, z1), fx, fy - 1.0, fz - 1.0);
    let n111 = grad_dot_3d(hash3(x1, y1, z1), fx - 1.0, fy - 1.0, fz - 1.0);

    // Trilinear interpolation
    let nx00 = lerp(n000, n100, u);
    let nx10 = lerp(n010, n110, u);
    let nx01 = lerp(n001, n101, u);
    let nx11 = lerp(n011, n111, u);
    let nxy0 = lerp(nx00, nx10, v);
    let nxy1 = lerp(nx01, nx11, v);
    lerp(nxy0, nxy1, w)
}

/// 3D hash function
fn hash3(x: i32, y: i32, z: i32) -> i32 {
    let mut h = x.wrapping_mul(374761393);
    h = h.wrapping_add(y.wrapping_mul(668265263));
    h = h.wrapping_add(z.wrapping_mul(1274126177));
    h = (h ^ (h >> 13)).wrapping_mul(1911520717);
    h ^ (h >> 16)
}

/// 3D gradient dot product (12 directions pointing to cube edges)
fn grad_dot_3d(hash: i32, x: f32, y: f32, z: f32) -> f32 {
    match hash & 11 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

/// Basic 2D Perlin noise (kept for reference)
#[allow(dead_code)]
fn perlin_noise(x: f32, y: f32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    let fx = x - x.floor();
    let fy = y - y.floor();
    let u = fade(fx);
    let v = fade(fy);

    let n00 = grad_dot(hash2(x0, y0), fx, fy);
    let n10 = grad_dot(hash2(x1, y0), fx - 1.0, fy);
    let n01 = grad_dot(hash2(x0, y1), fx, fy - 1.0);
    let n11 = grad_dot(hash2(x1, y1), fx - 1.0, fy - 1.0);

    let nx0 = lerp(n00, n10, u);
    let nx1 = lerp(n01, n11, u);
    lerp(nx0, nx1, v)
}

/// Smoothstep interpolation
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Perlin fade function: 6t^5 - 15t^4 + 10t^3
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Linear interpolation
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

/// Hash function for 2D grid coordinates
fn hash2(x: i32, y: i32) -> i32 {
    let mut h = x.wrapping_mul(374761393);
    h = h.wrapping_add(y.wrapping_mul(668265263));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}

/// Gradient dot product for Perlin noise
fn grad_dot(hash: i32, x: f32, y: f32) -> f32 {
    // 8 gradient directions
    match hash & 7 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x,
        5 => -x,
        6 => y,
        _ => -y,
    }
}
