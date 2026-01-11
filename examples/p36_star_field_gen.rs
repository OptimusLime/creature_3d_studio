//! Phase 36: Procedural Star Field Generator
//!
//! Generates a stylized star field texture with:
//! - Large bright stars (rare)
//! - Medium stars (moderate density)
//! - Small dim stars (many)
//! - Optional nebula wisps
//!
//! The texture is designed to tile seamlessly on a sky dome.
//!
//! Run: cargo run --example p36_star_field_gen
//!
//! Output:
//!   - assets/textures/generated/mj_stars.png

use image::{ImageBuffer, Rgba, RgbaImage};
use std::path::Path;

const OUTPUT_DIR: &str = "assets/textures/generated";

// Star field texture size - 2:1 aspect ratio for spherical UV mapping
// (horizontal wraps 360°, vertical spans 90° hemisphere)
const STAR_WIDTH: u32 = 2048;
const STAR_HEIGHT: u32 = 1024;

fn main() {
    println!("==============================================");
    println!("  Procedural Star Field Generator");
    println!("==============================================");
    println!();

    // Ensure output directory exists
    std::fs::create_dir_all(OUTPUT_DIR).expect("Failed to create output directory");

    println!("Generating star field...");
    generate_star_field("mj_stars.png", 42);

    println!();
    println!("==============================================");
    println!("  Generation complete!");
    println!("==============================================");
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

/// Star definition
struct Star {
    x: f32,
    y: f32,
    radius: f32,
    brightness: f32,
    color: [f32; 3], // RGB 0-1
}

fn generate_star_field(output_name: &str, seed: u64) {
    let mut rng = Rng::new(seed);

    let mut stars = Vec::new();

    // Large bright stars (rare, 15-25) - sharper, brighter
    let num_large = 15 + (rng.next() % 11) as usize;
    println!("  Large stars: {}", num_large);
    for _ in 0..num_large {
        let color = random_star_color(&mut rng, StarType::Large);
        stars.push(Star {
            x: rng.next_f32(),
            y: rng.next_f32(),
            radius: rng.next_range(1.5, 2.5),
            brightness: rng.next_range(1.0, 1.2), // Brighter
            color,
        });
    }

    // Medium stars (moderate, 80-150) - more frequent
    let num_medium = 80 + (rng.next() % 71) as usize;
    println!("  Medium stars: {}", num_medium);
    for _ in 0..num_medium {
        let color = random_star_color(&mut rng, StarType::Medium);
        stars.push(Star {
            x: rng.next_f32(),
            y: rng.next_f32(),
            radius: rng.next_range(0.8, 1.4),
            brightness: rng.next_range(0.7, 0.95),
            color,
        });
    }

    // Small dim stars (many, 400-600) - denser
    let num_small = 400 + (rng.next() % 201) as usize;
    println!("  Small stars: {}", num_small);
    for _ in 0..num_small {
        let color = random_star_color(&mut rng, StarType::Small);
        stars.push(Star {
            x: rng.next_f32(),
            y: rng.next_f32(),
            radius: rng.next_range(0.4, 0.7),
            brightness: rng.next_range(0.4, 0.7),
            color,
        });
    }

    // Tiny background stars (very many, 1200-1800) - much denser
    let num_tiny = 1200 + (rng.next() % 601) as usize;
    println!("  Tiny stars: {}", num_tiny);
    for _ in 0..num_tiny {
        let color = random_star_color(&mut rng, StarType::Tiny);
        stars.push(Star {
            x: rng.next_f32(),
            y: rng.next_f32(),
            radius: rng.next_range(0.2, 0.4),
            brightness: rng.next_range(0.25, 0.5),
            color,
        });
    }

    println!("  Total stars: {}", stars.len());

    // Render star field
    let img = render_star_field(&stars, &mut rng);

    // Save
    let output_path = Path::new(OUTPUT_DIR).join(output_name);
    img.save(&output_path).expect("Failed to save PNG");
    println!("  Saved: {}", output_path.display());
}

#[derive(Clone, Copy)]
enum StarType {
    Large,
    Medium,
    Small,
    Tiny,
}

/// Generate random star color based on type
/// Stars can be white, blue-white, yellow, orange, or red
fn random_star_color(rng: &mut Rng, star_type: StarType) -> [f32; 3] {
    let roll = rng.next_f32();

    match star_type {
        StarType::Large => {
            // Large stars: mostly white/blue-white, some yellow
            if roll < 0.4 {
                // Blue-white (hot stars)
                [
                    0.8 + rng.next_f32() * 0.2,
                    0.85 + rng.next_f32() * 0.15,
                    1.0,
                ]
            } else if roll < 0.7 {
                // Pure white
                [1.0, 1.0, 1.0]
            } else if roll < 0.9 {
                // Yellow-white
                [
                    1.0,
                    0.95 + rng.next_f32() * 0.05,
                    0.8 + rng.next_f32() * 0.1,
                ]
            } else {
                // Orange giant
                [1.0, 0.7 + rng.next_f32() * 0.2, 0.4 + rng.next_f32() * 0.2]
            }
        }
        StarType::Medium => {
            // Medium stars: varied colors
            if roll < 0.3 {
                // Blue-white
                [
                    0.75 + rng.next_f32() * 0.25,
                    0.8 + rng.next_f32() * 0.2,
                    1.0,
                ]
            } else if roll < 0.6 {
                // White
                let w = 0.9 + rng.next_f32() * 0.1;
                [w, w, w]
            } else if roll < 0.85 {
                // Yellow
                [1.0, 0.9 + rng.next_f32() * 0.1, 0.7 + rng.next_f32() * 0.15]
            } else {
                // Orange-red
                [1.0, 0.5 + rng.next_f32() * 0.3, 0.3 + rng.next_f32() * 0.2]
            }
        }
        StarType::Small | StarType::Tiny => {
            // Small/tiny stars: mostly white with slight tints
            let base = 0.8 + rng.next_f32() * 0.2;
            let tint = rng.next_f32();
            if tint < 0.3 {
                // Slight blue
                [base * 0.9, base * 0.95, base]
            } else if tint < 0.6 {
                // Pure white
                [base, base, base]
            } else {
                // Slight yellow
                [base, base * 0.95, base * 0.85]
            }
        }
    }
}

fn render_star_field(stars: &[Star], rng: &mut Rng) -> RgbaImage {
    let mut img: RgbaImage = ImageBuffer::new(STAR_WIDTH, STAR_HEIGHT);

    // Fill with transparent black (stars will be additive)
    for pixel in img.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 0]);
    }

    let width = STAR_WIDTH as f32;
    let height = STAR_HEIGHT as f32;

    // Add subtle nebula background in a few spots
    add_nebula_wisps(&mut img, rng);

    // Render each star
    for star in stars {
        let cx = (star.x * width) as i32;
        let cy = (star.y * height) as i32;
        let r = star.radius;

        // Draw star with sharper falloff (reduced extent for crisper stars)
        let extent = (r * 2.0).ceil() as i32;

        for dy in -extent..=extent {
            for dx in -extent..=extent {
                let px = cx + dx;
                let py = cy + dy;

                // Wrap for tiling (horizontal only, vertical clamps)
                let px_wrapped = px.rem_euclid(STAR_WIDTH as i32) as u32;
                let py_wrapped = py.clamp(0, STAR_HEIGHT as i32 - 1) as u32;

                let dist = ((dx * dx + dy * dy) as f32).sqrt();

                // Sharper falloff - use pow(4) for crisper stars
                let normalized_dist = dist / r;
                let falloff = (1.0 - normalized_dist.min(1.0)).powi(4);
                let intensity = star.brightness * falloff;

                if intensity > 0.01 {
                    let current = img.get_pixel(px_wrapped, py_wrapped);

                    // Additive blending
                    let new_r = (current[0] as f32 + star.color[0] * intensity * 255.0).min(255.0);
                    let new_g = (current[1] as f32 + star.color[1] * intensity * 255.0).min(255.0);
                    let new_b = (current[2] as f32 + star.color[2] * intensity * 255.0).min(255.0);
                    let new_a = (current[3] as f32 + intensity * 255.0).min(255.0);

                    img.put_pixel(
                        px_wrapped,
                        py_wrapped,
                        Rgba([new_r as u8, new_g as u8, new_b as u8, new_a as u8]),
                    );
                }
            }
        }
    }

    img
}

/// Add subtle nebula wisps in the background
fn add_nebula_wisps(img: &mut RgbaImage, rng: &mut Rng) {
    let width = STAR_WIDTH as f32;
    let height = STAR_HEIGHT as f32;

    // Create 3-6 nebula regions (more for larger texture)
    let num_nebulae = 3 + (rng.next() % 4) as usize;

    for _ in 0..num_nebulae {
        let cx = rng.next_f32() * width;
        let cy = rng.next_f32() * height;
        let nebula_radius = rng.next_range(100.0, 200.0);

        // Nebula color - subtle purple/blue/pink tints
        let color_roll = rng.next_f32();
        let nebula_color: [f32; 3] = if color_roll < 0.4 {
            // Purple
            [0.3, 0.1, 0.4]
        } else if color_roll < 0.7 {
            // Blue
            [0.1, 0.2, 0.35]
        } else {
            // Pink/magenta
            [0.35, 0.1, 0.25]
        };

        for y in 0..STAR_HEIGHT {
            for x in 0..STAR_WIDTH {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;

                // Handle wrapping for tiling (horizontal only)
                let dx = if dx.abs() > width / 2.0 {
                    dx - dx.signum() * width
                } else {
                    dx
                };

                let dist = (dx * dx + dy * dy).sqrt();

                if dist < nebula_radius {
                    // Noise-based intensity variation
                    let noise = simple_noise(x as f32 * 0.02, y as f32 * 0.02);
                    let falloff = 1.0 - (dist / nebula_radius);
                    let intensity = falloff * falloff * noise * 0.08; // Very subtle

                    if intensity > 0.001 {
                        let current = img.get_pixel(x, y);

                        let new_r =
                            (current[0] as f32 + nebula_color[0] * intensity * 255.0).min(255.0);
                        let new_g =
                            (current[1] as f32 + nebula_color[1] * intensity * 255.0).min(255.0);
                        let new_b =
                            (current[2] as f32 + nebula_color[2] * intensity * 255.0).min(255.0);
                        let new_a = (current[3] as f32 + intensity * 128.0).min(255.0);

                        img.put_pixel(
                            x,
                            y,
                            Rgba([new_r as u8, new_g as u8, new_b as u8, new_a as u8]),
                        );
                    }
                }
            }
        }
    }
}

/// Simple value noise
fn simple_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();

    let hash = |x: i32, y: i32| -> f32 {
        let h = (x as u32)
            .wrapping_mul(374761393)
            .wrapping_add((y as u32).wrapping_mul(668265263))
            .wrapping_mul(1274126177);
        (h as f32) / (u32::MAX as f32)
    };

    // Smoothstep interpolation
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);

    let v00 = hash(ix, iy);
    let v10 = hash(ix + 1, iy);
    let v01 = hash(ix, iy + 1);
    let v11 = hash(ix + 1, iy + 1);

    let v0 = v00 + sx * (v10 - v00);
    let v1 = v01 + sx * (v11 - v01);

    v0 + sy * (v1 - v0)
}
