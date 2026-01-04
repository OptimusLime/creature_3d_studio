# MarkovJunior Rust Implementation Plan

This document outlines the phased approach to porting MarkovJunior to Rust with Lua integration, following our HOW_WE_WORK principles of incremental building with verification.

## Summary

Port MarkovJunior procedural generation system from C# to Rust, integrate with our Lua scripting, and connect output to VoxelWorld for rendering in Bevy.

## Context & Motivation

MarkovJunior is a probabilistic programming language for procedural generation using rewrite rules. We want to use it to generate voxel structures (dungeons, mazes, terrain) that can be rendered in our engine.

## Naming Conventions for This PR

- **Module:** `markov_junior` (matches original project name)
- **Structs:** `MjGrid`, `MjRule`, `MjPalette` (Mj prefix to avoid conflicts)
- **Files:** snake_case matching struct names (`mj_grid.rs`, `one_node.rs`)
- **Tests:** `test_<function>_<scenario>` (e.g., `test_wave_single_value`)

## Key Integration Points
- `VoxelWorld` in `crates/studio_core/src/voxel.rs` - Our voxel storage
- `creature_script.rs` - Existing Lua voxel placement pattern
- `studio_scripting/src/lib.rs` - Lua VM with hot-reload

---

## Phase 0: End-to-End Skeleton

**Outcome:** Minimal working pipeline from hardcoded model to VoxelWorld, rendered on screen.

**Verification:** Run `cargo run --example p25_markov_junior`, see a 3x3x1 white cross pattern on black background, screenshot saved to `screenshots/p25_markov_junior.png`.

**Tasks:**

1. Create `crates/studio_core/src/markov_junior/mod.rs` with placeholder `MjGrid` struct:
   ```rust
   pub struct MjGrid { pub state: Vec<u8>, pub mx: usize, pub my: usize, pub mz: usize }
   ```

2. Create `crates/studio_core/src/markov_junior/voxel_bridge.rs` with `to_voxel_world()` that maps value 0=empty, 1=white voxel.

3. Create `examples/p25_markov_junior.rs` that:
   - Creates a hardcoded 5x5x1 MjGrid with a cross pattern (center + 4 adjacent = value 1)
   - Converts to VoxelWorld
   - Renders with camera at (10, 10, 10) looking at origin
   - Takes screenshot after 5 frames

4. Add `pub mod markov_junior;` to `crates/studio_core/src/lib.rs`

**This phase proves the pipeline works before adding algorithm complexity.**

---

## Phase 1: Foundation Data Structures

**Outcome:** Grid and Rule structs compile, parse patterns, and match correctly.

**Verification:** Run `cargo test -p studio_core markov_junior` and see:
- `test_grid_wave_bw ... ok` (grid.wave("B") == 1, grid.wave("W") == 2, grid.wave("BW") == 3)
- `test_grid_matches_rule ... ok` (rule "B" matches at (0,0,0) on grid starting with all B's)
- `test_rule_parse_2d ... ok` (parse "RB/WW" produces input array of length 4)
- `test_symmetry_square_8 ... ok` (square_symmetries returns exactly 8 unique variants)

**Tasks:**

1. Create `crates/studio_core/src/markov_junior/grid.rs`:
   - Struct `MjGrid` with fields: `state: Vec<u8>`, `mx: usize`, `my: usize`, `mz: usize`, `c: u8` (color count), `values: HashMap<char, u8>`, `waves: HashMap<char, u32>`
   - Method `fn new(mx, my, mz, values_str: &str) -> Self` that parses "BRGW" into mappings
   - Method `fn wave(&self, chars: &str) -> u32` returning bitmask
   - Method `fn matches(&self, rule: &MjRule, x: i32, y: i32, z: i32) -> bool`
   - Test `test_grid_wave_bw`: create grid with "BW", assert wave("B")==1, wave("W")==2, wave("BW")==3

2. Create `crates/studio_core/src/markov_junior/rule.rs`:
   - Struct `MjRule` with fields: `input: Vec<u32>`, `output: Vec<u8>`, `imx/imy/imz: usize`, `omx/omy/omz: usize`, `p: f64`
   - Function `fn parse(input_str: &str, output_str: &str, grid: &MjGrid) -> Result<MjRule>` where `/` = Y separator, ` ` = Z separator
   - Test `test_rule_parse_2d`: parse "RB/WW" -> "GG/RR", verify input.len()==4, imx==2, imy==2

3. Create `crates/studio_core/src/markov_junior/symmetry.rs`:
   - Function `fn square_symmetries<T>(thing: T, rotate: fn, reflect: fn, same: fn) -> Vec<T>` returning up to 8 variants
   - Test `test_symmetry_square_8`: pass identity rule, get 8 results (or fewer if symmetric)

4. Update `crates/studio_core/src/markov_junior/mod.rs` to export all types.

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

**Phase 1.2 Verification:** Run `cargo test -p studio_core markov_junior::node` and see:
- `test_one_node_applies_single_match ... ok` (5x1 grid "BBBBB" with rule B→W, after 1 step exactly 1 cell is W)
- `test_all_node_fills_entire_grid ... ok` (5x1 grid "BBBBB" with rule B→W, after 1 step all 5 cells are W)
- `test_all_node_non_overlapping ... ok` (5x1 grid with rule BB→WW, after 1 step exactly 4 cells are W, 1 remains B)
- `test_markov_node_loops_until_done ... ok` (MarkovNode with B→W rule, runs until no matches, all cells become W)
- `test_sequence_node_runs_in_order ... ok` (SequenceNode with [B→R, R→W], final grid all W)

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

**Phase 1.3 Verification:** Run `cargo test -p studio_core markov_junior::interpreter` and see:
- `test_interpreter_step_returns_false_when_done ... ok` (interpreter.step() returns false after model completes)
- `test_interpreter_run_with_max_steps ... ok` (run(seed, 10) stops after exactly 10 steps even if not done)
- `test_basic_model_matches_reference ... ok` (see cross-validation below)

**C# Cross-Validation Setup:**

1. Generate reference data (one-time setup):
   ```bash
   cd MarkovJunior && dotnet build
   # Add to Program.cs: --dump-state flag that writes grid.state to binary file
   dotnet run -- Basic 12345 --dump-state ../crates/studio_core/src/markov_junior/test_data/basic_12345.bin
   ```

2. Binary format: `[MX:u32][MY:u32][MZ:u32][state:u8[MX*MY*MZ]]` little-endian

3. Test implementation:
   ```rust
   #[test]
   fn test_basic_model_matches_reference() {
       let expected = include_bytes!("test_data/basic_12345.bin");
       let (mx, my, mz, ref_state) = parse_reference(expected);
       
       let mut interp = Interpreter::new_basic_model(); // hardcoded Basic model
       interp.run(12345, 10000);
       
       assert_eq!(interp.grid.mx, mx);
       assert_eq!(interp.grid.state, ref_state, "Grid state mismatch vs C# reference");
   }
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

**Phase 1.4 Verification:** Run `cargo test -p studio_core markov_junior::loader` and see:
- `test_load_basic_xml ... ok` (loads `MarkovJunior/models/Basic.xml`, grid.c == 2, values contains 'B' and 'W')
- `test_load_maze_backtracker_xml ... ok` (loads model, root is MarkovNode with 2 OneNode children)
- `test_load_missing_file_returns_error ... ok` (Model::load("nonexistent.xml") returns Err)
- `test_maze_backtracker_matches_reference ... ok` (run with seed 42, compare to `test_data/maze_backtracker_42.bin`)

**Reference files to generate:**
```bash
dotnet run -- MazeBacktracker 42 --dump-state ../crates/studio_core/src/markov_junior/test_data/maze_backtracker_42.bin
```

---

### Phase 1.5: Field, Path, and Heuristic Selection (COMPLETE)

**Outcome:** Distance fields, pathfinding, and heuristic-guided rule selection work.

**Status:** COMPLETE (92 tests passing)

**Files Created:**
```
crates/studio_core/src/markov_junior/
├── field.rs            # Field struct, BFS distance, delta_pointwise()
└── path_node.rs        # PathNode - Dijkstra pathfinding
```

**Files Modified:**
- `rule_node.rs` - Added potentials, fields, temperature
- `one_node.rs` - Added heuristic selection mode
- `all_node.rs` - Added heuristic sorting mode
- `loader.rs` - Added <field> and <path> parsing

**Verification:** All tests pass:
- `test_field_bfs_simple`, `test_field_bfs_with_obstacles`
- `test_path_node_simple`, `test_path_node_no_path`, `test_path_node_with_inertia`
- `test_one_node_heuristic_selection`, `test_all_node_heuristic_sorting`
- `test_load_biased_growth_xml` (uses fields)

---

### Phase 1.6: File Attribute, Union Types, and MapNode (COMPLETE)

**Outcome:** Can load PNG-based rules, use union types, and transform grids.

**Status:** COMPLETE (111 tests passing)

**Files Created/Modified:**
```
crates/studio_core/src/markov_junior/
├── helper.rs           # PNG loading utilities (~200 lines)
├── map_node.rs         # MapNode - grid transformation (~250 lines)
└── loader.rs           # Added file, union, map parsing (~200 lines added)
```

**What Was Implemented:**

1. **PNG Rule Loading (`file` attribute)**
   - `load_bitmap()` - PNG loading with `image` crate
   - `ords()` - Pixel to ordinal mapping
   - `load_resource()` - Combined loading with legend
   - `split_rule_image()` - Split into input/output halves
   - `LoadContext` for resource path management

2. **Union Types (`<union>` elements)**
   - `parse_union_elements()` scans document for unions
   - Adds combined wave to grid.waves
   - Duplicate symbol detection

3. **MapNode (grid transformation)**
   - `ScaleFactor` for fractional scaling
   - `MapNode` with toroidal wrapping
   - Child node execution on transformed grid
   - `load_map_node()` and `load_map_rules_from_xml()`

**Phase 1.6 Verification (All Pass):**
- `test_load_file_rule_from_png ... ok`
- `test_union_creates_combined_wave ... ok`
- `test_map_node_simple_2x_scale ... ok`
- `test_load_basic_dijkstra_dungeon ... ok`
- `test_load_dungeon_growth_xml ... ok`
- `test_load_maze_map_xml ... ok`

**Models Now Working:**
- BasicDijkstraDungeon.xml (uses `file` attribute)
- DungeonGrowth.xml (uses `file` and `<union>`)
- MazeMap.xml (uses `<map>` node)
- All models with PNG rules, unions, or map nodes

---

### Phase 1.7: Observation, Search, and Trajectory (COMPLETE)

**Outcome:** Constraint propagation and A* search work.

**Status:** COMPLETE (133 tests passing)

**Files Created/Modified:**
```
crates/studio_core/src/markov_junior/
├── observation.rs      # Observation struct, constraint propagation (~200 lines)
├── search.rs           # A* search, Board struct, trajectory (~350 lines)
├── rule_node.rs        # Added observations, future, search fields
└── loader.rs           # Added observe/search parsing (~100 lines added)
```

**What Was Implemented:**

1. **Observation (`observation.rs`)** ~200 lines
   - `Observation` struct with `from: u8`, `to: u32` wave mask
   - `compute_future_set_present()` - set future constraints from current state
   - `compute_forward_potentials()` - BFS propagation from current state
   - `compute_backward_potentials()` - BFS propagation backward from goal
   - `is_goal_reached()` - check if current state satisfies future
   - `forward_pointwise()` / `backward_pointwise()` - heuristic estimates

2. **Search (`search.rs`)** ~350 lines
   - `Board` struct for A* search nodes
   - `run_search()` - A* search with priority queue
   - `one_child_states()` / `all_child_states()` - child state generation
   - `state_hash()` for visited set deduplication

3. **Loader updates** ~100 lines
   - `load_observations_from_xml()` - parse `<observe>` elements
   - `parse_observe_element()` - parse single observe
   - Added `search`, `limit`, `depthCoefficient` attribute parsing

**Phase 1.7 Verification (All Pass):**
- `test_observation_new ... ok`
- `test_compute_future_set_present ... ok`
- `test_compute_backward_potentials_simple ... ok`
- `test_compute_forward_potentials_simple ... ok`
- `test_is_goal_reached ... ok`
- `test_forward_pointwise ... ok`
- `test_backward_pointwise ... ok`
- `test_search_finds_solution_simple ... ok`
- `test_search_already_at_goal ... ok`
- `test_search_no_solution ... ok`
- `test_search_with_limit ... ok`
- `test_load_model_with_observe ... ok`
- `test_load_model_with_search ... ok`

**Models Enabled After Phase 1.7:**
- Models with `<observe>` elements (parsing complete)
- Models with `search="True"` attribute (parsing complete)

---

### Phase 1.8: WFC Nodes (COMPLETE)

**Outcome:** Wave Function Collapse works for both tile and overlap modes.

**Status:** COMPLETE (173 tests passing)

**Files Created:**
```
crates/studio_core/src/markov_junior/
├── wfc/
│   ├── mod.rs          # WFC module exports (19 lines)
│   ├── wave.rs         # Wave state tracking (423 lines)
│   ├── wfc_node.rs     # Base WFC algorithms (796 lines)
│   ├── overlap_node.rs # Overlapping WFC (605 lines)
│   └── tile_node.rs    # Tile-based WFC (693 lines)
```

**What Was Implemented:**

1. **Wave State (`wave.rs`)** 423 lines
   - `Wave` struct: `data`, `compatible`, `sums_of_ones`
   - Shannon entropy support: `sums_of_weights`, `entropies`
   - Methods: `new()`, `init()`, `copy_from()`, `get_data()`, `set_data()`
   - 12 unit tests

2. **WfcNode (`wfc_node.rs`)** 796 lines
   - Core WFC algorithms: `initialize()`, `step()`, `ban()`, `propagate()`
   - `observe()`, `next_unobserved_node()`, `good_seed()`
   - `WfcState` enum for state tracking
   - Implements `Node` trait
   - 8 unit tests

3. **OverlapNode (`overlap_node.rs`)** 605 lines
   - Pattern extraction from sample PNG
   - Symmetry generation: `rotate_pattern()`, `reflect_pattern()`
   - Propagator: `build_overlap_propagator()`, `patterns_agree()`
   - Voting-based `update_state()`
   - 8 unit tests

4. **TileNode (`tile_node.rs`)** 693 lines
   - Tileset XML parsing
   - Propagator from neighbor constraints
   - Symmetry: `square_symmetries_3d()`, `cube_symmetries()` (stubbed)
   - **NOTE:** VOX loading stubbed - deferred to Phase 1.10
   - 6 unit tests

5. **Loader updates** ~230 lines
   - `load_wfc_node()`, `load_overlap_node()`, `load_tile_node()`
   - `load_wfc_rules_from_xml()`, `load_tile_rules_from_xml()`
   - `LoadContext.sample_path()`, `tileset_path()`
   - 5 integration tests

**Phase 1.8 Verification (All Pass):**
- `test_wave_entropy_calculation ... ok`
- `test_wfc_propagate_reduces_possibilities ... ok`
- `test_wfc_contradiction_detected ... ok`
- `test_wfc_complete_simple ... ok`
- `test_load_wfc_overlap_wave_flowers ... ok`
- `test_wfc_overlap_model_runs ... ok` (end-to-end execution)

**Models Enabled After Phase 1.8:**
- WaveFlowers.xml (verified working)
- All WFC models with `<wfc sample="...">` 
- TileNode parsing works (VOX loading deferred to 1.10)

---

### Phase 1.9: Convolution & ConvChain (COMPLETE)

**Outcome:** Cellular automata and MCMC texture synthesis work.

**Status:** COMPLETE (218 tests passing)

**Files Created:**
```
crates/studio_core/src/markov_junior/
├── convolution_node.rs  # Cellular automata rules (~811 lines)
└── convchain_node.rs    # MCMC texture synthesis (~950 lines)
```

**What Was Implemented:**

1. **ConvolutionNode (`convolution_node.rs`)** ~811 lines
   - `ConvolutionRule` struct with `input`, `output`, `values`, `sums`, `p`
   - `ConvolutionNode` struct with `rules`, `kernel`, `periodic`, `counter`, `steps`, `sumfield`
   - Pre-defined kernels: VonNeumann (4/6), Moore (8), NoCorners (18)
   - `compute_sumfield()` - neighbor counting per cell per color
   - `parse_sum_intervals()` - parses "5..8" or "2,5..7"
   - 15 unit tests including verification tests

2. **ConvChainNode (`convchain_node.rs`)** ~950 lines
   - `ConvChainNode` struct with `n`, `temperature`, `weights`, `c0`, `c1`, `substrate`
   - `from_sample()` - load and learn from sample image
   - `learn_pattern_weights()` - NxN pattern extraction with symmetry
   - MCMC Metropolis-Hastings Go() with quality ratio calculation
   - 16 unit tests including:
     - `test_weight_learning_from_sample` - verifies pattern learning
     - `test_output_patterns_have_positive_weights` - output validity
     - `test_mcmc_converges_to_high_weight_patterns` - convergence
     - `test_quality_ratio_calculation_correctness` - detailed balance
     - `test_convchain_end_to_end_with_sample_file` - real sample file

3. **Loader updates** ~110 lines
   - `load_convolution_node()` - parses `<convolution>` with `neighborhood`, `periodic`, `steps`
   - `load_convolution_rules_from_xml()` - parses `<rule in="A" out="D" sum="5..8" values="D"/>`
   - `load_convchain_node()` - parses `<convchain sample="..." on="B" black="D" white="A"/>`
   - 11 loader tests (6 convolution + 6 convchain)

**Phase 1.9 Verification (All Pass):**
- `test_convolution_game_of_life_rules ... ok`
- `test_convolution_neighbor_count_verification ... ok`
- `test_convolution_sumfield_correctness ... ok`
- `test_convchain_end_to_end_with_sample_file ... ok`
- `test_mcmc_converges_to_high_weight_patterns ... ok`
- `test_load_convchain_chain_maze_xml ... ok`

**Models Enabled After Phase 1.9:**
- Cave.xml (convolution with Moore kernel)
- ChainMaze.xml (convchain with Maze sample)
- ChainDungeon.xml, ChainDungeonMaze.xml
- All models using `<convolution>` rules
- All models using `<convchain>` texture synthesis (2D only)

---

### Phase 1.10: 3D Symmetries & VOX Loading (COMPLETE)

**Outcome:** Full 3D symmetry support and VOX file loading for voxel models.

**Status:** COMPLETE (237 tests passing)

**Files Modified:**
```
crates/studio_core/src/markov_junior/
├── symmetry.rs         # Added CubeSymmetries, cube subgroups, get_symmetry()
├── rule.rs             # Added y_rotated() method
├── helper.rs           # Added VOX file loading (load_vox, load_vox_ords, load_vox_resource)
└── wfc/tile_node.rs    # Replaced stubs with real VOX loading
```

**What Was Implemented:**

1. **y_rotated() (`rule.rs`)** ~50 lines
   - 90-degree rotation around Y axis for 3D rules
   - Dimensions swap: (IMX, IMY, IMZ) -> (IMZ, IMY, IMX)
   - C# Reference: `Rule.cs` YRotated() lines 76-87

2. **cube_subgroups() (`symmetry.rs`)** ~40 lines
   - Named subgroups: "()", "(x)", "(z)", "(xy)", "(xyz+)", "(xyz)"
   - HashMap lookup matching C# `cubeSubgroups` dictionary

3. **cube_symmetries() (`symmetry.rs`)** ~100 lines
   - 48-element 3D symmetry group
   - Uses a (Z-rotate), b (Y-rotate), r (reflect) operations
   - Generates all 48 variants and deduplicates

4. **get_symmetry() (`symmetry.rs`)** ~15 lines
   - Unified 2D/3D symmetry lookup by name
   - Returns Vec<bool> mask for subgroup

5. **load_vox() (`helper.rs`)** ~120 lines
   - Parse MagicaVoxel .vox files
   - Handle SIZE and XYZI chunks
   - Return (voxels, mx, my, mz)

6. **load_vox_ords() (`helper.rs`)** ~25 lines
   - Convert voxel palette indices to ordinals
   - Handle empty voxels (0xff marker)

7. **get_tile_size() (`wfc/tile_node.rs`)** ~15 lines
   - Replaced stub with real VOX header reading
   - Requires square XY dimensions

8. **load_vox_tile() (`wfc/tile_node.rs`)** ~40 lines
   - Replaced stub with real VOX loading
   - Maps palette indices to global ordinals

9. **cube_symmetries() (`wfc/tile_node.rs`)** ~80 lines
   - Full 48-element group for cubic tiles
   - Falls back to square symmetries for non-cubic
   - Added y_rotate() for tile rotation

**Phase 1.10 Verification (All Pass):**
- `test_rule_y_rotated ... ok`
- `test_rule_y_rotated_asymmetric ... ok`
- `test_cube_subgroups_defined ... ok`
- `test_cube_symmetries_generates_48 ... ok`
- `test_cube_symmetries_identity_subgroup ... ok`
- `test_cube_symmetries_asymmetric_rule ... ok`
- `test_get_symmetry_2d ... ok`
- `test_get_symmetry_3d ... ok`
- `test_get_symmetry_default ... ok`
- `test_load_vox_escher_empty ... ok`
- `test_load_vox_escher_line ... ok`
- `test_load_vox_ords ... ok`
- `test_load_vox_nonexistent ... ok`
- `test_load_vox_invalid_format ... ok`
- `test_get_tile_size_real_vox ... ok`
- `test_load_vox_tile_real ... ok`
- `test_cube_symmetries_cubic_tile ... ok`
- `test_cube_symmetries_symmetric_tile ... ok`
- `test_y_rotate ... ok`

**Models Enabled After Phase 1.10:**
- All 3D models with symmetry attributes
- 3D dungeon/cave generation (e.g., Dungeon3D.xml)
- TileNode with actual VOX tilesets
- Complete C# feature parity for core algorithm

**C# Reference Files:**
- `SymmetryHelper.cs` lines 37-104 (cube symmetries)
- `Rule.cs` lines 76-87 (YRotated)
- `VoxHelper.cs` (VOX loading)
- `TileModel.cs` lines 50-90 (tile loading)

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

**Phase 3.1 Verification:** Run `cargo test -p studio_core markov_junior::voxel_bridge` and see:
- `test_grid_to_voxel_world_maps_values ... ok` (4x4x1 grid with W at (0,0,0), world.get_voxel(0,0,0) returns Some with white color)
- `test_grid_to_voxel_world_skips_transparent ... ok` (B value at (1,0,0) maps to None/empty in VoxelWorld)
- `test_palette_pico8_has_16_colors ... ok` (MjPalette::pico8() returns palette with 16 entries)
- `test_palette_maps_char_to_color ... ok` (palette.get_voxel('R') returns red voxel)

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
1. Run `cargo run --example p25_markov_junior`
2. Screenshot saved to `screenshots/p25_markov_junior.png`
3. Verify screenshot shows:
   - 3D structure visible (not black screen)
   - Multiple voxels present (maze walls)
   - Camera positioned to see entire structure
4. Console output includes "Generated N voxels" where N > 100

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
1. Run `cargo run --example p26_markov_animated`
2. Verify visual behavior:
   - Initial frame shows only seed voxel(s)
   - Structure grows over multiple frames (not instant)
   - Press Space to pause/resume animation
   - Console prints "Step N: M voxels" showing incremental growth
3. Screenshot at frame 100 saved to `screenshots/p26_markov_animated_100.png`

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
1. Run `cargo run --example p27_markov_lua_demo`
2. Verify UI:
   - ImGui window titled "MarkovJunior Controls" appears
   - "Generate" button visible
   - "Step x100" button visible
3. Click "Generate":
   - Console prints "Generated with seed N"
   - Voxel world updates (not empty)
4. Modify `assets/scripts/markov_demo.lua`, save file:
   - Console prints hot-reload message
   - Next "Generate" click uses updated script
5. Screenshot after generate saved to `screenshots/p27_markov_lua.png`

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

| Phase | Description | Verification | Est. LOC | Status |
|-------|-------------|--------------|----------|--------|
| 1.1 | Foundation (Grid, Rule, Symmetry) | Unit tests | ~600 | COMPLETE |
| 1.2 | Node Infrastructure | Unit tests | ~500 | COMPLETE |
| 1.3 | Interpreter | C# cross-validation | ~300 | COMPLETE |
| 1.4 | XML Loading | Load real models | ~400 | COMPLETE |
| 1.5 | Field, Path, Heuristics | Unit tests | ~600 | COMPLETE |
| 1.6 | File attr, Union, MapNode | Load PNG rules | ~450 | COMPLETE |
| 1.7 | Observation, Search | C# cross-validation | ~650 | COMPLETE |
| 1.8 | WFC Nodes | Constraint validation | ~2536 | COMPLETE |
| 1.9 | Convolution, ConvChain | Unit tests | ~1761 | COMPLETE |
| 1.10 | 3D Symmetries & VOX | Unit tests | ~485 | COMPLETE |
| 2.1 | Lua Model DSL | Lua tests | ~720 | COMPLETE |
| 2.2 | Execution Callbacks | Lua tests | ~100 | COMPLETE |
| 2.3 | Studio Integration | Hot-reload test | ~50 | COMPLETE |
| 3.1 | VoxelWorld Bridge | Unit tests | ~150 | COMPLETE |
| 3.2 | Static Example | Visual | ~100 | PENDING |
| 3.3 | Animated Example | Visual | ~150 | PENDING |
| 3.4 | Lua-Driven Example | Visual + interactive | ~100 | PENDING |

**Phase 1 Total: ~8282 lines** (237 tests passing)
**Phase 2.1: ~720 lines** (15 tests passing)
**Phase 2.2: ~100 lines** (10 tests passing)
**Phase 2.3: ~50 lines Rust + ~100 lines Lua** 
**Running Total: ~9250 lines** (262 tests passing)

### Progress Summary

**Phase 1 COMPLETE!** All core algorithm features are implemented:

- Phase 1.1-1.4: Foundation, nodes, interpreter, XML loading
- Phase 1.5-1.6: Fields, paths, heuristics, PNG rules, unions, map nodes
- Phase 1.7: Observation, search, trajectory
- Phase 1.8: Wave Function Collapse (overlap and tile models)
- Phase 1.9: Convolution and ConvChain
- Phase 1.10: 3D symmetries and VOX loading

**Phase 2.1 COMPLETE!** Lua API for model creation and execution:

- mj.load_model() - load XML models from Lua
- mj.create_model() - programmatic model creation
- model:run(), :step(), :reset() - execution control
- grid:to_voxels(), :to_voxel_world() - voxel conversion

**Phase 2.2 COMPLETE!** Execution callbacks for animation:

- model:run_animated() - callback-based step-by-step execution
- model:changes() - access all changed positions
- model:last_changes() - access most recent step's changes

**Phase 2.3 COMPLETE!** Studio integration:

- mj.* API registered in studio_scripting
- scene.set_voxel_world() stores generated worlds
- GeneratedVoxelWorld resource for render systems
- main.lua updated with MarkovJunior demo window

**Next:** Phase 3 (Visual Examples)

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
