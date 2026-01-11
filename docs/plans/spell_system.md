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

A spell object is a physical entity in the voxel world. Its "state" is simply **where it is in the spell graph** plus **how much energy remains**:

```rust
/// ECS component for a spell in the world
struct SpellObject {
    // === The spell's current form (graph position) ===
    form: Box<dyn SpellNode>,       // Current active node
    
    // === Energy (the only "resource") ===
    energy: f32,                    // Remaining energy
    
    // === Physical state of current form ===
    position: Vec3,
    velocity: Vec3,
    mass: f32,                      // Derived from energy * density
    
    // === Visual ===
    color: Color,
    
    // === Debugging ===
    last_tape: Option<CostTape>,    // Tape from last tick (for inspection)
}
```

**There is no SpellState enum.** The spell's "state" is:
- Which node is active (`form`)
- How much energy remains (`energy`)  
- Physical properties (`position`, `velocity`, `mass`)

When `energy` reaches 0 or the form signals `Complete`, the spell is done.

**Volume-Energy Relationship:**
- Each voxel has a maximum energy density (~100 units)
- A spell with 1000 energy needs at least 10 voxels
- As energy depletes, voxels are consumed from the outer surface
- High-power spells are physically larger (can't hide a nuke in a marble)

### 2. Spell Graph and Energy Pointer

A spell's "state" is not an enum. It is:
1. **Where you are in the graph** (current node/form)
2. **How much energy/mass remains** at that node
3. **Physical properties** (position, velocity) of the current form

```rust
/// A spell is energy bound to a form (graph node)
struct SpellInstance {
    /// Current form - the active node in the spell graph
    form: SpellFormRef,
    
    /// Energy remaining in this instance
    energy: f32,
    
    /// Physical state of the current form
    position: Vec3,
    velocity: Vec3,
    mass: f32,  // Derived from energy via density
}

/// Reference to a node in the spell graph
enum SpellFormRef {
    /// Projectile form - has velocity, affected by physics
    Projectile { module_idx: usize },
    /// Effect form - stationary, applies effect over time
    Effect { module_idx: usize },
    /// Terminal - spell is complete
    Exhausted,
}
```

**Key insight**: There is no "SpellState" blob passed around. The spell IS the current node plus energy. When a sensor triggers, it tells the parent to transfer energy to a new form:

```
Example: Fireball with 100 energy
  1. Enter Sequential, costs 5 → 95 energy remains
  2. Launch converts 50 energy to mass+velocity → 45 energy cruising as Projectile
  3. Parallel runs: [GroundSensor, Projectile physics]
  4. GroundSensor triggers → tells parent to convert remaining energy to Explosion form
  5. Explosion form receives 45 energy, computes effect from mass/velocity/energy
  6. Explosion consumes energy over radius expansion → 0 energy → Exhausted
```

### 3. Spell Modules (Graph Nodes)

Modules are nodes in the spell graph. Each node type has different behavior:

```rust
/// A node in the spell graph
trait SpellNode: Send + Sync {
    /// Execute one tick. Actions are recorded to the tape.
    /// Does NOT return cost - cost is implicit in tape actions.
    fn tick(&mut self, ctx: &mut TickContext);
    
    /// Reset to initial state
    fn reset(&mut self);
    
    /// Node name for tape/debugging
    fn name(&self) -> &str;
}

/// Context passed during tick - includes the tape for recording costs
struct TickContext<'a> {
    /// The tape records all actions and their costs
    tape: &'a mut CostTape,
    
    /// Current physical state (read/write)
    position: &'a mut Vec3,
    velocity: &'a mut Vec3,
    
    /// Energy available (read-only during tick - tape handles consumption)
    energy_available: f32,
    
    /// World queries
    terrain: &'a TerrainOccupancy,
    
    /// Time
    dt: f32,
    time_alive: f32,
    
    /// Signals from node to parent
    signals: &'a mut Vec<SpellSignal>,
}

/// Signals a node can send to its parent
enum SpellSignal {
    /// Transfer all remaining energy to a new form
    TransformTo { target: Box<dyn SpellNode> },
    /// Split energy among multiple new forms
    Split { targets: Vec<Box<dyn SpellNode>>, distribution: Vec<f32> },
    /// This form is complete
    Complete,
}

### 4. Node Composition

#### Sequential
All children tick every frame in order:

```lua
local fireball = Sequential {
    Projectile {},                    -- Integrates velocity
    Gravity { strength = 9.8 },       -- Applies gravity
    GroundSensor {                    -- Watches for ground hit
        on_hit = Explosion { radius = 3 }
    },
    Timeout { seconds = 10 }          -- Fallback despawn
}
```

Execution per tick:
```
Projectile.tick(ctx)  -- position += velocity * dt
Gravity.tick(ctx)     -- velocity.y -= 9.8 * dt  
GroundSensor.tick(ctx) -- if hit: signal TransformTo(Explosion)
Timeout.tick(ctx)      -- if time_alive > 10: signal Complete
```

#### Parallel
Same as Sequential for now - all children tick. Used semantically to indicate "these run together":

```lua
local floaty_fireball = Sequential {
    Projectile {},
    Parallel {
        Gravity { strength = 9.8 },      -- Pulls down
        AntiGravity { strength = 9.8 },  -- Pushes up (costs energy!)
        GroundSensor { on_hit = Explosion { radius = 3 } },
    },
    Timeout { seconds = 10 },
}
```

#### Sensors (Conditional Triggers)
Sensors check conditions and emit signals:

```lua
GroundSensor { on_hit = Explosion { radius = 3 } }  -- TransformTo on ground hit
Timeout { seconds = 5 }                              -- Complete after 5 sec
ProximitySensor { radius = 2, on_enter = Detonate {} }
EnergySensor { below = 10, on_trigger = Fizzle {} }
```

---

## Energy & Cost System

### Cost Accounting (The "Tape")

Like PyTorch's autograd, **calling forward implies the backward tape**. Costs are NOT returned by tick - they are recorded implicitly by the actions taken.

```rust
/// The tape records all actions during a tick
struct CostTape {
    entries: Vec<CostEntry>,
    frozen: bool,  // Set after tick to prevent modification
}

struct CostEntry {
    node_name: String,
    action: CostAction,
    cost: f32,
}

enum CostAction {
    /// Physics integration (gravity, velocity)
    Physics { description: &'static str },
    /// Active force application (thrust, anti-gravity)
    Force { force_magnitude: f32 },
    /// Sensor query (ground detection, target search)
    Sensor { sensor_type: &'static str },
    /// Form transformation
    Transform { from: &'static str, to: &'static str },
}

impl CostTape {
    /// Record an action - automatically computes cost from action type
    fn record(&mut self, node: &str, action: CostAction) {
        let cost = self.compute_cost(&action);
        self.entries.push(CostEntry { 
            node_name: node.to_string(), 
            action, 
            cost 
        });
    }
    
    /// Get total cost of all recorded actions
    fn total_cost(&self) -> f32 {
        self.entries.iter().map(|e| e.cost).sum()
    }
    
    /// Check if we can afford to execute (call BEFORE applying state changes)
    fn can_afford(&self, available_energy: f32) -> bool {
        self.total_cost() <= available_energy
    }
}
```

### Two-Pass Execution Model

Since cost is implicit in actions, we may need two passes:

```rust
fn execute_tick(spell: &mut SpellInstance, dt: f32, world: &World) {
    // Pass 1: Record actions to tape (dry run)
    let mut tape = CostTape::new();
    let mut ctx = TickContext::new(&mut tape, spell, world, dt);
    spell.form.tick(&mut ctx);
    
    // Check affordability
    if !tape.can_afford(spell.energy) {
        // Can't afford - spell fizzles
        spell.form = SpellFormRef::Exhausted;
        log::info!("Spell fizzled: needed {} energy, had {}", 
                   tape.total_cost(), spell.energy);
        return;
    }
    
    // Pass 2: Actually apply state changes
    tape.freeze();
    spell.energy -= tape.total_cost();
    apply_recorded_actions(spell, &tape);
}
```

**Alternative (simpler)**: For Phase 0, we can use a single pass where actions that exceed energy simply don't execute, and the spell fizzles mid-tick. Optimize to two-pass later if needed.

The tape allows:
1. **Reporting**: Show player exactly why their spell fizzled ("AntiGravity cost 6.2 energy but you only had 5.1")
2. **Debugging**: Full trace of spell behavior step by step
3. **Balancing**: Tune cost formulas in one place (CostTape::compute_cost)

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
-- Note: tick does NOT return cost. Actions record to tape implicitly.

local Node = spell.Node

local MyHomingNode = Node:extend("MyHomingNode")

function MyHomingNode:init(params)
    self.turn_rate = params.turn_rate or 5.0
    self.target_type = params.target_type or "nearest_enemy"
end

function MyHomingNode:tick(ctx)
    -- Recording a sensor query costs energy (implicit via tape)
    local target = ctx:find_target(self.target_type)
    
    if target then
        local to_target = (target.position - ctx.position):normalize()
        local current_dir = ctx.velocity:normalize()
        
        -- Applying force costs energy (implicit via tape)
        local turn_force = self.turn_rate * ctx.dt
        ctx:apply_steering(to_target, turn_force)
    end
    
    -- No return value - cost is already on the tape
end

return MyHomingNode
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
// crates/studio_spell/src/node.rs

use bevy::prelude::*;

/// A node in the spell graph. Nodes do NOT return cost - cost is 
/// recorded to the tape implicitly via actions.
pub trait SpellNode: Send + Sync {
    /// Execute one tick. Record actions to ctx.tape.
    /// Physical state changes go through ctx.
    /// Signals (transform, split, complete) go to ctx.signals.
    fn tick(&mut self, ctx: &mut TickContext);
    
    /// Reset node to initial state (for reuse)
    fn reset(&mut self);
    
    /// Node name for tape entries
    fn name(&self) -> &str;
    
    /// Clone into boxed trait object
    fn box_clone(&self) -> Box<dyn SpellNode>;
}

/// Context for a tick - provides tape, state access, and world queries
pub struct TickContext<'a> {
    // === Cost tape (write-only during tick) ===
    pub tape: &'a mut CostTape,
    
    // === Physical state (read/write) ===
    pub position: Vec3,
    pub velocity: Vec3,
    pub mass: f32,
    
    // === Energy (read-only - tape handles consumption) ===
    pub energy_available: f32,
    
    // === Time ===
    pub dt: f32,
    pub time_alive: f32,
    
    // === World queries ===
    pub terrain: &'a TerrainOccupancy,
    
    // === Output signals ===
    pub signals: Vec<SpellSignal>,
}

/// Signals a node can emit
pub enum SpellSignal {
    /// Transfer all energy to a new form
    TransformTo(Box<dyn SpellNode>),
    /// Split into multiple forms
    Split { targets: Vec<Box<dyn SpellNode>>, weights: Vec<f32> },
    /// This form is complete (spell ends)
    Complete,
}
```

### Bevy Integration

```rust
// crates/studio_spell/src/spell_object.rs

use bevy::prelude::*;

/// ECS component for a spell object
#[derive(Component)]
pub struct SpellObject {
    /// Current form (graph node)
    pub form: Box<dyn SpellNode>,
    
    /// Energy remaining
    pub energy: f32,
    
    /// Physical state
    pub position: Vec3,
    pub velocity: Vec3,
    pub mass: f32,
    
    /// Timing
    pub time_alive: f32,
    
    /// Visual
    pub color: Color,
    
    /// Debug: last tick's tape
    pub last_tape: Option<CostTape>,
}

/// Resource for spell definitions
#[derive(Resource, Default)]
pub struct SpellRegistry {
    pub spells: HashMap<String, SpellDefinition>,
}

pub struct SpellDefinition {
    pub name: String,
    pub create_form: fn() -> Box<dyn SpellNode>,
    pub default_energy: f32,
    pub default_color: Color,
}
```

### Systems

```rust
// crates/studio_spell/src/systems.rs

/// Main spell simulation system
pub fn spell_tick_system(
    mut commands: Commands,
    mut spells: Query<(Entity, &mut SpellObject, &mut Transform)>,
    terrain: Res<TerrainOccupancy>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    
    for (entity, mut spell, mut transform) in spells.iter_mut() {
        // Create fresh tape for this tick
        let mut tape = CostTape::new();
        
        // Build tick context
        let mut ctx = TickContext {
            tape: &mut tape,
            position: spell.position,
            velocity: spell.velocity,
            mass: spell.mass,
            energy_available: spell.energy,
            dt,
            time_alive: spell.time_alive,
            terrain: &terrain,
            signals: Vec::new(),
        };
        
        // Execute current form - actions record to tape
        spell.form.tick(&mut ctx);
        
        // Check if we can afford the tick
        let cost = tape.total_cost();
        if cost > spell.energy {
            // Fizzle - can't afford
            log::debug!("Spell fizzled: cost {} > energy {}", cost, spell.energy);
            commands.entity(entity).despawn_recursive();
            continue;
        }
        
        // Apply results
        spell.energy -= cost;
        spell.position = ctx.position;
        spell.velocity = ctx.velocity;
        spell.mass = ctx.mass;
        spell.time_alive += dt;
        spell.last_tape = Some(tape);
        
        // Update transform
        transform.translation = spell.position;
        
        // Handle signals
        for signal in ctx.signals {
            match signal {
                SpellSignal::TransformTo(new_form) => {
                    spell.form = new_form;
                }
                SpellSignal::Complete => {
                    commands.entity(entity).despawn_recursive();
                }
                SpellSignal::Split { targets, weights } => {
                    // Spawn new spell entities for each target
                    // (implementation in spawn_split_spells helper)
                }
            }
        }
        
        // Check energy exhaustion
        if spell.energy <= 0.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}
```

### Lua Bindings (Phase 6 - Deferred)

```rust
// crates/studio_spell/src/lua_api.rs
// NOTE: This is Phase 6, not needed for initial implementation

use mlua::prelude::*;

pub fn register_spell_api(lua: &Lua) -> LuaResult<()> {
    let spell = lua.create_table()?;
    
    // Node constructors - return boxed SpellNode
    spell.set("Sequential", lua.create_function(create_sequential)?)?;
    spell.set("Parallel", lua.create_function(create_parallel)?)?;
    spell.set("Projectile", lua.create_function(create_projectile)?)?;
    spell.set("Gravity", lua.create_function(create_gravity)?)?;
    spell.set("AntiGravity", lua.create_function(create_anti_gravity)?)?;
    spell.set("GroundSensor", lua.create_function(create_ground_sensor)?)?;
    spell.set("Timeout", lua.create_function(create_timeout)?)?;
    spell.set("Explosion", lua.create_function(create_explosion)?)?;
    
    // Registration and casting
    spell.set("register", lua.create_function(register_spell)?)?;
    spell.set("cast", lua.create_function(cast_spell)?)?;
    
    lua.globals().set("spell", spell)?;
    Ok(())
}

fn create_sequential<'lua>(
    lua: &'lua Lua,
    children: LuaTable<'lua>,
) -> LuaResult<LuaAnyUserData<'lua>> {
    let mut nodes: Vec<Box<dyn SpellNode>> = Vec::new();
    
    for pair in children.pairs::<i32, LuaAnyUserData>() {
        let (_, ud) = pair?;
        let node: &LuaSpellNode = ud.borrow()?;
        nodes.push(node.inner.box_clone());
    }
    
    lua.create_userdata(LuaSpellNode { 
        inner: Box::new(SequentialNode { children: nodes }) 
    })
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

### Phase 0: Minimal Fireball End-to-End

**Goal:** A single glowing voxel that moves forward in a straight line. No gravity, no sensors, no explosions. Just: spawn → move → despawn after 3 seconds.

**Test Environment:**
- Simple flat voxel terrain (16x16 platform)
- Fixed camera looking at the platform
- Auto-spawns a fireball every 2 seconds from position (0, 5, 8) toward (0, 5, 0)
- Can run headless or rendered
- Video capture support for visual verification

**Verification (Measurable):**
```bash
# Build and run headless test
cargo test -p studio_spell fireball_straight_line

# Test assertions:
# 1. Spell spawns at (0, 5, 8)
# 2. After 1.0 seconds at speed 10, spell is at approximately (0, 5, -2) ± 0.1
# 3. After 3.0 seconds, spell entity is despawned (timeout)
# 4. Zero panics, zero errors in log

# Visual verification (optional)
cargo run --example p40_spell_fireball -- --record
# Creates: screenshots/spell_test/fireball_straight.mp4
# Verify: glowing voxel moves in straight line, disappears after 3 sec
```

**Files Created:**
```
crates/studio_spell/
  Cargo.toml
  src/
    lib.rs                 # Crate root, exports
    node.rs                # SpellNode trait
    tape.rs                # CostTape (minimal - just records, no cost calc yet)
    spell_object.rs        # SpellObject component
    systems.rs             # spell_tick_system
    nodes/
      mod.rs
      projectile.rs        # ProjectileNode: velocity integration
      timeout.rs           # TimeoutNode: despawn after N seconds
      sequential.rs        # SequentialNode: run children in order
    plugin.rs              # SpellPlugin for Bevy

examples/
  p40_spell_fireball.rs    # Test harness
```

**Pseudocode for Fireball Graph:**
```
Fireball = Sequential [
    Projectile { velocity: Vec3(0, 0, -10) },  // Constant velocity, no gravity
    Timeout { seconds: 3.0 }                   // Despawn signal after 3 sec
]

// Projectile.tick(ctx):
//   ctx.tape.record("Projectile", Physics { description: "velocity_integration" })
//   ctx.position += ctx.velocity * ctx.dt
//
// Timeout.tick(ctx):
//   if ctx.time_alive >= self.seconds:
//     ctx.signals.push(SpellSignal::Complete)
//
// Sequential.tick(ctx):
//   for child in children:
//     child.tick(ctx)
```

**Tasks:**
1. Create `crates/studio_spell/Cargo.toml` with bevy dependency
2. Create `SpellNode` trait in `node.rs` (tick takes `&mut TickContext`, no return)
3. Create `CostTape` in `tape.rs` (just Vec<CostEntry>, no cost calculation yet)
4. Create `ProjectileNode` in `nodes/projectile.rs` (integrates velocity)
5. Create `TimeoutNode` in `nodes/timeout.rs` (emits Complete signal)
6. Create `SequentialNode` in `nodes/sequential.rs` (runs children)
7. Create `SpellObject` component in `spell_object.rs`
8. Create `spell_tick_system` in `systems.rs`
9. Create `SpellPlugin` in `plugin.rs`
10. Create `examples/p40_spell_fireball.rs` test harness with video capture
11. Write unit test `fireball_straight_line` that asserts position after 1 second

---

### Phase 1: Add Gravity

**Goal:** Fireball arcs downward due to gravity. Separate test for anti-gravity (straight line despite world gravity).

**Verification:**
```bash
cargo test -p studio_spell fireball_with_gravity

# Assertions:
# 1. With GravityNode, after 1 sec starting at (0, 5, 8) with vel (0, 0, -10):
#    y position < 5.0 (fell due to gravity)
#    z position ≈ -2 (still moving forward)
# 2. With AntiGravityNode canceling gravity:
#    y position ≈ 5.0 (no vertical change)

cargo run --example p40_spell_fireball -- --gravity --record
# Video shows arcing trajectory
```

**Files Modified/Created:**
```
crates/studio_spell/src/nodes/
  gravity.rs       # NEW: GravityNode (applies g * dt to velocity.y)
  anti_gravity.rs  # NEW: AntiGravityNode (cancels gravity, costs energy)
```

**Tasks:**
1. Create `GravityNode` - applies gravity to velocity each tick
2. Create `AntiGravityNode` - applies upward force equal to gravity
3. Update test harness with `--gravity` flag
4. Add unit tests for gravity arc math

---

### Phase 2: Ground Collision Sensor

**Goal:** Fireball detects when it hits the terrain and emits a signal.

**Verification:**
```bash
cargo test -p studio_spell fireball_ground_hit

# Assertions:
# 1. Fireball starts at (0, 5, 0), gravity pulls it down
# 2. Terrain at y=0
# 3. When position.y <= terrain_height + 0.5, GroundSensor triggers
# 4. GroundSensor emits TransformTo signal (for now, just Complete)
# 5. Spell despawns on ground hit, NOT after timeout

cargo run --example p40_spell_fireball -- --gravity --ground-hit --record
# Video shows: fireball arcs down, hits ground, disappears immediately
```

**Files Created:**
```
crates/studio_spell/src/nodes/
  ground_sensor.rs  # NEW: queries terrain, emits signal on collision
```

**Tasks:**
1. Create `GroundSensorNode` with `on_hit: SpellSignal` parameter
2. Integrate with `TerrainOccupancy` query in TickContext
3. Add `--ground-hit` flag to test harness
4. Unit test for ground detection logic

---

### Phase 3: Tape-Based Energy Consumption

**Goal:** Actions cost energy. Spell fizzles if it can't afford a tick.

**Verification:**
```bash
cargo test -p studio_spell energy_consumption

# Assertions:
# 1. Spell starts with 10 energy
# 2. AntiGravity costs 2 energy/second
# 3. After 5 seconds, energy = 0, spell fizzles
# 4. Spell with 100 energy lasts full 50 seconds of anti-gravity

cargo test -p studio_spell insufficient_energy_fizzle

# 1. Spell starts with 1 energy
# 2. First tick costs 2 energy (anti-gravity)
# 3. tape.total_cost() > energy_available
# 4. Spell fizzles immediately with log message

cargo run --example p40_spell_fireball -- --energy=10 --record
# Video shows: spell flies, then fizzles mid-air when energy runs out
```

**Files Modified:**
```
crates/studio_spell/src/
  tape.rs          # Add compute_cost() function, cost formulas
  systems.rs       # Check tape.can_afford() before applying state
```

**Tasks:**
1. Implement `CostTape::compute_cost()` with action-type-based formulas
2. Implement `tape.can_afford(energy)` check in `spell_tick_system`
3. Add fizzle behavior when can't afford
4. Add `--energy=N` flag to test harness
5. Unit tests for cost calculation and fizzle

---

### Phase 4: Form Transformation

**Goal:** On ground hit, fireball transforms into explosion form (for now, explosion just logs and despawns).

**Verification:**
```bash
cargo test -p studio_spell form_transformation

# Assertions:
# 1. Fireball with GroundSensor { on_hit: TransformTo(ExplosionNode) }
# 2. On ground hit, form changes to ExplosionNode
# 3. ExplosionNode.tick() logs "Explosion at (x, y, z) with E energy"
# 4. ExplosionNode emits Complete after 1 tick

cargo run --example p40_spell_fireball -- --gravity --explosion --record
# Video: fireball arcs, hits ground, brief flash (explosion), gone
```

**Files Created:**
```
crates/studio_spell/src/nodes/
  explosion.rs     # NEW: logs explosion, emits Complete
```

**Tasks:**
1. Create `ExplosionNode` (placeholder - just logs and completes)
2. Update `GroundSensorNode` to take `on_hit: Box<dyn SpellNode>`
3. Implement signal handling in `spell_tick_system` for `TransformTo`
4. Unit test for transformation flow

---

### Phase 5: Visual Rendering

**Goal:** Spell renders as a glowing emissive voxel. Uses existing deferred renderer.

**Verification:**
```bash
cargo run --example p40_spell_fireball -- --rendered --record
# Video shows: orange glowing voxel moving through scene
# Bloom visible around the voxel
# Trail effect (optional, can defer)

# Screenshot test:
cargo run --example p40_spell_fireball -- --screenshot=spell_glow.png
# Inspect: voxel is emissive orange, bloom halo visible
```

**Files Created/Modified:**
```
crates/studio_spell/src/
  render.rs        # NEW: SpellVoxelVolume component, spawn mesh
  systems.rs       # Add mesh spawning on spell creation
```

**Tasks:**
1. Create `SpellVoxelVolume` component
2. Spawn emissive voxel mesh when SpellObject spawns
3. Update mesh position each tick
4. Despawn mesh when spell despawns
5. Test with deferred renderer bloom

---

### Later Phases (Deferred)

**Phase 6: Lua API** - Expose nodes to Lua for spell definition
**Phase 7: Package System** - Load spell definitions from files
**Phase 8: Volume Scaling** - Spell size proportional to energy
**Phase 9: Advanced Nodes** - Split, Homing, Area effects

---

## Early End-to-End: Fireball Flow

This section provides complete pseudocode for the minimal fireball implementation (Phases 0-2).

### Required Objects

```rust
// === Core Types ===

struct CostTape {
    entries: Vec<CostEntry>,
}
struct CostEntry {
    node: String,
    action: String,
    cost: f32,
}

struct TickContext<'a> {
    tape: &'a mut CostTape,
    position: Vec3,
    velocity: Vec3,
    energy_available: f32,
    dt: f32,
    time_alive: f32,
    terrain: &'a TerrainOccupancy,
    signals: Vec<SpellSignal>,
}

enum SpellSignal {
    TransformTo(Box<dyn SpellNode>),
    Complete,
}

trait SpellNode: Send + Sync {
    fn tick(&mut self, ctx: &mut TickContext);
    fn name(&self) -> &str;
}

// === Node Implementations ===

struct ProjectileNode;  // Just integrates velocity

impl SpellNode for ProjectileNode {
    fn tick(&mut self, ctx: &mut TickContext) {
        ctx.tape.record("Projectile", "velocity_integration", 0.0);
        ctx.position += ctx.velocity * ctx.dt;
    }
    fn name(&self) -> &str { "Projectile" }
}

struct GravityNode { strength: f32 }

impl SpellNode for GravityNode {
    fn tick(&mut self, ctx: &mut TickContext) {
        ctx.tape.record("Gravity", "apply_gravity", 0.0);  // Free
        ctx.velocity.y -= self.strength * ctx.dt;
    }
    fn name(&self) -> &str { "Gravity" }
}

struct AntiGravityNode { strength: f32 }

impl SpellNode for AntiGravityNode {
    fn tick(&mut self, ctx: &mut TickContext) {
        let cost = self.strength * 2.0 * ctx.dt;  // 2 energy/sec per unit strength
        ctx.tape.record("AntiGravity", "counter_gravity", cost);
        ctx.velocity.y += self.strength * ctx.dt;
    }
    fn name(&self) -> &str { "AntiGravity" }
}

struct TimeoutNode { seconds: f32 }

impl SpellNode for TimeoutNode {
    fn tick(&mut self, ctx: &mut TickContext) {
        if ctx.time_alive >= self.seconds {
            ctx.signals.push(SpellSignal::Complete);
        }
    }
    fn name(&self) -> &str { "Timeout" }
}

struct GroundSensorNode { 
    threshold: f32,
    on_hit: Option<Box<dyn SpellNode>>,
}

impl SpellNode for GroundSensorNode {
    fn tick(&mut self, ctx: &mut TickContext) {
        ctx.tape.record("GroundSensor", "query_terrain", 0.1 * ctx.dt);
        
        let ground_y = ctx.terrain.height_at(ctx.position.x, ctx.position.z);
        if ctx.position.y <= ground_y + self.threshold {
            if let Some(target) = self.on_hit.take() {
                ctx.signals.push(SpellSignal::TransformTo(target));
            } else {
                ctx.signals.push(SpellSignal::Complete);
            }
        }
    }
    fn name(&self) -> &str { "GroundSensor" }
}

struct SequentialNode { children: Vec<Box<dyn SpellNode>> }

impl SpellNode for SequentialNode {
    fn tick(&mut self, ctx: &mut TickContext) {
        for child in &mut self.children {
            child.tick(ctx);
        }
    }
    fn name(&self) -> &str { "Sequential" }
}

struct ParallelNode { children: Vec<Box<dyn SpellNode>> }

impl SpellNode for ParallelNode {
    fn tick(&mut self, ctx: &mut TickContext) {
        // Same as sequential for now - all run each tick
        for child in &mut self.children {
            child.tick(ctx);
        }
    }
    fn name(&self) -> &str { "Parallel" }
}
```

### Fireball Graph Construction

```rust
// Fireball with gravity, ground detection, explosion on hit
fn create_fireball(direction: Vec3, speed: f32) -> Box<dyn SpellNode> {
    Box::new(SequentialNode {
        children: vec![
            // Initial velocity
            Box::new(ProjectileNode),
            
            // Physics + sensors run in parallel
            Box::new(ParallelNode {
                children: vec![
                    Box::new(GravityNode { strength: 9.8 }),
                    Box::new(GroundSensorNode {
                        threshold: 0.5,
                        on_hit: Some(Box::new(ExplosionNode { radius: 3.0 })),
                    }),
                ],
            }),
            
            // Fallback timeout
            Box::new(TimeoutNode { seconds: 10.0 }),
        ],
    })
}

// Anti-gravity fireball (straight line)
fn create_floaty_fireball(direction: Vec3, speed: f32) -> Box<dyn SpellNode> {
    Box::new(SequentialNode {
        children: vec![
            Box::new(ProjectileNode),
            Box::new(ParallelNode {
                children: vec![
                    Box::new(GravityNode { strength: 9.8 }),
                    Box::new(AntiGravityNode { strength: 9.8 }),  // Cancels gravity
                    Box::new(GroundSensorNode {
                        threshold: 0.5,
                        on_hit: Some(Box::new(ExplosionNode { radius: 3.0 })),
                    }),
                ],
            }),
            Box::new(TimeoutNode { seconds: 10.0 }),
        ],
    })
}
```

### Main Tick Loop

```rust
fn spell_tick_system(
    mut commands: Commands,
    mut spells: Query<(Entity, &mut SpellObject, &mut Transform)>,
    terrain: Res<TerrainOccupancy>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    
    for (entity, mut spell, mut transform) in spells.iter_mut() {
        spell.time_alive += dt;
        
        // Fresh tape for this tick
        let mut tape = CostTape::new();
        
        // Build context
        let mut ctx = TickContext {
            tape: &mut tape,
            position: spell.position,
            velocity: spell.velocity,
            energy_available: spell.energy,
            dt,
            time_alive: spell.time_alive,
            terrain: &terrain,
            signals: Vec::new(),
        };
        
        // Run the spell graph
        spell.form.tick(&mut ctx);
        
        // Check if we can afford it
        let total_cost = tape.total_cost();
        if total_cost > spell.energy {
            // Fizzle
            println!("Spell fizzled: needed {}, had {}", total_cost, spell.energy);
            commands.entity(entity).despawn_recursive();
            continue;
        }
        
        // Apply state changes
        spell.energy -= total_cost;
        spell.position = ctx.position;
        spell.velocity = ctx.velocity;
        transform.translation = spell.position;
        
        // Process signals
        for signal in ctx.signals {
            match signal {
                SpellSignal::TransformTo(new_form) => {
                    spell.form = new_form;
                }
                SpellSignal::Complete => {
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
    }
}
```

### Test Harness Structure

```rust
// examples/p40_spell_fireball.rs

fn main() {
    let args = Args::parse();
    
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SpellPlugin)
        .add_plugins(if args.headless {
            HeadlessPlugin
        } else {
            RenderPlugin
        })
        .insert_resource(TestConfig {
            spawn_interval: 2.0,
            use_gravity: args.gravity,
            use_ground_hit: args.ground_hit,
            record_video: args.record,
            initial_energy: args.energy.unwrap_or(100.0),
        })
        .add_systems(Startup, setup_test_scene)
        .add_systems(Update, spawn_fireballs)
        .add_systems(Update, record_video.run_if(|c: Res<TestConfig>| c.record_video))
        .run();
}

fn setup_test_scene(mut commands: Commands) {
    // 16x16 flat terrain at y=0
    // Camera at (0, 10, 20) looking at origin
}

fn spawn_fireballs(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<TestConfig>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer >= config.spawn_interval {
        *timer = 0.0;
        
        let form = if config.use_gravity {
            create_fireball(Vec3::NEG_Z, 10.0)
        } else {
            create_floaty_fireball(Vec3::NEG_Z, 10.0)
        };
        
        commands.spawn(SpellObject {
            form,
            energy: config.initial_energy,
            position: Vec3::new(0.0, 5.0, 8.0),
            velocity: Vec3::new(0.0, 0.0, -10.0),
            mass: 1.0,
            time_alive: 0.0,
            color: Color::ORANGE,
            last_tape: None,
        });
    }
}
```

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
