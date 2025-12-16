# studio_physics

Physics simulation for Creature 3D Studio using Rapier3D.

## Purpose

Manages the physics world including rigid bodies, colliders, and physics stepping. Provides a command queue for UI-driven physics operations.

## Key Types

| Type | Purpose |
|------|---------|
| `PhysicsPlugin` | Bevy plugin that initializes physics systems |
| `PhysicsState` | Resource holding Rapier physics world state |
| `SceneCommand` | Enum of commands that can modify the scene |
| `CommandQueue` | Queue of pending scene commands |
| `RigidBodyLink` | Component linking Bevy entity to Rapier body |
| `DynamicBody` | Marker component for clearable bodies |

## Usage

```rust
use studio_physics::{PhysicsPlugin, CommandQueue};

// Add plugin
app.add_plugins(PhysicsPlugin);

// Queue commands from UI
fn my_system(mut commands: ResMut<CommandQueue>) {
    commands.spawn_cube(Vec3::new(0.0, 5.0, 0.0));
    commands.clear();
}
```

## Architecture

```
studio_physics/
  src/
    lib.rs    # Plugin, resources, systems, command queue
```

## Dependencies

- `bevy` - ECS framework
- `rapier3d` - Physics engine
