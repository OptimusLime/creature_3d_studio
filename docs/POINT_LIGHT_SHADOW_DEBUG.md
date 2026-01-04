# Point Light Shadow Debug Plan

## Current Problem

**Symptom**: Shadows render but are DETACHED from pillar bases. There's a visible gap between where shadows appear and where they should connect to the objects casting them.

## Latest Hypothesis (Dec 29, 2024)

### Bonsai Shadow Approach Analysis

After researching Bonsai's shadow implementation, key findings:

**1. Bonsai uses ONLY directional (sun) shadows, NOT point light shadows**

From `bonsai/src/engine/light.h:104-107`:
```cpp
#define SHADOW_MAP_X 1024*4
#define SHADOW_MAP_Y 1024*4
#define SHADOW_MAP_Z_MIN -1024*4
#define SHADOW_MAP_Z_MAX  1024*4
```

This defines an ORTHOGRAPHIC projection for directional light shadows, not perspective for point lights.

**2. Bonsai's shadow sampling uses ShadowMVP matrix directly**

From `bonsai/shaders/Lighting.fragmentshader:249-256`:
```glsl
v4 FragPShadowSpace = ShadowMVP * vec4(FragWorldP, 1.f);
f32 FragDepth = FragPShadowSpace.z - acneBias;
float ShadowSampleDepth = texture(shadowMap, FragPShadowSpace.xy).x;
if (FragDepth > ShadowSampleDepth) { ShadowVisibility -= vec3(1.f); }
```

**KEY INSIGHT**: Bonsai passes the full `ShadowMVP` matrix to the shader and uses it directly. It does NOT manually compute UV formulas - it transforms world position through the matrix and uses the resulting XY as texture coordinates.

**3. Bonsai's point lights have NO shadow support**

From `bonsai/shaders/Lighting.fragmentshader:182-210`:
```glsl
for (s32 LightIndex = 0; LightIndex < LightCount; ++LightIndex) {
    vec3 LightPosition = texelFetch(LightPositions, LightUV, 0).rgb;
    vec3 LightColor = texelFetch(LightColors, LightUV, 0).rgb;
    // ... simple distance attenuation, NO shadow lookup
}
```

Point lights in Bonsai only do distance falloff - no shadow maps consulted.

### What This Means For Us

We're implementing something Bonsai doesn't have: **point light cube shadow maps**. This is more complex because:

1. Point light needs 6 faces (cubemap) vs 1 face for directional
2. Each face has its own view-projection matrix
3. We need to select the correct face based on light-to-fragment direction
4. THEN sample that face's shadow map

### Root Cause Hypothesis

**The problem is likely that our UV formula derivation is correct mathematically, but there's a mismatch in coordinate systems:**

1. **Texture origin**: In some systems UV (0,0) is top-left, in others bottom-left
2. **NDC handedness**: Our view matrices use look_to_rh (right-handed), but wgpu NDC might expect different
3. **Depth comparison direction**: We use `LessEqual` but should verify this matches our depth output

### Recommended Fix: Use View-Proj Matrix Directly (Like Bonsai)

Instead of manually deriving UV formulas, pass the actual view-projection matrices to the shader:

```rust
// In lighting_node.rs - add face matrices to uniform
struct PointShadowUniforms {
    face_view_proj: [Mat4; 6],  // One for each face
    light_pos: Vec3,
    radius: f32,
}
```

```wgsl
// In deferred_lighting.wgsl - use matrix directly
fn calculate_point_shadow(...) -> f32 {
    let light_to_frag = world_pos - light_position;
    let face_index = select_cube_face(light_to_frag);
    
    // Use the ACTUAL matrix, not derived formula
    let clip = face_view_proj[face_index] * vec4(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    
    // Standard NDC to UV conversion
    let face_uv = vec2((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
    
    let compare_depth = ndc.z;  // or distance/radius, match shadow pass output
    return textureSampleCompare(shadow_face[face_index], sampler, face_uv, compare_depth);
}
```

This eliminates ALL manual UV formula derivation and uses the exact same transform as the render pass - if it still fails, the bug is elsewhere (depth format, texture binding, etc).

---

## Session Summary (Dec 29, 2024)

### What Was Verified

1. **G-buffer world positions are CORRECT**
   - Debug mode 51 showed markers at expected world positions
   - Pillar 1 base at (-1.5, 0, 2.5) - cyan marker appeared correctly
   - Test shadow point at (2.4, 0, 2.4) - magenta marker appeared correctly

2. **Shadow map IS being written correctly**
   - Debug mode 60 showed pillar silhouettes in shadow map
   - Depth values correct: pillars < 0.26, ground ~0.3

3. **Pillar locations in shadow map IDENTIFIED**
   - Debug mode 67: Placed marker at calculated UV (0.285, 0.855)
   - Marker landed directly on pillar 1 silhouette in shadow map
   - This proves the view matrix math derivation is correct

### Key Finding: Correct UV Formula for -Y Face

From manual derivation of `look_to_rh(light_pos, -Y, -Z)`:
- Right = +X, Up = -Z, Forward = -Y
- view = (ltf.x, -ltf.z, ltf.y)
- clip.w = -view.z = -ltf.y = abs_y (since ltf.y < 0)
- ndc = (view.x/clip.w, view.y/clip.w) = (ltf.x/abs_y, -ltf.z/abs_y)
- UV = ((ndc.x+1)/2, (1-ndc.y)/2) = **(-ltf.x/abs_y*0.5+0.5, ltf.z/abs_y*0.5+0.5)**

**BUT**: This formula (`U = -X/abs_y, V = Z/abs_y`) still produces detached shadows when tested.

### Unresolved Mystery

The UV formula matches where pillars appear in shadow map (verified with markers), yet:
- When applied in sampling, shadows appear offset
- All 8 combinations of flip/swap tried, none work
- This suggests the issue is NOT in UV calculation

---

## Test Scene

```
Ground: 16x16 at Y=0, chunk coords (8-24, 0, 8-24) -> world (-8 to +8, 0, -8 to +8)
Pillar 1: chunk (14-15, 1-4, 18-19) -> world (-2 to -1, 1-4, 2-3)
Pillar 2: chunk (18-19, 1-2, 12-13) -> world (2-3, 1-2, -4 to -3)
Light: (0, 6, 0), radius 20
Mesh transform: (0, 16, 0)
```

## Success Criteria

Shadow is "correct" when:
1. Shadow edge touches pillar base (no gap)
2. Shadow extends away from light
3. Shadow shape roughly matches pillar cross-section

## Next Steps (Priority Order)

1. **Pass view-proj matrices to shader** - Use exact same transform as render pass
2. **Verify depth comparison** - Ensure shadow depth output format matches compare input
3. **Check texture coordinate origin** - Verify UV (0,0) convention in wgpu

## Status

- [x] Verified G-buffer world positions correct
- [x] Verified shadow map stores pillar silhouettes correctly
- [x] Derived correct UV formula from view matrix
- [x] Verified formula matches shadow map locations
- [x] Researched Bonsai shadow approach (directional only, uses matrix directly)
- [ ] **NEXT**: Pass view-proj matrices to lighting shader
- [ ] Investigate texture coordinate origin
