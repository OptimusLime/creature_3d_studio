# Questions and Strategy Document

*Exploring the vision notes through questions, hypotheses, and decision points.*

*This document is designed for interactive refinement. Each question has space for your answer.*

---

## How to Use This Document

1. Read each question and the context/hypothesis provided
2. Write your answer in the **[YOUR ANSWER]** section
3. If the question is wrong or irrelevant, replace it
4. If the hypothesis is wrong, correct it
5. We'll use your answers to build the roadmap

---

## Part I: Foundational Decisions

### Q1: What is the MVP demo?

**Context:** You've described 9 TikTok ideas, a complex game loop (Tundra River World), and multiple system components. We can't build everything at once.

**Hypothesis:** The MVP demo should be the simplest thing that demonstrates the core differentiator. I believe that's either:
- **Option A:** 2D Map Editor with hot reload (shows AI-editable world creation)
- **Option B:** Physics-Based Mana spell creation (shows the novel magic system)
- **Option C:** Markov Jr. procedural growth (shows the "object lands → building grows" wow factor)

**My lean:** Option A, because it's the foundation everything else builds on. The map editor is the entry point.

**Question:** Which demo should we build first, and why?

**[YOUR ANSWER]:**




---

### Q2: Is the map editor the game, or a tool for making the game?

**Context:** You describe "playing God" (designing worlds) and then "dropping in to play." These could be:
- **One product:** A game where world-building IS the gameplay
- **Two modes:** Editor mode and play mode in the same app
- **Two products:** A separate editor tool and a separate game

**Hypothesis:** It's one product with two modes. The magic is the seamless transition from "I designed this" to "I'm playing in what I designed."

**Question:** Is the map editor the game itself, or is it a tool that produces games/worlds?

**[YOUR ANSWER]:**




---

### Q3: Who is the player?

**Context:** The vision involves coding (Lua), AI assistance, procedural generation, and physics-based magic. This appeals to different audiences:
- **Coders/modders:** Love the Lua scripting, package ecosystem
- **Creative builders:** Love the AI-assisted world design (Minecraft + AI)
- **Gamers:** Love the physics-based magic combat and survival
- **Content creators:** Love the TikTok-worthy visuals and shareable worlds

**Hypothesis:** The primary audience is creative builders who want Minecraft-like expression without Minecraft-like tedium. Coders are secondary (power users). Pure gamers come later once there's content to play.

**Question:** Who is the primary player, and how does that affect what we build first?

**[YOUR ANSWER]:**




---

### Q4: What does "done" look like for the map editor?

**Context:** The map editor has many possible features: palette editing, generator editing, hot reload, MCP server, persistence, search, sharing, 2D, 3D, video export...

**Hypothesis:** "Done" for V1 means:
1. Can define a palette in Lua
2. Can define a generator in Lua
3. Can see output rendered (2D first)
4. Hot reload works
5. AI can edit files externally and see results

Everything else (MCP server, persistence, search, 3D, sharing) is V2+.

**Question:** What is the minimum feature set for the map editor to be "shippable" as a demo?

**[YOUR ANSWER]:**




---

## Part II: Technical Architecture

### Q5: How do we structure the API for materials and generators?

**Context:** Materials should NOT be stored as Lua files—they belong in a database (SQLite/LanceDB) with embeddings for search. The API needs to support:
- Material creation (returns ID)
- Material search (semantic query)
- Palette composition (collection of material IDs)
- Generator definition (possibly composable, PyTorch-like)
- Partial execution and caching

**Hypothesis:** Materials are created via MCP calls, not Lua files:

```
// MCP Tool: create_material
{ "name": "stone", "color": [0.3, 0.3, 0.3], "roughness": 0.7, "tags": ["solid"] }
// Returns: { "id": 12345 }

// MCP Tool: search_materials
{ "query": "wood-like materials", "limit": 10 }
// Returns: [{ "id": 101, "name": "dark_wood", ... }, ...]

// MCP Tool: create_palette
{ "name": "dark_fantasy", "material_ids": [12345, 12346] }
// Returns: { "palette_id": 1 }
```

Generators might still be Lua (for composition logic), but materials are database records.

**Question:** Does this database-first approach for materials make sense? Should generators also be database records, or is Lua appropriate for their compositional nature?

**[YOUR ANSWER]:**




---

### Q6: How does hot reload work technically?

**Context:** With database-first materials, "hot reload" means something different:
- Materials: Database write triggers `MaterialChangedEvent`
- Generators: Could still be Lua files (or also database records)
- Config: Could be database or files

**Hypothesis:** Hot reload triggers are:
1. **Materials:** MCP call → database write → event → cache refresh → re-mesh if in use
2. **Generators:** Either Lua file change OR database record change → re-run generation
3. **Config:** Database or file change → update uniforms immediately

The key insight: database writes ARE the hot reload trigger for materials, not file watching.

**Open question:** Should generators also be database records for consistency, or is Lua better for their compositional/algorithmic nature?

**Question:** What are the hot reload semantics? Specifically, what triggers what, and what state survives vs. regenerates?

**[YOUR ANSWER]:**




---

### Q7: What is the MCP server's scope?

**Context:** You want an MCP server so AI can interact with the running game. MCP (Model Context Protocol) typically exposes:
- Resources (things AI can read)
- Tools (things AI can do)
- Prompts (templated interactions)

**Hypothesis:** The MCP server should expose:
- **Resources:** Current palette, current generator, current terrain state, game config
- **Tools:** `reload_file`, `run_generator`, `query_voxel(x,y,z)`, `restart_game`
- **Prompts:** "Edit palette to add voxel type X", "Regenerate terrain with seed Y"

**Question:** What should the MCP server expose? What queries and actions should AI be able to perform?

**[YOUR ANSWER]:**




---

### Q8: How do we handle the 2D → 3D transition?

**Context:** You want to start 2D for simplicity, then add 3D. But you also said "we don't want the exact same functionality."

**Hypothesis:** 2D and 3D should share:
- Palette definition format
- Generator composition model
- Hot reload mechanism
- MCP server interface

2D and 3D should differ:
- Renderer (obviously)
- Some generator operations (3D has height, caves, etc.)
- Performance characteristics

The transition is: 2D validates the workflow, 3D adds the Z dimension.

**Question:** What specifically should differ between 2D and 3D map editors? What's 2D-only, what's 3D-only, what's shared?

**[YOUR ANSWER]:**




---

## Part III: Physics-Based Mana System

### Q9: How is mana stored?

**Context:** You said mana is energy, and there's "energy allowed per voxel." This could mean:
- **Per-voxel storage:** Each voxel has a mana capacity and current mana
- **Per-region storage:** Chunks or areas have mana pools
- **Per-entity storage:** Players/objects have mana, not terrain

**Hypothesis:** Mana is stored per-voxel, but most voxels have 0. Special voxels (soul stones, crystals) have high mana. Players can extract mana from voxels. The terrain is a mana map.

**Question:** Where does mana live? Per-voxel, per-region, per-entity, or some combination?

**[YOUR ANSWER]:**




---

### Q10: What's the energy cost formula?

**Context:** You said:
- 1,000 small voxels cost more than one 1,000-voxel object
- Heavy/big things cost more to move
- Efficient code costs less mana

**Hypothesis:** Energy cost has components:
- **Mass:** Proportional to voxel count or volume
- **Distance:** Moving things far costs more
- **Fragmentation:** Many small objects cost more than one big object (overhead per object)
- **Computation:** Script complexity maps to mana cost (controversial—how?)

**Question:** What factors should go into the energy cost formula? How do we compute "efficiency"?

**[YOUR ANSWER]:**




---

### Q11: How does computation cost mana?

**Context:** You said "costs proportional to compute costs" and "better harvester brain costs less mana." This implies Lua execution has a mana cost.

**Hypothesis:** We could:
- **Option A:** Count Lua instructions/operations, charge mana per N ops
- **Option B:** Charge per frame that a script runs (simpler)
- **Option C:** Charge for specific expensive operations (A* pathfinding, etc.)
- **Option D:** Don't charge for computation, only for physical actions

**My concern:** Option A is complex and gameable. Option D loses the "efficiency matters" mechanic.

**Question:** How literally do we tie computation to mana cost? What's the mechanism?

**[YOUR ANSWER]:**




---

### Q12: How do power lines work?

**Context:** You mentioned "power lines" for moving energy around. This implies mana isn't just stored—it flows.

**Hypothesis:** Power lines are a type of voxel or structure that:
- Connects mana sources to mana consumers
- Has transmission loss over distance
- Can be built/destroyed
- Creates strategic considerations (protect your power grid)

**Question:** What are power lines mechanically? How do they work?

**[YOUR ANSWER]:**




---

## Part IV: Content and Sharing

### Q13: What's in a package?

**Context:** You want a Lua package manager for sharing. A package could contain:
- Palettes
- Generators
- Creature definitions
- Spell definitions
- Complete worlds
- Libraries of utility functions

**Hypothesis:** A package is a folder with:
- `package.json` manifest (name, version, dependencies)
- `palettes/` folder
- `generators/` folder
- `lib/` folder for shared Lua code
- `README.md` for documentation

**Question:** What's the structure of a shareable package? What can/should be packaged together?

**[YOUR ANSWER]:**




---

### Q14: Where do packages live?

**Context:** You mentioned Git, GCP, and potentially UV (Rust's package manager). Options:
- **Git repos:** Each package is a repo, clone to use
- **Central registry:** Like npm, packages uploaded to a server
- **Embedded in game:** Download from within the game UI
- **Local-first:** Packages are just folders, sharing is manual

**Hypothesis:** Start with Git repos (simple, familiar, AI can clone). Add a registry later if there's demand.

**Question:** Where should packages be hosted? Git repos, central registry, or something else?

**[YOUR ANSWER]:**




---

### Q15: How does search work?

**Context:** You mentioned embedding-based search with LanceDB. This implies semantic search over packages/content.

**Hypothesis:** Search flow:
1. When a package is created, generate embeddings from its code and docs
2. Store embeddings locally and/or in a central index
3. User (or AI) searches by description: "a generator that makes caves"
4. Return ranked results by embedding similarity

**Question:** What should be searchable? Just packages, or also individual components within packages?

**[YOUR ANSWER]:**




---

## Part V: Demos and Marketing

### Q16: Which TikTok idea is most achievable soonest?

**Context:** You listed 9 ideas. Ranking by achievability with current/planned tech:

1. **Two Moons (#8)** - Already have this, just need to record
2. **Markov Jr. Procedural (#9)** - Need 3D rendering + MJ integration
3. **Map Editor (#5)** - Need the map editor working
4. **Physics-Based Mana (#1)** - Need mana system + spell system
5. **Flexifying a Spell (#2)** - Need spell editor
6. **Harry Potter Problem (#4)** - Need mana system to contrast
7. **Weird Shit (#3)** - Need flexible creation tools
8. **Three Ambitious Things (#7)** - Need multiple systems working
9. **Bleak AI Future (#6)** - More essay than demo

**Hypothesis:** Two Moons is free. Markov Jr. Procedural is next closest if we have MJ integration. Map Editor demo requires the most new infrastructure but is foundational.

**Question:** Which demo should we target first? Which gives us the most leverage for future demos?

**[YOUR ANSWER]:**




---

### Q17: What's the "one sentence" pitch?

**Context:** You've described a complex vision. For marketing, we need a simple hook.

**Candidates:**
- "Design your own world with AI, then survive in it"
- "Physics-based magic in a voxel sandbox"
- "What if Minecraft had an AI architect?"
- "The Minecraft difficulty curve, solved with AI"
- "Build worlds with your voice, play in them with your hands"

**Question:** What's the one-sentence pitch for this game?

**[YOUR ANSWER]:**




---

### Q18: What's the 30-second TikTok script?

**Context:** TikTok needs a hook in the first 2 seconds, value in the middle, and a CTA at the end.

**Template:**
```
[0-2s] Hook: Question or surprising claim
[2-15s] Show: Visual demonstration
[15-25s] Explain: Why this matters
[25-30s] CTA: Follow for more / wishlist / etc.
```

**Question:** For your chosen demo (Q16), what's the 30-second script?

**[YOUR ANSWER]:**




---

## Part VI: Roadmap Strategy

### Q19: What are the critical path dependencies?

**Context:** Looking at the milestones from the notes:
1. 2D terrain editor
2. Hot reload
3. MCP server
4. Package manager
5. Persistence/search
6. Documentation
7. 3D upgrade

**Hypothesis:** The critical path is:
```
Lua integration → Palette loading → Generator running → 2D renderer
                                                            ↓
                                        Hot reload → MCP server → 3D
```

Package manager, persistence, and search are parallel/optional for MVP.

**Question:** What's the true critical path? What blocks what?

**[YOUR ANSWER]:**




---

### Q20: What can we cut for V1?

**Context:** The vision is large. For V1, we should cut aggressively.

**Candidates for cutting:**
- [ ] Package manager (just use folders)
- [ ] Search/embeddings (just browse folders)
- [ ] MCP server (just use file watching)
- [ ] 3D (stay 2D for V1)
- [ ] Video export (just screenshot)
- [ ] Creature editor (just terrain for now)
- [ ] Spell system (just world building)

**Question:** What should we explicitly cut from V1? What's V2?

**[YOUR ANSWER]:**




---

### Q21: What's the first commit?

**Context:** After answering these questions, we need to start coding. The first commit should be small and complete.

**Candidates:**
- Add `mlua` crate and load a Lua file
- Create `VoxelPalette` struct and parse from Lua
- Create 2D renderer that displays a grid of colors
- Add file watcher for hot reload
- Create example `p_map_editor_2d.rs`

**Question:** What's the literal first commit we should make?

**[YOUR ANSWER]:**




---

## Part VII: Open Concerns

### Q22: How do we avoid scope creep?

**Context:** Every time we talk, the vision expands. This is good for brainstorming, bad for shipping.

**Hypothesis:** We need a forcing function. Options:
- **Time-box:** "V1 ships in 4 weeks, whatever's done is done"
- **Demo-driven:** "V1 is done when we can record TikTok #X"
- **Feature-freeze:** "V1 is these 5 features, nothing else"

**Question:** How do we constrain scope and actually ship something?

**[YOUR ANSWER]:**




---

### Q23: What's the biggest risk?

**Context:** Complex projects fail for many reasons:
- Technical risk (can we build it?)
- Design risk (will it be fun/useful?)
- Scope risk (will we finish?)
- Market risk (will anyone care?)

**Hypothesis:** The biggest risk is scope—the vision is huge and it's easy to keep adding without finishing anything.

**Question:** What do you think is the biggest risk, and how do we mitigate it?

**[YOUR ANSWER]:**




---

### Q24: What am I (the AI) missing?

**Context:** I've read your notes carefully, but I'm an AI. I might be missing:
- Context you haven't written down
- Emotional priorities that don't show up in technical docs
- Prior art you're referencing
- Constraints you're operating under

**Question:** What am I missing? What haven't you told me that I should know?

**[YOUR ANSWER]:**




---

## Summary: Key Decisions Needed

Before we can write a roadmap, we need answers to at least these questions:

| # | Question | Impact |
|---|----------|--------|
| Q1 | What is the MVP demo? | Determines everything we prioritize |
| Q2 | Is the editor the game? | Determines product structure |
| Q3 | Who is the player? | Determines feature priorities |
| Q4 | What's "done" for V1? | Determines scope |
| Q5 | What's the Lua API shape? | Determines architecture |
| Q9 | How is mana stored? | Determines data model |
| Q16 | Which demo first? | Determines first milestone |
| Q20 | What do we cut? | Determines what we don't build |
| Q21 | What's the first commit? | Determines where we start tomorrow |

---

*Fill in your answers, and we'll build the roadmap from there.*
