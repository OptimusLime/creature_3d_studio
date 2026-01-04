# MarkovJunior Rust Port - Handoff Document

## Current State

**Branch:** `feature/markov-junior-rust`
**Phase:** Phase 3.0 COMPLETE (PNG Rendering Foundation), Phase 3.1+ PENDING
**Tests:** 273 passing (237 Phase 1 + 25 Phase 2 + 11 Phase 3.0)
**Total Lines:** ~9,650

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
- `scene.set_voxel_world()` - stores in GeneratedVoxelWorld resource (NOT YET RENDERED)

### Phase 3.0: PNG Rendering Foundation (COMPLETE) - 11 tests
- `render.rs` module - direct grid → PNG (no Bevy required)
- `render_2d()` - flat 2D grid rendering
- `render_3d_isometric()` - isometric 3D projection
- `Model::load_with_size()` - load XML with custom dimensions
- Automated tests that run real models and output PNGs

---

## Phase 3: Full Integration Plan

Following HOW_WE_WORK.md principles: incremental, verifiable, automated.

### Phase 3.1: Verify 2D Rendering Quality

**Outcome:** Our 2D PNG output matches or approximates C# MarkovJunior output quality.

**Verification:**
1. Run `cargo test -p studio_core test_markov_render_quality_2d`
2. Test loads MazeBacktracker.xml, runs with seed 42
3. Compares output dimensions, non-zero cell count to expected values
4. PNG output visually inspected: corridors visible, maze structure clear

**Tasks:**
1. Add test that verifies MazeBacktracker output has expected structure
2. Add test that verifies pixel colors match palette correctly
3. Document any visual differences from C# reference

**Est. Lines:** ~50

---

### Phase 3.2: Verify 3D Rendering Quality  

**Outcome:** Our 3D isometric rendering produces recognizable structures.

**Current Issue:** The 3D renders look "suspicious" - need to verify against C# reference.

**Verification:**
1. Run `cargo test -p studio_core test_markov_render_quality_3d`
2. Compare our isometric cube render to C# Graphics.cs IsometricRender
3. Verify face shading (top/left/right brightness)
4. Verify depth sorting (back-to-front painter's algorithm)

**Tasks:**
1. Create test with known 3D structure (e.g., 3x3x3 staircase)
2. Verify each visible face has correct color/shading
3. Compare to C# output if possible
4. Fix any rendering bugs found

**Est. Lines:** ~100

---

### Phase 3.3: Bevy Example with Screenshot

**Outcome:** Example that loads MarkovJunior XML, runs it, renders in Bevy 3D, takes screenshot.

**File:** `examples/p26_markov_bevy_3d.rs`

**Verification:**
1. Run `cargo run --example p26_markov_bevy_3d`
2. Screenshot saved to `screenshots/p26_markov_bevy_3d.png`
3. Screenshot shows 3D voxel structure (NOT isometric PNG, real 3D)
4. Console prints "Generated N voxels" where N > 100

**Tasks:**
1. Create example using VoxelWorldApp pattern
2. Load MazeGrowth.xml or similar model
3. Convert grid to VoxelWorld
4. Render with deferred lighting + shadows
5. Auto-screenshot and exit

**Est. Lines:** ~80

---

### Phase 3.4: ImGui PNG Save Button

**Outcome:** ImGui button that generates 2D MarkovJunior output and saves PNG to disk.

**File:** Modify `assets/scripts/ui/main.lua` and `crates/studio_scripting/src/lib.rs`

**Verification:**
1. Run `cargo run`
2. Click "Generate 2D" button in MarkovJunior window
3. Console shows "Saved to screenshots/mj_generated.png"
4. PNG file exists and shows maze pattern

**Tasks:**
1. Add `mj.render_to_png(grid, path, pixel_size)` Lua function
2. Add to main.lua: button that calls render_to_png
3. Test hot-reload of script

**Est. Lines:** ~40 Rust, ~20 Lua

---

### Phase 3.5: ImGui Inline Image Display

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

### Phase 3.6: ImGui Animation Preview

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

### Phase 3.7: 3D VoxelWorld Live Rendering

**Outcome:** Generated 3D MarkovJunior output renders in the Bevy 3D viewport.

**Verification:**
1. Run `cargo run`
2. Click "Generate 3D" in MarkovJunior window
3. 3D voxel structure appears in scene (not ImGui, actual 3D)
4. Click again: old structure removed, new one appears

**Tasks:**
1. Create `render_generated_voxel_world` system
2. Read from `GeneratedVoxelWorld` resource (already populated by scene.set_voxel_world)
3. Build chunk meshes using `build_chunk_mesh_greedy`
4. Spawn with marker component for cleanup
5. Register system in ScriptingPlugin

**Est. Lines:** ~80

---

### Phase 3.8: Model Browser Dropdown

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

### Phase 3.9: 2D Texture Viewport

**Outcome:** Full-window 2D view rendering MarkovJunior output as a Bevy texture (not ImGui).

**Verification:**
1. Run `cargo run --example p27_markov_2d_viewport`
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

| Phase | Description | Dependencies | Est. Lines | Status |
|-------|-------------|--------------|------------|--------|
| 3.0 | PNG Rendering Foundation | - | ~400 | COMPLETE |
| 3.1 | Verify 2D Rendering Quality | 3.0 | ~50 | PENDING |
| 3.2 | Verify 3D Rendering Quality | 3.0 | ~100 | PENDING |
| 3.3 | Bevy Example with Screenshot | 3.0 | ~80 | PENDING |
| 3.4 | ImGui PNG Save Button | 3.0 | ~60 | PENDING |
| 3.5 | ImGui Inline Image Display | 3.4 | ~130 | PENDING |
| 3.6 | ImGui Animation Preview | 3.5 | ~130 | PENDING |
| 3.7 | 3D VoxelWorld Live Rendering | 3.3 | ~80 | PENDING |
| 3.8 | Model Browser Dropdown | 3.4, 3.7 | ~140 | PENDING |
| 3.9 | 2D Texture Viewport | 3.0 | ~120 | PENDING |

**Total Remaining:** ~890 lines across 9 phases

---

## Execution Order

**Recommended order based on dependencies and value:**

1. **Phase 3.1-3.2** (Verification) - Ensure our rendering is correct before building more
2. **Phase 3.3** (Bevy Example) - Proves full 3D integration works
3. **Phase 3.4** (ImGui Save) - Quick win, simple feature
4. **Phase 3.7** (3D Live Render) - Big visual impact
5. **Phase 3.5** (ImGui Display) - Enables fast iteration
6. **Phase 3.6** (Animation) - Cool demo feature
7. **Phase 3.8** (Model Browser) - Polish feature
8. **Phase 3.9** (2D Viewport) - Alternative visualization

---

## Commands

```bash
# Run all tests (should be 273)
cargo test -p studio_core markov_junior

# Run render tests only (11)
cargo test -p studio_core markov_junior::render

# View generated test PNGs
ls screenshots/test_markov_*.png

# Run main app (has MarkovJunior ImGui window)
cargo run

# Run existing p25 example (hardcoded cross)
cargo run --example p25_markov_junior
```

---

## Key Files

| File | Purpose |
|------|---------|
| `crates/studio_core/src/markov_junior/render.rs` | PNG rendering (Phase 3.0) |
| `crates/studio_core/src/markov_junior/model.rs` | Model API, load_with_size |
| `crates/studio_core/src/markov_junior/voxel_bridge.rs` | Grid → VoxelWorld |
| `crates/studio_scripting/src/lib.rs` | GeneratedVoxelWorld resource, scene.set_voxel_world |
| `assets/scripts/ui/main.lua` | MarkovJunior demo window |
| `docs/markov_junior/IMPLEMENTATION_PLAN.md` | Original full plan |

---

## Critical Reminders

1. **273 tests must pass** before any commit
2. **HOW_WE_WORK.md** - incremental, verifiable, automated
3. **Each phase has verification criteria** - don't mark complete until verified
4. **PNG output is deterministic** - same seed = same image
5. **GeneratedVoxelWorld resource exists** but nothing renders it yet (Phase 3.7)
