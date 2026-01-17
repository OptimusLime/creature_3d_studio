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

The main example is **`p34_sky_terrain_test.rs`**. ALL fidelity work builds on this example. We do NOT create new examples for visual features—we add toggleable features to p34.

---

## Verification Principles (Applies to ALL Phases)

### Single Example, Feature Toggles

Every visual feature is implemented as a **toggleable option** in `p34_sky_terrain_test`:
- Features controlled via `FidelityConfig` resource
- Each feature has `enabled: bool` and intensity/strength parameters
- CLI flags: `--enable-<feature>` and `--disable-<feature>`
- All features default OFF until explicitly enabled

### Screenshot Workflow

All verification uses the same workflow:

```bash
# 1. Capture baseline (all features OFF)
cargo run --example p34_sky_terrain_test --release -- --screenshot --output=screenshots/fidelity/baseline.png

# 2. Capture with ONE feature enabled
cargo run --example p34_sky_terrain_test --release -- --screenshot --enable-crushed-blacks --output=screenshots/fidelity/crushed_blacks.png

# 3. Compare baseline vs feature
# Visual diff OR side-by-side comparison
```

### Folder Structure

```
screenshots/fidelity/
├── baseline.png                    # All features OFF (current p34 state)
├── crushed_blacks.png              # baseline + crushed blacks
├── moon_ambient.png                # baseline + moon ambient
├── crushed_blacks+moon_ambient.png # baseline + both
├── ...
└── all_features.png                # Everything enabled
```

### Verification Criteria (Same for Every Phase)

For EVERY phase, verification means:
1. **Baseline captured**: `screenshots/fidelity/baseline.png` exists
2. **Feature screenshot captured**: `screenshots/fidelity/<feature>.png` exists  
3. **Visual difference observable**: Side-by-side shows clear change
4. **Feature isolated**: Disabling feature returns to baseline appearance
5. **No regression**: Other features still work when this one is enabled

### CLI Argument Pattern

```bash
cargo run --example p34_sky_terrain_test --release -- [OPTIONS]

Screenshot options:
  --screenshot              Capture screenshot and exit (no interactive mode)
  --output=<path>           Output path for screenshot (default: screenshots/fidelity/test.png)
  --frames=<n>              Wait N frames before capture (default: 120)

Feature toggles (all default OFF):
  --enable-crushed-blacks   Enable crushed black color grading
  --enable-moon-ambient     Enable colored ambient from moons
  --enable-film-grain       Enable film grain post-process
  --enable-wet-specular     Enable wet PBR specularity
  --enable-rim-lighting     Enable rim lighting
  --enable-volumetric-fog   Enable enhanced volumetric fog
  --enable-vignette         Enable vignette
  --enable-all              Enable all fidelity features

Feature parameters (override defaults):
  --crush-strength=<f32>    Black crush strength (default: 0.8)
  --grain-strength=<f32>    Film grain intensity (default: 0.04)
  --specular-strength=<f32> Wet specular intensity (default: 0.3)
  --rim-strength=<f32>      Rim lighting intensity (default: 0.15)
  --fog-density=<f32>       Fog density (default: 0.02)
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
1. Start with foundation (feature toggle system)
2. Add highest-impact, lowest-difficulty techniques first
3. Build complexity incrementally
4. Every phase uses the same verification workflow

---

### Phase F0: Feature Toggle System & Baseline Capture
**Impact:** Foundation (blocks all other phases)  
**Difficulty:** Easy  
**Outcome:** p34 supports CLI-driven feature toggles and automated screenshot capture

Before implementing any visual changes, we need the infrastructure to toggle features on/off and capture comparable screenshots.

**Tasks:**

1. **Create `FidelityConfig` resource** in `crates/studio_core/src/fidelity.rs`:
   ```rust
   #[derive(Resource, Default)]
   pub struct FidelityConfig {
       // Post-processing
       pub crushed_blacks: FeatureToggle,
       pub film_grain: FeatureToggle,
       pub vignette: FeatureToggle,
       pub chromatic_depth: FeatureToggle,
       
       // Lighting
       pub moon_ambient: FeatureToggle,
       pub wet_specular: FeatureToggle,
       pub rim_lighting: FeatureToggle,
       pub color_bleeding: FeatureToggle,
       
       // Atmosphere
       pub volumetric_fog: FeatureToggle,
       pub cloud_tinting: FeatureToggle,
   }
   
   #[derive(Default)]
   pub struct FeatureToggle {
       pub enabled: bool,
       pub strength: f32,
   }
   ```

2. **Add CLI argument parsing** to `examples/p34_sky_terrain_test.rs`:
   - Parse `--screenshot` flag for non-interactive capture mode
   - Parse `--output=<path>` for screenshot destination
   - Parse `--enable-<feature>` and `--disable-<feature>` flags
   - Parse `--<feature>-strength=<f32>` for intensity overrides
   - Populate `FidelityConfig` from CLI args

3. **Add screenshot mode** to p34:
   - When `--screenshot` is passed: disable player input, use fixed camera position
   - Wait `--frames=N` frames (default 120) for scene to settle
   - Capture screenshot to `--output` path
   - Exit automatically after capture

4. **Create `screenshots/fidelity/` directory structure**

5. **Capture baseline screenshot**:
   ```bash
   mkdir -p screenshots/fidelity
   cargo run --example p34_sky_terrain_test --release -- --screenshot --output=screenshots/fidelity/baseline.png
   ```

**Verification:**

| Check | Command | Expected Result |
|-------|---------|-----------------|
| CLI parses | `cargo run --example p34_sky_terrain_test -- --help` | Shows all feature flags |
| Screenshot mode works | `cargo run --example p34_sky_terrain_test --release -- --screenshot --output=test.png` | Creates `test.png`, exits automatically |
| Baseline exists | `ls screenshots/fidelity/baseline.png` | File exists, shows current p34 scene |
| FidelityConfig accessible | Check shader uniforms receive config values | Uniforms are 0.0 when features disabled |

**Files:**
- `crates/studio_core/src/fidelity.rs` (new) - FidelityConfig resource
- `crates/studio_core/src/lib.rs` - Export fidelity module
- `examples/p34_sky_terrain_test.rs` - Add CLI parsing and screenshot mode

---

### Phase F1: Crushed Black Color Grading
**Impact:** CRITICAL  
**Difficulty:** Easy  
**Perf Cost:** ~5 ALU ops/pixel (negligible)  
**Outcome:** Shadows compressed to near-pure black, creating mystery and hiding detail

This is the single highest-impact change. Without deep blacks, nothing else will feel "80s dark fantasy."

**Tasks:**

1. **Add crushed blacks uniform to bloom composite shader**:
   - File: `assets/shaders/bloom_composite.wgsl`
   - Add to uniforms struct: `crush_blacks_strength: f32`
   - Add after tonemapping, before final output:
     ```wgsl
     // Crush blacks: remap dark values to near-black
     let luminance = dot(color.rgb, vec3(0.299, 0.587, 0.114));
     let crush_factor = smoothstep(0.0, 0.15, luminance);
     color.rgb = mix(vec3(0.0), color.rgb, mix(1.0, crush_factor, crush_blacks_strength));
     ```

2. **Add uniform binding in bloom node**:
   - File: `crates/studio_core/src/deferred/bloom_node.rs`
   - Read `FidelityConfig.crushed_blacks.strength` from world
   - Pass to shader uniform (0.0 when disabled, strength value when enabled)

3. **Wire FidelityConfig to bloom pass**:
   - File: `crates/studio_core/src/deferred/bloom.rs`
   - Add `crush_blacks_strength: f32` to `BloomUniforms` struct
   - Extract from `FidelityConfig` in prepare system

4. **Capture comparison screenshots**:
   ```bash
   # Feature OFF (should match baseline)
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/crushed_blacks_off.png
   
   # Feature ON
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-crushed-blacks --output=screenshots/fidelity/crushed_blacks_on.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| Shader compiles | `cargo build --example p34_sky_terrain_test` | No shader errors |
| OFF matches baseline | Diff `crushed_blacks_off.png` vs `baseline.png` | Identical or near-identical |
| ON shows effect | Compare `crushed_blacks_on.png` vs `baseline.png` | Shadows visibly darker/blacker |
| Lights preserved | Inspect `crushed_blacks_on.png` | Bright areas (moons, lanterns) retain detail |
| Toggle works | Run interactively, press key to toggle | Visual change immediate |

**Files:**
- `assets/shaders/bloom_composite.wgsl` - Add crush blacks math (lines ~50-60)
- `crates/studio_core/src/deferred/bloom_node.rs` - Add uniform extraction
- `crates/studio_core/src/deferred/bloom.rs` - Add to BloomUniforms struct

---

### Phase F2: Colored Ambient from Moons
**Impact:** CRITICAL  
**Difficulty:** Easy  
**Perf Cost:** ~10 ALU ops/pixel (negligible)  
**Outcome:** Entire scene tinted by dominant moon color (purple/orange), not neutral gray

**Tasks:**

1. **Add moon ambient toggle to lighting shader**:
   - File: `assets/shaders/deferred_lighting.wgsl`
   - Add uniform: `moon_ambient_enabled: f32` (0.0 or 1.0)
   - Add uniform: `moon_ambient_strength: f32`
   - Modify ambient calculation (around line 580):
     ```wgsl
     // Old: let ambient = DARK_AMBIENT_COLOR;
     // New:
     let moon1_contrib = max(0.0, -shadow_uniforms.moon1_direction.y + 0.1);
     let moon2_contrib = max(0.0, -shadow_uniforms.moon2_direction.y + 0.1);
     let total = moon1_contrib + moon2_contrib + 0.001;
     let moon_ambient = mix(
         shadow_uniforms.moon2_color_intensity.rgb,
         shadow_uniforms.moon1_color_intensity.rgb,
         moon1_contrib / total
     ) * moon_ambient_strength;
     let ambient = mix(DARK_AMBIENT_COLOR, moon_ambient, moon_ambient_enabled);
     ```

2. **Add uniforms to lighting node**:
   - File: `crates/studio_core/src/deferred/lighting_node.rs`
   - Read `FidelityConfig.moon_ambient.enabled` and `.strength`
   - Add to existing uniform buffer

3. **Capture comparison screenshots**:
   ```bash
   # Feature OFF
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/moon_ambient_off.png
   
   # Feature ON with purple moon high
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-moon-ambient --moon1-time=0.25 \
     --output=screenshots/fidelity/moon_ambient_purple.png
   
   # Feature ON with orange moon high  
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-moon-ambient --moon2-time=0.25 \
     --output=screenshots/fidelity/moon_ambient_orange.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| Shader compiles | `cargo build` | No errors |
| OFF matches baseline | Diff `moon_ambient_off.png` vs `baseline.png` | Identical |
| Purple tint visible | Inspect `moon_ambient_purple.png` | Scene has purple cast |
| Orange tint visible | Inspect `moon_ambient_orange.png` | Scene has orange cast |
| Moon position affects tint | Compare purple vs orange screenshots | Different tints based on which moon is higher |

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Add moon ambient calculation (~line 580)
- `crates/studio_core/src/deferred/lighting_node.rs` - Add uniforms

---

### Phase F3: Film Grain Post-Process
**Impact:** HIGH  
**Difficulty:** Easy  
**Perf Cost:** ~8 ALU ops/pixel (negligible)  
**Outcome:** Animated noise overlay that "dirties" clean digital render, softens voxel edges

**Tasks:**

1. **Add film grain to bloom composite shader**:
   - File: `assets/shaders/bloom_composite.wgsl`
   - Add uniforms: `film_grain_strength: f32`, `time: f32`
   - Add after crush blacks, before output:
     ```wgsl
     // Film grain - animated noise
     let grain_uv = uv * vec2(1920.0, 1080.0); // Scale to pixel coords
     let grain = fract(sin(dot(grain_uv + time * 100.0, vec2(12.9898, 78.233))) * 43758.5453);
     let grain_value = (grain - 0.5) * film_grain_strength;
     color.rgb += vec3(grain_value);
     ```

2. **Add time uniform to bloom node**:
   - File: `crates/studio_core/src/deferred/bloom_node.rs`
   - Extract `Time` resource, pass `time.elapsed_secs()` to shader
   - Add `film_grain_strength` from `FidelityConfig`

3. **Capture comparison screenshots**:
   ```bash
   # Feature OFF
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/film_grain_off.png
   
   # Feature ON (default strength)
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-film-grain --output=screenshots/fidelity/film_grain_on.png
   
   # Feature ON (strong, for visibility)
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-film-grain --grain-strength=0.1 \
     --output=screenshots/fidelity/film_grain_strong.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| Shader compiles | `cargo build` | No errors |
| OFF matches baseline | Diff `film_grain_off.png` vs `baseline.png` | Identical |
| Grain visible at 100% zoom | Open `film_grain_strong.png`, zoom to 100% | Visible noise pattern |
| Grain animates | Run interactively with grain enabled | Grain flickers/moves |
| Strength adjustable | Compare default vs strong screenshots | Strong has more visible noise |

**Files:**
- `assets/shaders/bloom_composite.wgsl` - Add grain calculation
- `crates/studio_core/src/deferred/bloom_node.rs` - Add time and strength uniforms

---

### Phase F4: Soft Atmospheric Glow (Bloom Tuning)
**Impact:** HIGH  
**Difficulty:** Easy  
**Perf Cost:** None (parameter change only)  
**Outcome:** Lights have soft, diffused halos like dirty lens/smoky air

**Tasks:**

1. **Add bloom preset to FidelityConfig**:
   - File: `crates/studio_core/src/fidelity.rs`
   - Add `soft_glow: FeatureToggle` with parameters:
     - `radius_multiplier: f32` (default 2.0 when enabled)
     - `threshold_reduction: f32` (default 0.3 when enabled)
     - `intensity_boost: f32` (default 1.5 when enabled)

2. **Apply bloom overrides in bloom node**:
   - File: `crates/studio_core/src/deferred/bloom_node.rs`
   - When `soft_glow.enabled`:
     - Multiply base radius by `radius_multiplier`
     - Subtract `threshold_reduction` from threshold
     - Multiply intensity by `intensity_boost`

3. **Capture comparison screenshots**:
   ```bash
   # Feature OFF (current bloom)
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/soft_glow_off.png
   
   # Feature ON
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-soft-glow --output=screenshots/fidelity/soft_glow_on.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| OFF matches baseline | Diff `soft_glow_off.png` vs `baseline.png` | Identical |
| Glow radius larger | Compare lantern halos in both screenshots | ON has larger, softer halos |
| Moons have atmospheric ring | Inspect moon edges in `soft_glow_on.png` | Soft glow extends beyond moon disc |
| Mid-tones bloom | Look at moderately bright surfaces | Slight glow on bright (not just emissive) surfaces |

**Files:**
- `crates/studio_core/src/fidelity.rs` - Add soft_glow parameters
- `crates/studio_core/src/deferred/bloom_node.rs` - Apply parameter overrides

---

### Phase F5: Wet PBR Specularity
**Impact:** CRITICAL  
**Difficulty:** Easy-Medium  
**Perf Cost:** ~15 ALU ops/pixel in lighting pass (low)  
**Outcome:** All surfaces have specular highlights simulating perpetual dampness

**Architecture Note:** We use a fully custom deferred pipeline (not Bevy's PBR). The G-buffer already has world position and normals - we just need to add Blinn-Phong specular to the lighting pass. This is straightforward shader work, not a material system change.

*(Bevy's `ExtendedMaterial` extends `StandardMaterial` which uses a different pipeline. Integrating it would require major refactoring of our custom G-buffer/lighting architecture. Not recommended for this feature.)*

**Tasks:**

1. **Add specular uniforms to lighting shader**:
   - File: `assets/shaders/deferred_lighting.wgsl`
   - Add uniforms: `wet_specular_enabled: f32`, `wet_specular_strength: f32`, `wet_specular_shininess: f32`
   - Add specular calculation in moon lighting section (~line 590):
     ```wgsl
     // Wet specular (Blinn-Phong)
     if (wet_specular_enabled > 0.5) {
         let V = normalize(camera_position - world_pos);
         
         // Moon 1 specular
         let H1 = normalize(-moon1_dir + V);
         let spec1 = pow(max(dot(normal, H1), 0.0), wet_specular_shininess) * wet_specular_strength;
         total_light += moon1_color * spec1 * moon1_shadow;
         
         // Moon 2 specular
         let H2 = normalize(-moon2_dir + V);
         let spec2 = pow(max(dot(normal, H2), 0.0), wet_specular_shininess) * wet_specular_strength;
         total_light += moon2_color * spec2 * moon2_shadow;
     }
     ```

2. **Add camera position uniform**:
   - File: `crates/studio_core/src/deferred/lighting_node.rs`
   - Extract camera world position from `ExtractedView`
   - Pass as uniform (may already exist)

3. **Wire FidelityConfig to lighting**:
   - File: `crates/studio_core/src/deferred/lighting_node.rs`
   - Read `FidelityConfig.wet_specular` and pass to shader

4. **Capture comparison screenshots**:
   ```bash
   # Feature OFF
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/wet_specular_off.png
   
   # Feature ON
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-wet-specular --output=screenshots/fidelity/wet_specular_on.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| Shader compiles | `cargo build` | No errors |
| OFF matches baseline | Diff screenshots | Identical |
| Specular visible on ground | Inspect ground in `wet_specular_on.png` | Bright highlights where moons reflect |
| Specular moves with camera | Run interactively, move camera | Highlights shift position |
| Specular color matches moon | Purple moon creates purple highlights | Tinted specular, not white |

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Add specular calculation (~line 590)
- `crates/studio_core/src/deferred/lighting_node.rs` - Add uniforms and camera position

---

### Phase F6: Emissive Color Bleeding
**Impact:** HIGH  
**Difficulty:** Easy (verification + tuning, may already work)  
**Perf Cost:** None (existing point light loop)  
**Outcome:** Point lights cast colored light onto surrounding surfaces

**Tasks:**

1. **Verify existing point light color handling**:
   - File: `assets/shaders/deferred_lighting.wgsl`
   - Check point light loop uses `light.color` for diffuse contribution
   - If using white, change to: `contribution = light.color.rgb * attenuation * NdotL`

2. **Add color bleeding intensity multiplier**:
   - File: `crates/studio_core/src/fidelity.rs`
   - Add `color_bleeding: FeatureToggle` with `intensity_multiplier: f32`
   - When enabled, multiply point light intensity by this factor

3. **Ensure test scene has saturated light colors**:
   - File: `examples/p34_sky_terrain_test.rs`
   - Verify `light_purple` and `light_orange` voxels create lights with saturated colors
   - If colors are desaturated, increase saturation

4. **Capture comparison screenshots**:
   ```bash
   # Feature OFF (or current state)
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/color_bleeding_off.png
   
   # Feature ON (boosted intensity)
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-color-bleeding --output=screenshots/fidelity/color_bleeding_on.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| Point lights use color | Inspect shader code | Diffuse uses `light.color`, not white |
| Orange visible on ground | Look at ground near orange lanterns | Orange tint on nearby surfaces |
| Purple visible on pillars | Look at pillars near purple crystals | Purple tint on pillar faces |
| Intensity adjustable | Compare OFF vs ON screenshots | ON has more visible color spread |

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Verify/fix colored light contribution
- `crates/studio_core/src/fidelity.rs` - Add color_bleeding toggle
- `examples/p34_sky_terrain_test.rs` - Verify light color saturation

---

### Phase F7: Height-Varying Volumetric Fog
**Impact:** HIGH  
**Difficulty:** Medium  
**Perf Cost:** ~10 ALU ops/pixel (low)  
**Outcome:** Fog thicker near ground, thins upward, tinted by moon color

Partially implemented in existing code. This phase enhances it with moon tinting and better height falloff.

**Tasks:**

1. **Add enhanced fog uniforms**:
   - File: `assets/shaders/deferred_lighting.wgsl`
   - Add uniforms: `volumetric_fog_enabled: f32`, `height_fog_density: f32`, `fog_tint_strength: f32`
   - Modify fog calculation (~line 620):
     ```wgsl
     if (volumetric_fog_enabled > 0.5) {
         // Height fog - thick at ground, thins upward
         let height_above_ground = max(0.0, world_pos.y - fog_base);
         let height_fog = exp(-height_above_ground * height_fog_density);
         
         // Distance fog
         let dist = length(world_pos - camera_position);
         let distance_fog = 1.0 - exp(-dist * distance_fog_density);
         
         // Combine: height fog adds to distance fog
         let total_fog = max(height_fog * 0.6, distance_fog);
         
         // Moon-tinted fog color
         let moon_tint = mix(moon2_color, moon1_color, moon1_contrib / total_contrib);
         let tinted_fog_color = mix(fog_color, moon_tint * 0.5, fog_tint_strength);
         
         final_color = mix(final_color, tinted_fog_color, total_fog);
     }
     ```

2. **Add fog config to FidelityConfig**:
   - File: `crates/studio_core/src/fidelity.rs`
   - Add `volumetric_fog: FeatureToggle` with:
     - `height_density: f32` (default 0.05)
     - `tint_strength: f32` (default 0.6)

3. **Capture comparison screenshots**:
   ```bash
   # Feature OFF
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/volumetric_fog_off.png
   
   # Feature ON
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-volumetric-fog --output=screenshots/fidelity/volumetric_fog_on.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| OFF matches baseline | Diff screenshots | Identical (or original fog behavior) |
| Ground mist visible | Look at base of structures | Partially obscured by low fog |
| Fog thins with height | Compare ground vs elevated areas | Less fog higher up |
| Fog is tinted | Inspect fog color | Purple or orange tint (not gray) |
| Distant terrain fades | Look at horizon | Smooth fade into colored haze |

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Enhanced fog calculation (~line 620)
- `crates/studio_core/src/fidelity.rs` - Add volumetric_fog config
- `crates/studio_core/src/deferred/lighting_node.rs` - Pass new uniforms

---

### Phase F8: Rim Lighting Enhancement
**Impact:** HIGH  
**Difficulty:** Medium  
**Perf Cost:** ~8 ALU ops/pixel (low)  
**Outcome:** Object edges facing away from camera are brightened, silhouettes pop against dark backgrounds

**Tasks:**

1. **Add rim lighting to lighting shader**:
   - File: `assets/shaders/deferred_lighting.wgsl`
   - Add uniforms: `rim_lighting_enabled: f32`, `rim_strength: f32`, `rim_power: f32`
   - Add after main lighting, before fog (~line 610):
     ```wgsl
     if (rim_lighting_enabled > 0.5) {
         let V = normalize(camera_position - world_pos);
         let VdotN = dot(V, normal);
         let rim = pow(1.0 - abs(VdotN), rim_power) * rim_strength;
         
         // Rim color from dominant moon
         let dominant_moon_color = select(moon2_color, moon1_color, moon1_contrib > moon2_contrib);
         total_light += dominant_moon_color * rim;
     }
     ```

2. **Add rim config to FidelityConfig**:
   - File: `crates/studio_core/src/fidelity.rs`
   - Add `rim_lighting: FeatureToggle` with:
     - `strength: f32` (default 0.15)
     - `power: f32` (default 3.0)

3. **Capture comparison screenshots**:
   ```bash
   # Feature OFF
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/rim_lighting_off.png
   
   # Feature ON
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-rim-lighting --output=screenshots/fidelity/rim_lighting_on.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| OFF matches baseline | Diff screenshots | Identical |
| Silhouettes brightened | Look at tree/structure edges against sky | Visible bright edge |
| Voxel stair-steps outlined | Look at stepped edges on structures | Each step has rim highlight |
| Rim color matches moon | Check rim color | Purple or orange tint |
| Effect subtle, not glowing | Overall appearance | Enhancement, not halo effect |

**Files:**
- `assets/shaders/deferred_lighting.wgsl` - Add rim lighting (~line 610)
- `crates/studio_core/src/fidelity.rs` - Add rim_lighting config
- `crates/studio_core/src/deferred/lighting_node.rs` - Pass uniforms

---

### Phase F9: Moon Surface Textures
**Impact:** MEDIUM  
**Difficulty:** Easy  
**Perf Cost:** 2 texture samples (negligible)  
**Outcome:** Moons display visible surface detail (craters, texture)

**Tasks:**

1. **Bind moon textures in sky dome node**:
   - File: `crates/studio_core/src/deferred/sky_dome_node.rs`
   - Load `assets/textures/generated/mj_moon_purple.png`
   - Load `assets/textures/generated/mj_moon_orange.png`
   - Add to bind group as texture + sampler

2. **Sample textures in moon rendering**:
   - File: `assets/shaders/sky_dome.wgsl`
   - In moon rendering function, calculate UV from ray-moon intersection
   - Sample texture: `let moon_detail = textureSample(moon1_texture, moon_sampler, moon_uv).rgb`
   - Multiply base moon color by texture detail

3. **Add limb darkening**:
   - Calculate distance from moon center in UV space
   - Darken edges: `let limb = 1.0 - smoothstep(0.7, 1.0, uv_dist_from_center)`

4. **Capture comparison screenshots**:
   ```bash
   # This feature doesn't have a toggle - it's always an improvement
   # Capture before implementation (current)
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/moon_textures_before.png
   
   # Capture after implementation
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/moon_textures_after.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| Textures load | Check console for errors | No texture loading errors |
| Purple moon has detail | Zoom in on purple moon | Visible craters/texture |
| Orange moon has detail | Zoom in on orange moon | Visible craters/texture |
| Limb darkening visible | Look at moon edges | Edges slightly darker than center |

**Files:**
- `crates/studio_core/src/deferred/sky_dome_node.rs` - Load and bind textures
- `assets/shaders/sky_dome.wgsl` - Sample textures in moon rendering

---

### Phase F10: Chromatic Depth Separation
**Impact:** MEDIUM  
**Difficulty:** Easy  
**Perf Cost:** 1 depth sample + ~5 ALU ops (negligible)  
**Outcome:** Foreground warmer, background cooler, enhances depth perception

**Tasks:**

1. **Add chromatic depth uniforms**:
   - File: `assets/shaders/bloom_composite.wgsl`
   - Add uniforms: `chromatic_depth_enabled: f32`, `chromatic_depth_strength: f32`
   - Add depth texture binding (may need to pass from G-buffer)

2. **Implement depth-based color shift**:
   - File: `assets/shaders/bloom_composite.wgsl`
   - Add after other post-process effects:
     ```wgsl
     if (chromatic_depth_enabled > 0.5) {
         let depth = textureSample(depth_texture, depth_sampler, uv).r;
         let normalized_depth = saturate(depth / far_plane);
         // Warm foreground (add red), cool background (add blue)
         color.r += (1.0 - normalized_depth) * 0.015 * chromatic_depth_strength;
         color.b += normalized_depth * 0.02 * chromatic_depth_strength;
     }
     ```

3. **Add to FidelityConfig and wire uniforms**:
   - File: `crates/studio_core/src/fidelity.rs`
   - Add `chromatic_depth: FeatureToggle`

4. **Capture comparison screenshots**:
   ```bash
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/chromatic_depth_off.png
   
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-chromatic-depth --output=screenshots/fidelity/chromatic_depth_on.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| OFF matches baseline | Diff screenshots | Identical |
| Foreground warmer | Sample pixel colors near camera | Slightly more red |
| Background cooler | Sample pixel colors far from camera | Slightly more blue |
| Effect subtle | Overall appearance | Not obviously tinted, just enhanced depth |

**Files:**
- `assets/shaders/bloom_composite.wgsl` - Add chromatic depth shift
- `crates/studio_core/src/deferred/bloom_node.rs` - Bind depth texture, pass uniforms

---

### Phase F11: Volumetric Cloud Density & Moon Tinting
**Impact:** MEDIUM  
**Difficulty:** Medium  
**Perf Cost:** ~10 ALU ops in sky pass (low)  
**Outcome:** Clouds appear thick and puffy, tinted by nearby moons

**Tasks:**

1. **Add cloud density uniform**:
   - File: `assets/shaders/sky_dome.wgsl`
   - Add uniforms: `cloud_density_multiplier: f32`, `cloud_tint_strength: f32`

2. **Increase cloud opacity**:
   - Find cloud rendering section
   - Multiply final cloud alpha by `cloud_density_multiplier`

3. **Add moon proximity tinting**:
   - File: `assets/shaders/sky_dome.wgsl`
   - In cloud color calculation:
     ```wgsl
     let moon1_proximity = max(0.0, dot(ray_dir, moon1_dir));
     let moon2_proximity = max(0.0, dot(ray_dir, moon2_dir));
     let tinted_cloud = cloud_color;
     tinted_cloud = mix(tinted_cloud, moon1_color, moon1_proximity * 0.3 * cloud_tint_strength);
     tinted_cloud = mix(tinted_cloud, moon2_color, moon2_proximity * 0.3 * cloud_tint_strength);
     ```

4. **Add to FidelityConfig**:
   - File: `crates/studio_core/src/fidelity.rs`
   - Add `cloud_tinting: FeatureToggle` with `density: f32`, `tint_strength: f32`

5. **Capture comparison screenshots**:
   ```bash
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/cloud_tinting_off.png
   
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-cloud-tinting --output=screenshots/fidelity/cloud_tinting_on.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| OFF matches baseline | Diff screenshots | Identical |
| Clouds denser | Compare cloud visibility | ON has thicker, more opaque clouds |
| Purple tint near purple moon | Look at clouds near purple moon | Purple coloration |
| Orange tint near orange moon | Look at clouds near orange moon | Orange coloration |

**Files:**
- `assets/shaders/sky_dome.wgsl` - Cloud density and tinting
- `crates/studio_core/src/deferred/sky_dome_node.rs` - Pass uniforms

---

### Phase F12: Stronger GTAO
**Impact:** MEDIUM  
**Difficulty:** Easy (config change only)  
**Perf Cost:** None (already running)  
**Outcome:** Ambient occlusion more pronounced in corners

**Tasks:**

1. **Add GTAO intensity to FidelityConfig**:
   - File: `crates/studio_core/src/fidelity.rs`
   - Add `gtao_boost: FeatureToggle` with `intensity_multiplier: f32` (default 1.8)

2. **Apply multiplier in GTAO node**:
   - File: `crates/studio_core/src/deferred/gtao_node.rs`
   - When `gtao_boost.enabled`, multiply final AO intensity

3. **Capture comparison screenshots**:
   ```bash
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/gtao_boost_off.png
   
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-gtao-boost --output=screenshots/fidelity/gtao_boost_on.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| OFF matches baseline | Diff screenshots | Identical |
| Corners darker | Look at structure corners | Visibly darker shadowing |
| Ground contact darker | Look at base of posts/structures | Darker where objects meet ground |
| Not too dark | Overall appearance | Enhanced shadows, not black blobs |

**Files:**
- `crates/studio_core/src/fidelity.rs` - Add gtao_boost toggle
- `crates/studio_core/src/deferred/gtao_node.rs` - Apply intensity multiplier

---

### Phase F13: Vignette
**Impact:** LOW  
**Difficulty:** Easy  
**Perf Cost:** ~5 ALU ops (negligible)  
**Outcome:** Screen corners darkened, focus draws to center

**Tasks:**

1. **Add vignette uniforms**:
   - File: `assets/shaders/bloom_composite.wgsl`
   - Add uniforms: `vignette_enabled: f32`, `vignette_strength: f32`, `vignette_radius: f32`

2. **Implement vignette**:
   - File: `assets/shaders/bloom_composite.wgsl`
   - Add as final post-process step:
     ```wgsl
     if (vignette_enabled > 0.5) {
         let center_dist = length(uv - vec2(0.5));
         let vignette = 1.0 - smoothstep(vignette_radius * 0.5, vignette_radius, center_dist);
         color.rgb *= mix(1.0, vignette, vignette_strength);
     }
     ```

3. **Add to FidelityConfig**:
   - File: `crates/studio_core/src/fidelity.rs`
   - Add `vignette: FeatureToggle` with `strength: f32`, `radius: f32`

4. **Capture comparison screenshots**:
   ```bash
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --output=screenshots/fidelity/vignette_off.png
   
   cargo run --example p34_sky_terrain_test --release -- \
     --screenshot --enable-vignette --output=screenshots/fidelity/vignette_on.png
   ```

**Verification:**

| Check | How to Verify | Expected Result |
|-------|---------------|-----------------|
| OFF matches baseline | Diff screenshots | Identical |
| Corners darker | Sample pixel brightness at corners | Noticeably darker than center |
| Center unaffected | Sample pixel brightness at center | Same as baseline |
| Gradual falloff | Look at mid-screen | Smooth gradient, not hard edge |

**Files:**
- `assets/shaders/bloom_composite.wgsl` - Add vignette calculation
- `crates/studio_core/src/fidelity.rs` - Add vignette config

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
F0 (Feature Toggle System) ─────────────────────────────────────────────┐
        │                                                               │
        │  BLOCKS ALL OTHER PHASES - Must complete first                │
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

## Implementation Timeline

### Session 1: Foundation (Required First)

| Phase | Task | Time Est. |
|-------|------|-----------|
| F0 | Create `FidelityConfig` resource | 30 min |
| F0 | Add CLI argument parsing to p34 | 45 min |
| F0 | Add screenshot mode (--screenshot, --output) | 30 min |
| F0 | Capture baseline screenshot | 5 min |
| | **Session 1 Total** | **~2 hours** |

**Verification:** `cargo run --example p34_sky_terrain_test -- --screenshot --output=test.png` works

### Session 2: Foundation Effects (High Impact)

| Phase | Task | Time Est. |
|-------|------|-----------|
| F1 | Add crushed blacks to bloom_composite.wgsl | 30 min |
| F1 | Wire uniform from FidelityConfig | 15 min |
| F1 | Capture comparison screenshots | 10 min |
| F2 | Add moon ambient to deferred_lighting.wgsl | 30 min |
| F2 | Capture comparison screenshots | 10 min |
| F3 | Add film grain to bloom_composite.wgsl | 20 min |
| F3 | Capture comparison screenshots | 10 min |
| | **Session 2 Total** | **~2 hours** |

**Verification:** Three feature comparison screenshot pairs exist, each shows clear effect

### Session 3: Materiality Effects

| Phase | Task | Time Est. |
|-------|------|-----------|
| F4 | Add soft glow bloom overrides | 20 min |
| F5 | Add wet specular to lighting shader | 45 min |
| F6 | Verify/fix point light color bleeding | 30 min |
| | **Session 3 Total** | **~1.5 hours** |

### Session 4: Atmosphere Effects

| Phase | Task | Time Est. |
|-------|------|-----------|
| F7 | Enhance volumetric fog with moon tinting | 45 min |
| F8 | Add rim lighting | 30 min |
| F9 | Bind moon textures, add sampling | 45 min |
| | **Session 4 Total** | **~2 hours** |

### Session 5: Polish Effects

| Phase | Task | Time Est. |
|-------|------|-----------|
| F10 | Add chromatic depth separation | 30 min |
| F11 | Add cloud density and moon tinting | 30 min |
| F12 | Add GTAO boost multiplier | 15 min |
| F13 | Add vignette | 20 min |
| | **Session 5 Total** | **~1.5 hours** |

**Total Estimated Time: ~9 hours across 5 sessions**

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

## Success Metrics (Verifiable Checklist)

Every metric is verified by comparing screenshots with features ON vs OFF.

### Phase F0 Complete When:
- [ ] `cargo run --example p34_sky_terrain_test -- --help` shows all feature flags
- [ ] `--screenshot --output=X` creates file and exits automatically
- [ ] `screenshots/fidelity/baseline.png` exists
- [ ] All `--enable-<feature>` flags parse without error (even if features not implemented)

### Phase F1-F3 Complete When:
- [ ] `screenshots/fidelity/crushed_blacks_on.png` exists
- [ ] Diff vs baseline shows shadows are darker (not identical)
- [ ] `screenshots/fidelity/moon_ambient_purple.png` shows purple tint
- [ ] `screenshots/fidelity/moon_ambient_orange.png` shows orange tint
- [ ] `screenshots/fidelity/film_grain_strong.png` shows visible noise at 100% zoom

### Phase F4-F6 Complete When:
- [ ] `screenshots/fidelity/soft_glow_on.png` shows larger bloom halos
- [ ] `screenshots/fidelity/wet_specular_on.png` shows specular highlights on ground
- [ ] `screenshots/fidelity/color_bleeding_on.png` shows orange tint near lanterns

### Phase F7-F9 Complete When:
- [ ] `screenshots/fidelity/volumetric_fog_on.png` shows ground mist
- [ ] `screenshots/fidelity/rim_lighting_on.png` shows bright edges on silhouettes
- [ ] `screenshots/fidelity/moon_textures_after.png` shows crater detail on moons

### Phase F10-F13 Complete When:
- [ ] `screenshots/fidelity/chromatic_depth_on.png` differs from baseline (subtle)
- [ ] `screenshots/fidelity/cloud_tinting_on.png` shows tinted clouds
- [ ] `screenshots/fidelity/gtao_boost_on.png` shows darker corners
- [ ] `screenshots/fidelity/vignette_on.png` shows darker screen corners

### All Phases Complete When:
- [ ] `--enable-all` produces screenshot with ALL effects active
- [ ] `screenshots/fidelity/all_features.png` exists
- [ ] Visual comparison to `screenshots/references/screenshot_ai_enhanced_moons.jpeg` shows significant improvement toward target aesthetic
- [ ] Every feature can be toggled OFF independently and returns to baseline behavior

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
