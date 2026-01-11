# Player Fantasy Demo

This is a brainstorming document. Nothing here is definitive. The goal is to explore what makes this game compelling and what we need to demonstrate that.

---

## The Fantasy

A spell system so deep that you can't find its edges.

You have access to a combinatorial magic system where spells are assembled from modules, behaviors stack, elements interact, and the outcomes scale with your creativity. The system is powerful enough that after months of play, you're still discovering new combinations. You look at someone else's spell and think "I didn't know you could do that."

The boundaries are unclear. That's the point.

### What Makes This Compelling

1. **Unbounded exploration.** The spell system has more depth than any single player will exhaust. There's always another combination to try, another interaction to discover.

2. **Personal investment.** The spells you create required your ideas, your iteration, your testing. They belong to you in a way that preset abilities never could.

3. **Emergent complexity.** Simple modules combine into complex behaviors. A fireball is simple. A fireball that arcs, splits at apex, deploys poison to transform targets to wood, then ignites them with delayed fire bolts is built from simple pieces.

4. **Collaboration with AI.** You describe intent; AI handles syntax. This removes the barrier of "I'd need to learn to code" while preserving the creative work of designing the spell's behavior.

### What This Is Not

This is not about "you don't have to write code." Plenty of games have visual scripting or preset abilities.

This is about access to a system complex enough that your creativity is the limiting factor. The system will support whatever you can imagine; the question is whether you can imagine it.

---

## The World

Dark 80s fantasy. Two moons (purple and orange) provide the only light. No sun. Perpetual twilight.

Voxel creatures roam the landscape. Emissive magic glows against the darkness. The aesthetic sits somewhere between Labyrinth, Dark Crystal, and early Final Fantasy concept art.

The world exists to be a canvas for spell experimentation. Creatures to target. Terrain to interact with. Environmental elements (water, metal, vegetation) that spells can exploit.

---

## Demo Narrative: Spell Progression

The demo shows escalating complexity. Each step builds on the last. The viewer should think: "Wait, you can do that? What else can you do?"

### Starting Point: Basic Fireball

Medium-range projectile. Travels forward. Explodes on impact. Pure fire.

This is the baseline. Everyone understands a fireball. It's satisfying but unremarkable.

```lua
return Fireball:new({
    speed = 20,
    gravity = 0.5,
    element = "fire",
    on_hit = Explosion:new({ radius = 3, damage = 50 })
})
```

### Modification 1: Artillery Trajectory

Player wants to hit distant targets with more force.

Change: Launch at 60 degrees. Gravity creates an arc. Impact velocity scales the explosion.

Tradeoff: Harder to aim. You're predicting where the target will be.

```lua
local ArtilleryFireball = Fireball:extend({
    launch_angle = 60,
    
    on_hit = function(self, ctx)
        local impact_energy = self.velocity:length()
        return Explosion:new({ 
            radius = 3 + impact_energy * 0.1,
            damage = 50 + impact_energy * 2
        })
    end
})
```

Visual: Fireball arcs into the sky, hangs at apex, plummets. Bigger explosion.

### Modification 2: Apex Split

Player wants area coverage.

Change: At the highest point of the arc (velocity.y crosses zero), split into four projectiles. Each child gets 25% of remaining energy.

```lua
tick = function(self, ctx)
    ArtilleryFireball.tick(self, ctx)
    
    if self.prev_velocity_y > 0 and ctx.velocity.y <= 0 then
        ctx:signal_split({
            count = 4,
            spread_angle = 15,
            child = Fireball:new({ gravity = self.gravity })
        })
    end
    self.prev_velocity_y = ctx.velocity.y
end
```

Visual: Single fireball arcs up. At apex, pops into four. They rain down across an area.

### Modification 3: Elemental Combo

Player wants synergy between elements.

Change: Two of the split projectiles are poison (transforms organic targets to wood). Two are fire (ignites wood for bonus damage). Poison falls faster so it lands first.

```lua
ctx:signal_split({
    children = {
        { projectile = PoisonBolt:new({ 
            gravity = self.gravity * 1.5,  -- Falls faster
            on_hit = OrganicTransform:new({ target_material = "wood" })
        }), angle = -10 },
        { projectile = PoisonBolt:new({ ... }), angle = 10 },
        { projectile = Fireball:new({ 
            on_hit = Explosion:new({ bonus_vs_material = { wood = 3.0 } })
        }), angle = -20 },
        { projectile = Fireball:new({ ... }), angle = 20 },
    }
})
```

Visual sequence:
1. Artillery arc upward
2. Apex split into four
3. Two green bolts (poison) fall faster, hit first
4. Targets transform: flesh becomes wood
5. Two orange bolts (fire) land on wooden targets
6. Wood ignites. 3x damage.

This is the payoff moment. A single spell with sequenced elemental interactions.

### The Signature

Each spell gets a procedurally generated insignia based on its composition. Visual identity for your creation.

Options for signature generation:
- Shape from spell type (projectile, area, beam)
- Color from primary element
- Complexity from module count
- Unique seed from code hash

Open question: What aesthetic? Runic? Geometric? Organic?

---

## Demo Storyboard (Draft)

Target: 90-120 seconds. Platform: TikTok/YouTube Shorts viable, but also works longer form.

### Opening: The World (10-15 sec)
- Dark landscape. Two moons. Voxel creatures in distance.
- Establish the aesthetic. No text yet, or minimal.

### The Hook (10 sec)
- Player faces a problem (enemies, obstacle).
- Quick cut to chat interface. Player types something.
- Spell appears. Cast. Effect.
- This should happen fast. Establish the loop: describe, create, cast.

### The Build (30-40 sec)
- Iteration montage. Quick cuts.
- "Make it arc higher" / artillery fires
- "Split at the top" / cluster rains down
- "Mix in poison and fire" / elemental combo
- Each iteration is 5-8 seconds. Show the conversation briefly, then the result.
- Spells get visually more impressive.

### The Payoff (15-20 sec)
- Full elemental combo in slow motion.
- Split. Poison lands. Transformation visible. Fire lands. Ignition.
- Let it breathe. This is the moment.

### The Close (10 sec)
- Signature reveal (optional).
- Logo. Simple call to action or just end.

### Alternative Structures

**Option A: Single spell focus.** Follow one spell from basic to complex. More narrative, less montage.

**Option B: Multiple spells.** Show three different players with three different spell styles. Demonstrates breadth.

**Option C: Problem/solution.** Present a challenge (tough enemy, environmental puzzle), show spell creation as the solution.

Open question: Which structure best sells the fantasy?

---

## System Requirements

### For Demo (Minimum)

| System | What It Does | Status |
|--------|--------------|--------|
| Spell Module System | Composable behaviors, energy tracking | In design |
| Lua API | Spell definition, require/extend | In design (phase 0) |
| MCP Server | AI reads/writes/executes spells | Not started |
| Projectile Physics | Flight, gravity, collision | Partial (VoxelFragment) |
| Split Mechanic | One projectile becomes many | In design |
| Elemental Interactions | Material transforms, damage bonuses | Not started |
| Visual Effects | Explosions, trails, transformations | Not started |
| Video Capture | Record demo footage | Exists |

### For Demo (Nice to Have)

| System | What It Does |
|--------|--------------|
| Signature Generator | Procedural spell insignia |
| More Elements | Ice, lightning, water interactions |
| Sound Design | Audio feedback for spells |

### Post-Demo

| System | What It Does |
|--------|--------------|
| Creature Sculpting | Define body shapes |
| Creature Physics/Animation | Movement, procedural animation |
| Spell Package Manager | Import/export/share spells |
| Multiplayer | See others' spells in action |

---

## MCP Server Design

The game runs as an MCP server. An external AI connects and has access to the spell system.

### AI Capabilities

**Read:**
- Spell source files
- API documentation
- Available modules and their parameters
- Game state (what's in the test environment)

**Write:**
- Create new spell files
- Modify existing spells
- Hot-reload changes

**Execute:**
- Cast spells
- Spawn targets
- Reset test environment

### Tool Sketch

```
list_spells          - what spells exist
read_spell(name)     - get Lua source
create_spell(name, source)
modify_spell(name, new_source)
cast_spell(name, target?)
get_api_docs         - spell API reference
list_modules         - available modules to import
```

### Example Flow

Player: "I want something that freezes enemies"

AI checks available modules, sees `stasis`, `ice_damage`, `area_effect`. Generates:

```lua
return Projectile:new({
    speed = 25,
    element = "ice",
    on_hit = Stasis:new({ duration = 3.0, radius = 0 })
})
```

Creates the spell. Player tests. Asks for area effect. AI modifies radius. Iterate.

The AI handles syntax. The player handles intent and iteration.

---

## Elemental Interaction Framework

This is where depth comes from. Simple elements combine into complex outcomes.

### Materials

| Material | Notes |
|----------|-------|
| Flesh | Default organic. Can be transformed. |
| Wood | Flammable. Created by organic transform. |
| Stone | Resistant. Can shatter. |
| Metal | Conducts lightning. |
| Ice | Melts to water. Shatters on impact. |
| Water | Conducts lightning. Extinguishes fire. |

### Interactions

| A | B | Result |
|---|---|--------|
| Fire | Wood | Ignition, spreads |
| Fire | Ice | Melt, creates water |
| Fire | Water | Steam, obscures |
| Poison | Flesh | Transform to wood |
| Lightning | Water | Chain to wet targets |
| Lightning | Metal | Bonus damage |
| Ice | Water | Freeze surface |

### Combo Examples

**Burn Chain:** Poison (flesh to wood) then Fire (ignites wood). Single target massive damage.

**Conductor:** Water splash (wets area) then Lightning (chains through wet). Crowd control.

**Shatter:** Ice (freezes, creates shell) then Impact (shatters). Burst damage.

These are the interactions we know about. The system should support discovering new ones through experimentation.

---

## Creature System (Secondary Pillar)

Not required for initial demo, but part of the full vision.

### Concept

1. **Sculpt:** Simple 3D drawing defines body shape. Drag limbs, pinch joints.

2. **Physicalize:** System generates physics model from shape. Mass, joints, collision.

3. **Animate:** AI generates locomotion. Gait from body type, procedural animation from physics.

### Example

Player sculpts: bulbous body, eight legs, pincers.
System infers: 8-legged crawler.
AI generates: alternating gait, climbing behavior, pincer attack.
Player iterates: "Make it jump." AI adds leap behavior.

### Connection to Spells

Creatures serve as:
- Targets for spell testing
- Hosts for spell-like abilities (breath weapons)
- Entities that react to elemental effects
- Summons from spells

---

## Development Phases (Rough)

This is not a schedule. These are the chunks of work.

**Phase 1: Fireball flies**
- Projectile physics
- Ground detection
- Basic explosion
- Lua spell definition working

**Phase 2: AI writes spells**
- MCP server running
- AI can create/modify spells
- Test environment for iteration

**Phase 3: Complexity**
- Arc trajectory
- Apex detection, split mechanic
- Multiple projectile types

**Phase 4: Elements**
- Material system
- Elemental interactions
- Visual effects for transformations

**Phase 5: Demo capture**
- Polish
- Environment setup
- Record footage

---

## Open Questions

- AI provider for MCP: Claude, GPT, local model?
- Signature aesthetic: runic, geometric, organic?
- Demo platform: TikTok, YouTube, both?
- Creatures in initial demo or later?
- Show spell sharing/multiplayer?

---

## Related Documents

- Spell system design: `docs/plans/spell_system.md`
- Phase 0 details: `docs/plans/spell_system_phase0.md`
- Visual fidelity: `docs/plans/visual_fidelity_improvements.md`
- Physics: `docs/physics/ARCHITECTURE.md`
