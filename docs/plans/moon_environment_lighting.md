# Moon-Position-Based Environment Lighting System

**Status:** ACTIVE PLAN - Integrates with `markov_cloud_sky_alternative.md`

**Problem Statement:** The current system doesn't show enough impact on terrain or overall look based on moon positions:
- No dusk effect when moons are near horizon
- No dawn effect when moons are rising
- No "really dark at zenith" effect (darkest point of night isn't dark on ground)

**Root Cause:** The deferred lighting shader (`deferred_lighting.wgsl`) uses **hardcoded constants** instead of the dynamic uniform data passed from Rust.

**Related Documents:**
- `docs/plans/markov_cloud_sky_alternative.md` - Main sky system plan
- `docs/plans/visual_fidelity_improvements.md` - Overall visual improvements

---

## Architecture Overview

### Current State (BROKEN)

```
Sky Dome (sky_dome.wgsl)
├── Moon rendering (CORRECT - moons have horizon effects)
├── Cloud lighting (CORRECT - uses moon positions)
└── Sky gradient (static - doesn't affect terrain)

Deferred Lighting (deferred_lighting.wgsl)
├── Moon directional shadows (CORRECT)
├── Moon directional lighting (WRONG - uses CONSTANT direction/intensity)
└── Ambient lighting (WRONG - CONSTANT, ignores moon positions)
```

### Target State

```
Sky Dome (sky_dome.wgsl)
├── Moon rendering with horizon warmth
├── Cloud lighting from moon positions
├── Dawn/dusk horizon scatter effects
└── Sky gradient responding to moon altitudes

Deferred Lighting (deferred_lighting.wgsl)
├── Moon directional shadows (unchanged)
├── Moon lighting from DYNAMIC uniforms (scaled by altitude)
├── Dynamic ambient from moon positions + colors
└── Zenith-darkness when both moons below horizon
```

---

## Critical Discovery: The Bug

**Lines 129-141 of `deferred_lighting.wgsl` have hardcoded constants:**

```wgsl
// CURRENT (WRONG):
const MOON1_DIRECTION: vec3<f32> = vec3<f32>(0.6, -0.6, 0.55);
const MOON1_COLOR: vec3<f32> = vec3<f32>(0.4, 0.15, 0.7);
const MOON1_INTENSITY: f32 = 0.15;

const MOON2_DIRECTION: vec3<f32> = vec3<f32>(-0.6, -0.6, -0.55);
const MOON2_COLOR: vec3<f32> = vec3<f32>(1.0, 0.45, 0.1);
const MOON2_INTENSITY: f32 = 0.12;
```

**The fix - use the uniforms that are already being passed:**

```wgsl
// CORRECT - use shadow_uniforms which already has this data:
let moon1_dir = normalize(-shadow_uniforms.moon1_direction.xyz);
let moon1_color = shadow_uniforms.moon1_color_intensity.rgb;
let moon1_intensity = shadow_uniforms.moon1_color_intensity.a;
```

---

## Implementation Phases

### Phase A: Fix Deferred Lighting to Use Dynamic Uniforms

**Priority:** HIGHEST - This is the root cause fix

**Outcome:** Terrain lighting changes when moon positions change (T/Y keys in test)

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Press T to move purple moon - terrain lighting should shift purple
# Press Y to move orange moon - terrain lighting should shift orange
```

**Changes to `deferred_lighting.wgsl`:**

1. Delete the hardcoded constants (lines 129-141)
2. In the DARK_WORLD_MODE section (lines 578-596), already uses `shadow_uniforms` - verify this is working
3. The constants at top are dead code but may confuse readers - remove them

**Files Modified:**
- `assets/shaders/deferred_lighting.wgsl`

---

### Phase B: Moon Altitude-Based Intensity Scaling

**Priority:** HIGH

**Outcome:** Moon light intensity on terrain scales with altitude:
- Moon at horizon (altitude ~0): dim light, warm color
- Moon at zenith (altitude ~1): full intensity
- Moon below horizon (altitude < 0): no contribution

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Move moon to horizon (T key) - terrain should be dimmer
# Move moon to zenith - terrain should be brighter
```

**Changes to `deferred_lighting.wgsl`:**

```wgsl
// Add altitude-based intensity scaling
fn calculate_moon_contribution(
    moon_dir: vec3<f32>,
    moon_color: vec3<f32>,
    moon_intensity: f32,
    world_normal: vec3<f32>,
    shadow: f32
) -> vec3<f32> {
    // Moon altitude: -1 (nadir) to +1 (zenith)
    let altitude = moon_dir.y;
    
    // Scale intensity by altitude (0 below horizon, full at zenith)
    let altitude_factor = smoothstep(-0.1, 0.5, altitude);
    
    // Horizon warmth: shift color warmer when near horizon
    let horizon_proximity = 1.0 - abs(altitude);
    let warm_shift = vec3<f32>(0.15, 0.05, -0.1) * horizon_proximity * horizon_proximity;
    let adjusted_color = moon_color + warm_shift;
    
    // Standard N.L lighting
    let n_dot_l = max(dot(world_normal, -moon_dir), 0.0);
    
    return adjusted_color * moon_intensity * altitude_factor * n_dot_l * shadow;
}
```

**Files Modified:**
- `assets/shaders/deferred_lighting.wgsl`

---

### Phase C: Zenith-Darkness (Both Moons Below Horizon)

**Priority:** HIGH

**Outcome:** When both moons are below the horizon, terrain becomes nearly black

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Move both moons below horizon (T and Y keys past 0.5)
# Terrain should be very dark (only ambient)
```

**Changes to `deferred_lighting.wgsl`:**

```wgsl
// Calculate night depth based on moon positions
fn calculate_night_depth(moon1_alt: f32, moon2_alt: f32) -> f32 {
    // How far below horizon are both moons?
    let moon1_below = max(0.0, -moon1_alt);
    let moon2_below = max(0.0, -moon2_alt);
    
    // Night depth: 0 = at least one moon visible, 1 = both deep below
    // Use minimum because we need BOTH moons below for true darkness
    let depth = min(moon1_below, moon2_below) * 2.0;  // 0.5 below = full night
    return clamp(depth, 0.0, 1.0);
}

// In lighting calculation:
let moon1_alt = shadow_uniforms.moon1_direction.y;
let moon2_alt = shadow_uniforms.moon2_direction.y;
let night_depth = calculate_night_depth(moon1_alt, moon2_alt);

// Darken everything when both moons are down
let darkness_factor = 1.0 - night_depth * 0.95;  // Goes to 0.05 at deepest
total_light *= darkness_factor;
```

**Files Modified:**
- `assets/shaders/deferred_lighting.wgsl`

---

### Phase D: Dynamic Ambient Color from Moon Positions

**Priority:** HIGH

**Outcome:** Ambient light color blends between moon colors based on which is higher/more visible

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Purple moon high, orange low - ambient has purple tint
# Orange moon high, purple low - ambient has orange tint
# Both visible - blended ambient
```

**Changes:**

1. **Update `DirectionalShadowUniforms` in Rust** to include ambient data:

```rust
// In lighting_node.rs or wherever DirectionalShadowUniforms is defined
pub struct DirectionalShadowUniforms {
    // ... existing fields ...
    pub ambient_color: [f32; 4],  // rgb = color, a = intensity
}
```

2. **Calculate ambient in Rust** based on moon positions:

```rust
fn calculate_ambient_from_moons(moon1_dir: Vec3, moon2_dir: Vec3, 
                                 moon1_color: Color, moon2_color: Color) -> (Color, f32) {
    let moon1_contrib = (moon1_dir.y + 0.1).max(0.0);
    let moon2_contrib = (moon2_dir.y + 0.1).max(0.0);
    let total = moon1_contrib + moon2_contrib + 0.001; // avoid div by zero
    
    let blend = moon1_contrib / total;
    let ambient_color = moon1_color.mix(moon2_color, 1.0 - blend) * 0.3;
    
    let max_visible = moon1_dir.y.max(moon2_dir.y);
    let intensity = if max_visible < -0.1 {
        0.01  // True darkness
    } else {
        0.02 + max_visible.max(0.0) * 0.08
    };
    
    (ambient_color, intensity)
}
```

3. **Use in shader:**

```wgsl
// Replace DARK_AMBIENT_COLOR constant with uniform
let ambient = shadow_uniforms.ambient_color.rgb * shadow_uniforms.ambient_color.a;
total_light = ambient;
```

**Files Modified:**
- `assets/shaders/deferred_lighting.wgsl`
- `crates/studio_core/src/deferred/lighting_node.rs` (or equivalent)

---

### Phase E: Dawn/Dusk Horizon Scatter in Sky Dome

**Priority:** MEDIUM

**Outcome:** Sky gradient shifts warmer near horizon when moon is rising/setting

**Verification:**
```bash
cargo run --example p34_sky_terrain_test
# Moon near horizon - sky has warm glow in that direction
# Moon at zenith - no horizon warmth
```

**Changes to `sky_dome.wgsl`:**

```wgsl
// Add horizon scatter effect to sky gradient
fn calculate_horizon_scatter(ray_dir: vec3<f32>, moon_dir: vec3<f32>, moon_color: vec3<f32>) -> vec3<f32> {
    // Only apply when moon is near horizon
    let moon_altitude = moon_dir.y;
    let horizon_proximity = 1.0 - abs(moon_altitude);
    let is_near_horizon = smoothstep(0.3, 0.0, abs(moon_altitude));
    
    // Scatter in direction of moon
    let scatter_angle = max(dot(ray_dir, moon_dir), 0.0);
    let scatter = pow(scatter_angle, 3.0) * is_near_horizon;
    
    // Warm color shift
    let warm_color = moon_color + vec3<f32>(0.2, 0.1, -0.05);
    
    return warm_color * scatter * 0.3;
}
```

**Files Modified:**
- `assets/shaders/sky_dome.wgsl`

---

### Phase F: Environment LUT (Optional - Artist Control)

**Priority:** LOW - Only if procedural approach isn't flexible enough

**Outcome:** 2D lookup texture maps moon altitudes to environment parameters

**Implementation:**
- Create 64x64 RGBA texture
- X axis = moon1 altitude (-1 to +1 mapped to 0-1)
- Y axis = moon2 altitude (-1 to +1 mapped to 0-1)
- R,G,B = ambient color
- A = ambient intensity

**Files Modified:**
- New texture asset
- `deferred_lighting.wgsl` - add texture sampler
- `lighting_node.rs` - add texture binding

---

## Integration with MarkovJunior Sky Plan

These phases slot into the existing `markov_cloud_sky_alternative.md` plan:

| MJ Phase | Environment Phase | Description |
|----------|------------------|-------------|
| 0-4 | - | Test harness, cloud textures (DONE) |
| - | **A** | Fix deferred lighting uniforms |
| - | **B** | Moon altitude intensity |
| - | **C** | Zenith darkness |
| - | **D** | Dynamic ambient |
| 5 | - | UV flow animation |
| - | **E** | Dawn/dusk scatter |
| 7 | - | Wind-based flow |
| 8 | - | Moon texture improvement |
| 12 | - | Star field |
| 13 | - | Moon light on clouds |
| - | **F** | Environment LUT (optional) |

---

## Verification Commands Summary

| Phase | Command | What to Check |
|-------|---------|---------------|
| A | `cargo run --example p34_sky_terrain_test` | T/Y keys change terrain lighting |
| B | Same | Moon at horizon = dimmer terrain |
| C | Same | Both moons below = very dark terrain |
| D | Same | Ambient color shifts with moon dominance |
| E | Same | Horizon glows warm when moon rises/sets |
| F | Same | LUT-controlled color grading |

---

## Files Summary

**Must Modify:**
- `assets/shaders/deferred_lighting.wgsl` - Phases A, B, C, D
- `assets/shaders/sky_dome.wgsl` - Phase E

**May Modify:**
- `crates/studio_core/src/deferred/lighting_node.rs` - Phase D (ambient uniform)
- `crates/studio_core/src/deferred/directional_shadow_node.rs` - If ambient needs new uniform

**New Files (Phase F only):**
- `assets/textures/environment_lut.png`

---

## Expected Visual Results

| Scenario | Current | After |
|----------|---------|-------|
| Purple moon at zenith | Static purple tint | Bright purple terrain lighting |
| Orange moon at zenith | Static orange tint | Bright orange terrain lighting |
| Moon rising (altitude 0.1) | Same as zenith | Dim, warm-shifted light |
| Moon setting (altitude 0.1) | Same as zenith | Dim, warm-shifted light |
| Both moons below horizon | Still visible light | Nearly black terrain |
| Purple high, orange low | Equal contribution | Purple-dominant ambient |
| Horizon toward rising moon | No effect | Warm glow in sky |

---

## References

- Current shader: `assets/shaders/deferred_lighting.wgsl` lines 129-141, 578-596
- Moon uniforms: `DirectionalShadowUniforms` in lighting node
- Sky dome: `assets/shaders/sky_dome.wgsl` already has horizon effects for moon rendering
