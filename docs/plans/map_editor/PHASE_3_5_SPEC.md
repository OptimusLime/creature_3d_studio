# Phase 3.5 Specification: Markov Jr. Introspection & Visualization

*Following HOW_WE_WORK.md and WRITING_MILESTONES.md.*

**Key principle:** Remove the opacity of Markov Jr. models. Users should see inside, not just outside.

---

## Why This Phase Exists

Phase 3 (M8-M10) integrated Markov Jr. as a black box:
- We call `model.step()` and get results
- We can see that "something changed" but not what or why
- The internal node tree (MarkovNode, SequenceNode, OneNode, etc.) is invisible
- We can't control step granularity (1 rule vs 100 rules per frame)

This matters because:
1. **Debugging:** When generation fails, you need to see which rule went wrong
2. **Visualization:** Interesting patterns emerge from specific rule sequences
3. **Education:** Understanding Markov Jr. requires seeing it work, not just results
4. **Control:** Different use cases need different step speeds

The C# Markov Jr. visualizer already does this. We need parity.

---

## Phase Outcome

**When Phase 3.5 is complete, I can:**
- Query the internal structure of any Markov Jr. model via MCP
- See which specific node (e.g., "root.markov.one[0]") made each change
- Control exactly how many rule applications happen per frame
- Watch a dedicated visualizer that shows the node tree and highlights active nodes
- **Render simulation and visualizer to separate textures, composited side-by-side**
- **Export a video of the generation process showing both grid and structure**

**Phase Foundation:**
1. `MjNode::structure()` - Every Markov node can describe its tree structure
2. Path tracking in `ExecutionContext` - Nodes report their location when making changes
3. Budget-aware stepping - Fine-grained control over rule applications
4. **`RenderSurface` abstraction** - Multiple render targets with independent layer stacks
5. `MjVisualizerLayer` - Dedicated visualization that understands Markov structure
6. **Video export** - Frame-by-frame PNG capture composited into video

---

## Current Architecture (For Reference)

### Markov Jr. Node Hierarchy

```
Interpreter
└── root: Box<dyn Node>
    └── MarkovNode (tries children until one succeeds, resets to child 0)
        ├── SequenceNode (runs children in order)
        │   ├── OneNode (applies ONE random matching rule)
        │   └── AllNode (applies ALL matching rules)
        ├── OneNode
        ├── AllNode
        ├── PathNode
        ├── MapNode
        ├── ConvChainNode
        ├── ConvolutionNode
        └── WFC nodes...
```

### Current Step Flow

```
Lua calls model:step()
  → MjLuaModel::step()
    → Model::step()
      → Interpreter::step()
        → root.go(ctx)  // ONE call to root node
          → MarkovNode tries children
            → OneNode.go() applies ONE rule
            → records change in ctx.changes
        → counter += 1
      → returns bool (still running?)
    → emits step info (ONCE, at end)
  → returns to Lua
```

**Problem:** We emit step info once per `model:step()` call. If the root is a SequenceNode with 3 children, we only see info from the last one that ran.

---

## Milestone Details

### M10.4: Multi-Surface Rendering Foundation

**Functionality:** I can render to multiple independent textures with separate layer stacks, composited side-by-side for screenshots and video export.

**Foundation:** `RenderSurface` abstraction that decouples render targets from the layer system. Each surface has its own dimensions and layer stack.

#### Why First

Everything else in Phase 3.5 depends on this. The MJ visualizer needs its own render target (not overlaid on the grid). Video export needs to capture both surfaces. Without this foundation, we'd be hacking around a single-texture assumption.

#### Architecture

```
RenderSurfaceManager
├── surfaces: HashMap<String, RenderSurface>
│   ├── "grid" → RenderSurface { width: 100, height: 100, layers: [base, grid_visualizer] }
│   └── "mj_structure" → RenderSurface { width: 100, height: 100, layers: [mj_tree, mj_highlight] }
├── layout: SurfaceLayout  // How surfaces are composited
└── output: CompositeBuffer  // Final combined image

SurfaceLayout
├── Horizontal([("mj_structure", 100), ("grid", 100)])  // left-to-right
├── Vertical([...])
└── Custom(fn compose)
```

**Example layout:**
```
┌─────────────┬─────────────┐
│ mj_structure│    grid     │
│  (100x100)  │  (100x100)  │
│             │             │
│ [node tree] │ [simulation]│
│ [active hl] │ [step trail]│
└─────────────┴─────────────┘
      200 x 100 total
```

#### API Design

```rust
/// A render target with its own pixel buffer and layer stack.
pub struct RenderSurface {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub buffer: PixelBuffer,
    pub layers: Vec<Box<dyn RenderLayer>>,
}

/// Manages multiple render surfaces and composites them.
#[derive(Resource)]
pub struct RenderSurfaceManager {
    surfaces: HashMap<String, RenderSurface>,
    layout: SurfaceLayout,
}

impl RenderSurfaceManager {
    /// Add a surface with specified dimensions.
    pub fn add_surface(&mut self, name: &str, width: usize, height: usize);
    
    /// Add a layer to a specific surface.
    pub fn add_layer(&mut self, surface: &str, layer: Box<dyn RenderLayer>);
    
    /// Render all surfaces and composite into final buffer.
    pub fn render_all(&mut self, ctx: &RenderContext) -> CompositeBuffer;
    
    /// Get a single surface's buffer (for surface-specific export).
    pub fn get_surface(&self, name: &str) -> Option<&RenderSurface>;
}

/// How surfaces are arranged in the final composite.
pub enum SurfaceLayout {
    /// Surfaces arranged left-to-right.
    Horizontal(Vec<String>),
    /// Surfaces arranged top-to-bottom.
    Vertical(Vec<String>),
    /// Grid arrangement (rows x cols).
    Grid { columns: usize },
}
```

**Lua API:**
```lua
-- Create surfaces
surfaces:create("mj_structure", 100, 100)
surfaces:create("grid", 100, 100)

-- Add layers to surfaces
surfaces:add_layer("grid", base_layer)
surfaces:add_layer("grid", step_trail_visualizer)
surfaces:add_layer("mj_structure", mj_tree_layer)
surfaces:add_layer("mj_structure", mj_highlight_layer)

-- Set layout
surfaces:set_layout("horizontal", {"mj_structure", "grid"})
```

**MCP API:**
```
GET /mcp/get_output
  → Returns composite PNG (both surfaces side-by-side)

GET /mcp/get_output?surface=grid
  → Returns only grid surface PNG

GET /mcp/get_output?surface=mj_structure
  → Returns only MJ structure surface PNG

GET /mcp/surfaces
  → Returns: {"surfaces": ["grid", "mj_structure"], "layout": "horizontal", "total_size": [200, 100]}
```

#### Video Export Foundation

```rust
/// Frame capture for video export.
pub struct FrameCapture {
    frames: Vec<CompositeBuffer>,
    frame_rate: u32,
}

impl FrameCapture {
    /// Capture current composite state as a frame.
    pub fn capture(&mut self, manager: &RenderSurfaceManager);
    
    /// Export all frames to PNG sequence.
    pub fn export_pngs(&self, dir: &Path) -> io::Result<()>;
    
    /// Export to video (requires ffmpeg).
    pub fn export_video(&self, path: &Path, codec: &str) -> io::Result<()>;
}
```

**MCP API:**
```
POST /mcp/start_recording
  → Starts capturing frames each step

POST /mcp/stop_recording
  → Stops capturing, returns frame count

POST /mcp/export_video
  Body: {"path": "/tmp/gen.mp4", "fps": 30}
  → Exports captured frames to video
```

#### Implementation Tasks

1. Create `RenderSurface` struct with buffer and layer stack
2. Create `RenderSurfaceManager` resource
3. Refactor current single-texture rendering to use manager with "grid" surface
4. Add `SurfaceLayout` enum and composite logic
5. Update MCP `get_output` to support `?surface=` parameter
6. Add `GET /mcp/surfaces` endpoint
7. Create `FrameCapture` struct for video export
8. Add recording MCP endpoints

#### Verification

```bash
# Check surfaces exist
curl http://127.0.0.1:8088/mcp/surfaces
# Returns: {"surfaces":["grid"],"layout":"horizontal","total_size":[100,100]}

# Get composite output
curl http://127.0.0.1:8088/mcp/get_output -o /tmp/composite.png

# Get single surface
curl "http://127.0.0.1:8088/mcp/get_output?surface=grid" -o /tmp/grid_only.png

# Video export (after recording)
curl -X POST http://127.0.0.1:8088/mcp/start_recording
# ... run generation ...
curl -X POST http://127.0.0.1:8088/mcp/stop_recording
curl -X POST http://127.0.0.1:8088/mcp/export_video -d '{"path":"/tmp/gen.mp4","fps":30}'
```

---

### M10.5: Markov Jr. Structure Introspection

**Functionality:** I can see the internal node tree of a Markov Jr. model via MCP.

**Foundation:** `structure()` method on `Node` trait (or new `MjNode` trait). All node types implement it.

#### API Design

**Node trait extension:**
```rust
pub trait Node {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool;
    fn reset(&mut self);
    fn is_branch(&self) -> bool { false }
    
    // NEW: Return structure for introspection
    fn structure(&self) -> MjNodeStructure;
}

#[derive(Clone, Serialize)]
pub struct MjNodeStructure {
    /// Node type: "Markov", "Sequence", "One", "All", "Path", etc.
    pub node_type: String,
    /// Path in the tree (e.g., "root.children[0]")
    pub path: String,
    /// Children (for branch nodes)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<MjNodeStructure>,
    /// Rule strings (for One/All nodes)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
    /// Additional config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}
```

**Example MCP response:**
```json
{
  "structure": {
    "type": "Sequential",
    "path": "root",
    "children": {
      "step_1": {
        "type": "MjModel",
        "path": "root.step_1",
        "model_name": "MazeGrowth",
        "mj_structure": {
          "node_type": "Markov",
          "path": "root.step_1.mj",
          "children": [
            {
              "node_type": "One",
              "path": "root.step_1.mj.children[0]",
              "rules": ["WBB=WAW", "WAW=WAA"]
            }
          ]
        }
      }
    }
  }
}
```

#### Implementation Tasks

1. Add `structure()` method to `Node` trait with default impl
2. Implement `structure()` for each node type:
   - `MarkovNode`: Returns children list
   - `SequenceNode`: Returns children list in order
   - `OneNode`: Returns rule strings
   - `AllNode`: Returns rule strings
   - `PathNode`: Returns config (start, end, etc.)
   - etc.
3. Add `MjRule::to_string()` for human-readable rule format
4. Expose via `Model::structure()` → `Interpreter::structure()` → `root.structure()`
5. Integrate with `MjLuaModel` to expose in Lua and MCP

#### Verification

```bash
curl http://127.0.0.1:8088/mcp/generator_state | jq '.structure.children.step_1.mj_structure'
# Returns: {"node_type":"Markov","path":"root.step_1.mj","children":[...]}

curl http://127.0.0.1:8088/mcp/generator_state | jq '.structure.children.step_1.mj_structure.children[0].rules'
# Returns: ["WBB=WAW", "WAW=WAA"]
```

---

### M10.6: Per-Node Step Info from Markov Jr.

**Functionality:** I can see which specific Markov Jr. node made a change and what rule it applied.

**Foundation:** Path tracking in `ExecutionContext`. Nodes push/pop their path during `go()`.

#### API Design

**Extended ExecutionContext:**
```rust
pub struct ExecutionContext<'a> {
    pub grid: &'a mut MjGrid,
    pub random: &'a mut dyn MjRng,
    pub changes: Vec<(i32, i32, i32)>,
    pub first: Vec<usize>,
    pub counter: usize,
    pub gif: bool,
    
    // NEW: Path tracking
    pub path_stack: Vec<String>,
    pub step_infos: Vec<MjStepInfo>,
}

#[derive(Clone)]
pub struct MjStepInfo {
    pub path: String,
    pub rule_applied: Option<String>,
    pub cells_changed: Vec<(i32, i32, i32)>,
    pub counter: usize,
}

impl ExecutionContext<'_> {
    pub fn push_path(&mut self, segment: &str) {
        let parent = self.path_stack.last().map(|s| s.as_str()).unwrap_or("root");
        self.path_stack.push(format!("{}.{}", parent, segment));
    }
    
    pub fn pop_path(&mut self) {
        self.path_stack.pop();
    }
    
    pub fn current_path(&self) -> &str {
        self.path_stack.last().map(|s| s.as_str()).unwrap_or("root")
    }
    
    pub fn emit_step_info(&mut self, rule: Option<String>, cells: Vec<(i32, i32, i32)>) {
        self.step_infos.push(MjStepInfo {
            path: self.current_path().to_string(),
            rule_applied: rule,
            cells_changed: cells,
            counter: self.counter,
        });
    }
}
```

**Node modifications (example OneNode):**
```rust
impl Node for OneNode {
    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        ctx.push_path(&format!("one[{}]", self.index));
        
        // ... existing logic ...
        
        if let Some((r, x, y, z)) = self.random_match(ctx) {
            let rule = &self.data.rules[r];
            let rule_str = rule.to_string(); // "WB=WW"
            
            // Track which cells this rule will change
            let cells_before = ctx.changes.len();
            self.apply(rule, x, y, z, ctx);
            let cells_after = ctx.changes.len();
            let changed: Vec<_> = ctx.changes[cells_before..cells_after].to_vec();
            
            // Emit step info with rule and cells
            ctx.emit_step_info(Some(rule_str), changed);
            
            ctx.pop_path();
            return true;
        }
        
        ctx.pop_path();
        false
    }
}
```

**MCP steps field now keyed by full path:**
```json
{
  "steps": {
    "root.step_1.mj.one[0]": {
      "step": 45,
      "rule_name": "WB=WW",
      "affected_cells": 3,
      "x": 12, "y": 8
    },
    "root.step_2": {
      "step": 12,
      "affected_cells": 1
    }
  }
}
```

#### Implementation Tasks

1. Add `path_stack` and `step_infos` to `ExecutionContext`
2. Add `push_path()`, `pop_path()`, `emit_step_info()` methods
3. Modify each node type to push/pop path and emit step info:
   - `OneNode`: Push "one[index]", emit rule string
   - `AllNode`: Push "all[index]", emit rule string for each application
   - `MarkovNode`: Push "markov", delegate to children
   - `SequenceNode`: Push "seq", delegate to children
   - etc.
4. Add `MjRule::to_string()` method
5. Collect step infos after `interpreter.step()` and expose to Lua
6. Update `MjLuaModel::step()` to emit step info for each `MjStepInfo`

#### Verification

```bash
# After stepping through a complex model
curl http://127.0.0.1:8088/mcp/generator_state | jq '.steps | keys'
# Returns: ["root.step_1.mj.one[0]", "root.step_1.mj.one[1]", ...]

curl http://127.0.0.1:8088/mcp/generator_state | jq '.steps["root.step_1.mj.one[0]"].rule_name'
# Returns: "WB=WW"
```

---

### M10.7: Markov Jr. Step Budget Control

**Functionality:** I can control exactly how many atomic rule applications happen per frame.

**Foundation:** Budget-aware stepping that counts rule applications.

#### API Design

**New stepping method:**
```rust
impl Interpreter {
    /// Step until budget exhausted or completion.
    /// Returns (steps_taken, still_running).
    pub fn step_n(&mut self, budget: usize) -> (usize, bool) {
        let mut steps = 0;
        while steps < budget && self.running {
            if !self.step() {
                break;
            }
            steps += 1;
        }
        (steps, self.running)
    }
}

impl Model {
    pub fn step_n(&mut self, budget: usize) -> (usize, bool) {
        self.interpreter.step_n(budget)
    }
}
```

**Lua API:**
```lua
-- Single step (budget = 1)
model:step()

-- Multiple steps with budget
local steps_taken, still_running = model:step_n(100)

-- Run to completion
model:run(seed, 0)  -- existing API
```

**MCP API:**
```
POST /mcp/step_generator
Body: {"budget": 100}
Response: {"steps_taken": 100, "running": true}
```

**Playback UI:**
- Step mode selector: "Single" (budget=1), "Fast" (budget=100), "Instant" (budget=10000)
- Budget slider for custom values

#### Implementation Tasks

1. Add `step_n(budget)` to `Interpreter` and `Model`
2. Add `model:step_n(budget)` to `MjLuaModel`
3. Add `POST /mcp/step_generator` endpoint
4. Update playback UI to support budget selection
5. Update `run_generation_step` system to use configured budget

#### Verification

```bash
# Start generation
cargo run --example p_map_editor_2d &
sleep 5

# Single step
curl -X POST http://127.0.0.1:8088/mcp/step_generator -d '{"budget":1}'
# Returns: {"steps_taken":1,"running":true}

# Fast forward
curl -X POST http://127.0.0.1:8088/mcp/step_generator -d '{"budget":1000}'
# Returns: {"steps_taken":1000,"running":true}
```

---

### M10.8: Markov Jr. Visualizer Layer

**Functionality:** I can see a real-time overlay showing the Markov Jr. structure and active nodes, rendered to a separate surface alongside the grid.

**Foundation:** Uses `RenderSurfaceManager` from M10.4. Two surfaces:
- `"grid"` surface: Simulation output + grid visualizer (step trail)
- `"mj_structure"` surface: Node tree + active highlighting

**Depends on:** M10.4 (multi-surface), M10.5 (structure), M10.6 (step info with paths), M10.7 (budget stepping)

#### Design

**Surface Layout:**
```
┌──────────────────┬──────────────────┐
│  mj_structure    │      grid        │
│    (100x100)     │    (100x100)     │
│                  │                  │
│ ┌─ Markov        │  [simulation     │
│ │  ├─ One [*]    │   output with    │
│ │  │  WB=WW ←    │   step trail     │
│ │  └─ All        │   highlighting]  │
│ └─ Path          │                  │
└──────────────────┴──────────────────┘
        200 x 100 composite
```

**Two visualizer types:**
1. **MJ Structure Visualizer** → renders to `"mj_structure"` surface
   - Node tree with hierarchy
   - Active node highlighted (based on step info path)
   - Current rule displayed
2. **Grid Step Trail Visualizer** → renders to `"grid"` surface
   - Overlays on grid
   - Shows affected cells from recent steps
   - Fades older steps (trail effect)

#### Layers

The visualizer renders to the `"mj_structure"` surface:
1. **Structure tree**: Node hierarchy from M10.5
2. **Active highlight**: Currently executing node highlighted (from M10.6 step path)
3. **Rule display**: Current rule being applied shown next to node

The grid visualizer renders to the `"grid"` surface:
4. **Cell overlay**: Cells affected by current step highlighted

#### Implementation Approach

**New Lua visualizer:**
```lua
-- visualizers/mj_structure.lua
local Visualizer = require("lib.visualizer")
local MjVis = Visualizer:extend("MjStructure")

function MjVis:render(ctx, step_info)
    -- Get structure from MCP or cached
    local structure = ctx:get_mj_structure()
    if not structure then return end
    
    -- Draw structure tree on left panel
    self:draw_tree(structure, 10, 10)
    
    -- Highlight active node based on step_info.path
    if step_info and step_info.path then
        self:highlight_node(step_info.path)
        
        -- Show rule if available
        if step_info.rule_name then
            self:draw_rule_label(step_info.path, step_info.rule_name)
        end
        
        -- Highlight affected cells on grid
        if step_info.affected_cells then
            for _, cell in ipairs(step_info.cells) do
                ctx:highlight_cell(cell.x, cell.y, {1, 0, 0, 0.5})
            end
        end
    end
end

return MjVis
```

**Required context extensions:**
- `ctx:get_mj_structure()` - Returns cached structure from MCP
- `ctx:highlight_cell(x, y, color)` - Draw highlight on grid cell
- Access to step info with full path

#### Implementation Tasks

1. Create `MjStructureVisualizer` Lua class
2. Add tree layout algorithm (vertical list with indentation)
3. Add node highlighting based on path match
4. Add cell highlighting for affected cells
5. Cache structure (don't query MCP every frame)
6. Add to default layer stack when Markov generator active

#### Verification

Visual verification:
1. Load a Markov model
2. Set playback to single-step mode (budget=1)
3. Step through generation
4. Verify: Structure tree visible, active node highlighted, rule shown, affected cells highlighted

---

## Files Changed

### New Files

| File | Purpose |
|------|---------|
| `map_editor/render/surface.rs` | `RenderSurface`, `RenderSurfaceManager` |
| `map_editor/render/frame_capture.rs` | `FrameCapture` for video export |
| `markov_junior/mj_structure.rs` | `MjNodeStructure` struct, `Node::structure()` implementations |
| `markov_junior/step_info.rs` | `MjStepInfo` struct, path tracking helpers |
| `assets/map_editor/visualizers/mj_structure.lua` | Markov Jr. structure visualizer (renders to mj_structure surface) |
| `assets/map_editor/visualizers/step_trail.lua` | Grid step trail visualizer (renders to grid surface) |

### Modified Files

| File | Change |
|------|--------|
| `markov_junior/node.rs` | Add `structure()` to `Node` trait |
| `markov_junior/one_node.rs` | Implement `structure()`, add path tracking |
| `markov_junior/all_node.rs` | Implement `structure()`, add path tracking |
| `markov_junior/parallel_node.rs` | Implement `structure()`, add path tracking |
| `markov_junior/convchain_node.rs` | Implement `structure()`, add path tracking |
| `markov_junior/path_node.rs` | Implement `structure()`, add path tracking |
| `markov_junior/interpreter.rs` | Add `step_n()`, expose `step_infos` |
| `markov_junior/model.rs` | Add `structure()`, `step_n()` |
| `markov_junior/lua_api.rs` | Expose structure and step_n to Lua |
| `markov_junior/rule.rs` | Add `MjRule::to_string()` |
| `map_editor/mcp_server.rs` | Add `step_generator` endpoint, include `mj_structure` in response |
| `map_editor/lua_generator.rs` | Update step info emission for multiple infos |

---

## Estimated Time

| Milestone | Time |
|-----------|------|
| M10.4 (Multi-Surface Rendering) | 6-8 hours |
| M10.5 (Structure Introspection) | 4-6 hours |
| M10.6 (Per-Node Step Info) | 6-8 hours |
| M10.7 (Step Budget Control) | 2-3 hours |
| M10.8 (Visualizer Layer) | 4-6 hours |
| **Total** | **22-31 hours** |

---

## Dependencies

```
Phase 3 (M8-M10)
    │
    ▼
M10.4: Multi-Surface Rendering  ◄── FOUNDATION (everything depends on this)
    │
    ├───────────────────┐
    ▼                   ▼
M10.5: Structure    M10.7: Step Budget
    │                   │
    ▼                   │
M10.6: Per-Node         │
Step Info               │
    │                   │
    └───────┬───────────┘
            ▼
    M10.8: Visualizer Layer
            │
            ▼
    Phase 4 (Unified Store)
```

**Phase 3 → M10.4:**
- `MjLuaModel` exists and integrates with step info system
- `StepInfoRegistry` supports path-keyed storage
- MCP server infrastructure exists
- Current single-texture rendering works

**M10.4 → M10.5, M10.7:**
- Multi-surface rendering works
- Surfaces can be created/configured via Lua/MCP
- Video export foundation in place

**M10.5 → M10.6:**
- Structure exists to define valid paths
- Path format established (used in step info)

**M10.6 + M10.7 → M10.8:**
- All step info infrastructure in place (M10.6)
- Budget allows single-stepping for clear visualization (M10.7)
- Multiple surfaces available to render to (M10.4)

**Phase 3.5 → Phase 4:**
- Markov introspection complete
- Multi-surface rendering enables future UI panels
- Video export ready for documentation/demos

---

## Reference: C# Markov Jr. Visualizer

The original C# implementation has a GUI that shows:
- The node tree (XML structure)
- Current execution point
- Rule that matched
- Cells that changed

Location: `MarkovJunior/GUI.cs` and related rendering code.

Key patterns to study:
1. How does it track current node during execution?
2. How does it render the tree?
3. How does it highlight affected cells?

---

## Cleanup Notes

Anticipated cleanup items (to be reviewed after completion):

- [ ] Should `MjNodeStructure` be merged with `GeneratorStructure`?
- [ ] Is path format `root.children[0]` vs `root.step_1` consistent enough?
- [ ] Should visualizer be Rust or Lua? (Lua chosen for flexibility)
- [ ] Performance: Is caching structure sufficient or need incremental updates?

---

## Phase 3.5 Verification Script

```bash
#!/bin/bash
set -e

echo "=== Phase 3.5 Verification ==="

cargo run --example p_map_editor_2d &
APP_PID=$!
sleep 8

# M10.5: Structure Introspection
echo "M10.5: Structure Introspection..."
curl -s http://127.0.0.1:8088/mcp/generator_state | jq '.structure.children.step_1.mj_structure' | grep -q "node_type" && echo "PASS: mj_structure"
curl -s http://127.0.0.1:8088/mcp/generator_state | jq '.structure.children.step_1.mj_structure.children[0].rules' | grep -q "\[" && echo "PASS: rules visible"

# M10.6: Per-Node Step Info
echo "M10.6: Per-Node Step Info..."
curl -s -X POST http://127.0.0.1:8088/mcp/step_generator -d '{"budget":10}'
curl -s http://127.0.0.1:8088/mcp/generator_state | jq '.steps | keys[]' | grep -q "mj" && echo "PASS: mj node paths"
curl -s http://127.0.0.1:8088/mcp/generator_state | jq '.steps | to_entries[0].value.rule_name' | grep -q "=" && echo "PASS: rule names"

# M10.7: Step Budget
echo "M10.7: Step Budget..."
BEFORE=$(curl -s http://127.0.0.1:8088/mcp/generator_state | jq '.step')
curl -s -X POST http://127.0.0.1:8088/mcp/step_generator -d '{"budget":50}'
AFTER=$(curl -s http://127.0.0.1:8088/mcp/generator_state | jq '.step')
[ "$AFTER" -gt "$BEFORE" ] && echo "PASS: budget stepping"

# M10.8: Visualizer (manual verification)
echo "M10.8: Visualizer requires manual verification"
echo "  - Load p_map_editor_2d"
echo "  - Enable MJ Structure visualizer layer"
echo "  - Single-step and verify tree + highlights"

kill $APP_PID 2>/dev/null
echo "=== Phase 3.5 Complete ==="
```
