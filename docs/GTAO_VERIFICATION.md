# GTAO Manual Verification Process

**Created:** 2024-12-31
**Purpose:** Phase-by-phase manual approval of GTAO implementation
**Status:** IN PROGRESS

---

## Algorithm Overview (XeGTAO Pipeline Order)

The XeGTAO algorithm executes in the following order:

```
Pass 1: Depth Prefilter
    Input:  NDC depth buffer (from G-buffer)
    Output: 5-level MIP chain of viewspace linear depth
    
Pass 2: Main GTAO (per pixel)
    Inputs: Depth MIPs, View-space normals, Noise
    Steps:
      2.1  Linearize depth -> viewspace Z
      2.2  Calculate edges (for denoiser)
      2.3  Compute viewspace position
      2.4  Compute view vector (normalize(-position))
      2.5  FOR each slice (direction):
           2.5.1  Compute slice direction (phi from slice index + noise)
           2.5.2  Project normal onto slice plane -> get n angle
           2.5.3  Initialize horizon angles
           2.5.4  FOR each step:
                  - Sample depth at offset (using MIP chain)
                  - Compute sample position
                  - Compute horizon vector and cos
                  - Apply falloff weight
                  - Update horizon angle (max)
           2.5.5  Integrate visibility using horizon angles and n
           2.5.6  Accumulate visibility
      2.6  Average visibility across slices
      2.7  Apply final power
      2.8  Clamp minimum (0.03)
    Outputs: Raw noisy visibility, Packed edges

Pass 3: Denoise (edge-aware spatial blur)
    Inputs: Raw visibility, Packed edges
    Steps:
      3.1  Unpack edges for center and neighbors
      3.2  Enforce edge symmetry
      3.3  Compute diagonal weights
      3.4  Weighted average of 3x3 neighborhood
    Output: Denoised visibility
    Note: Can run 1-3 passes for increasing smoothness
```

---

## Verification Phases

Each phase must be **MANUALLY APPROVED** before proceeding to the next.

### PHASE 1: G-Buffer Inputs

**What:** The raw inputs from the G-buffer that GTAO uses.

**Algorithm Step:** Pre-requisite - GTAO reads from these textures.

| Artifact | Debug Mode | Description | SUCCESS Criteria | APPROVED |
|----------|------------|-------------|------------------|----------|
| `p1_gbuffer_depth.png` | lighting=2 | NDC depth from G-buffer | Smooth gradient. Near geometry = one shade, far = another. No noise within flat surfaces. Background (sky) consistent. | [ ] |
| `p1_gbuffer_normals.png` | lighting=1 | World-space normals | Each flat surface is ONE solid color. Different surfaces have different colors based on orientation. Sharp transitions only at geometry edges. | [ ] |

**Hypothesis if FAIL:** G-buffer is corrupt, mesh normals are wrong, or depth precision issues.

---

### PHASE 2: Depth Linearization & MIP Chain

**What:** Converting NDC depth to viewspace linear Z, then building MIP pyramid.

**Algorithm Step:** Pass 1 - `XeGTAO_PrefilterDepths16x16` (L617-684)

| Artifact | Debug Mode | Description | SUCCESS Criteria | APPROVED |
|----------|------------|-------------|------------------|----------|
| `p2_depth_mip0.png` | gtao=11 | Viewspace linear depth (MIP 0) | Smooth gradient from near (dark) to far (bright). No banding on flat surfaces. Values increase monotonically with distance. | [ ] |
| `p2_depth_mip1.png` | gtao=12 | Depth MIP level 1 | Half resolution of MIP 0. Same smooth gradient, slightly blurrier. | [ ] |
| `p2_depth_mip2.png` | gtao=13 | Depth MIP level 2 | Quarter resolution. Preserves major depth discontinuities. | [ ] |

**Hypothesis if FAIL:** Depth unpacking formula wrong, projection matrix extraction incorrect, or MIP filter broken.

---

### PHASE 3: Edge Detection

**What:** Computing depth discontinuities for the denoiser.

**Algorithm Step:** Pass 2, Line 262-263 - `XeGTAO_CalculateEdges` (L120-129)

| Artifact | Debug Mode | Description | SUCCESS Criteria | APPROVED |
|----------|------------|-------------|------------------|----------|
| `p3_edges_packed.png` | gtao=40 | Packed edges (raw output) | Flat surfaces = uniform bright (high edge values = smooth). Dark lines ONLY at actual geometry edges (silhouettes, corners, stair edges). | [ ] |
| `p3_edges_inverted.png` | gtao=44 | Inverted edges (edges = bright) | Edges clearly visible at geometry discontinuities. NO noise dots on flat surfaces. Clean silhouette lines. | [ ] |

**Reference Formula (L128):**
```
edges = saturate(1.25 - abs(depth_delta) / (center_depth * 0.011))
```

**Hypothesis if FAIL:** Edge sensitivity constant (0.011) wrong for our depth range, or depth delta calculation incorrect.

---

### PHASE 4: View-Space Normal Reconstruction

**What:** The view-space normal used by GTAO (either from G-buffer or reconstructed from depth).

**Algorithm Step:** Pass 2 - Either passed in or computed via `XeGTAO_CalculateNormal` (L143-160)

| Artifact | Debug Mode | Description | SUCCESS Criteria | APPROVED |
|----------|------------|-------------|------------------|----------|
| `p4_normal_z.png` | gtao=20 | View-space normal.z | Surfaces facing camera = BRIGHT (z close to -1 in view space means facing camera). Side surfaces = medium gray. Surfaces facing away = dark. | [ ] |
| `p4_normal_xy.png` | gtao=21 | View-space normal.xy | Shows left/right and up/down facing encoded as grayscale. | [ ] |

**Hypothesis if FAIL:** Coordinate system mismatch (Y-up vs Y-down, Z direction), normal transformation wrong.

---

### PHASE 5: Screen-Space Radius

**What:** The effective sample radius in pixels.

**Algorithm Step:** Pass 2, Line 340 - `screenspaceRadius = effectRadius / pixelDirRBViewspaceSizeAtCenterZ`

| Artifact | Debug Mode | Description | SUCCESS Criteria | APPROVED |
|----------|------------|-------------|------------------|----------|
| `p5_radius.png` | gtao=30 | Screen-space radius | Near objects = LARGER radius (brighter). Far objects = smaller radius (darker). Smooth gradient following depth. | [ ] |

**Reference (L340):**
```
screenspaceRadius = effectRadius / (viewspaceZ * NDCToViewMul_x_PixelSize)
```

**Hypothesis if FAIL:** NDCToViewMul calculation wrong, effectRadius not set correctly.

---

### PHASE 6: Raw GTAO Output (Before Denoise)

**What:** The visibility term before denoising. This is the core algorithm output.

**Algorithm Step:** Pass 2 complete, before Pass 3

| Artifact | Debug Mode | Description | SUCCESS Criteria | APPROVED |
|----------|------------|-------------|------------------|----------|
| `p6_gtao_raw.png` | gtao=50 | Raw visibility | Corners/crevices = DARK. Open flat surfaces = BRIGHT (near white). Noise is expected but pattern should correlate with geometry - darker where occluded. NO uniform random stippling everywhere. | [ ] |

**Key Verification Points:**
1. Flat ground far from walls should be ~0.95+ (bright)
2. 90-degree corner should be ~0.3-0.5 (dark)
3. Under floating cube should show shadow on ground
4. Stair steps should show AO in the creases

**Hypothesis if FAIL:** 
- If everything dark: visibility integration formula wrong, horizon angles wrong
- If everything bright: no horizon found, sampling not working
- If random noise everywhere: noise input wrong, slice/step loop broken
- If inverted: final visibility inverted somewhere

---

### PHASE 7: Denoised GTAO Output

**What:** The final smoothed visibility after edge-aware blur.

**Algorithm Step:** Pass 3 - `XeGTAO_Denoise` (L734-826)

| Artifact | Debug Mode | Description | SUCCESS Criteria | APPROVED |
|----------|------------|-------------|------------------|----------|
| `p7_ao_denoised.png` | gtao=0, lighting=5 | Denoised visibility | SMOOTH gradients. No visible noise or stippling. Corners still dark, flats still bright. Edges preserved at geometry boundaries (not blurred across depth discontinuities). | [ ] |
| `p7_denoise_diff.png` | denoise=4 | abs(denoised - raw) * 10 | Shows where smoothing occurred. Should be strongest on flat surfaces, minimal at edges. | [ ] |

**Hypothesis if FAIL:**
- If still noisy: not enough passes, blur_beta too low, edge weights too low
- If over-blurred: too many passes, edges not detected properly
- If blocky artifacts: ping-pong texture issue, UV sampling wrong

---

### PHASE 8: Final Composited Render

**What:** The final lit scene with GTAO applied.

**Algorithm Step:** Lighting shader composites AO with scene

| Artifact | Debug Mode | Description | SUCCESS Criteria | APPROVED |
|----------|------------|-------------|------------------|----------|
| `p8_render.png` | default | Final render | AO subtly darkens corners and creases. No ugly dark splotches. No visible noise. Natural-looking depth enhancement. "Would you ship this?" = YES | [ ] |

**Hypothesis if FAIL:** AO strength too high, AO inverted in compositor, color space issues.

---

## Debug Mode Reference

### GTAO Shader Debug Modes (gtao_debug_mode)
- 0: Normal GTAO output
- 10: NDC depth (raw)
- 11: Viewspace linear depth (MIP 0)
- 12-15: Depth MIP levels 1-4
- 20: View-space normal.z
- 21: View-space normal.xy (needs implementation)
- 30: Screen-space radius
- 40: Packed edges (raw)
- 41: Raw depth differences
- 42: Edge divisor term
- 43: Unpacked edge values (min)
- 44: Inverted edges (edges = bright)

### Denoise Shader Debug Modes (denoise_debug_mode)
- 0: Normal denoised output
- 1: sum_weight / 8 (should be ~0.8-1.0 on smooth surfaces)
- 2: min(edges_c) after symmetry
- 3: blur_amount / 2
- 4: abs(output - input) * 10

### Lighting Shader Debug Modes (lighting_debug_mode)
- 0: Normal lit output
- 1: G-buffer normals
- 2: G-buffer depth
- 3: Albedo only
- 4: Shadow factor
- 5: AO only (grayscale passthrough of GTAO texture)

---

## Execution Commands

```bash
# Capture all debug screenshots
cargo run --example p20_gtao_test

# View results
open screenshots/gtao_test/

# Quick build check
cargo build --example p20_gtao_test
```

---

## Approval Log

| Phase | Date | Result | Notes |
|-------|------|--------|-------|
| 1 | | | |
| 2 | | | |
| 3 | | | |
| 4 | | | |
| 5 | | | |
| 6 | | | |
| 7 | | | |
| 8 | | | |

---

## Known Issues & Parameter Deviations

This section documents all known deviations from the XeGTAO reference implementation.

### Issue #1: Hardcoded Near Plane (MEDIUM)

**Status:** OPEN  
**Severity:** Medium - works for current scenes but not robust

**Problem:** The near clip plane is hardcoded to `0.1` in multiple places instead of being read from the camera's projection matrix.

**Locations:**
- `assets/shaders/gbuffer.wgsl:13` - `const NEAR_CLIP: f32 = 0.1;`
- `crates/studio_core/src/deferred/gtao_depth_prefilter.rs:116` - `let near = 0.1_f32;`

**XeGTAO Approach:** Extracts near plane from projection matrix dynamically.

**Impact:** If camera near plane changes, depth linearization will be incorrect.

**Fix Required:** Pass near plane as uniform from camera projection matrix.

---

### Issue #2: Edge Sensitivity Deviation (FIXED 2024-12-31)

**Status:** FIXED  
**Severity:** High - affects denoiser quality

**Problem:** Edge sensitivity was changed from XeGTAO's `0.011` to `0.025` with comment "to reduce edge over-detection." This made edges 2.3x LESS sensitive, allowing more blur where it shouldn't occur.

**Location:** `assets/shaders/gtao.wgsl:299`

**Fix Applied:** Reverted to XeGTAO default `0.011`.

---

### Issue #3: Effect Radius Deviation (FIXED 2024-12-31)

**Status:** FIXED  
**Severity:** Medium - affects AO appearance

**Problem:** Effect radius was set to `3.0` instead of XeGTAO default `0.5` (6x larger). This caused noise issues and required the edge_sensitivity hack.

**Location:** `crates/studio_core/src/deferred/gtao.rs:163`

**Fix Applied:** Reverted to XeGTAO default `0.5`. 

**Note:** Tested `1.0` which provides more visible AO for voxel scenes without noise. Can tune per-scene after baseline verification.

---

### Parameter Comparison Table (Phases 1-3)

| Parameter | XeGTAO Default | Our Value | Status |
|-----------|----------------|-----------|--------|
| **Phase 1: G-Buffer** |
| Near clip | From projection | `0.1` hardcoded | ISSUE #1 |
| Depth linearization | `near / ndc` | `near / (ε + ndc)` | OK (epsilon prevents div/0) |
| **Phase 2: Depth MIP** |
| `depth_unpack_consts.mul` | From projection | `0.1` hardcoded | ISSUE #1 |
| `depth_unpack_consts.add` | Small epsilon | `0.0001` | OK |
| `effect_radius` | `0.5` | `0.5` | FIXED |
| `effect_falloff_range` | `0.615` | `0.615` | OK |
| `radius_multiplier` | `1.457` | `1.457` | OK |
| **Phase 3: Edge Detection** |
| `edge_sensitivity` | `0.011` | `0.011` | FIXED |
| Edge formula | Standard | Standard | OK |
| Pack encoding | 2-bit/edge | 2-bit/edge | OK |

---

## Notes

- Do NOT proceed to next phase until current phase is APPROVED
- If a phase fails, document hypothesis and fix before re-testing
- All debug images must match SUCCESS criteria exactly
- "Close enough" is NOT approved
