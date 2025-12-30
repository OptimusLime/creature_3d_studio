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
- [ ] Phase 5: Quality tuning - **BLOCKED BY NOISE ISSUE**
- [ ] Phase 6: Spatial denoise

---

## KNOWN ISSUE: Excessive Noise

### Problem
The GTAO output has heavy stippling/grain pattern across all surfaces. Increasing sample count from 9 to 72 samples did NOT materially improve the noise.

### Hypotheses to Investigate
1. **Noise texture sampling** - Is the random rotation being applied correctly?
2. **Half-resolution rendering** - GTAO renders at half res, is upsampling causing aliasing?
3. **Sample distribution** - Are samples clustered rather than well-distributed?
4. **Missing denoise pass** - XeGTAO includes edge-aware spatial blur we haven't implemented

### Current Parameters
```wgsl
const SLICE_COUNT: i32 = 6;       // Direction slices
const STEPS_PER_SLICE: i32 = 6;   // Steps per direction
// Total: 6 slices × 6 steps × 2 directions = 72 samples
```

### Effect Radius
```rust
effect_radius: 3.0,  // World units (voxels are 1 unit)
```

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

## Next Steps

1. **Investigate noise source** - Check if noise is from sampling pattern or reconstruction
2. **Implement proper denoise** - XeGTAO has edge-aware spatial blur
3. **Verify half-res rendering** - Ensure upsampling isn't causing aliasing
4. **Compare with XeGTAO reference** - Ensure algorithm matches Intel's implementation
