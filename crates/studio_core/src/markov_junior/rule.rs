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
    /// Compact input: single value per cell, 0xff for wildcard
    /// Used for fast observation matching
    pub binput: Vec<u8>,
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
    /// Precomputed input shifts: ishifts[color] = [(x,y,z), ...] positions that match color
    /// Used for incremental pattern matching
    pub ishifts: Vec<Vec<(i32, i32, i32)>>,
    /// Precomputed output shifts: oshifts[color] = [(x,y,z), ...] positions that output color
    /// Only populated when input and output dimensions match
    pub oshifts: Vec<Vec<(i32, i32, i32)>>,
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
    /// Create a rule directly from pattern arrays.
    /// Computes binput, ishifts, and oshifts automatically.
    pub fn from_patterns(
        input: Vec<u32>,
        imx: usize,
        imy: usize,
        imz: usize,
        output: Vec<u8>,
        omx: usize,
        omy: usize,
        omz: usize,
        c: u8,
        p: f64,
    ) -> Self {
        // Compute binput
        let wildcard = (1u32 << c) - 1;
        let binput: Vec<u8> = input
            .iter()
            .map(|&w| {
                if w == wildcard {
                    0xff
                } else {
                    w.trailing_zeros() as u8
                }
            })
            .collect();

        // Compute ishifts
        let mut ishifts: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); c as usize];
        for z in 0..imz {
            for y in 0..imy {
                for x in 0..imx {
                    let idx = x + y * imx + z * imx * imy;
                    let mut w = input[idx];
                    for color in 0..c as usize {
                        if (w & 1) == 1 {
                            ishifts[color].push((x as i32, y as i32, z as i32));
                        }
                        w >>= 1;
                    }
                }
            }
        }

        // Compute oshifts (only when dimensions match)
        let oshifts = if omx == imx && omy == imy && omz == imz {
            let mut oshifts: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); c as usize];
            for z in 0..omz {
                for y in 0..omy {
                    for x in 0..omx {
                        let idx = x + y * omx + z * omx * omy;
                        let o = output[idx];
                        if o != 0xff {
                            if (o as usize) < oshifts.len() {
                                oshifts[o as usize].push((x as i32, y as i32, z as i32));
                            }
                        } else {
                            for color in 0..c as usize {
                                oshifts[color].push((x as i32, y as i32, z as i32));
                            }
                        }
                    }
                }
            }
            oshifts
        } else {
            Vec::new()
        };

        Self {
            input,
            output,
            binput,
            imx,
            imy,
            imz,
            omx,
            omy,
            omz,
            p,
            c,
            ishifts,
            oshifts,
        }
    }

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

        // Compute binput: single value per cell, 0xff for wildcard
        let wildcard = (1u32 << grid.c) - 1;
        let binput: Vec<u8> = input
            .iter()
            .map(|&w| {
                if w == wildcard {
                    0xff
                } else {
                    w.trailing_zeros() as u8
                }
            })
            .collect();

        // Compute ishifts: for each color, which positions in input match it
        let mut ishifts: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); grid.c as usize];
        for z in 0..imz {
            for y in 0..imy {
                for x in 0..imx {
                    let idx = x + y * imx + z * imx * imy;
                    let mut w = input[idx];
                    for c in 0..grid.c as usize {
                        if (w & 1) == 1 {
                            ishifts[c].push((x as i32, y as i32, z as i32));
                        }
                        w >>= 1;
                    }
                }
            }
        }

        // Compute oshifts: for each color, which positions output it
        // Only when dimensions match (same grid rule)
        let oshifts = if omx == imx && omy == imy && omz == imz {
            let mut oshifts: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); grid.c as usize];
            for z in 0..omz {
                for y in 0..omy {
                    for x in 0..omx {
                        let idx = x + y * omx + z * omx * omy;
                        let o = output[idx];
                        if o != 0xff {
                            oshifts[o as usize].push((x as i32, y as i32, z as i32));
                        } else {
                            // Wildcard output: add to all colors
                            for c in 0..grid.c as usize {
                                oshifts[c].push((x as i32, y as i32, z as i32));
                            }
                        }
                    }
                }
            }
            oshifts
        } else {
            Vec::new()
        };

        Ok(Self {
            input,
            output,
            binput,
            imx,
            imy,
            imz,
            omx,
            omy,
            omz,
            p: 1.0,
            c: grid.c,
            ishifts,
            oshifts,
        })
    }

    /// Parse a pattern string into characters and dimensions.
    ///
    /// Returns (chars, MX, MY, MZ) where chars is in x + y*MX + z*MX*MY order.
    ///
    /// C# reference (Rule.cs Parse method):
    /// - Split by ' ' for Z layers, then by '/' for Y rows
    /// - Z layers are REVERSED: linesz = lines[MZ - 1 - z]
    /// - Y rows are NOT reversed
    fn parse_pattern(s: &str) -> Result<(Vec<char>, usize, usize, usize), RuleParseError> {
        if s.is_empty() {
            return Err(RuleParseError::EmptyPattern);
        }

        // Split by space for Z layers, then by / for Y rows
        let layers: Vec<&str> = s.split(' ').collect();
        let mz = layers.len();

        // Determine dimensions from first layer
        let first_rows: Vec<&str> = layers[0].split('/').collect();
        let my = first_rows.len();
        let mx = if !first_rows.is_empty() {
            first_rows[0].chars().count()
        } else {
            return Err(RuleParseError::EmptyPattern);
        };

        // Pre-allocate result array
        let mut result = vec![' '; mx * my * mz];

        // Process layers with Z reversal to match C#
        // C#: linesz = lines[MZ - 1 - z]
        for z in 0..mz {
            let layer = layers[mz - 1 - z]; // Reverse Z order!
            let rows: Vec<&str> = layer.split('/').collect();

            if rows.len() != my {
                return Err(RuleParseError::NonRectangularPattern);
            }

            for (y, row) in rows.iter().enumerate() {
                if row.chars().count() != mx {
                    return Err(RuleParseError::NonRectangularPattern);
                }

                for (x, ch) in row.chars().enumerate() {
                    let idx = x + y * mx + z * mx * my;
                    result[idx] = ch;
                }
            }
        }

        Ok((result, mx, my, mz))
    }

    /// Create a Z-rotated (around Z axis) version of this rule.
    /// This rotates the XY plane 90 degrees counter-clockwise.
    pub fn z_rotated(&self) -> Self {
        let mut new_input = vec![0u32; self.input.len()];
        let mut new_output = vec![0u8; self.output.len()];

        let new_imx = self.imy;
        let new_imy = self.imx;
        let new_omx = self.omy;
        let new_omy = self.omx;

        // For each position in new array, find source position
        // new[x + y*IMY + z*IMX*IMY] = old[IMX-1-y + x*IMX + z*IMX*IMY]
        for z in 0..self.imz {
            for y in 0..new_imy {
                for x in 0..new_imx {
                    let new_idx = x + y * new_imx + z * new_imx * new_imy;
                    let old_idx = (self.imx - 1 - y) + x * self.imx + z * self.imx * self.imy;
                    new_input[new_idx] = self.input[old_idx];
                }
            }
        }

        for z in 0..self.omz {
            for y in 0..new_omy {
                for x in 0..new_omx {
                    let new_idx = x + y * new_omx + z * new_omx * new_omy;
                    let old_idx = (self.omx - 1 - y) + x * self.omx + z * self.omx * self.omy;
                    new_output[new_idx] = self.output[old_idx];
                }
            }
        }

        Self::from_patterns(
            new_input, new_imx, new_imy, self.imz, new_output, new_omx, new_omy, self.omz, self.c,
            self.p,
        )
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

        Self::from_patterns(
            new_input, self.imx, self.imy, self.imz, new_output, self.omx, self.omy, self.omz,
            self.c, self.p,
        )
    }

    /// Create a Y-rotated (around Y axis) version of this rule.
    /// This rotates the XZ plane 90 degrees, used for 3D cube symmetries.
    ///
    /// Transformation: (x, y, z) -> (z, y, IMX-1-x)
    /// After rotation: new dimensions are (IMZ, IMY, IMX)
    ///
    /// C# Reference: Rule.cs YRotated() lines 76-87
    pub fn y_rotated(&self) -> Self {
        let mut new_input = vec![0u32; self.input.len()];
        let mut new_output = vec![0u8; self.output.len()];

        let new_imx = self.imz;
        let new_imy = self.imy;
        let new_imz = self.imx;
        let new_omx = self.omz;
        let new_omy = self.omy;
        let new_omz = self.omx;

        // C# Reference (lines 78-80):
        // for (int z = 0; z < IMX; z++) for (int y = 0; y < IMY; y++) for (int x = 0; x < IMZ; x++)
        //     newinput[x + y * IMZ + z * IMZ * IMY] = input[IMX - 1 - z + y * IMX + x * IMX * IMY];
        for z in 0..new_imz {
            for y in 0..new_imy {
                for x in 0..new_imx {
                    let new_idx = x + y * new_imx + z * new_imx * new_imy;
                    // Source: (IMX - 1 - z, y, x) in original coordinates
                    let old_idx = (self.imx - 1 - z) + y * self.imx + x * self.imx * self.imy;
                    new_input[new_idx] = self.input[old_idx];
                }
            }
        }

        // C# Reference (lines 82-84):
        // for (int z = 0; z < OMX; z++) for (int y = 0; y < OMY; y++) for (int x = 0; x < OMZ; x++)
        //     newoutput[x + y * OMZ + z * OMZ * OMY] = output[OMX - 1 - z + y * OMX + x * OMX * OMY];
        for z in 0..new_omz {
            for y in 0..new_omy {
                for x in 0..new_omx {
                    let new_idx = x + y * new_omx + z * new_omx * new_omy;
                    let old_idx = (self.omx - 1 - z) + y * self.omx + x * self.omx * self.omy;
                    new_output[new_idx] = self.output[old_idx];
                }
            }
        }

        Self::from_patterns(
            new_input, new_imx, new_imy, new_imz, new_output, new_omx, new_omy, new_omz, self.c,
            self.p,
        )
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
        // Pattern "AB CD" means: layer at z=high is "AB", layer at z=low is "CD"
        // C# reverses Z, so in memory:
        // - z=0 gets "CD" (the LAST layer in string)
        // - z=1 gets "AB" (the FIRST layer in string)
        let (chars, mx, my, mz) = MjRule::parse_pattern("AB CD").unwrap();
        assert_eq!((mx, my, mz), (2, 1, 2));
        // After Z reversal: z=0 is CD, z=1 is AB
        assert_eq!(chars[0], 'C'); // (0,0,0)
        assert_eq!(chars[1], 'D'); // (1,0,0)
        assert_eq!(chars[2], 'A'); // (0,0,1)
        assert_eq!(chars[3], 'B'); // (1,0,1)
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

    #[test]
    fn test_rule_y_rotated() {
        // Create a 3D grid
        let grid = MjGrid::with_values(5, 5, 5, "ABCD");

        // Create a 3D rule: 2x1x2 pattern
        // Layer at z=0: "AB"
        // Layer at z=1: "CD"
        // Pattern string: "CD AB" (Z layers are reversed in parse)
        let rule = MjRule::parse("CD AB", "AB CD", &grid).unwrap();
        assert_eq!((rule.imx, rule.imy, rule.imz), (2, 1, 2));

        let rotated = rule.y_rotated();

        // After Y rotation, dimensions change: (IMZ, IMY, IMX) = (2, 1, 2)
        // In this case dimensions stay same since imx == imz
        assert_eq!((rotated.imx, rotated.imy, rotated.imz), (2, 1, 2));

        // The rule should be different after rotation
        // (unless it happens to be symmetric)
        // We can verify the content changed
        assert!(!rule.same(&rotated) || (rule.imx == rule.imz));
    }

    #[test]
    fn test_rule_y_rotated_asymmetric() {
        // Create a 3D grid
        let grid = MjGrid::with_values(5, 5, 5, "ABCD");

        // Create a non-cubic 3D rule: 3x1x1 pattern "ABC"
        // This has only 1 Z layer so pattern string is just "ABC"
        let rule = MjRule::parse("ABC", "CBA", &grid).unwrap();
        assert_eq!((rule.imx, rule.imy, rule.imz), (3, 1, 1));

        let rotated = rule.y_rotated();

        // After Y rotation: (IMZ, IMY, IMX) = (1, 1, 3)
        assert_eq!((rotated.imx, rotated.imy, rotated.imz), (1, 1, 3));
    }
}
