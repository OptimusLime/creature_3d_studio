# GTAO Implementation Session Cheatsheet

**Purpose:** Quick context restoration for AI assistants continuing this work.

---

## MUST READ FIRST (in order)

1. `docs/HOW_WE_WORK.md` - Our process (hypothesis-driven, no shortcuts, verify everything)
2. `docs/GTAO_IMPLEMENTATION_PLAN.md` - The master plan (100% XeGTAO compliance)
3. `XeGTAO/Source/Rendering/Shaders/XeGTAO.hlsli` - THE reference implementation
4. `XeGTAO/Source/Rendering/Shaders/XeGTAO.h` - Default constants

---

## Context Summary (30 seconds)

We're implementing Intel's XeGTAO (Ground Truth Ambient Occlusion) in our Bevy/Rust voxel engine. The remit is **100% compliance** with XeGTAO - no "simpler" approaches, no shortcuts.

**Current state:** All implementation phases complete, but **output shows excessive noise**. Debug infrastructure built. Ready for main pass audit.

**Completed work:**
- Phase 1: Wire GtaoConfig through
- Phase 2: Depth MIP chain infrastructure
- Phase 3: Main pass XeGTAO compliance
- Phase 4+5: Edge-aware denoiser (audited, all pass - items 46-54)
- Phase 6: TAA noise index (Hilbert curve + R2 sequence)
- Debug infrastructure: Runtime debug modes, multi-screenshot capture system

**Current problem:** GTAO output shows excessive noise despite denoiser working correctly.

**Next steps:** See `docs/GTAO_DEBUG_PLAN.md` for detailed SMART tasks:
1. Phase 0: Visual diagnosis from debug screenshots
2. Phase 1: Line-by-line audit of main pass (items 1-45)
3. Phase 2: Fix identified defects
4. Phase 3: Verify quality gates pass

---

## Key Files

| File | Purpose |
|------|---------|
| `assets/shaders/gtao.wgsl` | Main GTAO shader (outputs AO + packed edges) |
| `assets/shaders/gtao_depth_prefilter.wgsl` | Depth MIP chain compute shader |
| `assets/shaders/gtao_denoise.wgsl` | XeGTAO edge-aware denoiser (NEW) |
| `crates/studio_core/src/deferred/gtao.rs` | Config struct, texture allocation |
| `crates/studio_core/src/deferred/gtao_node.rs` | Main GTAO render node |
| `crates/studio_core/src/deferred/gtao_depth_prefilter.rs` | Depth prefilter node |
| `crates/studio_core/src/deferred/gtao_denoise.rs` | Denoise compute node (NEW) |
| `assets/shaders/deferred_lighting.wgsl` | Samples denoised GTAO (blur removed) |
| `XeGTAO/Source/Rendering/Shaders/XeGTAO.hlsli` | Reference implementation |

---

## Current Progress

| Phase | Task | Status |
|-------|------|--------|
| 0 | Document differences | **DONE** |
| 0 | Write implementation plan | **DONE** |
| 1 | Wire GtaoConfig through | **DONE** |
| 2 | Implement depth MIP chain | **DONE** |
| 3 | Main pass XeGTAO compliance | **DONE** |
| 4+5 | Edge-aware denoiser + edge packing | **DONE** (audited) |
| 6 | TAA noise index (Hilbert+R2) | **DONE** |

---

## What Was Done in Phase 4+5

**Merged Phase 4 (denoiser) and Phase 5 (edge packing) since they're interdependent.**

**AUDIT STATUS: ALL PASS** (verified line-by-line against XeGTAO.hlsli)

| Component | XeGTAO Lines | Our File:Lines | Status |
|-----------|--------------|----------------|--------|
| Edge calculation | L120-129 | gtao.wgsl:246-260 | ✅ |
| Edge packing | L132-141 | gtao.wgsl:264-269 | ✅ |
| Edge unpacking | L686-696 | gtao_denoise.wgsl:44-56 | ✅ |
| AddSample helper | L704-710 | gtao_denoise.wgsl:63-67 | ✅ |
| Blur amount calc | L736-737 | gtao_denoise.wgsl:86-93 | ✅ |
| Edge symmetry | L769-770 | gtao_denoise.wgsl:122-124 | ✅ |
| AO leaking prevention | L772-776 | gtao_denoise.wgsl:126-132 | ✅ |
| Diagonal weights | L785-788 | gtao_denoise.wgsl:134-139 | ✅ |
| Weighted sum | L801-814 | gtao_denoise.wgsl:152-166 | ✅ |

**Note:** We use individual texture samples instead of GatherRed (performance difference, not correctness).

Changes made:
1. **gtao.wgsl** - Added edge calculation (`calculate_edges`) and packing (`pack_edges`), changed output to MRT (AO + packed edges)
2. **gtao_node.rs** - Updated to output to two render targets (AO and edges textures)
3. **gtao_denoise.wgsl** (NEW) - XeGTAO edge-aware denoiser compute shader implementing:
   - `unpack_edges` (L686-696)
   - `add_sample` (L704-710)
   - `main` denoise kernel (L734-826)
4. **gtao_denoise.rs** (NEW) - Render node for denoiser compute pass
5. **lighting_node.rs** - Now uses denoised GTAO texture
6. **deferred_lighting.wgsl** - Removed 7x7 blur, samples denoised texture directly
7. **labels.rs, mod.rs, plugin.rs** - Wired denoiser into render graph

---

## Phase 6 Complete

**TAA Noise Index Support - DONE**

Replaced texture-based noise with XeGTAO's Hilbert curve + R2 sequence.

**Changes made:**
1. ✅ `GtaoFrameCount` resource tracks frame counter
2. ✅ `update_gtao_frame_count()` system increments each frame
3. ✅ `noise_index` passed via params3.z uniform
4. ✅ `hilbert_index()` function ported (XeGTAO.h L120-142)
5. ✅ `spatio_temporal_noise()` function ported (vaGTAO.hlsl L74-91)
6. ✅ `compute_gtao()` now uses Hilbert+R2 instead of noise texture

## Debug Infrastructure (NEW - Session 2024-12-30)

**Runtime Debug Mode Switching is now WORKING!**

The GTAO shader now reads debug mode from uniform (`params3.w`) instead of a constant.
The `DebugModes` resource is extracted to render world and passed through each frame.

**Debug Screenshot System:**
- `DebugScreenshotConfig` - Configure multi-capture session
- `DebugCapture::gtao_debug(mode)` - Set GTAO debug mode (0-40)
- `DebugCapture::lighting_debug(mode)` - Set lighting debug mode (0-7)
- All screenshots saved to specified folder

Example:
```rust
let config = DebugScreenshotConfig::new("screenshots/gtao_test")
    .with_capture("render", DebugCapture::default())
    .with_capture("gtao_depth", DebugCapture::gtao_debug(11))
    .with_capture("ao_only", DebugCapture::lighting_debug(5));
```

---

## GTAO Debug Modes (gtao.wgsl params3.w)

| Mode | Description | Expected Output |
|------|-------------|-----------------|
| 0 | Normal GTAO (AO + edges) | White=lit, dark=occluded |
| 10 | NDC depth (raw * 100) | Near=dark, far=bright |
| 11 | Linear viewspace depth (/50) | Smooth gradient, near=dark |
| 12-15 | Depth MIP levels 0-3 | Progressively blurrier |
| 20 | View-space normal.z | Camera-facing=bright |
| 30 | Screenspace radius (/100) | Brighter=larger radius |
| 40 | Packed edges | Edges=dark, smooth=bright |

---

## Lighting Debug Modes (deferred_lighting.wgsl)

| Mode | Description |
|------|-------------|
| 0 | Final lit scene |
| 1 | G-buffer normals (world space) |
| 2 | G-buffer depth (linear) |
| 3 | Albedo only |
| 4 | Shadow factor (R=moon1, G=moon2) |
| 5 | GTAO (denoised AO) |
| 6 | Point lights only |
| 7 | World position XZ |

**IMPORTANT:** To view GTAO intermediate values, you must set BOTH:
1. `gtao_debug_mode` to write the value to the GTAO texture
2. `lighting_debug_mode = 5` to pass through the GTAO texture as grayscale

Example:
```rust
DebugCapture {
    name: "gtao_depth".to_string(),
    gtao_debug_mode: 11,      // Linear viewspace depth -> GTAO texture
    lighting_debug_mode: 5,   // Pass through GTAO texture as grayscale
    wait_frames: 5,
}
```

---

## Hypothesis-Driven GTAO Audit

Following HOW_WE_WORK.md, we verify each layer of the GTAO pipeline systematically.

### Layer 1: Depth Reconstruction

**Hypothesis:** Viewspace depth is correctly computed from hardware depth buffer.

**Test:**
- Debug mode 11: Linear depth should show smooth gradient
- Near objects (distance ~5) → dark gray (~0.1)
- Far objects (distance ~50) → white (~1.0)
- Sky → black (0.0)

**Expected:** `gtao_depth.png` shows smooth gradient, no banding/noise.

### Layer 2: View-Space Normals

**Hypothesis:** G-buffer world normals are correctly transformed to view space.

**Test:**
- Debug mode 20: View-space normal.z
- Camera-facing surfaces → bright (normal.z close to -1)
- Side surfaces → medium gray
- Away-facing surfaces → dark

**Expected:** `gtao_normal.png` shows correct facing orientation.

### Layer 3: Screenspace Radius

**Hypothesis:** Effect radius in screen pixels is computed correctly per XeGTAO formula.

**Test:**
- Debug mode 30: Screenspace radius
- Near objects → larger radius (brighter)
- Far objects → smaller radius (darker)
- Radius should scale with `viewspaceZ * NDCToViewMul_x_PixelSize`

**Expected:** `gtao_radius.png` shows inverse depth relationship.

### Layer 4: Horizon Search

**Hypothesis:** Horizon angles are found correctly for each slice direction.

**Test (manual shader mod required):**
- Output `horizonCos0` or `horizonCos1` as grayscale
- Flat surface → horizon at ~90° (cos=0)
- Corner → horizon lower (cos > 0, darker)

### Layer 5: Visibility Integration

**Hypothesis:** XeGTAO visibility formula is implemented correctly.

**Key formula from GTAO paper:**
```
visibility = (projNormalLen / 4) * (cosNorm + 2h*sinN - cos(2h-n))
```

**Test:**
- Debug mode 5 (lighting): AO visualization
- Flat surfaces → AO > 0.95 (near white)
- Corners → AO 0.3-0.6 (significant darkening)
- No patchy noise patterns

### Layer 6: Edge Detection & Denoiser

**Hypothesis:** Edge-aware blur preserves depth discontinuities.

**Test:**
- Debug mode 40: Packed edges
- Sharp depth edges → dark
- Smooth surfaces → bright (high connectivity)

---

## Audit Checklist (Items 1-45)

See `docs/GTAO_DEBUG_PLAN.md` for the detailed 45-item audit checklist organized by groups:
- Group A: Setup & Precision (items 1-5)
- Group B: Slice Setup (items 6-18)
- Group C: Step Loop (items 19-29)
- Group D: Horizon Update (items 30-35)
- Group E: Visibility Integration (items 36-45)

Also see `docs/GTAO_IMPLEMENTATION_PLAN.md` for the full 54-item master checklist.

---

## Quick Commands

```bash
# Build and run GTAO test (captures 8 screenshots)
cargo run --example p20_gtao_test

# Screenshots saved to:
ls screenshots/gtao_test/

# Files generated:
# - render.png        (final scene)
# - ao_only.png       (GTAO visualization)
# - gtao_depth.png    (linear depth, mode 11)
# - gtao_normal.png   (view-space normal.z, mode 20)
# - gtao_edges.png    (packed edges, mode 40)
# - gtao_radius.png   (screenspace radius, mode 30)
# - gbuffer_normals.png (world normals, lighting mode 1)
# - gbuffer_depth.png   (depth buffer, lighting mode 2)
```

---

## XeGTAO HIGH Preset (Target)

| Setting | Value |
|---------|-------|
| SliceCount | 3 |
| StepsPerSlice | 3 |
| Total samples | 18 |
| RadiusMultiplier | 1.457 |
| FalloffRange | 0.615 |
| SampleDistributionPower | 2.0 |
| FinalValuePower | 2.2 |
| DenoiseBlurBeta | 1.2 |

---

## Process Reminders

From HOW_WE_WORK.md:
- **Never abandon** because it's hard
- **Never substitute** simpler approaches
- **Hypothesis-driven debugging** - observe, hypothesize, test, analyze
- **Verify each phase** before proceeding
- **Be honest** about defects - no wishful thinking
