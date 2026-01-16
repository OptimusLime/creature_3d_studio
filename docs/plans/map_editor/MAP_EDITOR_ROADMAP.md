# Map Editor and AI-Driven Voxel Terrain Demo

## Roadmap and Comprehensive Plan

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Why This Matters](#2-why-this-matters)
3. [Core Vision](#3-core-vision)
4. [High-Level Pillars](#4-high-level-pillars)
5. [The Map Editor Object](#5-the-map-editor-object)
6. [Voxel Palette System](#6-voxel-palette-system)
7. [Material Properties (PBR)](#7-material-properties-pbr)
8. [Terrain Generation Pipeline](#8-terrain-generation-pipeline)
9. [Hot Reloading Architecture](#9-hot-reloading-architecture)
10. [Scripting Model](#10-scripting-model)
11. [AI Assistant Integration](#11-ai-assistant-integration)
12. [Demo: AI-Edited Voxel Terrain](#12-demo-ai-edited-voxel-terrain)
13. [UI and Visualization](#13-ui-and-visualization)
14. [File Format Specifications](#14-file-format-specifications)
15. [Alignment with Fidelity and Optimization](#15-alignment-with-fidelity-and-optimization)
16. [Core Gameplay Loop](#16-core-gameplay-loop)
17. [Future Integration: opencode.ai](#17-future-integration-opencodeai)
18. [Implementation Phases](#18-implementation-phases)
19. [Success Criteria](#19-success-criteria)
20. [Suggested Follow-Up Documents](#20-suggested-follow-up-documents)

---

## 1. Executive Summary

This document describes the **Map Editor**, a critically important system that enables AI-driven world design for Creature 3D Studio. The Map Editor is not a minor feature—it is central to the entire creative pipeline and represents a fundamental shift in how worlds are constructed.

### What It Is

The Map Editor is an integrated toolset that allows:
- Definition of voxel palettes with full material properties (color, roughness, metallic, emission)
- Construction of terrain using those palettes via procedural generation (Markov models)
- Real-time visualization of both the palette and the resulting terrain
- Hot reloading of all definitions from disk
- AI-assisted editing of all world components

### Why It Matters

Without proper material properties, indoor spaces look wet when they should be dry. Stone looks the same as metal. Wood looks the same as crystal. This is unacceptable for any world that contains both interior and exterior spaces, which is to say, any real game world.

The Map Editor solves this by making material definition first-class and AI-editable.

### Core Deliverable

A working example (`p_map_editor.rs` or similar) that demonstrates:
1. A voxel palette displayed in UI and rendered in 3D
2. A terrain generated from that palette
3. Hot reloading when palette files change on disk
4. AI editability of palette and terrain definitions

---

## 2. Why This Matters

### 2.1 The Indoor/Outdoor Problem

Consider a simple scenario: a house in a field.

**Without material properties:**
- The field is wet (low roughness) - correct, it's raining
- The inside of the house is wet (low roughness) - WRONG, it's indoors
- Stone floor inside looks identical to stone path outside - WRONG

**With material properties:**
- Outdoor stone: roughness 0.3 (wet), metallic 0.0
- Indoor stone: roughness 0.7 (dry), metallic 0.0
- Metal lamp post: roughness 0.4, metallic 0.9
- Wooden beam: roughness 0.8, metallic 0.0

This isn't a nice-to-have. It's fundamental to creating believable spaces.

### 2.2 The Content Pipeline Problem

Currently, voxels are defined in Rust code. To add a new voxel type or change its properties requires:
1. Editing Rust source
2. Recompiling
3. Restarting the application

This is unacceptable for creative iteration. The Map Editor solves this by:
1. Defining voxels in external files (Lua, TOML, JSON, or similar)
2. Hot reloading when files change
3. Enabling AI to edit those files directly

### 2.3 The AI Editing Problem

We want AI to help design worlds. For AI to edit worlds, it needs:
1. A file format it can read and write
2. Clear semantics for what each property means
3. Immediate feedback on changes (hot reload)

The Map Editor provides all three.

### 2.4 The Demo Problem

We need to demonstrate the system works. The best demonstration is:
1. Show the voxel palette
2. Show terrain built from that palette
3. Have AI modify the palette
4. See the terrain update

This is the core demo we are building toward.

---

## 3. Core Vision

### 3.1 Design Your Own World with AI

The player/creator opens the Map Editor. They see:
- A palette of available voxel types
- A terrain preview showing those voxels in use
- An AI assistant interface (initially external, later integrated)

The creator says to the AI: "Make the stone darker and add a glowing crystal type."

The AI:
1. Edits the palette file on disk
2. The Map Editor detects the change
3. Hot reloads the palette
4. Re-renders the terrain with updated voxels

The creator sees the change in real-time.

### 3.2 Survive and Play in Your World

Once the world is designed, the creator can:
- Switch from editor mode to play mode
- Control a character in the world they designed
- Experience the lighting, materials, and atmosphere firsthand

### 3.3 Build and Craft Spells

A separate demo (future work) allows:
- Designing spell effects using similar AI-driven tools
- Combining voxel types with particle effects
- Testing spells in the designed world

---

## 4. High-Level Pillars

The Map Editor exists at the intersection of three pillars:

### 4.1 Fidelity

Visual quality improvements must be expressible through the Map Editor:
- PBR material properties (roughness, metallic) enable wet/dry distinction
- Emission properties enable glowing voxels
- Color properties enable artistic control

Every fidelity feature should be configurable per-voxel-type in the palette.

### 4.2 Optimization

Performance must be maintained as content scales:
- Efficient voxel mesh generation with material data
- LOD systems that respect material properties
- Streaming that loads palette data on demand

Optimization work must not break the Map Editor's hot reload capability.

### 4.3 Specific Demos

The Map Editor is itself a demo, and enables other demos:
- **Demo 1:** Voxel Inspector - view and edit palette
- **Demo 2:** AI Terrain Editor - AI modifies terrain via file edits
- **Demo 3:** Full Map Editor - integrated world design tool
- **Demo 4:** Spell Crafter - design effects using similar patterns

---

## 5. The Map Editor Object

### 5.1 Definition

The Map Editor is a cohesive system (an "object" in the design sense) comprising:

| Component | Responsibility |
|-----------|----------------|
| Voxel Palette | Defines what voxels exist and their properties |
| Terrain Generator | Constructs terrain from palette using rules |
| Visualizer | Renders palette and terrain for inspection |
| File Watcher | Detects changes to definition files |
| Hot Reloader | Updates runtime state from changed files |
| UI Layer | ImGui interface for inspection and control |
| AI Interface | Connection point for AI editing (initially external) |

### 5.2 Scope

The Map Editor controls:
- Voxel definitions (types, colors, materials)
- Terrain generation rules (Markov models, noise parameters)
- Lighting configuration (moon colors, positions, intensities)
- Atmosphere settings (fog density, tint, height)
- World bounds and chunking

The Map Editor does NOT control (for now):
- Player mechanics
- Spell systems
- Entity behavior
- Networking

### 5.3 Boundaries

**In scope for initial version:**
- Voxel palette hot reloading
- Single terrain preview
- ImGui-based UI
- File-based AI interaction

**Out of scope for initial version:**
- In-game AI assistant UI
- Multi-terrain support
- Undo/redo system
- Collaborative editing

---

## 6. Voxel Material System (Database-First)

### 6.1 Why Database, Not Files?

Storing materials as Lua files is short-sighted. Materials may have:
- Custom textures
- Associated assets (normal maps, etc.)
- Relationships to other materials
- Embeddings for semantic search
- Version history

A file-per-palette approach doesn't scale and makes search/query impossible.

**The correct approach:** Store all materials in an embedded database from the start.

### 6.2 What Is a Material?

A material (voxel type) is a database record with these properties:

| Property | Type | Description |
|----------|------|-------------|
| id | u64 | Unique identifier (auto-generated) |
| name | string | Human-readable name ("dark_stone", "glowing_crystal") |
| color | RGB | Base albedo color |
| roughness | f32 | Surface roughness (0.0 = mirror, 1.0 = matte) |
| metallic | f32 | Metallic factor (0.0 = dielectric, 1.0 = metal) |
| emission | f32 | Emission intensity (0.0 = none, 1.0 = full glow) |
| emission_color | RGB | Color of emission (if different from base) |
| tags | list | Semantic tags ("solid", "transparent", "liquid") |
| description | string | Human-readable description for search |
| embedding | vec<f32> | Semantic embedding for similarity search |
| texture_id | u64? | Optional reference to custom texture |
| created_at | timestamp | When this material was created |
| updated_at | timestamp | When this material was last modified |

### 6.3 What Is a Palette?

A palette is NOT a file—it's a database query result or a saved collection:

```sql
-- A palette is just a set of material IDs
CREATE TABLE palettes (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE palette_materials (
    palette_id INTEGER REFERENCES palettes(id),
    material_id INTEGER REFERENCES materials(id),
    local_id INTEGER,  -- The ID used within this palette (0-255 typically)
    PRIMARY KEY (palette_id, material_id)
);
```

A palette is a construction of individual materials, not a monolithic definition.

### 6.4 Database Architecture

**Storage:** SQLite with vector extension (for embeddings) or LanceDB

**In-memory:** Loaded at startup, kept in sync with on-disk database

```rust
/// The material database - constructed at startup, queryable at runtime
#[derive(Resource)]
pub struct MaterialDatabase {
    /// SQLite connection (or LanceDB handle)
    db: Connection,
    /// In-memory cache for fast lookup during rendering
    cache: HashMap<u64, Material>,
    /// Embedding index for semantic search
    embedding_index: EmbeddingIndex,
}

impl MaterialDatabase {
    /// Create a new material, returns its ID
    pub fn create_material(&mut self, material: MaterialDef) -> u64;
    
    /// Query materials by semantic similarity
    pub fn search(&self, query: &str, limit: usize) -> Vec<Material>;
    
    /// Query materials by tag
    pub fn find_by_tag(&self, tag: &str) -> Vec<Material>;
    
    /// Get a specific material by ID
    pub fn get(&self, id: u64) -> Option<&Material>;
    
    /// Update a material
    pub fn update(&mut self, id: u64, material: MaterialDef);
    
    /// Create or get a palette
    pub fn create_palette(&mut self, name: &str) -> u64;
    
    /// Add material to palette
    pub fn add_to_palette(&mut self, palette_id: u64, material_id: u64, local_id: u8);
}
```

### 6.5 MCP Interface for AI

AI interacts with materials via MCP server calls, NOT file edits:

```
// MCP Tool: create_material
{
  "name": "stone_outdoor",
  "color": [0.3, 0.3, 0.35],
  "roughness": 0.3,
  "metallic": 0.0,
  "tags": ["solid", "natural", "outdoor"],
  "description": "Wet outdoor stone, darkened by rain"
}
// Returns: { "id": 12345 }

// MCP Tool: search_materials
{
  "query": "wood",
  "limit": 10
}
// Returns: [{ "id": 101, "name": "dark_wood", ... }, ...]

// MCP Tool: create_palette
{
  "name": "dark_fantasy",
  "material_ids": [12345, 12346, 12347]
}
// Returns: { "palette_id": 1 }
```

### 6.6 Why This Is Better

| File-Based (Old) | Database-Based (New) |
|------------------|----------------------|
| Search = grep through files | Search = semantic query |
| Add material = edit Lua file | Add material = API call, get ID back |
| Custom textures = ??? | Custom textures = blob storage with foreign key |
| AI edits files | AI calls MCP tools |
| No history | Full version history |
| No relationships | Materials can reference each other |
| Palettes are monolithic | Palettes are composable queries |

### 6.7 Hot Reload Behavior

Hot reload still works, but the trigger is different:
- **Old:** File watcher detects `.lua` change
- **New:** Database write triggers update event

```rust
// When a material is created/updated via MCP
fn on_material_changed(
    mut events: EventReader<MaterialChangedEvent>,
    mut terrain: ResMut<Terrain>,
    db: Res<MaterialDatabase>,
) {
    for event in events.read() {
        // Refresh the in-memory cache
        db.refresh_cache(event.material_id);
        // Re-mesh terrain if this material is in use
        if terrain.uses_material(event.material_id) {
            terrain.schedule_remesh();
        }
    }
}
```

### 6.8 Migration Path

For existing Lua palette files (if any):
1. Parse the Lua file
2. Insert each voxel definition as a material in the database
3. Create a palette record linking those materials
4. Delete the Lua file (or keep as backup)

This is a one-time migration, not an ongoing workflow.

---

## 7. Material Properties (PBR)

### 7.1 Why PBR Materials

Physically Based Rendering (PBR) uses a standard set of material properties that produce consistent, believable results across different lighting conditions. The two key properties are:

**Roughness:** How rough or smooth a surface is
- 0.0 = Perfect mirror (polished metal, still water)
- 0.3 = Wet stone, glazed ceramic
- 0.5 = Plastic, satin finish
- 0.7 = Rough stone, unfinished wood
- 1.0 = Chalk, matte cloth

**Metallic:** Whether the surface is metallic or dielectric
- 0.0 = Non-metal (stone, wood, plastic, skin)
- 1.0 = Metal (iron, gold, copper, aluminum)
- 0.3-0.7 = Partially metallic (some crystals, coated metals)

### 7.2 Pipeline Changes Required

To support PBR materials per-voxel, we need changes to:

**Vertex Format (current → new):**
```
Current: 44 bytes
- Position: Float32x3 (12 bytes)
- Normal: Float32x3 (12 bytes)
- Color: Float32x3 (12 bytes)
- Emission: Float32 (4 bytes)
- AO: Float32 (4 bytes)

New: 52 bytes (+18%)
- Position: Float32x3 (12 bytes)
- Normal: Float32x3 (12 bytes)
- Color: Float32x3 (12 bytes)
- Emission: Float32 (4 bytes)
- AO: Float32 (4 bytes)
- Roughness: Float32 (4 bytes)  [NEW]
- Metallic: Float32 (4 bytes)   [NEW]
```

**G-Buffer (current → new):**
```
Current: 3 MRTs
- gColor: RGBA16F (RGB=albedo, A=emission)
- gNormal: RGBA16F (RGB=normal, A=AO)
- gPosition: RGBA32F (XYZ=position, W=depth)

New Option A: Pack into existing (complex)
- gColor: RGBA16F (RGB=albedo, A=emission)
- gNormal: RGBA16F (RG=normal_xy, B=roughness, A=metallic)
  - Reconstruct normal_z from xy
  - AO comes from GTAO pass

New Option B: Add 4th MRT (cleaner)
- gColor: RGBA16F (RGB=albedo, A=emission)
- gNormal: RGBA16F (RGB=normal, A=AO)
- gPosition: RGBA32F (XYZ=position, W=depth)
- gMaterial: RG16F (R=roughness, G=metallic)  [NEW]
```

**Lighting Shader:**
- Port GGX specular functions from Bevy's `pbr_lighting.wgsl`
- Read roughness/metallic from G-buffer
- Calculate specular per-light using Cook-Torrance BRDF

### 7.3 Material Data Flow

```
MCP Call (AI or User)
       ↓
MaterialDatabase (SQLite/LanceDB)
       ↓
MaterialChangedEvent (Bevy)
       ↓
In-Memory Cache Refresh
       ↓
Mesh Generation (CPU)
  - Looks up material per voxel type
  - Writes roughness/metallic to vertex
       ↓
Vertex Buffer (GPU)
       ↓
G-Buffer Pass (GPU)
  - Reads roughness/metallic from vertex
  - Writes to gMaterial MRT
       ↓
Lighting Pass (GPU)
  - Reads roughness/metallic from gMaterial
  - Calculates PBR specular
       ↓
Final Image
```

### 7.4 Default Materials

If a voxel type doesn't specify material properties:
- `roughness = 0.5` (neutral)
- `metallic = 0.0` (non-metal)

This ensures backwards compatibility with existing voxel data.

---

## 8. Generator API

*See `API_DESIGN.md` for full API specification.*

### 8.1 Philosophy

Generators are **classes**, not configuration files. You extend a base class and implement methods:

- `init(ctx)` - Setup, returns "ready" or "error"
- `step(ctx)` - Run one step, returns "done", "continue", or "error"
- `post_process(ctx)` - Optional, after all steps
- `teardown(ctx)` - Cleanup

This supports ANY generation method:
- Direct voxel writing
- Markov Jr. models
- Random placement/scatter
- Noise functions
- Compute shaders
- Live/streaming generation
- Custom algorithms

### 8.2 Rust Trait

```rust
pub trait VoxelGenerator {
    fn init(&mut self, ctx: &mut GeneratorContext) -> GeneratorState;
    fn step(&mut self, ctx: &mut GeneratorContext) -> StepResult;
    fn teardown(&mut self, ctx: &mut GeneratorContext);
    fn post_process(&mut self, _ctx: &mut GeneratorContext) {}
}

pub enum StepResult {
    Done,
    Continue,
    Error(String),
}
```

### 8.3 Lua Base Class

```lua
local Generator = {}
Generator.__index = Generator

function Generator:new()
    return setmetatable({}, self)
end

function Generator:init(ctx) return "ready" end
function Generator:step(ctx) return "done" end
function Generator:post_process(ctx) end
function Generator:teardown(ctx) end

-- Utilities available to all generators
function Generator:set_voxel(ctx, x, y, z, material_id)
    _rust_voxel_set(ctx.voxels, x, y, z, material_id)
end

function Generator:get_voxel(ctx, x, y, z)
    return _rust_voxel_get(ctx.voxels, x, y, z)
end

return Generator
```

### 8.4 Example: Direct Writer

```lua
local Generator = require("generator")
local DirectWriter = Generator:new()

function DirectWriter:step(ctx)
    for x = ctx.bounds.min.x, ctx.bounds.max.x do
        for z = ctx.bounds.min.z, ctx.bounds.max.z do
            self:set_voxel(ctx, x, 0, z, self.ground_material)
        end
    end
    return "done"
end

return DirectWriter
```

### 8.5 Example: Markov Jr. Wrapper

```lua
local Generator = require("generator")
local MarkovGenerator = Generator:new()

function MarkovGenerator:new(model_name)
    local instance = Generator.new(self)
    instance.model_name = model_name
    return instance
end

function MarkovGenerator:init(ctx)
    self.state = _rust_markov_init(self.model_name, ctx.seed, ctx.bounds)
    return self.state and "ready" or "error: failed to load model"
end

function MarkovGenerator:step(ctx)
    local result = _rust_markov_step(self.state)
    if result == "done" then
        _rust_markov_copy_to_voxels(self.state, ctx.voxels)
    end
    return result
end

function MarkovGenerator:teardown(ctx)
    _rust_markov_destroy(self.state)
end

return MarkovGenerator
```

### 8.6 Example: Composed Sequence

```lua
local Sequence = require("generators/sequence")
local Markov = require("generators/markov")
local Scatter = require("generators/scatter")

local terrain = Sequence:new({
    Markov:new("base_terrain.xml"),
    Scatter:new(crystal_id, 0.01, true),  -- surface scatter
})
```

### 8.7 Generation Flow

```
Lua Script (defines Generator subclass)
       ↓
Generator:init(ctx)
       ↓
Generator:step(ctx) [loops until "done"]
       ↓
Generator:post_process(ctx)
       ↓
Generator:teardown(ctx)
       ↓
VoxelBuffer ready for rendering
```

### 8.8 Hot Reloading

When a generator script changes:
1. Teardown current generator
2. Re-load Lua script
3. Re-instantiate generator class
4. Re-run from init with same seed (reproducible)

---

## 9. Hot Reloading Architecture

### 9.1 Two Reload Triggers

Hot reload is triggered by TWO mechanisms:

1. **Database writes** (for materials)
   - MCP call → MaterialStore write → MaterialChangedEvent
   - No file watching needed

2. **File changes** (for generator scripts, config)
   - File watcher detects `.lua` changes
   - Script re-loaded, class re-instantiated

### 9.2 Material Reload (Database-Triggered)

```rust
fn handle_material_change(
    mut events: EventReader<MaterialChangedEvent>,
    material_db: Res<MaterialDatabase>,
    mut terrain: ResMut<Terrain>,
) {
    for event in events.read() {
        // Refresh in-memory cache
        material_db.refresh_cache(event.material_id);
        
        // Re-mesh if material is in use
        if terrain.uses_material(event.material_id) {
            terrain.schedule_remesh();
        }
    }
}
```

### 9.3 Generator Reload (File-Triggered)

```rust
fn handle_generator_reload(
    mut events: EventReader<ScriptChangedEvent>,
    mut generator_manager: ResMut<GeneratorManager>,
) {
    for event in events.read() {
        if event.path.ends_with(".lua") {
            // Teardown current generator
            generator_manager.teardown_current();
            
            // Re-load script
            generator_manager.load_script(&event.path);
            
            // Re-run with same seed (reproducible)
            generator_manager.run_with_seed(generator_manager.last_seed);
        }
    }
}
```

### 9.4 Watched Paths

File watcher monitors:
- `assets/generators/*.lua` - Generator class definitions
- `assets/renderers/*.lua` - Renderer class definitions
- `assets/config/*.lua` - Configuration scripts

**NOT watched** (uses database instead):
- Materials (via MaterialStore)
- Palettes (via PaletteStore)

### 9.5 Cascading Reloads

| Change | Effect |
|--------|--------|
| Material updated | Re-mesh terrain if material in use |
| Generator script changed | Teardown, reload, re-run from init |
| Renderer script changed | Teardown, reload, re-init |
| Config changed | Update uniforms immediately |

### 9.6 What Requires Restart

Changes requiring app restart (new Rust code):
- New Rust traits / API methods
- New vertex attributes
- New G-buffer formats
- New render pipeline stages

These should be rare once the system is mature.

---

## 10. Scripting Model

*See `API_DESIGN.md` for full API specification.*

### 10.1 Philosophy: Classes, Not Data Tables

Scripts define **classes that extend base classes**, not data tables.

**Wrong approach (shitty JSON):**
```lua
-- DON'T do this - it's just a config file
return {
  name = "terrain",
  model = "dungeon.xml",
  post_process = { ... }
}
```

**Right approach (extensible classes):**
```lua
-- DO this - it's a class with behavior
local Generator = require("generator")
local MyTerrain = Generator:new()

function MyTerrain:init(ctx)
    self.markov = _rust_markov_init("dungeon.xml", ctx.seed)
    return "ready"
end

function MyTerrain:step(ctx)
    return _rust_markov_step(self.markov)
end

return MyTerrain
```

### 10.2 Why Classes

Classes enable:
- Method overriding (customize behavior)
- Composition (combine generators)
- State management (instance variables)
- Rust trait mapping (Lua class ↔ Rust trait)

Data tables are just config. Classes are behavior.

### 10.3 Base Classes (Provided by Engine)

```lua
-- generator.lua (base class)
local Generator = {}
Generator.__index = Generator
function Generator:new() return setmetatable({}, self) end
function Generator:init(ctx) return "ready" end
function Generator:step(ctx) return "done" end
function Generator:post_process(ctx) end
function Generator:teardown(ctx) end
return Generator

-- renderer.lua (base class)
local Renderer = {}
Renderer.__index = Renderer
function Renderer:new() return setmetatable({}, self) end
function Renderer:init(ctx) return "ready" end
function Renderer:render(ctx) return "ok" end
function Renderer:teardown(ctx) end
return Renderer
```

### 10.4 User Scripts (Extend Base Classes)

```lua
-- my_terrain.lua
local Generator = require("generator")
local Scatter = require("generators/scatter")

local MyTerrain = Generator:new()

function MyTerrain:init(ctx)
    -- Direct voxel access - nothing stopping you
    for x = 0, ctx.bounds.max.x do
        self:set_voxel(ctx, x, 0, 0, 1)  -- ground layer
    end
    return "ready"
end

function MyTerrain:step(ctx)
    -- Could use Markov, noise, shaders, anything
    return "done"
end

return MyTerrain
```

### 10.5 Rust Bindings

Lua classes call into Rust via exposed functions:

```rust
// Exposed to Lua as _rust_voxel_set
fn lua_voxel_set(voxels: &mut VoxelBuffer, x: i32, y: i32, z: i32, id: u16) {
    voxels.set(x, y, z, id);
}

// Exposed to Lua as _rust_markov_init
fn lua_markov_init(model: &str, seed: u64, bounds: Bounds3D) -> MarkovState {
    MarkovState::new(model, seed, bounds)
}
```

### 10.6 Error Handling

Script errors:
- Logged with file/line information
- Displayed in UI
- Do NOT crash the application
- Keep previous valid state

```
[ERROR] my_terrain.lua:15
  attempt to call nil value 'set_voxel'
  Generator unchanged.
```

---

## 11. AI Assistant Integration

### 11.1 Current State: External AI

For the initial version, the AI assistant runs externally:
- User has opencode/Claude open in terminal
- AI reads files from disk
- AI writes modified files to disk
- Map Editor hot reloads the changes

This is functional and demonstrates the concept without building AI UI.

### 11.2 Interaction Flow (External)

```
User: "Add a glowing blue crystal voxel to the palette"
       ↓
AI: Reads assets/palettes/dark_fantasy.lua
       ↓
AI: Modifies file, adds new voxel definition
       ↓
AI: Writes updated file to disk
       ↓
File Watcher: Detects change
       ↓
Map Editor: Reloads palette
       ↓
Visualizer: Shows new crystal in palette view
       ↓
Terrain: Re-meshes with new voxel available
```

### 11.3 Future State: In-Game AI

Later, the AI assistant will be accessible from within the game:
- Chat interface in ImGui
- Direct API calls to opencode.ai backend
- Real-time file modifications
- Immediate visual feedback

### 11.4 AI-Editable Components

The AI should be able to edit:
- Voxel palettes (add/modify/remove voxel types)
- Terrain definitions (rules, patterns, post-processing)
- Lighting configuration (moon colors, fog settings)
- World parameters (size, seed, biome)

The AI should NOT (initially) edit:
- Rust source code
- Shader code
- Binary assets

### 11.5 AI Context

For effective editing, the AI needs context:
- Current palette contents
- Current terrain definition
- Available voxel tags and their meanings
- Material property ranges and their visual effects

This context can be provided via:
- System prompts when calling the AI
- Structured comments in the Lua files
- Separate documentation files the AI can reference

---

## 12. Demo: AI-Edited Voxel Terrain

### 12.1 Demo Overview

The primary demo showcasing the Map Editor:

**Title:** "AI-Driven Voxel World Design"

**Duration:** 2-3 minutes

**Narrative:**
1. Open the Map Editor example
2. Show the voxel palette (UI + 3D preview)
3. Show terrain built from palette
4. External AI modifies palette file
5. See palette and terrain update in real-time
6. External AI modifies terrain rules
7. See new terrain generate
8. Switch to play mode, walk through the world

### 12.2 Demo Script

```
[00:00] Launch p_map_editor example
        - Window shows: palette UI (left), 3D view (right)
        - 3D view has: floating voxel samples (top), terrain (bottom)

[00:15] Narrate palette structure
        - "Each voxel type has color, roughness, metallic, emission"
        - "Stone_outdoor is wet (0.3 roughness), stone_indoor is dry (0.7)"
        - Highlight the difference visually

[00:30] Show AI interaction (terminal visible alongside game window)
        - "Let's ask AI to add a new crystal type"
        - Type: "Add a green crystal voxel with high emission"

[00:45] AI writes file, Map Editor reloads
        - New crystal appears in palette UI
        - New crystal sample appears in 3D view
        - "The palette hot reloaded from disk"

[01:00] Ask AI to place crystals in terrain
        - "Scatter green crystals on the terrain surface"
        - AI modifies terrain.lua post_process rules

[01:15] Terrain regenerates
        - Green crystals visible on terrain
        - They're emitting light, casting glow

[01:30] Ask AI to change atmosphere
        - "Make the fog denser and green-tinted"
        - AI modifies config.lua

[01:45] Atmosphere updates
        - Fog visibly thicker
        - Green tint visible
        - "All without touching code or recompiling"

[02:00] Switch to play mode
        - Camera becomes player controller
        - Walk through the terrain
        - "This is the world we designed with AI"

[02:30] End
```

### 12.3 Demo Requirements

For the demo to work, we need:
- [ ] Palette loading from Lua files
- [ ] Palette hot reloading
- [ ] Palette UI display (ImGui)
- [ ] Palette 3D preview (floating voxel samples)
- [ ] Terrain generation from palette
- [ ] Terrain hot reloading
- [ ] PBR materials rendering correctly
- [ ] Lighting config hot reloading
- [ ] Play mode camera switch

### 12.4 Demo Fallbacks

If something doesn't work in time:
- Pre-record the AI interaction (show as video-in-video)
- Use simpler terrain (flat plane with voxels)
- Skip play mode (just show editor)

---

## 13. UI and Visualization

### 13.1 ImGui Layout

```
+------------------------------------------+
|  Map Editor                    [x]       |
+------------------+-----------------------+
|                  |                       |
|  PALETTE         |   3D VIEWPORT         |
|                  |                       |
|  [+] Add Voxel   |   +--------------+    |
|                  |   | Voxel        |    |
|  > stone_outdoor |   | Samples      |    |
|    color: ###    |   +--------------+    |
|    rough: 0.3    |                       |
|    metal: 0.0    |   +--------------+    |
|                  |   |              |    |
|  > stone_indoor  |   |   Terrain    |    |
|    color: ###    |   |   Preview    |    |
|    rough: 0.7    |   |              |    |
|    metal: 0.0    |   +--------------+    |
|                  |                       |
|  > purple_crystal|                       |
|    color: ###    |   [Generate]          |
|    rough: 0.1    |   [Play Mode]         |
|    metal: 0.3    |                       |
|    emit:  0.7    |                       |
|                  |                       |
+------------------+-----------------------+
|  Status: Palette loaded (12 voxels)     |
|  Last reload: 2 seconds ago              |
+------------------------------------------+
```

### 13.2 Palette Panel

Shows all voxel types in current palette:
- Expandable tree view
- Color swatch (clickable to see in 3D)
- Numeric properties with sliders (read-only in V1, editable in V2)
- Tags as badges

### 13.3 3D Viewport

Shows:
- **Voxel Samples:** Floating cubes of each voxel type, arranged in a grid
  - Helps see material properties under current lighting
  - Rotates slowly to show all angles
- **Terrain Preview:** Generated terrain below the samples
  - Can orbit camera around
  - Shows actual gameplay view

### 13.4 Status Bar

Shows:
- Current palette file path
- Load status (success/error)
- Time since last reload
- Generation progress (if terrain is generating)

### 13.5 Controls

| Key | Action |
|-----|--------|
| Mouse drag | Orbit camera |
| Scroll | Zoom |
| R | Reset camera |
| G | Regenerate terrain |
| P | Toggle play mode |
| F1 | Toggle UI |

---

## 14. API Summary

*Full specification in `API_DESIGN.md`.*

### 14.1 Core APIs

| API | Rust Trait | Lua Class | Purpose |
|-----|------------|-----------|---------|
| **MaterialAPI** | `MaterialStore` | `Material` | Create/query/update materials in database |
| **GeneratorAPI** | `VoxelGenerator` | `Generator` | Produce voxel data via any method |
| **RendererAPI** | `VoxelRenderer` | `Renderer` | Display voxel data to screen/texture |

### 14.2 MaterialAPI (Database-Based)

```rust
// Rust trait
pub trait MaterialStore {
    fn create(&mut self, def: MaterialDef) -> MaterialId;
    fn get(&self, id: MaterialId) -> Option<&Material>;
    fn search(&self, query: &str, limit: usize) -> Vec<Material>;
    fn find_by_tag(&self, tag: &str) -> Vec<Material>;
    fn create_palette(&mut self, name: &str, ids: &[MaterialId]) -> PaletteId;
}
```

```lua
-- Lua usage
local mat = require("materials")
local id = mat.Material.create({ name = "stone", color = {0.5,0.5,0.5} })
local found = mat.Material.search("wood", 10)
```

### 14.3 GeneratorAPI (Class-Based)

```rust
// Rust trait
pub trait VoxelGenerator {
    fn init(&mut self, ctx: &mut GeneratorContext) -> GeneratorState;
    fn step(&mut self, ctx: &mut GeneratorContext) -> StepResult;
    fn post_process(&mut self, ctx: &mut GeneratorContext) {}
    fn teardown(&mut self, ctx: &mut GeneratorContext);
}
```

```lua
-- Lua usage
local Generator = require("generator")
local MyGen = Generator:new()
function MyGen:step(ctx)
    self:set_voxel(ctx, 0, 0, 0, stone_id)
    return "done"
end
```

### 14.4 RendererAPI (Class-Based)

```rust
// Rust trait
pub trait VoxelRenderer {
    fn init(&mut self, ctx: &mut RenderContext) -> RendererState;
    fn render(&mut self, ctx: &mut RenderContext) -> RenderResult;
    fn teardown(&mut self, ctx: &mut RenderContext);
    fn get_output(&self) -> Option<&RenderOutput>;
}
```

```lua
-- Lua usage
local Renderer = require("renderer")
local MyRenderer = Renderer:new()
function MyRenderer:render(ctx)
    -- Custom rendering logic
    return "ok"
end
```

### 14.5 MCP Tools (AI Access)

All APIs exposed via MCP for external AI:

```
// Materials
create_material(def) -> MaterialId
search_materials(query, limit) -> Material[]
create_palette(name, material_ids) -> PaletteId

// Generation
run_generator(script_path, seed, bounds) -> void
stop_generator() -> void
get_voxel(x, y, z) -> MaterialId

// Rendering
get_render_output() -> bytes (PNG)
set_camera(pos, target, up) -> void
```

---

## 15. Alignment with Fidelity and Optimization

### 15.1 Fidelity Features in Map Editor

Every fidelity feature from the roadmap should be expressible through Map Editor config:

| Fidelity Feature | Map Editor Control |
|------------------|-------------------|
| Crushed Blacks | `post_process.crush_blacks.strength` |
| Moon Ambient | `lighting.ambient` (color from moon config) |
| Film Grain | `post_process.film_grain.strength` |
| Wet Specular | Per-voxel `roughness` in palette |
| Rim Lighting | `post_process.rim_lighting.strength` |
| Volumetric Fog | `atmosphere.fog.density`, `.height_falloff` |
| Vignette | `post_process.vignette.strength` |

### 15.2 Material-Driven Fidelity

The most important fidelity improvement is **per-voxel materials**:
- Indoor stone (dry) vs outdoor stone (wet)
- Metal ore (metallic) vs regular stone (dielectric)
- Crystal (smooth, emissive) vs dirt (rough, matte)

This is why Option C (full material system) is necessary.

### 15.3 Optimization Constraints

The Map Editor must work within performance budgets:
- Palette loading: < 100ms
- Terrain generation (64^3): < 2s
- Hot reload (palette only): < 500ms
- Hot reload (terrain): < 3s

Optimizations to support this:
- Incremental mesh updates (only re-mesh changed chunks)
- Background terrain generation (don't block main thread)
- Palette texture atlas (single GPU upload)

### 15.4 Testing Integration

Optimization testing should use Map Editor scenarios:
- Load large palette (1000 voxel types) - stress palette system
- Generate large terrain (256^3) - stress generation system
- Rapid hot reloads (10/second) - stress reload system

---

## 16. Core Gameplay Loop

### 16.1 The Vision

Three interconnected experiences:

**1. Design Your World**
- Open Map Editor
- Define voxel types with AI assistance
- Generate terrain with AI assistance
- Configure atmosphere and lighting
- Preview and iterate

**2. Survive in Your World**
- Enter the designed world as a player
- Explore the terrain
- Interact with the environment
- Experience the atmosphere firsthand

**3. Build and Craft**
- Collect materials from the world
- Craft items and spells
- Modify the world through gameplay
- Create new content that feeds back into design

### 16.2 Map Editor in the Loop

The Map Editor is the entry point:
```
Design World (Map Editor)
       ↓
  Play World (Survival Mode)
       ↓
 Modify World (Building/Crafting)
       ↓
  Export/Share (Community)
       ↓
 Import Others' Worlds
       ↓
Design World (iterate)
```

### 16.3 Demo Sequence

The demos we build demonstrate this loop:

1. **Demo: Voxel Inspector** - See voxel types and their properties
2. **Demo: AI Terrain Editor** - Design terrain with AI
3. **Demo: Full Map Editor** - Complete world design tool
4. **Demo: Survival Mode** - Play in designed world
5. **Demo: Spell Crafter** - Create effects for the world

Each demo builds on the previous.

---

## 17. Future Integration: opencode.ai

### 17.1 Current Architecture

```
[User] <--terminal--> [opencode CLI] <--API--> [opencode.ai backend]
                            |
                            v
                    [File System]
                            |
                            v
                    [Map Editor] (watches files)
```

### 17.2 Future Architecture

```
[User] <--game UI--> [Map Editor] <--API--> [opencode.ai backend]
                            |
                            v
                    [File System] (still written, for persistence)
```

### 17.3 Integration Points

The game becomes a frontend for opencode:
- Chat interface embedded in ImGui
- API calls directly from game code
- Response handling triggers file writes + hot reload
- Context (current palette, terrain) sent with each request

### 17.4 Implementation Steps

1. **Current:** External AI, file watching (no code changes needed)
2. **Next:** Add HTTP client, call opencode API from game
3. **Then:** Add chat UI in ImGui
4. **Finally:** Stream responses, show typing indicator, etc.

### 17.5 API Contract

```rust
// Conceptual API call
async fn ask_ai(
    prompt: &str,
    context: &MapEditorContext,
) -> Result<AiResponse> {
    let request = OpenCodeRequest {
        prompt,
        context: context.to_json(),
        allowed_actions: vec!["edit_file", "create_file"],
    };
    
    let response = http_client.post(OPENCODE_API_URL)
        .json(&request)
        .send()
        .await?;
    
    Ok(response.json().await?)
}
```

---

## 18. Implementation Phases (API-Driven)

*Each phase introduces or extends APIs. See `API_DESIGN.md` for full specification.*

### 18.1 Philosophy: API-First, Incremental Progress

Each phase:
1. **Introduces or extends an API** (Rust trait + Lua class)
2. **Delivers working functionality** that uses that API
3. **Is independently testable** and demonstrable
4. **Builds on previous phases** without breaking them

```
API → Implementation → Test → API → Implementation → Test → ...
```

### 18.2 Phase Overview

| Phase | Name | API Introduced/Extended | Deliverable |
|-------|------|------------------------|-------------|
| P1 | Material Database | `MaterialStore` (Rust) | SQLite DB for materials |
| P2 | Material Lua Bindings | `Material` (Lua) | Create/search materials from Lua |
| P3 | PBR Pipeline | (extends vertex/shader) | Roughness/metallic rendering |
| P4 | Generator Base | `VoxelGenerator` (Rust), `Generator` (Lua) | Base class for all generators |
| P5 | Direct Writer | `DirectWriter` (Lua) | First generator implementation |
| P6 | 2D Renderer | `VoxelRenderer` (Rust), `Renderer` (Lua) | 2D grid renderer |
| P7 | Hot Reload | `on_change` events | Script/database change detection |
| P8 | MCP Server | MCP tools for all APIs | External AI access |
| P9 | Markov Generator | `MarkovGenerator` (Lua) | Markov Jr. integration |
| P10 | Composed Generators | `SequenceGenerator`, etc. | Generator composition |
| P11 | 3D Renderer | `DeferredRenderer3D` (Lua) | Full 3D rendering |
| P12 | Live Generators | `LiveGenerator` (Lua) | Never-ending generators |

### 18.3 Phase P1: Material Database

**API Introduced:** `MaterialStore` (Rust trait)

```rust
pub trait MaterialStore {
    fn create(&mut self, def: MaterialDef) -> MaterialId;
    fn get(&self, id: MaterialId) -> Option<&Material>;
    fn update(&mut self, id: MaterialId, def: MaterialDef) -> Result<()>;
    fn search(&self, query: &str, limit: usize) -> Vec<Material>;
    fn find_by_tag(&self, tag: &str) -> Vec<Material>;
}
```

**Tasks:**
1. Add `rusqlite` crate
2. Implement `SqliteMaterialStore`
3. Create database schema (materials, palettes, palette_materials)
4. Add embedding column for semantic search (optional V1)
5. Create `MaterialDatabase` Bevy resource

**Verification:**
- Create material via Rust API
- Query it back
- Search by tag works

**Files:**
- `crates/studio_core/src/materials/mod.rs` (new)
- `crates/studio_core/src/materials/store.rs` (new)
- `crates/studio_core/src/materials/sqlite.rs` (new)

### 18.4 Phase P2: Material Lua Bindings

**API Introduced:** `Material` (Lua class)

```lua
local mat = require("materials")
local id = mat.Material.create({ name = "stone", color = {0.5, 0.5, 0.5} })
local found = mat.Material.search("wood", 10)
```

**Tasks:**
1. Add `mlua` crate
2. Expose `MaterialStore` methods to Lua
3. Create `materials.lua` helper module
4. Test from Lua REPL

**Verification:**
- Create material from Lua
- Query it back
- Search works

**Files:**
- `crates/studio_core/src/scripting/mod.rs` (new)
- `crates/studio_core/src/scripting/materials.rs` (new)
- `assets/lua/materials.lua` (new)

### 18.5 Phase P3: PBR Pipeline

**API Extended:** Vertex format, G-buffer, lighting shader

**Tasks:**
1. Extend `GBufferVertex` with roughness, metallic (44 → 52 bytes)
2. Add 4th G-buffer MRT or pack into existing
3. Modify `gbuffer.wgsl` to output materials
4. Port GGX specular to `deferred_lighting.wgsl`
5. Mesh generation looks up material from database

**Verification:**
- Create wet material (roughness 0.3) and dry material (roughness 0.7)
- Render side by side
- Wet has specular highlights, dry is matte

**Files:**
- `crates/studio_core/src/deferred/gbuffer_geometry.rs`
- `assets/shaders/gbuffer.wgsl`
- `assets/shaders/deferred_lighting.wgsl`

### 18.6 Phase P4: Generator Base Class

**API Introduced:** `VoxelGenerator` (Rust), `Generator` (Lua)

```rust
pub trait VoxelGenerator {
    fn init(&mut self, ctx: &mut GeneratorContext) -> GeneratorState;
    fn step(&mut self, ctx: &mut GeneratorContext) -> StepResult;
    fn post_process(&mut self, ctx: &mut GeneratorContext) {}
    fn teardown(&mut self, ctx: &mut GeneratorContext);
}
```

```lua
local Generator = {}
function Generator:init(ctx) return "ready" end
function Generator:step(ctx) return "done" end
function Generator:teardown(ctx) end
```

**Tasks:**
1. Define `VoxelGenerator` trait in Rust
2. Create `GeneratorContext` with voxel buffer access
3. Create `Generator` base class in Lua
4. Add `_rust_voxel_set`, `_rust_voxel_get` bindings
5. Create `GeneratorManager` to run generators

**Verification:**
- Load a generator script
- Call init/step/teardown
- Voxels written correctly

**Files:**
- `crates/studio_core/src/generation/mod.rs` (new)
- `crates/studio_core/src/generation/trait.rs` (new)
- `crates/studio_core/src/generation/manager.rs` (new)
- `assets/lua/generator.lua` (new)

### 18.7 Phase P5: Direct Writer Generator

**API Extended:** First `Generator` subclass

```lua
local Generator = require("generator")
local DirectWriter = Generator:new()

function DirectWriter:step(ctx)
    for x = 0, ctx.bounds.max.x do
        self:set_voxel(ctx, x, 0, 0, self.ground_id)
    end
    return "done"
end
```

**Tasks:**
1. Create `DirectWriter` generator class
2. Test writing patterns directly
3. Verify voxel buffer contents

**Verification:**
- Run DirectWriter
- Inspect voxel buffer
- Pattern matches expectations

**Files:**
- `assets/lua/generators/direct_writer.lua` (new)

### 18.8 Phase P6: 2D Renderer

**API Introduced:** `VoxelRenderer` (Rust), `Renderer` (Lua)

```rust
pub trait VoxelRenderer {
    fn init(&mut self, ctx: &mut RenderContext) -> RendererState;
    fn render(&mut self, ctx: &mut RenderContext) -> RenderResult;
    fn get_output(&self) -> Option<&RenderOutput>;
    fn teardown(&mut self, ctx: &mut RenderContext);
}
```

**Tasks:**
1. Define `VoxelRenderer` trait
2. Create `GridRenderer2D` that renders to texture
3. Display texture in ImGui window
4. Create `Renderer` base class in Lua

**Verification:**
- Generate voxels with DirectWriter
- Render with GridRenderer2D
- See colored grid in ImGui

**Files:**
- `crates/studio_core/src/rendering/mod.rs` (new)
- `crates/studio_core/src/rendering/grid_2d.rs` (new)
- `assets/lua/renderer.lua` (new)
- `assets/lua/renderers/grid_2d.lua` (new)

### 18.9 Phase P7: Hot Reload

**API Extended:** Change events for all systems

**Tasks:**
1. Add `notify` crate for file watching
2. Watch `assets/lua/generators/*.lua`
3. On script change: teardown, reload, re-run
4. Database changes emit `MaterialChangedEvent`
5. Material changes trigger re-render

**Verification:**
- Edit generator script
- Save file
- See generation re-run automatically

**Files:**
- `crates/studio_core/src/hot_reload.rs` (new)

### 18.10 Phase P8: MCP Server

**API Extended:** All APIs exposed via MCP

**Tasks:**
1. Add MCP server crate
2. Expose `create_material`, `search_materials`, etc.
3. Expose `run_generator`, `get_render_output`
4. Start MCP server with example

**Verification:**
- Run example
- External AI calls `create_material`
- Material appears in database

**Files:**
- `crates/studio_core/src/mcp/mod.rs` (new)
- `crates/studio_core/src/mcp/tools.rs` (new)

### 18.11 Phase P9: Markov Generator

**API Extended:** `MarkovGenerator` (wraps Rust Markov Jr.)

```lua
local MarkovGenerator = Generator:new()
function MarkovGenerator:init(ctx)
    self.state = _rust_markov_init(self.model, ctx.seed)
    return "ready"
end
```

**Tasks:**
1. Integrate MarkovJunior Rust port
2. Create Lua bindings for Markov operations
3. Create `MarkovGenerator` class

**Verification:**
- Load a Markov model
- Run to completion
- Output matches expected patterns

**Files:**
- `crates/studio_core/src/generation/markov.rs` (new)
- `assets/lua/generators/markov.lua` (new)

### 18.12 Phase P10: Composed Generators

**API Extended:** Composition classes

```lua
local Sequence = require("generators/sequence")
local terrain = Sequence:new({
    MarkovGenerator:new("base.xml"),
    ScatterGenerator:new(crystal_id, 0.01),
})
```

**Tasks:**
1. Create `SequenceGenerator` (run in order)
2. Create `ScatterGenerator` (random placement)
3. Test composition

**Verification:**
- Compose Markov + Scatter
- Run composed generator
- Both effects visible

**Files:**
- `assets/lua/generators/sequence.lua` (new)
- `assets/lua/generators/scatter.lua` (new)

### 18.13 Phase P11: 3D Renderer

**API Extended:** `DeferredRenderer3D`

**Tasks:**
1. Wrap existing deferred pipeline in `VoxelRenderer`
2. Create Lua class for 3D rendering
3. Connect to existing examples

**Verification:**
- Generate voxels
- Render in 3D with PBR materials
- Matches existing rendering quality

**Files:**
- `crates/studio_core/src/rendering/deferred_3d.rs` (new)
- `assets/lua/renderers/deferred_3d.lua` (new)

### 18.14 Phase P12: Live Generators

**API Extended:** `LiveGenerator` (never returns "done")

```lua
local LiveGenerator = Generator:new()
function LiveGenerator:step(ctx)
    -- Called every frame, never "done"
    self.frame = self.frame + 1
    -- Update voxels based on frame
    return "continue"
end
```

**Tasks:**
1. Create `LiveGenerator` base class
2. Add pause/resume support
3. Test with animated effects

**Verification:**
- Run live generator
- Voxels update each frame
- Pause stops updates

**Files:**
- `assets/lua/generators/live.lua` (new)

**Verification:**
- See floating voxel cubes in 3D view
- Cubes show correct colors and materials
- Can see specular differences between wet/dry

**Files:**
- `crates/studio_core/src/voxel_samples.rs` (new)
- `examples/p_map_editor.rs`

### 18.8 Phase P6: Terrain Definition

**Goal:** Load terrain parameters from Lua file.

**Tasks:**
1. Create terrain definition schema
2. Add terrain loader function
3. Connect to MarkovJunior for generation
4. Generate terrain from definition

**Verification:**
- Load `haunted_graveyard.lua`
- Terrain generates according to definition
- Correct palette used

**Files:**
- `crates/studio_core/src/terrain_definition.rs` (new)
- `assets/terrain/haunted_graveyard.lua` (new)

### 18.9 Phase P7: Terrain Hot Reload

**Goal:** Regenerate terrain when definition changes.

**Tasks:**
1. Add terrain file to watch list
2. Detect terrain file changes
3. Re-parse definition
4. Re-generate terrain
5. Show progress indicator for long generations

**Verification:**
- Run with terrain loaded
- Edit terrain file (change size or rules)
- See terrain regenerate

**Files:**
- `crates/studio_core/src/file_watcher.rs`
- `crates/studio_core/src/terrain_definition.rs`

### 18.10 Phase P8: Config Hot Reload

**Goal:** Lighting and atmosphere configurable from files.

**Tasks:**
1. Create config file schema
2. Add config loader
3. Apply config to lighting uniforms
4. Apply config to post-processing
5. Hot reload on file change

**Verification:**
- Change fog density in config file
- See fog change in game
- Change moon color
- See lighting change

**Files:**
- `crates/studio_core/src/world_config.rs` (new)
- `assets/config/world.lua` (new)

### 18.11 Phase P9: Demo Polish

**Goal:** Complete demo scenario working smoothly.

**Tasks:**
1. Set up demo scene with good camera angle
2. Create demo palette with diverse voxels
3. Create demo terrain with interesting features
4. Test full AI interaction flow
5. Add play mode toggle
6. Fix any visual glitches

**Verification:**
- Run through full demo script (section 12.2)
- All steps work without errors
- Visually compelling result

### 18.12 Phase P10: Documentation

**Goal:** User-facing documentation for Map Editor.

**Tasks:**
1. Write palette file format reference
2. Write terrain file format reference
3. Write config file format reference
4. Write "Getting Started" guide
5. Document AI interaction patterns

**Files:**
- `docs/map_editor/palette_format.md`
- `docs/map_editor/terrain_format.md`
- `docs/map_editor/config_format.md`
- `docs/map_editor/getting_started.md`
- `docs/map_editor/ai_editing.md`

---

## 19. Success Criteria

### 19.1 Minimum Viable Demo

The demo is successful if:
- [ ] Palette loads from Lua file
- [ ] Palette displays in ImGui
- [ ] Voxel samples render with correct materials
- [ ] Terrain generates from palette
- [ ] Palette hot reloads when file changes
- [ ] Material differences visible (wet vs dry)

### 19.2 Full Demo

The full demo is successful if all minimum criteria plus:
- [ ] Terrain hot reloads when definition changes
- [ ] Lighting hot reloads when config changes
- [ ] AI can edit palette externally
- [ ] AI edits reflect in game within 2 seconds
- [ ] Play mode allows walking through terrain
- [ ] No crashes during 10-minute demo session

### 19.3 Production Ready

Production ready when:
- [ ] Error handling for all file operations
- [ ] Clear error messages in UI
- [ ] Performance targets met (section 15.3)
- [ ] Documentation complete
- [ ] Example palettes for different aesthetics
- [ ] Example terrains demonstrating features

---

## 20. Suggested Follow-Up Documents

This roadmap document provides the high-level vision and structure. The following documents should be created to detail specific aspects:

### 20.1 Document: Material Pipeline Implementation

**Focus:** Technical details of PBR material implementation

**Contents:**
- Vertex format changes (exact byte layout)
- G-buffer format decision (pack vs new MRT)
- Shader code for GGX specular
- GPU memory impact analysis
- Compatibility with existing examples

**When to write:** Before Phase P2

### 20.2 Document: Lua Scripting Integration

**Focus:** How Lua scripting works in the engine

**Contents:**
- Lua library choice and integration
- Sandboxing and security
- Helper functions available to scripts
- Error handling and reporting
- Performance considerations

**When to write:** Before Phase P1

### 20.3 Document: Hot Reload System Design

**Focus:** File watching and reload architecture

**Contents:**
- File watcher implementation details
- Event debouncing (avoid double-reloads)
- Cascade handling (what triggers what)
- Thread safety considerations
- Platform-specific notes (macOS FSEvents, Linux inotify)

**When to write:** Before Phase P3

### 20.4 Document: Terrain Generation Pipeline

**Focus:** How terrain is generated from definitions

**Contents:**
- MarkovJunior integration details
- Rule format and semantics
- Post-processing operations
- Chunk generation and streaming
- Performance optimization

**When to write:** Before Phase P6

### 20.5 Document: AI Editing Patterns

**Focus:** How AI should edit Map Editor files

**Contents:**
- Recommended prompts for common operations
- Context to provide to AI
- File format gotchas and tips
- Example AI interactions
- Future in-game integration API

**When to write:** Before Phase P9

### 20.6 Document: Demo Script and Storyboard

**Focus:** Exact demo flow and talking points

**Contents:**
- Minute-by-minute script
- Camera positions and movements
- Fallback plans if something fails
- Recording/streaming setup
- Post-demo Q&A preparation

**When to write:** Before Phase P9

### 20.7 Document: Testing Strategy

**Focus:** How to test Map Editor functionality

**Contents:**
- Unit tests for loaders
- Integration tests for hot reload
- Visual regression tests
- Performance benchmarks
- CI/CD integration

**When to write:** Alongside each phase

---

## Appendix A: Glossary

| Term | Definition |
|------|------------|
| Palette | Collection of voxel type definitions |
| Voxel Type | A single kind of voxel with specific properties |
| Roughness | PBR property controlling surface smoothness (0=mirror, 1=matte) |
| Metallic | PBR property controlling metal vs dielectric (0=non-metal, 1=metal) |
| G-Buffer | Geometry buffer storing per-pixel surface properties |
| MRT | Multiple Render Target (writing to multiple textures at once) |
| Hot Reload | Updating runtime state from changed files without restart |
| MarkovJunior | Procedural generation system using Markov chain rules |

---

## Appendix B: File Structure

```
creature_3d_studio/
├── assets/
│   ├── palettes/
│   │   ├── dark_fantasy.lua
│   │   ├── desert.lua
│   │   └── underground.lua
│   ├── terrain/
│   │   ├── haunted_graveyard.lua
│   │   └── test_terrain.lua
│   ├── config/
│   │   └── world.lua
│   └── shaders/
│       ├── gbuffer.wgsl
│       └── deferred_lighting.wgsl
├── crates/
│   └── studio_core/
│       └── src/
│           ├── voxel_palette.rs
│           ├── terrain_definition.rs
│           ├── world_config.rs
│           ├── file_watcher.rs
│           └── ui/
│               └── palette_ui.rs
├── examples/
│   └── p_map_editor.rs
└── docs/
    └── plans/
        └── map_editor/
            ├── MAP_EDITOR_ROADMAP.md (this document)
            ├── material_pipeline.md
            ├── lua_scripting.md
            ├── hot_reload.md
            ├── terrain_generation.md
            ├── ai_editing.md
            └── demo_script.md
```

---

## Appendix C: Example AI Interactions

### C.1 Adding a Voxel Type

**User prompt:**
```
Add a new voxel type called "mossy_stone" that looks like stone 
covered in moss. It should be slightly green-tinted, rougher than 
regular stone, and found outdoors.
```

**AI action:**
```lua
-- Added to dark_fantasy.lua
{
  id = 9,
  name = "mossy_stone",
  color = { 0.25, 0.32, 0.22 },  -- gray-green
  roughness = 0.75,              -- rougher than stone_outdoor (0.3)
  metallic = 0.0,
  emission = 0.0,
  tags = { "solid", "natural", "outdoor", "stone" }
},
```

### C.2 Modifying Atmosphere

**User prompt:**
```
Make the world feel more oppressive. Increase fog, make it purple-tinted.
```

**AI action:**
```lua
-- Modified in world.lua
atmosphere = {
  fog = {
    color = { 0.15, 0.08, 0.2 },  -- purple tint
    density = 0.04,               -- increased from 0.02
    height_falloff = 0.08,        -- thicker near ground
  },
},
```

### C.3 Changing Terrain Generation

**User prompt:**
```
Add glowing crystals scattered on the surface, mostly in clusters.
```

**AI action:**
```lua
-- Added to terrain.lua post_process
{
  type = "scatter_clustered",
  voxel = "purple_crystal",
  density = 0.005,
  cluster_size = 3,
  cluster_count = 20,
  surface_only = true,
},
```

---

## Appendix D: Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Lua parsing errors crash game | Medium | High | Catch all Lua errors, show in UI, keep last valid state |
| Hot reload causes visual glitch | Medium | Medium | Debounce reloads, validate before applying |
| Large terrain generation blocks UI | High | Medium | Run generation in background thread |
| AI generates invalid Lua | Medium | Medium | Validate AI output before writing, request fixes |
| Material changes break existing content | Low | High | Default values for missing properties |
| Performance regression from materials | Low | Medium | Profile regularly, optimize critical paths |

---

## Appendix E: Timeline

**Week 1:**
- P1: Palette Loader
- P2: Material Pipeline (start)

**Week 2:**
- P2: Material Pipeline (complete)
- P3: Hot Reload
- P4: Palette UI

**Week 3:**
- P5: Voxel Samples
- P6: Terrain Definition
- P7: Terrain Hot Reload

**Week 4:**
- P8: Config Hot Reload
- P9: Demo Polish
- P10: Documentation

**Total: 4 weeks to complete demo**

---

*Document created: [current date]*
*Last updated: [current date]*
*Author: AI Assistant*
*Status: Draft for Review*
