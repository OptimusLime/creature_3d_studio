# Vision Notes: Sectioned Summary

*A structured overview of all content in VISION_NOTES.md, organized by theme.*

---

## Part I: Foundation and Philosophy

### Section 1: Top-Level Vision (Notes §1)
The map editor is part of a three-pillar system: physics-based magic, AI-assisted coding, and shareable databases of game content.

### Section 2: Physics-Based Magic / Mana = Energy (Notes §2)
Mana equals energy—every action in the world has a real cost, creating natural constraints and making efficiency matter.

### Section 3: AI Features and Coding Model (Notes §3)
A layered architecture where users speak naturally, AI interprets intent, Lua scripts define behavior, and Rust provides the unchanging engine foundation.

### Section 4: Databases and Materials (Notes §4)
The system supports multiple databases (palettes, generators, creatures, spells) with PBR materials as the critical lowest-level building block.

### Section 5: Generators and Models (Notes §5)
Markov Jr. and similar generators take palettes and rules to build terrains, creatures, and mana distributions using a consistent layered pattern.

### Section 6: Mana Distribution as World Parameter (Notes §6)
Worlds can be mana-rich or mana-scarce, fundamentally changing gameplay difficulty and available strategies.

### Section 7: Spell System (Notes §7)
Spells use the same AI + Lua + database pattern as everything else, with energy accounting ensuring physical consistency.

### Section 8: Connections Between Systems (Notes §8)
A diagram showing how AI Assistant sits atop Map Editor, Spell Creator, and Creature Editor, all flowing through Lua to the Rust engine.

### Section 9: Open Questions (Notes §9)
Initial list of unresolved technical questions about mana storage, energy formulas, power transfer, and APIs.

### Section 10: Key Insight (Notes §10)
The map editor is not just a content tool—it's the entry point to a unified system where world building, spell creation, creature design, and energy management share the same patterns.

---

## Part II: Building Process and Architecture

### Section 11: Building Process and Workflow (Notes §11)
We want a semi-automated guess-and-check workflow: run generators, inspect output, specify what's wrong, iterate—with support for partial execution and caching.

### Section 12: AI Control and MCP/ACP (Notes §12)
Build the system as an MCP server from the start so AI agents can act in-world; define sessions; keep AI external with hot reload as the interaction mechanism.

### Section 13: Initial Map Editor Components (Notes §13)
Three key components: Terrain Palette (what can be created), Terrain Generator (how terrain is built), and Terrain Renderer (how it's displayed).

### Section 14: 2D-First Approach (Notes §14)
Start with 2D for simplicity and fast iteration; validate concepts before adding complexity—"we are a stepping-stone collecting farm."

### Section 15: Documentation Requirements (Notes §15)
We need docs explaining what the editor does, where code lives, and how to edit terrain, plus example code like a maze generator.

### Section 16: Persistence and Search (Notes §16)
Need save/load for models and generators, embedding-based search (e.g., LanceDB), and documentation attached to saved artifacts.

### Section 17: Simulation/Rendering Considerations (Notes §17)
The tool should support video rendering of simulations with configurable speed; priorities are functionality, efficiency, and incremental stepping stones.

### Section 18: Sharing and Namespacing (Notes §18)
Namespace everything from the start; support importing others' work like a package manager; organize Lua code in an AI-accessible way with manifests.

### Section 19: Package Manager Questions (Notes §19)
Research needed on Lua package management options, protocols, and whether we can leverage existing ecosystems.

### Section 20: Key Milestones Summary (Notes §20)
Seven milestones: 2D editor, hot reload, MCP server, package manager, persistence/search, documentation, 3D upgrade.

---

## Part III: Game Vision and Core Loop

### Section 21: Core Game Loop Vision (Notes §21)
The game involves terraforming worlds with fixed and changeable parameters; detailed "Tundra River World" example with geography, resources, factions, and the Ice King boss.

### Section 22: World Building with AI (Notes §22)
"Playing God" means describing a story, building the world, defining players, populating it, running the simulation, then dropping in to play.

### Section 23: RTS-Style Automation Concepts (Notes §23)
Objects can operate automatically via Lua scripts; code logic directly into game objects for mining, harvesting, etc.

### Section 24: Efficiency as Game Mechanic (Notes §24)
Better code = less mana cost = competitive advantage; this aligns player goals with game design by making efficiency intrinsically valuable.

### Section 25: AI That Understands the Code (Notes §25)
The AI assistant knows the underlying systems and can study them; users can peer behind the wall—cool but tricky.

### Section 26: The Problem: End-to-End Experience (Notes §26)
Challenge: imagining part of a thing but not the full player experience; need storyboards, TikTok ads, and clear communication of what's compelling.

### Section 27: Exemplars as North Stars (Notes §27)
Build exemplars, iterate them, list required systems, identify critical features, cut to essentials, pick a demo, then roadmap backward from that demo.

---

## Part IV: Marketing and Demo Ideas

### Section 28: TikTok Exemplars Introduction (Notes §28)
Core question: what end-to-end experience do we want to convey in a TikTok ad?

### Section 29: TikTok Idea #1 - Physics-Based Mana (Notes §29)
Show how PBM solves the "unbounded magic" problem with tradeoffs; demo making a spirit bomb from scratch.

### Section 30: TikTok Idea #2 - Flexifying a Spell (Notes §30)
Show iterative spell complexity: fireball → splitting → artillery → MIRV → heat-seeking.

### Section 31: TikTok Idea #3 - Weird Shit (Notes §31)
Make something attention-grabbing weird—creature, spell, world, or material.

### Section 32: TikTok Idea #4 - Harry Potter Problem (Notes §32)
"Magic sucks in HP" framing; present the problem of unlimited spells and our PBM solution; need a better acronym.

### Section 33: TikTok Idea #5 - Map Editor / AI for Gamers (Notes §33)
"Why gamers hate AI in games" angle—show our positive AI that expands creativity rather than limiting it.

### Section 34: TikTok Idea #6 - Bleak AI Future vs Cool Tech (Notes §34)
The AI future we're getting isn't the one we want; show how we strap AI to players/devs for ambitious games.

### Section 35: TikTok Idea #7 - Three Ambitious Things (Notes §35)
Hypothesis → evidence → conclusion format for three claims about the game.

### Section 36: What Resonates in TikTok Ads (Notes §36)
Key themes: ambition, control with thoughts, depth of simulation, personal expression, no hard walls.

### Section 37: Philosophical Video - Expression in Simulation (Notes §37)
"What Minecraft Gets Wrong"—difficulty curve of creation is too steep; AI unlocks creators in coding, why not in games?

### Section 38: TikTok Idea #8 - Two Moons (Notes §38)
Simple visual hook: "What's better than one moon? Two moons."

### Section 39: TikTok Idea #9 - Markov Jr. Procedural (Notes §39)
Object lands and grows an apartment complex; reveal the simple rules behind complex output.

### Section 40: TikTok Summary Table (Notes §40)
Table of all 9 TikTok ideas with hooks.

### Section 41: Meta Note (Notes §41)
These are unorganized raw materials for demos, marketing, and roadmap prioritization.

---

## Part V: Technical Strategy

### Section 42: Map Editor 2D and 3D Strategy (Notes §42)
Do 2D first, then create equivalent 3D objects; don't skip 2D, don't make them identical.

### Section 43: Map Editor Core Functionalities (Notes §43)
Three core requirements: hot reloading, separation of palette/generator/renderer, Lua accessibility.

### Section 44: MCP Server Integration (Notes §44)
Start MCP server with the game; allow external queries and Lua edits; server persists unless restart is commanded.

---

## Thematic Clusters

For roadmap planning, the notes cluster into these themes:

| Theme | Sections | Core Concern |
|-------|----------|--------------|
| **Mana/Physics System** | 2, 6, 7, 24, 29 | Energy constraints as game foundation |
| **AI Integration** | 3, 12, 22, 25, 33-34, 37 | How AI assists creation without replacing creators |
| **Map Editor Architecture** | 13, 14, 42-44 | Palette + Generator + Renderer, 2D then 3D |
| **Sharing/Ecosystem** | 4, 16, 18-19 | Databases, persistence, package management |
| **Workflow/UX** | 11, 15, 17 | Guess-and-check iteration, hot reload, docs |
| **Game Loop/Story** | 21-23, 26 | Terraforming, world building, playing what you made |
| **Marketing/Demos** | 27-41 | TikTok hooks, exemplars as North Stars |

---

## Confidence Check

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

These gaps become questions for the next document.
