# Vision Notes: Sectioned Summary

*A structured overview of all content in VISION_NOTES.md, organized by theme.*

---

## North Stars

These are the guiding principles that shape all decisions:

### 1. Physics-Based Magic is the Foundation
Everything in the game world operates under real energy constraints. Mana equals energy. Actions have costs. Efficiency matters. This creates natural balance and meaningful tradeoffs without arbitrary game-designer limits.

### 2. AI Expands Creativity, Not Replaces It
AI assists players and developers to be more ambitious, not less employed. The AI we're building is the opposite of what executives want—it's what creators want. It unlocks expression, not limits it.

### 3. Design Your World, Then Play In It
The core loop is: describe a world → build it with AI assistance → run the simulation → drop in and play. You are the god of your world, then you are the hero in it.

### 4. Demos Drive the Roadmap
We build backward from concrete, achievable demos. Each demo is a North Star. The roadmap is the path to that demo. Cut ruthlessly to reach the demo faster.

---

## Goals

| Goal | Description | Success Metric |
|------|-------------|----------------|
| **Ship a Map Editor** | A working tool for creating voxel worlds with AI assistance | Can record a TikTok showing AI-assisted world creation |
| **Prove Physics-Based Magic** | Demonstrate the mana-as-energy system with meaningful tradeoffs | Can show a spell with clear cost/benefit tradeoffs |
| **Enable Sharing** | Players can share worlds, generators, and content | Can import someone else's palette/generator |
| **Achieve Hot Reload** | Edit files externally, see changes instantly | Lua file change reflects in <1 second |
| **Validate 2D Before 3D** | Prove the workflow works in 2D before adding complexity | 2D editor is functional and useful |

---

## Objectives

### Objective 1: Build the Map Editor Core
Create the fundamental architecture: Palette + Generator + Renderer, accessible via Lua, with hot reload.

**Mechanisms:**
- Define palette format in Lua
- Define generator composition model (PyTorch-like)
- Implement 2D renderer as validation target
- Add file watcher for hot reload
- Wrap Rust APIs in Lua bindings

### Objective 2: Establish External AI Access
Make the running game controllable by external AI agents.

**Mechanisms:**
- Start MCP server with game launch
- Expose resources (palette, generator, terrain state)
- Expose tools (reload, regenerate, query)
- Keep server persistent; restart only on command

### Objective 3: Define the Mana System
Specify how energy works in the game world.

**Mechanisms:**
- Define mana storage (per-voxel, per-region, or per-entity)
- Define energy cost formulas (mass, distance, fragmentation, computation)
- Define power transfer mechanics (power lines)
- Balance through playtesting

### Objective 4: Create Shareable Content Ecosystem
Enable players to package and share their creations.

**Mechanisms:**
- Define package format (folders with manifests)
- Implement save/load for models and generators
- Add embedding-based search (LanceDB or similar)
- Namespace everything from the start

### Objective 5: Produce Demo Content
Create compelling demonstrations of the system.

**Mechanisms:**
- Select target TikTok demo
- Build minimum systems required for that demo
- Record and iterate on presentation
- Use demo as forcing function for scope

---

## Thematic Clusters (Table of Contents)

The vision notes are organized into seven thematic clusters. Each cluster represents a coherent area of work.

| Cluster | Core Concern | Key Question |
|---------|--------------|--------------|
| **A. Mana/Physics System** | Energy constraints as game foundation | How does mana work mechanically? |
| **B. AI Integration** | How AI assists creation without replacing creators | What can AI do? What can't it do? |
| **C. Map Editor Architecture** | Palette + Generator + Renderer | What are the components and how do they connect? |
| **D. Sharing/Ecosystem** | Databases, persistence, package management | How do players share content? |
| **E. Workflow/UX** | Guess-and-check iteration, hot reload | What's the editing experience? |
| **F. Game Loop/Story** | Terraforming, world building, playing | What does the player actually do? |
| **G. Marketing/Demos** | TikTok hooks, exemplars as North Stars | How do we communicate the vision? |

---

## Cluster A: Mana/Physics System

*Energy constraints as game foundation*

### A.1: Physics-Based Magic / Mana = Energy (Notes §2)
Mana equals energy—every action in the world has a real cost, creating natural constraints and making efficiency matter.

### A.2: Mana Distribution as World Parameter (Notes §6)
Worlds can be mana-rich or mana-scarce, fundamentally changing gameplay difficulty and available strategies.

### A.3: Spell System (Notes §7)
Spells use the same AI + Lua + database pattern as everything else, with energy accounting ensuring physical consistency.

### A.4: Efficiency as Game Mechanic (Notes §24)
Better code = less mana cost = competitive advantage; this aligns player goals with game design by making efficiency intrinsically valuable.

### A.5: TikTok Idea #1 - Physics-Based Mana (Notes §29)
Show how PBM solves the "unbounded magic" problem with tradeoffs; demo making a spirit bomb from scratch.

### A.6: TikTok Idea #4 - Harry Potter Problem (Notes §32)
"Magic sucks in HP" framing; present the problem of unlimited spells and our PBM solution; need a better acronym.

---

## Cluster B: AI Integration

*How AI assists creation without replacing creators*

### B.1: AI Features and Coding Model (Notes §3)
A layered architecture where users speak naturally, AI interprets intent, Lua scripts define behavior, and Rust provides the unchanging engine foundation.

### B.2: AI Control and MCP/ACP (Notes §12)
Build the system as an MCP server from the start so AI agents can act in-world; define sessions; keep AI external with hot reload as the interaction mechanism.

### B.3: World Building with AI (Notes §22)
"Playing God" means describing a story, building the world, defining players, populating it, running the simulation, then dropping in to play.

### B.4: AI That Understands the Code (Notes §25)
The AI assistant knows the underlying systems and can study them; users can peer behind the wall—cool but tricky.

### B.5: TikTok Idea #5 - Map Editor / AI for Gamers (Notes §33)
"Why gamers hate AI in games" angle—show our positive AI that expands creativity rather than limiting it.

### B.6: TikTok Idea #6 - Bleak AI Future vs Cool Tech (Notes §34)
The AI future we're getting isn't the one we want; show how we strap AI to players/devs for ambitious games.

### B.7: Philosophical Video - Expression in Simulation (Notes §37)
"What Minecraft Gets Wrong"—difficulty curve of creation is too steep; AI unlocks creators in coding, why not in games?

---

## Cluster C: Map Editor Architecture

*Palette + Generator + Renderer*

### C.1: Top-Level Vision (Notes §1)
The map editor is part of a three-pillar system: physics-based magic, AI-assisted coding, and shareable databases of game content.

### C.2: Connections Between Systems (Notes §8)
A diagram showing how AI Assistant sits atop Map Editor, Spell Creator, and Creature Editor, all flowing through Lua to the Rust engine.

### C.3: Key Insight (Notes §10)
The map editor is not just a content tool—it's the entry point to a unified system where world building, spell creation, creature design, and energy management share the same patterns.

### C.4: Initial Map Editor Components (Notes §13)
Three key components: Terrain Palette (what can be created), Terrain Generator (how terrain is built), and Terrain Renderer (how it's displayed).

### C.5: 2D-First Approach (Notes §14)
Start with 2D for simplicity and fast iteration; validate concepts before adding complexity—"we are a stepping-stone collecting farm."

### C.6: Map Editor 2D and 3D Strategy (Notes §42)
Do 2D first, then create equivalent 3D objects; don't skip 2D, don't make them identical.

### C.7: Map Editor Core Functionalities (Notes §43)
Three core requirements: hot reloading, separation of palette/generator/renderer, Lua accessibility.

### C.8: MCP Server Integration (Notes §44)
Start MCP server with the game; allow external queries and Lua edits; server persists unless restart is commanded.

---

## Cluster D: Sharing/Ecosystem

*Databases, persistence, package management*

### D.1: Database-First Material Storage (Roadmap §6)
Materials are stored in an embedded database (SQLite with vector extension or LanceDB), NOT as files. This enables semantic search, relationships between materials, custom textures, and proper versioning. A palette is a query result or saved collection, not a monolithic file. AI creates materials via MCP calls and gets IDs back.

### D.2: Databases and Materials (Notes §4)
The system supports multiple databases (palettes, generators, creatures, spells) with PBR materials as the critical lowest-level building block.

### D.3: Generators and Models (Notes §5)
Markov Jr. and similar generators take palettes and rules to build terrains, creatures, and mana distributions using a consistent layered pattern.

### D.4: Persistence and Search (Notes §16)
Need save/load for models and generators, embedding-based search (e.g., LanceDB), and documentation attached to saved artifacts.

### D.5: Sharing and Namespacing (Notes §18)
Namespace everything from the start; support importing others' work like a package manager; organize in an AI-accessible way with manifests.

### D.6: Package Manager Questions (Notes §19)
Research needed on package management options, protocols, and whether we can leverage existing ecosystems.

---

## Cluster E: Workflow/UX

*Guess-and-check iteration, hot reload, docs*

### E.1: Building Process and Workflow (Notes §11)
We want a semi-automated guess-and-check workflow: run generators, inspect output, specify what's wrong, iterate—with support for partial execution and caching.

### E.2: Documentation Requirements (Notes §15)
We need docs explaining what the editor does, where code lives, and how to edit terrain, plus example code like a maze generator.

### E.3: Simulation/Rendering Considerations (Notes §17)
The tool should support video rendering of simulations with configurable speed; priorities are functionality, efficiency, and incremental stepping stones.

### E.4: Key Milestones Summary (Notes §20)
Seven milestones: 2D editor, hot reload, MCP server, package manager, persistence/search, documentation, 3D upgrade.

### E.5: Open Questions (Notes §9)
Initial list of unresolved technical questions about mana storage, energy formulas, power transfer, and APIs.

---

## Cluster F: Game Loop/Story

*Terraforming, world building, playing what you made*

### F.1: Core Game Loop Vision (Notes §21)
The game involves terraforming worlds with fixed and changeable parameters; detailed "Tundra River World" example with geography, resources, factions, and the Ice King boss.

### F.2: RTS-Style Automation Concepts (Notes §23)
Objects can operate automatically via Lua scripts; code logic directly into game objects for mining, harvesting, etc.

### F.3: The Problem: End-to-End Experience (Notes §26)
Challenge: imagining part of a thing but not the full player experience; need storyboards, TikTok ads, and clear communication of what's compelling.

---

## Cluster G: Marketing/Demos

*TikTok hooks, exemplars as North Stars*

### G.1: Exemplars as North Stars (Notes §27)
Build exemplars, iterate them, list required systems, identify critical features, cut to essentials, pick a demo, then roadmap backward from that demo.

### G.2: TikTok Exemplars Introduction (Notes §28)
Core question: what end-to-end experience do we want to convey in a TikTok ad?

### G.3: TikTok Idea #2 - Flexifying a Spell (Notes §30)
Show iterative spell complexity: fireball → splitting → artillery → MIRV → heat-seeking.

### G.4: TikTok Idea #3 - Weird Shit (Notes §31)
Make something attention-grabbing weird—creature, spell, world, or material.

### G.5: TikTok Idea #7 - Three Ambitious Things (Notes §35)
Hypothesis → evidence → conclusion format for three claims about the game.

### G.6: What Resonates in TikTok Ads (Notes §36)
Key themes: ambition, control with thoughts, depth of simulation, personal expression, no hard walls.

### G.7: TikTok Idea #8 - Two Moons (Notes §38)
Simple visual hook: "What's better than one moon? Two moons."

### G.8: TikTok Idea #9 - Markov Jr. Procedural (Notes §39)
Object lands and grows an apartment complex; reveal the simple rules behind complex output.

### G.9: TikTok Summary Table (Notes §40)
Table of all 9 TikTok ideas with hooks.

### G.10: Meta Note (Notes §41)
These are unorganized raw materials for demos, marketing, and roadmap prioritization.

---

## Appendix: Confidence Check

Having reviewed all 44 sections, we are capturing:

- [x] The physics-based magic foundation
- [x] The AI-assisted coding model
- [x] The database/sharing ecosystem vision
- [x] The map editor architecture (palette/generator/renderer)
- [x] The 2D-first, then 3D strategy
- [x] The MCP server for external AI access
- [x] The game loop (design → simulate → play)
- [x] The efficiency-as-gameplay mechanic
- [x] 9 TikTok demo concepts
- [x] The "exemplars as North Stars" methodology

**What's missing or underspecified:**
- Concrete mana formulas and energy costs
- Specific Lua API design
- Package manager implementation details
- Creature editor specifics (mentioned but not detailed)
- Multiplayer/sharing mechanics
- Actual demo selection (which one first?)

These gaps become questions in `QUESTIONS_AND_STRATEGY.md`.
