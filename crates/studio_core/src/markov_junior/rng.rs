//! Random number generator abstraction for MarkovJunior.
//!
//! This module provides a trait-based RNG abstraction that allows swapping
//! between different RNG implementations:
//!
//! - `StdRandom`: Uses Rust's `rand::rngs::StdRng` (fast, good for normal use)
//! - `DotNetRandom`: Uses `clr_random::CLRRandom` (matches .NET System.Random for verification)
//!
//! The `MjRng` trait defines the interface used throughout MarkovJunior,
//! while the concrete types implement the actual random generation.
//!
//! # Example
//!
//! ```ignore
//! use studio_core::markov_junior::rng::{MjRng, StdRandom, DotNetRandom};
//!
//! // For normal use
//! let mut rng = StdRandom::from_seed(42);
//!
//! // For C# verification
//! let mut rng = DotNetRandom::from_seed(42);
//!
//! // Both implement the same interface
//! let value = rng.next_int(); // 0..i32::MAX
//! let bounded = rng.next_int_max(100); // 0..100
//! let ranged = rng.next_int_range(10, 20); // 10..20
//! let float = rng.next_double(); // 0.0..1.0
//! ```

use clr_random::CLRRandom;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_core::SeedableRng as RandCoreSeedableRng;

/// Trait for random number generators used in MarkovJunior.
///
/// This mirrors the methods used by C# System.Random:
/// - `Next()` -> `next_int()`
/// - `Next(maxValue)` -> `next_int_max(max)`
/// - `Next(minValue, maxValue)` -> `next_int_range(min, max)`
/// - `NextDouble()` -> `next_double()`
///
/// Additional methods for Rust compatibility:
/// - `next_usize_max(max)` - for array indexing
/// - `next_bool()` - random boolean
/// - `next_u64()` - for seeding sub-RNGs
pub trait MjRng: MjRngClone {
    /// Returns a non-negative random integer in [0, i32::MAX).
    /// Equivalent to C# `Random.Next()`.
    fn next_int(&mut self) -> i32;

    /// Returns a random integer in [0, max).
    /// Equivalent to C# `Random.Next(maxValue)`.
    fn next_int_max(&mut self, max: i32) -> i32;

    /// Returns a random integer in [min, max).
    /// Equivalent to C# `Random.Next(minValue, maxValue)`.
    fn next_int_range(&mut self, min: i32, max: i32) -> i32;

    /// Returns a random double in [0.0, 1.0).
    /// Equivalent to C# `Random.NextDouble()`.
    fn next_double(&mut self) -> f64;

    /// Fill a byte slice with random bytes.
    /// Equivalent to C# `Random.NextBytes(buffer)`.
    fn next_bytes(&mut self, buffer: &mut [u8]);

    /// Returns a random usize in [0, max).
    /// Convenience method for array indexing.
    fn next_usize_max(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        // Use next_double for uniform distribution across full usize range
        (self.next_double() * max as f64) as usize
    }

    /// Returns a random boolean.
    fn next_bool(&mut self) -> bool {
        self.next_double() < 0.5
    }

    /// Returns a random u64.
    /// Used for seeding sub-RNGs.
    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        self.next_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }
}

/// Shuffle a slice in place using Fisher-Yates algorithm.
/// This is a free function since generic methods aren't dyn-compatible.
pub fn shuffle_with_rng<T>(slice: &mut [T], rng: &mut dyn MjRng) {
    for i in (1..slice.len()).rev() {
        let j = rng.next_usize_max(i + 1);
        slice.swap(i, j);
    }
}

/// Helper trait for cloning boxed MjRng trait objects.
pub trait MjRngClone {
    fn clone_box(&self) -> Box<dyn MjRng>;
}

impl<T: MjRng + Clone + 'static> MjRngClone for T {
    fn clone_box(&self) -> Box<dyn MjRng> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn MjRng> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Standard Rust RNG wrapper using `rand::rngs::StdRng`.
///
/// This is the default RNG for normal use. It's fast and produces
/// high-quality random numbers, but the sequence differs from .NET.
#[derive(Clone)]
pub struct StdRandom {
    rng: StdRng,
}

impl StdRandom {
    /// Create a new StdRandom from an i32 seed.
    ///
    /// Note: The seed is converted to u64 for StdRng.
    pub fn from_seed(seed: i32) -> Self {
        // Use absolute value like .NET does, then extend to u64
        let abs_seed = if seed == i32::MIN {
            i32::MAX as u64
        } else {
            seed.abs() as u64
        };
        Self {
            rng: StdRng::seed_from_u64(abs_seed),
        }
    }

    /// Create from a u64 seed directly.
    pub fn from_u64_seed(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }
}

impl MjRng for StdRandom {
    fn next_int(&mut self) -> i32 {
        // StdRng.gen_range returns uniform distribution
        self.rng.gen_range(0..i32::MAX)
    }

    fn next_int_max(&mut self, max: i32) -> i32 {
        if max <= 0 {
            return 0;
        }
        self.rng.gen_range(0..max)
    }

    fn next_int_range(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        self.rng.gen_range(min..max)
    }

    fn next_double(&mut self) -> f64 {
        self.rng.gen()
    }

    fn next_bytes(&mut self, buffer: &mut [u8]) {
        self.rng.fill(buffer);
    }
}

/// .NET-compatible RNG wrapper using `clr_random::CLRRandom`.
///
/// This produces the exact same sequence as .NET's System.Random
/// when initialized with the same seed. Use this for:
/// - Verifying Rust output matches C# reference
/// - Reproducing specific C# MarkovJunior outputs
pub struct DotNetRandom {
    rng: CLRRandom,
    /// Track how many values we've generated for clone support
    call_count: u64,
    /// Original seed for clone support
    seed: i32,
}

impl Clone for DotNetRandom {
    fn clone(&self) -> Self {
        // Recreate RNG with same seed and advance to same position
        let mut new_rng = Self::from_seed(self.seed);
        for _ in 0..self.call_count {
            new_rng.rng.next_i32();
        }
        new_rng.call_count = self.call_count;
        new_rng
    }
}

impl DotNetRandom {
    /// Create a new DotNetRandom from an i32 seed.
    ///
    /// This matches `new System.Random(seed)` in .NET.
    pub fn from_seed(seed: i32) -> Self {
        Self {
            rng: CLRRandom::from_seed(clr_random::Seed::from(seed)),
            call_count: 0,
            seed,
        }
    }
}

impl MjRng for DotNetRandom {
    fn next_int(&mut self) -> i32 {
        self.call_count += 1;
        self.rng.next_i32()
    }

    fn next_int_max(&mut self, max: i32) -> i32 {
        if max <= 0 {
            return 0;
        }
        self.call_count += 1;
        // C# implementation: (int)(Sample() * maxValue)
        // Sample() returns InternalSample() / int.MaxValue
        let sample = self.rng.next_f64();
        (sample * max as f64) as i32
    }

    fn next_int_range(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        self.call_count += 1;
        // C# implementation for range <= int.MaxValue:
        // (int)(Sample() * range) + minValue
        let range = (max as i64) - (min as i64);
        if range <= i32::MAX as i64 {
            let sample = self.rng.next_f64();
            (sample * range as f64) as i32 + min
        } else {
            // For large ranges, C# uses GetSampleForLargeRange()
            // This is rare in MarkovJunior, so we approximate
            let sample = self.rng.next_f64();
            ((sample * range as f64) as i64 + min as i64) as i32
        }
    }

    fn next_double(&mut self) -> f64 {
        self.call_count += 1;
        self.rng.next_f64()
    }

    fn next_bytes(&mut self, buffer: &mut [u8]) {
        use rand_core::RngCore;
        // Each byte consumes one internal sample
        self.call_count += buffer.len() as u64;
        self.rng.fill_bytes(buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_std_random_basic() {
        let mut rng = StdRandom::from_seed(42);

        // Just verify it produces values in expected ranges
        for _ in 0..100 {
            let v = rng.next_int();
            assert!(v >= 0 && v < i32::MAX);
        }

        for _ in 0..100 {
            let v = rng.next_int_max(100);
            assert!(v >= 0 && v < 100);
        }

        for _ in 0..100 {
            let v = rng.next_int_range(10, 20);
            assert!(v >= 10 && v < 20);
        }

        for _ in 0..100 {
            let v = rng.next_double();
            assert!(v >= 0.0 && v < 1.0);
        }
    }

    #[test]
    fn test_dotnet_random_matches_csharp_next() {
        let mut rng = DotNetRandom::from_seed(42);

        // These values were generated by C# with seed 42:
        // var rng = new Random(42);
        // for (int i = 0; i < 10; i++) Console.WriteLine(rng.Next());
        let expected = [
            1434747710, 302596119, 269548474, 1122627734, 361709742, 563913476, 1555655117,
            1101493307, 372913049, 1634773126,
        ];

        for (i, &exp) in expected.iter().enumerate() {
            let got = rng.next_int();
            assert_eq!(
                got, exp,
                "Mismatch at index {}: expected {}, got {}",
                i, exp, got
            );
        }
    }

    #[test]
    fn test_dotnet_random_next_double_matches_csharp() {
        let mut rng = DotNetRandom::from_seed(42);

        // C# NextDouble for seed 42: 0.6681064659115423
        let expected = 0.6681064659115423;
        let got = rng.next_double();

        let diff = (got - expected).abs();
        assert!(
            diff < 1e-15,
            "NextDouble mismatch: expected {}, got {}",
            expected,
            got
        );
    }

    #[test]
    fn test_dotnet_random_next_max() {
        let mut rng = DotNetRandom::from_seed(42);

        // Verify bounds are respected
        for _ in 0..100 {
            let v = rng.next_int_max(10);
            assert!(v >= 0 && v < 10, "Value {} out of range [0, 10)", v);
        }
    }

    #[test]
    fn test_dotnet_random_next_range() {
        let mut rng = DotNetRandom::from_seed(42);

        // Verify bounds are respected
        for _ in 0..100 {
            let v = rng.next_int_range(5, 15);
            assert!(v >= 5 && v < 15, "Value {} out of range [5, 15)", v);
        }
    }

    #[test]
    fn test_both_rngs_are_deterministic() {
        // StdRandom
        let mut rng1 = StdRandom::from_seed(123);
        let mut rng2 = StdRandom::from_seed(123);
        for _ in 0..100 {
            assert_eq!(rng1.next_int(), rng2.next_int());
        }

        // DotNetRandom
        let mut rng1 = DotNetRandom::from_seed(123);
        let mut rng2 = DotNetRandom::from_seed(123);
        for _ in 0..100 {
            assert_eq!(rng1.next_int(), rng2.next_int());
        }
    }
}
