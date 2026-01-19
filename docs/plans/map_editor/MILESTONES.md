# Map Editor Milestones

*Functionality-first. Every milestone answers: "What can the user DO?"*

---

## Phase Summaries

| Phase | What I Can Do When This Phase Is Complete |
|-------|-------------------------------------------|
| **1. Foundation** | I can edit Lua materials and generators, see changes live in 2D, and external AI can create assets via MCP. Playback controls let me step through generation. |
| **2. Lua Rendering + Visualization** | I can define how terrain is rendered and visualized in Lua. I can see which rules the generator is executing as it runs. |
| **3. Advanced Generation** | I can load Markov Jr. models, chain generators together, and save/load generation checkpoints—all in 2D. |
| **3.5. Markov Jr. Introspection** | I can see inside Markov Jr. models—their node structure, which rules are firing, with fine-grained step control and dedicated visualization. |
| **4. Unified Asset Store (Database-Backed)** | All my Lua assets (materials, generators, renderers, visualizers) live in a database with unified search. I can browse, search semantically, and AI can write assets that appear automatically. |
| **5. 3D Upgrade** | I can generate and render 3D terrain with PBR materials. Wet stone looks different from dry stone. |
| **6. Polish** | When my Lua script has an error, I see the message and can fix it without losing state. |

---

## Architecture Note: Unified Asset Store

All Lua-defined assets (materials, generators, renderers, visualizers) share a common storage system:

```
AssetStore<T> (generic)
├── get(namespace, path) → T
├── set(namespace, path, T)
├── search(query) → Vec<T>  (text search)
├── search_semantic(query) → Vec<T>  (embedding search)
├── list(namespace, glob_pattern) → Vec<Path>
└── watch(directory)  → auto-import on file write

Backends (progressive):
├── InMemoryStore (Phase 1-3)
├── FileBackedStore (Phase 3)
└── DatabaseStore (Phase 4) ← target for persistence + sharing
```

Type-specific stores (`MaterialStore`, `GeneratorStore`, etc.) are thin wrappers that:
- Add type metadata for construction and filtering
- Define ingestion rules (e.g., how to extract embeddings)
- Share the same underlying storage and search infrastructure

---

## Phase 1: Foundation (2D Map Editor)

### M1: Static End-to-End with Playback Controls
**Functionality:** I can pick from 2 materials, see a checkerboard, and control generation playback.

- Window shows rendered 32x32 grid (2D)
- Material picker with stone, dirt
- Click material → checkerboard changes
- **Playback controls:** Play, Pause, Step, Speed slider
- Can step through generation one step at a time
- Static everything (no Lua, no files)
- Implements: `AssetStore` trait (in-memory), `VoxelGenerator` trait, `VoxelRenderer` trait (all static), playback UI
- `MaterialStore` is first instantiation of generic `AssetStore<Material>`

---

### M2: Lua Materials + Hot Reload
**Functionality:** I can edit a Lua file, save it, and see my materials update without restarting.

- Materials defined in `assets/materials.lua`
- Edit file → app updates within 1 second
- File watcher on assets directory triggers reload
- Implements: `Material` Lua class, file watcher, hot reload system
- Proves: mlua + Bevy integration works
- Store backend: in-memory (files are source of truth, loaded on change)

---

### M3: Lua Generator + Hot Reload
**Functionality:** I can edit a Lua generator script, save it, and see the terrain change without restarting.

- Generator defined in `assets/generator.lua`
- Edit file → terrain regenerates within 1 second
- Implements: `Generator` Lua base class (`init`/`step`/`teardown`), `ctx:set_voxel()` binding

---

### M4: MCP Server (External AI)
**Functionality:** An external AI can create materials and see the rendered output.

- AI calls `create_material` → appears in picker
- AI calls `get_output` → receives PNG
- Implements: MCP server, HTTP endpoints, cross-thread communication

---

## Phase 2: Lua Rendering + Visualization

### M5: Lua Renderer (2D)
**Functionality:** I can define how the 2D grid is rendered in Lua.

- Renderer defined in `assets/renderers/grid_2d.lua`
- Renderer reads voxel buffer, writes to texture
- Edit renderer → hot reload → display updates
- Implements: `Renderer` Lua base class, texture write bindings

---

### M6: Generator Visualizer
**Functionality:** I can see which rules the generator is executing as it runs.

- Visualizer shows current step, active rule, affected region
- For Markov: shows which XML rule matched
- Visualizer defined in Lua, hot reloadable
- Implements: `Visualizer` Lua base class, rule introspection API

**Visualizer API:**
```lua
Visualizer:new()
Visualizer:init(ctx)           -- called once when attached to generator
Visualizer:on_step(ctx, step)  -- called after each generator step
Visualizer:render(ctx)         -- called each frame to draw overlay
-- ctx provides: current_rule, affected_voxels, step_count, generator_state
```

---

### M7: Text Search Across Assets
**Functionality:** I can search for any asset (material, generator, renderer) by name or tag.

- Search by name: "stone" finds stone_indoor, stone_outdoor
- Search by tag: "natural" finds stone, dirt, grass
- Works across all asset types (filter by type if needed)
- Implements: Generic `AssetStore.search(query, type_filter)` API
- Store backend: still in-memory, indexed for search

---

## Phase 3: Advanced Generation (2D)

### M8: Markov Jr. Generator
**Functionality:** I can load a Markov Jr. model and see it generate terrain in 2D.

- Load `dungeon.xml` model
- Watch generation progress step-by-step (use playback controls)
- Final terrain matches model rules
- Implements: `MarkovGenerator` Lua class, Rust Markov Jr. bindings

---

### M9: Composed Generators
**Functionality:** I can chain generators together (base terrain + scatter crystals).

- Sequence generator runs multiple generators in order
- First: Markov generates base terrain
- Second: Scatter places crystals on surface
- All in 2D
- Implements: `SequenceGenerator`, `ScatterGenerator`

---

### M10: Generator Checkpointing
**Functionality:** I can save generation state and resume later.

- Save current state to file
- Load state, continue from where we left off
- Uses existing MJ sim file format
- Implements: State serialization, checkpoint save/load UI

---

## Phase 3.5: Markov Jr. Introspection & Visualization

*Removes the opacity of Markov Jr. models by exposing their internal structure and per-node step info.*

### M10.4: Multi-Surface Rendering Foundation
**Functionality:** I can render to multiple independent textures with separate layer stacks, composited side-by-side for screenshots and video export.

**Foundation:** `RenderSurface` abstraction that decouples render targets from the layer system. Each surface has its own dimensions and layer stack. `FrameCapture` enables video export.

- Create `"grid"` and `"mj_structure"` surfaces with independent layer stacks
- Surfaces composited horizontally (grid: 100x100, mj_structure: 100x100 → 200x100 total)
- MCP `get_output` returns composite; `?surface=grid` returns individual
- `POST /mcp/start_recording` / `export_video` for generation playback videos
- Implements: `RenderSurface`, `RenderSurfaceManager`, `FrameCapture`

---

### M10.5: Markov Jr. Structure Introspection
**Functionality:** I can see the internal node tree of a Markov Jr. model via MCP.

**Foundation:** `MjNode` trait with `structure()` method. All Markov nodes (MarkovNode, SequenceNode, OneNode, etc.) implement this trait. Structure is serializable and returned via `GET /mcp/generator_state`.

- MCP returns nested structure: `{"type":"Sequence","children":[{"type":"Markov","children":[{"type":"One","rules":["WB=WW"]}]}]}`
- Structure includes rule strings for leaf nodes (OneNode, AllNode)
- Node paths follow same convention as Lua generators (e.g., "root.step_1.markov.children[0]")
- Implements: `MjNode::structure()` on all node types, serialization

---

### M10.6: Per-Node Step Info from Markov Jr.
**Functionality:** I can see which specific Markov Jr. node made a change and what rule it applied.

**Foundation:** `ExecutionContext` extended with `current_path` tracking. Nodes push/pop their path during `go()`. Step info includes the full path to the node that made the change.

- Step info includes `path` field showing exact node (e.g., "root.step_1.markov.one[0]")
- Step info includes `rule_name` when applicable (e.g., "WB=WW")
- Multiple nodes in parallel emit separate step info entries
- MCP `steps` field keyed by full path, not just top-level generator path
- Implements: Path tracking in `ExecutionContext`, node-level step emission

---

### M10.7: Markov Jr. Step Budget Control
**Functionality:** I can control exactly how many atomic rule applications happen per frame.

**Foundation:** Step budget system that counts rule applications, not `interpreter.step()` calls. Visualizer can request fine-grained stepping (1 rule at a time) or coarse stepping (100 rules per frame).

- New `step_budget` parameter: "how many rule applications this frame"
- `interpreter.step_n(budget)` runs until budget exhausted or completion
- Each rule application emits step info (path, rule, affected cells)
- Playback UI can set budget: single-step mode (budget=1) vs fast-forward (budget=1000)
- Implements: Budget-aware stepping, step info per rule application

---

### M10.8: Markov Jr. Visualizer Layer
**Functionality:** I can see a real-time overlay showing which Markov Jr. nodes are active and what rules are firing.

**Foundation:** Dedicated visualizer texture that renders the Markov Jr. structure tree and highlights active nodes based on step info paths.

- Visualizer shows node tree on screen (structure from M10.5)
- Active node highlighted based on most recent step info path (from M10.6)
- Rule being applied shown next to active node
- Affected cells highlighted on the main grid
- Works with step budget from M10.7 for smooth animation
- Implements: `MjVisualizerLayer` Lua renderer, structure-aware layout

---

## Phase 4: Unified Asset Store (Database-Backed)

### M11: Database-Backed Asset Store
**Functionality:** All my Lua assets persist in a database, not just files.

- Assets stored in SQLite with namespace as key
- Namespace format: `username/folder/name` (slashes allowed)
- No collisions between users
- Same `AssetStore` API, different backend
- Implements: `DatabaseStore` backend, migration from file-backed

---

### M12: Semantic Search Across All Assets
**Functionality:** I can search for any asset by description, not just name.

- Search "something shiny for a cave" → finds crystal material, glow renderer
- Embeddings stored in database alongside assets
- Works across all asset types (filter by type if needed)
- AI can use search via MCP tool
- Implements: Embedding generation, vector similarity in `AssetStore.search_semantic()`

---

### M13: Asset Browser Panel
**Functionality:** I can browse all my assets in one panel with folder navigation.

- Tree view with folders (namespaced paths)
- Click to load any asset (material, generator, renderer, visualizer)
- Shows all asset types in one place
- MCP tool can write new assets → appear in browser automatically
- Implements: Asset browser UI, database query for listing

---

### M14: File Watcher Auto-Import
**Functionality:** When I (or AI) write a file to a watched directory, it auto-imports to the database.

- Watch `assets/incoming/` directory
- File path becomes namespace key: `assets/incoming/paul/crystal.lua` → `paul/crystal`
- File type detected from content or extension
- Implements: Directory watcher, auto-import to database

---

## Phase 5: 3D Upgrade

### M15: 3D Voxel Buffer
**Functionality:** I can generate a 3D terrain (not just 2D slice).

- VoxelBuffer now has X, Y, Z dimensions
- Generator writes to full 3D volume
- Same generator scripts work (just add Y loops)
- Implements: `VoxelBuffer3D`, extended generator context

---

### M16: 3D Rendering (Deferred Pipeline)
**Functionality:** I can see my 3D terrain rendered with full PBR lighting.

- Voxels meshed and rendered in 3D
- Orbit camera to inspect from all angles
- Implements: `DeferredRenderer3D`, mesh generation from voxel buffer

---

### M17: PBR Materials (Wet vs Dry)
**Functionality:** I can see the difference between wet stone and dry stone in 3D.

- Materials have roughness/metallic properties
- Wet stone (roughness 0.3) shows specular highlights
- Dry stone (roughness 0.7) looks matte
- Side-by-side comparison clearly visible in 3D
- Implements: Extended vertex format, G-buffer material output, GGX specular

---

### M18: 3D Material Preview
**Functionality:** I can see floating voxel samples above my terrain preview.

- Each material shown as a floating cube
- Cubes slowly rotate to show all angles
- Can see specular highlights on wet materials
- Terrain preview below the samples
- Implements: Voxel sample renderer, split viewport

---

## Phase 6: Error Handling + Polish

### M19: Error Recovery
**Functionality:** When my Lua script has an error, I see the error message and can fix it.

- Syntax error → error displayed in UI, previous state preserved
- Runtime error → error with line number, terrain unchanged
- Fix script → hot reload, terrain updates
- Implements: Lua error capture, UI error display, graceful fallback

---

## Future Work

### World Configuration

**Lighting Configuration:** Change moon color and position from config file.

**Atmosphere Configuration:** Change fog density and color from config file.

**Post-Processing Configuration:** Tweak crushed blacks, film grain, vignette.

---

### Play Mode

**Camera Switch:** Switch from editor camera to player camera.

**Walking on Terrain:** Walk on the terrain with collision and gravity.

**Terrain Interaction:** Break and place voxels in play mode.

---

### Demo + Documentation

**Full Demo Scene:** Complete AI-assisted world creation demo flow.

**Demo Recording:** Record editing session as video.

**Documentation:** Getting Started guide, API references.

---

### Extended Features

**Import from URL:** Import palette or generator from GitHub URL.

**Version Control:** Asset versioning with undo/history. Scope TBD—may integrate with Git (assets as repos) or custom versioning in database. Will interact with package management system.

**Package Management:** Share and import asset packages. Namespace-aware, version-aware. Design must consider from start even if implemented later.

### Creature Editor Integration
**Functionality:** I can place creatures I've designed into my terrain.

### Spell Testing
**Functionality:** I can test spells in my terrain.

### Multiplayer Preview
**Functionality:** I can invite someone else to see my world while I edit.

---

## Summary Table

| M# | Functionality | Phase |
|----|---------------|-------|
| 1 | Pick materials, see checkerboard, playback controls | Foundation |
| 2 | Edit Lua materials, see update live | Foundation |
| 3 | Edit Lua generator, see terrain change live | Foundation |
| 4 | External AI creates assets via MCP | Foundation |
| 5 | Define 2D renderer in Lua | Lua Rendering |
| 6 | See which rules generator is executing (visualizer) | Lua Rendering |
| 7 | Search any asset by name/tag | Lua Rendering |
| 8 | Load Markov Jr. model in 2D | Generation |
| 9 | Chain generators together | Generation |
| 10 | Save/load generation checkpoints | Generation |
| 10.4 | Render to multiple surfaces, video export | MJ Introspection |
| 10.5 | See internal Markov Jr. node tree via MCP | MJ Introspection |
| 10.6 | See which Markov Jr. node made each change | MJ Introspection |
| 10.7 | Control step budget (rules per frame) | MJ Introspection |
| 10.8 | Visualizer overlay for Markov Jr. structure | MJ Introspection |
| 11 | All assets persist in database | Unified Store |
| 12 | Semantic search across all assets | Unified Store |
| 13 | Browse all assets in one panel | Unified Store |
| 14 | File watcher auto-imports to database | Unified Store |
| 15 | Generate 3D terrain | 3D Upgrade |
| 16 | Render 3D with deferred pipeline | 3D Upgrade |
| 17 | PBR materials (wet vs dry) in 3D | 3D Upgrade |
| 18 | Floating voxel samples in 3D preview | 3D Upgrade |
| 19 | Error recovery with graceful fallback | Polish |

---

## Critical Path

The minimum path to "AI-assisted 2D world creation with persistent storage":

**M1 → M2 → M3 → M4** (Foundation: 4 milestones)
- Working 2D editor with hot reload, playback controls, and AI access via MCP

**→ M5 → M6 → M7** (Lua Rendering: 3 milestones)  
- Lua-defined renderer, visualizer, and text search

**→ M8 → M9 → M10** (Generation: 3 milestones)
- Markov Jr., composed generators, and checkpointing in 2D

**→ M11 → M12 → M13 → M14** (Unified Store: 4 milestones)
- Database-backed storage, semantic search, browser, file watcher

**Total 2D critical path: 14 milestones**

For 3D demo, add:

**→ M15 → M16 → M17 → M18** (3D: 4 milestones)
- 3D voxels, rendering, PBR, preview

**Total 3D critical path: 18 milestones**

---

## Store Backend Progression

| Phase | Backend | Persistence | Search |
|-------|---------|-------------|--------|
| 1-3 | In-memory | Files are source of truth | Text (in-memory index) |
| 4+ | SQLite | Database is source of truth | Text + Semantic (embeddings) |

The `AssetStore` API remains consistent across backends. MCP tools use the same API regardless of backend.
