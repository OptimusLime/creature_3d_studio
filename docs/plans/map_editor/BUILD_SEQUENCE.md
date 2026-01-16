# Build Sequence: Map Editor Foundation

*A single document defining the exact order of operations to build a rock-solid foundation.*

---

## Philosophy

1. **Simplest thing first.** In-memory before SQLite. 2D before 3D. One generator before composition.
2. **End-to-end before depth.** A working pipeline is more valuable than a perfect component.
3. **One script proves it works.** Each milestone has a single command that validates everything.
4. **Dependencies are explicit.** Nothing is built until its dependencies are solid.

---

## The Foundational Script

Everything we build leads to running this one script:

```bash
cargo run --example p_map_editor_2d
```

This script will:
1. Start with an in-memory material store
2. Create materials via Lua
3. Run a generator that writes voxels
4. Render the result to a 2D texture
5. Display in an ImGui window
6. Hot reload when the generator script changes

**When this works end-to-end, we have a foundation.**

---

## Dependency Graph

```
                                    ┌─────────────────────┐
                                    │   p_map_editor_2d   │
                                    │   (THE SCRIPT)      │
                                    └──────────┬──────────┘
                                               │
                         ┌─────────────────────┼─────────────────────┐
                         │                     │                     │
                         v                     v                     v
                ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
                │  Material UI    │   │  Generator UI   │   │  Render Output  │
                │  (ImGui panel)  │   │  (controls)     │   │  (ImGui image)  │
                │                 │   │                 │   │                 │
                │  [M7]           │   │  [M7]           │   │  [M7]           │
                └────────┬────────┘   └────────┬────────┘   └────────┬────────┘
                         │                     │                     │
                         v                     v                     v
                ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
                │  Hot Reload     │   │  Hot Reload     │   │  2D Renderer    │
                │  (materials)    │   │  (scripts)      │   │                 │
                │                 │   │                 │   │                 │
                │  [M6]           │   │  [M6]           │   │  [M5]           │
                └────────┬────────┘   └────────┬────────┘   └────────┬────────┘
                         │                     │                     │
                         v                     v                     v
                ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
                │  Lua Material   │   │  Generator      │   │  Voxel Buffer   │
                │  Bindings       │   │  Manager        │   │  (2D)           │
                │                 │   │                 │   │                 │
                │  [M3]           │   │  [M4]           │   │  [M4]           │
                └────────┬────────┘   └────────┬────────┘   └────────┬────────┘
                         │                     │                     │
                         v                     v                     v
                ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
                │  Material Store │   │  Lua Engine     │   │  (primitive)    │
                │  (in-memory)    │   │  (mlua)         │   │                 │
                │                 │   │                 │   │                 │
                │  [M2]           │   │  [M3]           │   │                 │
                └────────┬────────┘   └────────┬────────┘   └─────────────────┘
                         │                     │
                         v                     v
                ┌─────────────────────────────────────────┐
                │           Bevy App Shell                │
                │           (existing)                    │
                │                                         │
                │           [M1]                          │
                └─────────────────────────────────────────┘
```

---

## Milestone Sequence

| M# | Milestone | Proves | Dependencies | Complexity |
|----|-----------|--------|--------------|------------|
| **M1** | Bevy Shell | App runs, ImGui works | None | Trivial |
| **M2** | In-Memory Materials | Materials exist at runtime | M1 | Simple |
| **M3** | Lua Engine + Bindings | Lua can create materials | M1, M2 | Medium |
| **M4** | Generator + VoxelBuffer | Lua can write voxels | M3 | Medium |
| **M5** | 2D Renderer | Voxels display as colored grid | M4 | Medium |
| **M6** | Hot Reload | Script changes re-run generator | M4, M5 | Medium |
| **M7** | Full UI | ImGui shows materials, controls, output | M2-M6 | Simple |
| **M8** | MCP Server | External AI can call APIs | M2-M5 | Medium |

**After M7, the foundation is complete. M8+ adds external access.**

---

## Detailed Milestones

### M1: Bevy Shell

**Goal:** Empty example that runs Bevy with ImGui.

**Proof Script:**
```bash
cargo run --example p_map_editor_2d
# Opens window with empty ImGui panel
# No crashes
```

**What's Built:**
- `examples/p_map_editor_2d.rs`
- Basic Bevy app with `bevy_egui` plugin
- Empty "Map Editor" window

**Systems Required:**
| System | Level | Notes |
|--------|-------|-------|
| Bevy | Existing | Just App + DefaultPlugins |
| bevy_egui | Existing | Just plugin setup |

**Lines of Code:** ~50

**Verification:**
- [ ] Window opens
- [ ] ImGui panel visible
- [ ] No panics

---

### M2: In-Memory Materials

**Goal:** Create and query materials from Rust code.

**Proof Script:**
```bash
cargo run --example p_map_editor_2d
# Console prints: "Created material 'stone' with id 1"
# Console prints: "Found 1 materials matching 'stone'"
```

**What's Built:**
- `crates/studio_core/src/materials/mod.rs`
- `crates/studio_core/src/materials/store.rs`
- `MaterialStore` trait
- `InMemoryMaterialStore` implementation

**API:**
```rust
pub trait MaterialStore {
    fn create(&mut self, def: MaterialDef) -> MaterialId;
    fn get(&self, id: MaterialId) -> Option<&Material>;
    fn list(&self) -> Vec<&Material>;
    fn find_by_name(&self, name: &str) -> Option<&Material>;
    fn find_by_tag(&self, tag: &str) -> Vec<&Material>;
}

// SIMPLE: Just HashMap + Vec
pub struct InMemoryMaterialStore {
    materials: HashMap<MaterialId, Material>,
    next_id: u64,
}
```

**Systems Required:**
| System | Level | Notes |
|--------|-------|-------|
| M1 | Complete | App shell |
| MaterialStore trait | New | Define interface |
| InMemoryMaterialStore | New | HashMap-based |

**Lines of Code:** ~100

**What's NOT Built Yet:**
- SQLite persistence
- Embedding search
- Palettes (just individual materials)

**Verification:**
- [ ] Create material returns ID
- [ ] Get material by ID works
- [ ] Find by name works
- [ ] Find by tag works

---

### M3: Lua Engine + Material Bindings

**Goal:** Create materials from Lua code.

**Proof Script:**
```bash
cargo run --example p_map_editor_2d
# Loads assets/lua/test_materials.lua
# Console prints: "Lua created material 'stone' with id 1"
```

**Test Lua Script:** `assets/lua/test_materials.lua`
```lua
local mat = require("materials")

local stone_id = mat.create({
    name = "stone",
    color = {0.5, 0.5, 0.5},
    roughness = 0.7,
    tags = {"solid", "natural"}
})

print("Created stone with id: " .. stone_id)
```

**What's Built:**
- `crates/studio_core/src/scripting/mod.rs`
- `crates/studio_core/src/scripting/engine.rs`
- `crates/studio_core/src/scripting/materials.rs`
- `assets/lua/materials.lua` (helper module)

**API:**
```rust
pub struct LuaEngine {
    lua: Lua,
}

impl LuaEngine {
    pub fn new() -> Self;
    pub fn run_file(&mut self, path: &Path, materials: &mut dyn MaterialStore) -> Result<()>;
}
```

**Lua Bindings:**
```lua
-- Exposed to Lua
materials.create(def) -> id
materials.get(id) -> material_table
materials.find_by_name(name) -> material_table or nil
materials.find_by_tag(tag) -> array of material_tables
```

**Systems Required:**
| System | Level | Notes |
|--------|-------|-------|
| M1 | Complete | App shell |
| M2 | Complete | MaterialStore |
| mlua | New dependency | Lua runtime |
| LuaEngine | New | Script execution |
| Material bindings | New | Expose to Lua |

**Lines of Code:** ~200

**What's NOT Built Yet:**
- Generator bindings
- Hot reload
- Error recovery

**Verification:**
- [ ] Lua script executes without error
- [ ] Material created from Lua exists in Rust store
- [ ] Properties (color, roughness, tags) preserved

---

### M4: Generator + VoxelBuffer

**Goal:** Lua generator writes voxels to a buffer.

**Proof Script:**
```bash
cargo run --example p_map_editor_2d
# Loads assets/lua/generators/checkerboard.lua
# Console prints: "Generator completed: 64x64 voxels"
# Console prints: "Voxel (0,0) = 1, Voxel (1,0) = 2"
```

**Test Generator:** `assets/lua/generators/checkerboard.lua`
```lua
local Generator = require("generator")
local Checkerboard = Generator:new()

function Checkerboard:init(ctx)
    self.material_a = 1  -- stone
    self.material_b = 2  -- dirt
    return "ready"
end

function Checkerboard:step(ctx)
    for x = 0, ctx.bounds.max_x - 1 do
        for y = 0, ctx.bounds.max_y - 1 do
            local mat = ((x + y) % 2 == 0) and self.material_a or self.material_b
            self:set_voxel(ctx, x, y, mat)
        end
    end
    return "done"
end

return Checkerboard
```

**What's Built:**
- `crates/studio_core/src/generation/mod.rs`
- `crates/studio_core/src/generation/buffer.rs`
- `crates/studio_core/src/generation/manager.rs`
- `crates/studio_core/src/scripting/generator.rs`
- `assets/lua/generator.lua` (base class)

**API:**
```rust
// Simple 2D buffer
pub struct VoxelBuffer2D {
    data: Vec<MaterialId>,
    width: u32,
    height: u32,
}

impl VoxelBuffer2D {
    pub fn new(width: u32, height: u32) -> Self;
    pub fn set(&mut self, x: u32, y: u32, material: MaterialId);
    pub fn get(&self, x: u32, y: u32) -> MaterialId;
}

// Manager runs generators
pub struct GeneratorManager {
    engine: LuaEngine,
    buffer: VoxelBuffer2D,
}

impl GeneratorManager {
    pub fn load_generator(&mut self, path: &Path) -> Result<()>;
    pub fn run(&mut self, seed: u64) -> Result<()>;
}
```

**Lua Bindings:**
```lua
-- Exposed to generators
ctx.bounds.max_x, ctx.bounds.max_y
self:set_voxel(ctx, x, y, material_id)
self:get_voxel(ctx, x, y) -> material_id
```

**Systems Required:**
| System | Level | Notes |
|--------|-------|-------|
| M1-M3 | Complete | App, materials, Lua |
| VoxelBuffer2D | New | 2D grid storage |
| GeneratorManager | New | Run generators |
| Generator bindings | New | set_voxel, get_voxel |
| generator.lua | New | Base class |

**Lines of Code:** ~300

**What's NOT Built Yet:**
- 3D voxels
- Composition
- Live generators
- Markov integration

**Verification:**
- [ ] Generator script loads
- [ ] init() called, returns "ready"
- [ ] step() called, writes voxels
- [ ] VoxelBuffer contains expected pattern

---

### M5: 2D Renderer

**Goal:** VoxelBuffer displayed as colored grid in ImGui.

**Proof Script:**
```bash
cargo run --example p_map_editor_2d
# Window shows checkerboard pattern
# Stone voxels are gray, dirt voxels are brown
```

**What's Built:**
- `crates/studio_core/src/rendering/mod.rs`
- `crates/studio_core/src/rendering/grid_2d.rs`

**API:**
```rust
pub struct GridRenderer2D {
    texture: Option<Handle<Image>>,
    width: u32,
    height: u32,
}

impl GridRenderer2D {
    pub fn new(width: u32, height: u32) -> Self;
    pub fn render(
        &mut self,
        buffer: &VoxelBuffer2D,
        materials: &dyn MaterialStore,
        images: &mut Assets<Image>,
    );
    pub fn texture(&self) -> Option<Handle<Image>>;
}
```

**ImGui Integration:**
```rust
// In UI system
if let Some(texture) = renderer.texture() {
    ui.image(texture, [512.0, 512.0]);
}
```

**Systems Required:**
| System | Level | Notes |
|--------|-------|-------|
| M1-M4 | Complete | App, materials, Lua, generator |
| GridRenderer2D | New | Buffer → texture |
| Bevy Image | Existing | Texture asset |
| bevy_egui | Existing | Image display |

**Lines of Code:** ~150

**What's NOT Built Yet:**
- 3D rendering
- Camera controls
- Multiple viewports

**Verification:**
- [ ] Texture created with correct dimensions
- [ ] Each voxel colored by material.color
- [ ] Image displays in ImGui
- [ ] Checkerboard pattern visible

---

### M6: Hot Reload

**Goal:** Editing generator script re-runs generation.

**Proof Script:**
```bash
cargo run --example p_map_editor_2d
# Shows checkerboard
# Edit checkerboard.lua to change material_a = 3
# Save file
# Display updates to new pattern within 1 second
```

**What's Built:**
- `crates/studio_core/src/hot_reload.rs`

**API:**
```rust
pub struct HotReloadWatcher {
    watcher: RecommendedWatcher,
    rx: Receiver<DebouncedEvent>,
}

impl HotReloadWatcher {
    pub fn new(paths: &[PathBuf]) -> Result<Self>;
    pub fn poll(&self) -> Option<PathBuf>;
}

// Bevy system
fn hot_reload_system(
    watcher: Res<HotReloadWatcher>,
    mut generator_manager: ResMut<GeneratorManager>,
    mut renderer: ResMut<GridRenderer2D>,
    // ...
) {
    if let Some(changed_path) = watcher.poll() {
        if changed_path.ends_with(".lua") {
            generator_manager.reload_and_run();
            renderer.render(&generator_manager.buffer, &materials);
        }
    }
}
```

**Systems Required:**
| System | Level | Notes |
|--------|-------|-------|
| M1-M5 | Complete | Full pipeline |
| notify | New dependency | File watching |
| HotReloadWatcher | New | Debounced events |
| Reload system | New | Triggers re-run |

**Lines of Code:** ~100

**What's NOT Built Yet:**
- Material hot reload (database-triggered)
- Error recovery on bad script

**Verification:**
- [ ] File change detected within 500ms
- [ ] Generator re-loaded
- [ ] Generation re-run with same seed
- [ ] Display updates

---

### M7: Full UI

**Goal:** Complete ImGui interface with all panels.

**Proof Script:**
```bash
cargo run --example p_map_editor_2d
# Left panel: Material list with colors and properties
# Center: Rendered voxel output
# Bottom: Status bar with reload time
# Can click materials to see details
```

**What's Built:**
- `crates/studio_core/src/ui/mod.rs`
- `crates/studio_core/src/ui/material_panel.rs`
- `crates/studio_core/src/ui/viewport.rs`
- `crates/studio_core/src/ui/status_bar.rs`

**Layout:**
```
+------------------+------------------------+
|   MATERIALS      |                        |
|                  |      VIEWPORT          |
|   [stone]        |                        |
|   [dirt]         |    (rendered grid)     |
|   [crystal]      |                        |
|                  |                        |
+------------------+------------------------+
| Status: Ready | Last reload: 2s ago      |
+----------------------------------------------+
```

**Systems Required:**
| System | Level | Notes |
|--------|-------|-------|
| M1-M6 | Complete | Full pipeline + hot reload |
| Material panel | New | List + details |
| Viewport | New | Texture display |
| Status bar | New | Info display |

**Lines of Code:** ~200

**Verification:**
- [ ] Materials listed with correct colors
- [ ] Clicking material shows details
- [ ] Viewport shows rendered output
- [ ] Status updates on reload

---

### M8: MCP Server (Post-Foundation)

**Goal:** External AI can create materials and run generators.

**Proof Script:**
```bash
# Terminal 1:
cargo run --example p_map_editor_2d

# Terminal 2 (or AI tool):
curl -X POST http://localhost:8080/mcp/tools/create_material \
  -d '{"name": "crystal", "color": [0.8, 0.2, 0.8], "emission": 0.7}'
# Returns: {"id": 3}

# Material appears in UI immediately
```

**What's Built:**
- `crates/studio_core/src/mcp/mod.rs`
- `crates/studio_core/src/mcp/server.rs`
- `crates/studio_core/src/mcp/tools.rs`

**MCP Tools:**
```
create_material(def) -> {id}
list_materials() -> [{id, name, color, ...}]
run_generator(path, seed) -> {status}
get_render_output() -> {png_base64}
```

**Systems Required:**
| System | Level | Notes |
|--------|-------|-------|
| M1-M7 | Complete | Full foundation |
| HTTP server | New | Embedded in Bevy |
| MCP protocol | New | Tool definitions |
| Cross-thread comms | New | Server → Bevy events |

**Lines of Code:** ~300

**Verification:**
- [ ] Server starts on port 8080
- [ ] create_material works
- [ ] Material appears in UI
- [ ] run_generator works
- [ ] get_render_output returns valid PNG

---

## Sequential Build Order

```
Week 1: Core Pipeline
├── Day 1: M1 (Bevy Shell) + M2 (In-Memory Materials)
├── Day 2: M3 (Lua Engine + Bindings)
├── Day 3: M4 (Generator + VoxelBuffer)
└── Day 4: M5 (2D Renderer)

Week 2: Polish + External Access
├── Day 5: M6 (Hot Reload)
├── Day 6: M7 (Full UI)
└── Day 7: M8 (MCP Server)
```

---

## Dependency Table

| Milestone | Depends On | New Crates | New Files | LOC |
|-----------|------------|------------|-----------|-----|
| M1 | - | bevy_egui | 1 | ~50 |
| M2 | M1 | - | 2 | ~100 |
| M3 | M1, M2 | mlua | 4 | ~200 |
| M4 | M3 | - | 5 | ~300 |
| M5 | M4 | - | 2 | ~150 |
| M6 | M4, M5 | notify | 1 | ~100 |
| M7 | M2-M6 | - | 4 | ~200 |
| M8 | M7 | (http) | 3 | ~300 |
| **Total** | | **3** | **22** | **~1400** |

---

## Simplifications (What We're NOT Building Yet)

| Feature | Deferred Until | Why |
|---------|---------------|-----|
| SQLite persistence | After M8 | In-memory is simpler to debug |
| Embedding search | After SQLite | Requires vectors |
| 3D rendering | After M8 | 2D proves the pipeline |
| Markov Jr. | After M8 | Complex dependency |
| Generator composition | After basic generators work | Premature abstraction |
| Live generators | After composition | Advanced feature |
| Palettes | After materials stable | Materials are the primitive |
| PBR materials | After 3D | 2D doesn't need roughness/metallic |

---

## The One Script (Revisited)

After M7 is complete, this is what `p_map_editor_2d` does:

```rust
// examples/p_map_editor_2d.rs

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        // Our systems
        .init_resource::<InMemoryMaterialStore>()
        .init_resource::<LuaEngine>()
        .init_resource::<GeneratorManager>()
        .init_resource::<GridRenderer2D>()
        .init_resource::<HotReloadWatcher>()
        // Startup
        .add_systems(Startup, setup)
        // Update
        .add_systems(Update, (
            hot_reload_system,
            ui_system,
        ))
        .run();
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<InMemoryMaterialStore>,
    mut lua: ResMut<LuaEngine>,
    mut generator: ResMut<GeneratorManager>,
    mut renderer: ResMut<GridRenderer2D>,
    mut images: ResMut<Assets<Image>>,
) {
    // 1. Run initial materials script
    lua.run_file("assets/lua/init_materials.lua", &mut *materials);
    
    // 2. Load and run generator
    generator.load_generator("assets/lua/generators/checkerboard.lua");
    generator.run(12345);
    
    // 3. Render to texture
    renderer.render(&generator.buffer, &*materials, &mut images);
}

fn hot_reload_system(/* ... */) {
    // Watch for changes, re-run generator, re-render
}

fn ui_system(/* ... */) {
    // Draw material panel, viewport, status bar
}
```

**When this runs without crashing and shows a checkerboard, the foundation is complete.**

---

## Success Criteria

### Foundation Complete (M7)
- [ ] `cargo run --example p_map_editor_2d` works
- [ ] Materials created from Lua
- [ ] Checkerboard generator runs
- [ ] 2D grid renders correctly
- [ ] Hot reload works (edit script → display updates)
- [ ] UI shows materials and viewport

### Ready for AI (M8)
- [ ] MCP server running
- [ ] External tool can create materials
- [ ] External tool can trigger generation
- [ ] External tool can get render output

### Ready for 3D (Future)
- [ ] Swap `VoxelBuffer2D` → `VoxelBuffer3D`
- [ ] Swap `GridRenderer2D` → `DeferredRenderer3D`
- [ ] Same generator scripts work (just add Z)

---

## Next Steps After Foundation

1. **SQLite Materials** - Persist materials to disk
2. **Embedding Search** - Semantic material search
3. **3D VoxelBuffer** - Add Z dimension
4. **3D Renderer** - Use existing deferred pipeline
5. **Markov Generator** - Integrate MarkovJunior
6. **Generator Composition** - Sequence, parallel, etc.
7. **PBR Materials** - Roughness, metallic in 3D
8. **Live Generators** - Animated/streaming generation

Each of these builds on the rock-solid foundation established by M1-M8.
