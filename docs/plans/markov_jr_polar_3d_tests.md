# Markov Jr 3D Spherical: Test Plan

## Overview

This document defines the test suite for the 3D spherical coordinate extension of Markov Jr. It builds on the 2D polar test plan—all 2D tests must pass before 3D tests are meaningful.

**Prerequisite**: All tests in `markov_jr_polar_2d_tests.md` pass.

**Additional complexity in 3D**:
- Third dimension (φ/latitude) with its own variable subdivisions
- 8 symmetries instead of 4
- Polar cap handling (φ near 0° or 180°)
- Significantly more voxels (~460 million vs ~1.5 million)

---

## Test Infrastructure

### 3D-Specific Verification Methods

| Method | Use Case | Implementation |
|--------|----------|----------------|
| **Shell checksum** | Verify single radial shell | `assert_eq!(grid.shell_checksum(r), expected)` |
| **Latitude band check** | Verify single latitude | `assert_eq!(grid.band_checksum(r, phi), expected)` |
| **Volume sampling** | Spot-check large grids | Random sample of N voxels |
| **Cross-section image** | Visual slice verification | Render r-θ or r-φ slice |

### Test Harness Structure

```rust
#[cfg(test)]
mod spherical_3d_tests {
    use crate::markov_junior::spherical_grid::*;
    
    // Level 0: Data structure tests (builds on 2D)
    mod level_0_data_structures { ... }
    
    // Level 1: Coordinate math tests (φ dimension added)
    mod level_1_coordinates { ... }
    
    // Level 2: Neighbor relationships (6 directions now)
    mod level_2_neighbors { ... }
    
    // Level 3: Symmetry tests (8 symmetries)
    mod level_3_symmetries { ... }
    
    // Level 4: Single-step rule tests (3D patterns)
    mod level_4_single_step { ... }
    
    // Level 5: Multi-step model tests (3D models)
    mod level_5_models { ... }
    
    // Level 6: Rendering output tests (3D visualization)
    mod level_6_rendering { ... }
    
    // Level 7: Performance tests (large grids)
    mod level_7_performance { ... }
}
```

---

## Level 0: Data Structure Tests

**Goal**: Verify `SphericalMjGrid` stores and retrieves data correctly.

### Test 0.1: Grid Creation
```rust
#[test]
fn test_spherical_grid_creation() {
    let grid = SphericalMjGrid::new(
        r_min: 256,
        r_depth: 256,
        phi_min: 10,
        phi_max: 170,
        target_arc: 1.0,
    );
    
    assert_eq!(grid.r_min, 256);
    assert_eq!(grid.r_depth, 256);
    assert_eq!(grid.phi_min, 10);
    assert_eq!(grid.phi_max, 170);
    
    // Verify shell count
    assert_eq!(grid.shells.len(), 256);
    
    // Verify band count per shell
    for shell in &grid.shells {
        assert_eq!(shell.len(), 160);  // 170 - 10 = 160 latitude bands
    }
}
```

### Test 0.2: Cell Read/Write
```rust
#[test]
fn test_spherical_cell_read_write() {
    let mut grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    // Write to various locations
    grid.set(0, 0, 0, 42);      // Inner, north
    grid.set(128, 500, 80, 99); // Middle, equator
    grid.set(255, 1000, 159, 7); // Outer, south
    
    // Read back
    assert_eq!(grid.get(0, 0, 0), 42);
    assert_eq!(grid.get(128, 500, 80), 99);
    assert_eq!(grid.get(255, 1000, 159), 7);
}
```

### Test 0.3: Theta Wrapping at All Latitudes
```rust
#[test]
fn test_theta_wrapping_all_latitudes() {
    let mut grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    for phi in [0u8, 40, 80, 120, 159] {  // Various latitudes
        let r = 128u8;
        let theta_divs = grid.theta_divisions(r, phi);
        
        grid.set(r, 0, phi, 42);
        
        // Wrapped access should return same value
        assert_eq!(grid.get(r, theta_divs, phi), 42);
        assert_eq!(grid.get(r, theta_divs * 2, phi), 42);
    }
}
```

### Test 0.4: Phi Does Not Wrap
```rust
#[test]
fn test_phi_no_wrap() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    // Phi should NOT wrap - it's bounded by polar caps
    // Accessing phi outside [0, 159] should panic or return boundary
    
    // This tests that phi=0 and phi=159 are valid boundaries
    let _ = grid.get(128, 0, 0);    // Valid: phi=0 (10° latitude)
    let _ = grid.get(128, 0, 159);  // Valid: phi=159 (170° latitude)
    
    // phi=160 should be out of bounds
    let result = std::panic::catch_unwind(|| grid.get(128, 0, 160));
    assert!(result.is_err(), "phi=160 should be out of bounds");
}
```

### Test 0.5: Memory Layout (Variable θ per Latitude)
```rust
#[test]
fn test_memory_layout_variable_theta() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    let r = 200u8;  // Fixed radius
    
    // θ divisions at equator (phi=80) should be maximum
    let theta_equator = grid.theta_divisions(r, 80);
    
    // θ divisions near poles should be smaller
    let theta_north = grid.theta_divisions(r, 0);   // Near north cap
    let theta_south = grid.theta_divisions(r, 159); // Near south cap
    
    assert!(theta_equator > theta_north);
    assert!(theta_equator > theta_south);
    
    // Ratio should roughly match sin(φ)
    let ratio = theta_north as f32 / theta_equator as f32;
    let expected_ratio = (10.0_f32.to_radians()).sin() / (90.0_f32.to_radians()).sin();
    assert!((ratio - expected_ratio).abs() < 0.1);
}
```

### Test 0.6: Total Voxel Count
```rust
#[test]
fn test_total_voxel_count() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    let total = grid.total_voxel_count();
    
    // Expected: ~460 million for these parameters
    // Allow 10% tolerance for rounding in θ divisions
    assert!(total > 400_000_000);
    assert!(total < 520_000_000);
}
```

---

## Level 1: Coordinate Math Tests

**Goal**: Verify theta_divisions varies correctly with both r and φ.

### Test 1.1: Theta Divisions at Equator
```rust
#[test]
fn test_theta_divisions_equator() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    // At equator (phi=80, which is 90° latitude), sin(φ) = 1
    // θ_divisions should equal 2D polar case
    
    let phi_equator = 80u8;  // 10° + 80° = 90°
    
    for r in [0u8, 128, 255] {
        let r_actual = 256 + r as u32;
        let expected = (2.0 * PI * r_actual as f32 / 1.0).round() as u16;
        let actual = grid.theta_divisions(r, phi_equator);
        
        assert!((actual as i32 - expected as i32).abs() < 2,
            "At r={}, expected θ_divs={}, got {}", r, expected, actual);
    }
}
```

### Test 1.2: Theta Divisions at Various Latitudes
```rust
#[test]
fn test_theta_divisions_latitude_scaling() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    let r = 200u8;
    
    // θ_divisions should scale with sin(φ)
    let test_cases = [
        (0, 10.0),    // phi=0 -> 10° latitude
        (35, 45.0),   // phi=35 -> 45° latitude
        (80, 90.0),   // phi=80 -> 90° latitude (equator)
        (125, 135.0), // phi=125 -> 135° latitude
        (159, 170.0), // phi=159 -> 170° latitude
    ];
    
    let equator_divs = grid.theta_divisions(r, 80) as f32;
    
    for (phi, latitude_deg) in test_cases {
        let actual = grid.theta_divisions(r, phi) as f32;
        let expected_ratio = latitude_deg.to_radians().sin();
        let actual_ratio = actual / equator_divs;
        
        assert!((actual_ratio - expected_ratio).abs() < 0.05,
            "At φ={} ({}°), expected ratio {}, got {}", 
            phi, latitude_deg, expected_ratio, actual_ratio);
    }
}
```

### Test 1.3: Radial Distortion Within Bounds
```rust
#[test]
fn test_radial_distortion_3d() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    // Same as 2D: radial distortion should be < 1%
    for phi in [0u8, 40, 80, 120, 159] {
        for r in 0..255u8 {
            let current_divs = grid.theta_divisions(r, phi) as f32;
            let next_divs = grid.theta_divisions(r + 1, phi) as f32;
            
            if current_divs > 0.0 && next_divs > 0.0 {
                let ratio = next_divs / current_divs;
                let distortion = (ratio - 1.0).abs();
                
                assert!(distortion < 0.01,
                    "Radial distortion {} at r={}, φ={}", distortion, r, phi);
            }
        }
    }
}
```

### Test 1.4: Latitudinal Distortion Within Bounds
```rust
#[test]
fn test_latitudinal_distortion() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    let r = 200u8;
    
    // Latitudinal distortion: ratio of θ_divisions between adjacent φ bands
    // Should be < 10% except very near polar caps
    
    for phi in 5..155u8 {  // Skip edges
        let current_divs = grid.theta_divisions(r, phi) as f32;
        let next_divs = grid.theta_divisions(r, phi + 1) as f32;
        
        if current_divs > 0.0 {
            let ratio = next_divs / current_divs;
            let distortion = (ratio - 1.0).abs();
            
            assert!(distortion < 0.15,
                "Latitudinal distortion {} at φ={}", distortion, phi);
        }
    }
}
```

### Test 1.5: Polar Cap Edge Behavior
```rust
#[test]
fn test_polar_cap_edges() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    // At polar cap edges (φ=0 and φ=159), θ_divisions should be small but > 0
    for r in [0u8, 128, 255] {
        let north_divs = grid.theta_divisions(r, 0);
        let south_divs = grid.theta_divisions(r, 159);
        
        assert!(north_divs >= 1, "North cap edge has 0 divisions at r={}", r);
        assert!(south_divs >= 1, "South cap edge has 0 divisions at r={}", r);
    }
}
```

---

## Level 2: Neighbor Relationship Tests

**Goal**: Verify 6-directional neighbor lookups.

### Test 2.1: Angular Neighbors (Always Exactly 2)
```rust
#[test]
fn test_angular_neighbors_3d() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    for r in [0u8, 128, 255] {
        for phi in [0u8, 40, 80, 120, 159] {
            let divs = grid.theta_divisions(r, phi);
            if divs < 2 { continue; }  // Skip degenerate cases
            
            for theta in [0u16, divs / 2, divs - 1] {
                let neighbors = grid.neighbors(r, theta, phi);
                
                // Always exactly 2 angular neighbors
                assert_eq!(neighbors.theta_minus, (r, (theta + divs - 1) % divs, phi));
                assert_eq!(neighbors.theta_plus, (r, (theta + 1) % divs, phi));
            }
        }
    }
}
```

### Test 2.2: Latitude Neighbors (Variable Count)
```rust
#[test]
fn test_latitude_neighbors() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    let r = 128u8;
    let phi = 80u8;  // Equator
    let theta = grid.theta_divisions(r, phi) / 2;
    
    let neighbors = grid.neighbors(r, theta, phi);
    
    // Should have at least 1 neighbor in each φ direction
    assert!(!neighbors.phi_minus.is_empty(), "No phi_minus neighbors");
    assert!(!neighbors.phi_plus.is_empty(), "No phi_plus neighbors");
    
    // Neighbor count should be 1-2
    assert!(neighbors.phi_minus.len() <= 2);
    assert!(neighbors.phi_plus.len() <= 2);
}
```

### Test 2.3: Latitude Neighbor Boundaries
```rust
#[test]
fn test_latitude_neighbor_boundaries() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    let r = 128u8;
    
    // At φ=0 (north cap edge), phi_minus should be empty
    let neighbors = grid.neighbors(r, 0, 0);
    assert!(neighbors.phi_minus.is_empty());
    assert!(!neighbors.phi_plus.is_empty());
    
    // At φ=159 (south cap edge), phi_plus should be empty
    let neighbors = grid.neighbors(r, 0, 159);
    assert!(!neighbors.phi_minus.is_empty());
    assert!(neighbors.phi_plus.is_empty());
}
```

### Test 2.4: Radial Neighbors (Variable Count)
```rust
#[test]
fn test_radial_neighbors_3d() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    let r = 128u8;
    let phi = 80u8;
    let theta = grid.theta_divisions(r, phi) / 2;
    
    let neighbors = grid.neighbors(r, theta, phi);
    
    // Should have at least 1 neighbor in each r direction
    assert!(!neighbors.r_minus.is_empty());
    assert!(!neighbors.r_plus.is_empty());
}
```

### Test 2.5: Radial Neighbor Boundaries
```rust
#[test]
fn test_radial_neighbor_boundaries_3d() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    let phi = 80u8;
    
    // At r=0, r_minus should be empty
    let neighbors = grid.neighbors(0, 0, phi);
    assert!(neighbors.r_minus.is_empty());
    
    // At r=255, r_plus should be empty
    let neighbors = grid.neighbors(255, 0, phi);
    assert!(neighbors.r_plus.is_empty());
}
```

### Test 2.6: Neighbor Overlap Correctness (3D)
```rust
#[test]
fn test_neighbor_overlap_3d() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    // For any voxel, neighbors should have overlapping angular ranges
    let r = 128u8;
    let phi = 80u8;
    let theta = grid.theta_divisions(r, phi) / 2;
    
    let (my_start, my_end) = grid.angular_range(r, theta, phi);
    let neighbors = grid.neighbors(r, theta, phi);
    
    // Check all radial neighbors
    for &(nr, nt, np) in neighbors.r_minus.iter().chain(neighbors.r_plus.iter()) {
        let (n_start, n_end) = grid.angular_range(nr, nt, np);
        assert!(ranges_overlap(my_start, my_end, n_start, n_end),
            "Neighbor ({},{},{}) doesn't overlap", nr, nt, np);
    }
}
```

### Test 2.7: Neighbor Symmetry (3D)
```rust
#[test]
fn test_neighbor_symmetry_3d() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    let r = 128u8;
    let phi = 80u8;
    let theta = grid.theta_divisions(r, phi) / 2;
    
    let neighbors = grid.neighbors(r, theta, phi);
    
    // If B is r_plus neighbor of A, then A should be r_minus neighbor of B
    for &(nr, nt, np) in &neighbors.r_plus {
        let reverse = grid.neighbors(nr, nt, np);
        let found = reverse.r_minus.iter().any(|&(rr, rt, rp)| {
            rr == r && rt == theta && rp == phi
        });
        assert!(found, "Radial neighbor symmetry violated");
    }
}
```

### Test 2.8: Total Neighbor Count
```rust
#[test]
fn test_total_neighbor_count() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    // Interior voxels should have 6 neighbor directions
    // Each direction may have 1-2 actual neighbors
    
    let r = 128u8;
    let phi = 80u8;
    let theta = grid.theta_divisions(r, phi) / 2;
    
    let neighbors = grid.neighbors(r, theta, phi);
    
    let total_neighbors = 2  // theta always has exactly 2
        + neighbors.phi_minus.len()
        + neighbors.phi_plus.len()
        + neighbors.r_minus.len()
        + neighbors.r_plus.len();
    
    // Should be between 6 (all 1:1) and 10 (some 1:2)
    assert!(total_neighbors >= 6);
    assert!(total_neighbors <= 10);
}
```

---

## Level 3: Symmetry Tests

**Goal**: Verify the 8 spherical symmetries.

### Test 3.1: All 8 Symmetries Exist
```rust
#[test]
fn test_eight_symmetries_exist() {
    let symmetries = SphericalSymmetry::all();
    assert_eq!(symmetries.len(), 8);
}
```

### Test 3.2: Identity Transform
```rust
#[test]
fn test_identity_3d() {
    use SphericalSymmetry::*;
    
    assert_eq!(Identity.transform(1, 2, 3), (1, 2, 3));
    assert_eq!(Identity.transform(-1, -2, -3), (-1, -2, -3));
}
```

### Test 3.3: Individual Flip Transforms
```rust
#[test]
fn test_individual_flips() {
    use SphericalSymmetry::*;
    
    // ThetaFlip: (dr, dθ, dφ) → (dr, -dθ, dφ)
    assert_eq!(ThetaFlip.transform(1, 2, 3), (1, -2, 3));
    
    // PhiFlip: (dr, dθ, dφ) → (dr, dθ, -dφ)
    assert_eq!(PhiFlip.transform(1, 2, 3), (1, 2, -3));
    
    // RFlip: (dr, dθ, dφ) → (-dr, dθ, dφ)
    assert_eq!(RFlip.transform(1, 2, 3), (-1, 2, 3));
}
```

### Test 3.4: Combined Flip Transforms
```rust
#[test]
fn test_combined_flips() {
    use SphericalSymmetry::*;
    
    // ThetaPhiFlip
    assert_eq!(ThetaPhiFlip.transform(1, 2, 3), (1, -2, -3));
    
    // ThetaRFlip
    assert_eq!(ThetaRFlip.transform(1, 2, 3), (-1, -2, 3));
    
    // PhiRFlip
    assert_eq!(PhiRFlip.transform(1, 2, 3), (-1, 2, -3));
    
    // AllFlip
    assert_eq!(AllFlip.transform(1, 2, 3), (-1, -2, -3));
}
```

### Test 3.5: Symmetry Group Closure
```rust
#[test]
fn test_symmetry_group_closure_3d() {
    use SphericalSymmetry::*;
    
    // Z2 × Z2 × Z2 group: composing any two should give another element
    let symmetries = SphericalSymmetry::all();
    
    for &s1 in &symmetries {
        for &s2 in &symmetries {
            let (dr, dt, dp) = s1.transform(1, 1, 1);
            let composed = s2.transform(dr, dt, dp);
            
            let found = symmetries.iter().any(|&s| s.transform(1, 1, 1) == composed);
            assert!(found, "Composition not in group");
        }
    }
}
```

### Test 3.6: Involutions (Self-Inverse)
```rust
#[test]
fn test_involutions() {
    // Every symmetry in Z2³ is self-inverse
    for sym in SphericalSymmetry::all() {
        let (dr, dt, dp) = sym.transform(1, 2, 3);
        let back = sym.transform(dr, dt, dp);
        assert_eq!(back, (1, 2, 3), "{:?} is not self-inverse", sym);
    }
}
```

### Test 3.7: Pattern Symmetry Variants (3D)
```rust
#[test]
fn test_pattern_variants_3d() {
    // Asymmetric pattern should have 8 distinct variants
    let pattern = SphericalPattern {
        center: 1,
        theta_minus: Some(2),
        theta_plus: Some(3),
        phi_minus: Some(4),
        phi_plus: Some(5),
        r_minus: Some(6),
        r_plus: Some(7),
    };
    
    let variants: HashSet<_> = SphericalSymmetry::all()
        .iter()
        .map(|s| pattern.transform(*s))
        .collect();
    
    assert_eq!(variants.len(), 8);
}
```

### Test 3.8: Symmetric Pattern Fewer Variants
```rust
#[test]
fn test_symmetric_pattern_3d() {
    // Pattern symmetric under θ-flip should have 4 variants
    let pattern = SphericalPattern {
        center: 1,
        theta_minus: Some(2),
        theta_plus: Some(2),  // Same!
        phi_minus: Some(3),
        phi_plus: Some(4),
        r_minus: Some(5),
        r_plus: Some(6),
    };
    
    let variants: HashSet<_> = SphericalSymmetry::all()
        .iter()
        .map(|s| pattern.transform(*s))
        .collect();
    
    assert_eq!(variants.len(), 4);
}
```

---

## Level 4: Single-Step Rule Tests

**Goal**: Verify rules match and apply in 3D.

### Test 4.1: Simple 3D Rule Match
```rust
#[test]
fn test_simple_3d_rule_match() {
    let mut grid = SphericalMjGrid::new(256, 64, 10, 170, 1.0);
    
    let r = 32u8;
    let phi = 80u8;
    let theta = grid.theta_divisions(r, phi) / 2;
    
    // Set up: center=0, r_plus neighbor=1
    let neighbors = grid.neighbors(r, theta, phi);
    for &(nr, nt, np) in &neighbors.r_plus {
        grid.set(nr, nt, np, 1);
    }
    
    let pattern = SphericalPattern {
        center: 0,
        r_plus: Some(1),
        ..Default::default()
    };
    
    assert!(pattern.matches(&grid, r, theta, phi));
}
```

### Test 4.2: 3D Rule Application
```rust
#[test]
fn test_3d_rule_application() {
    let mut grid = SphericalMjGrid::new(256, 64, 10, 170, 1.0);
    
    let rule = SphericalRule {
        input: SphericalPattern { center: 0, ..Default::default() },
        output: 1,
    };
    
    let r = 32u8;
    let phi = 80u8;
    let theta = 100u16;
    
    assert_eq!(grid.get(r, theta, phi), 0);
    rule.apply(&mut grid, r, theta, phi);
    assert_eq!(grid.get(r, theta, phi), 1);
}
```

### Test 4.3: Rule With All 8 Symmetries
```rust
#[test]
fn test_rule_with_8_symmetries() {
    let base_rule = SphericalRule {
        input: SphericalPattern {
            center: 0,
            r_plus: Some(1),
            phi_plus: Some(2),
            ..Default::default()
        },
        output: 3,
    };
    
    let rules = base_rule.with_all_symmetries();
    assert_eq!(rules.len(), 8);
    
    // Verify each variant is distinct
    let patterns: HashSet<_> = rules.iter().map(|r| &r.input).collect();
    assert_eq!(patterns.len(), 8);
}
```

### Test 4.4: Rule Matching Across Dimensions
```rust
#[test]
fn test_rule_matching_dimensions() {
    let mut grid = SphericalMjGrid::new(256, 64, 10, 170, 1.0);
    
    let r = 32u8;
    let phi = 80u8;
    let theta = grid.theta_divisions(r, phi) / 2;
    
    // Set each neighbor direction to a different value
    let neighbors = grid.neighbors(r, theta, phi);
    
    // theta_minus = 1
    grid.set(neighbors.theta_minus.0, neighbors.theta_minus.1, neighbors.theta_minus.2, 1);
    // theta_plus = 2
    grid.set(neighbors.theta_plus.0, neighbors.theta_plus.1, neighbors.theta_plus.2, 2);
    // phi_minus = 3
    for &(nr, nt, np) in &neighbors.phi_minus {
        grid.set(nr, nt, np, 3);
    }
    // phi_plus = 4
    for &(nr, nt, np) in &neighbors.phi_plus {
        grid.set(nr, nt, np, 4);
    }
    // r_minus = 5
    for &(nr, nt, np) in &neighbors.r_minus {
        grid.set(nr, nt, np, 5);
    }
    // r_plus = 6
    for &(nr, nt, np) in &neighbors.r_plus {
        grid.set(nr, nt, np, 6);
    }
    
    // Pattern that requires all 6 directions
    let pattern = SphericalPattern {
        center: 0,
        theta_minus: Some(1),
        theta_plus: Some(2),
        phi_minus: Some(3),
        phi_plus: Some(4),
        r_minus: Some(5),
        r_plus: Some(6),
    };
    
    assert!(pattern.matches(&grid, r, theta, phi));
}
```

---

## Level 5: Multi-Step Model Tests

**Goal**: Verify complete 3D models.

### Test 5.1: Shell Fill (Surface Only)
```rust
#[test]
fn test_shell_fill() {
    let mut grid = SphericalMjGrid::new(256, 64, 10, 170, 1.0);
    
    // Seed: single point on outer surface
    grid.set(63, 0, 80, 1);
    
    // Rule: spread along surface (same r)
    let rule = SphericalRule {
        input: SphericalPattern {
            center: 0,
            theta_minus: Some(1),
            ..Default::default()
        },
        output: 1,
    };
    let rules = rule.with_all_symmetries();
    
    // Run until outer shell is filled
    run_until_stable(&mut grid, &rules, r_filter: Some(63));
    
    // Entire outer shell should be 1
    let phi_count = 160;
    for phi in 0..phi_count {
        let divs = grid.theta_divisions(63, phi as u8);
        for theta in 0..divs {
            assert_eq!(grid.get(63, theta, phi as u8), 1);
        }
    }
}
```

### Test 5.2: Radial Growth (Planet Formation)
```rust
#[test]
fn test_radial_growth() {
    let mut grid = SphericalMjGrid::new(256, 64, 10, 170, 1.0);
    
    // Seed: entire inner shell set to 1 (core)
    for phi in 0..160u8 {
        let divs = grid.theta_divisions(0, phi);
        for theta in 0..divs {
            grid.set(0, theta, phi, 1);
        }
    }
    
    // Rule: grow outward
    let rule = SphericalRule {
        input: SphericalPattern {
            center: 0,
            r_minus: Some(1),
            ..Default::default()
        },
        output: 1,
    };
    
    // Run 63 steps
    for _ in 0..63 {
        apply_rule_to_all(&mut grid, &rule);
    }
    
    // All cells should be 1
    assert!(grid.all_cells_equal(1));
}
```

### Test 5.3: Latitude Bands (Climate Zones)
```rust
#[test]
fn test_latitude_bands() {
    let mut grid = SphericalMjGrid::new(256, 64, 10, 170, 1.0);
    
    // Initialize based on latitude:
    // φ < 40: cold (1)
    // 40 ≤ φ < 120: temperate (2)  
    // φ ≥ 120: cold (1)
    
    for r in 0..64u8 {
        for phi in 0..160u8 {
            let value = if phi < 40 || phi >= 120 { 1 } else { 2 };
            let divs = grid.theta_divisions(r, phi);
            for theta in 0..divs {
                grid.set(r, theta, phi, value);
            }
        }
    }
    
    // Verify bands
    for phi in 0..160u8 {
        let expected = if phi < 40 || phi >= 120 { 1 } else { 2 };
        assert_eq!(grid.get(32, 0, phi), expected);
    }
}
```

### Test 5.4: Terrain Generation (Surface Detail)
```rust
#[test]
fn test_terrain_generation() {
    let mut grid = SphericalMjGrid::new(256, 32, 10, 170, 1.0);
    let mut rng = StdRng::seed_from_u64(42);
    
    // Generate terrain on outer shell
    // ... terrain rules ...
    
    // Verify terrain properties:
    // 1. No isolated single-cell features
    // 2. Reasonable distribution of terrain types
    // 3. Deterministic with same seed
    
    let checksum = grid.shell_checksum(31);
    
    // Re-run with same seed
    let mut grid2 = SphericalMjGrid::new(256, 32, 10, 170, 1.0);
    let mut rng2 = StdRng::seed_from_u64(42);
    // ... same generation ...
    
    assert_eq!(grid2.shell_checksum(31), checksum);
}
```

### Test 5.5: Layered Planet (Multiple Materials)
```rust
#[test]
fn test_layered_planet() {
    let mut grid = SphericalMjGrid::new(256, 64, 10, 170, 1.0);
    
    // Layer assignment:
    // r 0-15: core (4)
    // r 16-47: mantle (3)
    // r 48-55: crust (2)
    // r 56-63: surface (1)
    
    for r in 0..64u8 {
        let value = match r {
            0..=15 => 4,
            16..=47 => 3,
            48..=55 => 2,
            56..=63 => 1,
            _ => unreachable!(),
        };
        
        for phi in 0..160u8 {
            let divs = grid.theta_divisions(r, phi);
            for theta in 0..divs {
                grid.set(r, theta, phi, value);
            }
        }
    }
    
    // Verify layers
    assert_eq!(grid.get(8, 0, 80), 4);   // Core
    assert_eq!(grid.get(30, 0, 80), 3);  // Mantle
    assert_eq!(grid.get(52, 0, 80), 2);  // Crust
    assert_eq!(grid.get(60, 0, 80), 1);  // Surface
}
```

### Test 5.6: Reference Output (Full Model)
```rust
#[test]
fn test_reference_output_3d() {
    let grid = run_model("test_models/planet_terrain.xml", seed: 42);
    
    // Compare against pre-computed checksum
    assert_eq!(grid.checksum(), EXPECTED_CHECKSUM);
    
    // Verify shell checksums for each layer
    assert_eq!(grid.shell_checksum(0), EXPECTED_CORE_CHECKSUM);
    assert_eq!(grid.shell_checksum(63), EXPECTED_SURFACE_CHECKSUM);
}
```

---

## Level 6: Rendering Output Tests

**Goal**: Verify 3D grid can be visualized correctly.

### Test 6.1: Spherical to Cartesian Conversion
```rust
#[test]
fn test_spherical_to_cartesian() {
    // At equator (φ=90°), θ=0°: should be on +X axis
    let (x, y, z) = spherical_to_cartesian(r: 100, theta: 0, phi: 90, r_min: 256);
    let r_actual = 256 + 100;
    
    assert!((x - r_actual as f32).abs() < 1.0);
    assert!(y.abs() < 1.0);
    assert!(z.abs() < 1.0);
    
    // At north pole (φ=0°): should be on +Y axis
    let (x, y, z) = spherical_to_cartesian(100, 0, 0, 256);
    assert!(x.abs() < 10.0);  // Some tolerance due to cap not being exact pole
    assert!(y > 0.0);
}
```

### Test 6.2: Cross-Section Rendering
```rust
#[test]
fn test_cross_section_render() {
    let grid = create_test_planet();
    
    // Render equatorial cross-section (φ=80)
    let image = grid.render_cross_section(phi: 80, width: 512, height: 512);
    
    // Verify it shows concentric rings
    image.save("test_output/equatorial_cross_section.png").unwrap();
    
    // Compare against reference
    let diff = image_diff(&image, "test_references/equatorial_cross_section.png");
    assert!(diff < 0.01);
}
```

### Test 6.3: Surface Render (Mercator-ish Projection)
```rust
#[test]
fn test_surface_render() {
    let grid = create_test_planet();
    
    // Render outer surface as 2D map
    let image = grid.render_surface(r: 63, width: 1024, height: 512);
    
    image.save("test_output/surface_map.png").unwrap();
}
```

### Test 6.4: Mesh Generation (Cube Approximation)
```rust
#[test]
fn test_mesh_generation() {
    let grid = create_test_planet();
    
    // Generate mesh for outer shell
    let mesh = grid.generate_mesh(r_range: 60..64);
    
    // Verify mesh properties
    assert!(mesh.vertex_count() > 0);
    assert!(mesh.triangle_count() > 0);
    
    // Verify all vertices are at expected radius range
    for vertex in mesh.vertices() {
        let r = vertex.position.length();
        assert!(r >= 256.0 + 60.0);
        assert!(r <= 256.0 + 64.0);
    }
}
```

---

## Level 7: Performance Tests

**Goal**: Verify large grids perform acceptably.

### Test 7.1: Grid Creation Time
```rust
#[test]
fn test_grid_creation_performance() {
    let start = Instant::now();
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    let elapsed = start.elapsed();
    
    // Should create in < 5 seconds
    assert!(elapsed < Duration::from_secs(5),
        "Grid creation took {:?}", elapsed);
}
```

### Test 7.2: Full Iteration Time
```rust
#[test]
fn test_iteration_performance() {
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    
    let start = Instant::now();
    let mut count = 0;
    for r in 0..256u8 {
        for phi in 0..160u8 {
            let divs = grid.theta_divisions(r, phi);
            for theta in 0..divs {
                let _ = grid.get(r, theta, phi);
                count += 1;
            }
        }
    }
    let elapsed = start.elapsed();
    
    // Should iterate ~460M cells in < 30 seconds
    assert!(elapsed < Duration::from_secs(30),
        "Iteration of {} cells took {:?}", count, elapsed);
}
```

### Test 7.3: Rule Application Performance
```rust
#[test]
fn test_rule_application_performance() {
    let mut grid = SphericalMjGrid::new(256, 64, 10, 170, 1.0);  // Smaller
    
    let rule = SphericalRule {
        input: SphericalPattern { center: 0, ..Default::default() },
        output: 1,
    };
    
    let start = Instant::now();
    apply_rule_to_all(&mut grid, &rule);
    let elapsed = start.elapsed();
    
    // Single rule pass on ~30M cells should be < 5 seconds
    assert!(elapsed < Duration::from_secs(5));
}
```

### Test 7.4: Memory Usage
```rust
#[test]
fn test_memory_usage() {
    let before = get_memory_usage();
    let grid = SphericalMjGrid::new(256, 256, 10, 170, 1.0);
    let after = get_memory_usage();
    
    let used_mb = (after - before) / 1_000_000;
    
    // ~460M cells × 1 byte = ~460 MB
    // Allow 2x overhead for structure
    assert!(used_mb < 1000, "Used {} MB", used_mb);
}
```

---

## Test Models (3D)

### Model Files

| Model | Purpose | Expected Behavior |
|-------|---------|-------------------|
| `empty_sphere.xml` | Baseline | All cells remain 0 |
| `solid_sphere.xml` | Fill all | All cells become 1 |
| `layered_planet.xml` | Radial layers | Concentric shells |
| `climate_bands.xml` | Latitude bands | Horizontal stripes |
| `terrain_surface.xml` | Surface detail | Mountains/valleys on outer shell |
| `cave_system.xml` | Internal structure | Connected voids in mantle |

### Reference Outputs

Each model needs:
1. Full grid checksum
2. Per-shell checksums
3. Cross-section reference images
4. Surface map reference images

---

## Continuous Integration

### Test Execution Order

```yaml
test_3d:
  stage: test
  needs: [test_2d]  # 2D must pass first
  script:
    - cargo test spherical_3d_tests::level_0
    - cargo test spherical_3d_tests::level_1
    - cargo test spherical_3d_tests::level_2
    - cargo test spherical_3d_tests::level_3
    - cargo test spherical_3d_tests::level_4
    - cargo test spherical_3d_tests::level_5
    - cargo test spherical_3d_tests::level_6
    - cargo test spherical_3d_tests::level_7 --release  # Performance tests need release build
```

---

## Summary

| Level | Tests | Purpose |
|-------|-------|---------|
| 0 | 6 | Data structure basics |
| 1 | 5 | Coordinate math (r, θ, φ) |
| 2 | 8 | 6-directional neighbors |
| 3 | 8 | 8 symmetries |
| 4 | 4 | Single-step rules |
| 5 | 6 | Multi-step models |
| 6 | 4 | Rendering output |
| 7 | 4 | Performance |

**Total: 45 tests** for 3D spherical Markov Jr.

**Combined with 2D: 81 tests** covering the complete polar/spherical extension.
