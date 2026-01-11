# DMSkybox vs Our Sky Dome: Architecture Comparison

## Executive Summary

**DMSkybox uses a fundamentally different approach:** Physical 3D mesh objects for moons/suns placed between layered sky spheres, rather than procedurally rendering celestial bodies in a single shader pass.

This explains why our procedural moon rendering is more complex and harder to debug - we're trying to do in one shader what DMSkybox achieves through multi-layer geometry.

---

## Architectural Comparison

| Aspect | DMSkybox | Our Approach |
|--------|----------|--------------|
| **Moon Rendering** | Physical 3D sphere mesh | Procedural ray-sphere/angle in shader |
| **Moon Phases** | Automatic via Unity directional light | Manual calculation needed |
| **Sky Layers** | Multiple spheres with different blend modes | Single fullscreen pass |
| **Stars** | Cubemap texture with rotation | Procedural hash-based noise |
| **View Direction** | `normalize(WorldPosition)` from mesh vertex | Reconstruct from UV + inv_view_proj |
| **Sun/Moon Position** | Transform.forward of actual GameObject | Uniform vec4 direction |
| **Atmospheric Tint** | Additive layer BEHIND moons | Must composite in single pass |

---

## DMSkybox Layer Architecture

```
Camera
  |
  v
[Clouds Low Sphere]      - Alpha blended, closest
[Clouds High Sphere]     - Alpha blended
[Atmosphere Sphere]      - ADDITIVE blend (key!)
[Sun/Moon 3D Meshes]     - Physical objects between layers
[Aurora Sphere]          - Placeholder
[Stars Sphere]           - Furthest, cubemap sampled
```

**Critical insight:** The atmosphere layer uses **additive blending** and renders IN FRONT of the moons/suns. This creates natural atmospheric scattering without computing it procedurally - moons simply show through with tint added on top.

---

## View Direction Computation

### DMSkybox (from vertex shader)
```hlsl
// World position of vertex IS the view direction for a sky sphere
float3 normalizeResult566 = normalize(WorldPosition);
float dotResult563 = dot(normalizeResult566, float3(0,1,0));
```

**Simple:** Since the skybox is a sphere mesh centered on camera, the world position of each vertex (after centering) IS the view direction.

### Our Approach (from fullscreen quad)
```wgsl
fn get_view_direction(uv: vec2<f32>) -> vec3<f32> {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0);
    let world_pos = sky.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    return normalize(world_pos.xyz / world_pos.w);
}
```

**Complex:** We use a fullscreen triangle, so we must reconstruct view direction from screen UV using inverse view-projection matrix.

**Potential issue:** If `inv_view_proj` is incorrect or the depth value (1.0) doesn't correctly map to far plane, view direction will be wrong.

---

## Moon Rendering Deep Dive

### DMSkybox: Physical Mesh + Standard Lighting
```hlsl
// SkyboxPlanet.shader - runs on actual sphere mesh
float dotResult40 = dot(WorldNormalVector(i, UnpackScaleNormal(...)), ase_worldlightDir);
c.rgb = saturate((((_PhaseLightingColor * ase_lightAtten) * dotResult40) * Albedo47 * ase_lightColor));
```

**How it works:**
1. Moon is a 3D sphere mesh with normal map
2. Unity directional light provides `ase_worldlightDir`
3. Standard N.L (Normal dot Light) shading creates moon phase automatically
4. Atmosphere sphere in front adds color tint

**Advantages:**
- Moon phase is FREE from lighting system
- Texture mapping is trivial (UVs from mesh)
- Atmosphere integration automatic
- No complex procedural math

**Disadvantages:**
- Extra draw calls (one per celestial body)
- Can't easily change moon size at runtime
- Requires mesh management

### Our Approach: Procedural Angle-Based
```wgsl
fn render_moon(view_dir, moon_dir, moon_size, ...) {
    let cos_angle = dot(view_dir, moon_dir);
    let angle = acos(clamp(cos_angle, -1.0, 1.0));
    
    if angle < moon_size {
        // Inside moon disc
        let dist_from_center = angle / moon_size;
        let limb = pow(1.0 - dist_from_center, limb_darkening * 0.5);
        result = moon_color * limb * 2.0;
    }
}
```

**How it works:**
1. Compare view direction to moon direction
2. Calculate angle between them
3. If angle < moon angular size, we're looking at the moon disc
4. Manually compute limb darkening, surface detail, glow

**Advantages:**
- Single draw call
- Fully procedural, infinitely configurable
- No mesh assets needed

**Disadvantages:**
- Complex math prone to bugs
- Must manually implement EVERYTHING (phase, limb darkening, texture, glow)
- Harder to debug
- **View direction must be correct!**

---

## Sky Gradient Comparison

### DMSkybox
```hlsl
float SkyColorGradient573 = saturate(
    pow(1.0 - abs(dotResult563), _SkyGradientPower) * _SkyGradientScale
);
float4 lerpResult575 = lerp(_SkyColorTop, _SkyColorBottom, SkyColorGradient573);
```

Formula: `gradient = (1 - |y|)^power * scale`

This concentrates color change at the horizon where `|y|` is small.

### Our Approach
```wgsl
fn compute_sky_gradient(view_dir: vec3<f32>) -> vec3<f32> {
    let view_up = max(0.0, view_dir.y);
    let t = pow(view_up, blend_power);
    return mix(horizon, zenith, t);
}
```

Formula: `gradient = y^power`

Similar but we use `y` directly rather than `1 - |y|`. Our formula is simpler but may not concentrate at horizon as effectively.

---

## What We Can Learn from DMSkybox

### 1. View Direction is Critical
DMSkybox avoids view direction reconstruction entirely by using geometry-based skybox. Our approach REQUIRES correct `inv_view_proj` matrix.

**Action:** Verify `inv_view_proj` is being computed and passed correctly.

### 2. Separation of Concerns
DMSkybox renders each element independently:
- Stars: separate pass with cubemap
- Atmosphere: separate pass with additive blend
- Moons: physical meshes

We try to do everything in one pass, which is more efficient but harder to debug.

**Action:** Consider temporarily rendering just the moon disc (no glow, no atmosphere) to isolate the issue.

### 3. Direction Passing
DMSkybox passes sun direction via `Shader.SetGlobalVector` with `.xyz` for direction and `.w` for intensity. This matches our approach but they also **negate** the direction:

```hlsl
float3 SunDirection1070 = -appendResult1071;  // Note: negated!
```

**Action:** Check if our moon_dir needs negation. The direction TO the moon vs direction FROM the moon matters.

### 4. Horizon Glow Technique
DMSkybox computes horizon glow based on `dot(-SunDir, ViewDir)`:
```hlsl
float InvVDotL200 = dot(-SunDirection1070, ase_worldViewDir);
```

This creates a glow TOWARD the sun. We could use similar for moon glow.

---

## Recommended Debugging Steps

Based on DMSkybox analysis, our issue is likely in **view direction or moon direction**:

### Step 1: Verify View Direction
Output `view_dir * 0.5 + 0.5` and confirm:
- At screen center, view_dir should equal camera forward direction
- At screen edges, view_dir should vary smoothly

### Step 2: Verify Moon Direction Sign
Check if moon_dir should be negated:
- `moon_dir` = direction TO moon (from camera) - what we want for dot product
- If it's direction OF moon's motion or FROM moon, we need to negate

### Step 3: Test with Fixed Directions
Hardcode `moon_dir = vec3(0, 1, 0)` (directly up) and verify:
- Looking up should show green (inside moon)
- Looking horizontal should show red (angle > moon_size)

### Step 4: Consider Mesh-Based Approach
If procedural continues to fail, consider hybrid:
- Render sky gradient procedurally
- Use actual 3D sphere for moons (like DMSkybox)
- Bevy has `Mesh::from(shape::Icosphere)` 

---

## File Reference

| DMSkybox File | Purpose | Our Equivalent |
|--------------|---------|----------------|
| `SkyboxAtmosphere.shader` | Sky gradient, horizon glow | `sky_dome.wgsl` |
| `SkyboxPlanet.shader` | Moon rendering (on mesh) | `render_moon()` in sky_dome.wgsl |
| `SkyboxStars.shader` | Star field | `compute_stars()` in sky_dome.wgsl |
| `SimpleTimeController.cs` | Time + shader uniforms | `sky_dome_node.rs` |

---

## Conclusion

DMSkybox's physical-mesh approach sidesteps the exact problem we're debugging: correct procedural celestial body rendering. Their approach is:
- Easier to implement
- Harder to break
- Less efficient (multiple draw calls)
- Less flexible (need mesh assets)

Our procedural approach is more ambitious and efficient but requires:
- Perfect view direction reconstruction
- Correct direction uniform passing
- Manual implementation of all visual effects

**The bug is almost certainly in how view_dir or moon_dir is computed/passed, not in the rendering math itself.**
