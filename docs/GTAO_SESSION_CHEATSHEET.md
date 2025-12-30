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

**Current state:** Phases 1-5 complete and **AUDITED**. Edge-aware denoiser passes line-by-line audit.

**Remaining work:**
- Phase 6: TAA noise index support (Hilbert curve + R2 sequence)
- Final audit: line-by-line comparison of main pass with XeGTAO.hlsli

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
| 4+5 | Edge-aware denoiser + edge packing | **DONE** |
| 6 | TAA noise index | TODO (optional) |

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

## Next Step

**Phase 6: TAA Noise Index Support**

Replace texture-based noise with XeGTAO's Hilbert curve + R2 sequence.

Tasks:
1. Add frame counter to render world extraction
2. Pass `NoiseIndex = frameCounter % 64` to shader uniforms
3. Implement `hilbert_index()` function (XeGTAO.h L120-142)
4. Implement `spatio_temporal_noise()` (vaGTAO.hlsl L74-91)
5. Replace noise texture sampling in gtao.wgsl

Reference files to read:
- `XeGTAO/Source/Rendering/Shaders/XeGTAO.h` L120-142 (HilbertIndex)
- `XeGTAO/Source/Rendering/Shaders/vaGTAO.hlsl` L74-91 (SpatioTemporalNoise)

---

## Quick Commands

```bash
# Build and run GTAO test
cargo run --example p20_gtao_test

# Debug modes in deferred_lighting.wgsl:
# DEBUG_MODE = 0    Full lighting with denoised GTAO
# DEBUG_MODE = 100  Show denoised GTAO only
# DEBUG_MODE = 101  Raw GTAO (center sample, no blur)

# Debug modes in gtao.wgsl:
# DEBUG_GTAO = 0    Normal output (AO + edges)
# DEBUG_GTAO = 1    NDC depth
# DEBUG_GTAO = 2    Normal.z
# DEBUG_GTAO = 3    Linear depth
# DEBUG_GTAO = 4    Packed edges visualization
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
