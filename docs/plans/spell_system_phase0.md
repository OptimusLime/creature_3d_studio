# Spell System Phase 0: Foundation

## Purpose

This document details the foundational work required before we can have a working fireball. Phase 0 in the main design doc was too vague - it assumed Lua integration "just works" and jumped straight to rendering. 

This document breaks Phase 0 into sub-phases that:
1. Establish Lua mechanics (require, sandboxing, object extension)
2. Build Rust infrastructure for spell execution
3. Integrate with game world (terrain, physics)
4. Finally render a visible fireball

Each sub-phase is independently testable without rendering or complex simulation.

---

## Critical Unknowns to Resolve

### Lua Integration Questions

1. **Does `require` work in MLua?**
   - Can we call `require("my_module")` from Lua?
   - How does MLua resolve module paths?
   - Can we customize the module loader?

2. **Can we sandbox `require`?**
   - We do NOT want Lua to import standard libraries (io, os, etc.)
   - We need to whitelist only our spell modules
   - How do we intercept/replace the require function?

3. **How do we structure relative imports?**
   - `require("spells/fireball")` should load from our assets
   - `require("../base_projectile")` - does this work?
   - How do we set the Lua path?

4. **Can Lua objects extend other Lua objects?**
   - `HomingFireball = Fireball:extend({ ... })`
   - How does inheritance/composition work in our Lua environment?
   - Do we need to provide a class system, or use metatables directly?

### Rust Integration Questions

1. **How do we pass Rust objects to Lua?**
   - SpellNode trait objects
   - TickContext with mutable references
   - Terrain queries

2. **How do we call Lua functions from Rust?**
   - `spell.tick(ctx)` - calling a Lua method
   - Handling Lua errors gracefully
   - Performance of Rust↔Lua boundary

3. **How do we handle Lua-defined nodes in the spell graph?**
   - Lua defines a node, Rust executes it
   - Mixed graphs (some Rust nodes, some Lua nodes)

---

## Sub-Phases

### Phase 0A: Lua Mechanics (No Spells Yet)

**Goal:** Understand and establish Lua module loading, sandboxing, and object extension. No spell concepts yet - just Lua infrastructure.

#### 0A.1: Basic MLua Setup

**Question:** Can we create a Lua VM and call functions?

**Test:**
```rust
#[test]
fn test_mlua_basic() {
    let lua = Lua::new();
    
    // Execute Lua code
    lua.load(r#"
        function add(a, b)
            return a + b
        end
    "#).exec().unwrap();
    
    // Call Lua function from Rust
    let add: Function = lua.globals().get("add").unwrap();
    let result: i32 = add.call((2, 3)).unwrap();
    assert_eq!(result, 5);
}
```

**Verification:**
```bash
cargo test -p studio_spell test_mlua_basic
# Passes: we can create VM, define functions, call them
```

**Files:**
```
crates/studio_spell/
  Cargo.toml          # Add mlua dependency
  src/
    lib.rs
    lua/
      mod.rs
      vm.rs           # Lua VM wrapper
```

---

#### 0A.2: Custom Require Function

**Question:** Can we replace `require` with our own loader that only loads from our assets?

**Test:**
```rust
#[test]
fn test_custom_require() {
    let lua = Lua::new();
    
    // Remove default require, install our own
    setup_sandboxed_require(&lua, "assets/scripts/").unwrap();
    
    // This should work - loads from our assets
    lua.load(r#"
        local utils = require("utils")
        assert(utils.clamp(5, 0, 3) == 3)
    "#).exec().unwrap();
}

#[test]
fn test_require_blocks_stdlib() {
    let lua = Lua::new();
    setup_sandboxed_require(&lua, "assets/scripts/").unwrap();
    
    // This should FAIL - io is blocked
    let result = lua.load(r#"
        local io = require("io")
    "#).exec();
    
    assert!(result.is_err());
}

#[test]
fn test_require_relative_path() {
    let lua = Lua::new();
    setup_sandboxed_require(&lua, "assets/scripts/").unwrap();
    
    // assets/scripts/spells/fireball.lua does:
    //   local base = require("spells/base_projectile")
    lua.load(r#"
        local fireball = require("spells/fireball")
        assert(fireball.name == "Fireball")
    "#).exec().unwrap();
}
```

**Verification:**
```bash
cargo test -p studio_spell test_custom_require
cargo test -p studio_spell test_require_blocks_stdlib
cargo test -p studio_spell test_require_relative_path
```

**Research needed:**
- How does MLua's `package.loaders` / `package.searchers` work?
- Can we completely replace the require function?
- Reference: https://docs.rs/mlua/latest/mlua/struct.Lua.html

**Files:**
```
crates/studio_spell/src/lua/
  require.rs          # Custom require implementation
  sandbox.rs          # Sandboxing utilities

assets/scripts/
  utils.lua           # Test utility module
  spells/
    base_projectile.lua
    fireball.lua
```

---

#### 0A.3: Object Extension System

**Question:** Can Lua objects extend/inherit from other Lua objects?

We need a pattern like:
```lua
-- base_projectile.lua
local Projectile = {}
Projectile.__index = Projectile

function Projectile:new(params)
    local o = setmetatable({}, self)
    o.speed = params.speed or 10
    return o
end

function Projectile:tick(ctx)
    ctx.position = ctx.position + ctx.velocity * ctx.dt
end

return Projectile

-- fireball.lua
local Projectile = require("spells/base_projectile")

local Fireball = setmetatable({}, { __index = Projectile })
Fireball.__index = Fireball

function Fireball:new(params)
    local o = Projectile.new(self, params)
    o.color = params.color or "orange"
    return o
end

-- Override tick to add effects
function Fireball:tick(ctx)
    Projectile.tick(self, ctx)  -- Call parent
    -- Add fireball-specific behavior
end

return Fireball
```

**Test:**
```rust
#[test]
fn test_lua_inheritance() {
    let lua = Lua::new();
    setup_sandboxed_require(&lua, "assets/scripts/").unwrap();
    
    lua.load(r#"
        local Fireball = require("spells/fireball")
        local fb = Fireball:new({ speed = 20 })
        
        -- Has parent's properties
        assert(fb.speed == 20)
        
        -- Has child's properties
        assert(fb.color == "orange")
        
        -- Can call parent's methods
        local ctx = { position = 0, velocity = 1, dt = 0.1 }
        fb:tick(ctx)
        assert(ctx.position == 0.1)
    "#).exec().unwrap();
}

#[test]
fn test_lua_method_override() {
    let lua = Lua::new();
    setup_sandboxed_require(&lua, "assets/scripts/").unwrap();
    
    lua.load(r#"
        local Projectile = require("spells/base_projectile")
        local Fireball = require("spells/fireball")
        
        -- Create instances
        local proj = Projectile:new({})
        local fb = Fireball:new({})
        
        -- Both have tick, but they might behave differently
        -- (Fireball could override to add effects)
        assert(type(proj.tick) == "function")
        assert(type(fb.tick) == "function")
    "#).exec().unwrap();
}
```

**Verification:**
```bash
cargo test -p studio_spell test_lua_inheritance
cargo test -p studio_spell test_lua_method_override
```

**Decision:** Do we provide a class library, or use raw metatables?
- Option A: Raw metatables (above example) - more explicit, no magic
- Option B: Provide `spell.Class` helper - cleaner syntax but hidden complexity
- **Recommendation:** Start with raw metatables, add helper later if needed

---

#### 0A.4: Rust↔Lua Boundary

**Question:** How do we pass Rust objects to Lua and call Lua methods from Rust?

**Test:**
```rust
#[test]
fn test_rust_calls_lua_tick() {
    let lua = Lua::new();
    setup_sandboxed_require(&lua, "assets/scripts/").unwrap();
    
    // Load a spell module
    let fireball: Table = lua.load(r#"
        return require("spells/fireball"):new({ speed = 10 })
    "#).eval().unwrap();
    
    // Create a Rust context, pass to Lua
    let ctx = lua.create_table().unwrap();
    ctx.set("position", 0.0).unwrap();
    ctx.set("velocity", 10.0).unwrap();
    ctx.set("dt", 0.1).unwrap();
    
    // Call tick method
    let tick: Function = fireball.get("tick").unwrap();
    tick.call::<_, ()>((&fireball, &ctx)).unwrap();
    
    // Check Lua modified the context
    let new_pos: f64 = ctx.get("position").unwrap();
    assert!((new_pos - 1.0).abs() < 0.001);  // 0 + 10 * 0.1 = 1.0
}

#[test]
fn test_lua_calls_rust_terrain_query() {
    let lua = Lua::new();
    
    // Register Rust function that Lua can call
    let terrain_height = lua.create_function(|_, (x, z): (f64, f64)| {
        // Simulate terrain - flat at y=0
        Ok(0.0)
    }).unwrap();
    
    lua.globals().set("terrain_height_at", terrain_height).unwrap();
    
    // Lua code calls Rust
    lua.load(r#"
        local h = terrain_height_at(5.0, 10.0)
        assert(h == 0.0)
    "#).exec().unwrap();
}
```

**Verification:**
```bash
cargo test -p studio_spell test_rust_calls_lua_tick
cargo test -p studio_spell test_lua_calls_rust_terrain_query
```

**Files:**
```
crates/studio_spell/src/lua/
  bindings.rs         # Rust functions exposed to Lua
  context.rs          # TickContext as Lua table
```

---

### Phase 0B: Spell Infrastructure (Rust Side)

**Goal:** Build Rust types for spell execution, independent of rendering or game world.

#### 0B.1: Core Types in Rust

**Test:**
```rust
#[test]
fn test_spell_instance_creation() {
    let spell = SpellInstance {
        energy: 100.0,
        position: Vec3::new(0.0, 5.0, 0.0),
        velocity: Vec3::new(0.0, 0.0, -10.0),
        time_alive: 0.0,
    };
    
    assert_eq!(spell.energy, 100.0);
    assert_eq!(spell.position.z, 0.0);
}

#[test]
fn test_cost_tape_recording() {
    let mut tape = CostTape::new();
    
    tape.record("Projectile", CostAction::Physics("velocity_integration"), 0.0);
    tape.record("Gravity", CostAction::Physics("apply_gravity"), 0.0);
    tape.record("AntiGravity", CostAction::Force(9.8), 1.96);  // 9.8 * 0.2 * dt
    
    assert_eq!(tape.entries.len(), 3);
    assert!((tape.total_cost() - 1.96).abs() < 0.001);
}

#[test]
fn test_can_afford_check() {
    let mut tape = CostTape::new();
    tape.record("AntiGravity", CostAction::Force(9.8), 5.0);
    
    assert!(tape.can_afford(10.0));  // Have 10, costs 5
    assert!(tape.can_afford(5.0));   // Exact
    assert!(!tape.can_afford(4.0));  // Not enough
}
```

**Verification:**
```bash
cargo test -p studio_spell test_spell_instance
cargo test -p studio_spell test_cost_tape
cargo test -p studio_spell test_can_afford
```

**Files:**
```
crates/studio_spell/src/
  spell.rs            # SpellInstance struct
  tape.rs             # CostTape, CostAction, CostEntry
```

---

#### 0B.2: Lua-Driven Spell Tick (No Game World)

**Question:** Can we run a spell defined in Lua through Rust's tick loop?

**Test:**
```rust
#[test]
fn test_lua_spell_tick_from_rust() {
    let lua = Lua::new();
    setup_spell_environment(&lua).unwrap();
    
    // Load fireball spell from Lua
    let spell_def: Table = lua.load(r#"
        return require("spells/fireball"):new({ speed = 10 })
    "#).eval().unwrap();
    
    // Create spell instance in Rust
    let mut spell = SpellInstance {
        lua_object: spell_def,
        energy: 100.0,
        position: Vec3::new(0.0, 5.0, 8.0),
        velocity: Vec3::new(0.0, 0.0, -10.0),
        time_alive: 0.0,
    };
    
    // Run tick from Rust (no game world, no terrain)
    let mut tape = CostTape::new();
    let signals = tick_lua_spell(&lua, &mut spell, &mut tape, 0.1);
    
    // Verify position changed
    assert!((spell.position.z - 7.0).abs() < 0.01);  // 8 + (-10 * 0.1) = 7
    
    // Verify no signals yet (no ground hit)
    assert!(signals.is_empty());
}

#[test]
fn test_lua_spell_timeout() {
    let lua = Lua::new();
    setup_spell_environment(&lua).unwrap();
    
    let spell_def: Table = lua.load(r#"
        return require("spells/fireball"):new({ timeout = 1.0 })
    "#).eval().unwrap();
    
    let mut spell = SpellInstance {
        lua_object: spell_def,
        energy: 100.0,
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        time_alive: 0.0,
    };
    
    // Tick for 0.5 seconds - no timeout yet
    let mut tape = CostTape::new();
    let signals = tick_lua_spell(&lua, &mut spell, &mut tape, 0.5);
    assert!(signals.is_empty());
    
    // Tick for another 0.6 seconds - timeout should trigger
    tape.clear();
    spell.time_alive = 0.5;
    let signals = tick_lua_spell(&lua, &mut spell, &mut tape, 0.6);
    
    assert_eq!(signals.len(), 1);
    assert!(matches!(signals[0], SpellSignal::Complete));
}
```

**Verification:**
```bash
cargo test -p studio_spell test_lua_spell_tick_from_rust
cargo test -p studio_spell test_lua_spell_timeout
```

**Files:**
```
crates/studio_spell/src/
  executor.rs         # tick_lua_spell function
  signals.rs          # SpellSignal enum

assets/scripts/spells/
  fireball.lua        # Full fireball with timeout
```

---

#### 0B.3: Spell Graph Composition

**Question:** Can we compose spells from other spells in Lua?

**Test:**
```rust
#[test]
fn test_spell_composition() {
    let lua = Lua::new();
    setup_spell_environment(&lua).unwrap();
    
    // HomingFireball extends Fireball
    let spell_def: Table = lua.load(r#"
        return require("spells/homing_fireball"):new({ 
            speed = 15,
            turn_rate = 2.0 
        })
    "#).eval().unwrap();
    
    // Should have fireball properties
    let speed: f64 = spell_def.get("speed").unwrap();
    assert_eq!(speed, 15.0);
    
    // Should have homing-specific properties
    let turn_rate: f64 = spell_def.get("turn_rate").unwrap();
    assert_eq!(turn_rate, 2.0);
}

#[test]
fn test_composed_spell_tick() {
    let lua = Lua::new();
    setup_spell_environment(&lua).unwrap();
    
    // Homing fireball needs a target
    lua.globals().set("find_nearest_target", lua.create_function(|_, pos: Table| {
        // Mock: target is at (0, 5, -20)
        let target = lua.create_table()?;
        target.set("x", 0.0)?;
        target.set("y", 5.0)?;
        target.set("z", -20.0)?;
        Ok(target)
    }).unwrap()).unwrap();
    
    let spell_def: Table = lua.load(r#"
        return require("spells/homing_fireball"):new({ 
            speed = 10,
            turn_rate = 1.0 
        })
    "#).eval().unwrap();
    
    let mut spell = SpellInstance {
        lua_object: spell_def,
        energy: 100.0,
        position: Vec3::new(0.0, 5.0, 0.0),
        velocity: Vec3::new(5.0, 0.0, -5.0),  // Moving diagonally
        time_alive: 0.0,
    };
    
    let mut tape = CostTape::new();
    tick_lua_spell(&lua, &mut spell, &mut tape, 0.1);
    
    // Velocity should have turned toward target (more negative Z)
    // This is a rough check - homing should adjust direction
    // The exact values depend on turn_rate implementation
}
```

**Verification:**
```bash
cargo test -p studio_spell test_spell_composition
cargo test -p studio_spell test_composed_spell_tick
```

**Files:**
```
assets/scripts/spells/
  homing_fireball.lua   # Extends fireball, adds homing behavior
```

---

### Phase 0C: World Integration (Terrain, Sensors)

**Goal:** Connect Lua spells to game world queries (terrain height, collision detection).

#### 0C.1: Terrain Height Query

**Test:**
```rust
#[test]
fn test_terrain_query_from_lua() {
    let lua = Lua::new();
    
    // Create mock terrain
    let terrain = MockTerrain::flat(0.0);  // Flat at y=0
    
    // Register terrain query
    register_terrain_api(&lua, &terrain).unwrap();
    
    lua.load(r#"
        local h = terrain.height_at(5.0, 10.0)
        assert(h == 0.0, "Expected flat terrain at y=0")
    "#).exec().unwrap();
}

#[test]
fn test_terrain_query_varied() {
    let lua = Lua::new();
    
    // Terrain with a hill
    let terrain = MockTerrain::with_height_fn(|x, z| {
        if x > 0.0 && x < 10.0 { 5.0 } else { 0.0 }
    });
    
    register_terrain_api(&lua, &terrain).unwrap();
    
    lua.load(r#"
        assert(terrain.height_at(-5.0, 0.0) == 0.0, "Flat area")
        assert(terrain.height_at(5.0, 0.0) == 5.0, "Hill area")
    "#).exec().unwrap();
}
```

**Verification:**
```bash
cargo test -p studio_spell test_terrain_query_from_lua
cargo test -p studio_spell test_terrain_query_varied
```

**Files:**
```
crates/studio_spell/src/
  world/
    mod.rs
    terrain.rs        # Terrain trait, MockTerrain for testing
    api.rs            # register_terrain_api
```

---

#### 0C.2: Ground Collision Sensor

**Test:**
```rust
#[test]
fn test_ground_sensor_triggers() {
    let lua = Lua::new();
    let terrain = MockTerrain::flat(0.0);
    setup_spell_environment(&lua).unwrap();
    register_terrain_api(&lua, &terrain).unwrap();
    
    // Fireball with ground sensor
    let spell_def: Table = lua.load(r#"
        local fb = require("spells/fireball"):new({ speed = 10 })
        fb.ground_sensor = { threshold = 0.5 }
        return fb
    "#).eval().unwrap();
    
    // Start above ground
    let mut spell = SpellInstance {
        lua_object: spell_def,
        energy: 100.0,
        position: Vec3::new(0.0, 5.0, 0.0),
        velocity: Vec3::new(0.0, -10.0, 0.0),  // Falling
        time_alive: 0.0,
    };
    
    // Tick until we hit ground
    for _ in 0..100 {
        let mut tape = CostTape::new();
        let signals = tick_lua_spell(&lua, &mut spell, &mut tape, 0.1);
        
        if spell.position.y <= 0.5 {
            // Should have ground hit signal
            assert!(signals.iter().any(|s| matches!(s, SpellSignal::TransformTo(_))));
            return;
        }
    }
    
    panic!("Ground sensor never triggered");
}
```

**Verification:**
```bash
cargo test -p studio_spell test_ground_sensor_triggers
```

**Files:**
```
assets/scripts/spells/
  fireball.lua        # Add ground_sensor support
```

---

#### 0C.3: Form Transformation on Hit

**Test:**
```rust
#[test]
fn test_transform_to_explosion() {
    let lua = Lua::new();
    let terrain = MockTerrain::flat(0.0);
    setup_spell_environment(&lua).unwrap();
    register_terrain_api(&lua, &terrain).unwrap();
    
    // Fireball that transforms to explosion on ground hit
    let spell_def: Table = lua.load(r#"
        local Fireball = require("spells/fireball")
        local Explosion = require("spells/explosion")
        
        local fb = Fireball:new({ speed = 10 })
        fb.on_ground_hit = function(self)
            return Explosion:new({ radius = 3.0, energy = self.energy })
        end
        return fb
    "#).eval().unwrap();
    
    let mut spell = SpellInstance {
        lua_object: spell_def,
        energy: 50.0,
        position: Vec3::new(0.0, 0.3, 0.0),  // Just above ground
        velocity: Vec3::new(0.0, -1.0, 0.0),
        time_alive: 0.0,
    };
    
    let mut tape = CostTape::new();
    let signals = tick_lua_spell(&lua, &mut spell, &mut tape, 0.1);
    
    // Should have TransformTo signal with explosion
    assert_eq!(signals.len(), 1);
    if let SpellSignal::TransformTo(new_form) = &signals[0] {
        let radius: f64 = new_form.get("radius").unwrap();
        assert_eq!(radius, 3.0);
    } else {
        panic!("Expected TransformTo signal");
    }
}
```

**Verification:**
```bash
cargo test -p studio_spell test_transform_to_explosion
```

**Files:**
```
assets/scripts/spells/
  explosion.lua       # Explosion spell form
```

---

### Phase 0D: Simulation (No Rendering)

**Goal:** Run complete spell lifecycle as a simulation, verify with assertions.

#### 0D.1: Full Fireball Lifecycle Test

**Test:**
```rust
#[test]
fn test_fireball_full_lifecycle() {
    // Setup
    let lua = Lua::new();
    let terrain = MockTerrain::flat(0.0);
    setup_spell_environment(&lua).unwrap();
    register_terrain_api(&lua, &terrain).unwrap();
    
    // Create fireball
    let spell_def: Table = lua.load(r#"
        local Fireball = require("spells/fireball")
        return Fireball:new({ 
            speed = 10,
            gravity = 9.8,
            on_ground_hit = "explode"
        })
    "#).eval().unwrap();
    
    let mut spell = SpellInstance {
        lua_object: spell_def,
        energy: 100.0,
        position: Vec3::new(0.0, 10.0, 0.0),
        velocity: Vec3::new(10.0, 0.0, 0.0),  // Horizontal
        time_alive: 0.0,
    };
    
    // Simulate until spell completes
    let mut history = Vec::new();
    let dt = 0.016;  // ~60fps
    
    for frame in 0..1000 {
        history.push(spell.position);
        
        let mut tape = CostTape::new();
        let signals = tick_lua_spell(&lua, &mut spell, &mut tape, dt);
        spell.energy -= tape.total_cost();
        spell.time_alive += dt;
        
        // Check for completion
        for signal in signals {
            match signal {
                SpellSignal::Complete => {
                    println!("Spell completed at frame {}", frame);
                    
                    // Verify trajectory was parabolic (gravity worked)
                    let max_y = history.iter().map(|p| p.y).fold(0.0, f64::max);
                    assert!(max_y <= 10.0, "Should not go above start");
                    
                    // Verify hit ground
                    assert!(spell.position.y <= 0.5, "Should be near ground");
                    
                    return;
                }
                SpellSignal::TransformTo(new_form) => {
                    // Transform and continue
                    spell.lua_object = new_form;
                }
            }
        }
        
        // Safety check
        if spell.energy <= 0.0 {
            println!("Spell fizzled at frame {}", frame);
            return;
        }
    }
    
    panic!("Spell never completed in 1000 frames");
}
```

**Verification:**
```bash
cargo test -p studio_spell test_fireball_full_lifecycle
```

---

#### 0D.2: Energy Depletion Test

**Test:**
```rust
#[test]
fn test_spell_fizzles_when_out_of_energy() {
    let lua = Lua::new();
    setup_spell_environment(&lua).unwrap();
    
    // Spell with expensive anti-gravity
    let spell_def: Table = lua.load(r#"
        local Fireball = require("spells/fireball")
        return Fireball:new({ 
            speed = 10,
            anti_gravity = true,  -- Costs 2 energy/sec
        })
    "#).eval().unwrap();
    
    let mut spell = SpellInstance {
        lua_object: spell_def,
        energy: 5.0,  // Only 5 energy
        position: Vec3::new(0.0, 10.0, 0.0),
        velocity: Vec3::new(10.0, 0.0, 0.0),
        time_alive: 0.0,
    };
    
    let dt = 0.1;
    let mut total_time = 0.0;
    
    for _ in 0..100 {
        let mut tape = CostTape::new();
        let _signals = tick_lua_spell(&lua, &mut spell, &mut tape, dt);
        
        let cost = tape.total_cost();
        if cost > spell.energy {
            // Fizzle!
            println!("Fizzled after {} seconds", total_time);
            assert!(total_time < 3.0, "Should fizzle within 3 seconds");
            assert!(total_time > 2.0, "Should last at least 2 seconds");
            return;
        }
        
        spell.energy -= cost;
        total_time += dt;
    }
    
    panic!("Spell never fizzled");
}
```

**Verification:**
```bash
cargo test -p studio_spell test_spell_fizzles
```

---

### Phase 0E: Visual Rendering

**Goal:** Only NOW do we render. All logic is already tested.

#### 0E.1: Spawn Emissive Voxel

**Test:** (Visual verification required)
```rust
// examples/p40_spell_visual.rs

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(DeferredRenderingPlugin)
        .add_plugins(SpellPlugin)
        .add_systems(Startup, setup_test_scene)
        .add_systems(Update, spawn_test_fireball)
        .run();
}

fn setup_test_scene(mut commands: Commands) {
    // Flat terrain
    // Camera at (0, 10, 20) looking at origin
    // Basic lighting
}

fn spawn_test_fireball(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer >= 3.0 {
        *timer = 0.0;
        
        // Spawn fireball with visual component
        commands.spawn(SpellBundle {
            spell: SpellObject { /* ... */ },
            visual: SpellVisual {
                color: Color::ORANGE,
                emission: 2.0,
            },
            transform: Transform::from_xyz(0.0, 5.0, 10.0),
        });
    }
}
```

**Verification:**
```bash
cargo run --example p40_spell_visual -- --record
# Creates video: screenshots/spell_visual.mp4
# Verify: Orange glowing voxel visible, moves across scene
```

---

#### 0E.2: Full Fireball with Rendering

**Test:** (Visual verification required)
```bash
cargo run --example p40_spell_fireball -- --rendered --record
# Video shows:
# - Fireball spawns
# - Arcs downward (gravity)
# - Hits terrain
# - Transforms to explosion (flash)
# - Disappears
```

---

## Summary: What Each Sub-Phase Establishes

| Sub-Phase | Establishes | Tests Without |
|-----------|-------------|---------------|
| 0A.1 | MLua works, can call Lua functions | Rendering, game world |
| 0A.2 | Custom require, sandboxing | Rendering, game world |
| 0A.3 | Lua object inheritance | Rendering, game world |
| 0A.4 | Rust↔Lua boundary | Rendering, game world |
| 0B.1 | Core Rust types | Lua, rendering, game world |
| 0B.2 | Lua spell execution | Rendering, game world |
| 0B.3 | Spell composition/inheritance | Rendering, game world |
| 0C.1 | Terrain queries from Lua | Rendering |
| 0C.2 | Ground sensor | Rendering |
| 0C.3 | Form transformation | Rendering |
| 0D.1 | Full spell lifecycle simulation | Rendering |
| 0D.2 | Energy depletion/fizzle | Rendering |
| 0E.1 | Visual rendering works | - |
| 0E.2 | Complete visual fireball | - |

---

## Complexification Path

After Phase 0, we have:
- Lua module system with sandboxed require
- Lua object inheritance for spell composition
- Rust↔Lua boundary for tick execution
- Terrain integration
- Ground sensors and form transformation
- Energy/cost system
- Visual rendering

**Phase 1 can then add:**
- More sensors: `ProximitySensor`, `EnergySensor`
- Target tracking: `find_nearest_target` API
- Homing behavior: `HomingFireball` that uses tracking

**Phase 2 can add:**
- Split transformation (one spell → multiple)
- Area effects
- Sound

**Phase 3 can add:**
- Package manager for spell distribution
- Hot-reload of spell definitions

---

## Open Questions for Research

1. **MLua require mechanics**
   - Does MLua support `package.loaders`?
   - Can we completely replace require?
   - Performance of custom loader?

2. **Lua object system**
   - Raw metatables vs class library?
   - How to handle `self` in method calls across Rust↔Lua boundary?

3. **Terrain integration**
   - How to efficiently query terrain from Lua?
   - Cache terrain height? Batch queries?

4. **Signal handling**
   - How does Lua return signals to Rust?
   - Table with signal data? Special return value?

---

## Files to Create (Phase 0A)

```
crates/studio_spell/
  Cargo.toml
  src/
    lib.rs
    lua/
      mod.rs
      vm.rs
      require.rs
      sandbox.rs
      bindings.rs

assets/scripts/
  utils.lua
  spells/
    base_projectile.lua
    fireball.lua
```

Begin with 0A.1 (basic MLua test) and proceed sequentially.
