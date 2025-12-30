# GTAO Implementation Plan - 100% XeGTAO Compliance

## Remit

**There are NO acceptable differences between XeGTAO and our implementation.**

We will implement Intel's XeGTAO algorithm with **100% fidelity** to the reference. This means:
- All parameters use XeGTAO defaults (HIGH quality preset)
- All algorithm steps match XeGTAO exactly
- All pipeline stages exist (depth MIP, main pass, edge-aware denoise)
- No "simpler" approaches, no shortcuts, no "good enough"

If XeGTAO does it, we do it. Period.

Reference: https://github.com/GameTechDev/XeGTAO
Reference Files:
- `XeGTAO/Source/Rendering/Shaders/XeGTAO.hlsli` - Main algorithm
- `XeGTAO/Source/Rendering/Shaders/XeGTAO.h` - Constants and defaults

---

## Complete Difference Analysis

### Critical Architectural Differences

| # | Component | XeGTAO | Our Implementation | Status |
|---|-----------|--------|-------------------|--------|
| A1 | **Depth MIP Chain** | 5-level MIP pyramid with weighted average filter | Single depth texture | **MUST FIX** |
| A2 | **Edge-Aware Denoise** | XeGTAO_Denoise with packed edges, DenoiseBlurBeta=1.2 | 7x7 bilateral blur in lighting shader | **MUST FIX** |
| A3 | **Edge Packing** | Packed into R8 texture, XeGTAO_PackEdges/UnpackEdges | Not implemented | **MUST FIX** |
| A4 | **Separate Normals Pass** | Optional XeGTAO_ComputeViewspaceNormal pass | Using G-buffer normals | OK (XeGTAO supports both) |
| A5 | **Quality Presets** | Low/Medium/High/Ultra (1-3/2/3/9 slices, 2-3 steps) | Hardcoded 9 slices, 4 steps | **MUST FIX** |
| A6 | **Config System** | GtaoConfig not wired through | Hardcoded in gtao_node.rs:187-195 | **MUST FIX** |
| A7 | **Noise Index for TAA** | NoiseIndex = frameCounter % 64 | Static random noise | **MUST FIX** |

### Parameter Differences

| # | Parameter | XeGTAO Default | Our Value | File:Line | Status |
|---|-----------|---------------|-----------|-----------|--------|
| P1 | SLICE_COUNT (High) | 3 | 9 | gtao.wgsl:40 | **MUST FIX** |
| P2 | STEPS_PER_SLICE (High) | 3 | 4 | gtao.wgsl:41 | **MUST FIX** |
| P3 | effect_radius | Scene dependent | 3.0 | gtao_node.rs:187 | OK (tunable) |
| P4 | effect_falloff_range | 0.615 | 0.615 | gtao_node.rs:188 | OK |
| P5 | radius_multiplier | 1.457 | 1.457 | gtao_node.rs:192 | OK |
| P6 | final_value_power | 2.2 | 2.2 | gtao_node.rs:193 | OK |
| P7 | sample_distribution_power | 2.0 | 2.0 | gtao_node.rs:194 | OK |
| P8 | thin_occluder_compensation | 0.0 | 0.0 | gtao_node.rs:195 | OK |
| P9 | DenoiseBlurBeta | 1.2 (or 1e4 to disable) | N/A | - | **MUST FIX** |
| P10 | DepthMIPSamplingOffset | 3.30 | N/A (no MIPs) | - | **MUST FIX** |
| P11 | pixel_too_close_threshold | 1.3 | 1.3 | gtao.wgsl:217 | OK |
| P12 | projNormalLen fudge | 0.05 | 0.05 | gtao.wgsl:355 | OK |
| P13 | min visibility | 0.03 | 0.03 | gtao.wgsl:383 | OK |

### Algorithm Implementation Differences

| # | Component | XeGTAO Line | Our Line | Issue | Status |
|---|-----------|-------------|----------|-------|--------|
| I1 | Depth linearization | L112-117 | L109-113 | Different formula for Bevy reverse-Z | **VERIFY** |
| I2 | viewspaceZ precision | L281-284 | Not present | `viewspaceZ *= 0.99920` for FP16 | **MUST FIX** |
| I3 | MIP level selection | L438 | N/A | `mipLevel = clamp(log2(offset) - 3.3, 0, 5)` | **MUST FIX** |
| I4 | Thin occluder heuristic | L480-484 | Not present | `falloffBase = length(delta * (1 + thinOcc))` | **MUST FIX** |
| I5 | Occlusion term scale | L200,719 | Not present | `XE_GTAO_OCCLUSION_TERM_SCALE = 1.5` | **MUST FIX** |

---

## XeGTAO Quality Presets (Reference)

From XeGTAO.h and usage patterns:

| Quality | SliceCount | StepsPerSlice | Total Samples | Performance |
|---------|------------|---------------|---------------|-------------|
| Low | 1 | 2 | 4 | Fastest |
| Medium | 2 | 2 | 8 | Fast |
| **High** | **3** | **3** | **18** | **Target** |
| Ultra | 9 | 3 | 54 | Highest |

**We will implement HIGH preset as default (3 slices, 3 steps = 18 samples)**

---

## Implementation Phases

### Phase 1: Wire Config Through (Remove Hardcoding)

**Goal:** All parameters come from `GtaoConfig`, no hardcoded values.

**Tasks:**
1. Update `GtaoConfig` in `gtao.rs` to include ALL XeGTAO parameters:
   - `quality_level: u32` (0=Low, 1=Medium, 2=High, 3=Ultra)
   - `denoise_passes: u32` (0=disabled, 1=sharp, 2=medium, 3=soft)
   - `effect_radius: f32`
   - `radius_multiplier: f32` (default 1.457)
   - `falloff_range: f32` (default 0.615)
   - `sample_distribution_power: f32` (default 2.0)
   - `thin_occluder_compensation: f32` (default 0.0)
   - `final_value_power: f32` (default 2.2)
   - `depth_mip_sampling_offset: f32` (default 3.30)

2. Extract `GtaoConfig` in render world
3. Use extracted config in `gtao_node.rs` instead of hardcoded values
4. Pass quality preset to shader (slice count, steps per slice)

**Verification:** Build succeeds, GTAO still renders (visual unchanged)

### Phase 2: Implement Depth MIP Chain

**Goal:** Generate 5-level depth MIP pyramid exactly like XeGTAO.

**Tasks:**
1. Create new compute shader `gtao_depth_mip.wgsl` implementing `XeGTAO_PrefilterDepths16x16`
2. Create `GtaoDepthMipNode` render node that:
   - Takes hardware depth buffer as input
   - Outputs 5 MIP levels of linearized viewspace depth
   - Uses XeGTAO's weighted average filter for MIP generation
3. Update `GtaoPipeline` to use MIP chain texture
4. Update `gtao.wgsl` to sample from MIP chain based on sample offset distance

**Verification:** 
- DEBUG_MODE to visualize each MIP level
- Compare MIP values to reference implementation

### Phase 3: Update Main Pass for XeGTAO Compliance

**Goal:** Main GTAO pass matches XeGTAO exactly.

**Tasks:**
1. Update `gtao.wgsl`:
   - Add `viewspaceZ *= 0.99920` precision adjustment (L283)
   - Implement MIP level selection: `mipLevel = clamp(log2(offset) - depthMIPOffset, 0, 5)` (L438)
   - Implement thin occluder compensation heuristic (L480-484)
   - Add occlusion term scale (1.5) for proper packing (L200)
   - Use quality preset slice/step counts

2. Update shader uniforms to include:
   - `NoiseIndex` for TAA (frameCounter % 64 or 0)
   - `DepthMIPSamplingOffset` (default 3.30)

**Verification:**
- Raw AO output (DEBUG_MODE=101) matches reference quality
- No visible noise artifacts
- Proper falloff at edges

### Phase 4+5: Implement Edge-Aware Denoiser (COMPLETED)

**Goal:** Replace 7x7 bilateral blur with XeGTAO's edge-aware denoiser.

**Merged Phase 4 and Phase 5** since edge packing is required for the denoiser.

**Tasks (all completed):**
1. ✅ Add edge calculation to main pass: `XeGTAO_CalculateEdges` (L120-129)
2. ✅ Add edge packing: `XeGTAO_PackEdges` (L132-141)
3. ✅ Update main GTAO pass to output both AO and packed edges (MRT)
4. ✅ Create `gtao_denoise.wgsl` implementing `XeGTAO_Denoise` (L734-826):
   - `unpack_edges` (L686-696)
   - `add_sample` weighted averaging (L704-710)
   - Edge symmetry enforcement (L769-770)
   - AO leaking prevention for 3-4 edges (L772-776)
   - 3x3 kernel with diagonal weights (L785-788)
   - `DenoiseBlurBeta` parameter support
5. ✅ Create `GtaoDenoiseNode` render node (compute shader)
6. ✅ Remove 7x7 blur from `deferred_lighting.wgsl`
7. ✅ Update lighting node to use denoised texture
8. ✅ Wire denoiser into render graph after main GTAO pass

**Files created/modified:**
- `assets/shaders/gtao.wgsl` - Added edge calc, packing, MRT output
- `assets/shaders/gtao_denoise.wgsl` - NEW: XeGTAO denoiser compute shader
- `crates/studio_core/src/deferred/gtao_denoise.rs` - NEW: Denoise render node
- `crates/studio_core/src/deferred/gtao_node.rs` - MRT output (AO + edges)
- `crates/studio_core/src/deferred/gtao.rs` - Added ViewGtaoEdgesTexture
- `crates/studio_core/src/deferred/lighting_node.rs` - Uses ViewGtaoDenoised
- `crates/studio_core/src/deferred/labels.rs` - Added GtaoDenoise label
- `crates/studio_core/src/deferred/mod.rs` - Export gtao_denoise
- `crates/studio_core/src/deferred/plugin.rs` - Wire denoise node into graph
- `assets/shaders/deferred_lighting.wgsl` - Removed 7x7 blur

**Verification:**
- ✅ Build succeeds
- ✅ `cargo run --example p20_gtao_test` runs without errors
- Edge texture shows depth discontinuities
- Denoise respects edges (no blur across depth boundaries)

### Phase 6: TAA Noise Index Support

**Goal:** Support temporal noise distribution for TAA integration.

**Tasks:**
1. Add frame counter to render world
2. Pass `NoiseIndex = frameCounter % 64` to shader
3. Use Hilbert index for noise variation (optional optimization)

**Verification:**
- With TAA: no temporal artifacts
- Without TAA: still works (NoiseIndex = 0)

---

## XeGTAO Algorithm Re-Audit Checklist

After all fixes are complete, verify each line matches:

| # | Component | XeGTAO Line | Our Line | Verified |
|---|-----------|-------------|----------|----------|
| 1 | viewspaceZ precision | L281-284 | TBD | ⬜ |
| 2 | Falloff precompute: falloffMul | L315 | TBD | ⬜ |
| 3 | Falloff precompute: falloffAdd | L316 | TBD | ⬜ |
| 4 | Small radius fade | L342-343 | TBD | ⬜ |
| 5 | pixelTooCloseThreshold | L335 | TBD | ⬜ |
| 6 | minS calculation | L367 | TBD | ⬜ |
| 7 | sliceK with noise | L372 | TBD | ⬜ |
| 8 | phi = sliceK * PI | L374 | TBD | ⬜ |
| 9 | omega = (cosPhi, -sinPhi) | L377 | TBD | ⬜ |
| 10 | directionVec | L383 | TBD | ⬜ |
| 11 | orthoDirectionVec | L386 | TBD | ⬜ |
| 12 | axisVec | L390 | TBD | ⬜ |
| 13 | projectedNormalVec | L396 | TBD | ⬜ |
| 14 | signNorm | L399 | TBD | ⬜ |
| 15 | projectedNormalVecLength | L402 | TBD | ⬜ |
| 16 | cosNorm | L403 | TBD | ⬜ |
| 17 | n = signNorm * FastACos | L406 | TBD | ⬜ |
| 18 | lowHorizonCos0/1 | L409-410 | TBD | ⬜ |
| 19 | horizonCos0/1 init | L413-414 | TBD | ⬜ |
| 20 | R1 stepBaseNoise | L420 | TBD | ⬜ |
| 21 | stepNoise = frac | L421 | TBD | ⬜ |
| 22 | s = (step+noise)/steps | L424 | TBD | ⬜ |
| 23 | s = pow(s, power) | L427 | TBD | ⬜ |
| 24 | s += minS | L430 | TBD | ⬜ |
| 25 | sampleOffset = s * omega | L433 | TBD | ⬜ |
| 26 | MIP level selection | L438 | TBD | ⬜ |
| 27 | Snap to pixel center | L442 | TBD | ⬜ |
| 28 | Sample positive direction | L458-460 | TBD | ⬜ |
| 29 | Sample negative direction | L462-464 | TBD | ⬜ |
| 30 | sampleDelta calculation | L466-467 | TBD | ⬜ |
| 31 | sampleDist = length | L468-469 | TBD | ⬜ |
| 32 | sampleHorizonVec | L472-473 | TBD | ⬜ |
| 33 | Falloff weight (no thin occ) | L477-478 | TBD | ⬜ |
| 34 | Thin occluder falloff | L481-484 | TBD | ⬜ |
| 35 | shc = dot(horizonVec, viewVec) | L488-489 | TBD | ⬜ |
| 36 | shc = lerp(lowHorizon, shc, weight) | L492-493 | TBD | ⬜ |
| 37 | horizonCos = max | L505-506 | TBD | ⬜ |
| 38 | projNormalLen fudge | L532 | TBD | ⬜ |
| 39 | h0 = -FastACos(horizonCos1) | L536 | TBD | ⬜ |
| 40 | h1 = FastACos(horizonCos0) | L537 | TBD | ⬜ |
| 41 | iarc formula | L542-543 | TBD | ⬜ |
| 42 | localVisibility | L544 | TBD | ⬜ |
| 43 | visibility /= sliceCount | L556 | TBD | ⬜ |
| 44 | visibility = pow | L557 | TBD | ⬜ |
| 45 | visibility = max(0.03, ...) | L558 | TBD | ⬜ |
| 46 | Edge calculation | L120-129 | TBD | ⬜ |
| 47 | Edge packing | L132-141 | TBD | ⬜ |
| 48 | Denoise algorithm | L704-826 | TBD | ⬜ |

---

## Stack-Ranked Implementation Order

**Priority 1 - Architectural (blocking):**
1. Wire GtaoConfig through (Phase 1)
2. Implement depth MIP chain (Phase 2)
3. Add viewspaceZ precision (Phase 3)

**Priority 2 - Algorithm correctness:**
4. Implement MIP level sampling (Phase 3)
5. Implement thin occluder compensation (Phase 3)
6. Update to HIGH quality preset (3 slices, 3 steps)

**Priority 3 - Denoiser:**
7. Implement edge packing (Phase 5)
8. Implement edge-aware denoiser (Phase 4)
9. Remove 7x7 blur from lighting

**Priority 4 - Polish:**
10. TAA noise index support (Phase 6)
11. Full re-audit checklist verification

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/studio_core/src/deferred/gtao.rs` | Expand GtaoConfig, add extraction |
| `crates/studio_core/src/deferred/gtao_node.rs` | Use config, add MIP/denoise nodes |
| `crates/studio_core/src/deferred/mod.rs` | Register new nodes |
| `assets/shaders/gtao.wgsl` | Main pass updates |
| `assets/shaders/gtao_depth_mip.wgsl` | NEW: depth MIP compute shader |
| `assets/shaders/gtao_denoise.wgsl` | NEW: edge-aware denoiser |
| `assets/shaders/deferred_lighting.wgsl` | Remove 7x7 blur |

---

## Quality Gates

Before marking ANY phase complete:

1. **Build check:** `cargo build` succeeds
2. **Run check:** `cargo run --example p20_gtao_test` runs without panic
3. **Visual check:** DEBUG_MODE outputs match expectations
4. **Audit check:** Relevant checklist items verified against XeGTAO source

**NO PHASE IS COMPLETE UNTIL ALL GATES PASS.**
