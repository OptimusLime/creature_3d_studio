# SEUS-Inspired Sky Rendering Techniques

---

## CRITICAL CONTEXT: DUAL-MOON WORLD

**There is no sun. There is no daytime.**

This world has TWO MOONS that orbit independently:
- **Purple moon** - period 1.0, phase_offset 0.0, inclination 30°
- **Orange moon** - period 0.8, phase_offset 0.5, inclination 15°

Both moons are visible simultaneously at various points in the cycle. The sky is always night. Lighting comes from the moons.

**The current `sky_dome.wgsl` is WRONG** - it has:
- `day_sky_gradient()`, `twilight_gradient()` - WRONG, no day exists
- `render_sun()` - WRONG, no sun exists
- Sun-based atmospheric scattering - WRONG
- Moon phase from sun direction - WRONG, moons light themselves

**The existing system we MUST integrate with:**
- `day_night.rs`: `DayNightCycle` with `MoonCycleConfig` for each moon
- `MoonCycleConfig::calculate_position(cycle_time)` returns direction + height
- `MoonCycleConfig::calculate_color(height)` returns interpolated color
- `ColorLutConfig` - time-based color grading already exists

---

## EXECUTION PLAN (Highest Impact First)

### Outcomes

| Priority | Technique | Visual Outcome | Effort |
|----------|-----------|----------------|--------|
| 1 | Dual-Moon SEUS Rendering | Two visible moon discs with proper ray-sphere intersection, surface detail, orbital motion | Medium |
| 2 | Moon-Lit Sky Gradient | Sky color determined by moon positions/colors, not sun | Low |
| 3 | ACES Tonemapping | Bright moons retain color, no blowout | Low |
| 4 | Star Twinkle Animation | Stars flicker over time | Low |
| 5 | Fog/Horizon Integration | Terrain fades into sky seamlessly | Low |
| 6 | Two-Layer Clouds | Procedural clouds tinted by moon colors | Medium |
| 7 | Aurora Effects | Animated northern lights | High |

### Systems Being Changed

| File | What Changes |
|------|--------------|
| `assets/shaders/sky_dome.wgsl` | DELETE sun code, REWRITE for dual-moon world |
| `crates/studio_core/src/deferred/sky_dome.rs` | Remove sun config, add exposure |
| `crates/studio_core/src/deferred/sky_dome_node.rs` | Remove SunOrbit, use DayNightCycle moons |
| `examples/p31_visual_fidelity_test.rs` | Capture moon trajectories over time |

### What Gets Deleted

| Current Code | Why Delete |
|--------------|------------|
| `day_sky_gradient()` lines 203-225 | No daytime exists |
| `twilight_gradient()` lines 183-200 | No twilight exists |
| `render_sun()` lines 281-318 | No sun exists |
| `SunOrbit` struct in sky_dome_node.rs | No sun exists |
| `SunAppearance` in sky_dome.rs | No sun exists |
| All `sun_*` uniforms | No sun exists |

### What Gets Replaced

| Current Code | Problem | Replacement |
|--------------|---------|-------------|
| `render_moon()` lines 324-384 | Disc doesn't render | SEUS ray-sphere with proper math |
| `compute_atmospheric_scattering()` | Sun-based | Moon-based gradient |
| Tonemap lines 500-504 | Ad-hoc | ACES curve |
| Static stars | No animation | Time-modulated twinkle |

---

## PRIORITY 1: Dual-Moon SEUS Rendering

**Impact:** HIGHEST - The moons ARE the light source. If they don't render, nothing works.

**Effort:** Medium

### Reference

**Algorithm:** Ray-Sphere Intersection for Moon Rendering

**Source:** Feral_Pug Procedural Skybox Tutorial
- URL: https://feralpug.github.io/tutorial/2020-07-30-Part2-ProcSkybox/
- Technique: Sphere tracing to find moon fragment, then compute normal for UVs and lighting

**Reference Code (HLSL):**
```hlsl
// From Feral_Pug tutorial
float3 moonFragPos = normWorldPos * sphere + float3(0, 0, 0);
// The normal is how we eventually get UVs and lighting
float3 moonFragNormal = normalize(moonFragPos - currentMoonPos);

// If our sphere tracing returned a positive value we have a moon fragment
if (sphere >= 0.0) {
    // Grab the moon tex and multiply the color
    float3 moonTex = tex2D(_MoonTex, moonUV).rgb * _MoonColor.rgb;
}

// Moon phase from sun direction (Lambert lighting)
float NDotL = dot(moonPhase, phaseNormal);
```

**Additional Reference:** drcarademono/dynamic-skies (Daggerfall Unity)
- URL: https://github.com/drcarademono/dynamic-skies
- Adds second moon layer to the Feral_Pug approach

### What We Build

Two moon discs rendered via ray-sphere intersection with:
- Proper 3D sphere appearance
- Surface texture from procedural noise
- Orbital motion visible when recording over time
- Glow halos around each moon
- Horizon atmospheric effects

### Implementation

**File:** `assets/shaders/sky_dome.wgsl`

```wgsl
// Render a moon using ray-sphere intersection
// Based on Feral_Pug sphere tracing technique
fn render_moon_sphere(
    view_dir: vec3<f32>,
    moon_dir: vec3<f32>,    // Direction TO moon (from DayNightCycle)
    moon_size: f32,          // Angular size in radians
    moon_color: vec3<f32>,   // Color from MoonCycleConfig
    moon_intensity: f32,     // Intensity from MoonCycleConfig
) -> vec3<f32> {
    // Skip if moon below horizon
    if moon_dir.y < -0.1 {
        return vec3<f32>(0.0);
    }
    
    // Virtual sphere for intersection
    let sphere_dist = 100.0;
    let sphere_pos = moon_dir * sphere_dist;
    let sphere_radius = tan(moon_size) * sphere_dist;
    
    // Ray-sphere intersection
    let oc = -sphere_pos;
    let b = dot(oc, view_dir);
    let c = dot(oc, oc) - sphere_radius * sphere_radius;
    let discriminant = b * b - c;
    
    var result = vec3<f32>(0.0);
    
    if discriminant >= 0.0 {
        // Hit the moon sphere
        let t = -b - sqrt(discriminant);
        let hit_pos = view_dir * t;
        let moon_normal = normalize(hit_pos - sphere_pos);
        
        // Surface detail from noise
        let uv = vec2(
            atan2(moon_normal.z, moon_normal.x) / (2.0 * PI) + 0.5,
            asin(clamp(moon_normal.y, -1.0, 1.0)) / PI + 0.5
        );
        let surface = 0.85 + fbm(uv * 8.0) * 0.15;
        
        // Limb darkening
        let limb = pow(max(0.0, dot(moon_normal, -view_dir)), 0.3);
        
        // Full illumination (moons are self-luminous in this world)
        result = moon_color * surface * limb * moon_intensity * 2.0;
    }
    
    // Glow halo (always, even if disc not hit)
    let angle = acos(clamp(dot(view_dir, moon_dir), -1.0, 1.0));
    let glow_radius = moon_size * 3.0;
    if angle < glow_radius {
        let glow_t = angle / glow_radius;
        let glow = exp(-glow_t * glow_t * 3.0) * 0.4 * moon_intensity;
        result += moon_color * glow;
    }
    
    // Horizon atmospheric reddening
    if moon_dir.y < 0.25 {
        let reddening = 1.0 - moon_dir.y / 0.25;
        result *= mix(vec3(1.0), vec3(1.2, 0.85, 0.6), reddening * 0.4);
    }
    
    return result;
}
```

**Compositing both moons in `compute_sky_color()`:**
```wgsl
// Render both moons
let moon1 = render_moon_sphere(
    view_dir,
    normalize(sky.moon1_direction.xyz),
    sky.moon1_direction.w,  // size
    sky.moon1_color.rgb,
    sky.moon1_color.a       // intensity
);

let moon2 = render_moon_sphere(
    view_dir,
    normalize(sky.moon2_direction.xyz),
    sky.moon2_direction.w,
    sky.moon2_color.rgb,
    sky.moon2_color.a
);

sky_color += moon1 + moon2;
```

### Verification

```bash
cargo run --example p31_visual_fidelity_test
# Check: purple_moon.png - solid purple disc visible with surface detail
# Check: orange_moon.png - solid orange disc visible
# Check: dual_moons_sky.png - BOTH moons visible in same frame
```

**Trajectory recording:**
```bash
# Capture at multiple cycle times to verify orbital motion
for t in 0.0 0.1 0.2 0.3 0.4 0.5; do
    # Set time and capture
done
# Moons should be at different positions in each frame
```

---

## PRIORITY 2: Moon-Lit Sky Gradient

**Impact:** HIGH - Sky color should respond to moon positions, not sun.

**Effort:** LOW

### Reference

**Algorithm:** Horizon-based sky gradient with light source contribution

**Source:** Feral_Pug Procedural Skybox Tutorial
- URL: https://feralpug.github.io/tutorial/2020-07-30-Part1-ProcSkybox/
- Technique: `horizonValue = dot(normWorldPos, float3(0, 1, 0))` for vertical gradient

**Reference Code (HLSL):**
```hlsl
// From Feral_Pug tutorial - horizon fade calculation
float horizonValue = dot(normWorldPos, float3(0, 1, 0));
horizonValue = 1 - saturate(Remap(horizonValue, float2(_SkyFadeStart, _SkyFadeEnd), float2(0, 1)));

// Night transition based on sun height (we adapt for moon height)
float sunDotUp = dot(sunPos, float3(0, 1, 0));
float night = saturate(Remap(sunDotUp, float2(_NightStartHeight, _NightEndHeight), float2(0, 1)));

// Lerp to night colors
col.rgb = lerp(col.rgb, col.rgb + stars, night * horizonValue);
```

**Adaptation for Dual-Moon:** Instead of sun height determining night factor, we use moon positions to add color contribution to sky.

### What We Build

Replace sun-based gradient with moon-based gradient:
- Sky tinted by combined moon colors
- Darker when both moons low, brighter when moons high
- No day/twilight/night distinction - always night, varying intensity

### Implementation

**File:** `assets/shaders/sky_dome.wgsl`

Delete `day_sky_gradient()`, `twilight_gradient()`. Replace `compute_atmospheric_scattering()`:

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
    
    // Moon 1 contribution to sky color (glow toward moon)
    if moon1_dir.y > -0.1 {
        let moon1_angle = max(0.0, dot(view_dir, moon1_dir));
        let moon1_glow = pow(moon1_angle, 4.0) * moon1_intensity * 0.1;
        sky += moon1_color * moon1_glow;
    }
    
    // Moon 2 contribution
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
# Check: sky_with_moon.png - sky has subtle color tint toward moon
# Check: when purple moon high, sky has slight purple tint
# Check: when orange moon high, sky has slight orange tint
```

---

## PRIORITY 3: ACES Tonemapping

**Impact:** MEDIUM - Bright moons don't wash to white.

**Effort:** LOW

### Reference

**Algorithm:** ACES (Academy Color Encoding System) Filmic Tonemapping

**Source:** SEUS Shader Options + Krzysztof Narkowicz
- SEUS provides multiple tonemap operators: SEUSTonemap, ACESTonemap, Uncharted2Tonemap, BurgessTonemap, ReinhardJodie, ExponentialTonemap
- URL (Narkowicz ACES): https://knarkowicz.wordpress.com/2016/01/06/aces-filmic-tone-mapping-curve/

**Reference Code:**
```hlsl
// ACES approximation by Krzysztof Narkowicz
float3 ACESFilm(float3 x) {
    float a = 2.51f;
    float b = 0.03f;
    float c = 2.43f;
    float d = 0.59f;
    float e = 0.14f;
    return saturate((x*(a*x+b))/(x*(c*x+d)+e));
}
```

**Additional Reference:** LearnOpenGL HDR Tutorial
- URL: https://learnopengl.com/Advanced-Lighting/HDR
- Covers exposure control and various tonemapping operators

### Implementation

Replace lines 500-504 in `sky_dome.wgsl`:

```wgsl
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
}

// Apply in compute_sky_color():
return aces_tonemap(sky_color * exposure);
```

### Verification

```bash
cargo run --example p31_visual_fidelity_test
# Check: purple_moon.png - moon is bright but retains purple hue
```

---

## PRIORITY 4: Star Twinkle Animation

**Impact:** MEDIUM - Night sky feels alive.

**Effort:** LOW

### Reference

**Algorithm:** Time-modulated noise for star brightness variation

**Source:** Feral_Pug Procedural Skybox Tutorial
- URL: https://feralpug.github.io/tutorial/2020-07-30-Part2-ProcSkybox/
- Technique: Sample noise texture with time offset, subtract from star brightness

**Reference Code (HLSL):**
```hlsl
// From Feral_Pug tutorial - star twinkle
// Sample a basic noise texture with time offset to modulate star brightness
float twinkle = tex2D(_TwinkleTex, (starsUV * _TwinkleTex_ST.xy) + _TwinkleTex_ST.zw + float2(1, 0) * _Time.y * _TwinkleSpeed).r;
// Modulate the twinkle value
twinkle *= _TwinkleBoost;
// Adjust the final color
stars -= twinkle;
stars = saturate(stars);
```

**Adaptation:** We use procedural noise instead of texture sample since we already have `noise2d()` function.

### Implementation

In `compute_stars()`, add time modulation:

```wgsl
// Twinkle animation (adapted from Feral_Pug technique)
let twinkle = noise2d(grid + vec2(sky.params.w * 3.0, 0.0));
star_intensity *= (0.6 + twinkle * 0.8);
```

### Verification

```bash
# Run test at two different times, compare star brightness
```

---

## PRIORITY 5: Fog/Horizon Integration

**Impact:** MEDIUM - Terrain blends into sky.

**Effort:** LOW

### Reference

**Algorithm:** Horizon-value based fade with fog color matching

**Source:** Feral_Pug Procedural Skybox + drcarademono/dynamic-skies
- URL: https://feralpug.github.io/tutorial/2020-07-30-Part1-ProcSkybox/
- URL: https://github.com/drcarademono/dynamic-skies

**Reference Code (HLSL):**
```hlsl
// Horizon value calculation
float horizonValue = dot(normWorldPos, float3(0, 1, 0));
horizonValue = 1 - saturate(Remap(horizonValue, float2(_SkyFadeStart, _SkyFadeEnd), float2(0, 1)));
```

**From dynamic-skies README:**
> Fog distance - used in calculating how much the weather fog will influence the skybox. DFU uses Exponential fog by default.

### Implementation

Match fog to sky horizon color. Apply `horizonValue` fade to all elements.

```wgsl
fn calculate_horizon_value(view_dir: vec3<f32>) -> f32 {
    return 1.0 - smoothstep(0.0, 0.15, view_dir.y);
}
```

---

## PRIORITY 6: Two-Layer Clouds

**Impact:** MEDIUM - Adds depth to sky, but moons must work first.

**Effort:** MEDIUM

### Reference

**Algorithm:** Two-layer noise subtraction with normal-based lighting

**Source:** Feral_Pug Procedural Skybox Tutorial
- URL: https://feralpug.github.io/tutorial/2020-07-30-Part3-ProcSkybox/
- Technique: Sample cloud texture twice at different scales, subtract for billowy effect

**Reference Code (HLSL):**
```hlsl
// From Feral_Pug tutorial - two-layer cloud generation
// Sample the cloud texture twice at different speeds, offsets and scale
float cloud1 = tex2D(_CloudDiffuse, cloudUV * _CloudDiffuse_ST.xy + _CloudDiffuse_ST.zw + _Time.y * _CloudSpeed * cloudDir).x * horizonValue;
float cloud2 = tex2D(_CloudDiffuse, cloudUV * _CloudDiffuse_ST.xy * _CloudBlendScale + _CloudDiffuse_ST.zw - _Time.y * _CloudBlendSpeed * cloudDir + float2(.373, .47)).x * horizonValue;

// Subtract cloud2 from cloud1 - this is how we blend them
float clouds = cloud1 - cloud2;

// Smoothstep for edge control
clouds = smoothstep(_CloudAlphaCutoff, _CloudAlphaMax, clouds);

// Normal-based lighting for volume
float3 cloudNormal1 = UnpackNormal(tex2D(_CloudNormal, cloudUV...));
float3 cloudNormal2 = UnpackNormal(tex2D(_CloudNormal, cloudUV...));
// Blend with up vector for fluffy effect
float NdotUp = dot(cloudNormal, float3(0, 1, 0));
```

**Additional Reference:** drcarademono/dynamic-skies
- Adds second cloud layer for depth

**Masking stars/moons behind clouds:**
```hlsl
// From Feral_Pug - proper layer compositing
col.rgb = lerp(col.rgb, col.rgb + stars, night * horizonValue * (1.0 - clouds));
col.rgb = lerp(col.rgb, moonColor, horizon * (1.0 - clouds));
```

### What We Build

Procedural clouds tinted by moon colors (not sun). Cloud color = blend of moon colors based on positions.

### Implementation

```wgsl
fn compute_clouds(view_dir: vec3<f32>, time: f32, moon1_color: vec3<f32>, moon2_color: vec3<f32>) -> vec4<f32> {
    if view_dir.y < 0.05 { return vec4(0.0); }
    
    // UV from view direction
    let cloud_uv = view_dir.xz / (view_dir.y + 0.5);
    
    // Two-layer noise (Feral_Pug technique)
    let cloud1 = fbm(cloud_uv * 2.0 + vec2(time * 0.02, 0.0));
    let cloud2 = fbm(cloud_uv * 4.0 + vec2(0.373, 0.47) + vec2(0.0, time * 0.035));
    
    // Subtract for billowy effect
    let cloud_density = cloud1 - cloud2 * 0.5;
    let clouds = smoothstep(0.3, 0.6, cloud_density);
    
    // Cloud color from moon lighting (not sun)
    let cloud_color = (moon1_color + moon2_color) * 0.3 + vec3(0.1, 0.08, 0.12);
    
    // Horizon fade
    let horizon_fade = smoothstep(0.05, 0.3, view_dir.y);
    
    return vec4(cloud_color, clouds * horizon_fade);
}
```

---

## PRIORITY 7: Aurora Effects (Future)

**Impact:** HIGH for fantasy setting, but complex.

**Effort:** HIGH

### Reference

**Algorithm:** Volumetric raymarch through noise field with vertical curtain structure

**Source:** Shadertoy - Volumetric Aurora Borealis
- URL: https://godotshaders.com/shader/volumetric-aurora-borealis-with-polar-reflection/
- Technique: Raymarch with noise sampling, alpha blending over background

**Reference Code (GLSL):**
```glsl
// From Godot Shaders aurora
vec4 aur = smoothstep(0., 1.5, aurora(ro, rd, FRAGCOORD.xy)) * fade;
col += stars(rd, iResolution);
col = col * (1. - aur.a) + aur.rgb;

// Reflection handling
vec3 rrd = rd;
rrd.y = abs(rrd.y);
col = bg(rrd) * fade * 0.6;
vec4 aur = smoothstep(0.0, 2.5, aurora(ro, rrd, FRAGCOORD.xy));
col += stars(rrd, iResolution) * 0.1;
col = col * (1. - aur.a) + aur.rgb;
```

**Star integration from same shader:**
```glsl
vec3 stars(vec3 p, vec2 res) {
    vec3 c = vec3(0.);
    float res_val = res.x;
    for (float i=0.; i<4.; i++) {
        vec3 q = fract(p * (0.15 * res_val)) - 0.5;
        vec3 id = floor(p * (0.15 * res_val));
        vec2 rn = nmzHash33(id).xy;
        float c2 = 1. - smoothstep(0., 0.6, length(q));
        c2 *= step(rn.x, 0.0005 + i * i * 0.001);
        c += c2 * (mix(vec3(1.0, 0.49, 0.1), vec3(0.75, 0.9, 1.0), rn.y) * ...
    }
}
```

### Status

Deferred. Moons must work first. This is the most complex technique and requires raymarch infrastructure.

---

## Quick Reference

| Rank | Technique | Status | Reference Source |
|------|-----------|--------|------------------|
| 1 | Dual-Moon Rendering | REPLACE | Feral_Pug ray-sphere |
| 2 | Moon-Lit Sky Gradient | REPLACE | Feral_Pug horizon blend |
| 3 | ACES Tonemapping | REPLACE | Narkowicz ACES |
| 4 | Star Twinkle | ADD | Feral_Pug twinkle |
| 5 | Fog/Horizon | ADD | Feral_Pug + dynamic-skies |
| 6 | Two-Layer Clouds | ADD | Feral_Pug clouds |
| 7 | Aurora Effects | FUTURE | Godot Shaders volumetric |

---

## All References

| Technique | Algorithm | Source | URL |
|-----------|-----------|--------|-----|
| Moon Rendering | Ray-sphere intersection | Feral_Pug Skybox Part 2 | https://feralpug.github.io/tutorial/2020-07-30-Part2-ProcSkybox/ |
| Dual Moon | Second moon layer | drcarademono/dynamic-skies | https://github.com/drcarademono/dynamic-skies |
| Sky Gradient | Horizon value blend | Feral_Pug Skybox Part 1 | https://feralpug.github.io/tutorial/2020-07-30-Part1-ProcSkybox/ |
| ACES Tonemap | Filmic curve | Krzysztof Narkowicz | https://knarkowicz.wordpress.com/2016/01/06/aces-filmic-tone-mapping-curve/ |
| HDR/Exposure | Tone mapping overview | LearnOpenGL | https://learnopengl.com/Advanced-Lighting/HDR |
| Star Twinkle | Time-offset noise | Feral_Pug Skybox Part 2 | https://feralpug.github.io/tutorial/2020-07-30-Part2-ProcSkybox/ |
| Two-Layer Clouds | Noise subtraction | Feral_Pug Skybox Part 3 | https://feralpug.github.io/tutorial/2020-07-30-Part3-ProcSkybox/ |
| Cloud Lighting | Normal-based NdotUp | Feral_Pug Skybox Part 3 | https://feralpug.github.io/tutorial/2020-07-30-Part3-ProcSkybox/ |
| Fog Distance | Exponential fog | dynamic-skies README | https://github.com/drcarademono/dynamic-skies |
| Aurora | Volumetric raymarch | Godot Shaders | https://godotshaders.com/shader/volumetric-aurora-borealis-with-polar-reflection/ |
| Atmospheric Scattering | Nishita/O'Neil | NVIDIA GPU Gems Ch16 | https://developer.nvidia.com/gpugems/gpugems2/part-ii-shading-lighting-and-shadows/chapter-16-accurate-atmospheric-scattering |
| Mie Phase | Henyey-Greenstein | NVIDIA GPU Gems Ch16 | https://developer.nvidia.com/gpugems/gpugems2/part-ii-shading-lighting-and-shadows/chapter-16-accurate-atmospheric-scattering |
| Bloom | Gaussian blur | LearnOpenGL | https://learnopengl.com/Advanced-Lighting/Bloom |

---

## File Locations

- Sky shader: `assets/shaders/sky_dome.wgsl`
- Sky config: `crates/studio_core/src/deferred/sky_dome.rs`
- Sky node: `crates/studio_core/src/deferred/sky_dome_node.rs`
- Day/night cycle: `crates/studio_core/src/day_night.rs`
- Test harness: `examples/p31_visual_fidelity_test.rs`
