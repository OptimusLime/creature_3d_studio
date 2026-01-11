//! Phase 35: Procedural Moon Texture Generator
//!
//! Generates stylized moon textures with circular craters using procedural noise.
//! Creates a fantasy-style moon with smooth circular features.
//!
//! Run: cargo run --example p35_mj_moon_gen
//!
//! Output:
//!   - assets/textures/generated/mj_moon_purple.png (cratered purple moon)
//!   - assets/textures/generated/mj_moon_orange.png (cratered orange moon)

use image::{ImageBuffer, Rgba, RgbaImage};
use std::path::Path;

const OUTPUT_DIR: &str = "assets/textures/generated";

// Moon texture size (square, will be rendered as disc in shader)
const MOON_SIZE: u32 = 256;

fn main() {
    println!("==============================================");
    println!("  Procedural Moon Texture Generator");
    println!("==============================================");
    println!();

    // Ensure output directory exists
    std::fs::create_dir_all(OUTPUT_DIR).expect("Failed to create output directory");

    // Generate purple moon
    println!("Generating purple moon...");
    generate_moon("mj_moon_purple.png", 42, MoonPalette::Purple);

    // Generate orange moon (different seed for different crater pattern)
    println!();
    println!("Generating orange moon...");
    generate_moon("mj_moon_orange.png", 789, MoonPalette::Orange);

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

/// Simple pseudo-random number generator (xorshift)
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f32(&mut self) -> f32 {
        (self.next() % 10000) as f32 / 10000.0
    }

    fn next_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

/// Crater definition
struct Crater {
    x: f32,
    y: f32,
    radius: f32,
    depth: f32,     // 0.0-1.0, how dark the crater is
    rim_width: f32, // Width of the raised rim
}

fn generate_moon(output_name: &str, seed: u64, palette: MoonPalette) {
    let mut rng = Rng::new(seed);

    // Generate craters - various sizes
    let mut craters = Vec::new();

    // Large craters (3-8)
    let num_large = 3 + (rng.next() % 6) as usize;
    for _ in 0..num_large {
        let x = rng.next_range(0.15, 0.85);
        let y = rng.next_range(0.15, 0.85);
        let radius = rng.next_range(0.08, 0.18);
        craters.push(Crater {
            x,
            y,
            radius,
            depth: rng.next_range(0.4, 0.7),
            rim_width: rng.next_range(0.15, 0.25),
        });
    }

    // Medium craters (8-15)
    let num_medium = 8 + (rng.next() % 8) as usize;
    for _ in 0..num_medium {
        let x = rng.next_range(0.1, 0.9);
        let y = rng.next_range(0.1, 0.9);
        let radius = rng.next_range(0.03, 0.07);
        craters.push(Crater {
            x,
            y,
            radius,
            depth: rng.next_range(0.3, 0.5),
            rim_width: rng.next_range(0.2, 0.35),
        });
    }

    // Small craters (15-30)
    let num_small = 15 + (rng.next() % 16) as usize;
    for _ in 0..num_small {
        let x = rng.next_range(0.05, 0.95);
        let y = rng.next_range(0.05, 0.95);
        let radius = rng.next_range(0.01, 0.025);
        craters.push(Crater {
            x,
            y,
            radius,
            depth: rng.next_range(0.2, 0.4),
            rim_width: rng.next_range(0.3, 0.5),
        });
    }

    println!("  Generated {} craters", craters.len());

    // Render moon texture
    let img = render_moon(&craters, palette, &mut rng);

    // Apply circular mask
    let masked = apply_circular_mask(&img);

    // Apply glow
    let with_glow = apply_moon_glow(&masked, palette);

    // Save
    let output_path = Path::new(OUTPUT_DIR).join(output_name);
    with_glow.save(&output_path).expect("Failed to save PNG");
    println!("  Saved: {}", output_path.display());
}

fn render_moon(craters: &[Crater], palette: MoonPalette, rng: &mut Rng) -> RgbaImage {
    let mut img: RgbaImage = ImageBuffer::new(MOON_SIZE, MOON_SIZE);

    // Base colors
    let (base_color, dark_color, highlight_color) = match palette {
        MoonPalette::Purple => (
            [170, 150, 190], // Base moon surface
            [70, 50, 90],    // Dark crater interior
            [200, 180, 220], // Highlight (rim)
        ),
        MoonPalette::Orange => (
            [210, 180, 150], // Base moon surface
            [100, 70, 45],   // Dark crater interior
            [240, 210, 180], // Highlight (rim)
        ),
    };

    let size = MOON_SIZE as f32;

    for y in 0..MOON_SIZE {
        for x in 0..MOON_SIZE {
            // Normalized coordinates (0-1)
            let nx = x as f32 / size;
            let ny = y as f32 / size;

            // Start with base color + slight noise variation
            let noise = simple_noise(nx * 20.0, ny * 20.0, rng) * 0.1;
            let mut color = [
                (base_color[0] as f32 * (1.0 + noise - 0.05)) as u8,
                (base_color[1] as f32 * (1.0 + noise - 0.05)) as u8,
                (base_color[2] as f32 * (1.0 + noise - 0.05)) as u8,
            ];

            // Apply crater effects
            for crater in craters {
                let dx = nx - crater.x;
                let dy = ny - crater.y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < crater.radius * (1.0 + crater.rim_width) {
                    if dist < crater.radius {
                        // Inside crater - dark gradient toward center
                        let t = dist / crater.radius;
                        let depth_factor = 1.0 - (1.0 - t) * crater.depth;

                        // Crater floor is darker
                        color = [
                            lerp_u8(dark_color[0], color[0], depth_factor),
                            lerp_u8(dark_color[1], color[1], depth_factor),
                            lerp_u8(dark_color[2], color[2], depth_factor),
                        ];
                    } else {
                        // On the rim - slightly brighter
                        let rim_t = (dist - crater.radius) / (crater.radius * crater.rim_width);
                        let rim_factor = 1.0 - rim_t;
                        let rim_factor = rim_factor * rim_factor * 0.3; // Soft rim

                        color = [
                            lerp_u8(color[0], highlight_color[0], rim_factor),
                            lerp_u8(color[1], highlight_color[1], rim_factor),
                            lerp_u8(color[2], highlight_color[2], rim_factor),
                        ];
                    }
                }
            }

            img.put_pixel(x, y, Rgba([color[0], color[1], color[2], 255]));
        }
    }

    img
}

/// Simple value noise for texture variation
fn simple_noise(x: f32, y: f32, _rng: &mut Rng) -> f32 {
    // Simple hash-based noise
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();

    let hash = |x: i32, y: i32| -> f32 {
        // Use wrapping arithmetic to avoid overflow
        let h = (x as u32)
            .wrapping_mul(374761393)
            .wrapping_add((y as u32).wrapping_mul(668265263))
            .wrapping_mul(1274126177);
        (h as f32) / (u32::MAX as f32)
    };

    // Bilinear interpolation
    let v00 = hash(ix, iy);
    let v10 = hash(ix + 1, iy);
    let v01 = hash(ix, iy + 1);
    let v11 = hash(ix + 1, iy + 1);

    let v0 = v00 + fx * (v10 - v00);
    let v1 = v01 + fx * (v11 - v01);

    v0 + fy * (v1 - v0)
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    ((a as f32) * (1.0 - t) + (b as f32) * t) as u8
}

/// Apply circular mask to create moon disc shape
fn apply_circular_mask(img: &RgbaImage) -> RgbaImage {
    let width = img.width();
    let height = img.height();
    let mut result = img.clone();

    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let radius = cx.min(cy) * 0.92; // Leave room for glow

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist > radius {
                // Outside circle - fully transparent
                result.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else if dist > radius - 4.0 {
                // Edge - soft falloff
                let t = (radius - dist) / 4.0;
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
    let radius = cx.min(cy) * 0.92;

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
            let glow_radius = radius * 1.18;
            if dist > radius && dist < glow_radius {
                let t = 1.0 - (dist - radius) / (glow_radius - radius);
                let glow_alpha = (t * t * 100.0) as u8; // Quadratic falloff
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
