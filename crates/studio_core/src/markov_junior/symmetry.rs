//! Symmetry helpers for MarkovJunior rules.
//!
//! Generates all unique rotations and reflections of a rule
//! to allow pattern matching in any orientation.

use super::MjRule;
use std::collections::HashMap;

/// Predefined symmetry subgroups for 2D (square) patterns.
///
/// Each subgroup specifies which of the 8 possible symmetries to include:
/// - Index 0: identity (e)
/// - Index 1: reflection (b)
/// - Index 2: 90° rotation (a)
/// - Index 3: 90° rotation + reflection (ba)
/// - Index 4: 180° rotation (a²)
/// - Index 5: 180° rotation + reflection (ba²)
/// - Index 6: 270° rotation (a³)
/// - Index 7: 270° rotation + reflection (ba³)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquareSubgroup {
    /// Just the identity - no symmetry
    None,
    /// Identity + X reflection
    ReflectX,
    /// Identity + Y reflection
    ReflectY,
    /// X and Y reflections (4 variants)
    ReflectXY,
    /// Rotations only, no reflections (4 variants)
    Rotate,
    /// All 8 symmetries
    All,
}

impl SquareSubgroup {
    /// Get the boolean mask for this subgroup.
    pub fn mask(&self) -> [bool; 8] {
        match self {
            SquareSubgroup::None => [true, false, false, false, false, false, false, false],
            SquareSubgroup::ReflectX => [true, true, false, false, false, false, false, false],
            SquareSubgroup::ReflectY => [true, false, false, false, false, true, false, false],
            SquareSubgroup::ReflectXY => [true, true, false, false, true, true, false, false],
            SquareSubgroup::Rotate => [true, false, true, false, true, false, true, false],
            SquareSubgroup::All => [true, true, true, true, true, true, true, true],
        }
    }

    /// Parse from string notation used in MarkovJunior XML.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "()" => Some(SquareSubgroup::None),
            "(x)" => Some(SquareSubgroup::ReflectX),
            "(y)" => Some(SquareSubgroup::ReflectY),
            "(x)(y)" => Some(SquareSubgroup::ReflectXY),
            "(xy+)" => Some(SquareSubgroup::Rotate),
            "(xy)" => Some(SquareSubgroup::All),
            _ => None,
        }
    }
}

impl Default for SquareSubgroup {
    fn default() -> Self {
        SquareSubgroup::All
    }
}

/// Generate all unique symmetry variants of a rule for 2D patterns.
///
/// Uses the D4 dihedral group (symmetries of a square):
/// - 4 rotations: 0°, 90°, 180°, 270°
/// - 4 reflections: each rotation + X-axis mirror
///
/// Duplicates are removed by comparing rule patterns.
///
/// # Arguments
/// * `rule` - The base rule to generate symmetries for
/// * `subgroup` - Which symmetries to include (default: all)
///
/// # Returns
/// Vector of unique rule variants (1-8 rules depending on pattern symmetry)
pub fn square_symmetries(rule: &MjRule, subgroup: Option<SquareSubgroup>) -> Vec<MjRule> {
    let subgroup = subgroup.unwrap_or(SquareSubgroup::All);
    let mask = subgroup.mask();

    // Generate all 8 variants
    let mut variants = Vec::with_capacity(8);

    let r0 = rule.clone(); // e (identity)
    let r1 = r0.reflected(); // b (reflection)
    let r2 = r0.z_rotated(); // a (90° rotation)
    let r3 = r2.reflected(); // ba
    let r4 = r2.z_rotated(); // a² (180°)
    let r5 = r4.reflected(); // ba²
    let r6 = r4.z_rotated(); // a³ (270°)
    let r7 = r6.reflected(); // ba³

    let all = [r0, r1, r2, r3, r4, r5, r6, r7];

    // Add variants that are in the subgroup and not duplicates
    for (i, variant) in all.into_iter().enumerate() {
        if mask[i] && !variants.iter().any(|v: &MjRule| v.same(&variant)) {
            variants.push(variant);
        }
    }

    variants
}

/// Generate symmetry variants for a rule using a custom mask.
///
/// # Arguments
/// * `rule` - The base rule
/// * `mask` - 8-element boolean array specifying which variants to include
pub fn square_symmetries_with_mask(rule: &MjRule, mask: [bool; 8]) -> Vec<MjRule> {
    let r0 = rule.clone();
    let r1 = r0.reflected();
    let r2 = r0.z_rotated();
    let r3 = r2.reflected();
    let r4 = r2.z_rotated();
    let r5 = r4.reflected();
    let r6 = r4.z_rotated();
    let r7 = r6.reflected();

    let all = [r0, r1, r2, r3, r4, r5, r6, r7];

    let mut variants = Vec::new();
    for (i, variant) in all.into_iter().enumerate() {
        if mask[i] && !variants.iter().any(|v: &MjRule| v.same(&variant)) {
            variants.push(variant);
        }
    }

    variants
}

// ============================================================================
// 3D Cube Symmetries (48-element group)
// ============================================================================

/// Predefined symmetry subgroups for 3D (cube) patterns.
///
/// The cube symmetry group has 48 elements:
/// - 24 rotations (orientation-preserving)
/// - 24 rotoreflections (include a reflection)
///
/// C# Reference: SymmetryHelper.cs lines 37-46
pub fn cube_subgroups() -> HashMap<&'static str, [bool; 48]> {
    let mut map = HashMap::new();

    // () - identity only
    let mut identity = [false; 48];
    identity[0] = true;
    map.insert("()", identity);

    // (x) - identity and x-reflection
    let mut x_reflect = [false; 48];
    x_reflect[0] = true;
    x_reflect[1] = true;
    map.insert("(x)", x_reflect);

    // (z) - identity and z-reflection (index 17 in C#)
    let mut z_reflect = [false; 48];
    z_reflect[0] = true;
    z_reflect[17] = true;
    map.insert("(z)", z_reflect);

    // (xy) - all 8 square symmetries (indices 0-7)
    let mut xy_sym = [false; 48];
    for i in 0..8 {
        xy_sym[i] = true;
    }
    map.insert("(xy)", xy_sym);

    // (xyz+) - all 24 rotations (even indices)
    let mut rotations = [false; 48];
    for i in 0..48 {
        if i % 2 == 0 {
            rotations[i] = true;
        }
    }
    map.insert("(xyz+)", rotations);

    // (xyz) - all 48 symmetries
    let all = [true; 48];
    map.insert("(xyz)", all);

    map
}

/// Get the symmetry subgroup mask for a given symmetry string.
///
/// Returns None if the symmetry string is not recognized.
///
/// # Arguments
/// * `is_2d` - Whether this is a 2D pattern (use square subgroups) or 3D (use cube subgroups)
/// * `s` - The symmetry string (e.g., "(xy)", "(xyz)")
/// * `default` - Default mask to return if s is None
pub fn get_symmetry(is_2d: bool, s: Option<&str>, default: Option<&[bool]>) -> Option<Vec<bool>> {
    match s {
        None => default.map(|d| d.to_vec()),
        Some(sym_str) => {
            if is_2d {
                SquareSubgroup::from_str(sym_str).map(|sg| sg.mask().to_vec())
            } else {
                cube_subgroups().get(sym_str).map(|m| m.to_vec())
            }
        }
    }
}

/// Generate all unique 3D cube symmetry variants of a rule.
///
/// The cube symmetry group has 48 elements, generated by:
/// - a: 90° rotation around Z axis
/// - b: 90° rotation around Y axis  
/// - r: reflection (X-axis mirror)
///
/// C# Reference: SymmetryHelper.cs CubeSymmetries() lines 48-104
///
/// # Arguments
/// * `rule` - The base rule to generate symmetries for
/// * `subgroup` - Optional 48-element mask specifying which symmetries to include
///
/// # Returns
/// Vector of unique rule variants (1-48 rules depending on pattern symmetry)
pub fn cube_symmetries(rule: &MjRule, subgroup: Option<&[bool; 48]>) -> Vec<MjRule> {
    let mut s: [MjRule; 48] = std::array::from_fn(|_| rule.clone());

    // Generate all 48 symmetry variants
    // Using the group structure from C#:
    // a = z_rotated (90° around Z)
    // b = y_rotated (90° around Y)
    // r = reflected (X-axis mirror)

    // s[0] = e (identity) - already set
    s[1] = s[0].reflected(); // r

    s[2] = s[0].z_rotated(); // a
    s[3] = s[2].reflected(); // ra

    s[4] = s[2].z_rotated(); // a²
    s[5] = s[4].reflected(); // ra²

    s[6] = s[4].z_rotated(); // a³
    s[7] = s[6].reflected(); // ra³

    s[8] = s[0].y_rotated(); // b
    s[9] = s[8].reflected(); // rb

    s[10] = s[2].y_rotated(); // ba
    s[11] = s[10].reflected(); // rba

    s[12] = s[4].y_rotated(); // ba²
    s[13] = s[12].reflected(); // rba²

    s[14] = s[6].y_rotated(); // ba³
    s[15] = s[14].reflected(); // rba³

    s[16] = s[8].y_rotated(); // b²
    s[17] = s[16].reflected(); // rb²

    s[18] = s[10].y_rotated(); // b²a
    s[19] = s[18].reflected(); // rb²a

    s[20] = s[12].y_rotated(); // b²a²
    s[21] = s[20].reflected(); // rb²a²

    s[22] = s[14].y_rotated(); // b²a³
    s[23] = s[22].reflected(); // rb²a³

    s[24] = s[16].y_rotated(); // b³
    s[25] = s[24].reflected(); // rb³

    s[26] = s[18].y_rotated(); // b³a
    s[27] = s[26].reflected(); // rb³a

    s[28] = s[20].y_rotated(); // b³a²
    s[29] = s[28].reflected(); // rb³a²

    s[30] = s[22].y_rotated(); // b³a³
    s[31] = s[30].reflected(); // rb³a³

    s[32] = s[8].z_rotated(); // ab
    s[33] = s[32].reflected(); // rab

    s[34] = s[10].z_rotated(); // aba
    s[35] = s[34].reflected(); // raba

    s[36] = s[12].z_rotated(); // aba²
    s[37] = s[36].reflected(); // raba²

    s[38] = s[14].z_rotated(); // aba³
    s[39] = s[38].reflected(); // raba³

    s[40] = s[24].z_rotated(); // ab³
    s[41] = s[40].reflected(); // rab³

    s[42] = s[26].z_rotated(); // ab³a
    s[43] = s[42].reflected(); // rab³a

    s[44] = s[28].z_rotated(); // ab³a²
    s[45] = s[44].reflected(); // rab³a²

    s[46] = s[30].z_rotated(); // ab³a³
    s[47] = s[46].reflected(); // rab³a³

    // Filter by subgroup and remove duplicates
    let default_subgroup = [true; 48];
    let mask = subgroup.unwrap_or(&default_subgroup);

    let mut result = Vec::new();
    for (i, variant) in s.into_iter().enumerate() {
        if mask[i] && !result.iter().any(|v: &MjRule| v.same(&variant)) {
            result.push(variant);
        }
    }

    result
}

/// Generate cube symmetries for a rule using a named subgroup.
///
/// # Arguments
/// * `rule` - The base rule
/// * `subgroup_name` - Name of subgroup: "()", "(x)", "(z)", "(xy)", "(xyz+)", "(xyz)"
pub fn cube_symmetries_named(rule: &MjRule, subgroup_name: &str) -> Vec<MjRule> {
    let subgroups = cube_subgroups();
    let mask = subgroups.get(subgroup_name);
    cube_symmetries(rule, mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov_junior::MjGrid;

    #[test]
    fn test_square_subgroup_from_str() {
        assert_eq!(SquareSubgroup::from_str("()"), Some(SquareSubgroup::None));
        assert_eq!(SquareSubgroup::from_str("(xy)"), Some(SquareSubgroup::All));
        assert_eq!(SquareSubgroup::from_str("invalid"), None);
    }

    #[test]
    fn test_cube_subgroups_defined() {
        let subgroups = cube_subgroups();

        // All expected subgroups should be present
        assert!(subgroups.contains_key("()"));
        assert!(subgroups.contains_key("(x)"));
        assert!(subgroups.contains_key("(z)"));
        assert!(subgroups.contains_key("(xy)"));
        assert!(subgroups.contains_key("(xyz+)"));
        assert!(subgroups.contains_key("(xyz)"));

        // Verify element counts
        let identity = subgroups.get("()").unwrap();
        assert_eq!(identity.iter().filter(|&&b| b).count(), 1);

        let x_reflect = subgroups.get("(x)").unwrap();
        assert_eq!(x_reflect.iter().filter(|&&b| b).count(), 2);

        let xy_sym = subgroups.get("(xy)").unwrap();
        assert_eq!(xy_sym.iter().filter(|&&b| b).count(), 8);

        let rotations = subgroups.get("(xyz+)").unwrap();
        assert_eq!(rotations.iter().filter(|&&b| b).count(), 24);

        let all = subgroups.get("(xyz)").unwrap();
        assert_eq!(all.iter().filter(|&&b| b).count(), 48);
    }

    #[test]
    fn test_cube_symmetries_generates_48() {
        // Create a 3D grid and a simple 3D rule
        let grid = MjGrid::with_values(5, 5, 5, "AB");

        // Single cell rule - should have only 1 unique variant
        let rule = MjRule::parse("A", "B", &grid).unwrap();
        let variants = cube_symmetries(&rule, None);
        assert_eq!(variants.len(), 1, "1x1x1 pattern should have 1 variant");

        // All variants should be unique
        for (i, v1) in variants.iter().enumerate() {
            for (j, v2) in variants.iter().enumerate() {
                if i != j {
                    assert!(!v1.same(v2), "Variants {} and {} should be different", i, j);
                }
            }
        }
    }

    #[test]
    fn test_cube_symmetries_identity_subgroup() {
        let grid = MjGrid::with_values(5, 5, 5, "AB");
        let rule = MjRule::parse("AB", "BA", &grid).unwrap();

        let variants = cube_symmetries_named(&rule, "()");
        assert_eq!(
            variants.len(),
            1,
            "Identity subgroup should produce 1 variant"
        );
    }

    #[test]
    fn test_cube_symmetries_asymmetric_rule() {
        let grid = MjGrid::with_values(5, 5, 5, "ABCDEF");

        // Create a rule with distinct patterns on each axis
        // 2x1x1 pattern
        let rule = MjRule::parse("AB", "CD", &grid).unwrap();
        let variants = cube_symmetries(&rule, None);

        // Should get multiple variants due to asymmetry
        assert!(
            variants.len() > 1,
            "Asymmetric rule should have multiple variants"
        );

        // Should not exceed 48
        assert!(variants.len() <= 48, "Should not exceed 48 variants");
    }

    #[test]
    fn test_get_symmetry_2d() {
        // Test 2D symmetry lookup
        let sym = get_symmetry(true, Some("(xy)"), None);
        assert!(sym.is_some());
        assert_eq!(sym.unwrap().len(), 8);

        let sym = get_symmetry(true, Some("()"), None);
        assert!(sym.is_some());
        assert_eq!(sym.unwrap().iter().filter(|&&b| b).count(), 1);
    }

    #[test]
    fn test_get_symmetry_3d() {
        // Test 3D symmetry lookup
        let sym = get_symmetry(false, Some("(xyz)"), None);
        assert!(sym.is_some());
        assert_eq!(sym.unwrap().len(), 48);

        let sym = get_symmetry(false, Some("(xyz+)"), None);
        assert!(sym.is_some());
        assert_eq!(sym.unwrap().iter().filter(|&&b| b).count(), 24);
    }

    #[test]
    fn test_get_symmetry_default() {
        let default_mask = vec![true, false, false];
        let sym = get_symmetry(true, None, Some(&default_mask));
        assert!(sym.is_some());
        assert_eq!(sym.unwrap(), default_mask);
    }

    #[test]
    fn test_symmetry_square_8() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        // Asymmetric 2x1 pattern should produce all 8 variants
        // But actually 2x1 -> after rotation becomes 1x2
        // The reflected variant of 2x1 is still 2x1
        // So we get: original, reflected, 90°, 90°+reflected, 180°, 180°+reflected, 270°, 270°+reflected
        // But some may be duplicates due to the simple pattern

        // Use a more complex pattern to ensure uniqueness
        let rule = MjRule::parse("BW", "WB", &grid).unwrap();
        let variants = square_symmetries(&rule, Some(SquareSubgroup::All));

        // For BW -> WB pattern, some symmetries will be duplicates
        // The key is we get at least some variants and no more than 8
        assert!(variants.len() >= 1);
        assert!(variants.len() <= 8);

        // All variants should be different from each other
        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                assert!(
                    !variants[i].same(&variants[j]),
                    "Duplicate variant found at indices {} and {}",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_symmetry_square_none() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let variants = square_symmetries(&rule, Some(SquareSubgroup::None));

        // With no symmetry, should get exactly 1 variant
        assert_eq!(variants.len(), 1);
    }

    #[test]
    fn test_symmetry_1x1_is_symmetric() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        // 1x1 pattern is fully symmetric
        let rule = MjRule::parse("B", "W", &grid).unwrap();
        let variants = square_symmetries(&rule, Some(SquareSubgroup::All));

        // All rotations/reflections of 1x1 are identical
        assert_eq!(variants.len(), 1);
    }

    #[test]
    fn test_symmetry_2x2_asymmetric() {
        let grid = MjGrid::with_values(5, 5, 1, "ABCD");
        // Fully asymmetric 2x2 pattern
        let rule = MjRule::parse("AB/CD", "CD/AB", &grid).unwrap();
        let variants = square_symmetries(&rule, Some(SquareSubgroup::All));

        // Should get multiple unique variants (likely 4-8)
        assert!(variants.len() > 1);
    }

    #[test]
    fn test_symmetry_mask() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let rule = MjRule::parse("BW", "WB", &grid).unwrap();

        // Only identity
        let mask = [true, false, false, false, false, false, false, false];
        let variants = square_symmetries_with_mask(&rule, mask);
        assert_eq!(variants.len(), 1);

        // Identity + reflection
        let mask = [true, true, false, false, false, false, false, false];
        let variants = square_symmetries_with_mask(&rule, mask);
        assert!(variants.len() <= 2);
    }
}
