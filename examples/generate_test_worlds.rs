//! Generate and save test worlds for use by other examples.
//!
//! This creates a set of reusable voxworld files in assets/worlds/
//! so examples can load them instead of building worlds from scratch.
//!
//! Run with: `cargo run --example generate_test_worlds`

use studio_core::{save_world, Voxel, VoxelWorld, CHUNK_SIZE};

fn main() {
    println!("Generating test worlds...\n");
    std::fs::create_dir_all("assets/worlds").expect("Failed to create worlds directory");

    // 1. Simple ground with pillar (for basic lighting tests)
    generate_ground_pillar();

    // 2. Island scene (like p9_island but as voxworld)
    generate_island();

    // 3. Multi-chunk terrain (for chunk tests)
    generate_multi_chunk_terrain();

    // 4. Shadow test scene (ground + occluders)
    generate_shadow_test();

    // 5. Mesh optimization test (solid cubes for face culling/greedy mesh stats)
    generate_mesh_test();

    // 6. Fog test scene (voxels at different depths)
    generate_fog_test();

    // 7. Cross-chunk culling test scene
    generate_cross_chunk_test();

    // 8. GTAO test scene (for XeGTAO verification)
    generate_gtao_test();

    println!("\nAll test worlds generated!");
}

fn generate_ground_pillar() {
    println!("Generating: ground_pillar.voxworld");
    let mut world = VoxelWorld::new();

    // 7x7 ground at y=0
    let ground_color = Voxel::solid(128, 128, 128);
    for x in -3..=3 {
        for z in -3..=3 {
            world.set_voxel(x, 0, z, ground_color);
        }
    }

    // Red emissive pillar at center
    for y in 1..=4 {
        world.set_voxel(0, y, 0, Voxel::emissive(255, 50, 50));
    }

    save_world(&world, "assets/worlds/ground_pillar.voxworld").expect("Failed to save");
    println!(
        "  -> {} chunks, {} voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );
}

fn generate_island() {
    println!("Generating: island.voxworld");
    let mut world = VoxelWorld::new();

    // Floating island base (irregular shape)
    // Stone core
    for x in -6..=6 {
        for z in -6..=6 {
            let dist = ((x * x + z * z) as f32).sqrt();
            if dist < 7.0 {
                let depth = (3.0 - dist * 0.3).max(0.0) as i32;
                for y in -depth..0 {
                    world.set_voxel(x, y, z, Voxel::solid(100, 90, 80));
                }
            }
        }
    }

    // Dirt layer
    for x in -5..=5 {
        for z in -5..=5 {
            let dist = ((x * x + z * z) as f32).sqrt();
            if dist < 6.0 {
                world.set_voxel(x, 0, z, Voxel::solid(139, 90, 43));
            }
        }
    }

    // Grass layer
    for x in -5..=5 {
        for z in -5..=5 {
            let dist = ((x * x + z * z) as f32).sqrt();
            if dist < 5.5 {
                world.set_voxel(x, 1, z, Voxel::solid(34, 139, 34));
            }
        }
    }

    // Tree trunk
    for y in 2..=5 {
        world.set_voxel(2, y, 2, Voxel::solid(139, 90, 43));
    }

    // Tree leaves (simple sphere-ish)
    for x in 0..=4 {
        for y in 5..=8 {
            for z in 0..=4 {
                let dx = x - 2;
                let dy = y - 6;
                let dz = z - 2;
                if dx * dx + dy * dy + dz * dz < 5 {
                    world.set_voxel(x, y, z, Voxel::solid(0, 100, 0));
                }
            }
        }
    }

    // Glowing crystals
    world.set_voxel(-3, 2, -2, Voxel::emissive(100, 200, 255)); // Cyan
    world.set_voxel(-3, 3, -2, Voxel::emissive(100, 200, 255));
    world.set_voxel(-2, 2, 3, Voxel::emissive(255, 100, 200)); // Magenta
    world.set_voxel(-2, 3, 3, Voxel::emissive(255, 100, 200));
    world.set_voxel(-2, 4, 3, Voxel::emissive(255, 100, 200));

    save_world(&world, "assets/worlds/island.voxworld").expect("Failed to save");
    println!(
        "  -> {} chunks, {} voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );
}

fn generate_multi_chunk_terrain() {
    println!("Generating: multi_chunk_terrain.voxworld");
    let mut world = VoxelWorld::new();

    // 2x2 grid of chunks with distinct terrain
    let chunk_colors: [[Voxel; 2]; 2] = [
        [Voxel::solid(180, 60, 60), Voxel::solid(60, 180, 60)],
        [Voxel::solid(60, 60, 180), Voxel::solid(180, 180, 60)],
    ];

    for cx in 0..2 {
        for cz in 0..2 {
            let base_color = chunk_colors[cz as usize][cx as usize];
            let world_x_start = cx * CHUNK_SIZE as i32;
            let world_z_start = cz * CHUNK_SIZE as i32;

            for lx in 0..CHUNK_SIZE as i32 {
                for lz in 0..CHUNK_SIZE as i32 {
                    let wx = world_x_start + lx;
                    let wz = world_z_start + lz;
                    let height = 3 + ((wx.abs() + wz.abs()) % 5);

                    for wy in 0..height {
                        world.set_voxel(wx, wy, wz, base_color);
                    }
                }
            }

            // Crystal in center of each chunk
            let crystal_x = world_x_start + CHUNK_SIZE as i32 / 2;
            let crystal_z = world_z_start + CHUNK_SIZE as i32 / 2;
            for cy in 5..10 {
                let crystal = Voxel::new(
                    base_color.color[0].saturating_add(50),
                    base_color.color[1].saturating_add(50),
                    base_color.color[2].saturating_add(50),
                    200,
                );
                world.set_voxel(crystal_x, cy, crystal_z, crystal);
            }
        }
    }

    // Cross-chunk bridge (white)
    for x in 10..54 {
        world.set_voxel(x, 10, 16, Voxel::new(255, 255, 255, 128));
    }
    for z in 10..54 {
        world.set_voxel(16, 10, z, Voxel::new(255, 200, 100, 128));
    }

    save_world(&world, "assets/worlds/multi_chunk_terrain.voxworld").expect("Failed to save");
    println!(
        "  -> {} chunks, {} voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );
}

fn generate_shadow_test() {
    println!("Generating: shadow_test.voxworld");
    let mut world = VoxelWorld::new();

    // 16x16 light gray ground
    let ground = Voxel::solid(180, 180, 180);
    for x in 0..16 {
        for z in 0..16 {
            world.set_voxel(x, 0, z, ground);
        }
    }

    // Pillar 1: Dark red-brown
    let pillar1 = Voxel::solid(100, 60, 60);
    for y in 1..=4 {
        for x in 6..8 {
            for z in 10..12 {
                world.set_voxel(x, y, z, pillar1);
            }
        }
    }

    // Pillar 2: Dark green
    let pillar2 = Voxel::solid(60, 100, 60);
    for y in 1..=2 {
        for x in 10..12 {
            for z in 4..6 {
                world.set_voxel(x, y, z, pillar2);
            }
        }
    }

    save_world(&world, "assets/worlds/shadow_test.voxworld").expect("Failed to save");
    println!(
        "  -> {} chunks, {} voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );
}

fn generate_mesh_test() {
    println!("Generating: mesh_test.voxworld");
    let mut world = VoxelWorld::new();

    // 8x8x8 solid cube (for greedy mesh stats)
    let cube_color = Voxel::solid(100, 150, 200);
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                world.set_voxel(x, y, z, cube_color);
            }
        }
    }

    save_world(&world, "assets/worlds/mesh_test.voxworld").expect("Failed to save");
    println!(
        "  -> {} chunks, {} voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );

    // Also save in JSON for debugging
    save_world(&world, "assets/worlds/mesh_test.json").expect("Failed to save JSON");
    println!("  -> Also saved as mesh_test.json");
}

fn generate_fog_test() {
    println!("Generating: fog_test.voxworld");
    let mut world = VoxelWorld::new();

    // 4 white voxels at different Z depths to demonstrate fog gradient
    // Positions: spread out in X so they don't overlap visually
    // FOG_MAX_DISTANCE in shader is 50.0
    let positions = [
        (-3, 0, 2),  // Near
        (-1, 0, 10), // Mid-near
        (1, 0, 25),  // Mid-far
        (3, 0, 45),  // Far (very foggy)
    ];

    for (x, y, z) in positions {
        // White voxel, no emission
        world.set_voxel(x, y, z, Voxel::solid(255, 255, 255));
    }

    save_world(&world, "assets/worlds/fog_test.voxworld").expect("Failed to save");
    println!(
        "  -> {} chunks, {} voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );
}

fn generate_cross_chunk_test() {
    println!("Generating: cross_chunk_test.voxworld");
    let mut world = VoxelWorld::new();

    // Large wall at X chunk boundary (demonstrating seamless culling)
    for y in 4..20 {
        for z in 8..24 {
            world.set_voxel(31, y, z, Voxel::solid(180, 80, 60)); // Orange/red brick
            world.set_voxel(32, y, z, Voxel::solid(180, 80, 60));
        }
    }

    // Floor spanning chunks (both X and Z boundaries)
    for x in 24..40 {
        for z in 24..40 {
            world.set_voxel(x, 3, z, Voxel::solid(80, 80, 90)); // Gray stone
        }
    }

    // Glowing pillar at corner of 4 chunks
    for y in 4..12 {
        world.set_voxel(31, y, 31, Voxel::new(255, 200, 100, 200));
        world.set_voxel(32, y, 31, Voxel::new(255, 200, 100, 200));
        world.set_voxel(31, y, 32, Voxel::new(255, 200, 100, 200));
        world.set_voxel(32, y, 32, Voxel::new(255, 200, 100, 200));
    }

    // Bridge across Z chunk boundary
    for x in 16..28 {
        world.set_voxel(x, 8, 31, Voxel::solid(100, 140, 180)); // Blue-gray
        world.set_voxel(x, 8, 32, Voxel::solid(100, 140, 180));
    }

    save_world(&world, "assets/worlds/cross_chunk_test.voxworld").expect("Failed to save");
    println!(
        "  -> {} chunks, {} voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );
}

/// Generate GTAO test scene with specific geometries for AO verification.
///
/// Test geometries:
/// 1. Flat ground plane - should have NO false occlusion (AO ~1.0)
/// 2. 90-degree corner - should have proper corner darkening (AO ~0.3-0.5)
/// 3. Stairs/steps - should show smooth gradient without banding
/// 4. Floating cube - tests contact shadows
/// 5. Thin pillar - tests thin occluder handling
fn generate_gtao_test() {
    println!("Generating: gtao_test.voxworld");
    let mut world = VoxelWorld::new();

    let white = Voxel::solid(220, 220, 220); // Near-white for clear AO visibility
    let gray = Voxel::solid(180, 180, 180);

    // =========================================================================
    // 1. FLAT GROUND PLANE (16x16 at y=0)
    // Purpose: Verify no false occlusion on flat surfaces
    // Expected AO: ~1.0 (fully lit, no occlusion)
    // =========================================================================
    for x in 0..16 {
        for z in 0..16 {
            world.set_voxel(x, 0, z, white);
        }
    }

    // =========================================================================
    // 2. 90-DEGREE CORNER (at x=0, z=0)
    // Purpose: Verify proper corner darkening
    // Expected AO: ~0.3-0.5 in the corner crease
    // =========================================================================
    // Back wall (along X axis)
    for x in 0..8 {
        for y in 1..6 {
            world.set_voxel(x, y, 0, gray);
        }
    }
    // Side wall (along Z axis)
    for z in 1..8 {
        for y in 1..6 {
            world.set_voxel(0, y, z, gray);
        }
    }

    // =========================================================================
    // 3. STAIRS/STEPS (at x=10-14, z=0-4)
    // Purpose: Verify smooth AO gradient without banding
    // Expected: Each step should have slightly different AO, smooth transition
    // =========================================================================
    for step in 0..5 {
        let y = step + 1;
        for x in 10..15 {
            for z in 0..(5 - step) {
                world.set_voxel(x, y as i32, z, white);
            }
        }
    }

    // =========================================================================
    // 4. FLOATING CUBE (at x=4-6, y=3-5, z=8-10)
    // Purpose: Test contact shadows underneath
    // Expected: Ground beneath cube should show occlusion
    // =========================================================================
    for x in 4..7 {
        for y in 3..6 {
            for z in 8..11 {
                world.set_voxel(x, y, z, gray);
            }
        }
    }

    // =========================================================================
    // 5. THIN PILLAR (single column at x=12, z=12)
    // Purpose: Test thin occluder handling
    // Expected: Should not over-darken surrounding area
    // =========================================================================
    for y in 1..8 {
        world.set_voxel(12, y, 12, gray);
    }

    // =========================================================================
    // 6. AMBIENT LIGHT (emissive voxel for scene illumination)
    // =========================================================================
    world.set_voxel(8, 10, 8, Voxel::emissive(255, 250, 240)); // Warm white light

    save_world(&world, "assets/worlds/gtao_test.voxworld").expect("Failed to save");
    println!(
        "  -> {} chunks, {} voxels",
        world.chunk_count(),
        world.total_voxel_count()
    );
    println!("  Test geometries:");
    println!("    1. Flat ground (0-15, 0, 0-15) - expect AO ~1.0");
    println!("    2. Corner (0,0 walls) - expect AO ~0.3-0.5 in crease");
    println!("    3. Stairs (10-14, 1-5, 0-4) - expect smooth gradient");
    println!("    4. Floating cube (4-6, 3-5, 8-10) - expect shadow below");
    println!("    5. Thin pillar (12, 1-7, 12) - expect minimal over-darkening");
}
