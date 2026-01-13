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

## Narrative Structure: The Harmon Circle

### What It Is
Dan Harmon's Story Circle is a simplified version of Joseph Campbell's Hero's Journey. It's a tool to structure satisfying narratives. It ensures that characters (or players) actually *change* by the end of the experience.

It has 8 steps:
1.  **You:** A character is in a zone of comfort.
2.  **Need:** They want something.
3.  **Go:** They enter an unfamiliar situation.
4.  **Search:** They adapt to it.
5.  **Find:** They get what they wanted.
6.  **Take:** They pay a heavy price for it.
7.  **Return:** They return to their familiar situation.
8.  **Change:** Having changed.

In our context, we use this to structure **devlog videos**. The "character" is us, the developers. The "change" is showing how physics-based magic opens up gameplay that wasn't possible before.

---

## Physics-Based Magic (PBM)

The core premise: **Magic is physics. Mana is energy.**

This isn't a tweak. It changes everything about how magic games work. Spells have mass, velocity, energy costs. Mana doesn't regenerate—it's conserved, harvested, stored, transferred. Effects persist in the world.

The result: gameplay surfaces that don't exist in traditional magic games. Siege warfare. Minefields. Magical infrastructure. Territory control. Engineering, not just combat.

---

## Campaign Strategy

### Three Core Hypotheses

Every successful crowdfunding campaign proves a small set of hypotheses. Based on Pillars of Eternity and Star Citizen, ours are:

| Hypothesis | What we must prove | How videos prove it |
|------------|-------------------|---------------------|
| **Credible Vision** | We understand magic systems more deeply than anyone. We've thought about this obsessively. | Videos that make viewers say "I never thought about that" with specific, funny observations that reveal depth |
| **Unmet Fantasy** | There's real hunger for magic that makes sense. Current systems are broken in ways people feel but haven't articulated. | Videos that put words to a frustration viewers already have, creating "yes, exactly!" moments |
| **Tangible Progress** | We're not just talking. We're building. This is real. | Demos, prototypes, in-engine footage. Star Citizen showed a working prototype day one. |

The videos below form a **spanning set**—each advances a different hypothesis. They shouldn't repeat the same points.

### Pillars Lesson
Pillars succeeded by promising "the RPG you remember, made properly" to fans who felt abandoned. They didn't invent demand; they identified existing hunger and offered to satisfy it. Our equivalent: players who sense magic systems are shallow but haven't articulated why.

### Star Citizen Lesson
Star Citizen showed, didn't tell. Day-one prototype. In-engine footage. Specific feature breakdowns. Roberts spent a year building before asking for money. The "gamified" backing (buy ships, not t-shirts) made supporters feel like participants, not customers.

---

## Video Series

### Video 1: "Why Harry Potter Magic Makes No Sense"

**Hypothesis:** Credible Vision (we've thought about this deeply)

**The point:** Walk the viewer through the moment where beloved magic falls apart under scrutiny. The humor and specific observations ARE the content. By the end, they're thinking "huh, I never noticed that, but yeah, it's totally broken."

| Step | Content |
|------|---------|
| **You** | You loved Harry Potter. We all did. Hogwarts, wands, "it's leviOsa not levioSA." Magic felt real. |
| **Need** | But then you think about it for five minutes. |
| **Go** | Remember when the Death Eaters cast that giant skull in the sky? Super dramatic. Voldemort's mark. Everyone's terrified. |
| **Search** | But wait—they can just... do that? Project stuff into the sky? No cost? No limit? Why aren't teenagers projecting dicks onto the clouds 24/7? Why isn't the sky FULL of advertisements? |
| **Find** | And "leviosa"—they can levitate things. Okay, why does anyone walk? Why aren't wizards just floating everywhere? Why do they use brooms when they could leviosa themselves? |
| **Take** | Once you start pulling the thread, the whole thing unravels. Why is Dumbledore powerful? What does "powerful" even mean when anyone can do anything by saying Latin words? |
| **Return** | Harry Potter magic is a storytelling convenience. It does whatever the plot needs. There's no system underneath. |
| **Change** | And that's fine for books. But for a game? Where players push on systems? It falls apart instantly. We need rules. Real rules. |

---

### Video 2: "What If Dumbledore Was Actually Good At Something?"

**Hypothesis:** Credible Vision (continued)

**The point:** Flip from "Harry Potter is broken" to "what would it look like if it wasn't?" Introduce physics-based magic as the answer to the problems raised in Video 1.

| Step | Content |
|------|---------|
| **You** | So Dumbledore is supposedly the greatest wizard alive. But what does that mean? |
| **Need** | In Harry Potter, it means... he knows more spells? He says the words better? He has a fancier wand? None of that is satisfying. |
| **Go** | What if Dumbledore was more like Da Vinci? |
| **Search** | Da Vinci wasn't great because he knew more paint colors. He understood optics, anatomy, physics, engineering. He could combine principles in ways nobody else thought of. |
| **Find** | A physics-based Dumbledore would understand thermodynamics. Conservation of energy. Material properties. He'd chain effects together—not because he memorized a spell, but because he *understood* the underlying system. |
| **Take** | This means magic has limits. You can't just wave your wand and make a skull in the sky. You'd need to ionize atmosphere, project light, sustain the reaction. It would cost energy. A lot of energy. |
| **Return** | Suddenly "powerful wizard" means something. It means deep understanding. Creativity. Efficiency. Not just "knows more words." |
| **Change** | That's the magic system we're building. Spells aren't vocabulary. They're engineering. |

---

### Video 3: "The Mana Bar Is A Lie"

**Hypothesis:** Unmet Fantasy (articulating a frustration players feel)

**The point:** Take something players accept without question (mana bars) and reveal it as hollow. Create a "why did I never notice this?" moment.

| Step | Content |
|------|---------|
| **You** | Every RPG has a mana bar. Blue bar goes down when you cast spells. Blue bar goes up when you wait. This is how magic works. |
| **Need** | But what IS mana? Where does it come from when it "regenerates"? Where does it go when you "spend" it? |
| **Go** | The answer is: nowhere. It's not a thing. It's a UI element. It's a cooldown system with a blue skin. |
| **Search** | Think about it. If mana is energy, and you're creating fireballs, that energy has to come from somewhere. Thermodynamics. Conservation. You can't create energy from nothing. |
| **Find** | But in WoW, you sit down for 10 seconds and your mana refills. From where? The ground? The air? Your butt? |
| **Take** | Nobody asks because the answer is embarrassing. Mana isn't a resource. It's a game designer saying "you can only do this X times per minute." |
| **Return** | What if mana was real? Conserved, not regenerated. Harvested, stored, transferred. What if locations with more ambient energy were valuable? Worth fighting over? |
| **Change** | Mana stops being a number. It becomes logistics. And logistics is where interesting gameplay lives. |

---

### Video 4: "Why Every Magic Fight Feels The Same"

**Hypothesis:** Unmet Fantasy (articulating a frustration)

**The point:** Name the sameness that players feel in magic PvP. Then show what changes when spells are physical.

| Step | Content |
|------|---------|
| **You** | WoW arena. You queue up. You load in. You see the enemy team. You spam your rotation. Someone dies. Next match. |
| **Need** | Why does every fight feel identical? |
| **Go** | Because spells aren't physical. They're just damage numbers with animations. A fireball doesn't arc. It doesn't travel. It doesn't care about terrain. You press button, enemy takes damage. |
| **Search** | What if spells had travel time? Drop-off over distance? What if a wall actually blocked a fireball instead of just being decoration? |
| **Find** | Suddenly you can do things that don't exist in current games. Artillery—lobbing fireballs from a hilltop before the enemy even sees you. Minefields—stationary balls of energy waiting for someone to walk into them. Actual fortifications. |
| **Take** | This breaks "esports balance." You can't have fair 1v1s when terrain and preparation matter this much. |
| **Return** | But you get something else: fights with phases. Scouting. Bombardment. Approach. Breach. Fights that feel different based on where they happen. |
| **Change** | Magic combat stops being a rotation. It becomes tactics. |

---

### Video 5: "Wizards Should Be Engineers"

**Hypothesis:** Unmet Fantasy (showing a possibility nobody's explored)

**The point:** Expand what magic can do beyond combat. If magic is energy, you can do work. Real work.

| Step | Content |
|------|---------|
| **You** | In every fantasy game, wizards throw fireballs. That's it. That's the job. You're artillery. |
| **Need** | But if magic is energy, you can do more than explode things. You can move things. Heat things. Build things. |
| **Go** | Why don't wizards build roads? Power machines? Lift stones? In a world with magic, why does infrastructure look medieval? |
| **Search** | Because game designers only think about combat. Spells are "damage" or "heal" or "buff." Nobody asks "what's the energy cost of lifting a ton of stone?" |
| **Find** | If you ask that question, wizards become engineers. A spell that channels heat into a forge. A ward that stores kinetic energy from impacts and releases it on command. Automated defenses. Rube Goldberg spellcraft. |
| **Take** | This requires a robust physics simulation. It's harder to build. It's harder to balance. Most studios won't bother. |
| **Return** | But the payoff: magic that's creative, not just destructive. Players who build, not just fight. |
| **Change** | Wizards stop being artillery. They become inventors. |

---

### Video 6: "Here's What We're Actually Building"

**Hypothesis:** Tangible Progress (show, don't tell)

**The point:** Star Citizen showed a prototype on day one. This video shows our system working. Concrete footage, not concept art.

| Step | Content |
|------|---------|
| **You** | We've talked a lot about what magic *should* be. Now let's show you what we've built. |
| **Need** | [Show the engine. Show the world. Two moons, voxel terrain, the aesthetic.] |
| **Go** | [Show spell creation. Player describes intent, AI generates spell logic.] |
| **Search** | [Show iteration. "Make it arc." "Make it split at apex." Each change produces different behavior.] |
| **Find** | [Show emergent combo. Poison converts flesh to wood. Fire ignites wood. 3x damage. We didn't code this interaction—it emerged from material properties.] |
| **Take** | [Show something that doesn't work yet. Be honest about what's hard.] |
| **Return** | [Show the roadmap. What's next.] |
| **Change** | This is real. It's not a pitch deck. We're building it. If you want this to exist, you can help. |

---

### Video 7: "Why We're Asking You"

**Hypothesis:** All three (the ask)

**The point:** The direct funding appeal. Explain why crowdfunding, what backers get, what the deal is.

| Step | Content |
|------|---------|
| **You** | We could pitch this to publishers. |
| **Need** | But we know how that conversation goes. "Can you add cooldowns?" "Can you simplify the mana system?" "Can you make it more like [game that sold well]?" |
| **Go** | Publishers fund what's proven. Physics-based magic isn't proven. It's weird. It's niche. It might not work. |
| **Search** | So we're not asking publishers. We're asking you. |
| **Find** | If the videos we've made resonated—if you watched them and thought "I want that game"—then you're the answer to whether this gets built. |
| **Take** | We're asking for trust and patience. Development takes time. We'll be transparent, but we won't promise dates we can't keep. |
| **Return** | What we promise: the game we've described is the game we're making. No watering down. No "accessible version." |
| **Change** | You're not pre-ordering a product. You're funding a vision. If it resonates, we'd like your help making it real. |

---

### Video 8+: Progress Updates (Template)

**Hypothesis:** Tangible Progress (ongoing)

**The point:** Regular updates that show real work, admit real problems, maintain trust.

| Step | Content |
|------|---------|
| **You** | Here's where we were last month. |
| **Need** | Here's what we tried to do. |
| **Go** | Here's what actually happened. [Footage of work in progress.] |
| **Search** | Here's what broke. Here's what we learned. |
| **Find** | Here's what's working now. [Demo of new feature.] |
| **Take** | Here's what's still hard. Here's what we're worried about. |
| **Return** | Here's the plan for next month. |
| **Change** | We're building in public. You can watch. You can tell us if we're going off track. |

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
