# Phase 1 Specification: Foundation (2D Map Editor)

*Following HOW_WE_WORK.md and WRITING_MILESTONES.md.*

**Key principle:** Every task produces visible, verifiable functionality. Verification requires ZERO additional work—we look at a screenshot or check a file that already exists.

---

## Phase Outcome

**When Phase 1 is complete, I can:**
- Edit Lua materials and generators, see changes live in 2D
- External AI can create assets via MCP
- Step through generation with playback controls (play/pause/step/speed)

---

## Verification Infrastructure

**We use screenshot-based verification.** The example outputs `screenshots/p_map_editor_2d.png` automatically. We (and AI) can look at this file to verify functionality.

This follows existing patterns: `p0_screenshot_test.rs`, `p27_markov_imgui.rs`.

---

## Milestones in Phase 1

| M# | Functionality | Verification |
|----|---------------|--------------|
| M1 | Pick from 2 materials, see checkerboard, control playback | Screenshot shows grid + picker + playback controls |
| M2 | Edit Lua materials file, see update live | Screenshot changes when materials.lua changes |
| M3 | Edit Lua generator file, see terrain change live | Screenshot changes when generator.lua changes |
| M4 | External AI creates assets via MCP | `curl` returns JSON, screenshot shows new material |

---

## M1: Static End-to-End with Playback Controls

### Outcome
Run `cargo run --example p_map_editor_2d` and:
1. Screenshot `screenshots/p_map_editor_2d.png` is created
2. Screenshot shows: 32x32 checkerboard, material picker with "stone"/"dirt", playback controls
3. UI is functional (clicking/stepping works)

### Key Design Decisions

**Single-file example.** Following `p0_screenshot_test.rs` pattern—everything in one file for now. Extract to crate later if needed.

**bevy_mod_imgui, not egui.** Already in codebase via `studio_scripting`. Provides immediate-mode UI with docking.

**Core resources:**
- `VoxelBuffer2D` — 2D grid of material IDs (width × height)
- `MaterialPalette` — list of materials with id/name/color, tracks selection
- `PlaybackState` — playing/paused, speed, step index, completion flag

**Why step-by-step playback?** Generators fill cells one at a time. Playback controls let you watch/debug generation. Speed slider controls cells-per-second.

### Tasks

---

#### M1.1: Minimal window with screenshot output
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Window opens, takes a screenshot, exits.

**Verification:** File `screenshots/p_map_editor_2d.png` exists after running.

```bash
cargo run --example p_map_editor_2d
ls screenshots/p_map_editor_2d.png  # File exists
```

**What to build:**
- Bevy app with `bevy_mod_imgui`
- Screenshot capture after N frames (copy from p0_screenshot_test.rs pattern)
- Window title: "Map Editor 2D"

**Done when:** Screenshot file is created. Can be blank/magenta—we just need the infrastructure.

---

#### M1.2: Render checkerboard to screenshot
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Screenshot shows a 32x32 checkerboard (two colors).

**Verification:** Open `screenshots/p_map_editor_2d.png` and see checkerboard pattern.

**What to build:**
- `VoxelBuffer2D` struct (width, height, data)
- `Material` struct (id, name, color)
- Hardcoded 2 materials: stone (gray), dirt (brown)
- Checkerboard generator that fills buffer
- Render buffer to texture, display in ImGui window
- Screenshot captures this

**Done when:** Screenshot shows gray/brown checkerboard pattern.

---

#### M1.3: Material picker panel
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Screenshot shows material list with names and color swatches.

**Verification:** Screenshot shows "stone" and "dirt" labels with colored boxes.

**What to build:**
- ImGui side panel (left side)
- List materials with colored buttons/swatches
- Display material name and color

**Done when:** Screenshot shows left panel with "stone" (gray) and "dirt" (brown) listed.

---

#### M1.4: Material selection changes checkerboard
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Clicking a material changes the checkerboard colors.

**Verification:** 
1. Run example
2. Click "dirt" button
3. Screenshot updates to show different checkerboard colors

**What to build:**
- Track "selected material" state
- On click: change generator's material_a to selected material
- Regenerate checkerboard
- Re-render

**Done when:** Manually verify clicking changes the checkerboard. Screenshot after clicking shows different colors.

---

#### M1.5: Playback controls (Play/Pause/Step/Speed)
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Screenshot shows playback controls. They work.

**Verification:** Screenshot shows Play, Pause, Step buttons and Speed slider in bottom area.

**What to build:**
- ImGui bottom panel with:
  - Play/Pause toggle button
  - Step button
  - Speed slider (1-1000 cells/sec)
- `PlaybackState` resource (playing, speed, step_index)
- Step-by-step generator (fill one cell at a time)
- Playback system that advances based on speed

**Done when:** 
1. Screenshot shows playback controls
2. Manually verify: Click Step → one cell fills. Click Play → animation runs. Pause stops it.

---

### M1 Verification Checklist

All verification is by looking at `screenshots/p_map_editor_2d.png` or manual interaction:

- [ ] Screenshot file exists after running
- [ ] Screenshot shows 32x32 checkerboard
- [ ] Screenshot shows material picker with "stone" and "dirt"
- [ ] Screenshot shows playback controls (Play/Pause/Step/Speed)
- [ ] Manual: clicking material changes checkerboard
- [ ] Manual: Step advances one cell
- [ ] Manual: Play/Pause controls animation
- [ ] **This is a working app**

---

## M2: Lua Materials + Hot Reload

### Outcome
Materials loaded from `assets/map_editor/materials.lua`. Edit file, save, screenshot updates within 1 second.

### Key Design Decisions

**Lua for data, not code (yet).** `materials.lua` just returns a table of material definitions. Simple eval, no complex API.

**File format:**
```lua
return {
    { id = 1, name = "stone", color = {0.5, 0.5, 0.5} },
    { id = 2, name = "dirt",  color = {0.6, 0.4, 0.2} },
}
```

**Hot reload via `notify` crate.** Already used in `studio_scripting`. Watch `assets/map_editor/`, reload on change.

**Why hot reload matters:** AI/human edits Lua → sees result immediately. No restart cycle.

### Tasks

---

#### M2.1: Create materials.lua and load at startup
**Files:** 
- `assets/map_editor/materials.lua` (new)
- `examples/p_map_editor_2d.rs` (modify)

**Functionality:** Materials come from Lua file instead of hardcoded.

**Verification:** 
1. Screenshot shows same checkerboard as before
2. Edit `materials.lua` to add third material "crystal" with purple color
3. Restart app
4. Screenshot shows 3 materials in picker

**Done when:** Adding a material to Lua file → appears in screenshot after restart.

---

#### M2.2: Hot reload materials on file change
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Edit materials.lua, save, picker updates without restart.

**Verification:**
1. App running
2. Edit `materials.lua`: change stone color from gray to red `{1.0, 0.0, 0.0}`
3. Save file
4. Screenshot taken after 1 second shows red checkerboard

**What to build:**
- File watcher on `assets/map_editor/` (use notify crate, pattern from studio_scripting)
- On materials.lua change: reload, regenerate, re-render

**Done when:** Change color in Lua → screenshot shows new color without restart.

---

### M2 Verification Checklist

- [ ] `materials.lua` exists with stone/dirt definitions
- [ ] Screenshot shows materials from Lua file
- [ ] Add material to Lua, restart → screenshot shows new material
- [ ] Change color in Lua, save (no restart) → screenshot updates within 1 second
- [ ] **Hot reload works**

---

## M3: Lua Generator + Hot Reload

### Outcome
Generator loaded from `assets/map_editor/generator.lua`. Edit file, save, terrain updates within 1 second.

### Key Design Decisions

**Generator protocol: init/step/reset.** Generator is a Lua table with three methods:
- `init(ctx)` — initialize state
- `step(ctx) → bool` — fill one cell, return true when done
- `reset()` — reset to initial state

**Why step-by-step?** Matches playback controls from M1. Each step = one cell filled. Enables pause/resume/rewind.

**Context API:**
- `ctx.width`, `ctx.height` — buffer dimensions
- `ctx:set_voxel(x, y, material_id)` — write to buffer
- `ctx:get_voxel(x, y) → material_id` — read from buffer

**Why this API?** Minimal surface area. Generator only needs to read/write cells. No complex state management.

### Tasks

---

#### M3.1: Create generator.lua with init/step/reset
**Files:**
- `assets/map_editor/generator.lua` (new)
- `examples/p_map_editor_2d.rs` (modify)

**Functionality:** Generator pattern defined in Lua, not Rust.

**Verification:**
1. Screenshot shows same checkerboard as before (but generated by Lua)
2. Edit `generator.lua` to make vertical stripes instead of checkerboard:
   - Change `((x + y) % 2)` to `(x % 2)`
3. Restart app
4. Screenshot shows stripes instead of checkerboard

**Lua file content:**
```lua
local Generator = {}

function Generator:init(ctx)
    self.x, self.y = 0, 0
end

function Generator:step(ctx)
    if self.y >= ctx.height then return true end
    local mat = ((self.x + self.y) % 2 == 0) and 1 or 2
    ctx:set_voxel(self.x, self.y, mat)
    self.x = self.x + 1
    if self.x >= ctx.width then self.x = 0; self.y = self.y + 1 end
    return false
end

function Generator:reset() self.x, self.y = 0, 0 end

return Generator
```

**Done when:** Generator runs from Lua. Editing Lua and restarting changes pattern.

---

#### M3.2: Lua bindings for ctx:set_voxel
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Lua can call `ctx:set_voxel(x, y, material_id)`.

**Verification:** Generator.lua calls set_voxel and screenshot shows correct pattern.

**What to build:**
- `LuaGeneratorContext` userdata with set_voxel/get_voxel methods
- Pass ctx to Generator:step(ctx)

**Done when:** Lua generator successfully writes to buffer, visible in screenshot.

---

#### M3.3: Hot reload generator on file change
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Edit generator.lua, save, pattern updates without restart.

**Verification:**
1. App running, showing checkerboard
2. Edit `generator.lua`: change `((x + y) % 2)` to `(x % 2)` (stripes)
3. Save file
4. Screenshot taken after 1 second shows stripes instead of checkerboard

**Done when:** Edit Lua → screenshot shows new pattern without restart.

---

### M3 Verification Checklist

- [ ] `generator.lua` exists with init/step/reset
- [ ] Screenshot shows pattern generated by Lua
- [ ] `ctx:set_voxel(x, y, mat)` works from Lua
- [ ] Edit generator.lua, save → screenshot updates within 1 second
- [ ] **Lua generator with hot reload works**

---

## M4: MCP Server (External AI)

### Outcome
HTTP server on port 8080. External AI (or curl) can create materials and get PNG output.

### Key Design Decisions

**Simple HTTP, not full MCP protocol.** MCP is complex. Start with plain REST endpoints. Can add MCP framing later.

**Endpoints:**
| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Health check |
| GET | `/mcp/list_materials` | Get all materials as JSON |
| POST | `/mcp/create_material` | Add new material |
| GET | `/mcp/get_output` | Get current render as PNG |

**Why HTTP in background thread?** Bevy runs on main thread. HTTP server runs separately, communicates via `Arc<Mutex<SharedState>>`.

**Why PNG output?** AI can "see" the current state. Screenshot verification without needing to run the app interactively.

### Tasks

---

#### M4.1: Start HTTP server on port 8080
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** App starts HTTP server in background thread.

**Verification:**
```bash
cargo run --example p_map_editor_2d &
curl http://localhost:8080/health
# Returns: {"status": "ok"}
```

**Done when:** curl to /health returns OK.

---

#### M4.2: GET /mcp/list_materials returns material list
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Endpoint returns JSON array of materials.

**Verification:**
```bash
curl http://localhost:8080/mcp/list_materials
# Returns: [{"id":1,"name":"stone","color":[0.5,0.5,0.5]},{"id":2,"name":"dirt","color":[0.6,0.4,0.2]}]
```

**Done when:** curl returns JSON with correct materials.

---

#### M4.3: POST /mcp/create_material adds material
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Create material via API, appears in picker.

**Verification:**
```bash
curl -X POST http://localhost:8080/mcp/create_material \
  -H "Content-Type: application/json" \
  -d '{"id":3,"name":"crystal","color":[0.8,0.2,0.8]}'
# Returns: {"success":true,"id":3}

# Take screenshot, verify "crystal" appears in material picker
```

**Done when:** POST creates material, screenshot shows it in picker.

---

#### M4.4: GET /mcp/get_output returns PNG
**File:** `examples/p_map_editor_2d.rs`

**Functionality:** Endpoint returns current render as PNG.

**Verification:**
```bash
curl http://localhost:8080/mcp/get_output > /tmp/output.png
file /tmp/output.png
# Returns: PNG image data, 32 x 32, ...
```

**Done when:** curl saves valid PNG file.

---

### M4 Verification Checklist

- [ ] App starts HTTP server on port 8080
- [ ] `curl /health` returns OK
- [ ] `curl /mcp/list_materials` returns JSON material list
- [ ] `curl -X POST /mcp/create_material` adds material, visible in screenshot
- [ ] `curl /mcp/get_output` returns valid PNG
- [ ] **External AI can interact with map editor**

---

## Phase 1 Complete

### Final Verification Script

```bash
#!/bin/bash
set -e

echo "=== Phase 1 Verification ==="

# Start app in background
cargo run --example p_map_editor_2d &
APP_PID=$!
sleep 3  # Wait for startup

# M1: Static end-to-end
echo "M1: Checking screenshot exists..."
test -f screenshots/p_map_editor_2d.png && echo "PASS: Screenshot exists"

# M2: Lua materials
echo "M2: Checking materials from Lua..."
test -f assets/map_editor/materials.lua && echo "PASS: materials.lua exists"

# M3: Lua generator  
echo "M3: Checking generator from Lua..."
test -f assets/map_editor/generator.lua && echo "PASS: generator.lua exists"

# M4: MCP server
echo "M4: Checking MCP endpoints..."
curl -s http://localhost:8080/health | grep -q "ok" && echo "PASS: /health"
curl -s http://localhost:8080/mcp/list_materials | grep -q "stone" && echo "PASS: /list_materials"
curl -s http://localhost:8080/mcp/get_output > /tmp/test.png && file /tmp/test.png | grep -q "PNG" && echo "PASS: /get_output"

# Cleanup
kill $APP_PID 2>/dev/null

echo "=== Phase 1 Complete ==="
```

**Phase 1 is complete when this script passes.**

---

## Files Created/Modified

| File | Status | Purpose |
|------|--------|---------|
| `examples/p_map_editor_2d.rs` | New | Main example |
| `assets/map_editor/materials.lua` | New | Material definitions |
| `assets/map_editor/generator.lua` | New | Generator script |

---

## Dependencies (Already in Codebase)

| Crate | Purpose |
|-------|---------|
| `bevy_mod_imgui` | ImGui integration |
| `mlua` | Lua scripting |
| `notify` | File watching |
| `tiny_http` or similar | HTTP server for MCP |

---

## Estimated Time

| Milestone | Time |
|-----------|------|
| M1 | 2-3 hours |
| M2 | 1-2 hours |
| M3 | 1-2 hours |
| M4 | 2 hours |
| **Total** | **6-9 hours** |
