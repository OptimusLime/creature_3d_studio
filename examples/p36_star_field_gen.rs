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

// Star field texture size (must tile on sphere)
const STAR_SIZE: u32 = 512;

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

    // Large bright stars (rare, 5-12)
    let num_large = 5 + (rng.next() % 8) as usize;
    println!("  Large stars: {}", num_large);
    for _ in 0..num_large {
        let color = random_star_color(&mut rng, StarType::Large);
        stars.push(Star {
            x: rng.next_f32(),
            y: rng.next_f32(),
            radius: rng.next_range(2.5, 4.0),
            brightness: rng.next_range(0.9, 1.0),
            color,
        });
    }

    // Medium stars (moderate, 30-60)
    let num_medium = 30 + (rng.next() % 31) as usize;
    println!("  Medium stars: {}", num_medium);
    for _ in 0..num_medium {
        let color = random_star_color(&mut rng, StarType::Medium);
        stars.push(Star {
            x: rng.next_f32(),
            y: rng.next_f32(),
            radius: rng.next_range(1.2, 2.0),
            brightness: rng.next_range(0.6, 0.85),
            color,
        });
    }

    // Small dim stars (many, 150-300)
    let num_small = 150 + (rng.next() % 151) as usize;
    println!("  Small stars: {}", num_small);
    for _ in 0..num_small {
        let color = random_star_color(&mut rng, StarType::Small);
        stars.push(Star {
            x: rng.next_f32(),
            y: rng.next_f32(),
            radius: rng.next_range(0.5, 1.0),
            brightness: rng.next_range(0.3, 0.6),
            color,
        });
    }

    // Tiny background stars (very many, 400-700)
    let num_tiny = 400 + (rng.next() % 301) as usize;
    println!("  Tiny stars: {}", num_tiny);
    for _ in 0..num_tiny {
        let color = random_star_color(&mut rng, StarType::Tiny);
        stars.push(Star {
            x: rng.next_f32(),
            y: rng.next_f32(),
            radius: rng.next_range(0.3, 0.5),
            brightness: rng.next_range(0.15, 0.35),
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
    let mut img: RgbaImage = ImageBuffer::new(STAR_SIZE, STAR_SIZE);

    // Fill with transparent black (stars will be additive)
    for pixel in img.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 0]);
    }

    let size = STAR_SIZE as f32;

    // Add subtle nebula background in a few spots
    add_nebula_wisps(&mut img, rng);

    // Render each star
    for star in stars {
        let cx = (star.x * size) as i32;
        let cy = (star.y * size) as i32;
        let r = star.radius;

        // Draw star with gaussian-like falloff
        let extent = (r * 3.0).ceil() as i32;

        for dy in -extent..=extent {
            for dx in -extent..=extent {
                let px = cx + dx;
                let py = cy + dy;

                // Wrap for tiling
                let px_wrapped = px.rem_euclid(STAR_SIZE as i32) as u32;
                let py_wrapped = py.rem_euclid(STAR_SIZE as i32) as u32;

                let dist = ((dx * dx + dy * dy) as f32).sqrt();

                // Gaussian falloff
                let falloff = (-dist * dist / (2.0 * r * r)).exp();
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
    let size = STAR_SIZE as f32;

    // Create 2-4 nebula regions
    let num_nebulae = 2 + (rng.next() % 3) as usize;

    for _ in 0..num_nebulae {
        let cx = rng.next_f32() * size;
        let cy = rng.next_f32() * size;
        let nebula_radius = rng.next_range(60.0, 120.0);

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

        for y in 0..STAR_SIZE {
            for x in 0..STAR_SIZE {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;

                // Handle wrapping for tiling
                let dx = if dx.abs() > size / 2.0 {
                    dx - dx.signum() * size
                } else {
                    dx
                };
                let dy = if dy.abs() > size / 2.0 {
                    dy - dy.signum() * size
                } else {
                    dy
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
