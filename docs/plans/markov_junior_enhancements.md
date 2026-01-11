# Plan: MarkovJunior Enhancements

## Overview

This document outlines the major enhancements needed for the MarkovJunior integration in creature_3d_studio. The goal is to make MarkovJunior a first-class procedural generation tool with proper real-time visualization, performance optimization, and integration with our voxel world system.

---

## Priority 1: Fix Building Position (Ground Level)

### Problem
Currently the generated building is centered at origin (0,0,0), so half of it is below ground level. It looks stupid.

### Solution
Calculate the model's bounding box and offset so the bottom of the building sits at ground level (y=3 for the platform).

### Implementation

**File:** `examples/p30_markov_kinematic_animated.rs`

**Changes:**
1. After getting the grid, calculate min Y value of all non-zero voxels
2. Apply offset so min Y maps to platform height (y=3)

```rust
fn build_building_mesh(grid: &MjGrid) -> Option<Mesh> {
    // Calculate bounds
    let mut min_y = f32::MAX;
    for (_, y, _, _) in grid.iter_nonzero() {
        let wy = z as f32 - offset_z; // After coord swap
        if wy < min_y { min_y = wy; }
    }
    
    // Offset to place bottom at y=0 (then Transform moves to y=3)
    let y_offset = -min_y;
    
    // Apply y_offset when creating cube positions
}
```

**Verification:**
- Building sits flush on platform
- No voxels below ground level

---

## Priority 2: Step API Refactor

### Problem
Current `model.step()` doesn't give fine-grained control. We want:
- Control how many "atomic operations" happen per call
- Pause/resume at any point
- Query progress percentage
- Time budget per step

### Current Architecture
```
model.step() 
  -> interpreter.step()
    -> root.go(&mut ctx)
      -> [recursive calls to child nodes]
      -> Each node does arbitrary work before returning
```

### Proposed Architecture

#### Option A: Work Budget System (Recommended)

Add a "budget" to ExecutionContext that limits work per step:

```rust
pub struct ExecutionContext<'a> {
    // ... existing fields ...
    
    /// Maximum atomic operations this step. 0 = unlimited.
    pub budget: usize,
    /// Operations performed this step.
    pub operations: usize,
    /// Whether we ran out of budget.
    pub budget_exhausted: bool,
}

impl ExecutionContext {
    /// Consume one operation from budget. Returns false if exhausted.
    pub fn consume_operation(&mut self) -> bool {
        if self.budget > 0 && self.operations >= self.budget {
            self.budget_exhausted = true;
            return false;
        }
        self.operations += 1;
        true
    }
}
```

Then in nodes like OneNode:
```rust
fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
    // ... find match ...
    
    if !ctx.consume_operation() {
        // Save state for resumption
        return true; // Still running
    }
    
    self.apply(&rule, x, y, z, ctx);
    // ...
}
```

**API for users:**
```rust
// Single atomic operation
model.step_one() -> StepResult

// N atomic operations  
model.step_n(n: usize) -> StepResult

// Step with time budget
model.step_timed(duration: Duration) -> StepResult

// Full step (current behavior)
model.step() -> bool
```

```rust
pub enum StepResult {
    /// Made progress, more work to do
    Progress { operations: usize },
    /// Completed successfully
    Complete { total_operations: usize },
    /// Failed (contradiction, etc)
    Failed { reason: String },
}
```

#### Phases

**Phase 2.1: Add ExecutionContext budget fields**
- Add `budget`, `operations`, `budget_exhausted` fields
- Add `consume_operation()` method
- No behavior change yet

**Phase 2.2: Implement budget checking in OneNode**
- Most common node type
- Return early when budget exhausted
- Track match state for resumption

**Phase 2.3: Implement budget checking in AllNode**
- Similar to OneNode but processes all matches

**Phase 2.4: Implement budget checking in WFC nodes**
- TileNode, OverlapNode
- Each observe+propagate = 1 operation

**Phase 2.5: Add step_one(), step_n() to Model**
- New public API
- Backward compatible (step() = step_n(unlimited))

**Phase 2.6: Add step_timed() to Model**
- Time-based stepping
- Good for frame-locked animation

---

## Priority 3: Render Performance Optimization

### Problem
Rebuilding the entire mesh every frame is slow, especially for large grids.

### Solutions

#### 3.1: Incremental Mesh Updates
Instead of rebuilding entire mesh, track changes and update only affected regions.

```rust
pub struct IncrementalMeshBuilder {
    /// Chunks of the mesh (e.g., 8x8x8 regions)
    chunks: HashMap<ChunkCoord, ChunkMesh>,
    /// Dirty chunks that need rebuild
    dirty_chunks: HashSet<ChunkCoord>,
}
```

**Phase 3.1.1:** Track changes from ExecutionContext
**Phase 3.1.2:** Implement chunk-based dirty tracking
**Phase 3.1.3:** Only rebuild dirty chunks

#### 3.2: Greedy Meshing for Generated Content
Currently we spawn individual cubes. Use greedy meshing to reduce vertex count.

**Phase 3.2.1:** Port greedy mesher to work with MjGrid directly
**Phase 3.2.2:** Update build_building_mesh to use greedy meshing

#### 3.3: LOD for Distant Views
For large buildings, reduce detail at distance.

**Phase 3.3.1:** Generate multiple LOD levels
**Phase 3.3.2:** Switch based on camera distance

---

## Priority 4: Integration with VoxelWorld

### Problem
Generated content should integrate seamlessly with the terrain VoxelWorld.

### Solutions

#### 4.1: Direct VoxelWorld Writing
Instead of separate mesh, write directly to VoxelWorld.

```rust
pub fn write_to_voxel_world(
    grid: &MjGrid,
    world: &mut VoxelWorld,
    offset: IVec3,
    palette: &RenderPalette,
) {
    for (x, y, z, value) in grid.iter_nonzero() {
        let ch = grid.characters[value as usize];
        let color = palette.get(ch);
        world.set_voxel(
            x + offset.x,
            z + offset.z, // Y/Z swap
            y + offset.y,
            Voxel::from_color(color),
        );
    }
}
```

**Pros:** 
- Unified collision detection
- Single render pass
- Proper lighting/shadows

**Cons:**
- Need to clear region before regenerating
- Slower for animated visualization

#### 4.2: Hybrid Approach
Use separate mesh during generation, then "bake" to VoxelWorld when complete.

**Phase 4.2.1:** Add `bake_to_voxel_world()` method
**Phase 4.2.2:** Update TerrainOccupancy after baking
**Phase 4.2.3:** Remove temporary generation mesh after bake

---

## Priority 5: Model Library and Selection

### Problem
Currently hardcoded to Apartemazements. Need UI to select models.

### Solution

#### 5.1: Model Registry
```rust
pub struct ModelRegistry {
    models: HashMap<String, ModelInfo>,
}

pub struct ModelInfo {
    pub name: String,
    pub path: PathBuf,
    pub default_size: (usize, usize, usize),
    pub category: ModelCategory,
    pub description: String,
}

pub enum ModelCategory {
    Building,
    Dungeon,
    Terrain,
    Pattern,
    Other,
}
```

**Phase 5.1.1:** Scan MarkovJunior/models/ on startup
**Phase 5.1.2:** Parse model XML for metadata
**Phase 5.1.3:** Categorize models

#### 5.2: ImGui Model Browser
**Phase 5.2.1:** Model list with categories
**Phase 5.2.2:** Preview thumbnails
**Phase 5.2.3:** Size configuration
**Phase 5.2.4:** Generate button

---

## Priority 6: Multi-Model Composition

### Problem
Want to place multiple generated structures in world.

### Solution

```rust
pub struct GenerationJob {
    pub model_name: String,
    pub position: IVec3,
    pub size: (usize, usize, usize),
    pub seed: u64,
    pub status: JobStatus,
}

pub enum JobStatus {
    Queued,
    Running { progress: f32 },
    Complete { result: MjGrid },
    Failed { error: String },
}

pub struct GenerationQueue {
    jobs: VecDeque<GenerationJob>,
    active_job: Option<GenerationJob>,
}
```

**Phase 6.1:** Job queue system
**Phase 6.2:** Background generation (non-blocking)
**Phase 6.3:** Progress tracking per job
**Phase 6.4:** Result placement in world

---

## Priority 7: Serialization and Persistence

### Problem
Generated content should be saveable/loadable.

### Solution

```rust
// Save generated structure
pub fn save_generated(grid: &MjGrid, path: &Path) -> Result<()>;

// Load previously generated structure
pub fn load_generated(path: &Path) -> Result<MjGrid>;

// Save as part of world
pub fn serialize_to_world_save(grid: &MjGrid) -> Vec<u8>;
```

**Phase 7.1:** Binary format for MjGrid
**Phase 7.2:** Integration with world save system
**Phase 7.3:** Generation parameters saved with structure

---

## Priority 8: Custom Model Creation

### Problem
Users should be able to create custom models.

### Solution

#### 8.1: Lua API for Custom Rules
Already partially implemented. Extend with:
- Visual rule editor
- Pattern from selection
- Test/preview in editor

#### 8.2: Import from Voxel Selection
- Select region of voxel world
- Convert to MjGrid pattern
- Use as input for rules

---

## Implementation Order

### Immediate (This Week)
1. **Fix building position** - Quick win, makes demo look better
2. **Step API Phase 2.1-2.2** - Foundation for better control

### Short Term (Next 2 Weeks)
3. **Render Performance 3.1** - Incremental updates
4. **Step API Phase 2.3-2.6** - Complete step API
5. **Integration 4.1** - VoxelWorld writing

### Medium Term (Next Month)
6. **Model Library 5.1-5.2** - Model browser
7. **Render Performance 3.2** - Greedy meshing
8. **Multi-Model 6.1-6.2** - Job queue

### Long Term
9. **Serialization** - Save/load
10. **Custom Models** - User content
11. **LOD** - Large-scale optimization

---

## File Structure

```
crates/studio_core/src/markov_junior/
├── mod.rs
├── model.rs              # Model API (modify for step API)
├── interpreter.rs        # Interpreter (modify for budget)
├── node.rs               # ExecutionContext (add budget fields)
├── integration/
│   ├── mod.rs
│   ├── voxel_bridge.rs   # VoxelWorld integration
│   ├── mesh_builder.rs   # Incremental mesh building
│   └── registry.rs       # Model registry
└── ui/
    └── model_browser.rs  # ImGui model browser
```

---

## Success Metrics

1. **Performance:** 60 FPS during animated generation of 16x16x16 model
2. **Control:** Can step exactly 1 atomic operation at a time
3. **Integration:** Generated content has collision detection
4. **Usability:** Can browse and select any model via UI
5. **Visual Quality:** Building sits properly on ground, looks good

---

## Dependencies

- Bevy 0.17 (current)
- studio_core VoxelWorld
- ImGui integration (for UI)
- MarkovJunior models (submodule)

---

## Risks

1. **Budget system complexity** - May need significant refactor of node state
2. **Performance** - Incremental updates may have edge cases
3. **WFC state resumption** - WFC nodes have complex internal state
4. **Coordinate systems** - Y/Z swap already confusing, more transforms add risk
