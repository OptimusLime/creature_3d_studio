//! MjRule - Rewrite rules for MarkovJunior.
//!
//! A rule defines a pattern to match (input) and a pattern to write (output).
//! Patterns are parsed from strings like "RB/WW" where:
//! - Characters are value symbols
//! - `/` separates Y rows
//! - ` ` (space) separates Z layers
//! - `*` is a wildcard matching any value

use super::MjGrid;
use std::fmt;

/// A rewrite rule with input pattern (waves) and output pattern (values).
#[derive(Clone)]
pub struct MjRule {
    /// Input pattern as wave bitmasks (allows wildcards)
    pub input: Vec<u32>,
    /// Output pattern as byte values (0xff = don't change)
    pub output: Vec<u8>,
    /// Input pattern dimensions
    pub imx: usize,
    pub imy: usize,
    pub imz: usize,
    /// Output pattern dimensions
    pub omx: usize,
    pub omy: usize,
    pub omz: usize,
    /// Probability weight for this rule
    pub p: f64,
    /// Number of colors (for symmetry operations)
    pub c: u8,
}

impl fmt::Debug for MjRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MjRule")
            .field(
                "input",
                &format!("[{}x{}x{}]", self.imx, self.imy, self.imz),
            )
            .field(
                "output",
                &format!("[{}x{}x{}]", self.omx, self.omy, self.omz),
            )
            .field("p", &self.p)
            .finish()
    }
}

/// Error type for rule parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleParseError {
    /// Pattern is empty
    EmptyPattern,
    /// Pattern has inconsistent row lengths
    NonRectangularPattern,
    /// Unknown character in pattern
    UnknownCharacter(char),
    /// Input and output dimensions don't match
    DimensionMismatch,
}

impl fmt::Display for RuleParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleParseError::EmptyPattern => write!(f, "empty pattern"),
            RuleParseError::NonRectangularPattern => write!(f, "non-rectangular pattern"),
            RuleParseError::UnknownCharacter(c) => write!(f, "unknown character '{}'", c),
            RuleParseError::DimensionMismatch => {
                write!(f, "input and output dimensions don't match")
            }
        }
    }
}

impl std::error::Error for RuleParseError {}

impl MjRule {
    /// Parse a rule from input/output pattern strings.
    ///
    /// Pattern format:
    /// - Single characters: "B", "W"
    /// - Horizontal (X): "BW", "RGB"
    /// - 2D with rows (Y): "BW/WB" (top row / bottom row)
    /// - 3D with layers (Z): "BW/WB BB/WW" (front layer, back layer)
    ///
    /// Wildcards:
    /// - `*` in input matches any value
    /// - `*` in output means "don't change"
    pub fn parse(input_str: &str, output_str: &str, grid: &MjGrid) -> Result<Self, RuleParseError> {
        let (in_chars, imx, imy, imz) = Self::parse_pattern(input_str)?;
        let (out_chars, omx, omy, omz) = Self::parse_pattern(output_str)?;

        // For same grid input/output, dimensions must match
        if imx != omx || imy != omy || imz != omz {
            return Err(RuleParseError::DimensionMismatch);
        }

        // Convert input chars to wave bitmasks
        let mut input = Vec::with_capacity(in_chars.len());
        for ch in &in_chars {
            if let Some(&wave) = grid.waves.get(ch) {
                input.push(wave);
            } else {
                return Err(RuleParseError::UnknownCharacter(*ch));
            }
        }

        // Convert output chars to byte values
        let mut output = Vec::with_capacity(out_chars.len());
        for ch in &out_chars {
            if *ch == '*' {
                output.push(0xff); // wildcard = don't change
            } else if let Some(&value) = grid.values.get(ch) {
                output.push(value);
            } else {
                return Err(RuleParseError::UnknownCharacter(*ch));
            }
        }

        Ok(Self {
            input,
            output,
            imx,
            imy,
            imz,
            omx,
            omy,
            omz,
            p: 1.0,
            c: grid.c,
        })
    }

    /// Parse a pattern string into characters and dimensions.
    ///
    /// Returns (chars, MX, MY, MZ) where chars is in x + y*MX + z*MX*MY order.
    fn parse_pattern(s: &str) -> Result<(Vec<char>, usize, usize, usize), RuleParseError> {
        if s.is_empty() {
            return Err(RuleParseError::EmptyPattern);
        }

        // Split by space for Z layers, then by / for Y rows
        let layers: Vec<&str> = s.split(' ').collect();
        let mz = layers.len();

        let mut all_chars = Vec::new();
        let mut mx = 0;
        let mut my = 0;

        for (z, layer) in layers.iter().enumerate() {
            let rows: Vec<&str> = layer.split('/').collect();

            if z == 0 {
                my = rows.len();
            } else if rows.len() != my {
                return Err(RuleParseError::NonRectangularPattern);
            }

            // Process rows in reverse order (top row is last in Y)
            // Actually in MarkovJunior, the first row in the string is y=0
            // Looking at C# Parse: linesz = lines[MZ - 1 - z], then y iterates normally
            // The Z is reversed but Y is not. Let's keep it simple for now.
            for (y, row) in rows.iter().enumerate() {
                if z == 0 && y == 0 {
                    mx = row.chars().count();
                } else if row.chars().count() != mx {
                    return Err(RuleParseError::NonRectangularPattern);
                }

                // We need to store in x + y*MX + z*MX*MY order
                // Build up the chars array properly
                let row_chars: Vec<char> = row.chars().collect();

                // Calculate starting index for this row
                let base_idx = y * mx + z * mx * my;
                while all_chars.len() < base_idx + mx {
                    all_chars.push(' '); // placeholder
                }

                for (x, ch) in row_chars.into_iter().enumerate() {
                    let idx = x + y * mx + z * mx * my;
                    if idx < all_chars.len() {
                        all_chars[idx] = ch;
                    } else {
                        all_chars.push(ch);
                    }
                }
            }
        }

        // Compact: ensure we have exactly mx * my * mz chars
        all_chars.truncate(mx * my * mz);
        if all_chars.len() != mx * my * mz {
            return Err(RuleParseError::NonRectangularPattern);
        }

        Ok((all_chars, mx, my, mz))
    }

    /// Create a Z-rotated (around Z axis) version of this rule.
    /// This rotates the XY plane 90 degrees counter-clockwise.
    pub fn z_rotated(&self) -> Self {
        let mut new_input = vec![0u32; self.input.len()];
        let mut new_output = vec![0u8; self.output.len()];

        // For each position in new array, find source position
        // new[x + y*IMY + z*IMX*IMY] = old[IMX-1-y + x*IMX + z*IMX*IMY]
        for z in 0..self.imz {
            for y in 0..self.imx {
                // new Y goes up to old IMX
                for x in 0..self.imy {
                    // new X goes up to old IMY
                    let new_idx = x + y * self.imy + z * self.imx * self.imy;
                    let old_idx = (self.imx - 1 - y) + x * self.imx + z * self.imx * self.imy;
                    new_input[new_idx] = self.input[old_idx];
                }
            }
        }

        for z in 0..self.omz {
            for y in 0..self.omx {
                for x in 0..self.omy {
                    let new_idx = x + y * self.omy + z * self.omx * self.omy;
                    let old_idx = (self.omx - 1 - y) + x * self.omx + z * self.omx * self.omy;
                    new_output[new_idx] = self.output[old_idx];
                }
            }
        }

        Self {
            input: new_input,
            output: new_output,
            imx: self.imy,
            imy: self.imx,
            imz: self.imz,
            omx: self.omy,
            omy: self.omx,
            omz: self.omz,
            p: self.p,
            c: self.c,
        }
    }

    /// Create a reflected (X-axis mirror) version of this rule.
    pub fn reflected(&self) -> Self {
        let mut new_input = vec![0u32; self.input.len()];
        let mut new_output = vec![0u8; self.output.len()];

        // new[x + y*IMX + z*IMX*IMY] = old[IMX-1-x + y*IMX + z*IMX*IMY]
        for z in 0..self.imz {
            for y in 0..self.imy {
                for x in 0..self.imx {
                    let new_idx = x + y * self.imx + z * self.imx * self.imy;
                    let old_idx = (self.imx - 1 - x) + y * self.imx + z * self.imx * self.imy;
                    new_input[new_idx] = self.input[old_idx];
                }
            }
        }

        for z in 0..self.omz {
            for y in 0..self.omy {
                for x in 0..self.omx {
                    let new_idx = x + y * self.omx + z * self.omx * self.omy;
                    let old_idx = (self.omx - 1 - x) + y * self.omx + z * self.omx * self.omy;
                    new_output[new_idx] = self.output[old_idx];
                }
            }
        }

        Self {
            input: new_input,
            output: new_output,
            imx: self.imx,
            imy: self.imy,
            imz: self.imz,
            omx: self.omx,
            omy: self.omy,
            omz: self.omz,
            p: self.p,
            c: self.c,
        }
    }

    /// Check if two rules are the same (same dimensions and patterns).
    pub fn same(&self, other: &Self) -> bool {
        if self.imx != other.imx
            || self.imy != other.imy
            || self.imz != other.imz
            || self.omx != other.omx
            || self.omy != other.omy
            || self.omz != other.omz
        {
            return false;
        }

        self.input == other.input && self.output == other.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pattern_1d() {
        let (chars, mx, my, mz) = MjRule::parse_pattern("BW").unwrap();
        assert_eq!(chars, vec!['B', 'W']);
        assert_eq!((mx, my, mz), (2, 1, 1));
    }

    #[test]
    fn test_parse_pattern_2d() {
        let (chars, mx, my, mz) = MjRule::parse_pattern("RB/WW").unwrap();
        assert_eq!((mx, my, mz), (2, 2, 1));
        // chars[x + y*mx]: (0,0)=R, (1,0)=B, (0,1)=W, (1,1)=W
        assert_eq!(chars[0], 'R'); // (0,0)
        assert_eq!(chars[1], 'B'); // (1,0)
        assert_eq!(chars[2], 'W'); // (0,1)
        assert_eq!(chars[3], 'W'); // (1,1)
    }

    #[test]
    fn test_parse_pattern_3d() {
        let (chars, mx, my, mz) = MjRule::parse_pattern("AB CD").unwrap();
        assert_eq!((mx, my, mz), (2, 1, 2));
        // z=0: AB, z=1: CD
        assert_eq!(chars[0], 'A'); // (0,0,0)
        assert_eq!(chars[1], 'B'); // (1,0,0)
        assert_eq!(chars[2], 'C'); // (0,0,1)
        assert_eq!(chars[3], 'D'); // (1,0,1)
    }

    #[test]
    fn test_rule_parse_simple() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let rule = MjRule::parse("B", "W", &grid).unwrap();

        assert_eq!(rule.imx, 1);
        assert_eq!(rule.imy, 1);
        assert_eq!(rule.imz, 1);
        assert_eq!(rule.input.len(), 1);
        assert_eq!(rule.input[0], 1); // wave for B
        assert_eq!(rule.output[0], 1); // value for W
    }

    #[test]
    fn test_rule_parse_2d() {
        let grid = MjGrid::with_values(5, 5, 1, "RGBW");
        let rule = MjRule::parse("RB/WW", "GG/RR", &grid).unwrap();

        assert_eq!(rule.imx, 2);
        assert_eq!(rule.imy, 2);
        assert_eq!(rule.input.len(), 4);
    }

    #[test]
    fn test_rule_parse_wildcard() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let rule = MjRule::parse("*", "W", &grid).unwrap();

        // Wildcard should have wave = 0b11 (matches both B and W)
        assert_eq!(rule.input[0], 3);
    }

    #[test]
    fn test_rule_parse_output_wildcard() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let rule = MjRule::parse("BW", "W*", &grid).unwrap();

        assert_eq!(rule.output[0], 1); // W
        assert_eq!(rule.output[1], 0xff); // * = don't change
    }

    #[test]
    fn test_rule_parse_unknown_char() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let result = MjRule::parse("X", "W", &grid);
        assert!(matches!(result, Err(RuleParseError::UnknownCharacter('X'))));
    }

    #[test]
    fn test_rule_z_rotated() {
        let grid = MjGrid::with_values(5, 5, 1, "ABCD");
        // 2x1 pattern: AB
        let rule = MjRule::parse("AB", "CD", &grid).unwrap();
        assert_eq!((rule.imx, rule.imy), (2, 1));

        let rotated = rule.z_rotated();
        // After 90° rotation, 2x1 becomes 1x2
        assert_eq!((rotated.imx, rotated.imy), (1, 2));
    }

    #[test]
    fn test_rule_reflected() {
        let grid = MjGrid::with_values(5, 5, 1, "ABCD");
        let rule = MjRule::parse("AB", "CD", &grid).unwrap();

        let reflected = rule.reflected();
        // Reflection keeps dimensions but mirrors content
        assert_eq!((reflected.imx, reflected.imy), (2, 1));
        // AB reflected is BA
        assert_eq!(reflected.input[0], rule.input[1]); // B is now at position 0
        assert_eq!(reflected.input[1], rule.input[0]); // A is now at position 1
    }

    #[test]
    fn test_rule_same() {
        let grid = MjGrid::with_values(5, 5, 1, "BW");
        let rule1 = MjRule::parse("BW", "WB", &grid).unwrap();
        let rule2 = MjRule::parse("BW", "WB", &grid).unwrap();
        let rule3 = MjRule::parse("WB", "BW", &grid).unwrap();

        assert!(rule1.same(&rule2));
        assert!(!rule1.same(&rule3));
    }
}
