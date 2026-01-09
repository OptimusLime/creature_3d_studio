# Visual Fidelity Improvements Plan

## Summary

Transform the current prototype visuals into a compelling, mysterious aesthetic suitable for showcasing the MarkovJunior procedural generation system. This includes adding a procedural skybox with dual moons, implementing configurable voxel scale, expanding terrain to the horizon, and refining the color palette for a cohesive dark fantasy mood.

## Context & Motivation

The current demo looks "unfinished" for several reasons:
- **No skybox**: Looking up or into the distance shows flat fog color
- **Large voxels**: Buildings appear oversized relative to the character
- **Limited terrain**: Small 80x80 platform doesn't convey scale or atmosphere
- **Bright colors**: The palette (especially bright red windows) feels jarring rather than mysterious
- **Missing moons**: The day/night system has dual moons, but they're invisible in the sky

The goal is to make screenshots and videos visually compelling enough that someone seeing them understands "this is something interesting" rather than "this is a programmer's test scene."

## Naming Conventions for This PR

### Files
- Sky-related: `sky_*.rs`, `sky_*.wgsl` (e.g., `sky_dome.rs`, `sky_dome.wgsl`)
- Voxel scale: Changes to existing `voxel.rs`, `voxel_mesh.rs`
- Palette: `mj_palette_presets.rs` or modifications to `render.rs`

### Shaders
- Sky dome shader: `sky_dome.wgsl`
- Constants prefix: `SKY_`, `MOON_`, `ATMOSPHERE_`

### Components/Resources
- `SkyDomeConfig` - sky appearance configuration
- `VoxelScaleConfig` - global voxel scale factor
- `MysteryPalette` - the new dark palette preset

---

## Phases

### Phase 0: Visual Verification Test Harness

**Outcome:** A dedicated test example (`p31_visual_fidelity_test.rs`) that automatically captures screenshots from multiple camera angles, providing easy verification for all subsequent phases.

**Verification:** 
```bash
cargo run --example p31_visual_fidelity_test
# Exits automatically after capturing screenshots
ls screenshots/visual_fidelity_test/
# Shows: sky_up.png, sky_horizon.png, building_front.png, building_aerial.png, terrain_distance.png
```

**Tasks:**
1. Create `examples/p31_visual_fidelity_test.rs`:
   - Uses `DebugScreenshotConfig` with multiple captures
   - Each capture has a specific camera position/angle
   - Captures: looking up (sky), looking at horizon (sky+terrain), building close-up, aerial view, distant terrain
   - Auto-exits after all screenshots captured

2. Create `VisualTestCapture` helper in test example:
   - Struct holding camera position, look_at target, and capture name
   - System that sequences through captures, repositioning camera between shots

3. Define standard test scene:
   - Platform terrain (existing)
   - Pre-generated building at known position (or generate with fixed seed)
   - Known camera positions for each shot type

4. Verify harness works:
   - Run example, confirm 5 screenshots appear in `screenshots/visual_fidelity_test/`
   - Screenshots show different angles of same scene

**Why Phase 0 First:**
All subsequent phases add visual features. Without automated screenshot capture from known angles, verification requires manual "run, walk around, look up, observe" - which is slow and error-prone. This harness makes verification trivial: run test, look at screenshots.

---

### Phase 1: Sky Dome Pipeline (Facade)

**Outcome:** Sky dome shader runs in the deferred pipeline, outputting a constant color (purple) where there's no geometry. This proves the pipeline works before adding complexity.

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test
# Check screenshots/visual_fidelity_test/sky_up.png
# Should show solid purple (not fog color) where sky is visible
```

**Tasks:**
1. Create `crates/studio_core/src/deferred/sky_dome.rs`:
   - `SkyDomeConfig` resource (for now: just `enabled: bool`)
   - Minimal config, no gradient yet

2. Create `crates/studio_core/assets/shaders/sky_dome.wgsl`:
   - Full-screen pass shader
   - Returns constant `vec4(0.2, 0.1, 0.3, 1.0)` (purple) where depth > 999.0
   - Returns previous color otherwise (passthrough)

3. Create `crates/studio_core/src/deferred/sky_dome_node.rs`:
   - Render graph node running after lighting pass
   - Binds depth texture and output texture
   - Runs sky_dome.wgsl

4. Integrate in `crates/studio_core/src/deferred/plugin.rs`:
   - Add node to render graph
   - Register resource

5. Update test example to enable sky dome

**Key Principle:** Get the pipeline working with trivial output first. Constant purple sky proves: shader loads, node runs, depth test works, output appears.

### Phase 2: Sky Gradient (Complexify Sky Dome)

**Outcome:** Sky shows smooth gradient from horizon to zenith instead of constant color.

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test
# Check screenshots/visual_fidelity_test/sky_up.png - darker zenith color
# Check screenshots/visual_fidelity_test/sky_horizon.png - warmer horizon color blending to terrain
```

**Tasks:**
1. Add uniforms to `sky_dome.wgsl`:
   - `horizon_color: vec3<f32>`
   - `zenith_color: vec3<f32>`
   - `horizon_blend_power: f32`

2. Update shader to compute view direction from UV + inverse projection:
   - Calculate vertical angle from view direction
   - Blend horizon_color → zenith_color based on angle

3. Update `SkyDomeConfig` with gradient parameters

4. Update test to use dark fantasy gradient colors

---

### Phase 3: Moon Rendering in Sky

**Outcome:** Both moons visible as glowing discs at their computed positions.

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test
# Check screenshots/visual_fidelity_test/sky_horizon.png
# Moons visible as glowing discs (if above horizon at test time)
# Moon colors match configured colors (purple/orange)
```

**Tasks:**
1. Add moon uniforms to `sky_dome.wgsl`:
   - `moon1_direction: vec3<f32>`, `moon1_color: vec3<f32>`, `moon1_size: f32`
   - `moon2_direction: vec3<f32>`, `moon2_color: vec3<f32>`, `moon2_size: f32`

2. Render moons as smooth discs:
   - Calculate angle between view direction and moon direction
   - Draw disc with soft falloff glow
   - Skip if moon below horizon (direction.y < 0)

3. Update `SkyDomeConfig` and node to pass moon data from day/night cycle

4. Add test capture with camera pointed at known moon position

---

### Phase 4: Moon Horizon Effects

**Outcome:** Moons near horizon show atmospheric color shift (warmer/more saturated).

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test
# Compare moon color at zenith vs near horizon in different test captures
# Horizon moon should appear more saturated/warm
```

**Tasks:**
1. Add horizon tinting to moon rendering in shader:
   - Blend moon color toward horizon_color based on moon altitude
   - Increase saturation near horizon

2. Add `horizon_tint_strength` parameter to config

3. Update test to capture moon at different altitudes (may need day/night cycle time control)

### Phase 5: Mystery Palette for Generated Content

**Outcome:** Generated buildings use a cohesive dark fantasy palette instead of bright primary colors. Windows emit subtle glow rather than harsh red.

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test
# Check screenshots/visual_fidelity_test/building_front.png
# Building should show: muted stone grays, subtle amber window glow
# Compare to baseline screenshot (saved before this phase) - colors should be distinctly different
```

**Tasks:**
1. Add `mystery_palette()` to `crates/studio_core/src/markov_junior/render.rs`:
   - Color mapping: 
     - `A` (building) -> dark gray-purple stone (#3a3540)
     - `R` (windows) -> deep amber (#8b5a00) with emission ~80
     - `F` (columns) -> weathered dark bronze (#4a4035)
     - `E` (grass) -> dark moss (#2a3a25)
     - `B` (empty) -> transparent/air

2. Update test example to use `mystery_palette()`

3. Save baseline screenshot BEFORE this phase for comparison

---

### Phase 6: Configurable Voxel Scale

**Outcome:** Voxel size is configurable via `VoxelScaleConfig`. Buildings appear smaller relative to world.

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test -- --scale=0.5
# Check screenshots - building should appear half the size compared to scale=1.0
# Run collision test: cargo test voxel_scale_collision -- should pass
```

**Tasks:**
1. Add `VoxelScaleConfig` resource to `crates/studio_core/src/voxel.rs`:
   - `scale: f32` (default 1.0)

2. Update `crates/studio_core/src/voxel_mesh.rs`:
   - `build_merged_chunk_mesh` multiplies positions by scale

3. Update collision in `crates/studio_core/src/voxel_collision.rs`:
   - Coordinate conversions account for scale

4. Add `--scale` CLI argument to test example

5. Add unit test for scaled collision detection

---

### Phase 7: Extended Terrain with Height Variation

**Outcome:** Terrain extends to the horizon with gentle rolling hills, providing visual depth and scale.

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test
# Check screenshots/visual_fidelity_test/terrain_distance.png
# Terrain should extend to fog fade-out (not abrupt edge)
# Ground should show subtle height variation (rolling hills, not flat)
```

**Tasks:**
1. Add simple noise function to `crates/studio_core/src/noise.rs`:
   - 2D value noise or simplex noise
   - `noise_2d(x: f32, z: f32, scale: f32) -> f32` returning 0.0-1.0

2. Update `build_terrain()` in test example:
   - Expand to 400x400
   - Apply noise for height variation (amplitude ~5 voxels)
   - Use dark gray stone (#2a2a2a) for bleak aesthetic

3. Adjust fog_end to match terrain extent

4. Verify performance: terrain generation should complete < 1 second

---

### Phase 8: Height-Based Fog

**Outcome:** Fog is denser near ground level, creating atmospheric depth. Buildings emerge from mist.

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test
# Check screenshots/visual_fidelity_test/building_aerial.png
# Building base should be partially obscured by ground fog
# Building top should be clearer
# Horizon should show smooth terrain-to-sky transition
```

**Tasks:**
1. Update `deferred_lighting.wgsl`:
   - Add `height_fog_density` and `height_fog_base` uniforms
   - Compute height fog: `height_fog = exp(-(world_pos.y - base) * density)`
   - Combine with distance fog: `final_fog = max(distance_fog, height_fog)`

2. Add parameters to `DeferredLightingConfig`:
   - `height_fog_density: f32` (default 0.05)
   - `height_fog_base: f32` (default 0.0)

3. Tune parameters in test example for mysterious atmosphere

4. Update example with tuned fog values

---

## Full Outcome Across All Phases

When complete, the demo will show:
- **Sky**: Procedural gradient sky with visible dual moons that track the day/night cycle
- **Moons**: Purple and orange moons with glow, horizon tinting effects
- **Buildings**: Dark fantasy aesthetic with subtle amber/cyan window glow
- **Scale**: Voxels scaled so buildings feel appropriately sized (not giant blocks)
- **Terrain**: Rolling gray hills extending to the misty horizon
- **Atmosphere**: Height-based fog creating mysterious depth

The visual impression should be: "A mysterious, procedurally-generated world emerging from gray bleakness."

---

## Directory Structure (Anticipated)

```
crates/studio_core/src/
├── deferred/
│   ├── sky_dome.rs          # NEW: Sky dome config and integration
│   ├── sky_dome_node.rs     # NEW: Render graph node for sky
│   ├── lighting.rs          # MODIFIED: Add height fog params
│   └── plugin.rs            # MODIFIED: Register sky dome node
├── markov_junior/
│   └── render.rs            # MODIFIED: Add mystery_palette()
├── voxel.rs                 # MODIFIED: Add VoxelScaleConfig
├── voxel_mesh.rs            # MODIFIED: Apply voxel scale
├── voxel_collision.rs       # MODIFIED: Scale-aware collision
└── noise.rs                 # NEW: Simple noise functions

crates/studio_core/assets/shaders/
└── sky_dome.wgsl            # NEW: Procedural sky shader

examples/
├── p31_visual_fidelity_test.rs  # NEW: Automated visual verification test
└── p30_markov_kinematic_animated.rs  # MODIFIED: Use new visuals

screenshots/
└── visual_fidelity_test/    # Output from p31 test
    ├── sky_up.png
    ├── sky_horizon.png
    ├── building_front.png
    ├── building_aerial.png
    └── terrain_distance.png
```

---

## How to Review

**All verification is automated via the test harness.**

For each phase:
```bash
cargo run --example p31_visual_fidelity_test
ls screenshots/visual_fidelity_test/
# Open screenshots and verify expected visual changes
```

| Phase | Screenshot to Check | What to Look For |
|-------|---------------------|------------------|
| 0 | All 5 screenshots exist | Harness works |
| 1 | `sky_up.png` | Solid purple sky (not fog) |
| 2 | `sky_up.png` | Gradient: darker zenith, warmer horizon |
| 3 | `sky_horizon.png` | Moon discs visible with glow |
| 4 | `sky_horizon.png` | Moon color shifts near horizon |
| 5 | `building_front.png` | Muted colors, subtle amber glow |
| 6 | `building_front.png` | Building appears smaller |
| 7 | `terrain_distance.png` | Rolling hills to horizon |
| 8 | `building_aerial.png` | Ground fog obscures base |

---

## Future Work (Out of Scope)

These items are noted for future consideration but NOT part of this PR:

- **Volumetric fog/god rays**: Would enhance atmosphere but requires shadow map sampling
- **Stars in night sky**: Procedural star field when moons are below horizon
- **Cloud layer**: Animated cloud shadows
- **Biome-based terrain colors**: Different areas with different palettes
- **Chunk streaming for infinite terrain**: On-demand terrain generation
- **Multiple generator support**: 5-10 concurrent MarkovJunior generators
- **Performance profiling**: Frame-by-frame analysis of generation slowdown

---

## Dependencies

- Bevy 0.17 (current)
- Existing deferred rendering pipeline
- Existing day/night cycle system
- MarkovJunior integration (for building generation)

---

## Risks

1. **Sky dome integration complexity**: May require careful render graph ordering
2. **Voxel scale collision edge cases**: Floating point precision at small scales
3. **Performance at larger terrain**: May need LOD or chunking
4. **Color palette subjectivity**: "Mysterious" is somewhat subjective - may need iteration
