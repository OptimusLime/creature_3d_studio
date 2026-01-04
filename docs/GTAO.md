# XeGTAO Implementation Documentation

**Status:** Complete (Phase 1-6 implemented, verified working)  
**Last Updated:** 2024-12-31

---

## Overview

This document consolidates all GTAO (Ground Truth Ambient Occlusion) implementation work in this project. We implemented Intel's XeGTAO algorithm with the goal of 100% compliance to the reference implementation.

**Reference Implementation:** https://github.com/GameTechDev/XeGTAO

**Key Reference Files:**
- `XeGTAO/Source/Rendering/Shaders/XeGTAO.hlsli` - Core algorithm
- `XeGTAO/Source/Rendering/Shaders/XeGTAO.h` - Constants and defaults

---

## Algorithm Summary

XeGTAO is a screen-space ambient occlusion technique based on the GTAO paper by Activision ("Practical Real-Time Strategies for Accurate Indirect Occlusion" - Jimenez et al.). Unlike traditional SSAO which samples a hemisphere, GTAO searches for "horizon" angles in multiple slice directions, providing physically accurate occlusion.

### Three-Pass Pipeline

```
Pass 1: Depth Prefilter
    Input:  NDC depth buffer (from G-buffer)
    Output: 5-level MIP chain of viewspace linear depth
    
Pass 2: Main GTAO (per pixel)
    Inputs: Depth MIPs, View-space normals, Noise
    Steps:
      - Linearize depth -> viewspace Z
      - Calculate edges (for denoiser)
      - FOR each slice (direction):
        - Project normal onto slice plane
        - Search for horizon angles (positive/negative)
        - Integrate visibility using analytic formula
      - Average visibility across slices
      - Apply final power, clamp minimum
    Outputs: Raw noisy visibility, Packed edges

Pass 3: Denoise (edge-aware spatial blur)
    Inputs: Raw visibility, Packed edges
    Output: Denoised visibility
    Note: Can run 1-3 passes for increasing smoothness
```

### Core Formula (per slice)

```
visibility = (projNormalLen / 4) * (cos(n) + 2h*sin(n) - cos(2h - n))
```

Where:
- `n` = angle of projected normal in slice plane
- `h` = horizon angle (clamped to hemisphere)
- `projNormalLen` = length of normal projected onto slice plane

---

## Implementation Phases

### Phase 1: Config System
**Status:** Complete

Wired `GtaoConfig` through the render pipeline. All parameters now come from config instead of hardcoded values.

**Files:**
- `crates/studio_core/src/deferred/gtao.rs` - GtaoConfig struct

### Phase 2: Depth MIP Chain
**Status:** Complete

Implemented 5-level depth MIP pyramid using XeGTAO's weighted average filter (`XeGTAO_PrefilterDepths16x16`).

**Files:**
- `assets/shaders/gtao_depth_prefilter.wgsl` - Compute shader
- `crates/studio_core/src/deferred/gtao_depth_prefilter.rs` - Render node

### Phase 3: Main GTAO Pass
**Status:** Complete

Implemented full XeGTAO main pass with horizon search, visibility integration, and edge calculation.

**Files:**
- `assets/shaders/gtao.wgsl` - Main GTAO shader
- `crates/studio_core/src/deferred/gtao_node.rs` - Render node

### Phase 4+5: Edge-Aware Denoiser
**Status:** Complete (audited line-by-line against XeGTAO)

Implemented XeGTAO's edge-aware denoiser with:
- Edge packing/unpacking
- Edge symmetry enforcement
- AO leaking prevention
- Diagonal weight calculation

**Audit Results:**
| Component | XeGTAO Lines | Our File:Lines | Status |
|-----------|--------------|----------------|--------|
| Edge calculation | L120-129 | gtao.wgsl:280-301 | ✅ |
| Edge packing | L132-141 | gtao.wgsl:305-310 | ✅ |
| Edge unpacking | L686-696 | gtao_denoise.wgsl:44-56 | ✅ |
| AddSample helper | L704-710 | gtao_denoise.wgsl:63-67 | ✅ |
| Blur amount calc | L736-737 | gtao_denoise.wgsl:86-93 | ✅ |
| Edge symmetry | L769-770 | gtao_denoise.wgsl:122-124 | ✅ |
| AO leaking prevention | L772-776 | gtao_denoise.wgsl:126-132 | ✅ |
| Diagonal weights | L785-788 | gtao_denoise.wgsl:134-139 | ✅ |
| Weighted sum | L801-814 | gtao_denoise.wgsl:152-166 | ✅ |

**Files:**
- `assets/shaders/gtao_denoise.wgsl` - Denoise compute shader
- `crates/studio_core/src/deferred/gtao_denoise.rs` - Render node

### Phase 6: TAA Noise Index
**Status:** Complete

Replaced texture-based noise with XeGTAO's Hilbert curve + R2 sequence for proper spatio-temporal distribution.

**Files:**
- `assets/shaders/gtao.wgsl` - `hilbert_index()`, `spatio_temporal_noise()`
- `crates/studio_core/src/deferred/gtao_node.rs` - `GtaoFrameCount`

---

## File Reference

| File | Purpose |
|------|---------|
| `assets/shaders/gtao.wgsl` | Main GTAO shader (outputs AO + packed edges) |
| `assets/shaders/gtao_depth_prefilter.wgsl` | Depth MIP chain compute shader |
| `assets/shaders/gtao_denoise.wgsl` | XeGTAO edge-aware denoiser |
| `crates/studio_core/src/deferred/gtao.rs` | Config struct, texture allocation |
| `crates/studio_core/src/deferred/gtao_node.rs` | Main GTAO render node |
| `crates/studio_core/src/deferred/gtao_depth_prefilter.rs` | Depth prefilter node |
| `crates/studio_core/src/deferred/gtao_denoise.rs` | Denoise compute node |
| `assets/shaders/deferred_lighting.wgsl` | Samples denoised GTAO |

---

## Default Parameters (XeGTAO HIGH Preset)

| Parameter | Value | Description |
|-----------|-------|-------------|
| `slice_count` | 3 | Number of slice directions |
| `steps_per_slice` | 3 | Samples per slice direction |
| `effect_radius` | 0.5 | World-space AO radius |
| `effect_falloff_range` | 0.615 | Falloff range as fraction of radius |
| `radius_multiplier` | 1.457 | Screen-space radius adjustment |
| `final_value_power` | 2.2 | Power curve on final visibility |
| `sample_distribution_power` | 2.0 | Sample distribution curve |
| `thin_occluder_compensation` | 0.0 | Thin object handling (0=disabled) |
| `depth_mip_sampling_offset` | 3.30 | MIP level selection offset |
| `edge_sensitivity` | 0.011 | Edge detection threshold |

---

## Debug Modes

### GTAO Shader (gtao_debug_mode)
| Mode | Description |
|------|-------------|
| 0 | Normal GTAO output |
| 10 | NDC depth (raw) |
| 11 | Viewspace linear depth (MIP 0) |
| 12-15 | Depth MIP levels 1-4 |
| 16 | Log-scale depth (full range) |
| 20 | View-space normal.z |
| 21 | View-space normal.xy |
| 30 | Screen-space radius |
| 40 | Packed edges (raw) |
| 44 | Inverted edges (edges = bright) |
| 50 | Raw GTAO (before denoise) |

### Lighting Shader (lighting_debug_mode)
| Mode | Description |
|------|-------------|
| 0 | Final lit scene |
| 1 | G-buffer normals |
| 2 | G-buffer depth |
| 3 | Albedo only |
| 5 | GTAO (denoised AO) |

**Important:** To view GTAO debug output, set BOTH `gtao_debug_mode` AND `lighting_debug_mode = 5`.

---

## Verification Process

We established a manual phase-by-phase verification process with 8 phases:

1. **Phase 1:** G-Buffer Inputs (depth, normals)
2. **Phase 2:** Depth Linearization & MIP Chain
3. **Phase 3:** Edge Detection
4. **Phase 4:** View-Space Normals
5. **Phase 5:** Screen-Space Radius
6. **Phase 6:** Raw GTAO Output
7. **Phase 7:** Denoised GTAO
8. **Phase 8:** Final Render

Test command:
```bash
cargo run --example p20_gtao_test
# Screenshots saved to screenshots/gtao_test/
```

---

## Known Issues

### Issue #1: Hardcoded Near Plane
**Status:** Open (Medium severity)

The near clip plane is hardcoded to `0.1` instead of being read from the camera projection matrix.

**Locations:**
- `assets/shaders/gbuffer.wgsl:13`
- `crates/studio_core/src/deferred/gtao_depth_prefilter.rs:116`

**Impact:** If camera near plane changes, depth linearization will be incorrect.

### Issue #2: Potential Banding Artifacts
**Status:** Open (Low severity, needs investigation)

Horizontal banding visible at bottom of p17_chunk_streaming scene. May be GTAO-related or shadow/fog issue.

---

## Tuning Notes

- **effect_radius = 0.5** (XeGTAO default) produces subtle AO
- **effect_radius = 1.0** produces more visible AO for voxel scenes without noise
- Can tune per-scene via `GtaoConfig`

---

## Research References

### GTAO Paper
"Practical Real-Time Strategies for Accurate Indirect Occlusion"  
Jorge Jimenez, Xian-Chun Wu, Angelo Pesce, Adrian Jarabo  
Activision Blizzard, Universidad de Zaragoza  
Technical Memo ATVI-TR-19-01

Key contribution: Radiometrically-correct AO integral matching ground truth in 0.5ms on PS4 at 1080p.

### XeGTAO
Intel's evolution of GTAO with:
- Depth MIP-mapping for efficient multi-scale sampling
- Hilbert curve + R2 sequence for spatio-temporal noise
- Edge-aware spatial denoiser
- Thin occluder compensation heuristic

---

## Quick Commands

```bash
# Run GTAO test (captures all debug screenshots)
cargo run --example p20_gtao_test

# View screenshots
open screenshots/gtao_test/

# Run other examples
cargo run --example p9_island
cargo run --example p10_dark_world
```
