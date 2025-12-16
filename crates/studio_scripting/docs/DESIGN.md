# studio_scripting Design

## Architecture

```
studio_scripting/
  src/
    lib.rs          # Lua VM, ImGui facade, hot reload, UI rendering
  docs/
    DESIGN.md       # This file
  Cargo.toml
  README.md
```

## Data Flow

```
Startup:
  setup_lua_vm -> LuaVm (NonSend resource)
  setup_file_watcher -> ScriptWatcher (NonSend resource)

Each Frame:
  check_hot_reload -> (optionally) reload_lua_vm
       |
       v
  render_ui:
    1. Set CURRENT_UI, CURRENT_COMMANDS thread-locals
    2. Call Lua on_draw()
    3. Lua calls imgui.* / scene.* functions
    4. Clear thread-locals
    5. Display errors if any
```

## Key Design Decisions

### Thread-Local Pointer Pattern

**Decision**: Use `thread_local!` cells to hold raw pointers during Lua callbacks.

**Rationale**: 
- `imgui::Ui` has a lifetime tied to the frame - can't store reference
- Lua functions are registered at startup, but need frame-specific `Ui`
- Thread-local is safe because Bevy runs on main thread

**Safety invariant**: Pointers are only non-null during `on_draw()` callback execution.

```rust
thread_local! {
    static CURRENT_UI: Cell<*const Ui> = const { Cell::new(std::ptr::null()) };
}

// Set before Lua call
CURRENT_UI.with(|c| c.set(ui as *const Ui));
// Lua runs, calls with_ui() which reads the pointer
// Clear after Lua call
CURRENT_UI.with(|c| c.set(std::ptr::null()));
```

### NonSend Resources

**Decision**: `LuaVm` and `ScriptWatcher` are `NonSend` resources.

**Rationale**: 
- Lua's `Lua` type is `!Send` - cannot be moved between threads
- `notify::Watcher` contains thread handles
- Bevy's `NonSend`/`NonSendMut` ensures main-thread access

### Hot Reload via File Watcher

**Decision**: Use `notify` crate with channel-based notification.

**Rationale**:
- Cross-platform file watching
- Non-blocking check via `try_recv()`
- Recreate entire Lua VM on reload (simpler than incremental update)

**Trade-off**: Loses Lua state on reload. Acceptable for UI scripts.

## Pros/Cons

### Pros
- Hot reload enables rapid iteration
- Thread-local pattern is minimal overhead
- Clean separation: Rust owns UI frame, Lua defines content
- Error display in ImGui keeps user informed

### Cons
- Raw pointers require careful safety analysis
- Single Lua script - no multi-script support yet
- Window positions hardcoded in Rust

## Naming Things Analysis

| Current | Issue | Recommendation |
|---------|-------|----------------|
| `UI_PTR` | Screaming case, "PTR" is type encoding | Rename to `CURRENT_UI` |
| `ACTIONS_PTR` | Same issue | Rename to `CURRENT_COMMANDS` |
| `with_ui` | Generic "with" | Acceptable - common Rust pattern |
| `imgui_ui` | Redundant - "imgui" + "ui" | Rename to `render_ui` |
| `setup_lua` | Implementation detail ("lua") | Rename to `init_scripting` |
| `tools` (Lua) | Vague namespace | Rename to `scene` - domain term |
| `LuaVm` | Implementation detail | Consider `ScriptRuntime` |

## Future Considerations

1. **Multiple scripts**: Load all `.lua` files from scripts directory
2. **Script isolation**: Separate Lua states per script for safety
3. **Lua state persistence**: Save/restore state across hot reloads
4. **More ImGui widgets**: sliders, checkboxes, trees, color pickers
5. **Async script loading**: Don't block startup on script load
