# MarkovJunior Rust Implementation Plan

This document outlines the phased approach to porting MarkovJunior to Rust with Lua integration, following our HOW_WE_WORK principles of incremental building with verification.

## Overview

### Goals
1. **Port MarkovJunior to Rust** - Pure Rust implementation of the core algorithm
2. **Integrate with mlua** - Lua scripting for model definition and runtime control
3. **Connect to VoxelWorld** - Output MarkovJunior results as voxels in our engine
4. **Maintain test compliance** - Verify against C# reference at every step

### Key Integration Points
- `VoxelWorld` - Our existing voxel storage (chunked, supports multi-chunk)
- `creature_script.rs` - Existing Lua pattern for voxel placement
- `studio_scripting` - Existing Lua VM infrastructure with hot-reload

---

## Step 1: Core Rust Port

### Phase 1.1: Foundation Data Structures

**Outcome:** Core data structures compile and have full unit test coverage.

**Files to Create:**
```
crates/studio_core/src/markov_junior/
├── mod.rs              # Module root, re-exports
├── grid.rs             # Grid struct
├── rule.rs             # Rule struct + parsing
├── symmetry.rs         # SymmetryHelper equivalent
└── array_helper.rs     # Array utilities
```

**Tasks:**

1. **Create `grid.rs`**
   - `MjGrid` struct with `state: Vec<u8>`, `MX, MY, MZ: usize`
   - `values: HashMap<char, u8>` for symbol -> index mapping
   - `waves: HashMap<char, u32>` for symbol -> bitmask mapping
   - `fn wave(&self, values: &str) -> u32` - convert string to wave bitmask
   - `fn matches(&self, rule: &Rule, x, y, z) -> bool` - pattern matching

   **Verification:**
   ```rust
   #[test] fn test_grid_wave_single() { assert_eq!(grid.wave("B"), 0b001); }
   #[test] fn test_grid_wave_multi() { assert_eq!(grid.wave("RB"), 0b011); }
   #[test] fn test_grid_matches_simple() { /* B=W rule at (0,0,0) */ }
   ```

2. **Create `rule.rs`**
   - `Rule` struct with `input: Vec<u32>`, `output: Vec<u8>`, dimensions
   - `ishifts: Vec<Vec<(i32, i32, i32)>>` - precomputed lookup
   - `oshifts: Vec<Vec<(i32, i32, i32)>>` - for backward propagation
   - `fn parse(input: &str, output: &str, grid: &MjGrid) -> Result<Rule>`
   - `fn z_rotated(&self) -> Rule`, `fn y_rotated(&self) -> Rule`, `fn reflected(&self) -> Rule`

   **Verification:**
   ```rust
   #[test] fn test_rule_parse_1d() { /* "RBB" -> "GGR" */ }
   #[test] fn test_rule_parse_2d() { /* "RB/WW" -> "GG/RR" */ }
   #[test] fn test_rule_z_rotate() { /* verify rotation math */ }
   #[test] fn test_rule_ishifts() { /* verify shift computation */ }
   ```

3. **Create `symmetry.rs`**
   - `square_symmetries()` - 8 elements for 2D
   - `cube_symmetries()` - 48 elements for 3D
   - `get_symmetry(d2: bool, name: &str) -> Option<Vec<bool>>`
   - Subgroup definitions: `()`, `(x)`, `(y)`, `(xy)`, etc.

   **Verification:**
   ```rust
   #[test] fn test_square_symmetry_count() { assert_eq!(results.len(), 8); }
   #[test] fn test_square_no_duplicates() { /* after dedup */ }
   #[test] fn test_cube_symmetry_count() { assert_eq!(results.len(), 48); }
   ```

4. **Create `array_helper.rs`**
   - `array_2d<T>(mx: usize, my: usize, val: T) -> Vec<Vec<T>>`
   - `array_3d<T>(...) -> Vec<Vec<Vec<T>>>`
   - `set_2d<T>(arr: &mut Vec<Vec<T>>, val: T)`

   **Verification:** Simple unit tests for each helper.

**Phase 1.1 Verification:**
```bash
cargo test -p studio_core markov_junior::grid
cargo test -p studio_core markov_junior::rule
cargo test -p studio_core markov_junior::symmetry
# All tests pass
```

---

### Phase 1.2: Node Infrastructure

**Outcome:** Node trait and basic implementations compile with tests.

**Files to Create:**
```
crates/studio_core/src/markov_junior/
├── node.rs             # Node trait + Branch/Sequence/Markov
├── rule_node.rs        # RuleNode base with pattern matching
├── one_node.rs         # OneNode implementation
├── all_node.rs         # AllNode implementation
└── parallel_node.rs    # ParallelNode implementation
```

**Tasks:**

1. **Create `node.rs`**
   ```rust
   pub trait Node {
       fn reset(&mut self);
       fn go(&mut self, ctx: &mut ExecutionContext) -> bool;
   }
   
   pub struct SequenceNode { nodes: Vec<Box<dyn Node>>, n: usize }
   pub struct MarkovNode { nodes: Vec<Box<dyn Node>>, n: usize }
   ```

2. **Create `rule_node.rs`**
   - Base implementation with match tracking
   - `matches: Vec<(usize, i32, i32, i32)>` - (rule_idx, x, y, z)
   - `match_mask: Vec<Vec<bool>>` - dedupe tracking
   - Incremental pattern matching via `changes` list

3. **Create `one_node.rs`**
   - Picks random match, applies rule
   - Temperature/heuristic selection (optional in first pass)

4. **Create `all_node.rs`**
   - Greedy non-overlapping match selection
   - Uses `grid.mask` for conflict tracking

5. **Create `parallel_node.rs`**
   - Apply all matches simultaneously (double-buffer)

**Phase 1.2 Verification:**
```rust
#[test]
fn test_one_node_basic() {
    // Grid: BBBBB, Rule: B=W
    // After 3 steps: should have 3 W's
}

#[test]
fn test_all_node_fills_grid() {
    // Rule: B=W, should fill entire grid in 1 step
}

#[test]
fn test_markov_backtracker() {
    // Maze backtracker: RBB=GGR, RGG=WWR
    // Verify maze generation produces connected result
}
```

---

### Phase 1.3: Interpreter & Execution

**Outcome:** Can execute simple models programmatically.

**Files to Create:**
```
crates/studio_core/src/markov_junior/
├── interpreter.rs      # Main execution loop
└── execution_context.rs # Shared state during execution
```

**Tasks:**

1. **Create `execution_context.rs`**
   ```rust
   pub struct ExecutionContext<'a> {
       pub grid: &'a mut MjGrid,
       pub random: &'a mut StdRng,
       pub changes: Vec<(i32, i32, i32)>,
       pub first: Vec<usize>,  // change indices per turn
       pub counter: usize,
   }
   ```

2. **Create `interpreter.rs`**
   ```rust
   pub struct Interpreter {
       root: Box<dyn Node>,
       grid: MjGrid,
   }
   
   impl Interpreter {
       pub fn run(&mut self, seed: u64, max_steps: usize) -> &MjGrid;
       pub fn step(&mut self) -> bool;  // Single step for animation
   }
   ```

**Phase 1.3 Verification - C# Cross-Validation:**

Create a test that:
1. Runs C# MarkovJunior with seed 12345 on "Basic" model
2. Captures final grid state
3. Runs Rust implementation with same seed
4. Compares byte-for-byte

```rust
#[test]
fn test_basic_matches_csharp() {
    let expected = include_bytes!("../test_data/basic_seed_12345.bin");
    let mut interp = create_basic_interpreter();
    interp.run(12345, 1000);
    assert_eq!(interp.grid.state, expected);
}
```

**Generate reference data from C#:**
```bash
cd MarkovJunior
dotnet run -- --model Basic --seed 12345 --output test_data/basic_seed_12345.bin
```

---

### Phase 1.4: Model Loading (XML)

**Outcome:** Can load MarkovJunior XML models.

**Files to Create:**
```
crates/studio_core/src/markov_junior/
├── loader.rs           # XML parsing and model construction
└── model.rs            # High-level model definition
```

**Tasks:**

1. **Create `loader.rs`**
   - Use `quick-xml` or `roxmltree` for parsing
   - Parse `values`, `origin`, `symmetry` attributes
   - Parse `one`, `all`, `prl`, `markov`, `sequence` nodes
   - Parse `rule` elements with `in`, `out`, `file` attributes

2. **Create `model.rs`**
   ```rust
   pub struct Model {
       pub name: String,
       pub size: (usize, usize, usize),
       pub root: Box<dyn Node>,
       pub grid: MjGrid,
   }
   
   impl Model {
       pub fn load(path: &str) -> Result<Self>;
       pub fn run(&mut self, seed: u64) -> &MjGrid;
   }
   ```

**Phase 1.4 Verification:**
```rust
#[test]
fn test_load_basic() {
    let model = Model::load("MarkovJunior/models/Basic.xml").unwrap();
    assert_eq!(model.grid.C, 2); // B, W
}

#[test]
fn test_load_maze_backtracker() {
    let model = Model::load("MarkovJunior/models/MazeBacktracker.xml").unwrap();
    // Verify structure: markov with 2 one nodes
}

#[test]
fn test_maze_backtracker_matches_csharp() {
    let mut model = Model::load("MarkovJunior/models/MazeBacktracker.xml").unwrap();
    model.run(42);
    let expected = include_bytes!("../test_data/maze_42.bin");
    assert_eq!(model.grid.state, expected);
}
```

---

### Phase 1.5: Advanced Nodes (Field, Path, Observation)

**Outcome:** Inference and pathfinding work.

**Files to Create:**
```
crates/studio_core/src/markov_junior/
├── field.rs            # Distance field computation
├── observation.rs      # Future constraints + potential propagation
├── search.rs           # A* search through state space
└── path_node.rs        # Dijkstra pathfinding
```

**Priority Order:**
1. `field.rs` - BFS distance computation (needed by OneNode heuristics)
2. `path_node.rs` - Dijkstra for path generation
3. `observation.rs` - Constraint propagation
4. `search.rs` - Full A* (complex, can defer)

**Phase 1.5 Verification:**
```rust
#[test]
fn test_field_compute() {
    // Grid with B's and one W
    // Field from W should show distances
}

#[test]
fn test_path_node_connects() {
    // Start at corner, goal at opposite corner
    // Path should connect them
}

#[test]
fn test_dijkstra_dungeon_matches_csharp() {
    // Compare against C# reference
}
```

---

### Phase 1.6: WFC Nodes

**Outcome:** Wave Function Collapse works.

**Files to Create:**
```
crates/studio_core/src/markov_junior/
├── wfc/
│   ├── mod.rs
│   ├── wave.rs         # Wave state tracking
│   ├── wfc_node.rs     # Base WFC implementation
│   ├── tile_node.rs    # Tile-based WFC
│   └── overlap_node.rs # Overlapping WFC
```

**Tasks:**
1. `wave.rs` - Wave struct with compatible counts, entropy
2. `wfc_node.rs` - Observe, Propagate, Ban operations
3. `tile_node.rs` - Load tilesets, build propagator
4. `overlap_node.rs` - Extract patterns from sample, build propagator

**Phase 1.6 Verification:**
```rust
#[test]
fn test_wfc_propagate_no_contradiction() {
    // Simple 2-tile case should resolve
}

#[test]
fn test_tile_wfc_knots() {
    let mut model = Model::load("MarkovJunior/models/Knots3D.xml").unwrap();
    model.run(123);
    // Verify no contradiction (all cells resolved)
}
```

---

### Phase 1.7: Convolution & ConvChain

**Outcome:** Cellular automata and MCMC texture synthesis work.

**Files to Create:**
```
crates/studio_core/src/markov_junior/
├── convolution_node.rs  # Cellular automata
└── convchain_node.rs    # MCMC texture synthesis
```

**These are lower priority but included for completeness.**

---

## Step 2: Lua Integration

### When to Integrate Lua

**EARLIEST POINT: After Phase 1.3 (Interpreter works programmatically)**

At this point we have:
- Working Grid, Rule, Node types
- Interpreter that can execute models
- Programmatic API for creating models

Lua can be integrated to:
1. Define models in Lua instead of XML
2. Provide callbacks during execution
3. Enable runtime model modification

### Phase 2.1: Lua Model Definition DSL

**Outcome:** Can define MarkovJunior models in Lua.

**File:** `crates/studio_core/src/markov_junior/lua_api.rs`

**API Design:**
```lua
-- Define a model
local model = mj.create_model({
    values = "BRGW",
    size = {32, 32, 1},
    origin = true
})

-- Define rules inline
model:markov(function()
    model:one("RBB", "GGR")
    model:one("RGG", "WWR")
end)

-- Or use XML
local maze = mj.load_model("models/MazeBacktracker.xml")

-- Run with seed
local grid = model:run(12345)

-- Get result as table
local voxels = grid:to_voxels()
```

**Rust Implementation:**
```rust
pub fn register_markov_junior_api(lua: &Lua) -> LuaResult<()> {
    let mj = lua.create_table()?;
    
    mj.set("create_model", lua.create_function(|_, config: LuaTable| {
        // Parse config, create MjGrid
    })?)?;
    
    mj.set("load_model", lua.create_function(|_, path: String| {
        // Load from XML
    })?)?;
    
    lua.globals().set("mj", mj)?;
    Ok(())
}
```

**Phase 2.1 Verification:**
```lua
-- test_mj_basic.lua
local model = mj.create_model({ values = "BW", size = {10, 10, 1} })
model:one("B", "W")
local grid = model:run(42, 50)
assert(grid:count_value("W") > 40, "Should have many white cells")
```

---

### Phase 2.2: Execution Callbacks

**Outcome:** Lua can hook into execution for visualization/control.

**API:**
```lua
model:run_animated({
    seed = 12345,
    on_step = function(grid, step)
        -- Called after each step
        -- Can update visualization
        if step % 10 == 0 then
            visualize(grid)
        end
    end,
    on_complete = function(grid)
        print("Done after " .. grid.steps .. " steps")
    end
})
```

**Phase 2.2 Verification:**
```lua
local steps_seen = 0
model:run_animated({
    on_step = function() steps_seen = steps_seen + 1 end
})
assert(steps_seen > 0, "Should have seen steps")
```

---

### Phase 2.3: Integration with Existing Lua Infrastructure

**Outcome:** MarkovJunior available in `studio_scripting` context.

**Changes to `crates/studio_scripting/src/lib.rs`:**
```rust
fn register_lua_api(lua: &Lua) -> LuaResult<()> {
    // ... existing imgui, scene APIs ...
    
    // Add MarkovJunior API
    studio_core::markov_junior::register_lua_api(lua)?;
    
    Ok(())
}
```

**Verification:** Hot-reload a script that uses `mj.*` functions.

---

## Step 3: VoxelWorld Integration

### Phase 3.1: Grid to VoxelWorld Conversion

**Outcome:** Can convert MjGrid output to VoxelWorld.

**File:** `crates/studio_core/src/markov_junior/voxel_bridge.rs`

**API:**
```rust
impl MjGrid {
    /// Convert to VoxelWorld using a color palette.
    pub fn to_voxel_world(&self, palette: &MjPalette) -> VoxelWorld {
        let mut world = VoxelWorld::new();
        for z in 0..self.MZ {
            for y in 0..self.MY {
                for x in 0..self.MX {
                    let value = self.state[x + y * self.MX + z * self.MX * self.MY];
                    if let Some(voxel) = palette.get_voxel(value) {
                        world.set_voxel(x as i32, y as i32, z as i32, voxel);
                    }
                }
            }
        }
        world
    }
}

/// Palette mapping MJ values to voxel colors.
pub struct MjPalette {
    colors: HashMap<u8, Voxel>,
}

impl MjPalette {
    pub fn from_xml(path: &str) -> Self { /* load palette.xml */ }
    pub fn pico8() -> Self { /* default PICO-8 palette */ }
}
```

**Lua API:**
```lua
local grid = model:run(42)
local world = grid:to_voxel_world(mj.palette.pico8())

-- Or with custom palette
local palette = mj.palette.create({
    B = {0, 0, 0},      -- black
    W = {255, 255, 255}, -- white
    R = {255, 0, 0},     -- red, emissive
})
```

**Phase 3.1 Verification:**
```rust
#[test]
fn test_grid_to_voxel_world() {
    let mut grid = MjGrid::new(4, 4, 1, "BW");
    grid.state[0] = 1; // W at (0,0,0)
    
    let palette = MjPalette::default();
    let world = grid.to_voxel_world(&palette);
    
    assert!(world.get_voxel(0, 0, 0).is_some());
    assert!(world.get_voxel(1, 0, 0).is_none()); // B = transparent
}
```

---

### Phase 3.2: Example - p25_markov_junior.rs

**Outcome:** Working example that generates and displays MarkovJunior output.

**File:** `examples/p25_markov_junior.rs`

```rust
//! MarkovJunior Procedural Generation Demo
//!
//! Generates a 3D maze using MarkovJunior and renders it as voxels.
//!
//! Run with: `cargo run --example p25_markov_junior`

use bevy::prelude::*;
use studio_core::{
    markov_junior::{Model, MjPalette},
    voxel_mesh::build_voxel_world_meshes,
    VoxelWorld,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(studio_core::VoxelWorldPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Load and run MarkovJunior model
    let mut model = Model::load("MarkovJunior/models/MazeGrowth.xml")
        .expect("Failed to load model");
    
    // Set 3D dimensions
    model.resize(27, 27, 27);
    
    // Run with random seed
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    model.run(seed);
    
    // Convert to VoxelWorld
    let palette = MjPalette::pico8();
    let world = model.grid.to_voxel_world(&palette);
    
    println!("Generated {} voxels", world.total_voxel_count());
    
    // Build and spawn meshes
    let mesh_data = build_voxel_world_meshes(&world);
    // ... spawn mesh entities ...
    
    // Camera and lighting
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(40.0, 40.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
```

**Phase 3.2 Verification:**
```bash
cargo run --example p25_markov_junior
# Visual: 3D maze appears on screen
```

---

### Phase 3.3: Animated Execution in Bevy

**Outcome:** Can watch MarkovJunior execute step-by-step in realtime.

**Design:**
```rust
#[derive(Resource)]
pub struct MjAnimation {
    model: Model,
    world: VoxelWorld,
    steps_per_frame: usize,
    is_running: bool,
}

fn mj_step_system(
    mut animation: ResMut<MjAnimation>,
    mut world_changed: EventWriter<VoxelWorldChanged>,
) {
    if !animation.is_running { return; }
    
    for _ in 0..animation.steps_per_frame {
        if !animation.model.step() {
            animation.is_running = false;
            break;
        }
    }
    
    // Update VoxelWorld from MjGrid
    animation.world = animation.model.grid.to_voxel_world(&palette);
    world_changed.send(VoxelWorldChanged);
}
```

**Phase 3.3 Verification:**
```bash
cargo run --example p26_markov_animated
# Visual: Watch maze grow step by step
```

---

### Phase 3.4: Lua-Driven Example

**Outcome:** Can control MarkovJunior from Lua script.

**File:** `assets/scripts/markov_demo.lua`

```lua
-- MarkovJunior Demo Script
function on_load()
    -- Create a simple growth model
    model = mj.create_model({
        values = "BW",
        size = {32, 32, 32},
        origin = true
    })
    
    model:one("WB", "WW")  -- Growth rule
    
    scene.print("MarkovJunior model created")
end

function on_draw()
    imgui.window("MarkovJunior Controls", function()
        if imgui.button("Generate") then
            local seed = math.random(1, 999999)
            model:run(seed, 5000)
            
            local world = model:to_voxel_world()
            scene.set_voxel_world(world)
            
            scene.print("Generated with seed " .. seed)
        end
        
        if imgui.button("Step x100") then
            model:step(100)
            scene.set_voxel_world(model:to_voxel_world())
        end
    end)
end
```

**Phase 3.4 Verification:**
```bash
cargo run --example p27_markov_lua_demo
# Click "Generate" button, see voxel world update
```

---

## Test Data Generation

### Reference Data from C#

Create a script to generate reference test data:

```bash
#!/bin/bash
# generate_test_data.sh

cd MarkovJunior

# Modify Program.cs to output binary grid state
# Then run for each test case:

models=("Basic" "MazeBacktracker" "MazeGrowth" "Growth" "NystromDungeon")
seeds=(12345 42 99999 1 54321)

for model in "${models[@]}"; do
    for seed in "${seeds[@]}"; do
        dotnet run -- --model "$model" --seed "$seed" \
            --output "../test_data/${model}_${seed}.bin"
    done
done
```

### Binary Format
Simple format: raw bytes of grid.state array, prefixed with MX, MY, MZ as u32.

```rust
// Writing (C#)
writer.Write(MX);
writer.Write(MY);
writer.Write(MZ);
writer.Write(state);

// Reading (Rust)
fn load_reference(path: &str) -> (usize, usize, usize, Vec<u8>) {
    let data = std::fs::read(path).unwrap();
    let mx = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let my = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let mz = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    let state = data[12..].to_vec();
    (mx, my, mz, state)
}
```

---

## Dependency Summary

### New Crate Dependencies

```toml
# Cargo.toml additions for studio_core

[dependencies]
quick-xml = "0.31"        # XML parsing
rand = "0.8"              # Already present
rand_chacha = "0.3"       # Deterministic RNG for reproducibility
```

### Module Structure

```
crates/studio_core/src/
├── markov_junior/
│   ├── mod.rs
│   ├── grid.rs
│   ├── rule.rs
│   ├── symmetry.rs
│   ├── array_helper.rs
│   ├── node.rs
│   ├── rule_node.rs
│   ├── one_node.rs
│   ├── all_node.rs
│   ├── parallel_node.rs
│   ├── interpreter.rs
│   ├── execution_context.rs
│   ├── loader.rs
│   ├── model.rs
│   ├── field.rs
│   ├── observation.rs
│   ├── search.rs
│   ├── path_node.rs
│   ├── wfc/
│   │   ├── mod.rs
│   │   ├── wave.rs
│   │   ├── wfc_node.rs
│   │   ├── tile_node.rs
│   │   └── overlap_node.rs
│   ├── convolution_node.rs
│   ├── convchain_node.rs
│   ├── lua_api.rs
│   └── voxel_bridge.rs
```

---

## Milestone Summary

| Phase | Description | Verification | Est. LOC |
|-------|-------------|--------------|----------|
| 1.1 | Foundation (Grid, Rule, Symmetry) | Unit tests | ~600 |
| 1.2 | Node Infrastructure | Unit tests | ~500 |
| 1.3 | Interpreter | C# cross-validation | ~300 |
| 1.4 | XML Loading | Load real models | ~400 |
| 1.5 | Field, Path, Observation | C# cross-validation | ~600 |
| 1.6 | WFC Nodes | C# cross-validation | ~800 |
| 1.7 | Convolution, ConvChain | Unit tests | ~300 |
| 2.1 | Lua Model DSL | Lua tests | ~300 |
| 2.2 | Execution Callbacks | Lua tests | ~150 |
| 2.3 | Studio Integration | Hot-reload test | ~50 |
| 3.1 | VoxelWorld Bridge | Unit tests | ~150 |
| 3.2 | Static Example | Visual | ~100 |
| 3.3 | Animated Example | Visual | ~150 |
| 3.4 | Lua-Driven Example | Visual + interactive | ~100 |

**Total estimated: ~4500 lines of Rust** (comparable to C# ~4333 lines)

---

## Success Criteria

### Phase 1 Complete When:
- [ ] All unit tests pass
- [ ] Can load and run Basic, MazeBacktracker, MazeGrowth, NystromDungeon
- [ ] Output matches C# reference for all test seeds

### Phase 2 Complete When:
- [ ] Can define models in Lua
- [ ] Hot-reload works for Lua model definitions
- [ ] Callbacks fire during execution

### Phase 3 Complete When:
- [ ] VoxelWorld correctly populated from MjGrid
- [ ] Example renders 3D maze
- [ ] Animated example shows step-by-step execution
- [ ] Lua script can trigger generation and see results

---

## Risks & Mitigations

1. **RNG Divergence**
   - Risk: Rust and C# produce different random sequences
   - Mitigation: Use ChaCha8 in both, verify sequence matches

2. **Floating Point Differences**
   - Risk: Temperature/heuristic calculations differ slightly
   - Mitigation: Use exact integer comparison where possible

3. **XML Parsing Edge Cases**
   - Risk: Some models use obscure XML features
   - Mitigation: Start with simple models, expand coverage

4. **Performance**
   - Risk: Rust version slower than C#
   - Mitigation: Profile early, optimize hot paths (pattern matching)

---

## Next Steps

1. **Create test data generator** for C# reference outputs
2. **Start Phase 1.1** with Grid implementation
3. **Set up CI** to run cross-validation tests
