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

### Phase 1: Procedural Sky Dome with Gradient

**Outcome:** A procedural sky renders behind all geometry, showing a smooth gradient from horizon color to zenith color that integrates with the day/night cycle.

**Verification:** 
1. Run `cargo run --example p30_markov_kinematic_animated`
2. Look upward - see smooth gradient sky (not flat fog color)
3. Horizon shows warm/muted color blending into fog
4. Zenith shows darker/cooler color
5. Colors change appropriately with day/night cycle time (if enabled)

**Tasks:**
1. Create `crates/studio_core/src/deferred/sky_dome.rs`:
   - `SkyDomeConfig` resource with `horizon_color`, `zenith_color`, `horizon_blend_height`
   - Integration point with `DayNightColorState` for dynamic colors
   
2. Create `crates/studio_core/assets/shaders/sky_dome.wgsl`:
   - Full-screen quad shader
   - Sample view direction from screen UV + inverse projection
   - Blend between horizon and zenith based on vertical angle
   - Output to screen where depth = max (no geometry)

3. Create `crates/studio_core/src/deferred/sky_dome_node.rs`:
   - Render graph node that runs after lighting pass
   - Only renders where depth buffer shows "sky" (depth > 999.0)
   - Reads from `SkyDomeConfig` uniform

4. Integrate into deferred pipeline in `crates/studio_core/src/deferred/plugin.rs`:
   - Add sky dome node to render graph after lighting
   - Register `SkyDomeConfig` resource

5. Update `examples/p30_markov_kinematic_animated.rs`:
   - Add `SkyDomeConfig` with dark fantasy colors

### Phase 2: Dual Moon Rendering in Sky

**Outcome:** Both moons from the day/night cycle are visible in the sky as glowing discs with horizon tinting effects.

**Verification:**
1. Run example with day/night cycle enabled
2. Look toward each moon's computed position - see glowing disc
3. Purple moon appears purple-tinted, orange moon appears orange-tinted
4. Moons near horizon show color shift (more saturated/warm)
5. Moons fade/disappear when below horizon (set_height)
6. Moon positions match shadow light directions

**Tasks:**
1. Extend `sky_dome.wgsl`:
   - Add uniforms for moon1/moon2: direction, color, size, intensity
   - Render moon as smooth disc with glow falloff
   - Apply horizon color shift based on moon altitude
   - Blend moon into sky gradient

2. Extend `SkyDomeConfig` in `sky_dome.rs`:
   - Moon rendering parameters (size, glow_falloff)
   - Link to `DualMoonConfig` for position/color data

3. Update `sky_dome_node.rs`:
   - Pass moon uniforms from day/night cycle state
   - Calculate moon screen positions from world directions

4. Create test: Look at sky during cycle, verify moons track correctly

### Phase 3: Mystery Palette for Generated Content

**Outcome:** Generated buildings use a cohesive dark fantasy palette instead of bright primary colors. Windows emit subtle glow rather than harsh red.

**Verification:**
1. Generate a building with G key
2. Building colors are muted: dark stone grays, deep purples, weathered browns
3. Window emission is subtle amber/cyan glow, not bright red
4. Overall building "fits" the mysterious atmosphere
5. Building stands out from gray terrain through subtle color, not harsh contrast

**Tasks:**
1. Create `crates/studio_core/src/markov_junior/palette_presets.rs`:
   - `MysteryPalette` preset with dark fantasy colors
   - Color mapping: 
     - `A` (building) -> dark gray-purple stone (#3a3540)
     - `R` (windows) -> deep amber (#8b5a00) with emission ~80
     - `F` (columns) -> weathered dark bronze (#4a4035)
     - `E` (grass) -> dark moss (#2a3a25)
     - `B` (empty) -> transparent/air
   - Document color choices in code comments

2. Update `RenderPalette` in `crates/studio_core/src/markov_junior/render.rs`:
   - Add `mystery_palette()` constructor
   - Ensure emission values are subtle (60-100 range, not 130-255)

3. Update `examples/p30_markov_kinematic_animated.rs`:
   - Use new mystery palette instead of default with red emission
   - Verify colors look cohesive

4. Take comparison screenshots: before/after palette change

### Phase 4: Configurable Voxel Scale

**Outcome:** Voxel size is configurable, allowing buildings to appear smaller relative to the character (target: character ~10-15 voxels tall).

**Verification:**
1. Run example with voxel scale = 0.5
2. Buildings appear half the previous size
3. Character (if rendered) is ~10 voxels tall visually
4. Collision detection still works correctly
5. Chunk boundaries render correctly at new scale

**Tasks:**
1. Add `VoxelScaleConfig` resource to `crates/studio_core/src/voxel.rs`:
   - `scale: f32` (default 1.0)
   - Global scale applied to all voxel positions

2. Update `crates/studio_core/src/voxel_mesh.rs`:
   - `build_merged_chunk_mesh` multiplies positions by scale
   - Chunk offset calculations account for scale

3. Update collision detection in `crates/studio_core/src/voxel_collision.rs`:
   - World-to-voxel coordinate conversion accounts for scale
   - Voxel-to-world conversion accounts for scale

4. Update `crates/studio_core/src/voxel_layer.rs`:
   - Layer offsets multiply by scale

5. Update example to use scale = 0.5:
   - Verify building size is reasonable
   - Adjust camera distance defaults if needed

6. Test: Walk around building, verify no collision glitches

### Phase 5: Extended Terrain with Height Variation

**Outcome:** Terrain extends to the horizon with gentle rolling hills, providing visual depth and scale.

**Verification:**
1. Run example and look toward horizon
2. Terrain visible all the way to fog fade-out
3. Ground has subtle height variation (not flat)
4. Performance acceptable (< 5ms frame time increase)
5. Gray/muted terrain colors convey "bleak default world"

**Tasks:**
1. Update `build_terrain()` in `examples/p30_markov_kinematic_animated.rs`:
   - Expand terrain to 400x400 (or larger based on fog distance)
   - Add simplex noise height variation (amplitude ~5 voxels)
   - Use dark gray stone palette for bleak aesthetic

2. Add simple noise function to `crates/studio_core/src/noise.rs`:
   - Basic 2D simplex or value noise
   - Sufficient for gentle terrain undulation

3. Optimize terrain generation:
   - Only generate terrain within fog_end distance
   - Consider chunk-based generation for larger areas

4. Adjust fog to match terrain extent:
   - `fog_end` should be slightly beyond visible terrain edge

5. Take screenshot showing terrain extending to horizon

### Phase 6: Atmospheric Polish

**Outcome:** Height-based fog creates depth, terrain and sky blend naturally at horizon.

**Verification:**
1. Fog is denser near ground level, fades upward
2. Looking at horizon shows smooth terrain-to-sky transition
3. Tall structures (buildings) partially emerge from ground fog
4. Overall atmosphere feels cohesive and mysterious

**Tasks:**
1. Update `deferred_lighting.wgsl`:
   - Add height-fog component: `fog_factor *= exp(-height * height_fog_density)`
   - Blend height fog with distance fog

2. Add height fog parameters to `DeferredLightingConfig`:
   - `height_fog_density: f32`
   - `height_fog_base: f32` (ground level)

3. Tune fog parameters for mystery aesthetic:
   - Ground-hugging mist
   - Clear views upward toward moons/sky

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
│   ├── render.rs            # MODIFIED: Add mystery_palette()
│   └── palette_presets.rs   # NEW: Palette preset definitions
├── voxel.rs                 # MODIFIED: Add VoxelScaleConfig
├── voxel_mesh.rs            # MODIFIED: Apply voxel scale
├── voxel_collision.rs       # MODIFIED: Scale-aware collision
└── noise.rs                 # NEW: Simple noise functions

crates/studio_core/assets/shaders/
└── sky_dome.wgsl            # NEW: Procedural sky shader

examples/
└── p30_markov_kinematic_animated.rs  # MODIFIED: Use new visuals
```

---

## How to Review

1. **Phase 1**: Run example, look up - verify gradient sky appears
2. **Phase 2**: Enable day/night, look for moons - verify they render and track correctly
3. **Phase 3**: Generate building - verify muted mysterious colors, subtle glow
4. **Phase 4**: Observe building scale - verify it feels appropriately sized
5. **Phase 5**: Look at horizon - verify extended terrain with height variation
6. **Phase 6**: Observe fog - verify height-based atmospheric effect

Each phase can be verified independently by running the example and observing specific visual elements.

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
