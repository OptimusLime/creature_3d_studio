# Visual Fidelity Roadmap: From Current to Enhanced

## Summary

Transform the current voxel renderer from a flat, basic appearance to a rich, atmospheric dark fantasy aesthetic matching the reference enhanced screenshot. This plan identifies specific techniques by comparing the original and enhanced screenshots.

## Context

**Original State:** `screenshots/references/screenshot_voxel_moons.jpg`
- Flat terrain with uniform coloring
- Simple dual moon discs
- Thin, barely visible clouds
- No atmospheric depth
- Basic lighting without color bleeding
- Minimal ambient occlusion

**Target State:** `screenshots/references/screenshot_ai_enhanced_moons.jpeg`
- Rich terrain with varied textures (moss, stone, leaves)
- Detailed moons with surface texture
- Dramatic volumetric clouds with moon tinting
- Strong atmospheric haze and depth fog
- Color-rich lighting with moon-tinted ambient
- Pronounced ambient occlusion and contact shadows

## Latest Example

The main example to run and improve is **`p34_sky_terrain_test.rs`**, which integrates:
- Sky dome with dual moons
- Large rolling terrain
- Character controller for navigation
- Day/night cycle controls

```bash
cargo run --example p34_sky_terrain_test
```

---

## Techniques Comparison Matrix

| Feature | Current | Enhanced | Priority | Effort |
|---------|---------|----------|----------|--------|
| Moon Surface Detail | Flat disc | Crater/texture detail | HIGH | Medium |
| Cloud Density | Thin wisps | Volumetric puffs | HIGH | High |
| Cloud Moon-Tinting | None | Purple/orange glow | HIGH | Low |
| Atmospheric Haze | Basic fog | Height-based + distance | HIGH | Medium |
| Ground Variation | Uniform color | Multi-material (moss/stone/leaves) | MEDIUM | Medium |
| Ambient Occlusion | GTAO present | Stronger/more visible | MEDIUM | Low |
| Color Grading | Linear | Moody/saturated | MEDIUM | Low |
| Point Light Bloom | Basic | Dramatic glow halos | MEDIUM | Low |
| Procedural Props | Light posts only | Trees, ruins, debris | LOW | High |
| Particle Effects | None | Floating embers/dust | LOW | High |
| Depth of Field | None | Subtle background blur | LOW | Medium |

---

## Implementation Phases

### Phase F1: Moon Surface Enhancement
**Priority:** HIGHEST  
**Outcome:** Moons display visible surface detail (craters, texture variation)

**Technique:**
- Use MJ-generated moon textures already at `assets/textures/generated/mj_moon_purple.png` and `mj_moon_orange.png`
- Apply texture sampling in `sky_dome.wgsl` moon rendering
- Add surface normal perturbation for 3D appearance
- Implement limb darkening (edges slightly darker)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Moons should show visible crater/texture detail, not flat discs
```

**Files:**
- `assets/shaders/sky_dome.wgsl` - Add texture sampling to moon rendering
- `crates/studio_core/src/deferred/sky_dome_node.rs` - Bind moon textures

---

### Phase F2: Volumetric Cloud Enhancement
**Priority:** HIGH  
**Outcome:** Clouds appear as thick, volumetric puffs with depth

**Technique:**
- Increase cloud density in shader (current is too transparent)
- Add multi-layer cloud rendering (high and low altitude)
- Implement cloud edge scattering (brighter edges when backlit by moons)
- Add subtle cloud animation (drift over time)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Clouds should appear thick and puffy, not thin wisps
```

**Files:**
- `assets/shaders/sky_dome.wgsl` - Modify cloud rendering
- `crates/studio_core/src/deferred/sky_dome.rs` - Add cloud density config

---

### Phase F3: Moon-Tinted Clouds
**Priority:** HIGH  
**Outcome:** Clouds take on purple/orange tint from nearby moons

**Technique:**
- Calculate cloud-to-moon angle in shader
- Blend cloud color toward moon color based on proximity
- Stronger tinting for clouds near moon position
- Implement subsurface scattering approximation

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Clouds near purple moon should have purple tint
# Clouds near orange moon should have orange tint
```

**Files:**
- `assets/shaders/sky_dome.wgsl` - Add moon proximity color blending

---

### Phase F4: Enhanced Atmospheric Depth
**Priority:** HIGH  
**Outcome:** Scene has strong depth through layered atmospheric effects

**Technique:**
- Height-based fog (already partially implemented, needs tuning)
- Increase fog density near ground
- Add atmospheric scattering near horizon (sky meets terrain)
- Color-shift fog based on dominant moon (purple/orange tint)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Distant terrain should fade into colored haze
# Ground-level mist should partially obscure bases of structures
```

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Enhance fog calculation
- `crates/studio_core/src/deferred/lighting.rs` - Add fog tint config

---

### Phase F5: Stronger Ambient Occlusion
**Priority:** MEDIUM  
**Outcome:** AO is more pronounced in corners and crevices

**Technique:**
- Increase GTAO intensity (currently subtle)
- Adjust radius for smaller-scale details
- Darken minimum AO value
- Consider contact shadows for ground intersection

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Corners of structures should show clear darkening
# Base of light posts should have ground contact shadows
```

**Files:**
- `crates/studio_core/src/deferred/gtao.rs` - Adjust intensity/radius parameters
- `assets/shaders/gtao.wgsl` - Tune falloff curve

---

### Phase F6: Cinematic Color Grading
**Priority:** MEDIUM  
**Outcome:** Overall image has moody, saturated aesthetic

**Technique:**
- Implement color LUT or procedural color grading in bloom composite
- Increase saturation slightly
- Push shadows toward purple, highlights toward orange
- Add subtle vignette (darker corners)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Overall image should feel more dramatic and cinematic
# Colors should "pop" more than current flat appearance
```

**Files:**
- `assets/shaders/bloom_composite.wgsl` - Add color grading pass
- `crates/studio_core/src/deferred/bloom.rs` - Add grading config

---

### Phase F7: Enhanced Point Light Bloom
**Priority:** MEDIUM  
**Outcome:** Light sources have dramatic glow halos

**Technique:**
- Increase bloom intensity for high-emission sources
- Add glow halo effect around point lights
- Implement light shaft hints for ground lights

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Light posts should have visible glow halos
# Emissive voxels should bloom noticeably
```

**Files:**
- `assets/shaders/bloom_downsample.wgsl` - Adjust threshold/intensity
- `crates/studio_core/src/deferred/bloom.rs` - Tune parameters

---

### Phase F8: Terrain Color Variation
**Priority:** MEDIUM  
**Outcome:** Ground shows varied colors (not uniform)

**Technique:**
- Use noise-based color variation in terrain generation
- Add "moss" patches, "stone" areas, "leaf litter" zones
- Color variation based on height and slope
- Consider terrain texture atlas (future)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Terrain should show varied colors, not single uniform tone
```

**Files:**
- `examples/p34_sky_terrain_test.rs` - Modify `build_rolling_hills_terrain()`
- Consider new `terrain_generator.rs` module

---

### Phase F9: Procedural Props (Trees, Ruins)
**Priority:** LOW (High effort)  
**Outcome:** Scene has varied structures beyond light posts

**Technique:**
- Create MJ models for dead trees
- Create MJ models for stone ruins/walls
- Scatter placement using noise
- Ensure props integrate with terrain height

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Scene should include dead trees and ruined structures
```

**Files:**
- `MarkovJunior/models/DeadTree.xml` (new)
- `MarkovJunior/models/StoneRuin.xml` (new)
- `examples/p34_sky_terrain_test.rs` - Add prop spawning

---

### Phase F10: Particle Effects
**Priority:** LOW (High effort)  
**Outcome:** Floating embers, dust, magical particles

**Technique:**
- GPU particle system using compute shaders
- Particles follow wind direction
- Emit from light sources and certain voxel types
- Additive blending for glow effect

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Floating particles visible around lights and in air
```

**Files:**
- New particle system module
- New compute shader for particle simulation

---

## Dependency Graph

```
F1 (Moon Texture) ──────────────────────┐
                                        │
F2 (Volumetric Clouds) ─────────────────┤
        │                               │
        └──> F3 (Moon-Tinted Clouds) ───┴──> VISUAL CORE COMPLETE
                                        
F4 (Atmospheric Depth) ─────────────────┐
        │                               │
        └──> F6 (Color Grading) ────────┴──> ATMOSPHERE COMPLETE

F5 (Stronger AO) ───────────────────────┐
        │                               │
F7 (Light Bloom) ───────────────────────┴──> LIGHTING COMPLETE

F8 (Terrain Variation) ─────────────────┐
        │                               │
F9 (Procedural Props) ──────────────────┼──> CONTENT COMPLETE
        │                               │
F10 (Particles) ────────────────────────┘
```

---

## Quick Wins (Can implement immediately)

1. **Increase cloud opacity** - Single line change in `sky_dome.wgsl`
2. **Boost GTAO intensity** - Config change in `gtao.rs`
3. **Increase bloom intensity** - Config change in `bloom.rs`
4. **Add fog moon tinting** - Small shader modification
5. **Increase point light radius** - Already configurable

---

## Risk Assessment

| Phase | Risk | Mitigation |
|-------|------|------------|
| F2 | Performance impact from volumetric clouds | Use 2D approximation first |
| F9 | MJ model complexity | Start with simple dead tree |
| F10 | GPU particle performance | Limit particle count, use LOD |

---

## Success Metrics

- [ ] Moons have visible surface detail
- [ ] Clouds appear volumetric with moon tinting
- [ ] Strong atmospheric depth (can't see to horizon edge)
- [ ] AO visibly darkens corners and contact points
- [ ] Overall image has cinematic, moody feel
- [ ] Point lights have dramatic glow
- [ ] Terrain has color variation

---

## References

- Original: `screenshots/references/screenshot_voxel_moons.jpg`
- Target: `screenshots/references/screenshot_ai_enhanced_moons.jpeg`
- Existing visual plan: `docs/plans/visual_fidelity_improvements.md`
- SEUS techniques: `docs/plans/seus_sky_techniques.md`
- Moon lighting: `docs/plans/moon_environment_lighting.md`
