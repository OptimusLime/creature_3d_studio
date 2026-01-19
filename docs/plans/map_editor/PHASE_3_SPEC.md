# Phase 3 Specification: Advanced Generation (2D)

*Following HOW_WE_WORK.md and WRITING_MILESTONES.md.*

**Key principle:** Every task produces visible, verifiable functionality. Verification requires ZERO additional work—we look at a screenshot or check a file that already exists.

---

## Lessons Learned from Phase 2

### Patterns That Worked Well

| Pattern | Example | Keep For Phase 3 |
|---------|---------|------------------|
| **Foundation-first milestones** | M4.5 introduced `Asset` trait before M5/M6 needed it | Yes—introduce `MjGenerator` trait before M9 composed generators |
| **MCP endpoints for verification** | `curl /mcp/search?q=stone` verifies search works | Yes—add MCP endpoints for Markov state inspection |
| **Multi-instance registry** | `LuaLayerRegistry` manages N renderers/visualizers | Yes—extend registry for composed generators |
| **Hot-reload as default** | All Lua assets reload on file save | Yes—Markov XML models should hot-reload too |

### Planning Errors to Avoid

| Error | What Happened | Prevention |
|-------|---------------|------------|
| **Generator runs to completion immediately** | Phase 2 generators fill instantly, visualizer less useful | **Phase 3 changes this:** Markov generators step-by-step by default |
| **Hot-reload pattern duplicated twice** | Created then deleted `hot_reload.rs` before consolidating in `LuaLayerPlugin` | Use existing `LuaLayerRegistry` pattern from the start |
| **Multiplicity constraint missed** | Had to refactor after M7 to support multiple visualizers | Design for multiple generators and checkpoint slots from the start |

### Verification Infrastructure from Phase 1+2

**We already have:**
- Screenshot capture: `cargo run --example p_map_editor_2d -- --screenshot path.png --exit-frame 45`
- MCP endpoints: `curl http://127.0.0.1:8088/mcp/get_output -o output.png`
- Layer filtering: `curl /mcp/get_output?layers=base,visualizer`
- Asset search: `curl /mcp/search?q=natural`
- Layer registry: `curl /mcp/layer_registry`
- Step info: `CurrentStepInfo` resource with position/material/step_number

**Phase 3 adds:**
- MCP endpoint to query Markov model state: `GET /mcp/generator_state`
- MCP endpoint to set checkpoint: `POST /mcp/checkpoint`
- MCP endpoint to list/load checkpoints: `GET/POST /mcp/checkpoints`

---

## Directory Structure

All new code goes in `crates/studio_core/src/map_editor/`. Following Phase 1+2 pattern: library-first, examples are thin wrappers.

```
crates/studio_core/src/map_editor/
├── mod.rs                         # Add new exports
├── generator/
│   ├── mod.rs                     # Generator trait, StepInfo (existing)
│   ├── markov.rs                  # NEW: MarkovGenerator (wraps markov_junior)
│   ├── composed.rs                # NEW: ComposedGenerator (sequence/scatter)
│   └── checkpoint.rs              # NEW: Checkpoint serialization
├── asset/
│   ├── mod.rs                     # Asset trait (existing)
│   ├── store.rs                   # InMemoryStore (existing)
│   └── checkpoint.rs              # NEW: Checkpoint implements Asset
└── mcp_server.rs                  # Extended: checkpoint + generator state endpoints

assets/map_editor/
├── materials.lua                  # Existing
├── generator.lua                  # Existing (simple Lua generator)
├── generators/                    # NEW (M9)
│   ├── markov_dungeon.lua         # Lua wrapper calling mj.load_model
│   └── crystal_scatter.lua        # Scatter generator
├── checkpoints/                   # NEW (M10)
│   └── [saved checkpoint files]
└── renderers/                     # Existing
```

### Files by Milestone

| Milestone | Files Created/Modified |
|-----------|----------------------|
| **M8** | `generator/markov.rs` (MarkovGenerator wrapping Interpreter) |
|        | `lua_generator.rs` (extend to register mj API) |
|        | `mcp_server.rs` (add generator_state endpoint) |
|        | Example Markov Lua script using mj.load_model |
| **M9** | `generator/composed.rs` (SequenceGenerator, ScatterGenerator) |
|        | `assets/map_editor/generators/` directory |
|        | `mcp_server.rs` (add set_generators endpoint for composed) |
| **M10** | `generator/checkpoint.rs` (CheckpointState serialization) |
|         | `asset/checkpoint.rs` (Checkpoint implements Asset) |
|         | `mcp_server.rs` (add checkpoint endpoints) |
|         | `assets/map_editor/checkpoints/` directory |

---

## Current State Assessment

### What Exists After Phase 2

| Component | Status | Phase 3 Impact |
|-----------|--------|----------------|
| `markov_junior` module | Full implementation with Interpreter, Model, lua_api | **Reuse:** `MarkovGenerator` wraps existing `Interpreter` |
| `GeneratorListener` trait | Working, visualizers receive step events | **Extend:** Markov emits richer step info (rule name, affected cells) |
| `LuaLayerRegistry` | Multi-instance layer management | **Extend:** Pattern for multi-generator registry |
| `InMemoryStore<T>` | Generic asset store with search | **Reuse:** Checkpoints are just another `Asset` type |
| `CurrentStepInfo` | Simple step tracking (x, y, material_id) | **Extend:** Add rule_name, affected_count for Markov |
| `LuaGeneratorPlugin` | Single generator hot-reload | **Replace:** With multi-generator registry or extend |

### Architectural Decisions

**1. MarkovGenerator as Generator Adapter**

The existing `markov_junior::Interpreter` has a different API from our `Generator` pattern. `MarkovGenerator` is an adapter:

```rust
pub struct MarkovGenerator {
    interpreter: Interpreter,  // From markov_junior module
    model_path: String,        // For hot-reload
}

impl MarkovGenerator {
    /// Each step() call maps to interpreter.step()
    /// StepInfo extended to include rule information from interpreter
}
```

**2. Composed Generators Use Existing `Generator` Trait**

`SequenceGenerator` and `ScatterGenerator` implement `Generator`. They compose other generators:

```rust
pub struct SequenceGenerator {
    generators: Vec<Box<dyn Generator>>,
    current_index: usize,
}

pub struct ScatterGenerator {
    pattern: ScatterPattern,  // density, spacing, material
}
```

**3. Checkpoints as Assets**

Checkpoints implement `Asset` trait and store in `InMemoryStore<Checkpoint>`:

```rust
pub struct Checkpoint {
    name: String,
    generator_state: Vec<u8>,  // Serialized interpreter state
    voxel_buffer: Vec<u32>,    // Buffer snapshot
    step_number: usize,
    created_at: SystemTime,
}
```

---

## High-Level Summary

**What changes in Phase 3:**

| Area | Before (Phase 2) | After (Phase 3) |
|------|------------------|-----------------|
| **Generator Types** | Simple Lua fill patterns | Markov Jr. models + composed generators |
| **Step Info** | Position + material | Position + material + rule name + affected cells |
| **Generator Count** | 1 active generator | N generators in sequence |
| **State Persistence** | None | Checkpoint save/load |
| **MCP Queries** | Output image + search | + generator state + checkpoints |

**Why these changes:**
- Markov Jr. enables sophisticated procedural content (caves, dungeons, mazes)
- Composed generators enable layered generation (base terrain + decorations)
- Checkpoints enable iteration without losing interesting states
- Richer step info makes visualizer more useful for debugging complex generators

---

## Trait Hierarchy (Phase 3 Target)

```
┌─────────────────────────────────────────────────────────────┐
│                       GENERATION                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Generator (trait) [existing]                               │
│     fn init(ctx)                                             │
│     fn step(ctx) -> bool                                     │
│     fn reset()                                               │
│           │                                                  │
│           ├── LuaGenerator [existing]                        │
│           ├── MarkovGenerator [NEW - wraps Interpreter]      │
│           ├── SequenceGenerator [NEW - composes generators]  │
│           └── ScatterGenerator [NEW - placement pattern]     │
│                                                              │
│   GeneratorListener (trait) [existing]                       │
│     fn on_step(info: &StepInfo)                              │
│     fn on_reset()                                            │
│                                                              │
│   StepInfo (struct) [extended]                               │
│     step_number, x, y, material_id, completed                │
│     rule_name: Option<String>     [NEW]                      │
│     affected_cells: Option<usize> [NEW]                      │
│                                                              │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                      CHECKPOINTING                           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Checkpoint (struct) implements Asset                       │
│     name, generator_state, voxel_buffer, step_number         │
│                                                              │
│   CheckpointStore = InMemoryStore<Checkpoint>                │
│                                                              │
│   CheckpointManager                                          │
│     fn save(name, generator, buffer) -> Checkpoint           │
│     fn load(checkpoint) -> restores generator + buffer       │
│     fn list() -> Vec<&Checkpoint>                            │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase Outcome

**When Phase 3 is complete, I can:**
- Load a Markov Jr. XML model and watch it generate terrain step-by-step
- Chain generators together (base terrain from Markov, then scatter crystals)
- Save interesting generation states and resume from them later

**Phase Foundation:** Introduces two core abstractions:
1. `MarkovGenerator` adapter — wraps `markov_junior::Interpreter` as standard `Generator`; enables existing visualizer/playback to work with Markov
2. `Checkpoint` + `CheckpointManager` — serializable generation state; uses `Asset` trait for storage/search

These foundations enable Phase 4+ features (database-backed checkpoint sharing, semantic search over checkpoints) without refactoring.

---

## Milestones

| M# | Functionality | Foundation |
|----|---------------|------------|
| M8 | I can load a Markov Jr. model and watch it generate step-by-step | `mj` module exposed to Lua generators |
| M8.5 | I can compose generators and see step info from any node in the tree | `Generator` base class, `StepInfoRegistry`, scene tree paths |
| M9 | I can chain generators together (base terrain + scatter crystals) | `Sequential`, `Parallel` composers using M8.5 foundation |
| M10 | I can save generation state and resume later | `Checkpoint` implements `Asset`, `CheckpointManager` |

---

## M8: Markov Jr. Generator

**Functionality:** I can load a Markov Jr. XML model and watch it generate terrain step-by-step in 2D.

**Foundation:** `mj` Lua module exposed to generator scripts via `register_markov_junior_api()`. Existing playback controls and visualizer work unchanged with Markov models.

### Why First

Markov Jr. is the most capable generator—caves, dungeons, mazes, growth patterns. Getting it integrated enables M8.5/M9 (composed generators) to use Markov as a component.

### Design Decision: Lua-Based Instead of Rust Adapter

Original plan was a Rust `MarkovGenerator` struct. We chose Lua-based instead:
- **Simpler:** No new Rust code needed
- **More flexible:** Users can combine mj calls with other Lua logic
- **Follows existing pattern:** All generators are already Lua-based

### API Definitions

**Lua generator using mj module:**
```lua
-- generator_markov.lua
local Generator = {}
local model = nil

function Generator:init(ctx)
    model = mj.load_model("MarkovJunior/models/MazeGrowth.xml")
    model:reset(os.time())
end

function Generator:step(ctx)
    if not model:is_running() then return true end
    
    model:step()
    
    -- Copy grid to buffer
    local grid = model:grid()
    for y = 0, ctx.height - 1 do
        for x = 0, ctx.width - 1 do
            ctx:set_voxel(x, y, grid:get(x, y, 0))
        end
    end
    
    return not model:is_running()
end

return Generator
```

**Extended StepInfo** (optional fields for richer Markov info):
```rust
pub struct StepInfo {
    // Existing
    pub step_number: usize,
    pub x: usize,
    pub y: usize,
    pub material_id: u32,
    pub completed: bool,
    
    // New (optional, for Markov)
    pub rule_name: Option<String>,
    pub affected_cells: Option<usize>,
}
```

**Lua Integration:**

The existing `markov_junior::lua_api` already provides `mj.load_model()`. We expose this in the generator context:

```lua
-- generator.lua (using Markov Jr.)
local Generator = {}
local model = nil

function Generator:init(ctx)
    model = mj.load_model("MarkovJunior/models/BasicDungeon.xml")
    model:reset(os.time())
end

function Generator:step(ctx)
    local done = not model:step()
    if done then return true end
    
    -- Copy Markov grid to voxel buffer
    local grid = model:grid()
    for y = 0, ctx.height - 1 do
        for x = 0, ctx.width - 1 do
            local val = grid:get(x, y, 0)
            ctx:set_voxel(x, y, val)
        end
    end
    return false
end

return Generator
```

**New MCP Endpoint:**
```
GET /mcp/generator_state
  Response: {
    "type": "markov",
    "model": "BasicDungeon.xml",
    "step": 1234,
    "running": true,
    "grid_size": [32, 32, 1]
  }
```

### Verification

```bash
# Start app with Markov generator
cargo run --example p_map_editor_2d &
sleep 3

# Query generator state
curl http://127.0.0.1:8088/mcp/generator_state
# Returns: {"type":"markov","model":"BasicDungeon.xml","step":0,"running":true}

# Get output showing Markov generation in progress
curl http://127.0.0.1:8088/mcp/get_output -o /tmp/markov.png

# Visualizer shows current rule being applied
curl "http://127.0.0.1:8088/mcp/get_output?layers=visualizer" -o /tmp/markov_vis.png
```

### M8 Verification Checklist

- [ ] `MarkovGenerator` struct exists in `generator/markov.rs`
- [ ] `MarkovGenerator` implements `Generator` trait (init, step, reset)
- [ ] `mj` module available in generator Lua context
- [ ] Playback controls (play/pause/step) work with Markov generator
- [ ] Visualizer highlights current generation position
- [ ] `GET /mcp/generator_state` returns Markov model info
- [ ] Example Lua generator using `mj.load_model()` works

### M8 Cleanup Audit

**To be documented in [PHASE_3_CLEANUP.md](./PHASE_3_CLEANUP.md) after milestone completion.**

Anticipated items:
- [ ] Should `LuaGeneratorPlugin` be generalized to support multiple generator types?
- [ ] Is `StepInfo` extension backward-compatible with existing visualizers?

---

## M8.5: Generator Scene Tree & Step Info Registry

**Functionality:** I can compose generators (sequential/parallel) and see step info from any node in the composition tree via MCP.

**Foundation:** `Generator` Lua base class with scene tree paths, `StepInfoRegistry` keyed by path, introspection API for querying generator structure.

### Why Now

M8 revealed a critical gap: generators are opaque. We can't:
- See what's inside a composed generator
- Get step info from specific nodes (e.g., just the Markov, not the scatter)
- Let visualizers subscribe to specific parts of the generation

This follows the PyTorch `nn.Module` pattern: modules register children, you can iterate the tree, composers (`Sequential`, `Parallel`) are explicit.

### User Story

I write a composed generator:

```lua
-- generators/dungeon_with_crystals.lua
local generators = require("generators")

return generators.sequential({
    mj.load_model("MarkovJunior/models/MazeGrowth.xml"),
    generators.scatter({ material = 3, target = 1, density = 0.05 })
})
```

I load it via MCP and see the structure:

```bash
curl -X POST http://127.0.0.1:8088/mcp/set_generator \
  --data-binary @assets/map_editor/generators/dungeon_with_crystals.lua

curl http://127.0.0.1:8088/mcp/generator_state
```

Returns:
```json
{
  "structure": {
    "type": "Sequential",
    "path": "root",
    "children": {
      "step_1": {"type": "MjModel", "path": "root.step_1", "model": "MazeGrowth.xml"},
      "step_2": {"type": "Scatter", "path": "root.step_2"}
    }
  },
  "steps": {
    "root": {"step": 45, "running": true},
    "root.step_1": {"step": 45, "running": false, "completed": true},
    "root.step_2": {"step": 12, "running": true}
  }
}
```

The visualizer can watch a specific path:

```lua
-- visualizer.lua
function Visualizer:on_step(info)
    -- info.path tells us which node emitted this
    if info.path == "root.step_1" then
        -- Markov-specific visualization
        self:highlight_markov(info)
    end
end
```

### API Definitions

**StepInfoRegistry (Rust):**
```rust
#[derive(Resource, Default)]
pub struct StepInfoRegistry {
    /// Map of path -> most recent StepInfo for that node
    pub steps: HashMap<String, StepInfo>,
}

impl StepInfoRegistry {
    pub fn emit(&mut self, path: &str, info: StepInfo);
    pub fn get(&self, path: &str) -> Option<&StepInfo>;
    pub fn get_subtree(&self, prefix: &str) -> Vec<(&str, &StepInfo)>;
    pub fn clear(&mut self);
}
```

**StepInfo extended with path:**
```rust
pub struct StepInfo {
    // Existing fields...
    pub path: String,  // NEW: scene tree path (e.g., "root.step_1")
}
```

**Generator Lua base class:**
```lua
local Generator = {}

function Generator:new(type_name)
    -- Creates generator with _type, _path, _children
end

function Generator:add_child(name, child)
    -- Registers child with extended path (self._path .. "." .. name)
    -- Sets child's context so it can emit step info
end

function Generator:emit_step(info)
    -- Emits step info tagged with self._path
end

function Generator:get_structure()
    -- Returns recursive tree: {type, path, children}
end

return Generator
```

**Built-in composers:**
```lua
local generators = {}

generators.sequential = function(children)
    -- Returns Sequential generator that runs children in order
end

generators.parallel = function(children)
    -- Returns Parallel generator that runs all children each step
end

generators.scatter = function(opts)
    -- Returns Scatter generator (material, target, density)
end

return generators
```

**MjModel integration:**

`mj.load_model()` returns an object that:
- Supports `_set_path(path)` and `_set_context(ctx)`
- Emits step info with rule name when `step()` is called
- Reports its type and model path in `get_structure()`

### Verification

```bash
# Start app
cargo run --example p_map_editor_2d &
sleep 5

# Load composed generator
curl -X POST http://127.0.0.1:8088/mcp/set_generator \
  --data-binary @assets/map_editor/generators/dungeon_with_crystals.lua
# Returns: {"success":true}

# Query structure (recursive tree)
curl http://127.0.0.1:8088/mcp/generator_state | jq .structure
# Returns: {"type":"Sequential","path":"root","children":{"step_1":{"type":"MjModel",...},...}}

# Query step info by path
curl http://127.0.0.1:8088/mcp/generator_state | jq '.steps["root.step_1"]'
# Returns: {"step":45,"running":false,"path":"root.step_1"}

# Visualizer output shows current node being stepped
curl "http://127.0.0.1:8088/mcp/get_output?layers=visualizer" -o /tmp/viz.png
```

### M8.5 Verification Checklist

- [ ] `StepInfoRegistry` resource exists with path-keyed HashMap
- [ ] `StepInfo` has `path` field
- [ ] `Generator` Lua base class exists in `assets/map_editor/lib/generator.lua`
- [ ] `generators.sequential()` and `generators.parallel()` work
- [ ] `generators.scatter()` works
- [ ] `mj.load_model()` integrates with scene tree (supports `_set_path`, `_set_context`)
- [ ] `GET /mcp/generator_state` returns `structure` (recursive tree) and `steps` (path-keyed)
- [ ] Example composed generator (`dungeon_with_crystals.lua`) works
- [ ] Visualizer receives step info with path, can filter by path

### M8.5 Cleanup Audit

**To be documented in [PHASE_3_CLEANUP.md](./PHASE_3_CLEANUP.md) after milestone completion.**

Anticipated items:
- [ ] Should `Generator` base class be in Rust for type safety?
- [ ] Should path separator be `.` or `/`?
- [ ] How should hot-reload interact with scene tree (preserve paths or regenerate)?

---

## M9: Composed Generators (Polish)

**Functionality:** I can create complex multi-stage terrain with multiple generator types and see real-time progress for each stage.

**Foundation:** Uses M8.5's `Generator` base class and scene tree. Adds additional generator types and polishes the composition workflow.

### Why

M8.5 provides the infrastructure (scene tree, step registry, composers). M9 makes it useful:
1. More generator types beyond Markov (fill, noise, cellular automata)
2. Real-world example: dungeon with rooms, corridors, and scattered decorations
3. Visualizer that shows which stage is active

### User Story

I create a dungeon generator with three stages:

```lua
-- generators/full_dungeon.lua
local generators = require("generators")

return generators.sequential({
    -- Stage 1: Carve out dungeon structure
    mj.load_model("MarkovJunior/models/MazeGrowth.xml"),
    
    -- Stage 2: Add room floors
    generators.fill({ material = 2, where = "enclosed" }),
    
    -- Stage 3: Scatter crystals on floors
    generators.scatter({ material = 3, target = 2, density = 0.03 })
})
```

I watch generation and see:
- Stage indicator: "Running: root.step_1 (MazeGrowth)"
- Progress bar for current stage
- Visualizer highlights cells being modified

### New Generator Types

**Fill Generator:**
```lua
generators.fill({
    material = 2,
    where = "enclosed"  -- or "border", "all", custom predicate
})
```

**Noise Generator:**
```lua
generators.noise({
    material = 2,
    threshold = 0.5,
    scale = 4.0,
    seed = 12345
})
```

### Enhanced Visualizer

The visualizer uses step info paths to show stage-aware progress:

```lua
function Visualizer:on_step(info)
    -- Show which stage we're in
    self.current_stage = info.path
    
    -- Different highlight colors per generator type
    if info.path:match("MjModel") then
        self:highlight(info.x, info.y, {1, 0, 0, 0.5})  -- Red for Markov
    elseif info.path:match("Scatter") then
        self:highlight(info.x, info.y, {0, 1, 0, 0.5})  -- Green for scatter
    end
end
```

### Verification

```bash
# Load full dungeon generator
curl -X POST http://127.0.0.1:8088/mcp/set_generator \
  --data-binary @assets/map_editor/generators/full_dungeon.lua

# Check structure shows all three stages
curl http://127.0.0.1:8088/mcp/generator_state | jq '.structure.children | keys'
# Returns: ["step_1", "step_2", "step_3"]

# Watch progress - see which stage is active
curl http://127.0.0.1:8088/mcp/generator_state | jq '.steps | to_entries[] | select(.value.running)'
# Returns: {"key": "root.step_1", "value": {"running": true, ...}}

# Get output with visualizer showing stage colors
curl "http://127.0.0.1:8088/mcp/get_output?layers=base,visualizer" -o /tmp/dungeon.png
```

### M9 Verification Checklist

- [ ] `generators.fill()` works with material and where predicate
- [ ] `generators.noise()` works with threshold and scale
- [ ] Full dungeon example (`full_dungeon.lua`) runs all three stages
- [ ] Visualizer shows different colors per generator type
- [ ] `/mcp/generator_state` shows which stage is currently running
- [ ] Output image shows complete dungeon with scattered crystals

### M9 Cleanup Audit

**To be documented in [PHASE_3_CLEANUP.md](./PHASE_3_CLEANUP.md) after milestone completion.**

Anticipated items:
- [ ] Should predicate functions (`where = "enclosed"`) be Lua functions instead of strings?
- [ ] Should visualizer color mapping be configurable?

---

## M10: Generator Checkpointing

**Functionality:** I can save generation state at any point and resume later from that exact state.

**Foundation:** `Checkpoint` struct implementing `Asset` trait, stored in `InMemoryStore<Checkpoint>`. Uses existing asset infrastructure for persistence and search.

### Why

Procedural generation often produces interesting intermediate states. Without checkpointing:
- Interesting states are lost
- Iteration requires re-running from the start
- Comparison between variations is difficult

Checkpoints enable bookmarking and branching generation paths.

### API Definitions

**Checkpoint:**
```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub name: String,
    pub generator_type: String,
    pub generator_state: Vec<u8>,  // Serialized generator state
    pub voxel_buffer: Vec<u32>,    // Buffer snapshot
    pub step_number: usize,
    pub created_at: u64,           // Unix timestamp
    pub tags: Vec<String>,
}

impl Asset for Checkpoint {
    fn name(&self) -> &str { &self.name }
    fn asset_type() -> &'static str { "checkpoint" }
    fn tags(&self) -> &[String] { &self.tags }
}
```

**CheckpointManager:**
```rust
pub struct CheckpointManager {
    store: InMemoryStore<Checkpoint>,
    save_dir: PathBuf,
}

impl CheckpointManager {
    pub fn save(&mut self, name: &str, generator: &dyn Generator, buffer: &VoxelBuffer2D) -> usize;
    pub fn load(&self, id: usize) -> Option<Checkpoint>;
    pub fn list(&self) -> &[Checkpoint];
    pub fn search(&self, query: &str) -> Vec<&Checkpoint>;
}
```

**Serialization:**

Markov generator state uses existing `markov_junior` serialization (the interpreter's grid state). Lua generators save their Lua table state.

**New MCP Endpoints:**
```
POST /mcp/checkpoint
  Body: {"name": "interesting_cave", "tags": ["cave", "early"]}
  Response: {"id": 1, "name": "interesting_cave", "step": 456}

GET /mcp/checkpoints
  Response: [
    {"id": 1, "name": "interesting_cave", "step": 456, "tags": ["cave", "early"]},
    {"id": 2, "name": "dungeon_complete", "step": 1024, "tags": ["dungeon"]}
  ]

POST /mcp/load_checkpoint
  Body: {"id": 1}
  Response: {"success": true, "restored_step": 456}

GET /mcp/search?q=cave&type=checkpoint
  Response: [{"type": "checkpoint", "name": "interesting_cave", "id": 1}]
```

**UI:**
- "Save Checkpoint" button in playback panel
- Checkpoint list panel showing saved states with thumbnails
- Click checkpoint to load

### Verification

```bash
# Start generation
cargo run --example p_map_editor_2d &
sleep 5

# Save checkpoint at current state
curl -X POST http://127.0.0.1:8088/mcp/checkpoint \
  -H "Content-Type: application/json" \
  -d '{"name":"early_cave","tags":["cave","early"]}'
# Returns: {"id":1,"name":"early_cave","step":123}

# Continue generation...
sleep 5

# List checkpoints
curl http://127.0.0.1:8088/mcp/checkpoints
# Returns: [{"id":1,"name":"early_cave","step":123,"tags":["cave","early"]}]

# Load checkpoint (restores state)
curl -X POST http://127.0.0.1:8088/mcp/load_checkpoint \
  -H "Content-Type: application/json" \
  -d '{"id":1}'
# Returns: {"success":true,"restored_step":123}

# Verify output matches saved state
curl http://127.0.0.1:8088/mcp/get_output -o /tmp/restored.png

# Search checkpoints
curl "http://127.0.0.1:8088/mcp/search?q=cave&type=checkpoint"
# Returns: [{"type":"checkpoint","name":"early_cave","id":1}]
```

### M10 Verification Checklist

- [ ] `Checkpoint` struct exists in `generator/checkpoint.rs` or `asset/checkpoint.rs`
- [ ] `Checkpoint` implements `Asset` trait
- [ ] `CheckpointManager` can save/load/list checkpoints
- [ ] `POST /mcp/checkpoint` saves current state
- [ ] `GET /mcp/checkpoints` lists all saved checkpoints
- [ ] `POST /mcp/load_checkpoint` restores generation state
- [ ] `GET /mcp/search?type=checkpoint` searches checkpoints
- [ ] Saved checkpoints persist to `assets/map_editor/checkpoints/` directory
- [ ] UI shows checkpoint list and save/load buttons

### M10 Cleanup Audit

**To be documented in [PHASE_3_CLEANUP.md](./PHASE_3_CLEANUP.md) after milestone completion.**

Anticipated items:
- [ ] Should checkpoints auto-save at configurable intervals?
- [ ] Is file-based persistence sufficient or should checkpoints use `FileBackedStore`?
- [ ] Should checkpoint thumbnails be generated and stored?

---

## Phase 3 Cleanup Notes

**See [PHASE_3_CLEANUP.md](./PHASE_3_CLEANUP.md) for detailed cleanup audit.**

The cleanup document will track:
- Refactoring candidates identified during each milestone
- Current state vs proposed change with engineering rationale
- Criticality levels (High/Medium/Low)
- Recommended timing for each refactor

### Quick Reference (Anticipated)

| Milestone | Likely Cleanup Items | Criticality |
|-----------|---------------------|-------------|
| M8 | Design decision: Lua-based vs Rust adapter | N/A (documented) |
| M8.5 | Generator base class: Lua-only vs Rust backing | Medium |
| M8.5 | Path separator convention (`.` vs `/`) | Low |
| M9 | Predicate functions vs strings | Low |
| M10 | Checkpoint persistence strategy | Low |

### Cleanup Decision

At Phase 3 end, review [PHASE_3_CLEANUP.md](./PHASE_3_CLEANUP.md) and decide:
- **Do now:** Items that block Phase 4 or create significant tech debt
- **Defer:** Items that are nice-to-have but don't block progress
- **Drop:** Items that turned out to be unnecessary

---

## Files Changed

### New

| File | Purpose |
|------|---------|
| `assets/map_editor/lib/generator.lua` | Generator base class with scene tree support |
| `assets/map_editor/lib/generators.lua` | Built-in composers (sequential, parallel, scatter, fill, noise) |
| `assets/map_editor/generators/dungeon_with_crystals.lua` | Example composed generator |
| `assets/map_editor/generators/full_dungeon.lua` | Multi-stage dungeon example |
| `generator/checkpoint.rs` | `Checkpoint` struct, `CheckpointManager` |
| `asset/checkpoint.rs` | `Checkpoint` implements `Asset` |
| `assets/map_editor/checkpoints/` | Directory for saved checkpoint files |

### Modified

| File | Change |
|------|--------|
| `generator/mod.rs` | Add `StepInfoRegistry`, extend `StepInfo` with `path` field |
| `lua_generator.rs` | Register `mj` module, load generator lib, emit path-keyed step info |
| `markov_junior/lua_api.rs` | Add `_set_path`, `_set_context`, emit step info from `MjLuaModel` |
| `mcp_server.rs` | Update `generator_state` to return structure + steps, add checkpoint endpoints |
| `render/visualizer.rs` | Access step info by path from `StepInfoRegistry` |
| `app.rs` | Add `StepInfoRegistry` resource, `CheckpointManager` resource |

---

## Final Verification Script

```bash
#!/bin/bash
set -e

echo "=== Phase 3 Verification ==="

cargo run --example p_map_editor_2d &
APP_PID=$!
sleep 5

# M8: Markov Generator
echo "M8: Markov Generator..."
curl -s http://127.0.0.1:8088/mcp/generator_state | grep -q "type" && echo "PASS: generator_state"
curl -s http://127.0.0.1:8088/mcp/get_output -o /tmp/m8_output.png && echo "PASS: output"

# M8.5: Scene Tree & Step Registry
echo "M8.5: Scene Tree..."
curl -s -X POST http://127.0.0.1:8088/mcp/set_generator \
  --data-binary @assets/map_editor/generators/dungeon_with_crystals.lua
curl -s http://127.0.0.1:8088/mcp/generator_state | grep -q "structure" && echo "PASS: structure"
curl -s http://127.0.0.1:8088/mcp/generator_state | grep -q "steps" && echo "PASS: steps"
curl -s http://127.0.0.1:8088/mcp/generator_state | grep -q "root.step_1" && echo "PASS: path-keyed"

# M9: Composed Generators
echo "M9: Composed Generators..."
curl -s -X POST http://127.0.0.1:8088/mcp/set_generator \
  --data-binary @assets/map_editor/generators/full_dungeon.lua
sleep 3
curl -s http://127.0.0.1:8088/mcp/generator_state | grep -q "step_3" && echo "PASS: three stages"
curl -s http://127.0.0.1:8088/mcp/get_output -o /tmp/m9_dungeon.png && echo "PASS: dungeon output"

# M10: Checkpointing
echo "M10: Checkpointing..."
curl -s -X POST http://127.0.0.1:8088/mcp/checkpoint \
  -H "Content-Type: application/json" \
  -d '{"name":"test_checkpoint","tags":["test"]}' \
  | grep -q "id" && echo "PASS: save checkpoint"
curl -s http://127.0.0.1:8088/mcp/checkpoints | grep -q "test_checkpoint" && echo "PASS: list checkpoints"
curl -s -X POST http://127.0.0.1:8088/mcp/load_checkpoint \
  -H "Content-Type: application/json" \
  -d '{"id":1}' \
  | grep -q "success" && echo "PASS: load checkpoint"

kill $APP_PID 2>/dev/null
echo "=== Phase 3 Complete ==="
```

---

## Estimated Time

| Milestone | Time |
|-----------|------|
| M8 (Markov Generator) | 2 hours |
| M8.5 (Scene Tree & Step Registry) | 4 hours |
| M9 (Composed Generators Polish) | 2 hours |
| M10 (Checkpointing) | 4 hours |
| **Total** | **12 hours** |

---

## Dependencies

**Phase 2 → Phase 3:**
- `GeneratorListener` → Receives richer `StepInfo` with path
- `StepInfo` → Extended with `path` field
- `InMemoryStore<T>` → Reused for checkpoints
- `Asset` trait → Implemented by `Checkpoint`
- `LuaGeneratorPlugin` → Extended to register `mj` module and generators lib
- MCP server → Extended with structure/steps response

**M8 → M8.5:**
- `mj` module in Lua → Used by Generator base class
- Basic `StepInfo` → Extended with path

**M8.5 → M9:**
- `Generator` Lua base class → Used by all built-in composers
- `StepInfoRegistry` → Visualizer accesses path-keyed steps
- Scene tree structure → Returned by `/mcp/generator_state`

**M9 → M10:**
- Composed generators → Checkpoints save entire tree state

**Phase 3 → Phase 4:**
- `Checkpoint` implements `Asset` → Ready for `DatabaseStore<Checkpoint>`
- `CheckpointManager` uses `InMemoryStore` → Ready to swap to `DatabaseStore`
- Generator state serialization → Ready for semantic search embeddings

---

## Alignment with Phase 4

Phase 4 introduces `DatabaseStore` to replace `InMemoryStore`. Phase 3 designs ensure smooth transition:

| Phase 3 Component | Phase 4 Migration |
|-------------------|-------------------|
| `InMemoryStore<Checkpoint>` | Swap to `DatabaseStore<Checkpoint>` |
| Checkpoint file persistence | Move to database-backed persistence |
| `search()` by name/tag | Add `search_semantic()` with embeddings |
| `CheckpointManager` API | Unchanged—store backend swaps transparently |

This alignment follows the Library-Centric Rule: Phase 3 builds abstractions that Phase 4 uses without refactoring.
