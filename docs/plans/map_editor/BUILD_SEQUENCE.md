# Build Sequence: Map Editor Foundation

> **SUPERSEDED:** This document has been replaced by `MILESTONES.md` which contains the authoritative milestone sequence. This file is kept for historical reference only.

*Functionality-first. End-to-end from milestone 1. Static implementations that work, then complexify.*

---

## Philosophy: Facade Pattern

1. **All APIs exist from M1.** Three traits, three static implementations. Everything wired together.
2. **End-to-end works immediately.** M1 shows a rendered output. Not M5. Not M8. M1.
3. **Static is fine.** Hardcoded materials, hardcoded generator, hardcoded renderer. It runs.
4. **Functionality, not backends.** "I can pick materials" not "in-memory store exists."

---

## The Foundational Script

```bash
cargo run --example p_map_editor_2d
```

**From M1, this script:**
1. Shows a window with a 2D rendered grid (static checkerboard)
2. Shows a material picker (2 hardcoded materials)
3. Clicking a material changes the display

**That's M1. Not M7. M1.**

---

## Three APIs (All Exist From M1)

```rust
// All three traits defined in M1
// All three have static implementations in M1
// Everything is wired together in M1

pub trait MaterialStore {
    fn get(&self, id: MaterialId) -> Option<&Material>;
    fn list(&self) -> &[Material];
}

pub trait VoxelGenerator {
    fn generate(&self, buffer: &mut VoxelBuffer2D, materials: &dyn MaterialStore);
}

pub trait VoxelRenderer {
    fn render(&self, buffer: &VoxelBuffer2D, materials: &dyn MaterialStore) -> Image;
}
```

---

## Milestone Sequence (Functionality-Indexed)

| M# | Functionality | What You SEE | What Changes |
|----|---------------|--------------|--------------|
| **M1** | Pick from 2 materials, see checkerboard | Rendered grid + material picker | Static everything |
| **M2** | Pick from N materials (defined in Rust) | More materials in picker | MaterialStore gets `create()` |
| **M3** | Materials defined in Lua file | Same visuals | Lua loads materials |
| **M4** | Generator defined in Lua file | Same visuals | Lua runs generator |
| **M5** | Hot reload generator script | Edit → see change | File watcher added |
| **M6** | Hot reload materials | Edit → see change | Material reload |
| **M7** | External AI can edit | AI creates material → appears | MCP server |

**M1 takes 2-3 hours. You see a working app.**

---

## M1: Static End-to-End

### Functionality
- I see a 2D grid rendered in an ImGui window
- I see a material picker with 2 materials (stone, dirt)  
- Clicking a material changes which material is used in the checkerboard

### What's Built (All Static)

```rust
// examples/p_map_editor_2d.rs - THE WHOLE THING IN ONE FILE

// === STATIC MATERIALS ===
struct StaticMaterialStore {
    materials: Vec<Material>,
}

impl StaticMaterialStore {
    fn new() -> Self {
        Self {
            materials: vec![
                Material { id: 1, name: "stone".into(), color: [0.5, 0.5, 0.5] },
                Material { id: 2, name: "dirt".into(), color: [0.4, 0.3, 0.2] },
            ],
        }
    }
}

impl MaterialStore for StaticMaterialStore {
    fn get(&self, id: MaterialId) -> Option<&Material> { ... }
    fn list(&self) -> &[Material] { &self.materials }
}

// === STATIC GENERATOR ===
struct CheckerboardGenerator {
    material_a: MaterialId,
    material_b: MaterialId,
}

impl VoxelGenerator for CheckerboardGenerator {
    fn generate(&self, buffer: &mut VoxelBuffer2D, _materials: &dyn MaterialStore) {
        for x in 0..buffer.width {
            for y in 0..buffer.height {
                let mat = if (x + y) % 2 == 0 { self.material_a } else { self.material_b };
                buffer.set(x, y, mat);
            }
        }
    }
}

// === STATIC RENDERER ===
struct GridRenderer2D;

impl VoxelRenderer for GridRenderer2D {
    fn render(&self, buffer: &VoxelBuffer2D, materials: &dyn MaterialStore) -> Image {
        let mut pixels = vec![0u8; buffer.width * buffer.height * 4];
        for x in 0..buffer.width {
            for y in 0..buffer.height {
                let mat_id = buffer.get(x, y);
                let color = materials.get(mat_id).map(|m| m.color).unwrap_or([0.0; 3]);
                // Write RGBA to pixels...
            }
        }
        Image::new(/* ... */)
    }
}

// === APP ===
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .insert_resource(StaticMaterialStore::new())
        .insert_resource(CheckerboardGenerator { material_a: 1, material_b: 2 })
        .insert_resource(VoxelBuffer2D::new(32, 32))
        .add_systems(Startup, setup)
        .add_systems(Update, ui_system)
        .run();
}

fn setup(/* ... */) {
    // Generate once at startup
    generator.generate(&mut buffer, &materials);
    // Render to texture
    let image = renderer.render(&buffer, &materials);
    // Store texture handle
}

fn ui_system(/* ... */) {
    // Left panel: material picker
    for mat in materials.list() {
        if ui.button(&mat.name) {
            generator.material_a = mat.id;
            regenerate_and_rerender();
        }
    }
    // Right panel: rendered output
    ui.image(texture_handle, [256.0, 256.0]);
}
```

### Verification
- [ ] Window shows 32x32 checkerboard
- [ ] Two materials visible in picker
- [ ] Click "dirt" → checkerboard changes colors
- [ ] **This is a working app. In M1.**

---

## M2: Dynamic Materials (Rust)

### Functionality
- I can add materials in Rust code (not just 2 hardcoded)
- Picker shows all materials

### What Changes
```rust
// MaterialStore trait gets create()
pub trait MaterialStore {
    fn get(&self, id: MaterialId) -> Option<&Material>;
    fn list(&self) -> &[Material];
    fn create(&mut self, def: MaterialDef) -> MaterialId;  // NEW
}

// In setup:
materials.create(MaterialDef { name: "crystal", color: [0.8, 0.2, 0.8] });
materials.create(MaterialDef { name: "water", color: [0.2, 0.4, 0.8] });
// Now picker shows 4 materials
```

### Verification
- [ ] Picker shows 4+ materials
- [ ] New materials work in generator

---

## M3: Materials from Lua

### Functionality
- Materials defined in `assets/materials.lua`
- Same UI, same visuals
- Edit Lua file, restart app, see new materials

### What Changes
```rust
// Add mlua crate
// Load and execute Lua file at startup

// assets/materials.lua
return {
    { name = "stone", color = {0.5, 0.5, 0.5} },
    { name = "dirt", color = {0.4, 0.3, 0.2} },
    { name = "crystal", color = {0.8, 0.2, 0.8} },
}

// In setup:
let lua_materials = lua.load_file("assets/materials.lua")?;
for mat in lua_materials {
    materials.create(mat);
}
```

### Verification
- [ ] Edit `materials.lua`, add a material
- [ ] Restart app
- [ ] New material appears in picker

---

## M4: Generator from Lua

### Functionality
- Generator defined in `assets/generator.lua`
- Lua calls `set_voxel(x, y, material_id)`
- Same UI, same visuals

### What Changes
```lua
-- assets/generator.lua
local Generator = {}

function Generator:generate(ctx)
    for x = 0, ctx.width - 1 do
        for y = 0, ctx.height - 1 do
            local mat = ((x + y) % 2 == 0) and 1 or 2
            ctx:set_voxel(x, y, mat)
        end
    end
end

return Generator
```

```rust
// Lua bindings for VoxelBuffer
// GeneratorContext passed to Lua
// Generator trait now wraps Lua execution
```

### Verification
- [ ] Edit `generator.lua` to make stripes instead of checkerboard
- [ ] Restart app
- [ ] See stripes

---

## M5: Hot Reload Generator

### Functionality
- Edit `generator.lua`, save file
- Display updates within 1 second (no restart)

### What Changes
```rust
// Add notify crate
// Watch assets/generator.lua
// On change: reload Lua, re-run generator, re-render
```

### Verification
- [ ] App running
- [ ] Edit `generator.lua` to change pattern
- [ ] Save file
- [ ] Display updates automatically

---

## M6: Hot Reload Materials

### Functionality
- Edit `materials.lua`, save file
- Picker updates, display re-renders

### What Changes
```rust
// Watch assets/materials.lua
// On change: reload materials, update picker, re-render
```

### Verification
- [ ] Edit material color in `materials.lua`
- [ ] Save file
- [ ] Display updates automatically

---

## M7: MCP Server (External AI)

### Functionality
- AI can call `create_material`, `run_generator`, `get_output`
- Changes appear in app immediately

### What Changes
```rust
// Add HTTP server
// Expose MCP tools
// Tool calls trigger same hot-reload paths
```

### Verification
- [ ] AI calls `create_material`
- [ ] Material appears in picker
- [ ] AI calls `get_output`
- [ ] Returns valid PNG

---

## Dependency Graph (Functionality)

```
M1: See checkerboard, pick materials (STATIC - works immediately)
 │
 v
M2: Create materials in Rust (dynamic list)
 │
 v
M3: Materials from Lua (external file)
 │
 v
M4: Generator from Lua (external file)
 │
 v
M5: Hot reload generator (no restart)
 │
 v
M6: Hot reload materials (no restart)
 │
 v
M7: External AI access (MCP)
```

**Each milestone adds functionality to a WORKING app.**
**You never wait 8 milestones to see if it works.**

---

## Build Timeline

```
Day 1 (2-3 hours): M1 - Static end-to-end (SEE IT WORK)
Day 2: M2 + M3 - Dynamic materials, then Lua materials
Day 3: M4 + M5 - Lua generator + hot reload generator
Day 4: M6 + M7 - Hot reload materials + MCP server
```

---

## Dependency Table

| M# | Functionality | New Crates | Estimated Time |
|----|---------------|------------|----------------|
| M1 | Pick materials, see checkerboard | bevy_egui | 2-3 hours |
| M2 | Create materials in Rust | - | 1 hour |
| M3 | Materials from Lua file | mlua | 2 hours |
| M4 | Generator from Lua file | - | 2 hours |
| M5 | Hot reload generator | notify | 1 hour |
| M6 | Hot reload materials | - | 30 min |
| M7 | External AI access | (http) | 2-3 hours |

---

## What We're NOT Building Yet

| Feature | Deferred Until | Why |
|---------|---------------|-----|
| SQLite persistence | After M7 | In-memory is simpler to debug |
| Embedding search | After SQLite | Requires vectors |
| 3D rendering | After M7 | 2D proves the pipeline |
| Markov Jr. | After M7 | Complex dependency |
| Generator composition | After basic generators work | Premature abstraction |
| Live generators | After composition | Advanced feature |
| Palettes | After materials stable | Materials are the primitive |
| PBR materials | After 3D | 2D doesn't need roughness/metallic |

---

## The One Script: M1 vs M7

### M1 (Static - Day 1)

```rust
// examples/p_map_editor_2d.rs - EVERYTHING IN ONE FILE

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .insert_resource(StaticMaterialStore::new())  // 2 hardcoded materials
        .insert_resource(CheckerboardGenerator::new()) // Static generator
        .insert_resource(VoxelBuffer2D::new(32, 32))
        .add_systems(Startup, setup)
        .add_systems(Update, ui_system)
        .run();
}

fn setup(/* ... */) {
    // Generate checkerboard into buffer
    // Render buffer to texture
}

fn ui_system(/* ... */) {
    // Left: material picker (click to change)
    // Right: rendered checkerboard
}
```

**M1 is ~200 lines. It runs. It shows something. Day 1.**

### M7 (Full - Day 4)

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .init_resource::<MaterialStore>()      // Lua-loaded
        .init_resource::<LuaEngine>()          // Script execution
        .init_resource::<GeneratorManager>()   // Lua generators
        .init_resource::<HotReloadWatcher>()   // File watching
        .init_resource::<McpServer>()          // External AI access
        .add_systems(Startup, setup)
        .add_systems(Update, (hot_reload_system, mcp_system, ui_system))
        .run();
}
```

**M7 adds complexity to a WORKING app. It doesn't make a broken app work.**

---

## Success Criteria

### M1 Complete (Day 1)
- [ ] `cargo run --example p_map_editor_2d` shows a window
- [ ] 32x32 checkerboard visible
- [ ] Material picker shows 2 materials
- [ ] Click material → checkerboard colors change
- [ ] **THIS IS A WORKING APP**

### M4 Complete (Lua Generator)
- [ ] Generator defined in `assets/generator.lua`
- [ ] Edit Lua → restart → see different pattern
- [ ] Lua calls `set_voxel(x, y, mat_id)`

### M5 Complete (Hot Reload)
- [ ] Edit `generator.lua`, save
- [ ] Display updates within 1 second (no restart)

### M7 Complete (MCP)
- [ ] External AI calls `create_material` → appears in picker
- [ ] External AI calls `get_output` → returns PNG

### Ready for 3D (Future)
- [ ] Swap `VoxelBuffer2D` → `VoxelBuffer3D`
- [ ] Swap `GridRenderer2D` → `DeferredRenderer3D`
- [ ] Same generator scripts work (just add Z)

---

## Next Steps After M7

1. **SQLite Materials** - Persist materials to disk
2. **Embedding Search** - Semantic material search
3. **3D VoxelBuffer** - Add Z dimension
4. **3D Renderer** - Use existing deferred pipeline
5. **Markov Generator** - Integrate MarkovJunior
6. **Generator Composition** - Sequence, parallel, etc.
7. **PBR Materials** - Roughness, metallic in 3D
8. **Live Generators** - Animated/streaming generation

Each of these builds on the foundation established by M1-M7.
