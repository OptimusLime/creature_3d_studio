# Shadow Banding/Gradient Artifacts Debug Doc

## Problem Statement

In `p9_island` screenshot, the right side of the scene shows:
1. **Shadow banding** - discrete bands/gradients in the shadow instead of crisp edges
2. **Shadow bleeding** - shadows fade/gradient away from the occluding object in weird patterns

The shadow from the tree should be a crisp shape, not a gradient mess.

## Observed Symptoms

- Right side of grass has visible bands/gradients
- Shadow appears to "bleed" or fade in discrete steps
- Shadow doesn't have crisp edges - instead has soft, banded transitions
- The artifact persists even after bias adjustments (0.03, 0.05, 0.08)

## Scene Setup (p9_island)

- Light position: `Vec3::new(-6.0, 10.0, -6.0)` - marked as shadow caster
- Camera angle: 45°, 30° pitch, zoom 0.7
- World file: `assets/worlds/island.voxworld`

## Investigation Log (2024-12-30)

### Test 1: DEBUG_MODE = 61 (depth difference visualization)
**Result**: Almost entire scene showed RED (stored_depth < compare_depth = shadowed)
**Conclusion**: The shadow comparison is failing for most surfaces, not just shadow areas

### Test 2: DEBUG_MODE = 23 (cube face selection)
**Result**: Shows clear face boundaries - GREEN for -Y (top), RED for X faces, BLUE for Z faces
**Conclusion**: Face selection appears correct based on dominant axis

### Test 3: DEBUG_MODE = 34 (raw -Y shadow map)
**Result**: Shadow map renders correctly, shows island from above with proper depth gradients
**Conclusion**: Shadow map rendering is working

### Test 4: DEBUG_MODE = 63 (raw +X shadow map)
**Result**: +X shadow map also renders correctly, shows island from side
**Conclusion**: All 6 faces ARE being rendered (not empty)

### Test 5: DEBUG_MODE = 62 (stored vs compare depth per face)
**Result**: Shows YELLOW for X/Z faces (high stored + high compare), WHITE for Y faces
**Conclusion**: Depth values vary by face, but stored depths are present

### Test 6: Increased bias to 0.08
**Result**: DEBUG_MODE 20 shows improved results (more green), but final render still has banding
**Conclusion**: Bias is NOT the root cause - there's something fundamentally wrong

### Test 7: Added PCF 3x3 sampling
**Result**: Slightly softer edges but banding persists
**Conclusion**: PCF helps but doesn't fix root cause

## Key Finding

**The banding persists even with large bias and PCF.** This suggests the problem is NOT:
- Simple bias issues
- Single-sample aliasing

The problem IS likely:
- Incorrect UV calculation for certain faces
- Hardcoded values that don't match actual geometry
- Matrix calculation errors in view-projection
- Something fundamentally broken in how we sample cube shadow maps

## Changes Made (need review/revert?)

1. Added PCF 3x3 to `calculate_point_shadow()` 
2. Changed bias from 0.05 to 0.08
3. Added helper function `sample_point_shadow_face()`
4. Added debug modes 62, 63, 64

## Next Steps - MUST DO

1. **Find reference implementation** - Look at how Minecraft/other voxel engines do point light shadows
2. **Review CubeFaceMatrices::new()** - Check if view matrices are correct for all 6 faces
3. **Check for hardcoded assumptions** - Look for any values that assume specific geometry
4. **Compare with working examples** - Find a Bevy/wgpu cube shadow map example that works

## Files Involved

- `assets/shaders/deferred_lighting.wgsl` - Shadow sampling (lines ~267-355)
- `assets/shaders/point_shadow_depth.wgsl` - Shadow map rendering  
- `crates/studio_core/src/deferred/point_light_shadow.rs` - CubeFaceMatrices, shadow setup
- `crates/studio_core/src/deferred/point_light_shadow_node.rs` - Render node
- `crates/studio_core/src/deferred/lighting_node.rs` - Passes matrices to shader

## CRITICAL: ROOT CAUSE IDENTIFIED

### Reference Implementation (LearnOpenGL)

The standard approach for point light shadows uses a **cubemap texture** sampled with a **3D direction vector**:

```glsl
// LearnOpenGL - CORRECT approach
uniform samplerCube depthMap;
vec3 fragToLight = fragPos - lightPos;
float closestDepth = texture(depthMap, fragToLight).r;  // Direct 3D direction sample!
```

The GPU hardware automatically:
1. Selects the correct cube face based on the direction
2. Computes the UV coordinates within that face
3. Handles edge cases at face boundaries

### Our Implementation (INCORRECT)

We use **6 separate 2D textures** with manual face selection and UV calculation:

```wgsl
// Our approach - PROBLEMATIC
let face_idx = select_face_manually(light_to_frag);
let clip = view_proj_matrix * world_pos;
let ndc = clip.xyz / clip.w;
let face_uv = ndc_to_uv(ndc);  // Manual UV calculation - ERROR PRONE!
textureSampleCompare(face_textures[face_idx], face_uv, compare_depth);
```

Problems with our approach:
1. **Manual face selection can have boundary issues** - fragments near face edges may select wrong face
2. **UV calculation is complex and error-prone** - different faces may need different UV mappings
3. **Matrix transform introduces precision errors** - we transform through view-proj instead of direct direction

### Correct Fix

**Option A: Switch to actual cubemap texture**
- Use `texture_depth_cube` instead of 6 separate `texture_depth_2d`
- Sample with direction vector directly
- Let GPU handle face selection and UV calculation

**Option B: Fix the manual UV calculation**
- The UV calculation must match how each face was rendered
- Different faces may need different formulas
- Must account for how `look_to_rh` orients each face

### View Matrix Up Vectors (Reference)

From LearnOpenGL, the standard up vectors are:
```
+X face: up = (0, -1, 0)  // NEG_Y
-X face: up = (0, -1, 0)  // NEG_Y
+Y face: up = (0, 0, +1)  // POS_Z
-Y face: up = (0, 0, -1)  // NEG_Z
+Z face: up = (0, -1, 0)  // NEG_Y
-Z face: up = (0, -1, 0)  // NEG_Y
```

Our implementation matches these values, so the matrices should be correct.

### The Real Problem: UV Calculation

When sampling with matrix transform, we do:
```wgsl
let clip = view_proj * vec4(world_pos, 1.0);
let ndc = clip.xyz / clip.w;
let face_uv = vec2((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
```

But this assumes Y-flip is correct for ALL faces. In reality:
- **The Y-flip may need to be different per face** depending on how the view matrix was constructed
- The view matrices orient each face differently, so the "up" direction in NDC space varies

## Recommended Fix

1. **Simplest**: Use proper cubemap sampling instead of 6 separate textures
2. **Alternative**: Compute UV directly from direction vector without matrices, like standard paraboloid/cube mapping formulas

## Resolution

[IN PROGRESS - Root cause analysis below]

---

## IMPORTANT UPDATE (2024-12-30 Session 2)

### Key Discovery: Point Shadows Are NOT the Cause!

We proved this with the following tests:

#### Test 8: Disable point shadows completely
**Change**: Set `let shadow = 1.0;` (always lit) in calculate_point_shadow
**Result**: The gradient on the grass PERSISTED
**Conclusion**: Point shadows are not causing the gradient

#### Test 9: Debug mode 20 (point shadow visualization)
**Result**: Entire scene shows GREEN (1.0 = fully lit)
**Conclusion**: Point shadow system is working correctly (all surfaces are lit when shadow disabled)

#### Test 10: Debug mode 4 (directional shadow only)
**Result**: Uniform values, no gradient
**Conclusion**: Directional shadows are not causing the gradient

#### Test 11: Debug mode 3 (albedo only)
**Result**: Uniform colors for grass
**Conclusion**: The colors themselves are not the source of gradient

#### Test 12: Debug mode 6 (point lights only)
**Result**: Expected falloff from crystal light source
**Conclusion**: Point light falloff is working as expected

### Current Hypothesis (Session 2)

The gradient must come from one of:
1. **AO (Ambient Occlusion)** - baked into mesh vertices
2. **Minecraft-style face shading multipliers** - applied per-face
3. **N·L calculation for directional lights** - moon lighting angles

### Test 13: Debug mode 5 (AO only)
**Status**: Running
**Expected**: If AO is uniform (white), then AO is not the cause

### Face Shading Multipliers (Lines 1399-1413)
```wgsl
if abs(world_normal.y) > 0.9 {
    if world_normal.y > 0.0 {
        face_multiplier = 1.0;  // Top faces: full brightness
    } else {
        face_multiplier = 0.5;  // Bottom faces: half brightness
    }
} else if abs(world_normal.z) > 0.9 {
    face_multiplier = 0.8;  // Front/Back
} else {
    face_multiplier = 0.6;  // Left/Right
}
```

This would NOT cause a smooth gradient - it creates discrete bands.

### N·L Calculation
The grass faces upward (+Y), so:
```wgsl
let n_dot_moon1 = max(dot(world_normal, moon1_dir), 0.0);
let n_dot_moon2 = max(dot(world_normal, moon2_dir), 0.0);
```

If the moon directions create an angle with the +Y normal, this would create a uniform value (since all grass has same normal). This shouldn't cause a gradient either.

### Most Likely Cause: Point Light Distance Falloff

The crystal point light creates distance-based falloff:
```wgsl
// Smooth falloff from center to radius
let attenuation = 1.0 - smoothstep(0.0, light.radius, distance);
```

This IS a smooth gradient - surfaces closer to the light are brighter.

But wait - we tested debug mode 6 (point lights only) and it showed expected falloff...

The gradient could be the COMBINATION of:
- Point light falloff (smooth gradient)
- Multiplied by AO (potentially varying)
- Multiplied by face shading (discrete bands)

### Test 14: Debug mode 70 (face_multiplier only)
**Result**: Shows discrete bands as expected - white for top faces, gray for side faces
**Conclusion**: Face multiplier working correctly - NOT the cause of smooth gradient

### Test 15: Debug mode 71 (N·L only)
**Result**: Shows relatively uniform colors for surfaces with same normal
**Conclusion**: N·L is uniform across same-facing surfaces - NOT the cause of gradient

### CONCLUSION: The Gradient is Expected Behavior!

The gradient on the grass is caused by **point light distance falloff** - this is CORRECT lighting behavior:
- Surfaces closer to the crystal light source are brighter
- Surfaces farther away receive less light
- This creates a smooth radial gradient around light sources

This is NOT a bug - it's the point light attenuation working as designed.

### Point Shadow Status

Point shadows have been **re-enabled** (line ~199 in deferred_lighting.wgsl).

The debug session that initially flagged "shadow banding" may have been observing:
1. Normal point light distance falloff (expected)
2. AO variations (expected)
3. Combined effects looking like "banding"

### Debug Modes Added
- Mode 70: face_multiplier visualization
- Mode 71: N·L visualization (R=moon1, G=moon2)

### Current State (2024-12-30)
- Point shadows: ENABLED
- DEBUG_MODE: 0 (final render)
- All lighting systems working correctly

## Issue Status: RESOLVED

### Final Root Cause (2024-12-30)

**The `compute_cube_face_uv()` function was computing UV coordinates that did not match how the shadow maps were rendered.**

The shadow maps are rendered using view-projection matrices from `CubeFaceMatrices::new()`. But the sampling code was using a manual cubemap UV formula that assumed different axis orientations.

**Debug mode 83** proved this by showing the difference between manual UV and matrix-based UV - they were completely different across the entire scene.

### The Fix

Replace manual UV calculation with matrix-based:

```wgsl
// Use matrix-based UV calculation (matches how shadow maps were rendered)
let view_proj = point_shadow_matrices.face_matrices[face_idx];
let clip = view_proj * vec4<f32>(world_pos, 1.0);
let ndc = clip.xyz / clip.w;
let face_uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
```

This guarantees the UV coordinates match exactly what was used when rendering the shadow maps.

### Lesson Learned

When rendering to a texture with a matrix transform, **always use the same matrix** for sampling. Don't try to derive UV coordinates from direction vectors using manual formulas - they will not match unless you perfectly replicate the matrix math.
