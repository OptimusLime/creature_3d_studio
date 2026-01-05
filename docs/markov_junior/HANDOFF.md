# MarkovJunior Rust Port - Handoff Document

## Current State

**Branch:** `feature/markov-junior-rust`
**Phase:** Phase 3.6 COMPLETE (3D VoxelWorld Live Rendering)
**Tests:** 280 passing (237 Phase 1 + 25 Phase 2 + 18 Phase 3)
**Total Lines:** ~10,200

---

## Summary of Completed Work

### Phase 1: Core Algorithm (COMPLETE) - 237 tests
All MarkovJunior algorithm features ported from C#:
- Grid, Rule, Symmetry, Nodes (One, All, Parallel, Sequence, Markov)
- Interpreter, XML Loader, Fields, Paths, Heuristics
- WFC (Overlap, Tile), Convolution, ConvChain
- 3D Symmetries, VOX Loading

### Phase 2: Lua Integration (COMPLETE) - 25 tests
- `mj.load_model()`, `mj.create_model()` - model loading/creation
- `model:run()`, `model:step()`, `model:run_animated()` - execution
- `grid:to_voxel_world()` - VoxelWorld conversion
- `scene.set_voxel_world()` - stores in GeneratedVoxelWorld resource

### Phase 3.0: PNG Rendering Foundation (COMPLETE) - 11 tests
- `render.rs` module - direct grid → PNG (no Bevy required)
- `render_2d()` - flat 2D grid rendering
- `render_3d_isometric()` - isometric 3D projection
- `Model::load_with_size()` - load XML with custom dimensions
- Automated tests that run real models and output PNGs

### Phase 3.1: Rendering Quality Verification (COMPLETE) - 3 tests
- `RenderPalette` - loads colors from C# palette.xml (50+ character→color mappings)
- `colors_for_grid()` - maps grid character indices to proper palette colors
- Fixed 2D color bug: MazeGrowth now renders gray/white (A/W) instead of red/white
- Fixed 3D isometric artifacts: vertical banding on right face eliminated
- Matched C# Sprite brightness values: top=215, left=143, right=71

### Phase 3.2: Bevy 3D Example (COMPLETE) - 1 test
- `examples/p26_markov_bevy_3d.rs` - full 3D rendering example
- `MjPalette::from_grid()` - creates VoxelWorld palette from grid's character set
- Uses programmatic B→W growth model in 16³ grid
- Deferred lighting with proper shading and shadows
- Screenshot saved to `screenshots/p26_markov_bevy_3d.png`

### Phase 3.3: ImGui PNG Save Button (COMPLETE) - 3 tests
- `grid:render_to_png(path, [pixel_size])` - Lua method for saving PNGs
- "Save PNG" button in main.lua MarkovJunior window
- Saves isometric 3D renders to `screenshots/mj_generated_<seed>.png`
- Uses proper palette.xml colors via `colors_for_grid()`

### Phase 3.6: 3D VoxelWorld Live Rendering (COMPLETE)
- `render_generated_voxel_world` system in `studio_scripting/src/lib.rs`
- Watches `GeneratedVoxelWorld` resource for dirty flag
- Despawns old meshes, builds new chunk meshes with greedy meshing
- Spawns with `DeferredRenderable` marker for proper lighting
- `GeneratedVoxelMesh` component for cleanup tracking
- Main app now includes `VoxelMaterialPlugin` and `DeferredRenderingPlugin`
- `DeferredPointLight` added for scene lighting

### Phase 3.X: Enhanced Demo (COMPLETE)
- `examples/p27_markov_imgui.rs` - standalone test example
- `main.lua` updated with model type selection:
  - **Growth** - organic 3D growth from center (16³)
  - **Maze3D** - 3D maze corridors using WBB→WAW rule (17³)
  - **Dungeon** - floor expansion pattern (24×24×8)
- "Step x100" button for incremental animation
- Model switching with automatic reset

---

## How to Use

### Run the Main App
```bash
cargo run
```

1. **Select model type**: Click Growth, Maze3D, or Dungeon
2. **Generate**: Click "Generate" to create 3D structure
3. **Animate**: Click "Step x100" repeatedly to watch it grow
4. **Save**: Click "Save PNG" to save isometric render

### Run the Standalone Example
```bash
cargo run --example p27_markov_imgui
```
Creates PNG and 3D screenshot automatically.

### Run All Tests
```bash
cargo test -p studio_core markov_junior
# Expected: 280 passed
```

---

## Remaining Phases

### Phase 3.4: ImGui Inline Image Display (PENDING)

**Outcome:** Generated 2D output displays directly in ImGui window (no file save required).

**Verification:**
1. Run `cargo run`
2. Click "Generate 2D" button
3. Image appears INSIDE the ImGui window
4. Regenerating updates the displayed image

**Tasks:**
1. Add `imgui.image_from_bytes(rgba_bytes, width, height)` Lua function
2. Create Bevy texture from RGBA bytes
3. Display texture in ImGui using bevy_mod_imgui
4. Handle texture cleanup/replacement on regenerate

**Est. Lines:** ~100 Rust, ~30 Lua

---

### Phase 3.5: ImGui Animation Preview (PENDING)

**Outcome:** Can watch 2D MarkovJunior execute step-by-step in ImGui window.

**Verification:**
1. Run `cargo run`
2. Click "Animate" button
3. Image updates showing generation progress
4. "Pause/Resume" button works
5. "Speed" slider controls steps per frame

**Tasks:**
1. Add animation state to Lua (model, running, speed)
2. Each frame: if running, call model:step() N times
3. Re-render and update ImGui texture
4. Add pause/resume/speed controls

**Est. Lines:** ~80 Rust, ~50 Lua

---

### Phase 3.7: Model Browser Dropdown (PENDING)

**Outcome:** Dropdown in ImGui listing all XML models, can select and run any.

**Verification:**
1. Run `cargo run`
2. Dropdown shows all models from MarkovJunior/models/*.xml
3. Select "MazeBacktracker.xml" → runs and displays 2D output
4. Select "Growth.xml" → runs and displays different output
5. Toggle "3D Mode" → runs with mz > 1

**Tasks:**
1. Scan MarkovJunior/models/ directory for XML files
2. Populate Lua table with model names
3. Add dropdown widget to main.lua
4. On selection: load model, run, render
5. Add 2D/3D toggle that sets grid dimensions

**Est. Lines:** ~60 Rust, ~80 Lua

---

### Phase 3.8: 2D Texture Viewport (PENDING)

**Outcome:** Full-window 2D view rendering MarkovJunior output as a Bevy texture (not ImGui).

**Verification:**
1. Run `cargo run --example p28_markov_2d_viewport`
2. Entire window shows 2D MarkovJunior output
3. Press Space to regenerate with new seed
4. Press S to save screenshot

**Tasks:**
1. Create Bevy 2D camera setup
2. Create texture from grid RGBA bytes
3. Render texture as full-screen quad
4. Handle input for regenerate/save

**Est. Lines:** ~120

---

## Phase Summary Table

| Phase | Description | Status | Tests Added |
|-------|-------------|--------|-------------|
| 3.0 | PNG Rendering Foundation | COMPLETE | 11 |
| 3.1 | Rendering Quality Verification | COMPLETE | 3 |
| 3.2 | Bevy Example with Screenshot | COMPLETE | 1 |
| 3.3 | ImGui PNG Save Button | COMPLETE | 3 |
| 3.4 | ImGui Inline Image Display | PENDING | - |
| 3.5 | ImGui Animation Preview | PENDING | - |
| 3.6 | 3D VoxelWorld Live Rendering | COMPLETE | 0 |
| 3.7 | Model Browser Dropdown | PENDING | - |
| 3.8 | 2D Texture Viewport | PENDING | - |

**Remaining:** 4 phases (~400 lines)

---

## Key Files

| File | Purpose |
|------|---------|
| `crates/studio_core/src/markov_junior/render.rs` | PNG rendering, RenderPalette, isometric cubes |
| `crates/studio_core/src/markov_junior/lua_api.rs` | Lua API: mj.*, grid:render_to_png() |
| `crates/studio_core/src/markov_junior/voxel_bridge.rs` | Grid → VoxelWorld, MjPalette::from_grid |
| `crates/studio_scripting/src/lib.rs` | GeneratedVoxelWorld, render_generated_voxel_world system |
| `src/main.rs` | Main app with VoxelMaterialPlugin, DeferredRenderingPlugin |
| `examples/p26_markov_bevy_3d.rs` | Standalone 3D Bevy rendering example |
| `examples/p27_markov_imgui.rs` | Phase 3.3/3.6 verification example |
| `assets/scripts/ui/main.lua` | MarkovJunior demo with model selection |
| `MarkovJunior/resources/palette.xml` | C# color palette reference |
| `MarkovJunior/source/Graphics.cs` | C# rendering reference |

---

## Commands

```bash
# Run all MarkovJunior tests (280)
cargo test -p studio_core markov_junior

# Run render tests only
cargo test -p studio_core markov_junior::render

# Run Lua API tests only
cargo test -p studio_core markov_junior::lua_api

# View generated test PNGs
ls screenshots/test_markov_*.png

# Run main app (interactive demo)
cargo run

# Run p26 example (3D Bevy screenshot)
cargo run --example p26_markov_bevy_3d

# Run p27 example (Phase 3.3/3.6 verification)
cargo run --example p27_markov_imgui
```

---

## Critical Reminders

1. **280 tests must pass** before any commit
2. **HOW_WE_WORK.md** - incremental, verifiable, automated
3. **Each phase has verification criteria** - don't mark complete until verified
4. **PNG output is deterministic** - same seed = same image
5. **Value 0 is always transparent** - matches C# convention `visible[i] = value != 0`
6. **Use colors_for_grid()** for PNG rendering, `MjPalette::from_grid()` for VoxelWorld
7. **Main app requires** VoxelMaterialPlugin + DeferredRenderingPlugin for 3D rendering
