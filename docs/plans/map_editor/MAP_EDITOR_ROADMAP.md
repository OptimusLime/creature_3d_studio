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

## 8. Terrain Generation Pipeline

### 8.1 Markov Models for Terrain

Terrain is generated using Markov models (via MarkovJunior integration). The model defines:
- What voxel types can exist
- Rules for how they can be placed adjacent to each other
- Patterns that emerge from rule application

### 8.2 Terrain Definition File

```lua
-- terrain.lua
return {
  name = "haunted_graveyard",
  palette = "dark_fantasy",  -- reference to palette file
  
  size = { x = 64, y = 32, z = 64 },
  
  -- MarkovJunior model reference
  model = "graveyard.xml",
  
  -- Or inline rules
  rules = {
    { pattern = "air/stone", weight = 1.0 },
    { pattern = "stone/dirt", weight = 0.8 },
    { pattern = "dirt/air", weight = 0.3 },
  },
  
  -- Overrides and post-processing
  post_process = {
    -- Add crystals at random positions
    { type = "scatter", voxel = "purple_crystal", density = 0.001, height_min = 5 },
    -- Erode terrain edges
    { type = "erode", iterations = 2 },
  }
}
```

### 8.3 Terrain Generation Flow

```
Terrain File (disk)
       ↓
   Lua Parser
       ↓
TerrainDefinition (CPU)
       ↓
MarkovJunior Solver
  - Applies rules iteratively
  - Produces voxel grid (u16 per cell)
       ↓
Post-Processing
  - Scatter, erode, etc.
       ↓
VoxelWorld (CPU)
  - 3D array of voxel IDs
       ↓
Mesh Generation
  - Greedy meshing with material lookup
       ↓
Rendered Terrain
```

### 8.4 Hot Reloading Terrain

When `terrain.lua` changes:
1. Re-parse terrain definition
2. Re-run generation (may be slow for large terrains)
3. Re-mesh affected chunks
4. Update GPU buffers

For rapid iteration, support:
- Small preview terrain (16x16x16) for instant feedback
- Full terrain generation as separate "bake" step
- Incremental re-generation where possible

---

## 9. Hot Reloading Architecture

### 9.1 File Watcher

A background system monitors relevant directories:

```rust
pub struct FileWatcher {
    watcher: notify::RecommendedWatcher,
    receiver: Receiver<notify::Event>,
    watched_paths: HashSet<PathBuf>,
}
```

Watched paths:
- `assets/palettes/*.lua`
- `assets/terrain/*.lua`
- `assets/config/*.lua`

### 9.2 Change Detection

Each frame, check for file events:

```rust
fn check_file_changes(
    file_watcher: Res<FileWatcher>,
    mut reload_events: EventWriter<ReloadEvent>,
) {
    while let Ok(event) = file_watcher.receiver.try_recv() {
        match event.kind {
            EventKind::Modify(_) | EventKind::Create(_) => {
                for path in event.paths {
                    reload_events.send(ReloadEvent { path });
                }
            }
            _ => {}
        }
    }
}
```

### 9.3 Reload Handlers

Different handlers for different file types:

```rust
fn handle_palette_reload(
    mut events: EventReader<ReloadEvent>,
    mut palette: ResMut<VoxelPalette>,
    // ... other resources
) {
    for event in events.read() {
        if event.path.extension() == Some("lua") 
           && event.path.parent().unwrap().ends_with("palettes") 
        {
            match load_palette(&event.path) {
                Ok(new_palette) => {
                    *palette = new_palette;
                    // Trigger re-mesh of all terrain
                }
                Err(e) => {
                    error!("Failed to reload palette: {}", e);
                    // Keep old palette, show error in UI
                }
            }
        }
    }
}
```

### 9.4 Cascading Reloads

Some changes cascade:
- Palette change → re-mesh all terrain using that palette
- Terrain definition change → re-generate that terrain
- Lighting config change → update shader uniforms (no re-mesh)

### 9.5 What Requires Restart

Some changes cannot be hot reloaded (require new Rust code):
- New vertex attributes
- New G-buffer formats
- New shader entry points
- New ECS systems

These should be rare once the system is mature.

---

## 10. Scripting Model

### 10.1 Why Scripting

Scripting enables:
- External file editing (by AI or human)
- Hot reloading without recompilation
- Safer iteration (syntax errors don't crash the app)
- Future in-game scripting (console commands, modding)

### 10.2 Language Choice: Lua

Lua is chosen because:
- Lightweight, embeddable, fast
- Well-understood by AI models
- Existing Bevy integration (`bevy_mod_scripting`)
- Simple syntax for data definition
- Can also express logic if needed

### 10.3 Script Types

**Data Scripts:** Define static data (palettes, terrain params)
```lua
-- Pure data, no logic
return {
  voxels = { ... }
}
```

**Config Scripts:** Define runtime parameters
```lua
-- May include simple expressions
return {
  fog_density = 0.02,
  moon1_color = rgb(0.6, 0.3, 0.8),  -- helper function
}
```

**Logic Scripts (future):** Define behavior
```lua
-- Full logic, called per-frame or on events
function on_voxel_placed(x, y, z, type)
  if type == "water" then
    spread_water(x, y, z)
  end
end
```

### 10.4 Script Execution

```rust
pub struct ScriptEngine {
    lua: Lua,
    loaded_scripts: HashMap<PathBuf, ScriptState>,
}

impl ScriptEngine {
    pub fn load_data_script<T: FromLua>(&mut self, path: &Path) -> Result<T> {
        let source = std::fs::read_to_string(path)?;
        let value: LuaValue = self.lua.load(&source).eval()?;
        T::from_lua(value, &self.lua)
    }
    
    pub fn reload_script(&mut self, path: &Path) -> Result<()> {
        // Re-execute and update cached state
    }
}
```

### 10.5 Error Handling

Script errors should:
- Be logged clearly with file/line information
- Be displayed in the UI
- NOT crash the application
- Keep the previous valid state

```
[ERROR] Failed to load palette.lua:
  Line 15: attempt to index nil value 'voxels'
  Keeping previous palette.
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

## 14. File Format Specifications

### 14.1 Palette File Format

**Location:** `assets/palettes/<name>.lua`

**Schema:**
```lua
return {
  -- Required
  name = "string",           -- Unique palette name
  version = number,          -- Schema version (currently 1)
  voxels = {                 -- Array of voxel definitions
    {
      -- Required
      id = number,           -- Unique ID (1-65535, 0 reserved for air)
      name = "string",       -- Unique name within palette
      color = { r, g, b },   -- RGB values 0.0-1.0
      
      -- Optional (defaults shown)
      roughness = 0.5,       -- 0.0-1.0
      metallic = 0.0,        -- 0.0-1.0
      emission = 0.0,        -- 0.0-1.0
      emission_color = nil,  -- defaults to color if emission > 0
      tags = {},             -- array of strings
    },
    -- ... more voxels
  },
  
  -- Optional
  metadata = {
    author = "string",
    description = "string",
    created = "ISO date",
    modified = "ISO date",
  }
}
```

### 14.2 Terrain File Format

**Location:** `assets/terrain/<name>.lua`

**Schema:**
```lua
return {
  -- Required
  name = "string",
  palette = "string",        -- Name of palette file (without .lua)
  size = { x = n, y = n, z = n },
  
  -- Generation method (one of these)
  model = "string",          -- MarkovJunior .xml file name
  -- OR
  rules = {                  -- Inline rules
    { pattern = "a/b", weight = 1.0 },
  },
  -- OR
  heightmap = "string",      -- PNG file for heightmap-based generation
  
  -- Optional
  seed = number,             -- Random seed (default: random)
  post_process = {           -- Post-processing steps
    { type = "scatter", voxel = "name", density = 0.01 },
    { type = "erode", iterations = 2 },
    { type = "smooth", passes = 1 },
  },
}
```

### 14.3 Config File Format

**Location:** `assets/config/world.lua`

**Schema:**
```lua
return {
  -- Lighting
  lighting = {
    moon1 = {
      color = { r, g, b },
      intensity = 0.5,
      direction = { x, y, z },  -- or use orbit parameters
    },
    moon2 = { ... },
    ambient = {
      color = { r, g, b },
      intensity = 0.1,
    },
  },
  
  -- Atmosphere
  atmosphere = {
    fog = {
      color = { r, g, b },
      density = 0.02,
      height_falloff = 0.1,
    },
    sky_color = { r, g, b },
  },
  
  -- Post-processing
  post_process = {
    bloom = { intensity = 0.5, threshold = 0.8 },
    film_grain = { strength = 0.02 },
    vignette = { strength = 0.3 },
  },
}
```

### 14.4 Validation

On load, validate:
- Required fields present
- Types correct
- Values in valid ranges
- No duplicate IDs or names
- Referenced files exist

Report errors clearly with file/line numbers.

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

## 18. Implementation Phases

### 18.1 Philosophy: Incremental Progress

Each phase delivers working functionality:
```
Functionality → Test → Functionality → Test → ...
```

No phase should leave the system broken. Each phase should be:
- Independently testable
- Demonstrable to stakeholders
- A foundation for the next phase

### 18.2 Phase Overview

| Phase | Name | Deliverable | Est. Time |
|-------|------|-------------|-----------|
| P1 | Palette Loader | Load palette from Lua, use in mesh gen | 1 day |
| P2 | Material Pipeline | Roughness/metallic in vertex → G-buffer → lighting | 2 days |
| P3 | Hot Reload | File watcher + palette reload | 1 day |
| P4 | Palette UI | ImGui panel showing palette | 0.5 day |
| P5 | Voxel Samples | 3D preview of voxel types | 0.5 day |
| P6 | Terrain Definition | Load terrain from Lua | 1 day |
| P7 | Terrain Hot Reload | Regenerate on file change | 1 day |
| P8 | Config Hot Reload | Lighting/atmosphere from files | 0.5 day |
| P9 | Demo Polish | Full demo scenario working | 1 day |
| P10 | Documentation | User-facing docs, API reference | 0.5 day |

**Total: ~9 days of focused work**

### 18.3 Phase P1: Palette Loader

**Goal:** Load voxel palette from Lua file, use in mesh generation.

**Tasks:**
1. Add `mlua` crate for Lua parsing
2. Create `VoxelPalette` struct in Rust
3. Write palette loader function
4. Modify mesh generation to look up color from palette
5. Test with sample palette file

**Verification:**
- Load `dark_fantasy.lua`
- Generate terrain mesh
- Voxels have correct colors from palette

**Files:**
- `crates/studio_core/src/voxel_palette.rs` (new)
- `crates/studio_core/src/voxel_mesh.rs` (modify)
- `assets/palettes/dark_fantasy.lua` (new)

### 18.4 Phase P2: Material Pipeline

**Goal:** Roughness and metallic flow from palette through rendering.

**Tasks:**
1. Extend `GBufferVertex` with roughness, metallic
2. Update vertex buffer layout (44 → 52 bytes)
3. Add 4th G-buffer MRT for materials (or pack into existing)
4. Modify `gbuffer.wgsl` to output materials
5. Port GGX specular functions to `deferred_lighting.wgsl`
6. Read materials in lighting pass, calculate specular

**Verification:**
- Place wet stone (roughness 0.3) and dry stone (roughness 0.7) side by side
- Wet stone has visible specular highlights
- Dry stone is matte
- Metal voxels reflect colored highlights correctly

**Files:**
- `crates/studio_core/src/deferred/gbuffer_geometry.rs`
- `crates/studio_core/src/deferred/gbuffer.rs`
- `crates/studio_core/src/voxel_mesh.rs`
- `assets/shaders/gbuffer.wgsl`
- `assets/shaders/deferred_lighting.wgsl`

### 18.5 Phase P3: Hot Reload

**Goal:** Detect palette file changes, reload automatically.

**Tasks:**
1. Add `notify` crate for file watching
2. Create `FileWatcher` resource
3. Add system to check for file events
4. Add system to reload palette on change
5. Trigger re-mesh when palette changes

**Verification:**
- Run game with palette loaded
- Edit palette file externally (change a color)
- Save file
- See terrain update within 1 second

**Files:**
- `crates/studio_core/src/file_watcher.rs` (new)
- `crates/studio_core/src/voxel_palette.rs` (modify)

### 18.6 Phase P4: Palette UI

**Goal:** ImGui panel displaying current palette.

**Tasks:**
1. Add ImGui window for palette
2. List all voxel types
3. Show color swatches
4. Show numeric properties
5. Show load status and errors

**Verification:**
- Open Map Editor example
- See palette panel on left
- All voxel types listed with correct properties
- Error shown if palette has syntax error

**Files:**
- `examples/p_map_editor.rs` (new or extend existing)
- `crates/studio_core/src/ui/palette_ui.rs` (new)

### 18.7 Phase P5: Voxel Samples

**Goal:** 3D preview showing each voxel type as a cube.

**Tasks:**
1. Generate sample cubes for each voxel type
2. Arrange in grid above terrain
3. Add slow rotation for material visibility
4. Highlight selected voxel

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
