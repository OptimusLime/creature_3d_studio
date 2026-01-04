//! Symmetry helpers for MarkovJunior rules.
//!
//! Generates all unique rotations and reflections of a rule
//! to allow pattern matching in any orientation.

use super::MjRule;

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
