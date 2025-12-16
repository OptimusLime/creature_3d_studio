# studio_scripting

Lua scripting and ImGui integration for Creature 3D Studio.

## Purpose

Provides the bridge between Lua scripts and the Bevy/ImGui/Physics systems. Enables hot-reloadable UI scripting with full access to scene commands.

## Key Types

| Type | Purpose |
|------|---------|
| `ScriptingPlugin` | Bevy plugin that initializes Lua VM, ImGui, and hot reload |
| `LuaVm` | Non-Send resource holding Lua state and callbacks |
| `ScriptWatcher` | File watcher for hot reload support |

## Lua API

Scripts have access to two namespaces:

### `scene` - Scene manipulation
- `scene.print(msg)` - Log message to console
- `scene.spawn_cube(x, y, z)` - Spawn physics cube at position
- `scene.clear()` - Remove all dynamic bodies

### `imgui` - UI rendering
- `imgui.window(title, fn)` - Create window, call fn inside
- `imgui.text(str)` - Display text
- `imgui.button(label) -> bool` - Button, returns true if clicked
- `imgui.separator()` - Horizontal line
- `imgui.same_line()` - Next widget on same line

## Usage

```rust
use studio_scripting::ScriptingPlugin;

app.add_plugins(ScriptingPlugin);
```

Scripts are loaded from `assets/scripts/ui/main.lua` and hot-reloaded on save.

## Architecture

```
studio_scripting/
  src/
    lib.rs    # Plugin, Lua VM, ImGui facade, hot reload
```

## Dependencies

- `bevy` - ECS framework
- `bevy_mod_imgui` - ImGui integration
- `mlua` - Lua 5.4 bindings
- `notify` - File system watching
- `studio_physics` - Scene command queue
