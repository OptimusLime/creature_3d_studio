# Map Editor API Design

*Defining the facades and their evolution across phases.*

---

## Philosophy

1. **APIs first, implementations second.** We define what functions exist before we write them.
2. **Facade pattern.** Start simple, complexify over time. Each phase extends the API.
3. **Lua classes extend Rust traits.** The Lua API mirrors Rust traits that do the actual work.
4. **No shitty JSON.** Configuration is code, not data. Classes, not tables.

---

## Core APIs

There are three fundamental APIs in the Map Editor:

| API | Responsibility | Rust Trait | Lua Base Class |
|-----|----------------|------------|----------------|
| **MaterialAPI** | Create, query, update materials in the database | `MaterialStore` | `Material` |
| **GeneratorAPI** | Produce voxel data through any method | `VoxelGenerator` | `Generator` |
| **RendererAPI** | Display voxel data to screen/texture | `VoxelRenderer` | `Renderer` |

Each API is a facade. Implementations vary. The API contract is stable.

---

## 1. MaterialAPI

### Purpose
Create, query, and manage materials in the database.

### Rust Trait

```rust
pub trait MaterialStore {
    /// Create a new material, returns its ID
    fn create(&mut self, def: MaterialDef) -> MaterialId;
    
    /// Get a material by ID
    fn get(&self, id: MaterialId) -> Option<&Material>;
    
    /// Update a material
    fn update(&mut self, id: MaterialId, def: MaterialDef) -> Result<()>;
    
    /// Delete a material
    fn delete(&mut self, id: MaterialId) -> Result<()>;
    
    /// Search materials by semantic query
    fn search(&self, query: &str, limit: usize) -> Vec<Material>;
    
    /// Find materials by tag
    fn find_by_tag(&self, tag: &str) -> Vec<Material>;
    
    /// Create a palette (collection of materials)
    fn create_palette(&mut self, name: &str, material_ids: &[MaterialId]) -> PaletteId;
    
    /// Get palette by ID
    fn get_palette(&self, id: PaletteId) -> Option<&Palette>;
}
```

### Lua Base Class

```lua
-- materials.lua
-- Base class for material operations (wraps Rust MaterialStore)

local Material = {}
Material.__index = Material

function Material.create(def)
    -- Calls Rust: material_store.create(def)
    -- Returns: MaterialId
    return _rust_material_create(def)
end

function Material.get(id)
    -- Calls Rust: material_store.get(id)
    return _rust_material_get(id)
end

function Material.search(query, limit)
    -- Calls Rust: material_store.search(query, limit)
    return _rust_material_search(query, limit or 10)
end

function Material.find_by_tag(tag)
    -- Calls Rust: material_store.find_by_tag(tag)
    return _rust_material_find_by_tag(tag)
end

-- Palette operations
local Palette = {}
Palette.__index = Palette

function Palette.create(name, material_ids)
    return _rust_palette_create(name, material_ids)
end

function Palette.get(id)
    return _rust_palette_get(id)
end

return {
    Material = Material,
    Palette = Palette,
}
```

### MCP Tools (for AI access)

```
create_material(def) -> MaterialId
get_material(id) -> Material
update_material(id, def) -> void
delete_material(id) -> void
search_materials(query, limit) -> Material[]
find_materials_by_tag(tag) -> Material[]
create_palette(name, material_ids) -> PaletteId
get_palette(id) -> Palette
```

---

## 2. GeneratorAPI

### Purpose
Produce voxel data. Many implementations possible:
- Markov Jr. models
- Random placement
- Direct voxel writing
- Shaders
- Noise functions
- Polar coordinates
- Live/streaming generation

### Rust Trait

```rust
/// The core generator trait - all generators implement this
pub trait VoxelGenerator {
    /// Initialize the generator with a seed and bounds
    fn init(&mut self, ctx: &mut GeneratorContext) -> GeneratorState;
    
    /// Run one step of generation
    /// Returns: whether more steps are needed
    fn step(&mut self, ctx: &mut GeneratorContext) -> StepResult;
    
    /// Clean up after generation
    fn teardown(&mut self, ctx: &mut GeneratorContext);
    
    /// Optional: post-process after all steps complete
    fn post_process(&mut self, _ctx: &mut GeneratorContext) {}
}

pub struct GeneratorContext {
    /// The voxel buffer we're writing to
    pub voxels: VoxelBuffer,
    /// The palette we're using
    pub palette: Palette,
    /// Random seed
    pub seed: u64,
    /// Bounds of generation
    pub bounds: Bounds3D,
    /// Current step number
    pub step_count: usize,
}

pub enum StepResult {
    /// Generation complete
    Done,
    /// Need more steps
    Continue,
    /// Error occurred
    Error(String),
}

pub enum GeneratorState {
    Ready,
    Error(String),
}
```

### Lua Base Class

```lua
-- generator.lua
-- Base class for all generators. Extend this to create custom generators.

local Generator = {}
Generator.__index = Generator

function Generator:new()
    local instance = setmetatable({}, self)
    return instance
end

-- Override these in subclasses --

function Generator:init(ctx)
    -- Called once at start
    -- ctx.voxels: the voxel buffer to write to
    -- ctx.palette: the palette to use
    -- ctx.seed: random seed
    -- ctx.bounds: {min={x,y,z}, max={x,y,z}}
    return "ready"  -- or "error: message"
end

function Generator:step(ctx)
    -- Called repeatedly until returns "done"
    -- Return: "done", "continue", or "error: message"
    return "done"
end

function Generator:post_process(ctx)
    -- Called after all steps complete (optional)
end

function Generator:teardown(ctx)
    -- Called at end for cleanup (optional)
end

-- Utility methods available to all generators --

function Generator:set_voxel(ctx, x, y, z, material_id)
    _rust_voxel_set(ctx.voxels, x, y, z, material_id)
end

function Generator:get_voxel(ctx, x, y, z)
    return _rust_voxel_get(ctx.voxels, x, y, z)
end

function Generator:fill_box(ctx, min, max, material_id)
    _rust_voxel_fill_box(ctx.voxels, min, max, material_id)
end

function Generator:random(ctx)
    -- Returns deterministic random based on seed + step
    return _rust_random(ctx.seed, ctx.step_count)
end

return Generator
```

### Example: Direct Voxel Writing

```lua
local Generator = require("generator")

local DirectWriter = Generator:new()

function DirectWriter:init(ctx)
    -- Nothing to set up
    return "ready"
end

function DirectWriter:step(ctx)
    -- Write a simple pattern directly
    for x = ctx.bounds.min.x, ctx.bounds.max.x do
        for z = ctx.bounds.min.z, ctx.bounds.max.z do
            -- Ground layer
            self:set_voxel(ctx, x, 0, z, self.ground_material)
        end
    end
    return "done"
end

return DirectWriter
```

### Example: Markov Jr. Wrapper

```lua
local Generator = require("generator")

local MarkovGenerator = Generator:new()

function MarkovGenerator:new(model_name)
    local instance = Generator.new(self)
    instance.model_name = model_name
    instance.markov_state = nil
    return instance
end

function MarkovGenerator:init(ctx)
    -- Load and initialize the Markov model
    self.markov_state = _rust_markov_init(self.model_name, ctx.seed, ctx.bounds)
    if not self.markov_state then
        return "error: failed to load model " .. self.model_name
    end
    return "ready"
end

function MarkovGenerator:step(ctx)
    -- Run one step of the Markov model
    local result = _rust_markov_step(self.markov_state)
    if result == "done" then
        -- Copy markov output to voxel buffer
        _rust_markov_copy_to_voxels(self.markov_state, ctx.voxels)
        return "done"
    elseif result == "continue" then
        return "continue"
    else
        return "error: " .. result
    end
end

function MarkovGenerator:teardown(ctx)
    if self.markov_state then
        _rust_markov_destroy(self.markov_state)
        self.markov_state = nil
    end
end

return MarkovGenerator
```

### Example: Composed Generator (Sequential)

```lua
local Generator = require("generator")

local SequenceGenerator = Generator:new()

function SequenceGenerator:new(generators)
    local instance = Generator.new(self)
    instance.generators = generators
    instance.current_index = 1
    return instance
end

function SequenceGenerator:init(ctx)
    self.current_index = 1
    if #self.generators == 0 then
        return "ready"
    end
    return self.generators[1]:init(ctx)
end

function SequenceGenerator:step(ctx)
    if self.current_index > #self.generators then
        return "done"
    end
    
    local current = self.generators[self.current_index]
    local result = current:step(ctx)
    
    if result == "done" then
        current:post_process(ctx)
        current:teardown(ctx)
        self.current_index = self.current_index + 1
        
        if self.current_index <= #self.generators then
            local init_result = self.generators[self.current_index]:init(ctx)
            if init_result ~= "ready" then
                return init_result
            end
            return "continue"
        else
            return "done"
        end
    end
    
    return result
end

return SequenceGenerator
```

### Example: Random Scatter

```lua
local Generator = require("generator")

local ScatterGenerator = Generator:new()

function ScatterGenerator:new(material_id, density, surface_only)
    local instance = Generator.new(self)
    instance.material_id = material_id
    instance.density = density or 0.01
    instance.surface_only = surface_only or false
    return instance
end

function ScatterGenerator:step(ctx)
    local air = 0  -- assuming 0 is air
    
    for x = ctx.bounds.min.x, ctx.bounds.max.x do
        for y = ctx.bounds.min.y, ctx.bounds.max.y do
            for z = ctx.bounds.min.z, ctx.bounds.max.z do
                if self:random(ctx) < self.density then
                    if self.surface_only then
                        -- Only place if on surface (air above)
                        local current = self:get_voxel(ctx, x, y, z)
                        local above = self:get_voxel(ctx, x, y + 1, z)
                        if current ~= air and above == air then
                            self:set_voxel(ctx, x, y + 1, z, self.material_id)
                        end
                    else
                        self:set_voxel(ctx, x, y, z, self.material_id)
                    end
                end
            end
        end
    end
    return "done"
end

return ScatterGenerator
```

### Example: Live/Streaming Generator

```lua
local Generator = require("generator")

local LiveGenerator = Generator:new()

function LiveGenerator:new(update_fn)
    local instance = Generator.new(self)
    instance.update_fn = update_fn
    instance.frame = 0
    return instance
end

function LiveGenerator:init(ctx)
    self.frame = 0
    return "ready"
end

function LiveGenerator:step(ctx)
    -- Never returns "done" - runs forever
    self.frame = self.frame + 1
    self.update_fn(ctx, self.frame)
    return "continue"
end

return LiveGenerator
```

---

## 3. RendererAPI

### Purpose
Display voxel data to screen, texture, or file.

### Rust Trait

```rust
pub trait VoxelRenderer {
    /// Initialize the renderer with target dimensions
    fn init(&mut self, ctx: &mut RenderContext) -> RendererState;
    
    /// Render one frame
    fn render(&mut self, ctx: &mut RenderContext) -> RenderResult;
    
    /// Clean up resources
    fn teardown(&mut self, ctx: &mut RenderContext);
    
    /// Get the rendered output (if applicable)
    fn get_output(&self) -> Option<&RenderOutput>;
}

pub struct RenderContext {
    /// The voxels to render
    pub voxels: &VoxelBuffer,
    /// The palette for material lookup
    pub palette: &Palette,
    /// Target width
    pub width: u32,
    /// Target height
    pub height: u32,
    /// Camera/view settings
    pub camera: Camera,
}

pub enum RenderOutput {
    /// Rendered to a texture handle
    Texture(TextureHandle),
    /// Rendered to a byte buffer (PNG, etc.)
    Bytes(Vec<u8>),
    /// Rendered directly to screen (no retrievable output)
    Screen,
}
```

### Lua Base Class

```lua
-- renderer.lua
-- Base class for all renderers. Extend to create custom renderers.

local Renderer = {}
Renderer.__index = Renderer

function Renderer:new()
    local instance = setmetatable({}, self)
    return instance
end

-- Override these in subclasses --

function Renderer:init(ctx)
    -- ctx.voxels: voxel buffer to render
    -- ctx.palette: palette for materials
    -- ctx.width, ctx.height: target dimensions
    -- ctx.camera: {pos, target, up, fov}
    return "ready"
end

function Renderer:render(ctx)
    -- Render one frame
    -- Return: "ok" or "error: message"
    return "ok"
end

function Renderer:teardown(ctx)
    -- Clean up
end

function Renderer:get_output()
    -- Return the render output (texture handle, bytes, or nil for screen)
    return nil
end

return Renderer
```

### Example: 2D Grid Renderer

```lua
local Renderer = require("renderer")

local GridRenderer2D = Renderer:new()

function GridRenderer2D:new(slice_y)
    local instance = Renderer.new(self)
    instance.slice_y = slice_y or 0
    instance.texture = nil
    return instance
end

function GridRenderer2D:init(ctx)
    -- Create texture for output
    self.texture = _rust_texture_create(ctx.width, ctx.height)
    return "ready"
end

function GridRenderer2D:render(ctx)
    -- Render a 2D slice at y = slice_y
    for x = 0, ctx.width - 1 do
        for z = 0, ctx.height - 1 do
            local material_id = _rust_voxel_get(ctx.voxels, x, self.slice_y, z)
            local material = _rust_palette_get_material(ctx.palette, material_id)
            local color = material and material.color or {0, 0, 0}
            _rust_texture_set_pixel(self.texture, x, z, color)
        end
    end
    return "ok"
end

function GridRenderer2D:get_output()
    return self.texture
end

function GridRenderer2D:teardown(ctx)
    if self.texture then
        _rust_texture_destroy(self.texture)
    end
end

return GridRenderer2D
```

### Example: 3D Deferred Renderer

```lua
local Renderer = require("renderer")

local DeferredRenderer3D = Renderer:new()

function DeferredRenderer3D:init(ctx)
    -- Initialize the full 3D deferred pipeline
    -- This wraps our existing Rust deferred renderer
    _rust_deferred_init(ctx.width, ctx.height)
    return "ready"
end

function DeferredRenderer3D:render(ctx)
    -- Mesh the voxels and render with deferred pipeline
    _rust_deferred_render(ctx.voxels, ctx.palette, ctx.camera)
    return "ok"
end

function DeferredRenderer3D:get_output()
    -- Renders to screen, no texture output
    return nil
end

function DeferredRenderer3D:teardown(ctx)
    _rust_deferred_teardown()
end

return DeferredRenderer3D
```

### Example: ImGui Embedded Renderer

```lua
local Renderer = require("renderer")

local ImGuiRenderer = Renderer:new()

function ImGuiRenderer:new(window_name)
    local instance = Renderer.new(self)
    instance.window_name = window_name or "Viewport"
    instance.inner_renderer = nil
    return instance
end

function ImGuiRenderer:set_inner(renderer)
    self.inner_renderer = renderer
end

function ImGuiRenderer:init(ctx)
    if self.inner_renderer then
        return self.inner_renderer:init(ctx)
    end
    return "ready"
end

function ImGuiRenderer:render(ctx)
    -- Begin ImGui window
    _rust_imgui_begin(self.window_name)
    
    -- Render inner content
    if self.inner_renderer then
        self.inner_renderer:render(ctx)
        local output = self.inner_renderer:get_output()
        if output then
            -- Display texture in ImGui
            _rust_imgui_image(output, ctx.width, ctx.height)
        end
    end
    
    _rust_imgui_end()
    return "ok"
end

return ImGuiRenderer
```

---

## API Evolution by Phase

### Phase 1: Foundation

**APIs Introduced:**
- `MaterialStore` (Rust) - SQLite backend
- `Material` (Lua) - Basic create/get/search

**APIs NOT YET:**
- `VoxelGenerator` - no generation yet
- `VoxelRenderer` - no rendering yet

**What You Can Do:**
```lua
local mat = require("materials")
local stone_id = mat.Material.create({
    name = "stone",
    color = {0.5, 0.5, 0.5},
    roughness = 0.7,
})
local found = mat.Material.search("stone", 10)
```

---

### Phase 2: 2D Generator

**APIs Extended:**
- `VoxelGenerator` (Rust) - Core trait
- `Generator` (Lua) - Base class with init/step/teardown
- `VoxelBuffer` (Rust) - 2D grid storage

**First Implementation:**
- `DirectWriter` - Direct voxel access

**What You Can Do:**
```lua
local Generator = require("generator")

local MyGen = Generator:new()
function MyGen:step(ctx)
    self:set_voxel(ctx, 5, 0, 5, stone_id)
    return "done"
end
```

---

### Phase 3: 2D Renderer

**APIs Extended:**
- `VoxelRenderer` (Rust) - Core trait
- `Renderer` (Lua) - Base class with init/render/teardown
- `GridRenderer2D` - Render 2D slice to texture

**What You Can Do:**
```lua
local GridRenderer2D = require("renderers/grid_2d")
local renderer = GridRenderer2D:new(0)  -- slice at y=0
renderer:init(ctx)
renderer:render(ctx)
local texture = renderer:get_output()
```

---

### Phase 4: Hot Reload

**APIs Extended:**
- `MaterialStore.on_change(callback)` - Subscribe to changes
- `Generator.on_reload()` - Hook for script reload

**What You Can Do:**
```lua
-- Generator automatically re-runs when script changes
-- Materials trigger re-render when updated
```

---

### Phase 5: MCP Server

**APIs Extended:**
- All `MaterialStore` methods exposed as MCP tools
- Generator execution exposed as MCP tools
- Renderer output exposed as MCP resources

**What You Can Do:**
```
// From external AI:
create_material({name: "crystal", color: [0.8, 0.2, 0.8], emission: 0.7})
run_generator("my_terrain.lua", {seed: 12345})
get_render_output()
```

---

### Phase 6: Markov Integration

**APIs Extended:**
- `MarkovGenerator` (Lua) - Wraps Rust Markov Jr. implementation
- `_rust_markov_init`, `_rust_markov_step`, etc.

**What You Can Do:**
```lua
local MarkovGenerator = require("generators/markov")
local terrain = MarkovGenerator:new("dungeon.xml")
```

---

### Phase 7: Composed Generators

**APIs Extended:**
- `SequenceGenerator` - Run generators in sequence
- `ParallelGenerator` - Run generators in parallel (different regions)
- `ConditionalGenerator` - Run based on conditions

**What You Can Do:**
```lua
local Sequence = require("generators/sequence")
local terrain = Sequence:new({
    MarkovGenerator:new("base_terrain.xml"),
    ScatterGenerator:new(crystal_id, 0.01, true),
    ErosionGenerator:new(2),
})
```

---

### Phase 8: 3D Extension

**APIs Extended:**
- `VoxelBuffer` - Now supports 3D (x, y, z)
- `DeferredRenderer3D` - Full 3D rendering
- All generators work in 3D

**What You Can Do:**
```lua
-- Same generator code, but now ctx.bounds has y dimension
self:set_voxel(ctx, x, y, z, material_id)
```

---

### Phase 9: Live Generators

**APIs Extended:**
- `LiveGenerator` - Never returns "done"
- `Generator:pause()`, `Generator:resume()`
- Frame-based callbacks

**What You Can Do:**
```lua
local LiveGenerator = require("generators/live")
local waves = LiveGenerator:new(function(ctx, frame)
    -- Update water height based on frame
    local height = math.sin(frame * 0.1) * 2
    -- ... update voxels
end)
```

---

### Phase 10: Shader Generators

**APIs Extended:**
- `ShaderGenerator` - Run compute shaders for generation
- `_rust_shader_dispatch(shader, voxels, params)`

**What You Can Do:**
```lua
local ShaderGenerator = require("generators/shader")
local noise = ShaderGenerator:new("perlin_terrain.wgsl", {
    octaves = 4,
    scale = 0.1,
})
```

---

## Summary: API Facades

| Facade | Rust Trait | Lua Class | First Phase |
|--------|------------|-----------|-------------|
| Materials | `MaterialStore` | `Material`, `Palette` | Phase 1 |
| Generation | `VoxelGenerator` | `Generator` | Phase 2 |
| Rendering | `VoxelRenderer` | `Renderer` | Phase 3 |
| Composition | (built on Generator) | `SequenceGenerator`, etc. | Phase 7 |
| Live | (built on Generator) | `LiveGenerator` | Phase 9 |
| Shaders | (built on Generator) | `ShaderGenerator` | Phase 10 |

Each phase extends existing APIs. No phase throws away previous work.
