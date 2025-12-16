# Lua Scripts

Hot-reloadable Lua scripts for Creature 3D Studio UI.

## Directory Structure

```
assets/scripts/
  ui/
    main.lua    # Main UI script, loaded at startup
  README.md     # This file
```

## Script Lifecycle

1. **Startup**: `main.lua` is loaded and executed
2. **Each Frame**: `on_draw()` function is called
3. **Hot Reload**: On file save, script is reloaded automatically

## Lua API Reference

### `scene` Namespace - Scene Manipulation

| Function | Description |
|----------|-------------|
| `scene.print(msg)` | Log message to console (prefixed with `[lua]`) |
| `scene.spawn_cube(x, y, z)` | Spawn physics cube at world position |
| `scene.clear()` | Remove all dynamic bodies from scene |

### `imgui` Namespace - UI Rendering

| Function | Returns | Description |
|----------|---------|-------------|
| `imgui.window(title, fn)` | - | Create window, call `fn` inside it |
| `imgui.text(str)` | - | Display text |
| `imgui.button(label)` | `bool` | Button, returns `true` if clicked |
| `imgui.separator()` | - | Draw horizontal line |
| `imgui.same_line()` | - | Place next widget on same line |

## Example Script

```lua
-- Main UI script
scene.print("Script loaded!")

function on_draw()
    imgui.window("My Window", function()
        imgui.text("Hello from Lua!")
        imgui.separator()
        
        if imgui.button("Spawn Cube") then
            local x = math.random() * 6 - 3
            local y = math.random() * 5 + 3
            local z = math.random() * 6 - 3
            scene.spawn_cube(x, y, z)
        end
        
        imgui.same_line()
        
        if imgui.button("Clear") then
            scene.clear()
        end
    end)
end
```

## Error Handling

- **Syntax errors**: Displayed in "Lua Error" ImGui window
- **Runtime errors**: Displayed in "Lua Error" ImGui window
- **Missing `on_draw`**: Script loads but no UI rendered
- **Fix and save**: Error clears, script resumes

## Hot Reload Notes

- State is **lost** on reload (local variables reset)
- Use Bevy resources for persistent state (future feature)
- Reload triggers on any `.lua` file change in `assets/scripts/`
