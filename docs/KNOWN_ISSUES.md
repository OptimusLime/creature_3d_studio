# Known Issues - Critical Bugs

This document tracks critical visual bugs that must be fixed before adding new features.

---

## Issue 1: Severe AO/Lighting Artifacts (BLACK TRIANGULAR PATCHES)

**Severity:** CRITICAL  
**Affected Examples:** p16_multi_chunk, p17_chunk_streaming, p18_cross_chunk_culling

### Symptoms
- Black triangular/diamond-shaped patches appear on voxel surfaces
- The artifacts follow a consistent diagonal pattern across the terrain
- Appears to be related to ambient occlusion interpolation
- Creates a "zebra stripe" or "checkerboard" effect on large flat surfaces
- Visible in p16 screenshot: diagonal black lines across the entire terrain
- Visible in p17 screenshot: same pattern, very prominent on the stepped terrain

### Comparison
- **p9_island**: Clean, no artifacts (single chunk, simple geometry)
- **p10_dark_world**: Clean, looks great (single chunk)
- **p13_point_light_shadow**: Clean shadows work correctly (simple geometry)
- **p16_multi_chunk**: BROKEN - severe artifacts
- **p17_chunk_streaming**: BROKEN - severe artifacts
- **p18_cross_chunk_culling**: Looks OK (simple geometry, fewer voxels)

### Hypothesis 1: Greedy Meshing AO Interpolation Bug
The greedy meshing algorithm merges multiple voxel faces into larger quads. When this happens, the AO values at each corner need to be interpolated correctly. 

**Possible cause:** The AO values are being calculated per-voxel but when faces are merged into larger quads, the interpolation across the quad creates incorrect gradients.

**Evidence:** 
- The artifacts appear as triangular patterns (triangles are how quads are rendered)
- The pattern is consistent and geometric, not random
- Simple scenes (p18) don't show the issue because quads are small

### Hypothesis 2: Cross-Chunk AO Boundary Issue
When calculating AO for voxels at chunk boundaries, we may be getting incorrect neighbor data.

**Possible cause:** The `extract_borders()` function might be providing incorrect occupancy data, or the AO calculation doesn't properly handle chunk edges.

**Evidence:**
- Multi-chunk scenes show the issue
- Single-chunk scenes (p9, p10) look fine

### Hypothesis 3: Normal/AO Mismatch in Greedy Quads
When greedy meshing combines faces, the normals are correct but AO corner values may be assigned in the wrong order.

**Possible cause:** The vertex winding order or AO corner assignment doesn't match the expected interpolation direction.

### Proposed Fix Strategy
1. **First:** Test with greedy meshing DISABLED on p16/p17 to isolate if it's greedy-specific
2. **If greedy is the cause:** Review `emit_greedy_quad_with_borders()` AO calculation
3. **If not greedy:** Review the basic per-face AO calculation in `calculate_corner_ao_cross_chunk()`
4. **Verify:** Check that AO values are in range [0,1] and corners are assigned consistently

---

## Issue 2: Point Light Shadows Not Working in Multi-Chunk Scenes

**Severity:** CRITICAL  
**Affected Examples:** p16_multi_chunk, p17_chunk_streaming

### Symptoms
- Point lights illuminate surfaces correctly
- BUT no shadows are cast by objects
- Compare p13 (shadows work) vs p16 (no visible shadows)
- The glowing pillars light up surrounding voxels but don't cast shadows

### Comparison
- **p13_point_light_shadow**: Shadows work perfectly - two cubes cast clear shadows on the floor
- **p16_multi_chunk**: NO shadows visible despite 116 point lights and many voxels

### Hypothesis 1: Shadow Map Resolution/Coverage Issue
The shadow map may not be large enough to cover multi-chunk worlds, or the projection is wrong.

**Evidence from logs:**
```
Shadow render using light at Vec3(48.5, 6.5, 48.5), radius 12
...
NDC values are WAY outside [-1,1] range: (-7.46, 7.46)
```
The NDC values being far outside the valid range means the shadow map isn't covering the geometry.

### Hypothesis 2: Shadow Map Only Renders Nearby Geometry
Looking at the code, point shadow rendering may only process geometry within a certain radius of the light, but the shadow sampling happens for all fragments.

**Possible cause:** The shadow depth texture is rendered from one light's perspective but we're sampling it for ALL lights.

### Hypothesis 3: Light Position vs World Scale Mismatch
In p13, the scene is small (within one chunk). In p16, the world spans coordinates 0-64. The shadow system may assume a smaller world.

**Evidence:**
```
Point shadow mesh transform: Mat4 { ... w_axis: Vec4(48.0, 16.0, 48.0, 1.0) }
```
The shadow mesh is being placed at (48, 16, 48) but the shadow calculation uses light at (48.5, 6.5, 48.5).

### Proposed Fix Strategy
1. **Debug:** Add visual debug to show where shadow map thinks occluders are
2. **Check:** Verify shadow map projection matrix covers the visible world
3. **Review:** The `point_light_shadow.rs` prepare/render logic for multi-light scenarios
4. **Test:** Create minimal repro - single light, two chunks, one shadow caster

---

## Issue 3: Chunk Streaming Distance Too Far

**Severity:** MEDIUM  
**Affected Examples:** p17_chunk_streaming

### Symptoms
- Camera starts very far from the terrain
- Can barely see the voxels
- The "dark fantasy" aesthetic is lost because everything is distant

### Current Values
From `chunk_streaming.rs`:
```rust
pub load_distance: f32,      // Default: 64.0 (4 chunks)
pub unload_distance: f32,    // Default: 96.0 (6 chunks)  
```

Camera position in p17:
```rust
transform: Transform::from_xyz(64.0, 80.0, 160.0).looking_at(Vec3::new(64.0, 0.0, 64.0), Vec3::Y),
```
Camera is at Y=80, looking at Y=0, from 160 units away in Z.

### Hypothesis
The example camera is placed too far back. The streaming distances are reasonable but the camera setup in p17 doesn't showcase the streaming well.

### Proposed Fix
1. Move camera closer: `Transform::from_xyz(64.0, 30.0, 90.0)`
2. Or adjust FOV to show more of the scene
3. The streaming distance parameters are likely fine

---

## Issue 4: Emissive "Light Beams" Rendering Issue

**Severity:** LOW (cosmetic)  
**Affected Examples:** p16_multi_chunk

### Symptoms
- Emissive voxels show strange elongated "beam" effects
- See p16 screenshot - the glowing pillars have horizontal light streaks

### Hypothesis
This might be bloom bleeding along certain axes, or an artifact of how emission is written to the G-buffer for merged quads.

### Proposed Fix
Lower priority - fix AO and shadows first.

---

## Priority Order

1. **AO Artifacts** - Most visible, affects all multi-chunk scenes
2. **Point Light Shadows** - Core visual feature not working
3. **Camera Distance** - Quick fix, improves examples
4. **Light Beams** - Cosmetic, can wait

---

## Testing Commands

```bash
# Run affected examples
cargo run --example p16_multi_chunk
cargo run --example p17_chunk_streaming
cargo run --example p18_cross_chunk_culling

# Run working examples for comparison
cargo run --example p9_island
cargo run --example p10_dark_world
cargo run --example p13_point_light_shadow

# Run tests
cargo test --package studio_core
```
