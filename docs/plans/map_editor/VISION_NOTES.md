# Map Editor Vision Notes (Brain Dump)

*Raw notes capturing the full vision. Will be organized into structured documents.*

---

## 1. Top-Level Vision

The map editor is a key part of the experience, but it's part of a larger system.

**Three pillars of the system:**

1. **Physics-based magic (mana system)** - Energy constraints govern everything
2. **AI-assisted coding/workflows** - Voice-driven, Lua scripting, shareable code
3. **Databases/libraries** - Materials, terrains, creatures, mana distribution, spells

---

## 2. Physics-Based Magic (Mana = Energy)

Core concept: **Mana equals energy. Doing anything in the world costs energy.**

Key principles:
- There is a certain amount of energy allowed per voxel
- Larger actions/objects require more energy and are physically larger/heavier
- This physics-based magic is the **novel foundation of everything**

### Power/Energy Transfer
- "Power lines" concept: moving energy around creates constraints and logistics
- Energy doesn't just exist - it flows, can be stored, transferred, consumed

### World = Physics + Alchemy
All actions operate within energy constraints:
- **Sensing** - detecting things costs energy
- **Transmutation** - changing voxel types costs energy
- **Terraforming** - moving/shaping terrain costs energy
- More powerful effects = more energy required

### Tuning
- Energy constraints are tricky to balance
- Ongoing tuning required
- This is core game design work

---

## 3. AI Features and Coding Model

### Voice-Driven AI Coding
- Complex behaviors built through top-level voice-driven interaction
- "I can do coding through AI"
- Sophisticated libraries exposed via natural language

### Architecture Layers
```
┌─────────────────────────────────────┐
│  Voice / Natural Language (User)    │
├─────────────────────────────────────┤
│  AI Assistant (interprets intent)   │
├─────────────────────────────────────┤
│  Lua Scripts (behavior layer)       │
│  - Hot reloadable                   │
│  - Shareable                        │
│  - Package ecosystem                │
├─────────────────────────────────────┤
│  Rust Engine (low-level APIs)       │
│  - NOT changing                     │
│  - Physics, rendering, core systems │
└─────────────────────────────────────┘
```

### Code Sharing Ecosystem
- Share code with others
- Build a library ecosystem
- **Lua package manager** to import others' work
- Similar to Node.js ecosystem model
- Build on top of community contributions

---

## 4. Databases and Materials

### Multiple "Databases of Stuff"
The map editor supports collections of:
- Voxel materials (palettes)
- Terrain generators
- Creature definitions
- Mana distributions
- Spell definitions

### Voxel Materials (PBR is Critical)
- Terrain's lowest-level concept: voxel materials
- Akin to color map palette (like Markov Jr.)
- Allow editing of materials in the voxel world
- **Allow uploading custom materials** (textures, etc.)
- Any type of voxels can exist in the world

---

## 5. Generators and Models

### Markov Jr. Integration
Terrain generators that:
- Take the palette (what voxels exist)
- Take rules (how they combine)
- Build terrains/biomes/world features

### Example: Complex World Generation
Build worlds with:
- Rivers
- Tundra on both sides
- Cultural factions (frozen people vs. river people who hate each other)

### Layered Concept Applies To Everything
Same pattern repeats:
- **Terrains** + their palettes → generated landscapes
- **Creatures** + their definitions → populated worlds
- **Mana distribution** + rules → magic-rich or magic-scarce regions

---

## 6. Mana Distribution as World Parameter

Worlds can vary in mana availability:
- **Mana-rich worlds** - abundant magic, powerful spells possible
- **Mana-scarce worlds** - harder gameplay, fewer spells, tougher survival

This is a fundamental world parameter that changes gameplay dramatically.

---

## 7. Spell System

### Same Systems, Different Application
- Map editing uses: AI + Lua + databases
- Spell creation uses: AI + Lua + databases
- **Unified approach**

### Creating Spells
- Talk to AI assistant to create complex spells
- Lua APIs exposed for dynamic spell definitions
- Build sophisticated behaviors through conversation

### Energy Accounting (Core Constraint)
Spells must track mana (energy) costs in a physically based way:

**Example: Launching a Rocket**
- Rocket requires fuel
- Fuel has weight
- Heavy/big voxels take more energy to move
- All physically consistent

**The physics grounds the magic.** It's not arbitrary - it follows rules.

---

## 8. Connections Between Systems

```
                    ┌─────────────────┐
                    │   AI Assistant  │
                    └────────┬────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
         v                   v                   v
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│   Map Editor    │ │  Spell Creator  │ │ Creature Editor │
│                 │ │                 │ │                 │
│ - Materials     │ │ - Effects       │ │ - Behaviors     │
│ - Terrains      │ │ - Costs         │ │ - Stats         │
│ - Mana dist.    │ │ - Triggers      │ │ - Abilities     │
└────────┬────────┘ └────────┬────────┘ └────────┬────────┘
         │                   │                   │
         └───────────────────┼───────────────────┘
                             │
                             v
                    ┌─────────────────┐
                    │   Lua Scripts   │
                    │   (Shareable)   │
                    └────────┬────────┘
                             │
                             v
                    ┌─────────────────┐
                    │   Rust Engine   │
                    │ (Physics, Mana, │
                    │  Rendering)     │
                    └─────────────────┘
```

---

## 9. Open Questions / To Explore

- How exactly is mana stored per voxel?
- What's the energy cost formula for different actions?
- How do power lines / energy transfer work mechanically?
- What Lua APIs need to be exposed for spells?
- How do creature behaviors interact with mana?
- What's the package manager architecture?

---

## 10. Key Insight

**The map editor is not just a content creation tool.**

It's the entry point to a unified system where:
- World building
- Spell creation
- Creature design
- Energy management

...are all done through the same AI + scripting + database pattern.

The physics-based mana system is what makes this coherent. Everything has a cost. Everything follows rules. The AI helps you work within those constraints to build interesting things.

---

## 11. Building Process and Workflow

### Goal: Capture the Process of Building

We have multiple subcomponents. For a map editor, there's a lot going on: create/delete things (manual approach).

**We want a semi-automated, guess-and-check approach:**
- Run a generator, inspect output
- If we don't like it, specify what's wrong
- Edit the generator and iterate
- Optionally manually edit the generator if we understand rules/composition

### Workflow for Partial Execution and Isolation

- Isolate a subsequence to run up to a certain point, then build on top later
- Run a subset of the model, inspect terrain impact, fix it, then run the rest
- In a sequence model (Markov Jr. perspective), break down and support these edits

**Key capability:** Run partial models, cache results, continue from cached state.

---

## 12. AI Control and MCP/ACP

### Edits Must Be AI-Controllable

- Build system as an **MCP or ACP server from the start** to allow agents to act in-world
- Our Lua/Rust setup exposes function calls for models/partial models
- During editing, AI can show/make edits or try different models

### Session Concept for Map Editor

Define "session" concept:
- AI can access everything needed from MCP server perspective
- AI also knows the code (coding context) and associated skills
- Map editor + session behaviors are defined

### AI Integration is External; Interaction via Hot Reload

- Keep AI external; it needs a way to interact with the world
- **Best/simple path: hot reloading Lua scripts**
- If Lua changes, hot reload inside the running scene
- Keep the editor up; edit Lua files outside; hot reload refreshes the editor experience

---

## 13. Initial Map Editor Components

### Three Key Components

```
┌─────────────────────────────────────────────────────────┐
│                    MAP EDITOR                           │
├─────────────────┬─────────────────┬─────────────────────┤
│  (a) TERRAIN    │ (b) TERRAIN     │ (c) TERRAIN         │
│      PALETTE    │     GENERATOR   │     RENDERER        │
│                 │                 │                     │
│  What can be    │  How terrain    │  How terrain        │
│  created        │  is built       │  is displayed       │
│                 │                 │                     │
│  - Voxel types  │  - Fixed seed   │  - 2D or 3D         │
│  - Materials    │  - Reproducible │  - Real-time        │
│  - Properties   │  - Composable   │  - Configurable     │
└─────────────────┴─────────────────┴─────────────────────┘
```

### Terrain Generator Details

- We edit the terrain generator, NOT manual terrain
- Terrain generator could be code (not necessarily Markov Jr.)
- Could be Lua execute
- **Prefer composition of objects** (PyTorch-like; Markov Jr. sequence objects)

**Composition model:**
- Compose objects
- Run them in isolation or with a seed
- Run from previous output
- Cache partial runs and continue
- It's a simulation; we just need to support this behavior

---

## 14. 2D-First Approach

### Start Simple
- Start with a **2D map** for simplicity and fast rendering
- Render-to-texture simple 2D renderer
- Edit types of objects in the 2D palette
- Edit the Markov model; validate real-time behavior

### Incremental Process
- Start with very low complexity
- Validate concepts
- All three (terrain, generator, renderer) defined in Rust, wrapped in Lua
- Live hot reload for terrain palette and generator
- Edits re-import and re-run
- Renderer shows on screen

**Philosophy: We are a stepping-stone collecting farm.**

---

## 15. Documentation Requirements

Construct documentation for:
- What the terrain editor does
- Where the code is
- How to edit terrain successfully

**Provide example code** (e.g., maze generator) as reference.

---

## 16. Persistence and Search

### Next Layer: Persistence

- Previous generated artifacts as reference
- Generate new ones based on history

### Search is Critical

Need search capabilities:
- Store chunks of code with described functionality
- Save code in a format somewhere

### Git/Package Manager Approach

Recommend Git/Git-like repo structure:
- Read details of Lua scripts for a given package
- **Package management + persistence are critical**

### Save/Load Capabilities

- Save/load specific models
- Save/load libraries of generators

### Backend/Database

Need a database/back end to save in some format.

**Embedding-based storage:**
- Embedding DB in Rust (e.g., VectorDB like LanceDB)
- In-memory embeddings
- Pull from global cache, embed into local cache for fast retrieval

### Search Importance

- Search materials
- Describe materials and systems when ingested
- Attach documentation upon logging/uploading/saving

### Incremental Tests

- Build tools that unlock functionality/efficiency early
- Later layer complex features (3D scenes, PBR) on top of validated foundations

---

## 17. Simulation/Rendering Considerations

### Multi-Purpose Tool

- Can function as a Markov Jr. model editor
- Focus = terrain palette + terrain generator + terrain renderer

### Video Rendering

- Support rendering to video
- See simulation play out
- Speed of simulation and rendering configurable (large/small renders)

### Priorities

1. Functionality
2. Efficiency
3. Incremental stepping stones

---

## 18. Sharing and Namespacing

### Personal and Shared Use

- Ability to play personally and share with others
- Others must use the same tool, build/save their own things

### Namespacing from Start

- Start with namespacing everything
- Import others' stuff like a package manager
- **Package manager should be an early feature**

### Organize Lua Code for AI Access

From the get-go, organize in an AI-accessible way:
- Folders
- Setup scripts
- JSON manifest (e.g., package.json-style) assembling components

---

## 19. Package Manager Questions

**Research needed:**

1. Is there an open source package manager for Lua packages?
2. Is there an open source protocol for Lua package management to clone from repos?
3. Is there a package manager usable within GCP or elsewhere?
4. Can we leverage Rust ecosystem (e.g., UV) if relevant?

**Goal:** Super-efficient package management.

---

## 20. Key Milestones Summary

From this brain dump, key milestones are:

1. **2D terrain editor** with palette + generator + renderer
2. **Hot reload** working for Lua scripts
3. **MCP/ACP server** for AI control
4. **Package manager** infrastructure
5. **Persistence/search** with embeddings
6. **Documentation** with example code
7. **3D upgrade** after 2D is validated

---

## 21. Core Game Loop Vision

### Terraforming a World

Part of the game is the ability to terraform a world:
- Some world parameters are fixed (e.g., number of moons)
- Others are changeable through gameplay

### Example World: Tundra River World

A rich example to illustrate the vision:

**Geography:**
- Tundra world with a river down the center
- Along the river it's very tropical
- As soon as you exit the tropical region, you hit tundra
- The only habitable region is the river
- Gutters of hot steaming water where volcanic activity happens under the surface

**Underground:**
- Dig down: lots of lava and "lava mana" that you can mine

**Tundra Resources:**
- Outside in the tundra are "hard mana"
- Soul stones: rocky objects holding dense mana
- If you can find and tolerate that area, you can live there
- The closer you get to the poles or certain regions, the more soul stone and more powerful objects

**Creatures:**
- Tundra monsters that murder anything that enters; very powerful

**Factions:**
- River people live on the water
- They're terrorized by the tundra
- Castles along the river representing the river people

**Story/Conflict:**
- Some areas getting pummeled by coordinated attacks from two sides of the tundra
- This is unusual - they've never cooperated before
- Two enemies working together:
  - **Tundra Necromancer** - raises ice zombies from death; if you kill somebody, he can rise up
  - **Ice King** - cools things; the big boss

**Goal of the simulation:**
- You start as a river person
- Your goal is to kill the Ice King

---

## 22. World Building with AI

### The Vision

I want to be able to design that story:
- People
- Things
- Timelines
- All the stuff that creates a rich atmosphere

### What "Playing God" Means

The AI-assisted map editor is "playing God":
1. Describe the story and worldbuild
2. Build the world
3. Define key players
4. Populate the world with stuff (tundra monsters, etc.)
5. Run the simulation
6. Then drop yourself into it and play it out

---

## 23. RTS-Style Automation Concepts

### Automatic Operations

- Objects that mine automatically
- Miners operate automatically
- "How'd you get that?" - Lua scripts that operate these guys according to our API

### Code Logic Into Objects

Idea: code a lot of the logic directly into the game, directly into the objects.

---

## 24. Efficiency as Game Mechanic

### Objects Take Power to Operate

Objects might take power to operate because they're not efficient.

**Example: Harvester Brain**
- If I make a better harvester brain, I can put it on the object and have a better harvester
- It doesn't need to run A* every frame and cost a bajillion mana
- A physically based system means we can compute costs
- Costs proportional to compute costs

### Alignment of Player and Game Goals

More efficient things are better things:
- They use less energy
- Call fewer frames
- **This aligns players and the game: efficiency matters**

Players are incentivized to write better code because better code = less mana cost = competitive advantage.

---

## 25. AI That Understands the Code

### Starting Point: Map Editor

It all starts from a map editor where you build the fantasy with an AI that understands the code underneath:
- It knows the systems
- If it doesn't, it can study them
- It potentially has direct access to the source code
- You can peer behind the wall and see what's going on

**This is super duper cool, but it's tricky business.**

---

## 26. The Problem: End-to-End Experience

### My Typical Problem

I imagine part of a thing, not the full end-to-end thing. I can't maintain the full end-to-end player experience in my mind.

### What We Need

We need to:
- Make a storyboard
- Make a TikTok ad
- Communicate to ourselves and others: what happens in this game? What's interesting and compelling?

---

## 27. Exemplars as North Stars

### We Need Exemplars

- The player story
- Break that down into what functionality we need
- If the loop is wildly complex, produce a better (simpler) exemplar that still has the minimum core gameplay loop

### Defining Characteristics for Constructing the Roadmap

1. **Build our exemplars**
2. **Iterate the shit out of our exemplars**
3. For each idea, **list the systems needed** to support it
4. Identify the **critical system** we want to communicate
5. Create a new example with **just that**; cut, cut, cut to show that functionality
6. **Pick a demo to produce**
7. Once we have the demo, the roadmap is clear: **incrementally complexify until we reach that demo**

### Demos as North Stars

- Think about our demos as our North Stars
- Each demo represents a concrete, achievable goal
- The roadmap flows backward from the demo

---

## 28. TikTok Exemplars: End-to-End Experiences

### Core Question

What full end-to-end experience do I want to convey in a TikTok ad? What is the core thing I want to convey?

---

## 29. TikTok Idea #1: Physics-Based Mana (PBM) System

### The Benefit

Solves many problems with magic systems if it existed.

### Problem with Typical Magic

Magic in most games is **unbounded**:
- "I regenerate 50 mana per second" simplifies to time- and mana-based management
- It's easy: blue bar full = cast
- All spells draw from the same pool
- Generic, limited, artist-constructed
- Often lacking personality and creativity
- **Balance is a nightmare** (e.g., massively powerful spell with modest mana cost becomes dominant)

### Opposite Extreme: Purely User-Generated Spells

Devolves into bullet hell:
- "More shit" dominates
- Real world has capacity limits (you can't manufacture infinite bullets)

### Our PBM Solution

- Small particles can't go far; they burn out quickly
- Minimum voxel size and minimum energy per voxel
- **1,000 small voxels cost more energy than one object of 1,000 combined voxels**
- Efficiency discourages bullet hell
- Large "spirit bomb" is possible but has tradeoffs:
  - Limited range
  - Charge-up locks you in place
  - Makes you vulnerable (Goku spirit bomb principle)

### TikTok Execution

- "Watch me make a spirit bomb" from scratch
- Explain tradeoffs; if missing tradeoffs, then massive mana costs
- "Watch me edit a spell" live: make something creative with constraints

---

## 30. TikTok Idea #2: Flexifying a Spell

### Progression

Show iterative complexity:
1. Simple fireball
2. Splitting fireball
3. Artillery-shell fireball
4. MIRV (multiple independently targetable reentry vehicle)
5. Heat-seeking fireball

### The Point

Show the iterative process of making it more interesting and complex.

---

## 31. TikTok Idea #3: Weird Shit

### Concept

Make a weird creature/spell/world/material:
- E.g., diamond-looking something
- Must be **attention-grabbing weird**
- I don't know exactly what yet, but it's gotta be weird

---

## 32. TikTok Idea #4: What's Wrong with Harry Potter (HPP)

### The Problem

"Magic actually sucks" in HP:
- Makes no sense
- But charm/whimsy is why we like it

### Investigate

Why do we like HP despite failures?

### Present "The Harry Potter Problem" (HPP)

Spells are basically unlimited.

### Our Solution

Physics-based magic (PBM).

### Need Better Acronym

PBM is descriptive but not exciting. Candidates:
- PIM
- PAM (Physical Alchemy Magic)
- RBM (Reality-Based Magic)

We need a term that conveys physics + magic cleanly.

---

## 33. TikTok Idea #5: Map Editor (with AI Angle)

### Basic Angle (Probably Not Compelling)

"Come with me while I make two warring nations on a tundra planet."

### Better Angle: Why Gamers Hate AI in Games

**The Reason:**
- It doesn't give them shit
- It doesn't do anything for them

**As a Coder vs Gamer:**
- As a coder, AI helps
- For gamers, misaligned incentives
- What excites executives about AI is diametrically opposed to what excites gamers
- The AI we're seeing is not the AI we want
- But we DO want AI—just not the executive version

**Show Our Solution:**
- Positive AI that expands player creativity, doesn't limit
- Augments devs instead of replacing them
- Makes devs more ambitious

**On Visual Artists:**
- Visual artists in 3D aren't currently at risk (rigging/designing is hard)
- But it will get there
- We want artists supercharged: from one piece/week to one piece/day
- More styles, expression unlocked

---

## 34. TikTok Idea #6: Bleak AI Future vs Cool Tech

### The Premise

The world that's coming (AI) is not the one we want. Especially true in games.

**Bleak future, cool tech.**

### Our Vision

- Strap AI to gamers and developers to build incredibly ambitious games
- Explain what that ambition looks like
- How we unlock it

---

## 35. TikTok Idea #7: "Three Most Ambitious Things About My Game"

### Format (I Don't Love It, But...)

Example claims with hypothesis → evidence → conclusion (x3):
- E.g., full map editor with AI assistance
- Etc.

---

## 36. What Resonates in TikTok Ads?

### Key Themes

- **Ambition** and "simulator of your wildest dreams"
- **Control with thoughts**
- Depth of simulation tied to personal interest and expression
- No hard walls

### Requirements

Requires key systems (philosophical, not just technical).

---

## 37. Heady Philosophical Video: Expression in Simulation

### Core Topic

Expression in simulation and the difficulty curve of creating in Minecraft.

### "What Minecraft Gets Wrong"

**The Problem:**
- The fundamental difficulty of making something nice is a learning bottleneck
- Too steep a curve means work doesn't get done
- Manual work in Minecraft is hard

**Why Don't More Simulation Nerds Play It?**
- Difficulty of making nice/interesting things is too high

**Counterargument:**
- "That's the charm"
- Sure, philosophically valid—but practically, AI tools matter

### Coding as Metaphor

**Software runs the world:**
- Few coders relative to impact
- Massive wealth concentrated in orgs good at coding/distributed systems

**Why isn't 90% of the world coding?**
- Because it's fucking hard
- Hard doesn't equal motivating

**Minecraft's "hard to make pretty" ≈ coding:**
- Huge, beautiful things only a few can achieve
- Discouraging for most

**AI in coding is unlocking creators:**
- Coders will quadruple in two years
- Non-coders writing code (sales, business, managers, doctors) via AI

### So Why Not Bring That to Games?

**Imagine:**
- An authoring tool that builds whatever you want in Minecraft

**Key Insight:**
- Games need constraints
- Minecraft does constraints well (you find tiles/resources)
- But it's also very limiting
- Resource farming/building is hard

**Use This Critique as Inspiration:**
- AI puts us on the most interesting precipice
- Build that into our game

---

## 38. TikTok Idea #8: Day-Night Cycle with Two Moons

### Hook

"What's better than one moon? Two moons. Check it out."

Simple, visual, immediate.

---

## 39. TikTok Idea #9: Markov Jr. and Procedural Generation

### Hook (Unclear, But Focus Is...)

Whole video on Markov Jr./Markov models/wave function collapse/procedural gen.

### Structure

1. **Start with most interesting view:** Rendered apartment complex in an environment
2. **Reveal:** "Would you believe this apartment comes from something as simple as this?"
3. **Show:** The simple growth model/cellular automata

### Key Insight

Cellular automata shine when combined with simulation + voxels.

### Example Execution

- Toss an object
- Upon landing, it grows an apartment complex
- Two moons, strong time-of-day
- Side details, PBR
- **Make it sexy**

---

## 40. TikTok Summary: Candidate Ideas

| # | Idea | Hook/Core |
|---|------|-----------|
| 1 | Physics-Based Mana | "Watch me make a spirit bomb" - tradeoffs and constraints |
| 2 | Flexifying a Spell | Fireball → MIRV progression |
| 3 | Weird Shit | Attention-grabbing strange creation |
| 4 | Harry Potter Problem | "Magic sucks in HP" → our solution |
| 5 | Map Editor / AI for Gamers | "Why gamers hate AI" → our positive AI |
| 6 | Bleak AI Future | Cool tech, bad trajectory → our vision |
| 7 | Three Ambitious Things | Hypothesis → evidence → conclusion format |
| 8 | Two Moons | Simple visual hook |
| 9 | Markov Jr. Procedural | Object lands → apartment grows |

---

## 41. Meta: What These Notes Are

This is super unorganized. Dumping it here to think about and create from.

**Purpose:**
- Raw material for demos
- Raw material for marketing
- Raw material for roadmap prioritization

**Next:**
- More rants may follow
- Will organize into actionable docs later

---

*More notes to be appended...*
