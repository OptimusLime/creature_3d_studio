# Visual Fidelity Roadmap: 80s Dark Fantasy Aesthetic

## Summary

Transform the current voxel renderer into an **80s Dark Fantasy** aesthetic—characterized by melancholy, grit, practical effects, and harsh contrasts. We use modern rendering techniques to emulate the limitations and artistic choices of 1980s practical filmmaking: heavy fog machines, wet surfaces, neon accents against deep shadow, and film grain that softens digital edges.

## Target Aesthetic

The goal is NOT photorealism. The goal is **cinematic atmosphere**:
- Deep, crushed blacks that hide detail and create mystery
- Wet, shiny surfaces that catch light and feel physical
- Colored light bleeding from emissive sources
- Thick atmospheric haze like stage smoke
- Film grain that "dirties" the clean digital render

## Context

**Original State:** `screenshots/references/screenshot_voxel_moons.jpg`
- Flat terrain with uniform coloring
- Simple dual moon discs
- Thin, barely visible clouds
- No atmospheric depth
- Basic lighting without color bleeding
- Minimal ambient occlusion
- Clean, digital appearance

**Target State:** `screenshots/references/screenshot_ai_enhanced_moons.jpeg`
- Crushed blacks with selective visibility
- Wet specularity on all surfaces
- Purple ambient wash from dominant moon
- Orange light bleeding from lanterns
- Thick volumetric haze
- Film grain softening voxel edges
- Rim-lit silhouettes against dark backgrounds

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

## Master Technique Table (Sorted by Impact)

| Technique | Description | Impact | Difficulty | Perf Impact | Compute Cost |
|-----------|-------------|--------|------------|-------------|--------------|
| **Crushed Black LUT** | Aggressive shadow compression to near-pure black via post-process color grading. Hides detail, creates mystery. | CRITICAL | Easy | Negligible | 1 texture sample/pixel |
| **Wet PBR Specularity** | Add specular highlights to all surfaces simulating perpetual dampness. Makes voxels feel like wet stone. | CRITICAL | Medium | Low | ~8 ALU ops/pixel in lighting |
| **Colored Ambient from Moons** | Ambient light takes color from dominant moon (purple/orange), not neutral gray. Entire scene is tinted. | CRITICAL | Easy | Negligible | 3 uniforms, simple blend |
| **Emissive Color Bleeding** | Point lights cast colored light onto surrounding surfaces, not just white bloom. Warm/cool contrast. | HIGH | Medium | Low | Per-light color in existing loop |
| **Height-Varying Volumetric Fog** | Fog thicker near ground, thins upward. Color-tinted by moon. Like stage smoke machines. | HIGH | Medium | Low-Medium | exp() per pixel + height sample |
| **Film Grain Post-Process** | Overlay animated noise to "dirty" the clean digital image. Softens voxel edges perceptually. | HIGH | Easy | Negligible | 1 noise sample/pixel |
| **Soft Atmospheric Glow** | Bloom with larger kernel, softer falloff. Lights have diffused halos like dirty lens/smoky air. | HIGH | Easy | Low | Existing bloom, wider kernel |
| **Rim Lighting Enhancement** | Detect and brighten edges of objects facing away from camera. Silhouettes pop against dark BG. | HIGH | Medium | Low | Fresnel term in lighting |
| **Chromatic Depth Separation** | Foreground warmer, background cooler via depth-based color grading. Enhances depth perception. | MEDIUM | Easy | Negligible | Depth sample + lerp |
| **Moon Surface Textures** | Sample existing MJ-generated moon textures instead of flat color discs. Add limb darkening. | MEDIUM | Easy | Negligible | 1 texture sample/moon |
| **Volumetric Cloud Density** | Increase cloud opacity, add multi-layer rendering, edge scattering when backlit by moons. | MEDIUM | Medium | Low | Existing cloud pass, density tweak |
| **Moon-Tinted Clouds** | Clouds near moon take on purple/orange tint. Subsurface scattering approximation. | MEDIUM | Easy | Negligible | Dot product + color blend |
| **Stronger GTAO** | Increase AO intensity, adjust radius for smaller details. More pronounced corner darkening. | MEDIUM | Easy | None | Config change only |
| **Vignette** | Darken screen corners to focus attention center-frame. Classic cinematic technique. | LOW | Easy | Negligible | Distance from center calc |
| **Ground Debris/Particles** | Small-scale particles (leaves, dust, embers) scattered on ground and floating in air. | LOW | Hard | Medium | GPU particle system |
| **Terrain Color Variation** | Noise-based color variation: moss patches, stone areas, leaf litter. Break up uniform ground. | LOW | Medium | None | CPU terrain gen change |
| **Procedural Props** | MJ-generated dead trees, ruins, stone walls scattered across terrain. | LOW | Hard | Low | One-time mesh gen |
| **Crepuscular Rays (God Rays)** | Visible light shafts from moons through gaps in geometry, interacting with fog. | LOW | Hard | Medium-High | Raymarching or radial blur |
| **Depth of Field** | Subtle background blur to separate focal plane. | LOW | Medium | Medium | Gather blur based on CoC |

---

## Implementation Phases (Ordered by Impact & Complexity)

Phases are ordered to:
1. Start with highest-impact, lowest-difficulty techniques
2. Build complexity incrementally
3. Establish visual test harness early for verification
4. Group related techniques to minimize context switching

---

### Phase F0: Visual Regression Test Harness
**Impact:** Foundation  
**Difficulty:** Easy  
**Outcome:** Automated screenshot capture from fixed camera positions for A/B comparison

Before implementing any visual changes, we need reproducible verification.

**Tasks:**
1. Create `examples/p35_visual_regression.rs`:
   - Fixed camera positions (no player input)
   - Deterministic scene (fixed seed, fixed moon positions)
   - Captures 5 screenshots: sky_up, horizon, structure_close, structure_far, ground_detail
   - Outputs to `screenshots/regression/` with timestamp
2. Create baseline screenshots before any changes

**Verification:**
```bash
cargo run --example p35_visual_regression
ls screenshots/regression/
# Should contain: baseline_sky_up.png, baseline_horizon.png, etc.
```

**Files:**
- `examples/p35_visual_regression.rs` (new)

---

### Phase F1: Crushed Black Color Grading (LUT)
**Impact:** CRITICAL  
**Difficulty:** Easy  
**Perf Cost:** 1 texture sample/pixel (negligible)  
**Outcome:** Shadows compressed to near-pure black, creating mystery and hiding detail

This is the single highest-impact change. Without deep blacks, nothing else will feel "80s dark fantasy."

**Tasks:**
1. Add color grading pass to `bloom_composite.wgsl` (after tonemapping, before output)
2. Implement procedural "crush blacks" curve:
   ```wgsl
   // Crush blacks: remap 0.0-0.15 to 0.0-0.02
   let crushed = smoothstep(0.0, 0.15, color) * 0.98 + 0.02 * step(0.15, color);
   ```
3. Add `ColorGradingConfig` resource with `black_crush_strength: f32`
4. Expose in example via keyboard (B key to toggle/adjust)

**Verification:**
```bash
cargo run --example p35_visual_regression
# Compare: shadows should be MUCH darker, near-black in unlit areas
# Light areas should retain detail
```

**Files:**
- `assets/shaders/bloom_composite.wgsl` - Add color grading
- `crates/studio_core/src/deferred/bloom.rs` - Add `ColorGradingConfig`

---

### Phase F2: Colored Ambient from Moons
**Impact:** CRITICAL  
**Difficulty:** Easy  
**Perf Cost:** 3 uniforms + simple blend (negligible)  
**Outcome:** Entire scene tinted by dominant moon color (purple/orange), not neutral gray

**Tasks:**
1. In `deferred_lighting.wgsl`, replace hardcoded ambient with moon-derived ambient:
   ```wgsl
   let moon1_contrib = max(0.0, moon1_dir.y + 0.1);
   let moon2_contrib = max(0.0, moon2_dir.y + 0.1);
   let total = moon1_contrib + moon2_contrib + 0.001;
   let ambient_color = mix(moon2_color, moon1_color, moon1_contrib / total) * 0.08;
   ```
2. Pass moon colors and directions as uniforms (may already exist in shadow uniforms)
3. Remove `DARK_AMBIENT_COLOR` constant

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Press T to move purple moon high - scene should have purple tint
# Press Y to move orange moon high - scene should have orange tint
```

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Dynamic ambient calculation
- `crates/studio_core/src/deferred/lighting_node.rs` - Ensure uniforms passed

---

### Phase F3: Film Grain Post-Process
**Impact:** HIGH  
**Difficulty:** Easy  
**Perf Cost:** 1 noise sample/pixel (negligible)  
**Outcome:** Animated noise overlay that "dirties" clean digital render, softens voxel edges

**Tasks:**
1. Add film grain to `bloom_composite.wgsl`:
   ```wgsl
   let grain = (fract(sin(dot(uv + time * 0.1, vec2(12.9898, 78.233))) * 43758.5453) - 0.5) * grain_strength;
   color += grain;
   ```
2. Add `grain_strength: f32` to `ColorGradingConfig` (default 0.03-0.05)
3. Pass `time` uniform for animation

**Verification:**
```bash
cargo run --example p35_visual_regression
# Zoom in on screenshot - should see visible noise pattern
# Should NOT look like clean CG render
```

**Files:**
- `assets/shaders/bloom_composite.wgsl` - Add grain calculation
- `crates/studio_core/src/deferred/bloom.rs` - Add grain config

---

### Phase F4: Soft Atmospheric Glow (Bloom Tuning)
**Impact:** HIGH  
**Difficulty:** Easy  
**Perf Cost:** Existing bloom, minimal change  
**Outcome:** Lights have soft, diffused halos like dirty lens/smoky air

**Tasks:**
1. Adjust bloom parameters in `bloom.rs`:
   - Increase `radius` for softer spread
   - Lower `threshold` to catch more mid-tones
   - Increase `intensity` slightly
2. Consider adding second bloom pass with much larger radius for "atmospheric glow"

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Light posts should have large, soft glow halos (not sharp bloom)
# Moons should have atmospheric glow ring
```

**Files:**
- `crates/studio_core/src/deferred/bloom.rs` - Tune parameters

---

### Phase F5: Wet PBR Specularity
**Impact:** CRITICAL  
**Difficulty:** Medium  
**Perf Cost:** ~8 ALU ops/pixel in lighting pass (low)  
**Outcome:** All surfaces have specular highlights simulating perpetual dampness

This requires adding specular to the lighting model. Currently we have diffuse-only (N·L).

**Tasks:**
1. Add Blinn-Phong or GGX specular to `deferred_lighting.wgsl`:
   ```wgsl
   let H = normalize(L + V);
   let NdotH = max(dot(N, H), 0.0);
   let specular = pow(NdotH, shininess) * specular_strength;
   ```
2. Add `wetness` uniform or derive from material (all voxels get baseline wetness)
3. Specular color should be white/light gray (not material color)
4. Consider roughness variation based on block type (future)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Stone surfaces should have visible specular highlights
# Rotating camera should show specular moving across surfaces
# Ground should glisten when lit by moons
```

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Add specular calculation
- `crates/studio_core/src/deferred/lighting.rs` - Add wetness config

---

### Phase F6: Emissive Color Bleeding
**Impact:** HIGH  
**Difficulty:** Medium  
**Perf Cost:** Per-light color already in loop (minimal)  
**Outcome:** Point lights cast colored light onto surrounding surfaces

**Tasks:**
1. Verify point lights already have color in `DeferredPointLight`
2. Ensure lighting shader uses light color for diffuse contribution (not just white)
3. Increase point light intensity for more visible color bleeding
4. Add subtle "warm" bias to orange lights, "cool" bias to purple lights

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Ground near orange lanterns should be visibly orange
# Purple crystal lights should cast purple light on pillars
```

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Verify colored light contribution
- `examples/p34_sky_terrain_test.rs` - Ensure light colors are saturated

---

### Phase F7: Height-Varying Volumetric Fog
**Impact:** HIGH  
**Difficulty:** Medium  
**Perf Cost:** exp() per pixel + height sample (low-medium)  
**Outcome:** Fog thicker near ground, thins upward, tinted by moon color

Partially implemented. Needs enhancement.

**Tasks:**
1. Enhance fog in `deferred_lighting.wgsl`:
   ```wgsl
   let height_fog = exp(-max(0.0, world_pos.y - fog_base) * height_fog_density);
   let distance_fog = exp(-distance * distance_fog_density);
   let fog = max(height_fog * 0.7, distance_fog);
   let fog_color = mix(moon2_color, moon1_color, moon_blend) * fog_brightness;
   ```
2. Tune parameters for "stage smoke" effect (thick at ground, clears above head height)
3. Moon-tint the fog color (not neutral gray)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Base of structures should be partially obscured by ground mist
# Distant terrain should fade into colored haze (not gray)
```

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Enhanced fog calculation
- `crates/studio_core/src/deferred/lighting.rs` - Fog tint config

---

### Phase F8: Rim Lighting Enhancement
**Impact:** HIGH  
**Difficulty:** Medium  
**Perf Cost:** Fresnel term in lighting (low)  
**Outcome:** Object edges facing away from camera are brightened, silhouettes pop

**Tasks:**
1. Add Fresnel-based rim lighting to `deferred_lighting.wgsl`:
   ```wgsl
   let VdotN = dot(view_dir, normal);
   let rim = pow(1.0 - abs(VdotN), rim_power) * rim_strength;
   let rim_color = dominant_moon_color * rim;
   total_light += rim_color;
   ```
2. Rim color should come from dominant (highest) moon
3. Tune `rim_power` (2-4) and `rim_strength` (0.1-0.3)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Dead trees should have bright edge against dark sky
# Voxel stair-steps should be visibly outlined
```

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Add rim lighting
- `crates/studio_core/src/deferred/lighting.rs` - Rim config

---

### Phase F9: Moon Surface Textures
**Impact:** MEDIUM  
**Difficulty:** Easy  
**Perf Cost:** 1 texture sample/moon (negligible)  
**Outcome:** Moons display visible surface detail (craters, texture)

**Tasks:**
1. Bind existing moon textures in `sky_dome_node.rs`:
   - `assets/textures/generated/mj_moon_purple.png`
   - `assets/textures/generated/mj_moon_orange.png`
2. Sample textures in `sky_dome.wgsl` moon rendering
3. Add limb darkening (edges darker than center)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Moons should show crater/texture detail, not flat color discs
```

**Files:**
- `assets/shaders/sky_dome.wgsl` - Add texture sampling
- `crates/studio_core/src/deferred/sky_dome_node.rs` - Bind textures

---

### Phase F10: Chromatic Depth Separation
**Impact:** MEDIUM  
**Difficulty:** Easy  
**Perf Cost:** Depth sample + lerp (negligible)  
**Outcome:** Foreground warmer, background cooler, enhances depth perception

**Tasks:**
1. In `bloom_composite.wgsl`, add depth-based color shift:
   ```wgsl
   let depth = texture_sample(depth_texture, uv).r;
   let normalized_depth = saturate(depth / far_plane);
   // Warm foreground, cool background
   color.r += (1.0 - normalized_depth) * 0.02;
   color.b += normalized_depth * 0.03;
   ```
2. Tune shift amounts for subtle effect

**Verification:**
```bash
cargo run --example p35_visual_regression
# Close objects should have subtle warm tint
# Distant objects should have subtle cool tint
```

**Files:**
- `assets/shaders/bloom_composite.wgsl` - Depth-based color shift

---

### Phase F11: Volumetric Cloud Density & Moon Tinting
**Impact:** MEDIUM  
**Difficulty:** Medium  
**Perf Cost:** Existing cloud pass, density tweak (low)  
**Outcome:** Clouds appear thick and puffy, tinted by nearby moons

**Tasks:**
1. Increase cloud opacity in `sky_dome.wgsl`
2. Add moon proximity tinting:
   ```wgsl
   let moon1_proximity = max(0.0, dot(ray_dir, moon1_dir));
   let moon2_proximity = max(0.0, dot(ray_dir, moon2_dir));
   cloud_color = mix(cloud_color, moon1_color, moon1_proximity * 0.3);
   cloud_color = mix(cloud_color, moon2_color, moon2_proximity * 0.3);
   ```
3. Consider two-layer clouds (high wispy, low puffy)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Clouds should be visibly thicker (not thin wisps)
# Clouds near purple moon should have purple tint
```

**Files:**
- `assets/shaders/sky_dome.wgsl` - Cloud density and tinting

---

### Phase F12: Stronger GTAO
**Impact:** MEDIUM  
**Difficulty:** Easy (config change only)  
**Perf Cost:** None (already running)  
**Outcome:** Ambient occlusion more pronounced in corners

**Tasks:**
1. In `gtao.rs`, adjust default config:
   - Increase `intensity` (try 1.5-2.0x current)
   - Adjust `radius` for tighter corner detection
2. Test with various quality presets

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Corners of structures should show clear darkening
# Contact between objects and ground should be darker
```

**Files:**
- `crates/studio_core/src/deferred/gtao.rs` - Config tweaks

---

### Phase F13: Vignette
**Impact:** LOW  
**Difficulty:** Easy  
**Perf Cost:** Negligible  
**Outcome:** Screen corners darkened, focus draws to center

**Tasks:**
1. Add vignette to `bloom_composite.wgsl`:
   ```wgsl
   let vignette = 1.0 - smoothstep(vignette_inner, vignette_outer, length(uv - 0.5));
   color *= vignette;
   ```
2. Add `vignette_strength: f32` to `ColorGradingConfig`

**Verification:**
```bash
cargo run --example p35_visual_regression
# Screen corners should be noticeably darker than center
```

**Files:**
- `assets/shaders/bloom_composite.wgsl` - Vignette calculation

---

### Future Phases (Lower Priority)

These require significant new systems or have higher performance cost:

#### Phase F14: Terrain Color Variation
- Noise-based color variation in terrain generation
- Moss patches, stone areas, leaf litter

#### Phase F15: Ground Debris/Particles
- GPU particle system for floating dust, leaves, embers
- Medium performance impact

#### Phase F16: Procedural Props
- MJ-generated dead trees, ruins
- One-time generation cost

#### Phase F17: Crepuscular Rays
- God rays from moons through geometry
- Requires raymarching or radial blur (higher cost)

#### Phase F18: Depth of Field
- Subtle background blur
- Gather blur based on circle of confusion

---

## Dependency Graph

```
F0 (Test Harness) ──────────────────────────────────────────────────────┐
        │                                                               │
        v                                                               │
F1 (Crushed Blacks) ────┐                                               │
        │               │                                               │
F2 (Moon Ambient) ──────┼──> FOUNDATION COMPLETE (Mood established)     │
        │               │                                               │
F3 (Film Grain) ────────┘                                               │
        │                                                               │
        v                                                               │
F4 (Soft Glow) ─────────┐                                               │
        │               │                                               │
F5 (Wet Specularity) ───┼──> MATERIALITY COMPLETE (Surfaces feel real)  │
        │               │                                               │
F6 (Color Bleeding) ────┘                                               │
        │                                                               │
        v                                                               │
F7 (Volumetric Fog) ────┐                                               │
        │               │                                               │
F8 (Rim Lighting) ──────┼──> ATMOSPHERE COMPLETE (Depth & separation)   │
        │               │                                               │
F9 (Moon Textures) ─────┘                                               │
        │                                                               │
        v                                                               │
F10 (Chromatic Depth) ──┐                                               │
        │               │                                               │
F11 (Cloud Tinting) ────┼──> POLISH COMPLETE (Refined details)          │
        │               │                                               │
F12 (Stronger AO) ──────┤                                               │
        │               │                                               │
F13 (Vignette) ─────────┘                                               │
        │                                                               │
        v                                                               │
    80s DARK FANTASY CORE AESTHETIC ACHIEVED                            │
        │                                                               │
        v                                                               │
F14-F18 (Content: Terrain, Props, Particles, Rays, DoF) ────────────────┘
```

---

## Quick Wins (Implement in First Session)

These can all be done in a single coding session with immediate visible impact:

| Quick Win | Time Est. | Impact |
|-----------|-----------|--------|
| F1: Crushed blacks (shader math) | 30 min | CRITICAL |
| F2: Moon ambient color | 30 min | CRITICAL |
| F3: Film grain | 15 min | HIGH |
| F4: Bloom tuning (config only) | 10 min | HIGH |
| F12: GTAO intensity (config only) | 5 min | MEDIUM |

**Total: ~90 minutes for dramatic visual improvement**

---

## Risk Assessment

| Phase | Risk | Mitigation |
|-------|------|------------|
| F5 | Specular adds visual noise on voxel stairs | Tune shininess low, use Fresnel mask |
| F7 | Fog obscures too much detail | Add config for density, test interactively |
| F8 | Rim lighting looks artificial | Keep strength subtle (0.1-0.2) |
| F15 | Particles hurt performance | Limit count, use LOD, GPU compute |
| F17 | God rays expensive | Start with radial blur (cheaper than raymarching) |

---

## Success Metrics (80s Dark Fantasy Checklist)

### Foundation (Phases F0-F3)
- [ ] Shadows are near-pure black (crushed)
- [ ] Scene has visible purple or orange tint from dominant moon
- [ ] Visible film grain on close inspection
- [ ] Test harness captures reproducible screenshots

### Materiality (Phases F4-F6)
- [ ] Lights have soft, diffused halos
- [ ] Surfaces have visible specular highlights ("wet" look)
- [ ] Colored light from lanterns visible on surrounding geometry

### Atmosphere (Phases F7-F9)
- [ ] Ground-level fog partially obscures structure bases
- [ ] Silhouettes have bright rim against dark backgrounds
- [ ] Moons show texture detail, not flat discs

### Polish (Phases F10-F13)
- [ ] Foreground feels warmer, background cooler
- [ ] Clouds tinted by nearby moons
- [ ] Corners and contacts show darker AO
- [ ] Screen corners subtly vignetted

### Final Impression
- [ ] Screenshot looks "filmed" not "rendered"
- [ ] Mood is melancholy, mysterious, atmospheric
- [ ] Voxels feel like practical miniatures, not digital cubes

---

## Performance Budget

Target: Maintain 60 FPS on mid-range GPU (e.g., RTX 3060, RX 6700)

| Phase | Added Cost | Running Total |
|-------|------------|---------------|
| Baseline | - | ~8ms |
| F1-F3 (Post-process) | +0.2ms | ~8.2ms |
| F4 (Bloom tuning) | +0.1ms | ~8.3ms |
| F5 (Specular) | +0.3ms | ~8.6ms |
| F6 (Color bleeding) | +0.0ms | ~8.6ms |
| F7 (Fog enhance) | +0.2ms | ~8.8ms |
| F8 (Rim lighting) | +0.2ms | ~9.0ms |
| F9-F13 (Various) | +0.3ms | ~9.3ms |
| **Headroom to 16.67ms** | | **~7ms** |

We have significant headroom. F14-F18 (particles, props, rays) will consume more.

---

## References

- Original: `screenshots/references/screenshot_voxel_moons.jpg`
- Target: `screenshots/references/screenshot_ai_enhanced_moons.jpeg`
- Existing visual plan: `docs/plans/visual_fidelity_improvements.md`
- SEUS techniques: `docs/plans/seus_sky_techniques.md`
- Moon lighting: `docs/plans/moon_environment_lighting.md`
- 80s Dark Fantasy techniques: See "Master Technique Table" above
