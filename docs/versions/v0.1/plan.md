# v0.1 - Repository Bootstrap & Lua-ImGui-Physics Pipeline

## Goal
Establish Rust workspace with ImGui UI controlling a Rapier physics scene, then add Lua scripting to drive the UI.

---

## Phase 1: Rust Workspace Skeleton

**Tasks:**
- [ ] Create root `Cargo.toml` as workspace
- [ ] Create `src/main.rs` binary that runs empty Bevy app
- [ ] Create `crates/studio_core/` with `lib.rs` stub
- [ ] Create `crates/studio_physics/` with `lib.rs` stub  
- [ ] Create `crates/studio_scripting/` with `lib.rs` stub
- [ ] Add base dependencies: `bevy = "0.15"`

**Verification:**
```bash
cargo build
# Exit 0, no errors

cargo run
# Opens gray window, closes cleanly with Escape/window close
```

---

## Phase 2: ImGui Hello World (Rust-only)

**Tasks:**
- [ ] Add `bevy_mod_imgui` with `docking` feature to `studio_scripting`
- [ ] Create `ImGuiPlugin` in `studio_scripting` that adds imgui context
- [ ] Enable dockspace over main viewport
- [ ] Show ImGui demo window
- [ ] Create custom "Debug" window with text "ImGui is working"

**Verification:**
```bash
cargo run
# Window contains:
# 1. ImGui Demo window (can dock)
# 2. Window titled "Debug" with text "ImGui is working"
# 3. Windows can be dragged and docked together
```

---

## Phase 3: Rapier Physics Scene (Rust-only)

**Tasks:**
- [ ] Add `rapier3d` dependency to `studio_physics`
- [ ] Create `PhysicsPlugin` with `RigidBodySet`, `ColliderSet` as resources
- [ ] Add ground plane (fixed body with cuboid collider)
- [ ] Add one falling cube (dynamic body) at startup
- [ ] Step physics each frame
- [ ] Sync Bevy transforms from Rapier bodies

**Verification:**
```bash
cargo run
# 3D scene shows:
# 1. A cube falls from height
# 2. Cube lands on ground (stops falling around y=0)
# 3. ImGui windows still functional on top
```

---

## Phase 4: ImGui Controls Physics (Rust-only)

**Tasks:**
- [ ] Add "Scene" ImGui window with spawn/clear buttons
- [ ] "Spawn Cube" button creates dynamic body at random position above ground
- [ ] "Clear" button removes all dynamic bodies (ground stays)
- [ ] Display body count in window
- [ ] Cubes collide with each other and ground

**Verification:**
```bash
cargo run
# 1. "Scene" window visible with "Spawn Cube", "Clear" buttons, body count
# 2. Click "Spawn Cube" -> cube appears, falls, body count increments
# 3. Spawn 5 cubes -> they collide with each other
# 4. Click "Clear" -> all cubes gone, count shows 0, ground remains
```

---

## Phase 5: Lua VM via bevy_mod_scripting

**Tasks:**
- [ ] Add `bevy_mod_scripting` with `lua54` feature
- [ ] Create `ScriptingPlugin` that initializes BMS
- [ ] Create `assets/scripts/ui/main.lua` returning `{ on_draw = function() end }`
- [ ] Load script as static script at startup
- [ ] Define `OnDraw` callback label, fire it each frame
- [ ] Register `tools.print(msg)` via `NamespaceBuilder`

**Verification:**
```bash
cargo run
# Console shows "main.lua loaded" once
# Add tools.print("tick") in on_draw -> prints every frame
# ImGui + physics still work unchanged
```

---

## Phase 6: ImGui Facade for Lua

**Tasks:**
- [ ] Implement thread-local `UI_PTR` for safe `imgui::Ui` access during callback
- [ ] Register via NamespaceBuilder:
  - `imgui.text(str)`
  - `imgui.button(label) -> bool`
  - `imgui.window(title, fn)` 
  - `imgui.checkbox(label, val) -> (val, changed)`
  - `imgui.slider_float(label, val, min, max) -> (val, changed)`
  - `imgui.input_text(label, val) -> (val, changed)`
- [ ] Update `main.lua` to render "Lua UI" window with text and button

**Verification:**
```bash
cargo run
# New window titled "Lua UI" rendered by Lua containing:
# - Text "Lua-driven ImGui"
# - Button that prints to console when clicked
# Rust "Scene" window still works independently
```

---

## Phase 7: Lua Controls Physics via Action Queue

**Tasks:**
- [ ] Create `UiAction` enum: `SpawnCube(Vec3)`, `ClearBodies`
- [ ] Create `UiActions` resource (event queue)
- [ ] Register `tools.spawn_cube(x, y, z)` -> pushes `SpawnCube`
- [ ] Register `tools.clear()` -> pushes `ClearBodies`
- [ ] `apply_ui_actions` system in `studio_physics` consumes queue
- [ ] Update `main.lua`: add spawn/clear buttons that call tools

**Verification:**
```bash
cargo run
# Lua "Lua UI" window has "Spawn" and "Clear" buttons
# Click Lua "Spawn" -> cube appears (same behavior as Rust button)
# Click Lua "Clear" -> cubes cleared
# Both Rust and Lua UI can control the same physics scene

cargo test -p studio_physics
# Tests verify:
# - UiAction variants can be created/matched
# - Spawn action creates body in RigidBodySet
# - Clear action removes dynamic bodies only
```

---

## Phase 8: Hot Reload

**Tasks:**
- [ ] Enable `file_watcher` feature on Bevy AssetPlugin
- [ ] On script asset change: reload Lua VM
- [ ] Show Lua errors in persistent "Lua Error" ImGui window (Rust-side)

**Verification:**
```bash
cargo run
# Test 1: Edit main.lua, change button label
# -> Label updates within 1s without restart

# Test 2: Add syntax error to main.lua
# -> "Lua Error" window appears with error text

# Test 3: Fix syntax error
# -> Error window disappears, Lua UI restored
```

---

## Completion Criteria

All phases verified when:
1. `cargo build --workspace` exits 0
2. `cargo test --workspace` exits 0  
3. `cargo clippy --workspace` no warnings
4. Rust UI and Lua UI both control physics scene
5. Hot reload works as Phase 8 describes
6. PR merged to main
