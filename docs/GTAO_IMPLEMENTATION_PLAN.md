# GTAO Implementation Plan

## Summary

Implement Intel's XeGTAO (Ground Truth Ambient Occlusion) algorithm for high-quality ambient occlusion in our deferred rendering pipeline.

## Context & Motivation

We need ambient occlusion to add depth and realism to our voxel scenes. XeGTAO provides:
- Ground-truth quality AO (better than traditional SSAO)
- Excellent performance (designed for real-time)
- No banding artifacts (common in hemisphere SSAO)
- Well-documented reference implementation

Reference: https://github.com/GameTechDev/XeGTAO

---

## CRITICAL DISCOVERY: Bevy Reverse-Z Depth Buffer

**Bevy uses INFINITE REVERSE-Z projection!**

This was the root cause of GTAO not working initially. The standard XeGTAO depth linearization formula does NOT work with Bevy's projection.

### Bevy's Projection Matrix Structure

```
col0: [1.81066, 0.0, 0.0, 0.0]
col1: [0.0, 2.4142134, 0.0, 0.0]
col2: [0.0, 0.0, 0.0, -1.0]      // Note: [2][2]=0, [2][3]=-1
col3: [0.0, 0.0, 0.1, 0.0]       // near plane = 0.1
```

### Depth Buffer Behavior
- **Near plane (z=0.1)**: NDC depth = 1.0
- **Far plane (z=inf)**: NDC depth = 0.0
- Objects at z=40 have NDC depth ~0.0025

### Correct Linearization Formula
```wgsl
// For Bevy infinite reverse-Z:
fn screen_space_to_viewspace_depth(screen_depth: f32) -> f32 {
    let near = 0.1;  // From projection matrix col3[2]
    return near / (0.0001 + screen_depth);  // Small epsilon prevents div by zero
}
```

### XeGTAO's Original Formula (DOES NOT WORK WITH BEVY)
```wgsl
// XeGTAO expects standard depth:
return depthLinearizeMul / (depthLinearizeAdd - screenDepth);
// This produces near-zero values with Bevy's reverse-Z!
```

---

## Current Status

- [x] Phase 0: Pipeline setup
- [x] Phase 1: View-space position reconstruction
- [x] Phase 2: View-space normal reconstruction  
- [x] Phase 3: Single slice horizon search
- [x] Phase 4: Multi-slice integration
- [x] **CRITICAL FIX: Reverse-Z depth buffer handling**
- [x] **CRITICAL FIX: Full XeGTAO algorithm alignment** (see audit below)
- [ ] Phase 5: Final polish (proper falloff, snap to pixel center)
- [x] Phase 6: Spatial denoise (7x7 bilateral blur in deferred_lighting.wgsl)

---

## XeGTAO Algorithm Alignment Audit

### Audit Date: 2024-12-30

Systematic comparison of our implementation against XeGTAO reference (`XeGTAO/Source/Rendering/Shaders/XeGTAO.hlsli`).

| # | Component | XeGTAO Line | Our Line | Status |
|---|-----------|-------------|----------|--------|
| 1 | R1 quasi-random noise sequence | L419-421 | L271-274 | ✅ ALIGNED |
| 2 | minS calculation (avoid center pixel) | L335,367 | L217-218 | ✅ ALIGNED |
| 3 | Small radius fade | L342-343 | L214 | ✅ ALIGNED |
| 4 | s += minS | L430 | L284 | ✅ ALIGNED |
| 5 | Slice angle formula (sliceK * PI) | L372-376 | L229-232 | ✅ ALIGNED |
| 6 | omega sign (cosPhi, -sinPhi) | L377 | L235 | ✅ ALIGNED |
| 7 | directionVec | L383 | L238 | ✅ ALIGNED |
| 8 | orthoDirectionVec (3D projection) | L386 | L241 | ✅ ALIGNED |
| 9 | axisVec (cross product) | L390 | L244 | ✅ ALIGNED |
| 10 | projectedNormalVec | L396 | L247 | ✅ ALIGNED |
| 11 | signNorm | L399 | L250 | ✅ ALIGNED |
| 12 | cosNorm (saturate, div by length) | L403 | L254 | ✅ ALIGNED |
| 13 | n angle (signNorm * acos) | L406 | L257 | ✅ ALIGNED |
| 14 | lowHorizonCos (cos(n±PI/2)) | L409-410 | L260 | ✅ ALIGNED |
| 15 | horizonCos initialization | L413-414 | L263-264 | ✅ ALIGNED |
| 16 | sampleHorizonVec | L472 | L304 | ✅ ALIGNED |
| 17 | shc = dot(horizonVec, viewVec) | L488 | L307 | ✅ ALIGNED |
| 18 | shc lerp with falloff weight | L492 | L310 | ✅ ALIGNED |
| 19 | horizonCos max update | L505 | L312 | ✅ ALIGNED |
| 20 | projNormalLen fudge (lerp to 1, 0.05) | L532 | L341 | ✅ ALIGNED |
| 21 | h0 = -acos(horizonCos1) | L536 | L344 | ✅ ALIGNED |
| 22 | h1 = acos(horizonCos0) | L537 | L345 | ✅ ALIGNED |
| 23 | iarc formula (cosNorm, 2h*sin, cos(2h-n)) | L542-543 | L352-353 | ✅ ALIGNED |
| 24 | localVisibility = projLen * (iarc0+iarc1) | L544 | L356 | ✅ ALIGNED |
| 25 | visibility /= sliceCount | L556 | L361 | ✅ ALIGNED |
| 26 | pow(visibility, finalPower) | L557 | L364 | ✅ ALIGNED |
| 27 | max(0.03, visibility) | L558 | L367 | ✅ ALIGNED |

### Known Differences (Lower Priority)

| # | Component | XeGTAO | Ours | Impact |
|---|-----------|--------|------|--------|
| A | Falloff calculation | Precomputed falloffMul/falloffAdd (L315-316, L477) | Simplified `1 - dist/radius` | Minor - affects edge softness |
| B | Snap to pixel center | round(sampleOffset) (L442) | No rounding | Minor - may cause sub-pixel noise |
| C | Depth MIP chain | Uses MIP levels for large radii (L438) | Single-level sampling | Minor - affects large radius performance |

---

## Directory Structure

```
assets/shaders/
├── gtao.wgsl              # Main GTAO shader (horizon-based)

crates/studio_core/src/deferred/
├── gtao.rs                # GTAO config and texture resources
├── gtao_node.rs           # GTAO render node and pipeline

examples/
├── p20_gtao_test.rs       # Test harness

assets/worlds/
├── gtao_test.voxworld     # Test scene geometry

XeGTAO/                    # Reference implementation (local copy)
├── Source/Rendering/Shaders/XeGTAO.hlsli
```

---

## Bind Group Layout

### Group 0: G-Buffer
- binding 0: g_normal (texture_2d<f32>)
- binding 1: g_position (texture_2d<f32>) - for sky detection
- binding 2: gbuffer_sampler (sampler)
- binding 3: depth_texture (texture_depth_2d) - **HARDWARE DEPTH BUFFER**

### Group 1: Noise
- binding 0: noise_texture (texture_2d<f32>)
- binding 1: noise_sampler (sampler)

### Group 2: Camera Uniforms
```rust
pub struct GtaoCameraUniform {
    view: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    inv_projection: [[f32; 4]; 4],
    screen_size: [f32; 4],
    // Packed vec4s for alignment:
    depth_unpack_and_ndc_mul: [f32; 4],  // xy=depth_unpack, zw=ndc_to_view_mul
    ndc_add_and_params1: [f32; 4],       // xy=ndc_to_view_add, z=effect_radius, w=falloff
    params2: [f32; 4],                   // x=radius_mul, y=final_power, z=sample_dist, w=thin_comp
}
```

---

## Debug Modes

In `gtao.wgsl`:
- `DEBUG_GTAO = 0`: Normal GTAO output
- `DEBUG_GTAO = 1`: Raw NDC depth (×100 for visibility)
- `DEBUG_GTAO = 2`: View-space normal.z
- `DEBUG_GTAO = 3`: Linear view-space depth (/50 for visibility)

In `deferred_lighting.wgsl`:
- `DEBUG_MODE = 0`: Full lighting with GTAO
- `DEBUG_MODE = 100`: Raw GTAO texture (with blur)
- `DEBUG_MODE = 101`: Raw GTAO center sample (no blur)

---

## Parameters

```wgsl
const SLICE_COUNT: i32 = 9;       // Direction slices
const STEPS_PER_SLICE: i32 = 4;   // Steps per direction
// Total: 9 slices × 4 steps × 2 directions = 72 samples
```

```rust
effect_radius: 3.0,  // World units (voxels are 1 unit)
```

---

## Next Steps

1. **Implement proper falloff** - Match XeGTAO's falloffMul/falloffAdd precomputation
2. **Add snap to pixel center** - Round sample offsets for cleaner sampling
3. **Full re-audit** - Verify all items after fixes
