# Player Fantasy Demo: AI-Powered Spell Creation

## Core Thesis

In a dark 80s fantasy world lit only by two moons, players don't just cast spells—they *invent* them. With AI assistance, anyone can become a spell architect, designing increasingly complex magical systems without writing a single line of code themselves.

**The pitch in one sentence:** "Holy shit, I can basically be Harry Potter and invent my own spells."

---

## The World

### Setting
- **Aesthetic:** Dark 80s fantasy—think Labyrinth meets Dark Crystal meets early Final Fantasy
- **Lighting:** Two moons (purple and orange) are the only light sources. No sun. Perpetual night.
- **Inhabitants:** Weird voxel creatures roam the land—procedurally animated, AI-shaped beings
- **Atmosphere:** Mysterious, dangerous, and deeply personal. Your spells and creatures are *yours*.

### Visual Identity
- Voxel-based world with emissive magic
- Glowing spells contrast against the dark landscape
- Creatures with bioluminescent features
- Everything feels handcrafted yet alive

---

## Core Player Fantasy

### Two Pillars of Creation

**1. Spell Design**
- Players design spells through conversation with AI
- Spells are modular: packages of behaviors that combine
- No coding required—AI writes Lua, player iterates on ideas
- Deep ownership: your spell is *yours*, tuned through experimentation

**2. Creature Design**
- Simple 3D sculpting to define body shape
- Physics system handles movement
- AI generates the "brain" for locomotion
- Procedural animation makes them feel alive

### The AI Integration

The game runs as an **MCP server** that an external AI can connect to. The AI:
- Understands the Lua spell API and game context
- Can read existing spells and modify them
- Generates new spell code based on player descriptions
- Explains what spells do and suggests improvements

**Player workflow:**
1. Describe what you want: "I want a fireball that splits into smaller fireballs"
2. AI generates the spell code
3. Test it in-game
4. Iterate: "Make it split at the highest point of the arc"
5. AI refines
6. Repeat until satisfied

This creates **deep ownership**—the spell required your ideas, your iteration, your tuning. It's not a preset; it's *yours*.

---

## Spell Progression Narrative

The demo tells a story through escalating spell complexity. Each step shows what's possible.

### Act 1: The Basic Fireball

**What it is:**
- Medium-range projectile
- Travels in a straight line (or slight arc)
- Explodes on impact
- Pure fire mana

**Player prompt:** "Create a basic fireball spell"

**What AI generates:**
```lua
local Fireball = require("spells/base_projectile")
local Explosion = require("spells/explosion")

return Fireball:new({
    speed = 20,
    gravity = 0.5,
    element = "fire",
    on_hit = Explosion:new({ radius = 3, damage = 50 })
})
```

**Visual:** Orange glowing orb, slight trail, blooms on impact.

**Demo moment:** Player casts, fireball flies, explosion. Simple. Satisfying. "Okay, I made a fireball. Cool."

---

### Act 2: The Artillery Strike

**Player realization:** "What if I could hit things further away? Or harder?"

**Player prompt:** "Make it an artillery strike—shoots up high, then comes down with more force"

**What changes:**
- Initial trajectory points upward at 60°
- Gravity pulls it into an arc
- Gains energy as it falls (potential → kinetic)
- Bigger explosion on impact

**What AI generates:**
```lua
local ArtilleryFireball = Fireball:extend({
    launch_angle = 60,  -- degrees from horizontal
    
    on_launch = function(self, ctx)
        -- Convert horizontal aim to upward arc
        local aim = ctx.aim_direction
        self.velocity = rotate_toward_vertical(aim, self.launch_angle) * self.speed
    end,
    
    on_hit = function(self, ctx)
        -- Explosion scales with impact velocity
        local impact_energy = self.velocity:length()
        return Explosion:new({ 
            radius = 3 + impact_energy * 0.1,
            damage = 50 + impact_energy * 2
        })
    end
})
```

**Tradeoff introduced:** Landing precision is harder. You're aiming at where the target *will be*.

**Demo moment:** Player aims at distant enemy. Fireball arcs into the sky, hangs at apex, plummets down. Massive explosion. "Whoa, that's way more satisfying."

---

### Act 3: The Split

**Player realization:** "What if it split into multiple projectiles?"

**Player prompt:** "At the highest point of the arc, split into four smaller fireballs"

**What changes:**
- Spell detects apex (velocity.y crosses zero)
- Splits into 4 child projectiles
- Each child has 25% of remaining energy
- Children spread in a pattern

**What AI generates:**
```lua
local SplitArtillery = ArtilleryFireball:extend({
    split_count = 4,
    split_spread = 15,  -- degrees
    
    tick = function(self, ctx)
        ArtilleryFireball.tick(self, ctx)
        
        -- Detect apex
        if self.prev_velocity_y > 0 and ctx.velocity.y <= 0 then
            ctx:signal_split({
                count = self.split_count,
                spread_angle = self.split_spread,
                child = Fireball:new({
                    speed = ctx.velocity:length(),
                    gravity = self.gravity,
                    on_hit = self.on_hit
                })
            })
        end
        self.prev_velocity_y = ctx.velocity.y
    end
})
```

**Demo moment:** Fireball arcs up, reaches apex, *pop*—four smaller fireballs spread out and rain down. Area denial. "Now we're talking."

---

### Act 4: The Elemental Combo

**Player realization:** "What if different projectiles did different things? What if they *synergized*?"

**Player prompt:** "Two of the split projectiles should be poison that converts targets to wood. The other two are fire. Poison lands first, then fire ignites the wood."

**What changes:**
- Split produces mixed children (2 poison, 2 fire)
- Poison children have faster descent (heavier? different arc?)
- Poison effect: transforms organic targets into wood material
- Fire effect: ignites wood for bonus damage
- Sequencing matters: poison must land first

**What AI generates:**
```lua
local ElementalCombo = SplitArtillery:extend({
    tick = function(self, ctx)
        ArtilleryFireball.tick(self, ctx)
        
        if self.prev_velocity_y > 0 and ctx.velocity.y <= 0 then
            -- Create mixed payload
            local poison_bolt = PoisonBolt:new({
                gravity = self.gravity * 1.5,  -- Falls faster
                on_hit = OrganicTransform:new({ 
                    target_material = "wood",
                    duration = 3.0
                })
            })
            
            local fire_bolt = Fireball:new({
                gravity = self.gravity,  -- Normal fall speed
                on_hit = Explosion:new({
                    radius = 2,
                    damage = 30,
                    bonus_vs_material = { wood = 3.0 }  -- 3x damage to wood
                })
            })
            
            ctx:signal_split({
                children = {
                    { projectile = poison_bolt, angle = -10 },
                    { projectile = poison_bolt, angle = 10 },
                    { projectile = fire_bolt, angle = -20 },
                    { projectile = fire_bolt, angle = 20 },
                }
            })
        end
        self.prev_velocity_y = ctx.velocity.y
    end
})
```

**Demo moment:** 
1. Artillery fireball arcs up
2. At apex, splits into four
3. Two green bolts (poison) fall faster, hit first
4. Targets transform—flesh becomes wood
5. Two orange bolts (fire) land on the now-wooden targets
6. *WHOOSH*—wood ignites, massive fire damage

**Player reaction:** "Holy shit. I just invented a spell that turns people into trees and then burns them."

---

### Act 5: The Signature

Every spell gets a **cast signature**—a procedurally generated insignia that represents its unique identity.

**Components of the signature:**
- Base shape from spell type (projectile, area, utility)
- Color from primary element
- Complexity from module count
- Unique seed from spell code hash

**Demo moment:** After creating the elemental combo, the game reveals its signature—a complex symbol combining fire and poison motifs. "This is YOUR spell. No one else has this exact combination."

---

## Demo Storyboard

### Scene 1: The World (15 seconds)
- Slow pan across dark voxel landscape
- Two moons visible in sky
- Strange creatures moving in distance
- Text: "A world lit only by two moons..."

### Scene 2: The Problem (10 seconds)
- Player character faces enemies
- No weapons, no obvious solution
- Text: "...where you must create your own power."

### Scene 3: The AI Conversation (20 seconds)
- Split screen: game world + chat interface
- Player types: "Create a fireball spell"
- AI responds with explanation
- Spell code appears (briefly visible)
- Text: "Describe what you want. AI builds it."

### Scene 4: First Cast (10 seconds)
- Player casts basic fireball
- Simple but satisfying explosion
- Text: "Start simple."

### Scene 5: Iteration Montage (30 seconds)
- Quick cuts of player refining:
  - "Make it arc higher" → artillery variant
  - "Split at the top" → cluster variant
  - "Mix in poison" → elemental combo
- Each iteration shows the AI conversation briefly
- Spells get visually more complex
- Text: "Iterate. Experiment. Perfect."

### Scene 6: The Payoff (20 seconds)
- Full elemental combo in action
- Slow-motion: split, poison lands, transformation, fire lands, ignition
- Massive damage
- Text: "Create something no one has ever seen."

### Scene 7: The Signature (10 seconds)
- Spell signature reveals
- Zoom into the intricate symbol
- Text: "Your spell. Your signature. Your magic."

### Scene 8: Call to Action (5 seconds)
- Game logo
- Text: "Become a spell architect."

**Total runtime:** ~2 minutes

---

## System Requirements

### Required for Demo

| System | Purpose | Status |
|--------|---------|--------|
| **Spell Module System** | Composable spell behaviors | In design (spell_system.md) |
| **Lua API** | Spell definition language | In design (spell_system_phase0.md) |
| **MCP Server** | AI connection to game | Not started |
| **Projectile Physics** | Fireball flight, gravity, arcs | Partially exists (VoxelFragment) |
| **Ground Detection** | Impact triggering | In design |
| **Explosion Effect** | Visual + damage | Not started |
| **Split Mechanic** | One spell → many | In design |
| **Elemental System** | Material types, interactions | Not started |
| **Signature Generator** | Procedural spell insignia | Not started |
| **Video Capture** | Demo recording | Exists (screenshot system) |

### Required for Full Vision (Post-Demo)

| System | Purpose |
|--------|---------|
| Creature Sculpting | 3D body definition |
| Creature Physics | Movement and locomotion |
| Creature AI | Procedural animation brain |
| Spell Package Manager | Import/export spells |
| Multiplayer | Share spells with others |
| Spell Marketplace | Trade/sell spell designs |

---

## MCP Server Design

### What the AI Needs Access To

**Read access:**
- Current spell definitions (Lua source)
- Spell API documentation
- Available modules and their parameters
- Game state (for context-aware suggestions)
- Player's spell history

**Write access:**
- Create new spell files
- Modify existing spell files
- Trigger spell reload

**Execute access:**
- Cast spells for testing
- Spawn test targets
- Reset test environment

### MCP Tools to Implement

```typescript
// Example MCP tool definitions

tools: [
  {
    name: "list_spells",
    description: "List all spells the player has created",
    parameters: {}
  },
  {
    name: "read_spell",
    description: "Read the Lua source of a spell",
    parameters: {
      spell_name: { type: "string" }
    }
  },
  {
    name: "create_spell",
    description: "Create a new spell from Lua source",
    parameters: {
      name: { type: "string" },
      source: { type: "string" },
      description: { type: "string" }
    }
  },
  {
    name: "modify_spell",
    description: "Modify an existing spell",
    parameters: {
      spell_name: { type: "string" },
      new_source: { type: "string" }
    }
  },
  {
    name: "cast_spell",
    description: "Cast a spell in the test environment",
    parameters: {
      spell_name: { type: "string" },
      target_position: { type: "object" }
    }
  },
  {
    name: "get_spell_api_docs",
    description: "Get documentation for the spell API",
    parameters: {}
  },
  {
    name: "list_available_modules",
    description: "List spell modules that can be imported",
    parameters: {}
  }
]
```

### Conversation Example

**Player:** "I want a spell that freezes enemies in place"

**AI (using tools):**
1. Calls `list_available_modules` → sees `stasis`, `ice_damage`, `area_effect`
2. Calls `get_spell_api_docs` → understands how to compose them
3. Generates Lua:
```lua
local Projectile = require("spells/base_projectile")
local Stasis = require("effects/stasis")

return Projectile:new({
    speed = 25,
    element = "ice",
    on_hit = Stasis:new({
        duration = 3.0,
        radius = 0,  -- single target
        visual = "ice_encase"
    })
})
```
4. Calls `create_spell` with the source
5. Responds: "I've created an ice bolt that freezes a single target for 3 seconds. Want me to add an area effect so it freezes nearby enemies too?"

**Player:** "Yeah, make it freeze everyone within 5 meters"

**AI:**
1. Calls `modify_spell` with updated radius
2. Responds: "Done! Now it freezes all enemies within 5 meters of the impact point. The duration is still 3 seconds. Want to test it?"

---

## Elemental Interaction Framework

### Material Types

| Material | Properties |
|----------|------------|
| Flesh | Default organic, takes poison damage |
| Wood | Flammable, created by organic transform |
| Stone | Resistant, can be shattered |
| Metal | Conductive, reflects some magic |
| Ice | Melts to water, shatters on impact |
| Water | Conducts lightning, extinguishes fire |

### Element Interactions

| Element A | Element B | Result |
|-----------|-----------|--------|
| Fire | Wood | Ignition (bonus damage, spreads) |
| Fire | Ice | Melt (removes ice, creates water) |
| Fire | Water | Steam (area denial, obscures) |
| Poison | Flesh | Transform to Wood |
| Lightning | Water | Conduct (chains to wet targets) |
| Lightning | Metal | Amplify (bonus damage) |
| Ice | Water | Freeze (creates ice surface) |

### Example Combo Chains

**The Burn Chain:**
1. Poison bolt → transforms flesh to wood
2. Fire bolt → ignites wood (3x damage)
3. Result: Massive single-target damage

**The Conductor:**
1. Water splash → wets area
2. Lightning bolt → chains through wet targets
3. Result: Multi-target crowd control

**The Shatter:**
1. Ice bolt → freezes target (creates ice shell)
2. Impact bolt → shatters ice
3. Result: Massive burst damage to frozen targets

---

## Creature System (Secondary Pillar)

### Creation Flow

1. **Sculpt:** Simple 3D drawing to define body shape
   - Drag to create limbs
   - Pinch to define joints
   - Paint to add features

2. **Physicalize:** System generates physics model
   - Mass distribution from volume
   - Joint constraints from shape
   - Collision mesh from surface

3. **Animate:** AI generates movement brain
   - Locomotion style from body type
   - Procedural animation from physics
   - Behavior patterns from player hints

### Example: Creating a Spider-Thing

1. Player sculpts: bulbous body, eight legs, two pincers
2. System infers: "This is an 8-legged crawler with melee attacks"
3. AI generates:
   - Walking gait (alternating leg groups)
   - Climbing behavior (wall adhesion)
   - Attack pattern (pincer strike)
4. Player tests, iterates: "Make it jump"
5. AI adds: leap behavior, landing recovery

### Connection to Spells

Creatures can:
- Be targets for spell testing
- Have spell-like abilities (breath weapons, projectiles)
- React to elemental effects (burn when hit by fire)
- Be summoned by spells

---

## Success Metrics for Demo

### Emotional Response
- "Holy shit" moment when elemental combo lands
- Sense of ownership over created spell
- Desire to create more, experiment further

### Technical Proof Points
- AI successfully generates working spell code
- Spell composition creates emergent gameplay
- Visual effects match the power fantasy

### Funding Viability
- Demo shows clear product differentiation
- AI integration is novel and compelling
- Path to full game is believable

---

## Development Priority

### Phase 1: Minimum Viable Fireball (Weeks 1-2)
- Basic projectile physics
- Simple explosion effect
- Ground detection
- Lua spell definition

### Phase 2: AI Integration (Weeks 3-4)
- MCP server setup
- Basic spell creation via AI
- Test environment for iteration

### Phase 3: Artillery + Split (Weeks 5-6)
- Arc trajectory
- Apex detection
- Split mechanic
- Multiple projectile types

### Phase 4: Elemental Combo (Weeks 7-8)
- Material system
- Element interactions
- Poison → Wood → Fire chain
- Visual effects for transformations

### Phase 5: Polish + Capture (Weeks 9-10)
- Signature generator
- Demo environment
- Video capture
- Edit and publish

**Total estimated time to demo: 10 weeks**

---

## Open Questions

1. **AI Provider:** Which AI service for MCP? Claude? GPT? Local model?
2. **Signature Style:** What aesthetic for spell insignias?
3. **Demo Platform:** TikTok? YouTube? Both?
4. **Creature Priority:** Include in initial demo or save for later?
5. **Multiplayer Tease:** Show spell sharing in demo?

---

## References

- Spell system design: `docs/plans/spell_system.md`
- Phase 0 details: `docs/plans/spell_system_phase0.md`
- Visual fidelity: `docs/plans/visual_fidelity_improvements.md`
- Physics system: `docs/physics/ARCHITECTURE.md`
