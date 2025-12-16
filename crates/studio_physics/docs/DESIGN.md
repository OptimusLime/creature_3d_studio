# studio_physics Design

## Architecture

```
studio_physics/
  src/
    lib.rs          # All physics logic in single file (simple for now)
  docs/
    DESIGN.md       # This file
  Cargo.toml
  README.md
```

## Data Flow

```
CommandQueue (UI requests)
     |
     v
apply_scene_commands (system)
     |
     v
SpawnCubeMessage / ClearBodiesMessage
     |
     v
handle_spawn_cube / handle_clear_bodies (systems)
     |
     v
PhysicsState (Rapier world)
     |
     v
step_physics (system)
     |
     v
sync_transforms (system)
     |
     v
Bevy Transforms (visual)
```

## Key Design Decisions

### Command Queue Pattern

**Decision**: Use a `CommandQueue` resource instead of direct system parameters.

**Rationale**: Lua scripts run during ImGui rendering, which happens in a single system. They can't directly write Bevy messages. The queue decouples "request" from "execute".

**Alternative considered**: Have Lua store commands in Lua state, read them after callback. Rejected because it requires more Lua-side complexity.

### Message vs Event

**Decision**: Use Bevy 0.17's `Message` system instead of `Event`.

**Rationale**: `Event` is deprecated in Bevy 0.17. `Message` is the replacement with similar semantics.

### Single File Structure

**Decision**: Keep all physics code in `lib.rs`.

**Rationale**: Current scope is small (~250 lines). Split when it grows beyond ~500 lines or gains distinct subsystems.

## Pros/Cons

### Pros
- Simple, flat structure easy to navigate
- Command queue cleanly separates UI from physics
- Rapier integration is straightforward
- Message system properly handles event ordering

### Cons
- `PhysicsState` holds many Rapier types - could be unwieldy as it grows
- No abstraction over Rapier - tightly coupled
- Ground plane is hardcoded in setup

## Naming Things Analysis

| Current | Issue | Recommendation |
|---------|-------|----------------|
| `UiAction` | "Ui" is ambiguous (UI element? User input?) | Rename to `SceneCommand` |
| `UiActions` | Same issue, plus plural unclear | Rename to `CommandQueue` |
| `SpawnCubeEvent` | Good - verb + noun + type suffix | Keep |
| `ClearBodiesEvent` | Good | Keep |
| `handle_spawn_cube` | Good - verb + object | Keep |
| `DynamicBody` | Good - adjective + noun | Keep |

## Future Considerations

1. **Entity-Body mapping**: Consider a bidirectional map for O(1) lookups
2. **Physics configuration**: Extract gravity, timestep to config resource
3. **Collider shapes**: Support more than cubes
4. **Physics layers**: For selective collision filtering
