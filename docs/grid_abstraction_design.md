# Grid Abstraction Layer Design Document

## 1. Executive Summary

This document describes the design for abstracting the MarkovJunior grid system to support both Cartesian (x,y,z) and Polar/Spherical (r,θ,φ) coordinate systems through a unified trait-based interface.

### Goals
1. **Zero regression** - Existing Cartesian tests must pass unchanged
2. **Minimal changes** - Lift existing code with smallest possible modifications
3. **Incremental verification** - Every change verified before proceeding
4. **Future-proof** - Polar designed as 3D (spherical) from day one

### Non-Goals
- Rewriting working Cartesian code
- Adding new features during abstraction
- Optimizing performance (that comes later)

---

## 2. Current Architecture

### 2.1 Cartesian Grid (`MjGrid`)

```rust
pub struct MjGrid {
    pub state: Vec<u8>,           // Flat array of cell values
    pub mask: Vec<bool>,          // Modification tracking
    pub mx: usize,                // X dimension
    pub my: usize,                // Y dimension  
    pub mz: usize,                // Z dimension (1 for 2D)
    pub c: u8,                    // Number of distinct values
    pub characters: Vec<char>,   // Index -> character
    pub values: HashMap<char, u8>, // Character -> index
    pub waves: HashMap<char, u32>, // Character -> wave bitmask
}

// Indexing: i = x + y * mx + z * mx * my
```

### 2.2 Polar Grid (`PolarMjGrid`)

```rust
pub struct PolarMjGrid {
    pub rings: Vec<Vec<u8>>,      // rings[r][theta] = value
    pub r_min: u32,               // Minimum radius
    pub r_depth: u16,             // Number of radial levels
    pub theta_divisions: u16,     // Angular divisions (fixed)
    pub target_arc_length: f32,   // Target arc length
}

// Indexing: Currently uses (r, theta) tuple access
```

### 2.3 Key Differences

| Aspect | Cartesian | Polar |
|--------|-----------|-------|
| Storage | Single flat `Vec<u8>` | Nested `Vec<Vec<u8>>` |
| Indexing | `x + y*mx + z*mx*my` | `rings[r][theta]` |
| Dimensions | Always 3 (mz=1 for 2D) | Currently 2 only |
| Neighbors | ±1 in x,y,z | r±1, theta±1 (wrapping) |
| Values/Waves | In grid struct | Not in grid struct |

---

## 3. Proposed Abstraction

### 3.1 Core Trait: `MjGridOps`

```rust
/// Core operations that any MJ-compatible grid must support.
/// 
/// This trait abstracts over coordinate systems while preserving
/// the flat-index access pattern used throughout the codebase.
pub trait MjGridOps {
    // === Dimensions ===
    
    /// Total number of cells in the grid
    fn len(&self) -> usize;
    
    /// Whether grid is 2D (used for symmetry selection)
    fn is_2d(&self) -> bool;
    
    // === State Access ===
    
    /// Get cell value at flat index
    fn get_state(&self, idx: usize) -> u8;
    
    /// Set cell value at flat index
    fn set_state(&mut self, idx: usize, value: u8);
    
    /// Get entire state as slice (for bulk operations)
    fn state(&self) -> &[u8];
    
    /// Get mutable state as slice
    fn state_mut(&mut self) -> &mut [u8];
    
    // === Value System ===
    
    /// Number of distinct values/colors
    fn num_values(&self) -> u8;
    
    /// Get index for character (e.g., 'B' -> 0)
    fn value_for_char(&self, ch: char) -> Option<u8>;
    
    /// Get character for index (e.g., 0 -> 'B')
    fn char_for_value(&self, val: u8) -> Option<char>;
    
    /// Get wave bitmask for character
    fn wave_for_char(&self, ch: char) -> Option<u32>;
    
    /// Get combined wave for string (e.g., "BW" -> 0b11)
    fn wave(&self, chars: &str) -> u32;
    
    // === Mask (for AllNode non-overlap checking) ===
    
    /// Get mask value at flat index
    fn get_mask(&self, idx: usize) -> bool;
    
    /// Set mask value at flat index  
    fn set_mask(&mut self, idx: usize, value: bool);
    
    /// Clear all mask values
    fn clear_mask(&mut self);
    
    // === Coordinate System Info ===
    
    /// Dimension sizes as (d0, d1, d2) - interpretation varies by grid type
    /// Cartesian: (mx, my, mz)
    /// Spherical: (theta_divs, phi_divs, r_depth)
    fn dimensions(&self) -> (usize, usize, usize);
}
```

### 3.2 Why Flat Indices?

The key insight is that **most MJ logic already uses flat indices internally**:

```rust
// Current Cartesian code in rule_node.rs
let i = x as usize + y as usize * mx + z as usize * mx * my;
self.match_mask[r][i] = true;
```

By making flat indices the universal currency, we:
1. Minimize changes to existing code
2. Avoid coordinate conversion overhead in hot paths
3. Keep `changes: Vec<usize>` simple for both systems

### 3.3 Coordinate Conversion (Grid-Specific)

Each grid type provides its own coordinate methods (not in trait):

```rust
// Cartesian-specific
impl MjGrid {
    pub fn coord_to_index(&self, x: i32, y: i32, z: i32) -> usize;
    pub fn index_to_coord(&self, idx: usize) -> (i32, i32, i32);
}

// Spherical-specific  
impl SphericalMjGrid {
    pub fn coord_to_index(&self, r: u16, theta: u16, phi: u16) -> usize;
    pub fn index_to_coord(&self, idx: usize) -> (u16, u16, u16);
}
```

### 3.4 Polar Grid Redesign (3D from Day 1)

```rust
pub struct SphericalMjGrid {
    // Flat storage like Cartesian
    pub state: Vec<u8>,
    pub mask: Vec<bool>,
    
    // Dimensions
    pub r_depth: u16,           // Radial levels
    pub theta_divisions: u16,   // Azimuthal divisions
    pub phi_divisions: u16,     // Elevation divisions (1 for 2D)
    
    // Geometry
    pub r_min: u32,             // Minimum radius
    pub target_arc_length: f32,
    
    // Value system (same as Cartesian)
    pub c: u8,
    pub characters: Vec<char>,
    pub values: HashMap<char, u8>,
    pub waves: HashMap<char, u32>,
}

// Indexing: i = theta + phi * theta_divs + r * theta_divs * phi_divs
// For 2D (phi_divs=1): i = theta + r * theta_divs
```

---

## 4. What Changes, What Stays

### 4.1 No Changes Required

| Component | Why No Change |
|-----------|---------------|
| `SequenceNode` | Only calls `child.go()` - grid-agnostic |
| `MarkovNode` | Only calls `child.go()` - grid-agnostic |
| Step counting | Uses `counter: usize` - grid-agnostic |
| Temperature | Uses `f64` - grid-agnostic |
| RNG | Uses trait `MjRng` - grid-agnostic |
| Recording system | Already uses `RecordableGrid` trait |

### 4.2 Minimal Changes Required

| Component | Change | Scope |
|-----------|--------|-------|
| `ExecutionContext` | Change `grid: &mut MjGrid` to `grid: &mut dyn MjGridOps` | 1 struct |
| `RuleNodeData` | Change `matches: Vec<(usize, i32, i32, i32)>` to `matches: Vec<(usize, usize)>` | 1 struct |
| `changes` tracking | Change `Vec<(i32, i32, i32)>` to `Vec<usize>` | 2 locations |
| `OneNode::apply` | Use flat indices | 1 method |
| `AllNode::apply` | Use flat indices | 1 method |

### 4.3 Grid-Specific (Separate Implementations)

| Component | Why Separate |
|-----------|--------------|
| Pattern matching | Different neighbor relationships |
| Rule struct | Different coordinate representation |
| Symmetry | Square vs Polar symmetry groups |
| XML pattern parsing | Different syntax |

---

## 5. Trait Implementation Strategy

### 5.1 For `MjGrid` (Cartesian)

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
        // Existing implementation
        self.wave(chars)
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

### 5.2 For `SphericalMjGrid` (Polar)

Same implementation pattern, just different dimension interpretation.

---

## 6. Match System Redesign

### 6.1 Current Match Format

```rust
// Current: (rule_index, x, y, z)
pub type Match = (usize, i32, i32, i32);

// In RuleNodeData
pub matches: Vec<Match>,
```

### 6.2 New Match Format

```rust
// New: (rule_index, flat_index)
pub type Match = (usize, usize);

// In RuleNodeData  
pub matches: Vec<Match>,
```

### 6.3 Change Tracking

```rust
// Current in ExecutionContext
pub changes: Vec<(i32, i32, i32)>,

// New in ExecutionContext
pub changes: Vec<usize>,
```

---

## 7. Rule Abstraction

Rules are the most coordinate-specific component. We have two options:

### Option A: Separate Rule Types (Recommended)

Keep `MjRule` for Cartesian, create `SphericalRule` for Polar.

Pros:
- No changes to working Cartesian rule code
- Each rule type optimized for its coordinate system
- Cleaner separation of concerns

Cons:
- Some code duplication in rule nodes

### Option B: Generic Rule Trait

```rust
trait MjRuleOps {
    fn matches(&self, grid: &dyn MjGridOps, idx: usize) -> bool;
    fn apply(&self, grid: &mut dyn MjGridOps, idx: usize);
}
```

Pros:
- Maximum code reuse
- Single implementation of rule nodes

Cons:
- Virtual dispatch overhead
- More complex generic bounds
- Harder to optimize

**Recommendation**: Start with Option A for safety, evaluate Option B later.

---

## 8. Compatibility Layer

To minimize risk, we provide a compatibility layer:

```rust
/// Extension trait providing coordinate-based access for Cartesian grids
pub trait CartesianGridExt: MjGridOps {
    fn mx(&self) -> usize;
    fn my(&self) -> usize;
    fn mz(&self) -> usize;
    
    fn get_xyz(&self, x: i32, y: i32, z: i32) -> u8;
    fn set_xyz(&mut self, x: i32, y: i32, z: i32, value: u8);
    fn coord_to_index(&self, x: i32, y: i32, z: i32) -> usize;
}

impl CartesianGridExt for MjGrid {
    // Implementations that call through to existing methods
}
```

This allows existing code that uses `grid.mx` to continue working.

---

## 9. Testing Strategy

### 9.1 Test Categories

1. **Unit tests** - Individual trait method tests
2. **Existing tests** - All current Cartesian tests must pass
3. **Parity tests** - Compare against reference MJ output
4. **Integration tests** - Full model execution

### 9.2 Verification Checkpoints

| Checkpoint | Tests | Pass Criteria |
|------------|-------|---------------|
| Trait definition | Compiles | No errors |
| MjGrid impl | `cargo test grid` | All pass |
| ExecutionContext change | `cargo test node` | All pass |
| Match format change | `cargo test rule_node` | All pass |
| Changes format | `cargo test interpreter` | All pass |
| Full integration | `cargo test markov` | All pass |
| Parity | `cargo test verification` | Matches C# output |

### 9.3 Test Commands

```bash
# Quick sanity check
cargo test -p studio_core grid --no-fail-fast

# Node tests  
cargo test -p studio_core node --no-fail-fast

# Full MJ tests
cargo test -p studio_core markov_junior --no-fail-fast

# Parity verification
cargo test -p studio_core verification --no-fail-fast
```

---

## 10. Risk Assessment

### 10.1 Low Risk

- Trait definition (additive)
- Trait implementation for MjGrid (wrapper methods)
- SphericalMjGrid creation (new code)

### 10.2 Medium Risk

- ExecutionContext grid type change
- Match format change (affects multiple files)

### 10.3 High Risk

- Changes format change (affects change tracking throughout)
- Any modification to rule matching logic

### 10.4 Mitigation

- **Feature flags**: Use `cfg` attributes to switch between old/new
- **Parallel implementations**: Keep old code until new is verified
- **Incremental migration**: One component at a time

---

## 11. Open Questions

1. **Should SphericalMjGrid use nested Vecs or flat Vec?**
   - Flat: Consistent with Cartesian, simpler indexing
   - Nested: Matches current Polar design, variable theta per ring
   - **Recommendation**: Flat, with fixed divisions

2. **How to handle variable theta divisions per ring?**
   - Current Polar: Each ring can have different theta divisions
   - Proposed: Fixed theta divisions (already implemented)
   - **Decision needed**: Is variable division required?

3. **Should we support both coordinate and index access in trait?**
   - Option A: Index only in trait, coordinates in extensions
   - Option B: Both in trait with associated types
   - **Recommendation**: Option A for simplicity

---

## 12. Success Criteria

1. All existing Cartesian tests pass
2. All parity tests pass (same output as C# MJ)
3. SphericalMjGrid can be used with recording system
4. XML loading works for both grid types
5. No performance regression (within 5%)

---

## 13. Next Steps

1. **Research phase**: Audit existing tests, identify parity infrastructure
2. **Implementation document**: Detailed step-by-step plan
3. **Phase 1**: Trait definition + MjGrid implementation
4. **Phase 2**: ExecutionContext migration
5. **Phase 3**: Match/changes format migration
6. **Phase 4**: SphericalMjGrid implementation
7. **Phase 5**: Polar XML loading
