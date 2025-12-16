# Creature 3D Studio

A Bevy-based 3D creature simulation studio with Lua-scripted ImGui UI and Rapier physics.

## Features

- **Bevy 0.17** - Modern ECS game engine
- **Rapier3D Physics** - Rigid body simulation with collisions
- **ImGui UI** - Dockable debug windows
- **Lua Scripting** - Hot-reloadable UI scripts via mlua
- **Live Editing** - Edit Lua scripts, see changes instantly

## Quick Start

```bash
# Build and run
cargo run

# Run tests
cargo test --workspace

# Check for warnings
cargo clippy --workspace
```

## Project Structure

```
creature_3d_studio/
  src/
    main.rs                 # Application entry point
  crates/
    studio_core/            # Shared utilities (stub)
    studio_physics/         # Rapier physics integration
    studio_scripting/       # Lua VM + ImGui facade
  assets/
    scripts/
      ui/
        main.lua            # Main UI script (hot-reloadable)
  docs/
    versions/
      v0.1/
        plan.md             # Development phases
    DEVELOPMENT.md          # Development methodology
```

## Lua API

Scripts in `assets/scripts/ui/` are hot-reloaded on save.

### Scene Namespace
```lua
scene.print(msg)              -- Log to console
scene.spawn_cube(x, y, z)     -- Spawn physics cube
scene.clear()                 -- Remove all dynamic bodies
```

### ImGui Namespace
```lua
imgui.window(title, fn)       -- Create window
imgui.text(str)               -- Display text
imgui.button(label) -> bool   -- Button (returns true if clicked)
imgui.separator()             -- Horizontal line
imgui.same_line()             -- Next widget on same line
```

### Example Script
```lua
scene.print("Script loaded!")

function on_draw()
    imgui.window("My Window", function()
        imgui.text("Hello from Lua!")
        if imgui.button("Spawn") then
            scene.spawn_cube(0, 5, 0)
        end
    end)
end
```

## Architecture

See individual crate READMEs and `docs/` folders for detailed design documentation:

- [studio_physics README](crates/studio_physics/README.md)
- [studio_scripting README](crates/studio_scripting/README.md)
- [Lua API Reference](assets/scripts/README.md)

## Development

This project follows incremental MVP development with verification-driven phases. See [DEVELOPMENT.md](docs/DEVELOPMENT.md) for methodology.

## License

MIT
