# Spell System Design

## Summary

A physics-based spell system inspired by PyTorch's module composition patterns. Spells are programmable sequences of physics behaviors that consume energy/mana over time, with spell objects represented as emissive voxel volumes that shrink as energy depletes. The system provides a Lua API backed by Rust physics implementations, with a package ecosystem for sharing and composing spell modules.

## Context & Motivation

### Setting
- Bevy-based voxel world with GPU physics and GPU occupancy for chunk meshing
- Two moons, 80s dark fantasy aesthetic
- Existing systems: physics projectiles, dropping objects, emissive volumes, MarkovJunior cellular automata

### Design Philosophy
Spells and magic are **energy**. To do things, you must provide energy. This creates natural constraints:
- Powerful spells require more energy
- Long-range spells consume energy over distance/time
- Complex spell behaviors have compounding costs
- Spell objects are physical: they have volume, they glow, they interact with the world

### Inspiration: PyTorch Module System
The spell system borrows PyTorch's key abstractions:

| PyTorch Concept | Spell System Analog |
|-----------------|---------------------|
| `nn.Module` | Spell module (behavior unit) |
| `nn.Sequential` | Sequential spell chain |
| `nn.Parallel` | Parallel spell branches |
| `forward(input)` | `tick(state) -> state` |
| Tensor flow | Energy/state flow |
| Autograd tape | Cost accounting tape |
| `backward()` | Cost calculation/reporting |

---

## Core Concepts

### 1. Spell Objects

A spell object is a physical entity in the voxel world:

```
SpellObject {
    // Physical properties
    position: Vec3,
    velocity: Vec3,
    rotation: Quat,
    angular_velocity: Vec3,
    
    // Energy properties  
    energy: f32,                    // Current mana/energy
    max_energy: f32,                // Initial energy budget
    energy_density: f32,            // Energy per voxel (max ~100)
    
    // Visual representation
    voxel_volume: VoxelVolume,      // Emissive voxels forming the spell
    color: Color,                   // Emission color
    glow_intensity: f32,            // Bloom factor
    
    // Behavior
    brain: Box<dyn SpellModule>,    // The spell's "neural network"
    
    // Lifecycle
    state: SpellState,              // Active, Fizzling, Exploding, Dead
}
```

**Volume-Energy Relationship:**
- Each voxel has a maximum energy density (~100 units)
- A spell with 1000 energy needs at least 10 voxels
- As energy depletes, voxels are consumed from the outer surface
- High-power spells are physically larger (can't hide a nuke in a marble)

### 2. Spell Modules

Modules are the building blocks of spell behavior. Each module has:

```rust
trait SpellModule {
    /// Execute one tick of this module
    /// Returns the new state and energy consumed
    fn tick(&mut self, input: SpellState, dt: f32) -> (SpellState, f32);
    
    /// Base cost per second to keep this module "loaded"
    fn base_cost(&self) -> f32;
    
    /// Reset module to initial state
    fn reset(&mut self);
    
    /// Human-readable name for debugging
    fn name(&self) -> &str;
}
```

**SpellState** flows through modules like tensors through a neural network:

```rust
struct SpellState {
    // Transform
    position: Vec3,
    velocity: Vec3,
    rotation: Quat,
    angular_velocity: Vec3,
    
    // Resources
    energy: f32,
    
    // Sensors
    ground_distance: Option<f32>,
    target_distance: Option<f32>,
    time_alive: f32,
    
    // Flags
    triggered: bool,
    should_terminate: bool,
}
```

### 3. Module Composition

#### Sequential
Execute modules in order, each transforms the state:

```lua
local fireball = Sequential {
    Launch { direction = "forward", speed = 20 },
    ApplyGravity { strength = 0.5 },  -- Half gravity (partially floaty)
    OnGroundHit { 
        transform_to = Explosion { radius = 3, damage = 50 }
    }
}
```

Execution flow:
```
state_0 -> Launch.tick() -> state_1 -> ApplyGravity.tick() -> state_2 -> OnGroundHit.tick() -> state_3
```

#### Parallel
Execute multiple module chains simultaneously, merge results:

```lua
local split_bolt = Parallel {
    -- Branch A: goes left
    Sequential {
        Deflect { angle = -30 },
        ApplyGravity {},
    },
    -- Branch B: goes right  
    Sequential {
        Deflect { angle = 30 },
        ApplyGravity {},
    },
    -- Merge strategy
    merge = "split"  -- Creates two spell objects
}
```

#### Conditional (Sensors/Triggers)
Modules that watch for conditions and transform behavior:

```lua
OnGroundHit { transform_to = explosion }
OnTimeout { seconds = 5, transform_to = fizzle }
OnTargetNear { radius = 2, transform_to = detonate }
WhenEnergyLow { threshold = 10, transform_to = fizzle }
```

---

## Energy & Cost System

### Cost Accounting (The "Tape")

Like PyTorch's autograd tape, we record all operations and their costs:

```rust
struct CostTape {
    entries: Vec<CostEntry>,
    total_cost: f32,
}

struct CostEntry {
    module_name: String,
    operation: String,
    cost: f32,
    timestamp: f32,
}
```

The tape allows:
1. **Reporting**: Show player exactly why their spell fizzled
2. **Optimization**: Identify expensive modules to optimize
3. **Debugging**: Trace spell behavior step by step
4. **Balancing**: Game designers can tune costs based on actual usage patterns

### Cost Categories

| Category | Examples | Cost Model |
|----------|----------|------------|
| **Base Load** | Having modules loaded | Per-second while spell active |
| **Physics** | Gravity cancellation, propulsion | Per-second while active |
| **Sensing** | Ground detection, target tracking | Per-second while active |
| **Transformation** | Explosion, split, morph | One-time on trigger |
| **Damage** | Fire damage, knockback | Per unit of effect |

### Example: Fireball Cost Breakdown

```
Module: Launch
  - Initial velocity impulse: 5 energy (one-time)

Module: AntiGravity  
  - Gravity cancellation: 2 energy/second
  - Duration: 3 seconds flight time
  - Total: 6 energy

Module: OnGroundHit (Sensor)
  - Ground detection: 0.5 energy/second
  - Duration: 3 seconds
  - Total: 1.5 energy

Module: Explosion (Triggered)
  - Radius 3 expansion: 20 energy (one-time)
  - Fire damage 50: 25 energy (one-time)
  - Total: 45 energy

TOTAL SPELL COST: 57.5 energy minimum
```

### Energy Density Constraint

```
max_energy_density = 100 energy/voxel

For a 57.5 energy spell:
  minimum_voxels = ceil(57.5 / 100) = 1 voxel (minimum)
  
For a 500 energy spell:
  minimum_voxels = ceil(500 / 100) = 5 voxels
```

The spell physically manifests with at least this many voxels. As energy depletes, voxels are consumed.

---

## Module Types

### Physics Modules

#### Gravity Modifiers
```lua
ApplyGravity { strength = 1.0 }      -- Normal gravity
ApplyGravity { strength = 0.0 }      -- Zero-G
ApplyGravity { strength = -0.5 }     -- Floats upward
AntiGravity {}                       -- Exactly cancels world gravity
```

**Cost**: `|strength - 1.0| * 2.0` energy/second (canceling gravity costs energy)

#### Propulsion
```lua
Thrust { direction = "forward", force = 10 }
Thrust { direction = "toward_target", force = 5 }
HomingThrust { target_type = "nearest_enemy", force = 3 }
```

**Cost**: `force * 0.5` energy/second

#### Launch
```lua
Launch { direction = "forward", speed = 20 }
Launch { direction = Vec3(0, 1, 0), speed = 15 }  -- Straight up
Launch { spread = 10 }  -- Random cone spread
```

**Cost**: `speed * 0.25` energy (one-time)

### Sensor Modules

#### Ground Detection
```lua
OnGroundHit { transform_to = explosion }
OnGroundHit { bounce = true, energy_loss = 0.3 }  -- Bouncy spell
```

**Cost**: `0.5` energy/second

#### Proximity
```lua
OnTargetNear { radius = 2, target_type = "enemy", transform_to = detonate }
OnObjectNear { radius = 1, transform_to = stick }
```

**Cost**: `radius * 0.3` energy/second

#### Time
```lua
OnTimeout { seconds = 5, transform_to = fizzle }
AfterDistance { meters = 50, transform_to = dissipate }
```

**Cost**: `0.1` energy/second (just bookkeeping)

### Transformation Modules

#### Explosion
```lua
Explosion {
    radius = 3,
    damage = 50,
    knockback = 10,
    fire_duration = 2,
}
```

**Cost**: `radius^2 * 2 + damage * 0.5 + knockback * 0.3` energy (one-time)

#### Split
```lua
Split {
    count = 3,
    angle_spread = 45,
    energy_distribution = "equal",  -- Each gets 33%
}
```

**Cost**: `count * 10` energy (one-time, creating new objects is expensive)

#### Morph
```lua
Morph {
    to = ice_spike,
    preserve_velocity = true,
}
```

**Cost**: `new_spell.base_cost * 0.5` energy (one-time)

### Effect Modules

#### Emissive Trail
```lua
Trail {
    length = 10,
    color = Color.ORANGE,
    fade_time = 0.5,
}
```

**Cost**: `length * 0.1` energy/second

#### Sound
```lua
PlaySound { sound = "fireball_whoosh", volume = 0.8 }
```

**Cost**: `0` energy (sounds are free, just feedback)

---

## Lua API Design

### Core API

```lua
-- spell.lua: Spell module definitions

local spell = require("spell")

-- Define a simple fireball
local fireball = spell.Sequential {
    spell.Launch { speed = 20 },
    spell.AntiGravity {},
    spell.OnGroundHit {
        transform_to = spell.Explosion { radius = 3, damage = 50 }
    }
}

-- Register for use
spell.register("fireball", fireball)
```

### Casting API

```lua
-- In-game casting
local function cast_fireball(caster)
    local direction = caster:get_look_direction()
    local position = caster:get_position() + Vec3(0, 1, 0)  -- At eye level
    
    return spell.cast("fireball", {
        position = position,
        direction = direction,
        energy = 100,  -- Allocate 100 energy
        color = Color.ORANGE,
    })
end
```

### Custom Module Definition

```lua
-- custom_module.lua: Example custom module

local Module = spell.Module

local MyHomingModule = Module:extend("MyHomingModule")

function MyHomingModule:init(params)
    self.turn_rate = params.turn_rate or 5.0
    self.target_type = params.target_type or "nearest_enemy"
end

function MyHomingModule:tick(state, dt)
    local target = self:find_target(state.position, self.target_type)
    if target then
        local to_target = (target.position - state.position):normalize()
        local current_dir = state.velocity:normalize()
        local new_dir = current_dir:lerp(to_target, self.turn_rate * dt)
        state.velocity = new_dir * state.velocity:length()
    end
    
    local cost = self.turn_rate * 0.2 * dt  -- Cost scales with turn rate
    return state, cost
end

function MyHomingModule:base_cost()
    return 1.0  -- 1 energy/second just to have homing loaded
end

return MyHomingModule
```

### World Interaction API

```lua
-- Modules can interact with the voxel world

local function on_explosion(center, radius)
    -- Query voxels in radius
    local voxels = world.query_sphere(center, radius)
    
    -- Destroy voxels
    for _, voxel in ipairs(voxels) do
        if voxel.hardness < 5 then
            world.remove_voxel(voxel.position)
            -- Spawn fragment with velocity away from center
            physics.spawn_fragment(voxel.position, voxel.color, {
                velocity = (voxel.position - center):normalize() * 10
            })
        end
    end
    
    -- Spawn emissive flash
    effects.spawn_flash(center, {
        color = Color.ORANGE,
        intensity = 5.0,
        duration = 0.2,
    })
end
```

---

## Package System

### Package Structure

```
spells/
  my_package/
    manifest.lua          -- Package metadata
    modules/
      homing.lua          -- Custom module
      sticky.lua          -- Custom module
    spells/
      homing_fireball.lua -- Complete spell using modules
      sticky_bomb.lua
    assets/
      sounds/
        explosion.ogg
```

### manifest.lua

```lua
return {
    name = "my_awesome_spells",
    version = "1.0.0",
    author = "wizard123",
    
    dependencies = {
        { name = "core_physics", version = ">=1.0" },
        { name = "particle_effects", version = ">=0.5" },
    },
    
    modules = {
        "modules/homing",
        "modules/sticky",
    },
    
    spells = {
        "spells/homing_fireball",
        "spells/sticky_bomb",
    },
    
    -- Cost modifiers (for balancing)
    cost_multipliers = {
        homing = 1.2,  -- Homing is 20% more expensive than base
    },
}
```

### Package Repository

We host our own Lua package repository (similar to LuaRocks but simpler):

```
Repository API:
  GET  /packages                     -- List all packages
  GET  /packages/{name}              -- Package info
  GET  /packages/{name}/{version}    -- Download specific version
  POST /packages                     -- Upload new package (authenticated)
  
Local cache:
  ~/.creature_studio/
    packages/
      core_physics/1.0.0/
      particle_effects/0.5.2/
      my_awesome_spells/1.0.0/
```

### Package CLI

```bash
# In-game console or external tool
spell install my_awesome_spells
spell install particle_effects@0.5
spell update
spell list
spell search "homing"
```

---

## Rust Implementation Architecture

### Crate Structure

```
crates/
  studio_spell/
    src/
      lib.rs              -- Public API
      module.rs           -- SpellModule trait
      modules/
        mod.rs
        gravity.rs        -- Gravity modules
        launch.rs         -- Launch modules
        sensors.rs        -- Sensor modules
        explosion.rs      -- Explosion transformation
        sequential.rs     -- Sequential composition
        parallel.rs       -- Parallel composition
      spell_object.rs     -- SpellObject ECS component
      energy.rs           -- Energy/cost system
      tape.rs             -- Cost accounting tape
      lua_api.rs          -- MLua bindings
      systems.rs          -- Bevy ECS systems
      plugin.rs           -- Bevy plugin
    docs/
      DESIGN.md
    Cargo.toml
```

### Core Trait (Rust)

```rust
// crates/studio_spell/src/module.rs

use bevy::prelude::*;

/// The state flowing through spell modules
#[derive(Clone, Debug)]
pub struct SpellState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
    pub angular_velocity: Vec3,
    pub energy: f32,
    pub time_alive: f32,
    pub ground_distance: Option<f32>,
    pub triggered: bool,
    pub should_terminate: bool,
}

/// Result of a module tick
pub struct TickResult {
    pub state: SpellState,
    pub energy_consumed: f32,
    pub transformation: Option<Box<dyn SpellModule>>,
}

/// Core trait for spell modules
pub trait SpellModule: Send + Sync {
    /// Execute one tick
    fn tick(&mut self, state: SpellState, dt: f32, world: &SpellWorld) -> TickResult;
    
    /// Base energy cost per second
    fn base_cost(&self) -> f32;
    
    /// Reset to initial state
    fn reset(&mut self);
    
    /// Module name for debugging/tape
    fn name(&self) -> &str;
    
    /// Clone into boxed trait object
    fn box_clone(&self) -> Box<dyn SpellModule>;
}

/// World context passed to modules
pub struct SpellWorld<'w> {
    pub terrain: &'w TerrainOccupancy,
    pub targets: &'w Query<'w, 'w, (Entity, &'w Transform), With<Target>>,
    // ... other world queries
}
```

### Bevy Integration

```rust
// crates/studio_spell/src/spell_object.rs

use bevy::prelude::*;

/// ECS component for a spell object
#[derive(Component)]
pub struct SpellObject {
    pub brain: Box<dyn SpellModule>,
    pub state: SpellState,
    pub voxel_entity: Entity,
    pub color: Color,
    pub tape: CostTape,
}

/// Resource for spell definitions
#[derive(Resource, Default)]
pub struct SpellRegistry {
    pub spells: HashMap<String, SpellDefinition>,
}

pub struct SpellDefinition {
    pub name: String,
    pub create_brain: fn() -> Box<dyn SpellModule>,
    pub default_energy: f32,
    pub default_color: Color,
}
```

### Systems

```rust
// crates/studio_spell/src/systems.rs

/// Main spell simulation system
pub fn spell_tick_system(
    mut spells: Query<(&mut SpellObject, &mut Transform)>,
    terrain: Res<TerrainOccupancy>,
    targets: Query<(Entity, &Transform), With<Target>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    
    for (mut spell, mut transform) in spells.iter_mut() {
        // Update sensor data
        spell.state.ground_distance = compute_ground_distance(
            spell.state.position, 
            &terrain
        );
        spell.state.time_alive += dt;
        
        // Build world context
        let world = SpellWorld {
            terrain: &terrain,
            targets: &targets,
        };
        
        // Execute brain
        let result = spell.brain.tick(spell.state.clone(), dt, &world);
        
        // Record cost
        spell.tape.record(spell.brain.name(), result.energy_consumed);
        
        // Apply result
        spell.state = result.state;
        spell.state.energy -= result.energy_consumed;
        
        // Update transform
        transform.translation = spell.state.position;
        transform.rotation = spell.state.rotation;
        
        // Handle transformation
        if let Some(new_brain) = result.transformation {
            spell.brain = new_brain;
            spell.brain.reset();
        }
        
        // Check death conditions
        if spell.state.energy <= 0.0 || spell.state.should_terminate {
            spell.state.should_terminate = true;
        }
    }
}

/// Resize spell voxel volume based on remaining energy
pub fn spell_volume_system(
    mut spells: Query<(&SpellObject, &mut VoxelVolume)>,
) {
    for (spell, mut volume) in spells.iter_mut() {
        let target_voxels = (spell.state.energy / MAX_ENERGY_DENSITY).ceil() as usize;
        volume.resize_to(target_voxels.max(1));
    }
}

/// Remove dead spells
pub fn spell_cleanup_system(
    mut commands: Commands,
    spells: Query<(Entity, &SpellObject)>,
) {
    for (entity, spell) in spells.iter() {
        if spell.state.should_terminate {
            commands.entity(entity).despawn_recursive();
        }
    }
}
```

### Lua Bindings

```rust
// crates/studio_spell/src/lua_api.rs

use mlua::prelude::*;

pub fn register_spell_api(lua: &Lua) -> LuaResult<()> {
    let spell = lua.create_table()?;
    
    // Module constructors
    spell.set("Sequential", lua.create_function(create_sequential)?)?;
    spell.set("Parallel", lua.create_function(create_parallel)?)?;
    spell.set("Launch", lua.create_function(create_launch)?)?;
    spell.set("AntiGravity", lua.create_function(create_anti_gravity)?)?;
    spell.set("ApplyGravity", lua.create_function(create_apply_gravity)?)?;
    spell.set("OnGroundHit", lua.create_function(create_on_ground_hit)?)?;
    spell.set("Explosion", lua.create_function(create_explosion)?)?;
    // ... etc
    
    // Registration
    spell.set("register", lua.create_function(register_spell)?)?;
    
    // Casting
    spell.set("cast", lua.create_function(cast_spell)?)?;
    
    lua.globals().set("spell", spell)?;
    Ok(())
}

fn create_sequential<'lua>(
    lua: &'lua Lua,
    modules: LuaTable<'lua>,
) -> LuaResult<LuaAnyUserData<'lua>> {
    let mut children: Vec<Box<dyn SpellModule>> = Vec::new();
    
    for pair in modules.pairs::<i32, LuaAnyUserData>() {
        let (_, ud) = pair?;
        let module: &LuaSpellModule = ud.borrow()?;
        children.push(module.inner.box_clone());
    }
    
    let sequential = Sequential::new(children);
    lua.create_userdata(LuaSpellModule { inner: Box::new(sequential) })
}
```

---

## Rendering

### Emissive Voxel Volumes

Spell objects render as collections of emissive voxels:

```rust
#[derive(Component)]
pub struct SpellVoxelVolume {
    /// Voxel positions relative to spell center
    pub voxels: Vec<IVec3>,
    /// Emission color
    pub color: Color,
    /// Emission intensity (for bloom)
    pub intensity: f32,
}
```

The existing deferred renderer handles emissive voxels via the `emission` vertex attribute. Spell voxels are added to the chunk mesh with high emission values.

### Volume Shapes

Spells can have different volume shapes:

```rust
enum VolumeShape {
    Sphere { radius: f32 },
    Cube { size: f32 },
    Custom { voxels: Vec<IVec3> },
}

impl VolumeShape {
    fn generate_voxels(&self, voxel_count: usize) -> Vec<IVec3> {
        // Generate voxel positions fitting the shape
        // with approximately voxel_count voxels
    }
}
```

### Trail Rendering

Spell trails use a separate system that spawns fading emissive particles:

```rust
pub fn spell_trail_system(
    spells: Query<(&SpellObject, &Transform), With<TrailEffect>>,
    mut trails: ResMut<TrailParticles>,
) {
    for (spell, transform) in spells.iter() {
        trails.spawn(TrailParticle {
            position: transform.translation,
            color: spell.color,
            intensity: spell.glow_intensity * 0.5,
            lifetime: 0.3,
        });
    }
}
```

---

## Integration with Existing Systems

### Physics Integration

Spell objects can interact with the existing physics system:

1. **As Projectiles**: Use `VoxelFragment` physics for realistic trajectory
2. **With Terrain**: Use `TerrainOccupancy` for ground detection
3. **With Characters**: Use collision detection for damage/effects

### MarkovJunior Integration

Spells can trigger MarkovJunior patterns for effects:

```lua
local frost_spread = spell.MarkovEffect {
    pattern = "frost_grow",  -- .xml pattern name
    steps = 10,
    on_complete = spell.Fizzle {},
}
```

This allows spells to create spreading ice, growing vines, corrupting terrain, etc.

### Emissive Light Generation

High-energy spells automatically generate point lights via the existing `extract_emissive_lights` system.

---

## Implementation Phases

### Phase 0: Test Harness

**Outcome:** A dedicated test example that spawns spell objects and verifies basic behavior.

**Verification:**
```bash
cargo run --example p40_spell_test
# Spell spawns, moves forward, hits ground, prints "explosion triggered"
```

**Tasks:**
1. Create `crates/studio_spell/` crate structure
2. Create `examples/p40_spell_test.rs`
3. Minimal `SpellObject` component
4. Minimal `spell_tick_system`

### Phase 1: Core Module Trait

**Outcome:** `SpellModule` trait defined with `tick()`, `base_cost()`, `reset()`.

**Verification:**
```bash
cargo test -p studio_spell module_trait
# Tests pass for trait implementation
```

**Tasks:**
1. Define `SpellState` struct
2. Define `SpellModule` trait
3. Implement `Sequential` module
4. Unit tests for sequential execution

### Phase 2: Physics Modules

**Outcome:** Basic physics modules: `Launch`, `ApplyGravity`, `AntiGravity`, `Thrust`.

**Verification:**
```bash
cargo run --example p40_spell_test
# Spell launches forward, arcs due to gravity, reaches ground
```

**Tasks:**
1. Implement `Launch` module
2. Implement `ApplyGravity` module
3. Implement `AntiGravity` module
4. Implement `Thrust` module
5. Visual test with trajectory

### Phase 3: Sensor Modules

**Outcome:** Sensor modules that detect conditions: `OnGroundHit`, `OnTimeout`, `OnTargetNear`.

**Verification:**
```bash
cargo run --example p40_spell_test
# Spell detects ground hit, triggers transformation
```

**Tasks:**
1. Implement `OnGroundHit` sensor
2. Implement `OnTimeout` sensor
3. Implement `OnTargetNear` sensor
4. Integrate with terrain query

### Phase 4: Transformation Modules

**Outcome:** Modules that transform spells: `Explosion`, `Split`, `Fizzle`.

**Verification:**
```bash
cargo run --example p40_spell_test
# Spell hits ground, transforms to explosion, explosion completes
```

**Tasks:**
1. Implement `Explosion` transformation
2. Implement `Split` transformation
3. Implement `Fizzle` termination
4. Handle brain swapping

### Phase 5: Energy System

**Outcome:** Energy consumption, voxel volume scaling, spell death on energy depletion.

**Verification:**
```bash
cargo run --example p41_spell_energy_test
# Spell shrinks over time, fizzles when energy depleted
```

**Tasks:**
1. Implement energy consumption in tick
2. Implement `CostTape` recording
3. Implement volume resize system
4. Implement energy depletion fizzle

### Phase 6: Lua API

**Outcome:** Lua bindings for defining and casting spells.

**Verification:**
```lua
-- Test script
local fireball = spell.Sequential {
    spell.Launch { speed = 20 },
    spell.OnGroundHit { transform_to = spell.Explosion { radius = 3 } }
}
spell.register("fireball", fireball)
spell.cast("fireball", { position = Vec3(0, 5, 0) })
```

**Tasks:**
1. Create `lua_api.rs` with MLua bindings
2. Implement module constructors
3. Implement `spell.register()`
4. Implement `spell.cast()`
5. Hot-reload support

### Phase 7: Rendering Integration

**Outcome:** Spells render as emissive voxel volumes with trails.

**Verification:**
```bash
cargo run --example p42_spell_visual_test
# Spell visible as glowing orb, leaves trail
```

**Tasks:**
1. Implement `SpellVoxelVolume` component
2. Integrate with deferred renderer
3. Implement trail particle system
4. Volume shape generation

### Phase 8: Package System

**Outcome:** Package loading from filesystem, manifest parsing, dependency resolution.

**Verification:**
```bash
# Create test package
mkdir -p assets/spells/test_package
# Create manifest.lua and modules
cargo run --example p43_package_test
# Package loads, spell available
```

**Tasks:**
1. Design manifest format
2. Implement package loader
3. Implement dependency resolver
4. Implement `spell install` command

### Phase 9: Example Spells

**Outcome:** Suite of example spells demonstrating system capabilities.

**Verification:**
```bash
cargo run --example p44_spell_gallery
# Interactive gallery of example spells
```

**Tasks:**
1. Fireball (basic projectile)
2. Ice spike (ground-targeting)
3. Chain lightning (target-chaining)
4. Meteor (high-energy, large volume)
5. Shield (defensive, area effect)

---

## Naming Conventions

### Files
- Rust modules: `snake_case.rs`
- Lua modules: `snake_case.lua`
- Package directories: `snake_case/`

### Components/Structs
- `SpellObject` - The ECS component
- `SpellModule` - Behavior trait
- `SpellState` - State flowing through modules
- `SpellWorld` - World context for queries
- `CostTape` - Cost accounting record

### Module Names
- Physics: `Launch`, `ApplyGravity`, `AntiGravity`, `Thrust`
- Sensors: `OnGroundHit`, `OnTimeout`, `OnTargetNear`, `WhenEnergyLow`
- Transforms: `Explosion`, `Split`, `Morph`, `Fizzle`
- Composition: `Sequential`, `Parallel`

---

## Directory Structure (Anticipated)

```
creature_3d_studio/
  crates/
    studio_spell/
      src/
        lib.rs
        module.rs
        modules/
          mod.rs
          gravity.rs
          launch.rs
          thrust.rs
          sensors.rs
          explosion.rs
          split.rs
          fizzle.rs
          sequential.rs
          parallel.rs
        spell_object.rs
        energy.rs
        tape.rs
        volume.rs
        lua_api.rs
        package.rs
        systems.rs
        plugin.rs
      docs/
        DESIGN.md
      Cargo.toml
      README.md
      
  assets/
    spells/
      core/
        manifest.lua
        modules/
          basic_physics.lua
        spells/
          fireball.lua
          icebolt.lua
      
  examples/
    p40_spell_test.rs
    p41_spell_energy_test.rs
    p42_spell_visual_test.rs
    p43_package_test.rs
    p44_spell_gallery.rs
    
  docs/
    plans/
      spell_system.md          # This document
```

---

## Risks & Mitigations

### Performance
**Risk:** Complex spell brains with many modules could be expensive.
**Mitigation:** 
- Profile early with many simultaneous spells
- Consider budgeting system (max spells per frame)
- Implement LOD for distant spells (simplified physics)

### Lua Overhead
**Risk:** Lua-defined modules could be slower than Rust.
**Mitigation:**
- Core physics modules implemented in Rust
- Lua only for composition and custom logic
- Cache Lua function references

### Energy Balance
**Risk:** Hard to balance costs so spells feel fair.
**Mitigation:**
- CostTape provides detailed breakdown
- Make all costs configurable
- Playtest with real scenarios early

### Package Security
**Risk:** Malicious packages could execute arbitrary code.
**Mitigation:**
- Sandbox Lua execution
- Review packages before adding to repository
- Package signing for trusted sources

---

## Future Considerations

1. **Spell Crafting UI**: Visual node-based spell editor
2. **Spell Learning**: Characters learn spells by observation
3. **Spell Evolution**: Spells that improve with use
4. **Cooperative Casting**: Multiple casters combine spells
5. **Environmental Interaction**: Spells interact with weather, time of day
6. **AI Spellcasting**: NPCs compose spells dynamically

---

## References

- PyTorch `nn.Module`: https://pytorch.org/docs/stable/nn.html
- Original LuaTorch: https://github.com/torch/nn
- MLua (Rust Lua bindings): https://github.com/mlua-rs/mlua
- Bevy ECS: https://bevyengine.org/
