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
- [x] **CRITICAL FIX: Full XeGTAO algorithm alignment**
- [x] Phase 5: Final polish (proper falloff, snap to pixel center)
- [x] Phase 6: Spatial denoise (7x7 bilateral blur in deferred_lighting.wgsl)

---

## XeGTAO Algorithm Full Re-Audit Checklist

### Audit Date: 2024-12-30 (Post-Final-Fixes)

Systematic comparison of our implementation against XeGTAO reference (`XeGTAO/Source/Rendering/Shaders/XeGTAO.hlsli`).

**Instructions:** Each item must be independently verified by reading both the XeGTAO reference and our implementation. Mark "VERIFIED" only after confirming the code matches.

| # | Component | XeGTAO Line | Our Line | Previous | Re-Check |
|---|-----------|-------------|----------|----------|----------|
| 1 | Falloff precompute: falloffMul = -1/falloffRange | L315 | L220 | ✅ | ⬜ NOT YET CHECKED |
| 2 | Falloff precompute: falloffAdd = falloffFrom/falloffRange + 1 | L316 | L221 | ✅ | ⬜ NOT YET CHECKED |
| 3 | Small radius fade: saturate((10-r)/100)*0.5 | L342-343 | L224 | ✅ | ⬜ NOT YET CHECKED |
| 4 | pixelTooCloseThreshold = 1.3 | L335 | L227 | ✅ | ⬜ NOT YET CHECKED |
| 5 | minS = pixelTooCloseThreshold / screenspaceRadius | L367 | L228 | ✅ | ⬜ NOT YET CHECKED |
| 6 | sliceK = (slice + noiseSlice) / sliceCount | L372 | L239 | ✅ | ⬜ NOT YET CHECKED |
| 7 | phi = sliceK * PI | L374 | L240 | ✅ | ⬜ NOT YET CHECKED |
| 8 | omega = (cosPhi, -sinPhi) | L377 | L245 | ✅ | ⬜ NOT YET CHECKED |
| 9 | directionVec = (cosPhi, sinPhi, 0) | L383 | L248 | ✅ | ⬜ NOT YET CHECKED |
| 10 | orthoDirectionVec = dirVec - dot(dirVec,viewVec)*viewVec | L386 | L251 | ✅ | ⬜ NOT YET CHECKED |
| 11 | axisVec = normalize(cross(orthoDir, viewVec)) | L390 | L254 | ✅ | ⬜ NOT YET CHECKED |
| 12 | projectedNormalVec = N - axisVec*dot(N,axisVec) | L396 | L257 | ✅ | ⬜ NOT YET CHECKED |
| 13 | signNorm = sign(dot(orthoDir, projNormal)) | L399 | L260 | ✅ | ⬜ NOT YET CHECKED |
| 14 | projectedNormalVecLength = length(projNormal) | L402 | L263 | ✅ | ⬜ NOT YET CHECKED |
| 15 | cosNorm = saturate(dot(projNormal,viewVec)/len) | L403 | L264 | ✅ | ⬜ NOT YET CHECKED |
| 16 | n = signNorm * FastACos(cosNorm) | L406 | L267 | ✅ | ⬜ NOT YET CHECKED |
| 17 | lowHorizonCos0 = cos(n + PI_HALF) | L409 | L270 | ✅ | ⬜ NOT YET CHECKED |
| 18 | lowHorizonCos1 = cos(n - PI_HALF) | L410 | L270 | ✅ | ⬜ NOT YET CHECKED |
| 19 | horizonCos0 = lowHorizonCos0 | L413 | L273 | ✅ | ⬜ NOT YET CHECKED |
| 20 | horizonCos1 = lowHorizonCos1 | L414 | L274 | ✅ | ⬜ NOT YET CHECKED |
| 21 | stepBaseNoise = (slice + step*stepsPerSlice) * 0.618... | L420 | L283 | ✅ | ⬜ NOT YET CHECKED |
| 22 | stepNoise = frac(noiseSample + stepBaseNoise) | L421 | L284 | ✅ | ⬜ NOT YET CHECKED |
| 23 | s = (step + stepNoise) / stepsPerSlice | L424 | L287 | ✅ | ⬜ NOT YET CHECKED |
| 24 | s = pow(s, sampleDistributionPower) | L427 | L290 | ✅ | ⬜ NOT YET CHECKED |
| 25 | s += minS | L430 | L293 | ✅ | ⬜ NOT YET CHECKED |
| 26 | sampleOffset = s * omega (in pixels) | L433 | L296 | ✅ | ⬜ NOT YET CHECKED |
| 27 | Snap to pixel: sampleOffset = round(offset) * pixelSize | L442 | L299 | ✅ | ⬜ NOT YET CHECKED |
| 28 | sampleDelta = samplePos - centerPos | L466-467 | L307-308 | ✅ | ⬜ NOT YET CHECKED |
| 29 | sampleHorizonVec = sampleDelta / length | L472 | L312 | ✅ | ⬜ NOT YET CHECKED |
| 30 | weight = saturate(dist * falloffMul + falloffAdd) | L477 | L315 | ✅ | ⬜ NOT YET CHECKED |
| 31 | shc = dot(sampleHorizonVec, viewVec) | L488 | L318 | ✅ | ⬜ NOT YET CHECKED |
| 32 | shc = lerp(lowHorizonCos, shc, weight) | L492 | L321 | ✅ | ⬜ NOT YET CHECKED |
| 33 | horizonCos = max(horizonCos, shc) | L505 | L324 | ✅ | ⬜ NOT YET CHECKED |
| 34 | projNormalLen = lerp(projNormalLen, 1, 0.05) | L532 | L352 | ✅ | ⬜ NOT YET CHECKED |
| 35 | h0 = -FastACos(horizonCos1) | L536 | L356 | ✅ | ⬜ NOT YET CHECKED |
| 36 | h1 = FastACos(horizonCos0) | L537 | L357 | ✅ | ⬜ NOT YET CHECKED |
| 37 | iarc = (cosNorm + 2*h*sin(n) - cos(2*h-n)) / 4 | L542-543 | L363-364 | ✅ | ⬜ NOT YET CHECKED |
| 38 | localVisibility = projLen * (iarc0 + iarc1) | L544 | L367 | ✅ | ⬜ NOT YET CHECKED |
| 39 | visibility /= sliceCount | L556 | L372 | ✅ | ⬜ NOT YET CHECKED |
| 40 | visibility = pow(visibility, finalValuePower) | L557 | L375 | ✅ | ⬜ NOT YET CHECKED |
| 41 | visibility = max(0.03, visibility) | L558 | L378 | ✅ | ⬜ NOT YET CHECKED |

### Known Differences (Acceptable)

| # | Component | XeGTAO | Ours | Reason |
|---|-----------|--------|------|--------|
| A | Depth MIP chain | Uses MIP levels (L438) | Single-level | Performance vs quality tradeoff |
| B | Thin occluder heuristic | thinOccluderCompensation (L480-484) | Not implemented | Optional feature |

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

## Parameter Tuning Checklist

### XeGTAO Default Values (from XeGTAO.h)

```cpp
#define XE_GTAO_DEFAULT_RADIUS_MULTIPLIER         1.457f   // counter screen space biases
#define XE_GTAO_DEFAULT_FALLOFF_RANGE             0.615f   // distant samples contribute less
#define XE_GTAO_DEFAULT_SAMPLE_DISTRIBUTION_POWER 2.0f     // small crevices more important
#define XE_GTAO_DEFAULT_THIN_OCCLUDER_COMPENSATION 0.0f    // thickness heuristic
#define XE_GTAO_DEFAULT_FINAL_VALUE_POWER         2.2f     // power function on final value
#define XE_GTAO_DEFAULT_DEPTH_MIP_SAMPLING_OFFSET 3.30f    // MIP selection
```

### Parameter Verification Checklist

| # | Parameter | XeGTAO Default | Our Value | File:Line | Status |
|---|-----------|---------------|-----------|-----------|--------|
| 1 | SLICE_COUNT | 3/6/9 (Low/Med/High) | 9 | gtao.wgsl:40 | ⬜ NOT YET CHECKED |
| 2 | STEPS_PER_SLICE | 2-4 typical | 4 | gtao.wgsl:41 | ⬜ NOT YET CHECKED |
| 3 | effect_radius | Scene dependent | 3.0 | gtao_node.rs:187 | ⬜ NOT YET CHECKED |
| 4 | effect_falloff_range | **0.615** | 0.615 | gtao_node.rs:188 | ⬜ NOT YET CHECKED |
| 5 | radius_multiplier | **1.457** | 1.457 | gtao_node.rs:192 | ⬜ NOT YET CHECKED |
| 6 | final_value_power | **2.2** | 2.2 | gtao_node.rs:193 | ⬜ NOT YET CHECKED |
| 7 | sample_distribution_power | **2.0** | 2.0 | gtao_node.rs:194 | ⬜ NOT YET CHECKED |
| 8 | thin_occluder_compensation | **0.0** | 0.0 | gtao_node.rs:195 | ⬜ NOT YET CHECKED |
| 9 | pixel_too_close_threshold | **1.3** | 1.3 | gtao.wgsl:227 | ⬜ NOT YET CHECKED |
| 10 | projNormalLen fudge | **0.05** | 0.05 | gtao.wgsl:352 | ⬜ NOT YET CHECKED |
| 11 | min visibility | **0.03** | 0.03 | gtao.wgsl:378 | ⬜ NOT YET CHECKED |
| 12 | Blur kernel_radius | XeGTAO: edge-aware | 3 (7x7) | deferred_lighting.wgsl:98 | ⬜ NOT YET CHECKED |
| 13 | Half-resolution | Yes | Yes | gtao.rs:70-71 | ⬜ NOT YET CHECKED |

### Known Issues

1. **Config not wired through**: `gtao.rs` has `GtaoConfig` struct but `gtao_node.rs` uses hardcoded values (lines 187-195)
2. **7x7 blur may be excessive**: XeGTAO uses edge-aware denoise with `DenoiseBlurBeta = 1.2`, not a fixed kernel

### XeGTAO Quality Presets

| Quality | SliceCount | StepsPerSlice | Total Samples |
|---------|------------|---------------|---------------|
| Low | 1 | 2 | 4 |
| Medium | 2 | 2 | 8 |
| High | 3 | 3 | 18 |
| Ultra | 9 | 3 | 54 |

Our current: 9 slices × 4 steps = 72 samples (higher than Ultra)
