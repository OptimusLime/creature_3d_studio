# Grid Abstraction Implementation Plan

## 1. Research Summary

### 1.1 Test Infrastructure

**Total MarkovJunior Tests**: 388 tests across 29 modules

**Key Test Categories**:
| Category | Count | Purpose |
|----------|-------|---------|
| `loader` | 43 | XML parsing, model loading |
| `wfc` | 44 | Wave Function Collapse |
| `polar_grid` | 42 | Polar coordinate system |
| `render` | 31 | PNG/image rendering |
| `convchain_node` | 16 | MCMC texture synthesis |
| `convolution_node` | 15 | Cellular automata |
| `verification` | 11 | **C# parity checks** |
| `node_tests` | 8 | Core node behavior |
| `interpreter` | 5 | Execution loop |

**Critical Test Subsets for Verification**:
```bash
# Tier 1: Core abstractions (must pass first)
cargo test -p studio_core markov_junior::node

# Tier 2: Rule nodes (next)
cargo test -p studio_core markov_junior::one_node
cargo test -p studio_core markov_junior::all_node
cargo test -p studio_core markov_junior::parallel_node

# Tier 3: Interpreter (integration)
cargo test -p studio_core markov_junior::interpreter

# Tier 4: Parity (C# compatibility)
cargo test -p studio_core markov_junior::verification
```

### 1.2 Code Access Patterns

**Grid Dimension Access** (49 occurrences):
```rust
ctx.grid.mx  // 20 times
ctx.grid.my  // 19 times  
ctx.grid.mz  // 10 times
```

**Grid State Access** (93 occurrences):
```rust
ctx.grid.state[idx]      // Direct indexing
ctx.grid.state.iter()    // Iteration
ctx.grid.state.len()     // Size
```

**Grid Method Calls**:
```rust
ctx.grid.matches(&rule, x, y, z)  // 11 occurrences
ctx.grid.values.get(&ch)          // Value lookups
ctx.grid.wave(chars)              // Wave bitmask
```

### 1.3 Parity Infrastructure

The `verification.rs` module provides:
- `capture_model_state()` - Run model and capture final state
- `compare_states()` - Diff two states cell-by-cell
- `DotNetRandom` - C#-compatible RNG for reproducibility
- JSON output format for cross-language comparison

**Key Parity Tests**:
- `test_basic_model_matches_reference` - Uses DotNetRandom
- `test_flowers_bisect` - Binary search for divergence
- `test_river_bisect` - Another bisection test

---

## 2. Implementation Phases

### Overview

```
Phase 0: Baseline (no code changes)
    └── Run all tests, record baseline

Phase 1: Trait Definition (additive only)
    └── Define MjGridOps trait
    └── Tests: Compilation only

Phase 2: MjGrid Implementation (minimal change)
    └── impl MjGridOps for MjGrid
    └── Tests: markov_junior::tests (grid-specific)

Phase 3: ExecutionContext Migration
    └── Add trait bound to ExecutionContext
    └── Tests: markov_junior::node

Phase 4: Match Format Migration
    └── Change Match type to use indices
    └── Tests: markov_junior::rule_node, one_node, all_node

Phase 5: Full Integration
    └── Run all 388 tests
    └── Run parity verification

Phase 6: SphericalMjGrid (new code)
    └── Implement SphericalMjGrid
    └── Tests: markov_junior::polar_grid (adapted)
```

---

## 3. Phase 0: Establish Baseline

### 3.0.1 Run All Tests

```bash
# Record baseline test results
cargo test -p studio_core markov_junior 2>&1 | tee baseline_tests.log

# Count passing tests
grep -c "test result: ok" baseline_tests.log
```

**Expected**: All 388 tests pass.

### 3.0.2 Run Parity Verification

```bash
cargo test -p studio_core markov_junior::verification -- --nocapture
```

**Expected**: Basic, Flowers, River models match C# output.

### 3.0.3 Create Snapshot

```bash
git stash  # If needed
git checkout -b feature/grid-abstraction
```

**Checkpoint**: We have a known-good baseline to compare against.

---

## 4. Phase 1: Trait Definition

### 4.1.1 Create New File

Create `crates/studio_core/src/markov_junior/grid_ops.rs`:

```rust
//! Grid operations trait for abstracting over coordinate systems.

use std::collections::HashMap;

/// Core operations that any MJ-compatible grid must support.
pub trait MjGridOps {
    /// Total number of cells in the grid
    fn len(&self) -> usize;
    
    /// Whether grid is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Whether grid is 2D (for symmetry selection)
    fn is_2d(&self) -> bool;
    
    /// Get cell value at flat index
    fn get_state(&self, idx: usize) -> u8;
    
    /// Set cell value at flat index
    fn set_state(&mut self, idx: usize, value: u8);
    
    /// Get entire state as slice
    fn state(&self) -> &[u8];
    
    /// Get mutable state as slice
    fn state_mut(&mut self) -> &mut [u8];
    
    /// Number of distinct values/colors
    fn num_values(&self) -> u8;
    
    /// Get index for character
    fn value_for_char(&self, ch: char) -> Option<u8>;
    
    /// Get character for index
    fn char_for_value(&self, val: u8) -> Option<char>;
    
    /// Get wave bitmask for character
    fn wave_for_char(&self, ch: char) -> Option<u32>;
    
    /// Get combined wave for string
    fn wave(&self, chars: &str) -> u32;
    
    /// Get mask value at flat index
    fn get_mask(&self, idx: usize) -> bool;
    
    /// Set mask value at flat index
    fn set_mask(&mut self, idx: usize, value: bool);
    
    /// Clear all mask values
    fn clear_mask(&mut self);
    
    /// Dimension sizes (interpretation varies by grid type)
    fn dimensions(&self) -> (usize, usize, usize);
}
```

### 4.1.2 Add Module

In `mod.rs`, add:
```rust
pub mod grid_ops;
pub use grid_ops::MjGridOps;
```

### 4.1.3 Verify

```bash
cargo build -p studio_core
```

**Expected**: Compiles with no errors. No tests affected (trait not used yet).

**Checkpoint**: Trait exists, zero functional changes.

---

## 5. Phase 2: MjGrid Implementation

### 5.2.1 Implement Trait for MjGrid

In `mod.rs`, add impl block:

```rust
impl MjGridOps for MjGrid {
    fn len(&self) -> usize {
        self.state.len()
    }
    
    fn is_2d(&self) -> bool {
        self.mz == 1
    }
    
    fn get_state(&self, idx: usize) -> u8 {
        self.state[idx]
    }
    
    fn set_state(&mut self, idx: usize, value: u8) {
        self.state[idx] = value;
    }
    
    fn state(&self) -> &[u8] {
        &self.state
    }
    
    fn state_mut(&mut self) -> &mut [u8] {
        &mut self.state
    }
    
    fn num_values(&self) -> u8 {
        self.c
    }
    
    fn value_for_char(&self, ch: char) -> Option<u8> {
        self.values.get(&ch).copied()
    }
    
    fn char_for_value(&self, val: u8) -> Option<char> {
        self.characters.get(val as usize).copied()
    }
    
    fn wave_for_char(&self, ch: char) -> Option<u32> {
        self.waves.get(&ch).copied()
    }
    
    fn wave(&self, chars: &str) -> u32 {
        // Delegate to existing method
        MjGrid::wave(self, chars)
    }
    
    fn get_mask(&self, idx: usize) -> bool {
        self.mask[idx]
    }
    
    fn set_mask(&mut self, idx: usize, value: bool) {
        self.mask[idx] = value;
    }
    
    fn clear_mask(&mut self) {
        self.mask.fill(false);
    }
    
    fn dimensions(&self) -> (usize, usize, usize) {
        (self.mx, self.my, self.mz)
    }
}
```

### 5.2.2 Add Unit Tests for Trait

```rust
#[cfg(test)]
mod grid_ops_tests {
    use super::*;
    
    #[test]
    fn test_mjgrid_implements_ops() {
        let grid = MjGrid::with_values(4, 4, 1, "BW");
        
        // Test trait methods
        assert_eq!(grid.len(), 16);
        assert!(grid.is_2d());
        assert_eq!(grid.get_state(0), 0);
        assert_eq!(grid.num_values(), 2);
        assert_eq!(grid.value_for_char('B'), Some(0));
        assert_eq!(grid.value_for_char('W'), Some(1));
        assert_eq!(grid.dimensions(), (4, 4, 1));
    }
    
    #[test]
    fn test_mjgrid_ops_3d() {
        let grid = MjGrid::with_values(2, 2, 2, "BWR");
        
        assert_eq!(grid.len(), 8);
        assert!(!grid.is_2d());
        assert_eq!(grid.dimensions(), (2, 2, 2));
    }
}
```

### 5.2.3 Verify

```bash
# Run new tests
cargo test -p studio_core grid_ops_tests

# Run existing grid tests (should still pass)
cargo test -p studio_core markov_junior::tests
```

**Expected**: New tests pass. Existing tests unchanged.

**Checkpoint**: Trait implemented for MjGrid, existing behavior preserved.

---

## 6. Phase 3: ExecutionContext Migration

### 6.3.1 Current ExecutionContext

```rust
pub struct ExecutionContext<'a> {
    pub grid: &'a mut MjGrid,  // Concrete type
    pub random: &'a mut dyn MjRng,
    pub changes: Vec<(i32, i32, i32)>,
    pub first: Vec<usize>,
    pub counter: usize,
    pub gif: bool,
}
```

### 6.3.2 Strategy: Backward Compatible

Rather than changing `ExecutionContext` immediately, we add a **second constructor** that accepts `&mut dyn MjGridOps`:

```rust
impl<'a> ExecutionContext<'a> {
    // Existing constructors unchanged
    
    /// Create context with any grid implementing MjGridOps
    /// For now, this just casts - later we'll make grid generic
    pub fn with_grid_ops(
        grid: &'a mut MjGrid,  // Still concrete for now
        random: &'a mut dyn MjRng
    ) -> Self {
        Self::new(grid, random)
    }
}
```

### 6.3.3 Verify

```bash
# Core node tests
cargo test -p studio_core markov_junior::node

# Node implementation tests
cargo test -p studio_core markov_junior::node_tests
```

**Expected**: All 8 node tests pass.

**Checkpoint**: ExecutionContext has trait-aware constructor, but no breaking changes.

---

## 7. Phase 4: Match Format Migration

This is the **highest risk** phase. We change the Match type from coordinate-based to index-based.

### 7.4.1 Current Match Type

```rust
// In rule_node.rs
pub type Match = (usize, i32, i32, i32);  // (rule_idx, x, y, z)
```

### 7.4.2 New Match Type

```rust
pub type Match = (usize, usize);  // (rule_idx, flat_index)
```

### 7.4.3 Migration Steps

**Step 1**: Add index conversion to MjGrid (if not exists)
```rust
impl MjGrid {
    pub fn coord_to_index(&self, x: i32, y: i32, z: i32) -> usize {
        x as usize + y as usize * self.mx + z as usize * self.mx * self.my
    }
    
    pub fn index_to_coord(&self, idx: usize) -> (i32, i32, i32) {
        let x = (idx % self.mx) as i32;
        let y = ((idx / self.mx) % self.my) as i32;
        let z = (idx / (self.mx * self.my)) as i32;
        (x, y, z)
    }
}
```

**Step 2**: Update RuleNodeData
```rust
// Old
pub matches: Vec<(usize, i32, i32, i32)>,

// New  
pub matches: Vec<(usize, usize)>,

// Update add_match
pub fn add_match(&mut self, r: usize, idx: usize) {
    // Simplified - no coordinate conversion needed
    if !self.match_mask[r][idx] {
        self.match_mask[r][idx] = true;
        if self.match_count < self.matches.len() {
            self.matches[self.match_count] = (r, idx);
        } else {
            self.matches.push((r, idx));
        }
        self.match_count += 1;
    }
}
```

**Step 3**: Update callers (one at a time)

### 7.4.4 Incremental Verification

After EACH file change:
```bash
cargo test -p studio_core markov_junior::rule_node
cargo test -p studio_core markov_junior::one_node
cargo test -p studio_core markov_junior::all_node
```

### 7.4.5 Full Rule Node Tests

```bash
cargo test -p studio_core markov_junior::rule_node
cargo test -p studio_core markov_junior::one_node  
cargo test -p studio_core markov_junior::all_node
cargo test -p studio_core markov_junior::parallel_node
```

**Expected**: All rule node tests pass.

**Checkpoint**: Match format is index-based, all rule nodes work.

---

## 8. Phase 5: Full Integration

### 8.5.1 Run All MJ Tests

```bash
cargo test -p studio_core markov_junior 2>&1 | tee post_migration_tests.log

# Compare with baseline
diff baseline_tests.log post_migration_tests.log
```

**Expected**: Identical results (all 388 tests pass).

### 8.5.2 Run Parity Verification

```bash
cargo test -p studio_core markov_junior::verification -- --nocapture
```

**Expected**: C# parity maintained.

### 8.5.3 Run Recording Tests

```bash
cargo test -p studio_core markov_junior::recording
```

**Expected**: Video export still works.

**Checkpoint**: Full regression test passed.

---

## 9. Phase 6: SphericalMjGrid

### 9.6.1 Create SphericalMjGrid

This is NEW code - doesn't affect Cartesian.

```rust
pub struct SphericalMjGrid {
    pub state: Vec<u8>,
    pub mask: Vec<bool>,
    pub r_depth: u16,
    pub theta_divisions: u16,
    pub phi_divisions: u16,  // 1 for 2D polar
    pub r_min: u32,
    pub target_arc_length: f32,
    pub c: u8,
    pub characters: Vec<char>,
    pub values: HashMap<char, u8>,
    pub waves: HashMap<char, u32>,
}

impl MjGridOps for SphericalMjGrid {
    // Implementation...
}
```

### 9.6.2 Adapt Polar Tests

Update existing `polar_grid.rs` tests to use new structure.

### 9.6.3 Verify

```bash
cargo test -p studio_core markov_junior::polar_grid
```

**Expected**: All 42 polar tests pass.

---

## 10. Verification Checkpoints Summary

| Phase | Tests | Command | Expected |
|-------|-------|---------|----------|
| 0 | Baseline | `cargo test -p studio_core markov_junior` | 388 pass |
| 1 | Compile | `cargo build -p studio_core` | No errors |
| 2 | Trait impl | `cargo test grid_ops_tests` | New tests pass |
| 3 | Context | `cargo test markov_junior::node` | 8 pass |
| 4a | RuleNodeData | `cargo test markov_junior::rule_node` | Pass |
| 4b | OneNode | `cargo test markov_junior::one_node` | Pass |
| 4c | AllNode | `cargo test markov_junior::all_node` | Pass |
| 4d | ParallelNode | `cargo test markov_junior::parallel_node` | Pass |
| 5a | Full suite | `cargo test markov_junior` | 388 pass |
| 5b | Parity | `cargo test verification` | C# match |
| 6 | Polar | `cargo test polar_grid` | 42 pass |

---

## 11. Rollback Strategy

At each phase, if tests fail:

1. **Stop immediately** - Don't proceed to next phase
2. **Identify failure** - Which specific test(s) failed?
3. **Minimal fix** - Fix only what's broken
4. **Re-run** - Verify fix doesn't break other tests
5. **If stuck** - `git stash && git checkout main` to return to baseline

---

## 12. Risk Mitigation

### 12.1 Feature Flags (Optional)

If changes are too risky, use compile-time flags:

```rust
#[cfg(feature = "new_match_format")]
pub type Match = (usize, usize);

#[cfg(not(feature = "new_match_format"))]
pub type Match = (usize, i32, i32, i32);
```

### 12.2 Parallel Implementations

Keep old code alongside new:

```rust
// Old (keep until new is verified)
pub fn add_match_xyz(&mut self, r: usize, x: i32, y: i32, z: i32, mx: usize, my: usize) {
    let idx = x as usize + y as usize * mx + z as usize * mx * my;
    self.add_match(r, idx);
}

// New
pub fn add_match(&mut self, r: usize, idx: usize) {
    // ...
}
```

### 12.3 Test-First Changes

For each function being modified:
1. Write a test that exercises current behavior
2. Run test (should pass)
3. Make change
4. Run test (should still pass)

---

## 13. Timeline Estimate

| Phase | Effort | Risk |
|-------|--------|------|
| Phase 0 | 10 min | None |
| Phase 1 | 30 min | None |
| Phase 2 | 1 hour | Low |
| Phase 3 | 30 min | Low |
| Phase 4 | 2-3 hours | **Medium** |
| Phase 5 | 30 min | Low |
| Phase 6 | 2-3 hours | Low |

**Total**: ~8 hours of focused work

---

## 14. Definition of Done

- [x] All 388 MarkovJunior tests pass (verified 2026-01-15)
- [x] Parity verification matches C# output (8 passed, 3 ignored)
- [x] SphericalMjGrid implements MjGridOps (12 tests, commit bfa96d6)
- [x] Polar tests pass with new structure (42 passed)
- [ ] No performance regression >5% (not yet benchmarked)
- [x] Documentation updated
- [ ] Code reviewed and merged

## 15. Implementation Status

**Completed Phases:**
| Phase | Description | Commit | Tests |
|-------|-------------|--------|-------|
| 0 | Baseline established | - | 388 pass |
| 1 | MjGridOps trait definition | 8ef8cc3 | Compile |
| 2 | MjGrid trait implementation | 8ef8cc3 | 11 pass |
| 3 | ExecutionContext generic | 8ef8cc3 | 8 pass |
| 4 | Match/Changes format migration | a6a78fc | 13 pass |
| 5 | Full integration verification | a6a78fc | 388 pass |
| 6 | SphericalMjGrid implementation | bfa96d6 | 12 pass |

**All core phases complete.** The grid abstraction layer is fully implemented and tested.
