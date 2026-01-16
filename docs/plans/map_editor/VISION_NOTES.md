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

*More notes to be appended...*
