# v0.1 - Repository Bootstrap & Lua-ImGui-Physics Pipeline

## Goal
Establish Rust workspace with Lua scripting (via `bevy_mod_scripting`), ImGui UI, and Rapier physics - ending with a Lua-driven UI that can spawn/remove 3D physics bodies.

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
# 2. Cube lands on invisible ground (stops falling around y=0)
# 3. ImGui windows still functional on top
```

---

## Phase 4: Lua VM via bevy_mod_scripting

**Tasks:**
- [ ] Add `bevy_mod_scripting` with `lua54` feature
- [ ] Create `ScriptingPlugin` that initializes BMS
- [ ] Create `assets/scripts/ui/main.lua` returning `{ draw = function() end }`
- [ ] Load script as static script at startup
- [ ] Define `OnDraw` callback label, fire it each frame
- [ ] Register `tools.print(msg)` via `NamespaceBuilder`

**Verification:**
```bash
cargo run
# Console output (once): "main.lua loaded"
# Add tools.print("tick") in on_draw -> prints "tick" every frame
# No Lua errors in console
```

---

## Phase 5: ImGui Facade for Lua

**Tasks:**
- [ ] Implement thread-local `UI_PTR` for safe `imgui::Ui` access during callback
- [ ] Register via NamespaceBuilder:
  - `imgui.text(str)`
  - `imgui.button(label) -> bool`
  - `imgui.window(title, fn)` 
  - `imgui.checkbox(label, val) -> (val, changed)`
  - `imgui.slider_float(label, val, min, max) -> (val, changed)`
  - `imgui.input_text(label, val) -> (val, changed)`
- [ ] Update `main.lua` to render window with controls

**Verification:**
```bash
cargo run
# Lua-rendered window titled "Creature Builder" containing:
# - Text "Lua-driven ImGui is live."
# - Slider "Mass" (draggable, value shown)
# - Button "Send" (click prints to console)
# - Input text field
```

---

## Phase 6: Hot Reload

**Tasks:**
- [ ] Enable `file_watcher` feature on Bevy AssetPlugin
- [ ] Implement custom `LuaScript` asset loader
- [ ] On `AssetEvent::Modified`: destroy and recreate Lua VM
- [ ] Show Lua errors in persistent "Lua Error" ImGui window (Rust-side)

**Verification:**
```bash
cargo run
# Test 1: Edit main.lua, change window title
# -> Title updates within 1s without restart

# Test 2: Add syntax error to main.lua
# -> "Lua Error" window appears with error text

# Test 3: Fix syntax error
# -> Error window disappears, UI restored
```

---

## Phase 7: Action Queue & Physics Control

**Tasks:**
- [ ] Create `UiAction` enum: `Print(String)`, `SpawnCube(Vec3)`, `ClearBodies`
- [ ] Create `UiActions` resource (event queue)
- [ ] `tools.print` pushes `UiAction::Print`
- [ ] Add `tools.spawn_cube(x, y, z)` -> pushes `SpawnCube`
- [ ] Add `tools.clear()` -> pushes `ClearBodies`
- [ ] `apply_ui_actions` system in `studio_physics` consumes queue
- [ ] Update `main.lua` with spawn/clear buttons

**Verification:**
```bash
cargo run
# 1. Click "Spawn Cube" button -> new cube appears at specified position, falls
# 2. Click "Clear" button -> all dynamic bodies removed (ground stays)
# 3. Spawn multiple cubes -> they collide with each other

cargo test -p studio_physics
# Tests pass:
# - UiAction variants can be created/matched
# - Spawn action creates body in RigidBodySet
# - Clear action removes dynamic bodies only
```

---

## Completion Criteria

All phases verified when:
1. `cargo build --workspace` exits 0
2. `cargo test --workspace` exits 0  
3. `cargo clippy --workspace` no warnings
4. Hot reload works as Phase 6 describes
5. Lua UI can spawn/clear physics cubes as Phase 7 describes
6. PR merged to main
