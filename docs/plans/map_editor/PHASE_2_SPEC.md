# Phase 2 Specification: Lua Rendering + Visualization

*Following HOW_WE_WORK.md and WRITING_MILESTONES.md.*

**Key principle:** Every task produces visible, verifiable functionality. Verification requires ZERO additional work—we look at a screenshot or check a file that already exists.

---

## Lessons Learned from Phase 1

### Planning Errors That Were Corrected Mid-Flight

| Error | What Happened | Prevention |
|-------|---------------|------------|
| **Library extraction mid-phase** | M1.5 was added because we dumped everything into example file | Phase 2 continues library-first: all code in `crates/studio_core/src/map_editor/` |
| **ImGui screenshot surprise** | ImGui renders AFTER Bevy's screenshot capture | Phase 2 uses existing Bevy screenshot (captures Sprite, not ImGui) |
| **Port conflict** | MCP port 8080 conflicted with node process | Phase 2 continues using 8088 |
| **Canvas architecture** | ImGui Image widget doesn't update on texture change | Phase 2 continues using Bevy Sprite for canvas |
| **Missing Palette Builder UI** | Generator was unusable without way to select materials | Phase 2 builds on existing UI patterns |
| **Generator immediate fill** | Had to add instant fill for visual feedback | **Phase 2 changes this:** Generator runs step-by-step on launch (not completed). Visualizer immediately useful. |

### Verification Infrastructure from Phase 1

**We already have:**
- Screenshot capture: `cargo run --example p_map_editor_2d -- --screenshot path.png --exit-frame 45`
- MCP endpoints: `curl http://127.0.0.1:8088/mcp/get_output -o output.png`
- Hot reload: Edit Lua file, app updates within 1 second

**Phase 2 adds critical verification capability:**
- MCP endpoint to overwrite generator.lua: `POST /mcp/set_generator` with Lua source
- This enables AI-assisted testing of hot reload without manual file editing
- Layer filtering in get_output: `curl /mcp/get_output?layers=base` or `?layers=base,visualizer`

**Phase 2 verification uses these tools.** We extend MCP for iteration speed.

---

## Directory Structure

All new code goes in `crates/studio_core/src/map_editor/`. Following Phase 1 pattern: library-first, examples are thin wrappers.

```
crates/studio_core/src/map_editor/
├── mod.rs                         # Add new exports
├── asset/                         # NEW: Generic asset system (M7)
│   ├── mod.rs                     # Asset, AssetStore traits
│   ├── store.rs                   # InMemoryStore<T>
│   └── material.rs                # Material implements Asset (moved from material.rs)
├── generator/                     # NEW: Generator system (M6)
│   ├── mod.rs                     # Generator trait, StepInfo, GeneratorListener
│   └── lua.rs                     # LuaGenerator (refactored from lua_generator.rs)
├── render/                        # NEW: Render layer system (M5)
│   ├── mod.rs                     # RenderLayer trait, RenderLayerTree, PixelBuffer
│   ├── base.rs                    # BaseRenderLayer (voxels → pixels)
│   └── visualizer.rs              # LuaVisualizer (M6)
├── ui.rs                          # NEW: ImGui systems (extracted from app.rs)
├── mcp.rs                         # Refactored: thin wrapper, uses shared render
├── app.rs                         # Add new systems, remove old rendering
├── voxel_buffer_2d.rs             # Keep as-is
├── playback.rs                    # Keep as-is
├── imgui_screenshot.rs            # Keep as-is
└── [DELETED: checkerboard.rs]     # Superseded by Lua generator

assets/map_editor/
├── materials.lua                  # Existing
├── generator.lua                  # Existing
├── renderers/                     # NEW (M5)
│   └── grid_2d.lua                # Base renderer
└── visualizers/                   # NEW (M6)
    └── step_highlight.lua         # Step visualizer
```

### Files by Milestone

| Milestone | Files Created/Modified |
|-----------|----------------------|
| **M4.5** | `asset/mod.rs` (Asset, AssetStore traits) |
|          | `asset/store.rs` (InMemoryStore<T>) |
|          | `asset/material.rs` (Material implements Asset) |
|          | `mcp.rs` (add search endpoint) |
| **M4.75** | `app.rs` (PlaybackState starts playing) |
|           | `mcp.rs` (add set_generator, set_materials) |
| **M5** | `render/mod.rs` (RenderLayer, RenderLayerTree, PixelBuffer) |
|        | `render/base.rs` (BaseRenderLayer) |
|        | `mcp.rs` (add list_layers, get_output?layers, set_renderer) |
|        | `assets/map_editor/renderers/grid_2d.lua` |
| **M6** | `generator/mod.rs` (Generator, StepInfo, GeneratorListener) |
|        | `generator/lua.rs` (LuaGenerator refactored) |
|        | `render/visualizer.rs` (LuaVisualizer) |
|        | `assets/map_editor/visualizers/step_highlight.lua` |

---

## Current State Assessment

### File Inventory (What Exists After Phase 1)

| File | Purpose | Status |
|------|---------|--------|
| `app.rs` | Main app builder, UI systems | Keep, extract UI to `ui.rs` |
| `voxel_buffer_2d.rs` | 2D grid storage | Keep |
| `material.rs` | Material struct + MaterialPalette | Refactor: implement `Asset` trait |
| `playback.rs` | Play/pause/step state | Keep |
| `checkerboard.rs` | Fallback pattern generator | **Delete** - superseded by Lua generator |
| `lua_generator.rs` | Lua generator loading + hot reload | Refactor: emit `StepInfo` events |
| `lua_materials.rs` | Lua materials loading + hot reload | Refactor: generic loader pattern |
| `mcp_server.rs` | HTTP API | Thin out: move rendering to shared code |
| `imgui_screenshot.rs` | Screenshot capture | Keep |

### Architectural Problems to Fix

**1. No Generic Store Abstraction**

Materials and generators are both assets with hot reload, but have separate implementations. Should be ONE generic `AssetStore<T>` system.

**2. MaterialPalette Conflates Two Concepts**

`MaterialPalette.available` is a store, `MaterialPalette.active` is a selection. These should be separate.

**3. MCP Server Has Rendering Logic**

`generate_png_from_buffer()` duplicates rendering. MCP should call shared renderer.

**4. No Trait Hierarchy**

No `Asset` trait, no `Generator` trait, no `RenderLayer` trait. Everything is concrete types.

---

## High-Level Summary

**What changes in Phase 2:**

| Area | Before (Phase 1) | After (Phase 2) |
|------|------------------|-----------------|
| **Rendering** | Rust code maps voxels→pixels | `RenderLayer` trait, Lua layers, compositing |
| **Generator Events** | None | `GeneratorListener` trait, `StepInfo` events |
| **Asset Storage** | Ad-hoc `MaterialPalette` | Generic `AssetStore<T>` trait |
| **Search** | None | `AssetStore::search()` + MCP endpoint |
| **Generator Startup** | Fills immediately | Runs step-by-step on launch |

**Why these changes:**
- Lua rendering enables user customization without recompiling
- Generator events enable visualizers and debugging tools
- Generic store enables unified search (M7 requirement)
- Step-by-step startup makes visualizer immediately useful

---

## Trait Hierarchy (Phase 2 Target)

```
┌─────────────────────────────────────────────────────────────┐
│                         STORAGE                              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Asset (trait)                                              │
│     fn name() -> &str                                        │
│     fn asset_type() -> &'static str                          │
│           │                                                  │
│           ├── Material (struct)                              │
│           ├── GeneratorDef (struct) [Future]                 │
│           └── RenderLayerDef (struct) [Future]               │
│                                                              │
│   AssetStore<T: Asset> (trait)                               │
│     fn get(id) -> Option<&T>                                 │
│     fn list() -> &[T]                                        │
│     fn set(asset) -> id                                      │
│     fn search(query) -> Vec<&T>                              │
│           │                                                  │
│           └── InMemoryStore<T> (struct)                      │
│                                                              │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                       GENERATION                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Generator (trait)                                          │
│     fn init(ctx: &mut GeneratorContext)                      │
│     fn step(ctx: &mut GeneratorContext) -> bool              │
│     fn reset()                                               │
│           │                                                  │
│           └── LuaGenerator (struct)                          │
│                                                              │
│   GeneratorListener (trait)                                  │
│     fn on_step(info: &StepInfo)                              │
│           │                                                  │
│           └── LuaVisualizer (also implements RenderLayer)    │
│                                                              │
│   StepInfo (struct)                                          │
│     step_number, x, y, material_id, completed                │
│                                                              │
│   GeneratorContext (struct)                                  │
│     buffer: &mut VoxelBuffer2D                               │
│     palette: &[u32]                                          │
│                                                              │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                        RENDERING                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   RenderLayer (trait)                                        │
│     fn name() -> &str                                        │
│     fn enabled() -> bool                                     │
│     fn render(ctx: &RenderContext, pixels: &mut PixelBuffer) │
│           │                                                  │
│           ├── BaseRenderLayer (voxels → pixels)              │
│           └── LuaVisualizer (overlay)                        │
│                                                              │
│   RenderLayerTree (struct)                                   │
│     layers: Vec<Box<dyn RenderLayer>>                        │
│     fn render_all(ctx, pixels)                               │
│     fn render_filtered(ctx, pixels, layer_names)             │
│                                                              │
│   RenderContext (struct)                                     │
│     buffer: &VoxelBuffer2D                                   │
│     materials: &dyn AssetStore<Material>                     │
│                                                              │
│   PixelBuffer (struct)                                       │
│     data: Vec<u8>  // RGBA                                   │
│     width, height                                            │
│     fn set_pixel(x, y, r, g, b, a)                           │
│     fn get_pixel(x, y) -> (r, g, b, a)                       │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase Outcome

**When Phase 2 is complete, I can:**
- Search for any material by name
- Define how terrain is rendered in Lua (custom colors, effects)
- See which cell the generator is filling as it runs

**Phase Foundation:** Introduces three core abstractions:
1. `Asset` + `AssetStore<T>` — unified asset management with search; all storable things implement `Asset`
2. `RenderLayer` trait — compositable rendering; all visual overlays are just layers
3. `GeneratorListener` trait — observable generation; any system can listen to step events

These foundations enable Phase 3+ features (semantic search, multiple visualizers, debug overlays) without refactoring.

---

## Milestones

| M# | Functionality | Foundation |
|----|---------------|------------|
| M4.5 | I can search materials by name | `Asset` trait, `AssetStore<T>` with `search()` |
| M4.75 | I can watch generation step-by-step on launch; push generators via curl | MCP write endpoints (`set_generator`, `set_materials`) |
| M5 | I can edit Lua renderer and see output change live | `RenderLayer` trait, `RenderLayerTree` compositing |
| M6 | I can see which cell is being filled as generation runs | `GeneratorListener` trait, `StepInfo` events |

---

## M4.5: Asset Trait + Search

**Functionality:** I can search for materials by name and see matching results.

**Foundation:** `Asset` trait and `AssetStore<T>` with `search()` — unified interface for all asset types. Materials implement `Asset` first; future milestones add generators, renderers, visualizers as Assets.

### Why First

This foundation must come before M5/M6 because:
- M5's renderers can be stored in `AssetStore<Renderer>`
- M6's visualizers can be stored in `AssetStore<Visualizer>`
- Building the abstraction first prevents feature-specific code that gets refactored later

### API Definitions

**Asset trait:**
```rust
pub trait Asset: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn asset_type() -> &'static str where Self: Sized;
}
```

**AssetStore trait:**
```rust
pub trait AssetStore<T: Asset>: Send + Sync {
    fn get(&self, id: usize) -> Option<&T>;
    fn list(&self) -> &[T];
    fn set(&mut self, asset: T) -> usize;
    fn search(&self, query: &str) -> Vec<&T>;
}
```

**InMemoryStore** implements search with substring matching (case-insensitive).

**Material** implements `Asset`:
```rust
impl Asset for Material {
    fn name(&self) -> &str { &self.name }
    fn asset_type() -> &'static str { "material" }
}
```

**New MCP Endpoint:**
```
GET /mcp/search?q=stone&type=material
  Response: [{"type": "material", "name": "stone", "id": 1}]
```

### UI

Search panel with:
- Text input for query
- Results list
- Click result to select (for materials)

### Verification

```bash
# Search for stone
curl "http://127.0.0.1:8088/mcp/search?q=stone"
# Returns: [{"type":"material","name":"stone","id":1}]

# Search with no results
curl "http://127.0.0.1:8088/mcp/search?q=xyz"
# Returns: []
```

### M4.5 Verification Checklist

- [ ] `Asset` trait exists in `asset/mod.rs`
- [ ] `AssetStore<T>` trait exists with `get()`, `list()`, `set()`, `search()`
- [ ] `InMemoryStore<T>` implements `AssetStore<T>`
- [ ] `Material` implements `Asset` trait
- [ ] `GET /mcp/search?q=stone` returns matching materials as JSON
- [ ] UI shows search panel with text input and results list

### M4.5 Cleanup Audit

**Documented in [PHASE_2_CLEANUP.md](./PHASE_2_CLEANUP.md)**

- [x] `MaterialPalette.available` → `InMemoryStore<Material>` (Medium criticality)
- [x] Search implementation consistency (Low criticality)

---

## M4.75: Generator Runs on Launch + MCP Set Endpoints

**Functionality:** I can launch the app and watch generation happen step-by-step; I can push a new generator via curl and see it hot reload.

**Foundation:** MCP write endpoints (`set_generator`, `set_materials`) — pattern for all future MCP mutations. Enables programmatic asset updates without manual file editing.

### API Changes

**New MCP Endpoints:**
```
POST /mcp/set_generator
  Body: Lua source code (text/plain)
  Effect: Writes to generator.lua, triggers hot reload
  Response: {"success": true}

POST /mcp/set_materials  
  Body: Lua source code (text/plain)
  Effect: Writes to materials.lua, triggers hot reload
  Response: {"success": true}
```

**Behavior Change:**
- `PlaybackState` starts with `playing = true`
- Generator `init` no longer runs to completion; runs at `speed` cells/sec

### Verification

```bash
# Launch app, observe generation in progress (not completed)
cargo run --example p_map_editor_2d &
sleep 2

# Push stripes generator via MCP
curl -X POST http://127.0.0.1:8088/mcp/set_generator \
  -H "Content-Type: text/plain" \
  --data-binary @assets/map_editor/generator_stripes.lua

# Verify output changed
curl http://127.0.0.1:8088/mcp/get_output -o /tmp/stripes.png
```

### M4.75 Verification Checklist

- [ ] App launches with generation in progress (not completed)
- [ ] `POST /mcp/set_generator` accepts Lua source and triggers hot reload
- [ ] `POST /mcp/set_materials` accepts Lua source and triggers hot reload
- [ ] Generation restarts when generator is hot-reloaded

### M4.75 Cleanup Audit

**To be documented in [PHASE_2_CLEANUP.md](./PHASE_2_CLEANUP.md) after milestone completion.**

---

## M5: Lua Renderer with Layer System

**Functionality:** I can edit a Lua renderer file, save it, and see visual output change without restarting.

**Foundation:** `RenderLayer` trait with compositing (`RenderLayerTree`) — all visual overlays are just layers. M6's visualizer is "just another layer" with no special cases.

### Why

- Users can customize rendering without recompiling
- Layer system enables M6 visualizer as overlay
- Layer filtering enables debugging individual layers via MCP

### API Definitions

**RenderLayer trait:**
```rust
pub trait RenderLayer: Send + Sync {
    fn name(&self) -> &str;
    fn enabled(&self) -> bool;
    fn render(&mut self, ctx: &RenderContext, pixels: &mut PixelBuffer);
}
```

**RenderLayerTree** composites layers in order:
```rust
impl RenderLayerTree {
    pub fn render_all(&mut self, ctx: &RenderContext) -> PixelBuffer;
    pub fn render_filtered(&mut self, ctx: &RenderContext, names: &[&str]) -> PixelBuffer;
    pub fn list_layers(&self) -> Vec<&str>;
}
```

**New MCP Endpoints:**
```
GET /mcp/list_layers
  Response: ["base", "visualizer"]

GET /mcp/get_output?layers=base,visualizer
  Effect: Renders only specified layers
  Response: PNG image

POST /mcp/set_renderer
  Body: Lua source code
  Effect: Writes to renderers/grid_2d.lua, triggers hot reload
```

### Lua Protocol

Renderer Lua files implement:
```lua
-- renderers/grid_2d.lua
local Layer = {}
function Layer:render(ctx, pixels)
  -- ctx.width, ctx.height, ctx:get_voxel(x,y), ctx:get_material(id)
  -- pixels:set_pixel(x, y, r, g, b, a)
  -- pixels:get_pixel(x, y) -> r, g, b, a
end
return Layer
```

### Verification

```bash
# List layers
curl http://127.0.0.1:8088/mcp/list_layers
# Returns: ["base"]

# Get base layer only
curl "http://127.0.0.1:8088/mcp/get_output?layers=base" -o /tmp/base.png

# Push modified renderer
curl -X POST http://127.0.0.1:8088/mcp/set_renderer \
  -H "Content-Type: text/plain" \
  -d '... lua with inverted colors ...'

# Verify change
curl http://127.0.0.1:8088/mcp/get_output -o /tmp/inverted.png
```

### M5 Verification Checklist

- [ ] `GET /mcp/list_layers` returns `["base"]`
- [ ] `GET /mcp/get_output?layers=base` returns PNG with only base layer
- [ ] `POST /mcp/set_renderer` accepts Lua source and triggers hot reload
- [ ] Renderer Lua can call `ctx:get_voxel()`, `ctx:get_material()`, `pixels:set_pixel()`
- [ ] `assets/map_editor/renderers/grid_2d.lua` exists and works

### M5 Cleanup Audit

**To be documented in [PHASE_2_CLEANUP.md](./PHASE_2_CLEANUP.md) after milestone completion.**

---

## M6: Generator Visualizer

**Functionality:** I can see which cell the generator is currently filling as generation runs.

**Foundation:** `GeneratorListener` trait with `StepInfo` events — any system can observe generation. Visualizer is one listener; future uses include logging, analytics, debugging tools.

### Why

- Users can see generation progress
- Enables debugging generator behavior
- Visualizer is both `GeneratorListener` AND `RenderLayer`

### API Definitions

**StepInfo** (emitted after each generator step):
```rust
pub struct StepInfo {
    pub step_number: usize,
    pub x: usize,
    pub y: usize,
    pub material_id: u32,
    pub completed: bool,
}
```

**GeneratorListener trait:**
```rust
pub trait GeneratorListener: Send + Sync {
    fn on_step(&mut self, info: &StepInfo);
}
```

**LuaVisualizer** implements both:
- `GeneratorListener` (receives step events, updates internal state)
- `RenderLayer` (renders overlay based on state)

### Lua Protocol

Visualizer Lua files implement both protocols:
```lua
-- visualizers/step_highlight.lua
local Visualizer = {}

function Visualizer:on_step(step_info)
  -- step_info.step_number, .x, .y, .material_id, .completed
  self.last_x = step_info.x
  self.last_y = step_info.y
end

function Visualizer:render(ctx, pixels)
  -- Draw highlight at self.last_x, self.last_y
end

return Visualizer
```

### Verification

```bash
# List layers (should include visualizer)
curl http://127.0.0.1:8088/mcp/list_layers
# Returns: ["base", "visualizer"]

# Get visualizer layer only (highlight on black)
curl "http://127.0.0.1:8088/mcp/get_output?layers=visualizer" -o /tmp/vis_only.png

# UI shows Visualizer panel with step count, position, material
```

### M6 Verification Checklist

- [ ] Generator emits `StepInfo` after each step
- [ ] `LuaVisualizer` receives step events via `GeneratorListener`
- [ ] `GET /mcp/list_layers` returns `["base", "visualizer"]`
- [ ] `GET /mcp/get_output?layers=visualizer` returns overlay-only PNG
- [ ] Visualizer shows highlight at current generation position
- [ ] `assets/map_editor/visualizers/step_highlight.lua` exists and works

### M6 Cleanup Audit

**To be documented in [PHASE_2_CLEANUP.md](./PHASE_2_CLEANUP.md) after milestone completion.**

---

## Phase 2 Cleanup Notes

**See [PHASE_2_CLEANUP.md](./PHASE_2_CLEANUP.md) for detailed cleanup audit.**

The cleanup document tracks:
- Refactoring candidates identified during each milestone
- Current state vs proposed change with engineering rationale
- Criticality levels (High/Medium/Low)
- Recommended timing for each refactor

### Quick Reference

| Milestone | Key Cleanup Items | Criticality |
|-----------|-------------------|-------------|
| M4.5 | `MaterialPalette.available` → `InMemoryStore<Material>` | Medium |
| M4.5 | Search implementation consistency | Low |
| M4.75 | TBD | |
| M5 | TBD | |
| M6 | TBD | |

### Cleanup Decision

At Phase 2 end, review [PHASE_2_CLEANUP.md](./PHASE_2_CLEANUP.md) and decide:
- **Do now:** Items that block Phase 3 or create significant tech debt
- **Defer:** Items that are nice-to-have but don't block progress
- **Drop:** Items that turned out to be unnecessary

---

## Files Changed

### Deleted

| File | Reason |
|------|--------|
| `checkerboard.rs` | Superseded by Lua generator |

### Refactored

| Current | New Location | Change |
|---------|--------------|--------|
| `material.rs` | `asset/material.rs` | Implement `Asset` trait |
| `lua_materials.rs` | `asset/loader.rs` | Generic hot-reload loader |
| `lua_generator.rs` | `generator/lua.rs` | Emit `StepInfo` events |
| `mcp_server.rs` | `mcp.rs` | Thin wrapper, calls shared render |

### New

| File | Purpose |
|------|---------|
| `asset/mod.rs` | `Asset`, `AssetStore` traits |
| `asset/store.rs` | `InMemoryStore<T>` |
| `generator/mod.rs` | `Generator`, `GeneratorListener`, `StepInfo` |
| `render/mod.rs` | `RenderLayer`, `RenderLayerTree`, `PixelBuffer` |
| `render/base.rs` | Base voxel→pixel layer |
| `render/visualizer.rs` | `LuaVisualizer` |
| `ui.rs` | ImGui systems (extracted from `app.rs`) |

---

## Final Verification Script

```bash
#!/bin/bash
set -e

echo "=== Phase 2 Verification ==="

cargo run --example p_map_editor_2d &
APP_PID=$!
sleep 5

# M4.5: Asset + Search
echo "M4.5: Asset + Search..."
curl -s "http://127.0.0.1:8088/mcp/search?q=stone" | grep -q "stone" && echo "PASS: search"

# M4.75: Generator runs, MCP set works
echo "M4.75: Generator + MCP set..."
curl -s -X POST http://127.0.0.1:8088/mcp/set_generator \
  -H "Content-Type: text/plain" \
  --data-binary @assets/map_editor/generator_stripes.lua | grep -q "success" && echo "PASS: set_generator"

# M5: Layer system
echo "M5: Render layers..."
curl -s http://127.0.0.1:8088/mcp/list_layers | grep -q "base" && echo "PASS: list_layers"
curl -s "http://127.0.0.1:8088/mcp/get_output?layers=base" -o /tmp/base.png && echo "PASS: layer filter"

# M6: Visualizer
echo "M6: Visualizer..."
curl -s http://127.0.0.1:8088/mcp/list_layers | grep -q "visualizer" && echo "PASS: visualizer layer"

kill $APP_PID 2>/dev/null
echo "=== Phase 2 Complete ==="
```

---

## Estimated Time

| Milestone | Time |
|-----------|------|
| M4.5 (Asset trait + Search) | 3 hours |
| M4.75 (Generator on launch + MCP set) | 2 hours |
| M5 (RenderLayer system) | 4 hours |
| M6 (GeneratorListener + Visualizer) | 4 hours |
| **Total** | **13 hours** |

---

## Dependencies

**Phase 1 → Phase 2:**
- `VoxelBuffer2D` → Used by `RenderContext`
- `Material` → Implements `Asset`
- Hot reload infrastructure → Generalized in `asset/loader.rs`
- MCP server → Extended with new endpoints
