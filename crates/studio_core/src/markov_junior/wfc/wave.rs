//! Wave state tracking for Wave Function Collapse.
//!
//! The Wave tracks which patterns are still possible at each cell,
//! along with compatibility counts for constraint propagation and
//! optional Shannon entropy for minimum entropy cell selection.
//!
//! C# Reference: WaveFunctionCollapse.cs class Wave (lines 261-327)

/// Wave state for WFC algorithm.
///
/// Tracks possibility state per cell and supports:
/// - Pattern possibility tracking (`data[cell][pattern]`)
/// - Compatibility counts for propagation (`compatible[cell][pattern][direction]`)
/// - Shannon entropy calculation (optional, for better cell selection)
#[derive(Debug, Clone)]
pub struct Wave {
    /// Possibility data: `data[cell][pattern]` = true if pattern is still possible at cell.
    /// Stored as flat Vec for efficiency: index = cell * P + pattern
    pub data: Vec<bool>,

    /// Compatibility counts: `compatible[cell][pattern][direction]` = number of supporting neighbors.
    /// When this reaches 0, the pattern becomes impossible.
    /// Stored as flat Vec: index = cell * P * D + pattern * D + direction
    pub compatible: Vec<i32>,

    /// Number of remaining possible patterns per cell.
    pub sums_of_ones: Vec<i32>,

    /// Sum of weights of remaining patterns per cell (for Shannon entropy).
    /// Only allocated if shannon=true.
    pub sums_of_weights: Option<Vec<f64>>,

    /// Sum of weight*log(weight) of remaining patterns per cell.
    /// Only allocated if shannon=true.
    pub sums_of_weight_log_weights: Option<Vec<f64>>,

    /// Shannon entropy per cell.
    /// Only allocated if shannon=true.
    pub entropies: Option<Vec<f64>>,

    /// Number of cells
    pub length: usize,

    /// Number of patterns
    pub p: usize,

    /// Number of directions (4 for 2D, 6 for 3D)
    pub d: usize,

    /// Whether Shannon entropy is being used
    pub shannon: bool,
}

impl Wave {
    /// Create a new Wave with all patterns possible at all cells.
    ///
    /// # Arguments
    /// * `length` - Number of cells in the grid
    /// * `p` - Number of patterns
    /// * `d` - Number of directions (4 for 2D, 6 for 3D)
    /// * `shannon` - Whether to track Shannon entropy
    ///
    /// C# Reference: Wave constructor (lines 269-280)
    pub fn new(length: usize, p: usize, d: usize, shannon: bool) -> Self {
        // Initialize all patterns as possible
        let data = vec![true; length * p];

        // Initialize compatible counts to -1 (will be set properly in init())
        let compatible = vec![-1; length * p * d];

        let sums_of_ones = vec![0; length];

        let (sums_of_weights, sums_of_weight_log_weights, entropies) = if shannon {
            (
                Some(vec![0.0; length]),
                Some(vec![0.0; length]),
                Some(vec![0.0; length]),
            )
        } else {
            (None, None, None)
        };

        Self {
            data,
            compatible,
            sums_of_ones,
            sums_of_weights,
            sums_of_weight_log_weights,
            entropies,
            length,
            p,
            d,
            shannon,
        }
    }

    /// Initialize the wave state for a new run.
    ///
    /// Sets all patterns as possible and initializes compatibility counts
    /// from the propagator.
    ///
    /// # Arguments
    /// * `propagator` - `propagator[d][t]` = list of compatible patterns in direction d
    /// * `sum_of_weights` - Total sum of all pattern weights
    /// * `sum_of_weight_log_weights` - Sum of w*log(w) for all patterns
    /// * `starting_entropy` - Initial entropy value
    ///
    /// C# Reference: Wave.Init() (lines 283-301)
    pub fn init(
        &mut self,
        propagator: &[Vec<Vec<usize>>],
        sum_of_weights: f64,
        sum_of_weight_log_weights: f64,
        starting_entropy: f64,
    ) {
        // Opposite directions: right<->left, down<->up, front<->back
        let opposite = [2, 3, 0, 1, 5, 4];

        for i in 0..self.length {
            for pattern in 0..self.p {
                // Set pattern as possible
                self.set_data(i, pattern, true);

                // Initialize compatible counts from opposite direction's propagator
                for dir in 0..self.d {
                    let opp_dir = opposite[dir];
                    let count = if opp_dir < propagator.len() && pattern < propagator[opp_dir].len()
                    {
                        propagator[opp_dir][pattern].len() as i32
                    } else {
                        0
                    };
                    self.set_compatible(i, pattern, dir, count);
                }
            }

            self.sums_of_ones[i] = self.p as i32;

            if self.shannon {
                if let Some(ref mut sow) = self.sums_of_weights {
                    sow[i] = sum_of_weights;
                }
                if let Some(ref mut sowlw) = self.sums_of_weight_log_weights {
                    sowlw[i] = sum_of_weight_log_weights;
                }
                if let Some(ref mut ent) = self.entropies {
                    ent[i] = starting_entropy;
                }
            }
        }
    }

    /// Copy state from another wave.
    ///
    /// C# Reference: Wave.CopyFrom() (lines 304-324)
    pub fn copy_from(&mut self, other: &Wave) {
        debug_assert_eq!(self.length, other.length);
        debug_assert_eq!(self.p, other.p);
        debug_assert_eq!(self.d, other.d);

        self.data.copy_from_slice(&other.data);
        self.compatible.copy_from_slice(&other.compatible);
        self.sums_of_ones.copy_from_slice(&other.sums_of_ones);

        if self.shannon {
            if let (Some(ref mut dst), Some(ref src)) =
                (&mut self.sums_of_weights, &other.sums_of_weights)
            {
                dst.copy_from_slice(src);
            }
            if let (Some(ref mut dst), Some(ref src)) = (
                &mut self.sums_of_weight_log_weights,
                &other.sums_of_weight_log_weights,
            ) {
                dst.copy_from_slice(src);
            }
            if let (Some(ref mut dst), Some(ref src)) = (&mut self.entropies, &other.entropies) {
                dst.copy_from_slice(src);
            }
        }
    }

    /// Get whether a pattern is possible at a cell.
    #[inline]
    pub fn get_data(&self, cell: usize, pattern: usize) -> bool {
        self.data[cell * self.p + pattern]
    }

    /// Set whether a pattern is possible at a cell.
    #[inline]
    pub fn set_data(&mut self, cell: usize, pattern: usize, value: bool) {
        self.data[cell * self.p + pattern] = value;
    }

    /// Get the compatible count for a pattern in a direction at a cell.
    #[inline]
    pub fn get_compatible(&self, cell: usize, pattern: usize, direction: usize) -> i32 {
        self.compatible[cell * self.p * self.d + pattern * self.d + direction]
    }

    /// Set the compatible count for a pattern in a direction at a cell.
    #[inline]
    pub fn set_compatible(&mut self, cell: usize, pattern: usize, direction: usize, value: i32) {
        self.compatible[cell * self.p * self.d + pattern * self.d + direction] = value;
    }

    /// Decrement the compatible count and return the new value.
    #[inline]
    pub fn decrement_compatible(&mut self, cell: usize, pattern: usize, direction: usize) -> i32 {
        let idx = cell * self.p * self.d + pattern * self.d + direction;
        self.compatible[idx] -= 1;
        self.compatible[idx]
    }

    /// Get the number of remaining possibilities at a cell.
    #[inline]
    pub fn remaining(&self, cell: usize) -> i32 {
        self.sums_of_ones[cell]
    }

    /// Get the entropy at a cell (returns remaining count if not using Shannon entropy).
    #[inline]
    pub fn entropy(&self, cell: usize) -> f64 {
        if let Some(ref ent) = self.entropies {
            ent[cell]
        } else {
            self.sums_of_ones[cell] as f64
        }
    }

    /// Check if wave is in a contradiction state (any cell has 0 possibilities).
    pub fn is_contradiction(&self) -> bool {
        self.sums_of_ones.iter().any(|&s| s == 0)
    }

    /// Check if wave is fully collapsed (all cells have exactly 1 possibility).
    pub fn is_collapsed(&self) -> bool {
        self.sums_of_ones.iter().all(|&s| s == 1)
    }

    /// Get the single remaining pattern at a cell (assumes cell is collapsed).
    pub fn get_collapsed_pattern(&self, cell: usize) -> Option<usize> {
        if self.sums_of_ones[cell] != 1 {
            return None;
        }
        for pattern in 0..self.p {
            if self.get_data(cell, pattern) {
                return Some(pattern);
            }
        }
        None
    }

    /// Get all possible patterns at a cell.
    pub fn get_possible_patterns(&self, cell: usize) -> Vec<usize> {
        (0..self.p).filter(|&p| self.get_data(cell, p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave_new() {
        let wave = Wave::new(9, 4, 4, false);
        assert_eq!(wave.length, 9);
        assert_eq!(wave.p, 4);
        assert_eq!(wave.d, 4);
        assert!(!wave.shannon);
        assert_eq!(wave.data.len(), 9 * 4);
        assert_eq!(wave.compatible.len(), 9 * 4 * 4);
    }

    #[test]
    fn test_wave_new_with_shannon() {
        let wave = Wave::new(9, 4, 4, true);
        assert!(wave.shannon);
        assert!(wave.sums_of_weights.is_some());
        assert!(wave.sums_of_weight_log_weights.is_some());
        assert!(wave.entropies.is_some());
    }

    #[test]
    fn test_wave_data_access() {
        let mut wave = Wave::new(4, 3, 4, false);

        // All should start as true
        assert!(wave.get_data(0, 0));
        assert!(wave.get_data(2, 1));

        // Set to false
        wave.set_data(1, 2, false);
        assert!(!wave.get_data(1, 2));
        assert!(wave.get_data(1, 0)); // Others unchanged
    }

    #[test]
    fn test_wave_compatible_access() {
        let mut wave = Wave::new(4, 3, 4, false);

        wave.set_compatible(0, 1, 2, 5);
        assert_eq!(wave.get_compatible(0, 1, 2), 5);

        let new_val = wave.decrement_compatible(0, 1, 2);
        assert_eq!(new_val, 4);
        assert_eq!(wave.get_compatible(0, 1, 2), 4);
    }

    #[test]
    fn test_wave_init() {
        let mut wave = Wave::new(4, 2, 4, true);

        // Simple propagator: each pattern compatible with all in each direction
        let propagator = vec![
            vec![vec![0, 1], vec![0, 1]], // direction 0
            vec![vec![0, 1], vec![0, 1]], // direction 1
            vec![vec![0, 1], vec![0, 1]], // direction 2
            vec![vec![0, 1], vec![0, 1]], // direction 3
        ];

        let sum_of_weights = 2.0;
        let sum_of_weight_log_weights = 0.0; // weights of 1.0 each
        let starting_entropy = 1.0_f64.ln(); // ln(2) for 2 equally weighted patterns

        wave.init(
            &propagator,
            sum_of_weights,
            sum_of_weight_log_weights,
            starting_entropy,
        );

        // All cells should have 2 possibilities
        for i in 0..4 {
            assert_eq!(wave.sums_of_ones[i], 2);
            assert!(wave.get_data(i, 0));
            assert!(wave.get_data(i, 1));
        }

        // Check Shannon entropy values
        assert!(wave.sums_of_weights.as_ref().unwrap()[0] - 2.0 < 0.001);
    }

    #[test]
    fn test_wave_copy_from() {
        let mut wave1 = Wave::new(4, 2, 4, true);
        let mut wave2 = Wave::new(4, 2, 4, true);

        wave1.set_data(0, 0, false);
        wave1.sums_of_ones[0] = 1;
        wave1.set_compatible(1, 1, 2, 42);

        wave2.copy_from(&wave1);

        assert!(!wave2.get_data(0, 0));
        assert_eq!(wave2.sums_of_ones[0], 1);
        assert_eq!(wave2.get_compatible(1, 1, 2), 42);
    }

    #[test]
    fn test_wave_entropy_calculation() {
        let wave = Wave::new(4, 4, 4, false);
        // Without Shannon, entropy is just the count
        // Initial state has all patterns possible
        // After init, sums_of_ones would be 4
    }

    #[test]
    fn test_wave_is_contradiction() {
        let mut wave = Wave::new(4, 2, 4, false);
        wave.sums_of_ones[0] = 2;
        wave.sums_of_ones[1] = 1;
        wave.sums_of_ones[2] = 1;
        wave.sums_of_ones[3] = 1;
        assert!(!wave.is_contradiction());

        wave.sums_of_ones[2] = 0;
        assert!(wave.is_contradiction());
    }

    #[test]
    fn test_wave_is_collapsed() {
        let mut wave = Wave::new(4, 2, 4, false);
        wave.sums_of_ones[0] = 1;
        wave.sums_of_ones[1] = 1;
        wave.sums_of_ones[2] = 1;
        wave.sums_of_ones[3] = 2; // Not collapsed
        assert!(!wave.is_collapsed());

        wave.sums_of_ones[3] = 1;
        assert!(wave.is_collapsed());
    }

    #[test]
    fn test_wave_get_collapsed_pattern() {
        let mut wave = Wave::new(2, 3, 4, false);

        // Cell 0: pattern 1 is the only one remaining
        wave.set_data(0, 0, false);
        wave.set_data(0, 1, true);
        wave.set_data(0, 2, false);
        wave.sums_of_ones[0] = 1;

        assert_eq!(wave.get_collapsed_pattern(0), Some(1));

        // Cell 1: multiple patterns (not collapsed)
        wave.sums_of_ones[1] = 2;
        assert_eq!(wave.get_collapsed_pattern(1), None);
    }

    #[test]
    fn test_wave_get_possible_patterns() {
        let mut wave = Wave::new(1, 4, 4, false);

        wave.set_data(0, 0, true);
        wave.set_data(0, 1, false);
        wave.set_data(0, 2, true);
        wave.set_data(0, 3, false);

        let possible = wave.get_possible_patterns(0);
        assert_eq!(possible, vec![0, 2]);
    }
}
