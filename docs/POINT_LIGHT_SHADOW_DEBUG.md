# Point Light Shadow Debug Plan

## Current Problem

**Symptom**: Shadows render but are DETACHED from pillar bases. There's a visible gap between where shadows appear and where they should connect to the objects casting them.

**Visual Evidence**: In `screenshots/p13_point_light_shadow.png`, both pillars cast shadows that extend in the correct direction (away from center light), but the shadows float ~1-2 units away from the pillar bases instead of touching them.

## What We Know Works

1. Shadow textures ARE being written (debug mode 34 shows pillar shapes in shadow map)
2. Light position is consistent: (0, 6, 0) in both render and sample paths
3. Mesh transform is consistent: translation (0, 16, 0) in both paths
4. Comparison sampler direction is correct (LessEqual)
5. Render graph order is correct (shadow pass runs before lighting)

## What We Know Is Wrong

1. Shadows appear at WRONG UV positions relative to pillar bases
2. The offset is consistent - shadows shift away from scene center
3. All 4 UV flip combinations (none, U, V, both) produce wrong results

## Root Cause Hypotheses

### Hypothesis A: Texture Y-Flip Convention
**Theory**: wgpu/Metal texture coordinate origin may differ from what we assume.

**Evidence for**: Different flip combinations move shadows to different positions.
**Evidence against**: None of the 4 flip combinations produce correct results.

**Test**: Render a known pattern (gradient or crosshair) to shadow map, read it back with known UV, verify orientation.

### Hypothesis B: World Position Mismatch Between Passes
**Theory**: The world position in G-buffer (used for shadow sampling) differs from world position used in shadow render.

**Evidence for**: Both use same mesh transform, but computed in different shader passes.
**Evidence against**: Debug mode 33 shows light_to_frag looks correct.

**Test**: Output world position from BOTH shaders, compare visually.

### Hypothesis C: Perspective Projection Math Error
**Theory**: The simplified UV formula `x/abs_y * 0.5 + 0.5` doesn't correctly match the perspective projection used in rendering.

**Evidence for**: Math derivation looked correct but shadows are still wrong.
**Evidence against**: Manual calculation for specific points seemed to match.

**Test**: Use the ACTUAL view-projection matrix in the lighting shader instead of simplified formula.

---

## Debug Plan - Ordered by Simplicity

### Step 1: Verify Shadow Map Orientation (30 min)

**Goal**: Confirm which corner of shadow map is which in world space.

**Method**: 
1. In shadow depth shader, output a gradient based on clip position instead of depth
2. Or: Place a single test voxel at known world position, see where it appears in shadow map

**Implementation**:
```wgsl
// In point_shadow_depth.wgsl, temporarily replace fs_main:
@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    // Output clip.x as depth (will show as gradient)
    out.depth = (in.clip_position.x / 512.0);  // 0-1 across texture
    return out;
}
```

**Expected Result**: Shadow map should show horizontal gradient. Left edge = dark, right edge = bright.

**What It Tells Us**: If gradient goes wrong direction, we know U is flipped. Do same for V with clip_position.y.

---

### Step 2: Trace Single Point Through Both Paths (30 min)

**Goal**: Pick ONE world point and trace its UV through both render and sample.

**Method**: 
1. Place a single bright voxel at exact known position (e.g., world (2, 1, 2))
2. Log its clip coords in shadow depth shader
3. Log the UV we'd sample for a ground point directly below it

**Implementation**:

In Rust (point_light_shadow.rs), add to CubeFaceMatrices::new():
```rust
// Test point: world (2, 1, 2) - this should cast shadow on ground at ~(3.4, 0, 3.4)
let test_world = Vec3::new(2.0, 1.0, 2.0);
let clip = view_proj[3] * Vec4::new(test_world.x, test_world.y, test_world.z, 1.0);
let ndc = clip / clip.w;
let tex_u = (ndc.x + 1.0) / 2.0;
let tex_v = (1.0 - ndc.y) / 2.0;  // Standard Y flip
info!("Test voxel (2,1,2) renders to UV ({}, {})", tex_u, tex_v);

// Ground point that should be in shadow: ray from (0,6,0) through (2,1,2) to y=0
// Direction: (2, -5, 2), t = 6/5 = 1.2 to reach y=0
// Ground point: (2*1.2, 0, 2*1.2) = (2.4, 0, 2.4)
let shadow_ground = Vec3::new(2.4, 0.0, 2.4);
let ltf = shadow_ground - light_pos; // (2.4, -6, 2.4)
let sample_u = ltf.x / ltf.y.abs() * 0.5 + 0.5;
let sample_v = ltf.z / ltf.y.abs() * 0.5 + 0.5;
info!("Ground point (2.4, 0, 2.4) samples from UV ({}, {})", sample_u, sample_v);
info!("These should MATCH if UV formula is correct!");
```

**Expected Result**: Both UVs should be identical (or very close).

**What It Tells Us**: If they differ, the UV formula is wrong. The difference tells us exactly how to fix it.

---

### Step 3: Pass View-Proj Matrix to Lighting Shader (1 hour)

**Goal**: Use exact same projection in sampling as in rendering - eliminate formula as variable.

**Method**:
1. Add the -Y face view-proj matrix to lighting shader uniforms
2. Transform world_pos through it to get clip coords
3. Use clip.xy / clip.w for UV instead of simplified formula

**Implementation**:

In lighting_node.rs, add to point shadow bind group:
```rust
// Add view_proj matrix for face 3 (-Y) to uniforms
struct PointShadowSampleUniforms {
    face_view_proj: [[f32; 4]; 4],  // -Y face view-proj
    light_pos: [f32; 4],
}
```

In deferred_lighting.wgsl:
```wgsl
// Instead of simplified formula:
let clip = point_shadow_uniforms.face_view_proj * vec4<f32>(world_pos, 1.0);
let ndc = clip.xy / clip.w;
let face_uv = vec2<f32>(
    (ndc.x + 1.0) * 0.5,
    (1.0 - ndc.y) * 0.5  // Try with and without Y flip
);
```

**Expected Result**: Shadows should align with pillar bases.

**What It Tells Us**: If this works, the simplified formula was wrong. If still broken, issue is elsewhere (texture coords, render setup, etc.)

---

### Step 4: Verify G-Buffer World Position (30 min)

**Goal**: Confirm world positions in G-buffer match expected values.

**Method**: Debug mode that shows world position for pixels where we expect shadow.

**Implementation**:
```wgsl
// Debug mode: Show world pos for area where shadow SHOULD be
if DEBUG_MODE == 50 {
    // Shadow from pillar at (-1.5, *, 2.5) should appear around (-2.5, 0, 4.3)
    let expected_shadow = vec3<f32>(-2.5, 0.0, 4.3);
    let dist = length(world_pos.xz - expected_shadow.xz);
    if dist < 0.5 {
        // Show actual world position here
        return vec4<f32>(
            (world_pos.x + 5.0) / 10.0,  // R = X
            (world_pos.z + 5.0) / 10.0,  // G = Z  
            world_pos.y / 5.0,            // B = Y
            1.0
        );
    }
    // ... normal rendering
}
```

**Expected Result**: The highlighted area should show world coords matching expected_shadow.

**What It Tells Us**: If world pos is wrong in G-buffer, that's our bug. If correct, issue is in shadow sampling.

---

## Simplest First Approach

**Do Step 2 first** - it's pure logging, no shader changes needed, and will immediately tell us if UV formula matches.

If Step 2 shows UVs match but shadows are still wrong, do Step 1 to verify texture orientation.

If Step 2 shows UVs DON'T match, the difference tells us exactly what's wrong with the formula.

---

## Test Scene Simplification

Current scene has too many variables. Create minimal test:

```rust
// In p13 example, replace scene with:
// 1. Single ground voxel at (0, 0, 0) - world origin
// 2. Single pillar voxel at (2, 1, 0) - easy math
// 3. Light at (0, 6, 0)
// 
// Expected shadow: pillar at (2, 1, 0) casts shadow to ~(2.4, 0, 0)
// This is on X axis only - simplest possible case
```

This eliminates Z component entirely, making math trivial to verify.

---

## Success Criteria

Shadow is "correct" when:
1. Shadow edge touches pillar base (no gap)
2. Shadow extends away from light
3. Shadow shape roughly matches pillar cross-section
4. Shadow fades/ends at light radius edge

## Current Status

- [x] Committed debug investigation code
- [ ] Step 1: Verify shadow map orientation
- [ ] Step 2: Trace single point through both paths  
- [ ] Step 3: Pass view-proj matrix to lighting shader
- [ ] Step 4: Verify G-buffer world position
