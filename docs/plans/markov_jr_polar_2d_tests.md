# Markov Jr 2D Polar: Test Plan

## Overview

This document defines the test suite for the 2D polar coordinate extension of Markov Jr. Tests are ordered from foundational to complex, building confidence incrementally. Each test must pass before proceeding to the next level.

**Philosophy**: Tests first. We define what correct behavior looks like before writing implementation code. Every test should be automatable—no manual visual inspection required for pass/fail.

---

## Test Infrastructure

### Verification Methods

| Method | Use Case | Implementation |
|--------|----------|----------------|
| **Value assertion** | Single cell or small region values | `assert_eq!(grid.get(r, theta), expected)` |
| **Pattern assertion** | Expected pattern at location | `assert!(pattern.matches(grid, r, theta))` |
| **Checksum** | Deterministic full-grid verification | `assert_eq!(grid.checksum(), expected_hash)` |
| **Image diff** | Visual output comparison | Generate PNG, compare against reference |
| **Invariant check** | Properties that must always hold | `assert!(grid.all_cells_in_valid_range())` |

### Test Harness Structure

```rust
#[cfg(test)]
mod polar_2d_tests {
    use crate::markov_junior::polar_grid::*;
    
    // Level 0: Data structure tests
    mod level_0_data_structures { ... }
    
    // Level 1: Coordinate math tests
    mod level_1_coordinates { ... }
    
    // Level 2: Neighbor relationship tests
    mod level_2_neighbors { ... }
    
    // Level 3: Symmetry tests
    mod level_3_symmetries { ... }
    
    // Level 4: Single-step rule tests
    mod level_4_single_step { ... }
    
    // Level 5: Multi-step model tests
    mod level_5_models { ... }
    
    // Level 6: Rendering output tests
    mod level_6_rendering { ... }
}
```

---

## Level 0: Data Structure Tests

**Goal**: Verify the `PolarMjGrid` struct stores and retrieves data correctly.

### Test 0.1: Grid Creation
```rust
#[test]
fn test_grid_creation() {
    let grid = PolarMjGrid::new(r_min: 256, r_depth: 256, target_arc: 1.0);
    
    // Verify dimensions
    assert_eq!(grid.r_min, 256);
    assert_eq!(grid.r_depth, 256);
    
    // Verify ring count
    assert_eq!(grid.rings.len(), 256);
    
    // Verify all cells initialized to 0
    for r in 0..256u8 {
        let theta_divs = grid.theta_divisions(r);
        for theta in 0..theta_divs {
            assert_eq!(grid.get(r, theta), 0);
        }
    }
}
```

### Test 0.2: Cell Read/Write
```rust
#[test]
fn test_cell_read_write() {
    let mut grid = PolarMjGrid::new(256, 256, 1.0);
    
    // Write to various locations
    grid.set(0, 0, 42);
    grid.set(128, 500, 99);
    grid.set(255, 1000, 7);
    
    // Read back
    assert_eq!(grid.get(0, 0), 42);
    assert_eq!(grid.get(128, 500), 99);
    assert_eq!(grid.get(255, 1000), 7);
    
    // Verify other cells unchanged
    assert_eq!(grid.get(0, 1), 0);
    assert_eq!(grid.get(128, 501), 0);
}
```

### Test 0.3: Theta Wrapping
```rust
#[test]
fn test_theta_wrapping() {
    let mut grid = PolarMjGrid::new(256, 256, 1.0);
    let theta_divs = grid.theta_divisions(100);
    
    // Set a value
    grid.set(100, 0, 42);
    
    // Access via wrapped index should return same value
    assert_eq!(grid.get(100, theta_divs), 42);  // wraps to 0
    assert_eq!(grid.get(100, theta_divs * 2), 42);  // wraps to 0
    assert_eq!(grid.get(100, theta_divs + 5), grid.get(100, 5));
}
```

### Test 0.4: Memory Layout
```rust
#[test]
fn test_memory_layout() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    
    // Verify ring sizes increase with r
    let inner_size = grid.rings[0].len();
    let outer_size = grid.rings[255].len();
    
    assert!(outer_size > inner_size);
    
    // Verify approximate ratio matches r ratio
    let expected_ratio = (256.0 + 255.0) / 256.0;  // r_max / r_min
    let actual_ratio = outer_size as f32 / inner_size as f32;
    assert!((actual_ratio - expected_ratio).abs() < 0.01);
}
```

---

## Level 1: Coordinate Math Tests

**Goal**: Verify theta_divisions calculation and coordinate conversions.

### Test 1.1: Theta Divisions Formula
```rust
#[test]
fn test_theta_divisions_formula() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    
    // θ_divisions(r) = floor(2πr / target_arc)
    // At r=256 (index 0): 2π×256/1 ≈ 1608
    // At r=511 (index 255): 2π×511/1 ≈ 3210
    
    let divs_inner = grid.theta_divisions(0);
    let divs_outer = grid.theta_divisions(255);
    
    assert!((divs_inner as f32 - 1608.0).abs() < 2.0);
    assert!((divs_outer as f32 - 3210.0).abs() < 2.0);
}
```

### Test 1.2: Distortion Within Bounds
```rust
#[test]
fn test_distortion_within_bounds() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    
    // Max distortion should be at r_min (index 0)
    // distortion = (r+1)/r - 1 = 1/r
    // At r=256: distortion = 1/256 ≈ 0.0039
    
    for r in 0..255u8 {
        let current_divs = grid.theta_divisions(r) as f32;
        let next_divs = grid.theta_divisions(r + 1) as f32;
        let ratio = next_divs / current_divs;
        let distortion = (ratio - 1.0).abs();
        
        assert!(distortion < 0.01, "Distortion {} at r={} exceeds 1%", distortion, r);
    }
}
```

### Test 1.3: Arc Length Uniformity
```rust
#[test]
fn test_arc_length_uniformity() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    
    // Arc length = 2πr / θ_divisions(r)
    // Should be approximately target_arc (1.0) everywhere
    
    for r in 0..256u8 {
        let r_actual = 256 + r as u32;
        let divs = grid.theta_divisions(r);
        let arc_length = 2.0 * std::f32::consts::PI * r_actual as f32 / divs as f32;
        
        assert!((arc_length - 1.0).abs() < 0.1, 
            "Arc length {} at r={} deviates from target", arc_length, r);
    }
}
```

### Test 1.4: Angular Range Calculation
```rust
#[test]
fn test_angular_range() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    
    // Voxel (r, θ) should span [θ/divs × 2π, (θ+1)/divs × 2π]
    let r = 100u8;
    let theta = 50u16;
    let divs = grid.theta_divisions(r);
    
    let (start, end) = grid.angular_range(r, theta);
    
    let expected_start = theta as f32 / divs as f32 * 2.0 * PI;
    let expected_end = (theta + 1) as f32 / divs as f32 * 2.0 * PI;
    
    assert!((start - expected_start).abs() < 0.0001);
    assert!((end - expected_end).abs() < 0.0001);
}
```

---

## Level 2: Neighbor Relationship Tests

**Goal**: Verify neighbor lookups return correct voxels.

### Test 2.1: Angular Neighbors (Always Exactly 2)
```rust
#[test]
fn test_angular_neighbors() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    
    for r in [0u8, 50, 100, 200, 255] {
        let divs = grid.theta_divisions(r);
        
        for theta in [0u16, divs / 2, divs - 1] {
            let neighbors = grid.neighbors(r, theta);
            
            // Always exactly 2 angular neighbors
            assert_eq!(neighbors.theta_minus, (r, (theta + divs - 1) % divs));
            assert_eq!(neighbors.theta_plus, (r, (theta + 1) % divs));
        }
    }
}
```

### Test 2.2: Angular Neighbor Wrapping
```rust
#[test]
fn test_angular_neighbor_wrapping() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    let r = 100u8;
    let divs = grid.theta_divisions(r);
    
    // At theta=0, theta_minus should wrap to divs-1
    let neighbors = grid.neighbors(r, 0);
    assert_eq!(neighbors.theta_minus.1, divs - 1);
    
    // At theta=divs-1, theta_plus should wrap to 0
    let neighbors = grid.neighbors(r, divs - 1);
    assert_eq!(neighbors.theta_plus.1, 0);
}
```

### Test 2.3: Radial Neighbors (1:1 Mapping)
```rust
#[test]
fn test_radial_neighbors_one_to_one() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    
    // At low distortion, most voxels should have exactly 1 radial neighbor
    let mut one_to_one_count = 0;
    let mut total_count = 0;
    
    for r in 1..255u8 {  // Skip boundaries
        let divs = grid.theta_divisions(r);
        for theta in 0..divs {
            let neighbors = grid.neighbors(r, theta);
            total_count += 1;
            
            if neighbors.r_minus.len() == 1 && neighbors.r_plus.len() == 1 {
                one_to_one_count += 1;
            }
        }
    }
    
    let ratio = one_to_one_count as f32 / total_count as f32;
    assert!(ratio > 0.99, "Only {}% of voxels have 1:1 radial mapping", ratio * 100.0);
}
```

### Test 2.4: Radial Neighbor Overlap Correctness
```rust
#[test]
fn test_radial_neighbor_overlap() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    
    // For any voxel, its radial neighbors' angular ranges should overlap its own range
    for r in 1..255u8 {
        let theta = grid.theta_divisions(r) / 2;  // Middle theta
        let (my_start, my_end) = grid.angular_range(r, theta);
        
        let neighbors = grid.neighbors(r, theta);
        
        // Check inner neighbors overlap
        for &(nr, nt) in &neighbors.r_minus {
            let (n_start, n_end) = grid.angular_range(nr, nt);
            assert!(ranges_overlap(my_start, my_end, n_start, n_end),
                "Inner neighbor ({}, {}) doesn't overlap ({}, {})", nr, nt, r, theta);
        }
        
        // Check outer neighbors overlap
        for &(nr, nt) in &neighbors.r_plus {
            let (n_start, n_end) = grid.angular_range(nr, nt);
            assert!(ranges_overlap(my_start, my_end, n_start, n_end),
                "Outer neighbor ({}, {}) doesn't overlap ({}, {})", nr, nt, r, theta);
        }
    }
}
```

### Test 2.5: Boundary Conditions
```rust
#[test]
fn test_boundary_neighbors() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    
    // At r=0 (inner boundary), r_minus should be empty
    let neighbors = grid.neighbors(0, 0);
    assert!(neighbors.r_minus.is_empty());
    assert!(!neighbors.r_plus.is_empty());
    
    // At r=255 (outer boundary), r_plus should be empty
    let neighbors = grid.neighbors(255, 0);
    assert!(!neighbors.r_minus.is_empty());
    assert!(neighbors.r_plus.is_empty());
}
```

### Test 2.6: Neighbor Symmetry
```rust
#[test]
fn test_neighbor_symmetry() {
    let grid = PolarMjGrid::new(256, 256, 1.0);
    
    // If B is a neighbor of A, then A should be a neighbor of B
    for r in 1..255u8 {
        let theta = grid.theta_divisions(r) / 2;
        let neighbors = grid.neighbors(r, theta);
        
        // Check each outer neighbor lists us as inner neighbor
        for &(nr, nt) in &neighbors.r_plus {
            let reverse_neighbors = grid.neighbors(nr, nt);
            assert!(reverse_neighbors.r_minus.contains(&(r, theta)),
                "Neighbor symmetry violated: ({},{}) -> ({},{}) but not reverse", 
                r, theta, nr, nt);
        }
    }
}
```

---

## Level 3: Symmetry Tests

**Goal**: Verify the 4 polar symmetries transform patterns correctly.

### Test 3.1: Identity Transform
```rust
#[test]
fn test_identity_symmetry() {
    use PolarSymmetry::*;
    
    // Identity should not change anything
    assert_eq!(Identity.transform(1, 2), (1, 2));
    assert_eq!(Identity.transform(-1, -2), (-1, -2));
    assert_eq!(Identity.transform(0, 0), (0, 0));
}
```

### Test 3.2: Theta Flip Transform
```rust
#[test]
fn test_theta_flip_symmetry() {
    use PolarSymmetry::*;
    
    // ThetaFlip: (dr, dθ) → (dr, -dθ)
    assert_eq!(ThetaFlip.transform(1, 2), (1, -2));
    assert_eq!(ThetaFlip.transform(-1, 3), (-1, -3));
    assert_eq!(ThetaFlip.transform(0, 0), (0, 0));
    
    // Double application should return to original
    let (dr, dt) = ThetaFlip.transform(1, 2);
    assert_eq!(ThetaFlip.transform(dr, dt), (1, 2));
}
```

### Test 3.3: R Flip Transform
```rust
#[test]
fn test_r_flip_symmetry() {
    use PolarSymmetry::*;
    
    // RFlip: (dr, dθ) → (-dr, dθ)
    assert_eq!(RFlip.transform(1, 2), (-1, 2));
    assert_eq!(RFlip.transform(-1, 3), (1, 3));
    
    // Double application should return to original
    let (dr, dt) = RFlip.transform(1, 2);
    assert_eq!(RFlip.transform(dr, dt), (1, 2));
}
```

### Test 3.4: Both Flip Transform
```rust
#[test]
fn test_both_flip_symmetry() {
    use PolarSymmetry::*;
    
    // BothFlip: (dr, dθ) → (-dr, -dθ)
    assert_eq!(BothFlip.transform(1, 2), (-1, -2));
    assert_eq!(BothFlip.transform(-1, -3), (1, 3));
    
    // Should equal ThetaFlip composed with RFlip
    for dr in [-1i8, 0, 1] {
        for dt in [-2i8, 0, 2] {
            let both = BothFlip.transform(dr, dt);
            let composed = RFlip.transform(ThetaFlip.transform(dr, dt).0, 
                                           ThetaFlip.transform(dr, dt).1);
            assert_eq!(both, composed);
        }
    }
}
```

### Test 3.5: Symmetry Group Closure
```rust
#[test]
fn test_symmetry_group_closure() {
    use PolarSymmetry::*;
    
    // The 4 symmetries form a group (Klein four-group)
    // Composing any two should give another element of the group
    let symmetries = [Identity, ThetaFlip, RFlip, BothFlip];
    
    for &s1 in &symmetries {
        for &s2 in &symmetries {
            // Compose s1 then s2
            let (dr, dt) = s1.transform(1, 1);
            let composed = s2.transform(dr, dt);
            
            // Result should be achievable by a single symmetry
            let found = symmetries.iter().any(|&s| s.transform(1, 1) == composed);
            assert!(found, "Composition of {:?} and {:?} not in group", s1, s2);
        }
    }
}
```

### Test 3.6: Pattern Symmetry Variants
```rust
#[test]
fn test_pattern_symmetry_variants() {
    // A pattern with distinct neighbors should have 4 distinct variants
    let pattern = PolarPattern {
        center: 1,
        theta_minus: Some(2),
        theta_plus: Some(3),
        r_minus: Some(4),
        r_plus: Some(5),
    };
    
    let variants: Vec<_> = PolarSymmetry::all()
        .iter()
        .map(|s| pattern.transform(*s))
        .collect();
    
    // All 4 should be distinct
    for i in 0..4 {
        for j in (i+1)..4 {
            assert_ne!(variants[i], variants[j], 
                "Variants {} and {} are identical", i, j);
        }
    }
}
```

### Test 3.7: Symmetric Pattern Has Fewer Variants
```rust
#[test]
fn test_symmetric_pattern_fewer_variants() {
    // A pattern symmetric under theta flip should have only 2 unique variants
    let pattern = PolarPattern {
        center: 1,
        theta_minus: Some(2),
        theta_plus: Some(2),  // Same as theta_minus!
        r_minus: Some(3),
        r_plus: Some(4),
    };
    
    let variants: HashSet<_> = PolarSymmetry::all()
        .iter()
        .map(|s| pattern.transform(*s))
        .collect();
    
    assert_eq!(variants.len(), 2, "Expected 2 unique variants for θ-symmetric pattern");
}
```

---

## Level 4: Single-Step Rule Tests

**Goal**: Verify individual rules match and apply correctly.

### Test 4.1: Simple Rule Matching
```rust
#[test]
fn test_simple_rule_matching() {
    let mut grid = PolarMjGrid::new(256, 256, 1.0);
    
    // Set up a pattern: center=0, all neighbors=0 except r_plus=1
    let r = 100u8;
    let theta = 500u16;
    let neighbors = grid.neighbors(r, theta);
    
    // Set r_plus neighbor to 1
    for &(nr, nt) in &neighbors.r_plus {
        grid.set(nr, nt, 1);
    }
    
    // Pattern: center=0, r_plus=1, others=wildcard
    let pattern = PolarPattern {
        center: 0,
        theta_minus: None,
        theta_plus: None,
        r_minus: None,
        r_plus: Some(1),
    };
    
    assert!(pattern.matches(&grid, r, theta));
    
    // Shouldn't match at a location without r_plus=1
    assert!(!pattern.matches(&grid, r - 10, theta));
}
```

### Test 4.2: Rule Application
```rust
#[test]
fn test_rule_application() {
    let mut grid = PolarMjGrid::new(256, 256, 1.0);
    
    // Rule: 0 -> 1 (unconditional)
    let rule = PolarRule {
        input: PolarPattern { center: 0, ..Default::default() },
        output: 1,
    };
    
    let r = 100u8;
    let theta = 500u16;
    
    assert_eq!(grid.get(r, theta), 0);
    rule.apply(&mut grid, r, theta);
    assert_eq!(grid.get(r, theta), 1);
}
```

### Test 4.3: Conditional Rule
```rust
#[test]
fn test_conditional_rule() {
    let mut grid = PolarMjGrid::new(256, 256, 1.0);
    
    // Rule: 0 with r_plus=1 -> 2
    let rule = PolarRule {
        input: PolarPattern { 
            center: 0, 
            r_plus: Some(1),
            ..Default::default() 
        },
        output: 2,
    };
    
    let r = 100u8;
    let theta = 500u16;
    let neighbors = grid.neighbors(r, theta);
    
    // Without r_plus=1, rule shouldn't apply
    assert!(!rule.matches(&grid, r, theta));
    
    // Set r_plus neighbor to 1
    for &(nr, nt) in &neighbors.r_plus {
        grid.set(nr, nt, 1);
    }
    
    // Now it should match and apply
    assert!(rule.matches(&grid, r, theta));
    rule.apply(&mut grid, r, theta);
    assert_eq!(grid.get(r, theta), 2);
}
```

### Test 4.4: Rule With All Symmetries
```rust
#[test]
fn test_rule_with_symmetries() {
    let mut grid = PolarMjGrid::new(256, 256, 1.0);
    
    // Asymmetric rule: 0 with theta_plus=1 -> 2
    let base_rule = PolarRule {
        input: PolarPattern {
            center: 0,
            theta_plus: Some(1),
            ..Default::default()
        },
        output: 2,
    };
    
    let rules = base_rule.with_all_symmetries();
    assert_eq!(rules.len(), 4);
    
    // Test that theta_minus=1 matches the theta-flipped variant
    let r = 100u8;
    let theta = 500u16;
    let neighbors = grid.neighbors(r, theta);
    
    grid.set(neighbors.theta_minus.0, neighbors.theta_minus.1, 1);
    
    // Base rule shouldn't match (theta_plus isn't 1)
    assert!(!base_rule.matches(&grid, r, theta));
    
    // But one of the symmetry variants should
    let any_matches = rules.iter().any(|rule| rule.matches(&grid, r, theta));
    assert!(any_matches, "No symmetry variant matched");
}
```

### Test 4.5: Multiple Rules Priority
```rust
#[test]
fn test_multiple_rules_priority() {
    let mut grid = PolarMjGrid::new(256, 256, 1.0);
    
    // Two rules that could both match
    let rule1 = PolarRule {
        input: PolarPattern { center: 0, ..Default::default() },
        output: 1,
    };
    let rule2 = PolarRule {
        input: PolarPattern { center: 0, ..Default::default() },
        output: 2,
    };
    
    // First matching rule should apply (order matters)
    let rules = vec![rule1, rule2];
    let r = 100u8;
    let theta = 500u16;
    
    for rule in &rules {
        if rule.matches(&grid, r, theta) {
            rule.apply(&mut grid, r, theta);
            break;
        }
    }
    
    assert_eq!(grid.get(r, theta), 1);  // First rule's output
}
```

---

## Level 5: Multi-Step Model Tests

**Goal**: Verify complete models produce expected results over multiple steps.

### Test 5.1: Fill Model (Flood Fill)
```rust
#[test]
fn test_fill_model() {
    let mut grid = PolarMjGrid::new(256, 64, 1.0);  // Smaller for speed
    
    // Seed: single cell set to 1
    grid.set(32, 0, 1);
    
    // Rule: 0 adjacent to 1 -> 1 (spread)
    let rule = PolarRule {
        input: PolarPattern {
            center: 0,
            theta_minus: Some(1),
            ..Default::default()
        },
        output: 1,
    };
    let rules = rule.with_all_symmetries();  // Spread in all directions
    
    // Run until no changes
    let mut changed = true;
    let mut steps = 0;
    while changed && steps < 10000 {
        changed = false;
        for r in 0..64u8 {
            let divs = grid.theta_divisions(r);
            for theta in 0..divs {
                for rule in &rules {
                    if rule.matches(&grid, r, theta) {
                        rule.apply(&mut grid, r, theta);
                        changed = true;
                    }
                }
            }
        }
        steps += 1;
    }
    
    // All cells should be 1
    for r in 0..64u8 {
        let divs = grid.theta_divisions(r);
        for theta in 0..divs {
            assert_eq!(grid.get(r, theta), 1, "Cell ({}, {}) not filled", r, theta);
        }
    }
}
```

### Test 5.2: Ring Growth (Radial Spread)
```rust
#[test]
fn test_ring_growth() {
    let mut grid = PolarMjGrid::new(256, 64, 1.0);
    
    // Seed: entire inner ring set to 1
    let inner_divs = grid.theta_divisions(0);
    for theta in 0..inner_divs {
        grid.set(0, theta, 1);
    }
    
    // Rule: 0 with r_minus=1 -> 1 (grow outward)
    let rule = PolarRule {
        input: PolarPattern {
            center: 0,
            r_minus: Some(1),
            ..Default::default()
        },
        output: 1,
    };
    
    // Run 63 steps (should fill all 64 rings)
    for _ in 0..63 {
        let mut to_set = vec![];
        for r in 0..64u8 {
            let divs = grid.theta_divisions(r);
            for theta in 0..divs {
                if rule.matches(&grid, r, theta) {
                    to_set.push((r, theta));
                }
            }
        }
        for (r, theta) in to_set {
            grid.set(r, theta, 1);
        }
    }
    
    // All cells should be 1
    for r in 0..64u8 {
        let divs = grid.theta_divisions(r);
        for theta in 0..divs {
            assert_eq!(grid.get(r, theta), 1, "Cell ({}, {}) not filled", r, theta);
        }
    }
}
```

### Test 5.3: Wave Pattern (Periodic)
```rust
#[test]
fn test_wave_pattern() {
    let mut grid = PolarMjGrid::new(256, 64, 1.0);
    
    // Create alternating rings: 1, 0, 1, 0, ...
    for r in 0..64u8 {
        let value = (r % 2) as u8;
        let divs = grid.theta_divisions(r);
        for theta in 0..divs {
            grid.set(r, theta, value);
        }
    }
    
    // Verify pattern
    for r in 0..64u8 {
        let expected = (r % 2) as u8;
        let divs = grid.theta_divisions(r);
        for theta in 0..divs {
            assert_eq!(grid.get(r, theta), expected);
        }
    }
}
```

### Test 5.4: Maze Generation
```rust
#[test]
fn test_maze_generation() {
    let mut grid = PolarMjGrid::new(256, 32, 1.0);
    let mut rng = StdRng::seed_from_u64(42);  // Deterministic
    
    // Initialize with walls (1) everywhere
    for r in 0..32u8 {
        let divs = grid.theta_divisions(r);
        for theta in 0..divs {
            grid.set(r, theta, 1);
        }
    }
    
    // Carve maze using randomized rules
    // ... (maze algorithm)
    
    // Verify maze properties:
    // 1. Path exists from inner to outer ring
    // 2. No isolated regions
    // 3. Walls form connected structure
    
    let path_exists = find_path(&grid, (0, 0), (31, 0));
    assert!(path_exists, "No path through maze");
}
```

### Test 5.5: Deterministic Output
```rust
#[test]
fn test_deterministic_output() {
    // Same seed should produce identical results
    let result1 = run_model_with_seed(42);
    let result2 = run_model_with_seed(42);
    
    assert_eq!(result1.checksum(), result2.checksum());
    
    // Different seed should produce different results
    let result3 = run_model_with_seed(43);
    assert_ne!(result1.checksum(), result3.checksum());
}
```

### Test 5.6: Reference Output Comparison
```rust
#[test]
fn test_reference_output() {
    // Run a known model and compare against saved reference
    let grid = run_model("test_models/ring_growth.xml", seed: 42);
    
    let expected_checksum = 0x1234567890ABCDEF;  // Pre-computed
    assert_eq!(grid.checksum(), expected_checksum);
}
```

---

## Level 6: Rendering Output Tests

**Goal**: Verify the grid can be converted to visual output correctly.

### Test 6.1: Polar to Cartesian Conversion
```rust
#[test]
fn test_polar_to_cartesian() {
    let grid = PolarMjGrid::new(256, 64, 1.0);
    
    // Convert to Cartesian coordinates
    let (x, y) = polar_to_cartesian(r: 50, theta: 0, r_min: 256);
    
    // At theta=0, should be along positive x-axis
    let r_actual = 256 + 50;
    assert!((x - r_actual as f32).abs() < 0.01);
    assert!(y.abs() < 0.01);
    
    // At theta=quarter_circle, should be along positive y-axis
    let divs = grid.theta_divisions(50);
    let (x, y) = polar_to_cartesian(50, divs / 4, 256);
    assert!(x.abs() < 1.0);  // Near zero
    assert!((y - r_actual as f32).abs() < 1.0);
}
```

### Test 6.2: Image Generation
```rust
#[test]
fn test_image_generation() {
    let mut grid = PolarMjGrid::new(256, 64, 1.0);
    
    // Create a simple pattern (checkerboard in theta)
    for r in 0..64u8 {
        let divs = grid.theta_divisions(r);
        for theta in 0..divs {
            grid.set(r, theta, (theta % 2) as u8);
        }
    }
    
    // Render to image
    let image = grid.render_to_image(512, 512);
    
    // Save and compare against reference
    image.save("test_output/polar_checkerboard.png").unwrap();
    
    let reference = image::open("test_references/polar_checkerboard.png").unwrap();
    let diff = image_diff(&image, &reference);
    assert!(diff < 0.01, "Image differs from reference by {}%", diff * 100.0);
}
```

### Test 6.3: Color Mapping
```rust
#[test]
fn test_color_mapping() {
    let palette = Palette::new(vec![
        (0, Color::BLACK),
        (1, Color::WHITE),
        (2, Color::RED),
    ]);
    
    assert_eq!(palette.get(0), Color::BLACK);
    assert_eq!(palette.get(1), Color::WHITE);
    assert_eq!(palette.get(2), Color::RED);
}
```

### Test 6.4: Full Pipeline Test
```rust
#[test]
fn test_full_pipeline() {
    // Load model -> Run -> Render -> Compare
    let grid = run_model("test_models/spiral.xml", seed: 42);
    let image = grid.render_to_image(512, 512);
    
    // Verify image properties
    assert_eq!(image.width(), 512);
    assert_eq!(image.height(), 512);
    
    // Check specific pixels at known locations
    // Center should be at (256, 256), corresponding to r=0
    let center_color = image.get_pixel(256, 256);
    assert_eq!(center_color, expected_center_color);
}
```

---

## Test Models

### Model Files for Testing

| Model | Purpose | Expected Behavior |
|-------|---------|-------------------|
| `empty.xml` | Baseline | All cells remain 0 |
| `fill_all.xml` | Flood fill | All cells become 1 |
| `ring_grow.xml` | Radial growth | Concentric rings |
| `theta_grow.xml` | Angular growth | Spiral pattern |
| `checkerboard.xml` | Alternating pattern | Checkerboard in (r, θ) |
| `maze.xml` | Maze generation | Connected path exists |
| `wave.xml` | Wave propagation | Circular waves |

### Reference Outputs

Each model should have:
1. Expected checksum for deterministic verification
2. Reference image for visual verification
3. Expected cell counts per value

---

## Continuous Integration

### Test Execution Order

```yaml
test:
  stage: test
  script:
    - cargo test polar_2d_tests::level_0 --no-fail-fast
    - cargo test polar_2d_tests::level_1 --no-fail-fast
    - cargo test polar_2d_tests::level_2 --no-fail-fast
    - cargo test polar_2d_tests::level_3 --no-fail-fast
    - cargo test polar_2d_tests::level_4 --no-fail-fast
    - cargo test polar_2d_tests::level_5 --no-fail-fast
    - cargo test polar_2d_tests::level_6 --no-fail-fast
```

### Failure Handling

- Level 0-2 failures: Block all subsequent levels (foundational)
- Level 3-4 failures: Block level 5-6 (rule system broken)
- Level 5-6 failures: Report but don't block (may be model-specific)

---

## Summary

| Level | Tests | Purpose |
|-------|-------|---------|
| 0 | 4 | Data structure basics |
| 1 | 4 | Coordinate math |
| 2 | 6 | Neighbor relationships |
| 3 | 7 | Symmetry transforms |
| 4 | 5 | Single-step rules |
| 5 | 6 | Multi-step models |
| 6 | 4 | Rendering output |

**Total: 36 tests** covering the complete 2D polar Markov Jr pipeline from data structures to rendered output.
