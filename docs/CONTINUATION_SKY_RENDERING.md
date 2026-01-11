# Continuation Prompt: Dual-Moon Sky Rendering

## Repository
**Path:** `/Users/paul/coding/creatures/creature_3d_studio`
**Branch:** `feature/visual-fidelity-improvements`

---

## CRITICAL CONTEXT

**There is no sun. There is no daytime.**

This world has TWO MOONS that orbit independently:
- **Purple moon** - period 1.0, phase_offset 0.0, inclination 30°
- **Orange moon** - period 0.8, phase_offset 0.5, inclination 15°

Both moons are visible simultaneously at various points in the cycle. The sky is always night. Lighting comes from the moons.

**The current `sky_dome.wgsl` is WRONG** - it has sun-based code that must be deleted.

---

## MASTER PLAN

### Execution Order (Highest Impact First)

| Phase | Technique | What To Do | Reference |
|-------|-----------|------------|-----------|
| 1 | Dual-Moon Rendering | Replace `render_moon()` with Feral_Pug ray-sphere method | feralpug.github.io Part 2 |
| 2 | Moon-Lit Sky Gradient | Delete sun gradients, add moon-based sky tinting | feralpug.github.io Part 1 |
| 3 | ACES Tonemapping | Replace ad-hoc tonemap with ACES curve | Narkowicz |
| 4 | Star Twinkle | Add time-modulated brightness to stars | feralpug.github.io Part 2 |
| 5 | Fog/Horizon | Match fog to sky, apply horizon fade | feralpug.github.io + dynamic-skies |
| 6 | Two-Layer Clouds | Procedural clouds tinted by moon colors | feralpug.github.io Part 3 |

### What Gets Deleted

| File | Code to Delete | Why |
|------|----------------|-----|
| `sky_dome.wgsl` | `day_sky_gradient()` lines 203-225 | No daytime |
| `sky_dome.wgsl` | `twilight_gradient()` lines 183-200 | No twilight |
| `sky_dome.wgsl` | `render_sun()` lines 281-318 | No sun |
| `sky_dome.wgsl` | `compute_atmospheric_scattering()` | Sun-based, replace with moon-based |
| `sky_dome_node.rs` | `SunOrbit` struct | No sun |
| `sky_dome.rs` | `SunAppearance` struct | No sun |

### What Gets Replaced

| Current | Problem | Replacement |
|---------|---------|-------------|
| `render_moon()` | Disc doesn't render | Feral_Pug ray-sphere intersection |
| Tonemap lines 500-504 | Ad-hoc, washes colors | ACES filmic curve |
| Static stars | No animation | Time-modulated noise |

---

## PHASE 1: Dual-Moon Rendering

### Outcome
Two moon discs visible as 3D spheres with surface detail, moving through sky over time.

### Reference Algorithm
**Source:** Feral_Pug Procedural Skybox Tutorial Part 2
**URL:** https://feralpug.github.io/tutorial/2020-07-30-Part2-ProcSkybox/

**Reference Code (HLSL):**
```hlsl
float3 moonFragPos = normWorldPos * sphere + float3(0, 0, 0);
float3 moonFragNormal = normalize(moonFragPos - currentMoonPos);

if (sphere >= 0.0) {
    float3 moonTex = tex2D(_MoonTex, moonUV).rgb * _MoonColor.rgb;
}

// Moon is self-luminous (no sun in this world)
// Skip phase calculation - moons light themselves
```

### Implementation

**File:** `assets/shaders/sky_dome.wgsl`

Replace `render_moon()` entirely with:

```wgsl
fn render_moon_sphere(
    view_dir: vec3<f32>,
    moon_dir: vec3<f32>,
    moon_size: f32,
    moon_color: vec3<f32>,
    moon_intensity: f32,
) -> vec3<f32> {
    if moon_dir.y < -0.1 { return vec3<f32>(0.0); }
    
    // Ray-sphere intersection (Feral_Pug method)
    let sphere_dist = 100.0;
    let sphere_pos = moon_dir * sphere_dist;
    let sphere_radius = tan(moon_size) * sphere_dist;
    
    let oc = -sphere_pos;
    let b = dot(oc, view_dir);
    let c = dot(oc, oc) - sphere_radius * sphere_radius;
    let discriminant = b * b - c;
    
    var result = vec3<f32>(0.0);
    
    if discriminant >= 0.0 {
        // Hit - calculate surface normal
        let t = -b - sqrt(discriminant);
        let hit_pos = view_dir * t;
        let moon_normal = normalize(hit_pos - sphere_pos);
        
        // UV from normal for texture
        let uv = vec2(
            atan2(moon_normal.z, moon_normal.x) / (2.0 * PI) + 0.5,
            asin(clamp(moon_normal.y, -1.0, 1.0)) / PI + 0.5
        );
        
        // Surface detail from procedural noise
        let surface = 0.85 + fbm(uv * 8.0) * 0.15;
        
        // Limb darkening
        let limb = pow(max(0.0, dot(moon_normal, -view_dir)), 0.3);
        
        // Self-luminous moon (no sun phase)
        result = moon_color * surface * limb * moon_intensity * 2.0;
    }
    
    // Glow halo
    let angle = acos(clamp(dot(view_dir, moon_dir), -1.0, 1.0));
    let glow_radius = moon_size * 3.0;
    if angle < glow_radius {
        let glow_t = angle / glow_radius;
        let glow = exp(-glow_t * glow_t * 3.0) * 0.4 * moon_intensity;
        result += moon_color * glow;
    }
    
    // Horizon reddening
    if moon_dir.y < 0.25 {
        let reddening = 1.0 - moon_dir.y / 0.25;
        result *= mix(vec3(1.0), vec3(1.2, 0.85, 0.6), reddening * 0.4);
    }
    
    return result;
}
```

### Verification

```bash
cargo run --example p31_visual_fidelity_test
# Check: purple_moon.png - solid purple disc with surface detail
# Check: orange_moon.png - solid orange disc with surface detail
# Check: dual_moons_sky.png - BOTH moons visible simultaneously
```

**Trajectory Test:**
```bash
# Capture at multiple cycle times
# Moons should be at different positions in each frame
```

---

## PHASE 2: Moon-Lit Sky Gradient

### Outcome
Sky gradient responds to moon positions and colors, not sun.

### Reference Algorithm
**Source:** Feral_Pug Procedural Skybox Tutorial Part 1
**URL:** https://feralpug.github.io/tutorial/2020-07-30-Part1-ProcSkybox/

**Reference Code:**
```hlsl
float horizonValue = dot(normWorldPos, float3(0, 1, 0));
horizonValue = 1 - saturate(Remap(horizonValue, float2(_SkyFadeStart, _SkyFadeEnd), float2(0, 1)));
```

### Implementation

**File:** `assets/shaders/sky_dome.wgsl`

Delete `day_sky_gradient()`, `twilight_gradient()`, `compute_atmospheric_scattering()`.

Add:

```wgsl
fn compute_moon_lit_sky(
    view_dir: vec3<f32>,
    moon1_dir: vec3<f32>,
    moon1_color: vec3<f32>,
    moon1_intensity: f32,
    moon2_dir: vec3<f32>,
    moon2_color: vec3<f32>,
    moon2_intensity: f32,
) -> vec3<f32> {
    // Base night sky (always dark)
    let zenith = vec3<f32>(0.01, 0.01, 0.02);
    let horizon = vec3<f32>(0.02, 0.015, 0.03);
    
    // Vertical gradient
    let view_up = max(0.0, view_dir.y);
    var sky = mix(horizon, zenith, pow(view_up, 0.7));
    
    // Moon 1 glow contribution
    if moon1_dir.y > -0.1 {
        let moon1_angle = max(0.0, dot(view_dir, moon1_dir));
        let moon1_glow = pow(moon1_angle, 4.0) * moon1_intensity * 0.1;
        sky += moon1_color * moon1_glow;
    }
    
    // Moon 2 glow contribution
    if moon2_dir.y > -0.1 {
        let moon2_angle = max(0.0, dot(view_dir, moon2_dir));
        let moon2_glow = pow(moon2_angle, 4.0) * moon2_intensity * 0.1;
        sky += moon2_color * moon2_glow;
    }
    
    return sky;
}
```

### Verification

```bash
cargo run --example p31_visual_fidelity_test
# Check: sky has subtle color tint toward each moon
# When purple moon high, sky has slight purple tint
# When orange moon high, sky has slight orange tint
```

---

## PHASE 3: ACES Tonemapping

### Outcome
Bright moons retain color, no blowout.

### Reference Algorithm
**Source:** Krzysztof Narkowicz
**URL:** https://knarkowicz.wordpress.com/2016/01/06/aces-filmic-tone-mapping-curve/

### Implementation

**File:** `assets/shaders/sky_dome.wgsl`

Replace lines 500-504 with:

```wgsl
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
}

// In compute_sky_color():
return aces_tonemap(sky_color * exposure);
```

### Verification

```bash
cargo run --example p31_visual_fidelity_test
# Check: purple_moon.png - bright but retains purple hue, not washed white
```

---

## PHASE 4: Star Twinkle

### Outcome
Stars flicker over time.

### Reference Algorithm
**Source:** Feral_Pug Procedural Skybox Part 2
**URL:** https://feralpug.github.io/tutorial/2020-07-30-Part2-ProcSkybox/

### Implementation

In `compute_stars()`:

```wgsl
let twinkle = noise2d(grid + vec2(sky.params.w * 3.0, 0.0));
star_intensity *= (0.6 + twinkle * 0.8);
```

---

## PHASE 5: Fog/Horizon Integration

### Outcome
Terrain fades seamlessly into sky.

### Reference
- Feral_Pug Part 1: horizon value calculation
- drcarademono/dynamic-skies: fog distance integration

### Implementation

```wgsl
fn calculate_horizon_value(view_dir: vec3<f32>) -> f32 {
    return 1.0 - smoothstep(0.0, 0.15, view_dir.y);
}
```

Apply to all sky elements.

---

## PHASE 6: Two-Layer Clouds

### Outcome
Procedural clouds tinted by moon colors.

### Reference Algorithm
**Source:** Feral_Pug Procedural Skybox Part 3
**URL:** https://feralpug.github.io/tutorial/2020-07-30-Part3-ProcSkybox/

**Reference Code:**
```hlsl
float cloud1 = tex2D(_CloudDiffuse, cloudUV + _Time.y * _CloudSpeed).x;
float cloud2 = tex2D(_CloudDiffuse, cloudUV * _CloudBlendScale - _Time.y * _CloudBlendSpeed).x;
float clouds = cloud1 - cloud2;
clouds = smoothstep(_CloudAlphaCutoff, _CloudAlphaMax, clouds);
```

### Implementation

```wgsl
fn compute_clouds(view_dir: vec3<f32>, time: f32, moon1_color: vec3<f32>, moon2_color: vec3<f32>) -> vec4<f32> {
    if view_dir.y < 0.05 { return vec4(0.0); }
    
    let cloud_uv = view_dir.xz / (view_dir.y + 0.5);
    
    // Two-layer noise (Feral_Pug technique)
    let cloud1 = fbm(cloud_uv * 2.0 + vec2(time * 0.02, 0.0));
    let cloud2 = fbm(cloud_uv * 4.0 + vec2(0.373, 0.47) + vec2(0.0, time * 0.035));
    
    let cloud_density = cloud1 - cloud2 * 0.5;
    let clouds = smoothstep(0.3, 0.6, cloud_density);
    
    // Cloud color from moon lighting
    let cloud_color = (moon1_color + moon2_color) * 0.3 + vec3(0.1, 0.08, 0.12);
    
    let horizon_fade = smoothstep(0.05, 0.3, view_dir.y);
    
    return vec4(cloud_color, clouds * horizon_fade);
}
```

---

## KEY FILES

| File | Purpose |
|------|---------|
| `assets/shaders/sky_dome.wgsl` | Sky shader - REWRITE |
| `crates/studio_core/src/deferred/sky_dome.rs` | Config - remove sun |
| `crates/studio_core/src/deferred/sky_dome_node.rs` | Uniforms - remove sun |
| `crates/studio_core/src/day_night.rs` | Moon cycle - USE THIS |
| `examples/p31_visual_fidelity_test.rs` | Test harness |
| `docs/plans/seus_sky_techniques.md` | Full research with all references |

---

## EXISTING MOON SYSTEM TO USE

The `day_night.rs` already has proper dual-moon orbital calculation:

```rust
// In DayNightCycle
pub moon1_config: MoonCycleConfig,  // Purple moon
pub moon2_config: MoonCycleConfig,  // Orange moon

// MoonCycleConfig::calculate_position(cycle_time) returns (direction, height)
// MoonCycleConfig::calculate_color(height) returns color
// MoonCycleConfig::calculate_intensity(height) returns intensity
```

The sky shader should receive these values through uniforms, NOT recalculate them.

---

## VERIFICATION COMMANDS

```bash
# Run test harness
cargo run --example p31_visual_fidelity_test

# Screenshots in:
ls screenshots/visual_fidelity_test/
# - purple_moon.png
# - orange_moon.png
# - dual_moons_sky.png
# - zenith_stars.png
# - sky_with_moon.png
# - terrain_moon.png
# - building_scene.png
```

---

## REFERENCE URLS

| Technique | URL |
|-----------|-----|
| Moon ray-sphere | https://feralpug.github.io/tutorial/2020-07-30-Part2-ProcSkybox/ |
| Sky gradient | https://feralpug.github.io/tutorial/2020-07-30-Part1-ProcSkybox/ |
| ACES tonemap | https://knarkowicz.wordpress.com/2016/01/06/aces-filmic-tone-mapping-curve/ |
| Clouds | https://feralpug.github.io/tutorial/2020-07-30-Part3-ProcSkybox/ |
| Dual moon | https://github.com/drcarademono/dynamic-skies |
| Atmospheric | https://developer.nvidia.com/gpugems/gpugems2/part-ii-shading-lighting-and-shadows/chapter-16-accurate-atmospheric-scattering |
