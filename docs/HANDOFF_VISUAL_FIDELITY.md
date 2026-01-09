# Handoff: Visual Fidelity Improvements Feature Branch

## Branch Information
- **Branch:** `feature/visual-fidelity-improvements`
- **Base:** `main`
- **Status:** Phase 0 complete, Phase 1 in progress

---

## Critical Documents to Read First

### 1. How We Work (MANDATORY)
**File:** `docs/HOW_WE_WORK.md`

This document defines our collaboration process. Key principles:
- **Verification is first-class**: Every phase must have simple, automated verification
- **Facade pattern**: Build end-to-end pipeline first with trivial output, then complexify
- **No manual verification**: Use automated screenshots, not "run and look around"
- **Hypothesis-driven debugging**: When something fails, form hypothesis, test, iterate
- **Never abandon tasks**: Debug systematically, don't substitute simpler alternatives

### 2. The Plan Document (MANDATORY)
**File:** `docs/plans/visual_fidelity_improvements.md`

This is the implementation plan with 9 phases (0-8). Each phase has:
- **Outcome**: What will be true when complete
- **Verification**: How to prove it's done (must be simple bash commands + screenshot checks)
- **Tasks**: Specific file paths and changes

### 3. Deferred Rendering Architecture
**Directory:** `crates/studio_core/src/deferred/`

Key files to understand:
- `plugin.rs` - How render graph nodes are registered and ordered
- `labels.rs` - Render graph node labels
- `lighting_node.rs` - Example of a fullscreen pass node (model for sky dome)
- `bloom_node.rs` - Another post-process node example
- `lighting.rs` - DeferredLightingConfig resource pattern

### 4. Day/Night Cycle System
**File:** `crates/studio_core/src/day_night.rs`

The sky dome needs to integrate with this for moon positions and colors.
- `MoonCycleConfig` - Per-moon orbital parameters
- `DayNightCycle` - Main cycle resource
- `DayNightColorState` - Current interpolated colors

---

## Current State

### Completed: Phase 0 - Visual Verification Test Harness

**File created:** `examples/p31_visual_fidelity_test.rs`

This test harness:
- Captures 5 screenshots from different camera angles
- Auto-exits after capture
- Output: `screenshots/visual_fidelity_test/`
  - `sky_up.png` - Looking straight up
  - `sky_horizon.png` - Looking at horizon
  - `building_front.png` - Front view of building area
  - `building_aerial.png` - Top-down aerial view
  - `terrain_distance.png` - Distant terrain view

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test
ls screenshots/visual_fidelity_test/
# Should show 5 PNG files
```

### In Progress: Phase 1 - Sky Dome Pipeline (Facade)

**Goal:** Get a sky dome shader running in the deferred pipeline that outputs constant purple where depth > 999.0 (no geometry).

**Files to create:**
1. `crates/studio_core/src/deferred/sky_dome.rs` - Config resource
2. `assets/shaders/sky_dome.wgsl` - Fullscreen shader
3. `crates/studio_core/src/deferred/sky_dome_node.rs` - Render graph node

**Key insight:** The sky dome should run AFTER bloom pass, filling in sky color where there's no geometry. Current order ends with:
```
LightingPass -> BloomPass -> MainTransparentPass
```
Sky dome should go:
```
LightingPass -> BloomPass -> SkyDomePass -> MainTransparentPass
```

---

## Remaining Phases (Summary)

| Phase | Description | Key Change |
|-------|-------------|------------|
| 1 | Sky Dome Pipeline (Facade) | Constant purple sky |
| 2 | Sky Gradient | Horizon-to-zenith color blend |
| 3 | Moon Rendering | Glowing moon discs in sky |
| 4 | Moon Horizon Effects | Color shift near horizon |
| 5 | Mystery Palette | Dark fantasy building colors |
| 6 | Voxel Scale | Configurable voxel size |
| 7 | Extended Terrain | Rolling hills to horizon |
| 8 | Height Fog | Ground-hugging atmospheric fog |

---

## Key Code Patterns

### Render Graph Node Pattern
From `lighting_node.rs`:
```rust
#[derive(Default)]
pub struct LightingPassNode;

impl ViewNode for LightingPassNode {
    type ViewQuery = (/* components needed from view */);

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        query_item: QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // Get pipeline from cache
        // Create bind groups
        // Run render pass
        Ok(())
    }
}
```

### Pipeline Initialization Pattern
From `lighting_node.rs`:
```rust
pub fn init_lighting_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
) {
    // Load shader
    let shader = asset_server.load("shaders/deferred_lighting.wgsl");
    
    // Create bind group layouts
    // Queue pipeline creation
    // Insert pipeline resource
}
```

### Registering Node in Plugin
From `plugin.rs`:
```rust
render_app
    .add_render_graph_node::<ViewNodeRunner<BloomNode>>(
        Core3d, 
        DeferredLabel::BloomPass
    );

render_app.add_render_graph_edges(
    Core3d,
    (
        DeferredLabel::LightingPass,
        DeferredLabel::BloomPass,
        Node3d::MainTransparentPass,
    ),
);
```

---

## Shader Location

Shaders are loaded via asset server from `assets/shaders/`. Example:
```rust
let shader = asset_server.load("shaders/deferred_lighting.wgsl");
```

The sky dome shader should be at: `assets/shaders/sky_dome.wgsl`

---

## Verification Table

All verification uses the test harness:
```bash
cargo run --example p31_visual_fidelity_test
```

| Phase | Screenshot | What to Look For |
|-------|------------|------------------|
| 0 | All 5 exist | Harness works |
| 1 | `sky_up.png` | Solid purple (not fog color) |
| 2 | `sky_up.png` | Gradient: darker zenith, warmer horizon |
| 3 | `sky_horizon.png` | Moon discs visible with glow |
| 4 | `sky_horizon.png` | Moon color shifts near horizon |
| 5 | `building_front.png` | Muted colors, subtle amber glow |
| 6 | `building_front.png` | Building appears smaller |
| 7 | `terrain_distance.png` | Rolling hills to horizon |
| 8 | `building_aerial.png` | Ground fog obscures base |

---

## Common Pitfalls

1. **Don't use manual verification** - Always use the test harness
2. **Build facade first** - Constant purple sky proves pipeline works before adding gradient
3. **Check render graph order** - Sky dome must run after bloom, before transparent
4. **Shaders load async** - Pipeline may not be ready on first frame; handle gracefully
5. **ViewNode needs ViewTarget** - To write to the screen, query `ViewTarget`

---

## File Structure (What to Create)

```
crates/studio_core/
├── src/deferred/
│   ├── sky_dome.rs          # NEW: SkyDomeConfig resource
│   ├── sky_dome_node.rs     # NEW: Render graph node
│   ├── labels.rs            # MODIFY: Add SkyDomePass label
│   ├── plugin.rs            # MODIFY: Register node and edges
│   └── mod.rs               # MODIFY: Export new modules

assets/shaders/
└── sky_dome.wgsl            # NEW: Sky dome shader
```

---

## Dependencies

- Bevy 0.17
- Existing deferred rendering pipeline
- Day/night cycle system (for moon data in later phases)

---

## Quick Start Commands

```bash
# Build and run test harness
cargo run --example p31_visual_fidelity_test

# Check screenshots
ls screenshots/visual_fidelity_test/

# Build only (faster iteration)
cargo build --example p31_visual_fidelity_test
```

---

## Contact/Context

This work is part of improving visual fidelity for the MarkovJunior procedural generation demo. The goal is to make screenshots compelling enough that viewers understand "this is something interesting" rather than "this is a programmer's test scene."

Key visual goals:
- Procedural sky with dual moons
- Dark fantasy / mysterious aesthetic
- Buildings that don't look oversized
- Terrain extending to misty horizon
