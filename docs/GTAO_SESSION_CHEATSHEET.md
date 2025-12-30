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

**Current state:** Basic GTAO works but has architectural gaps vs XeGTAO.

**Critical gaps to fix:**
- ~~No depth MIP chain (XeGTAO uses 5-level)~~ **DONE**
- ~~Config not wired through~~ **DONE**
- Main pass needs to sample from MIP chain (currently uses hardware depth)
- Wrong denoiser (we use 7x7 blur, XeGTAO uses edge-aware)
- Missing thin occluder compensation

---

## Key Files

| File | Purpose |
|------|---------|
| `assets/shaders/gtao.wgsl` | Main GTAO shader |
| `assets/shaders/gtao_depth_prefilter.wgsl` | Depth MIP chain compute shader |
| `crates/studio_core/src/deferred/gtao.rs` | Config struct (now wired through) |
| `crates/studio_core/src/deferred/gtao_node.rs` | GTAO render node |
| `crates/studio_core/src/deferred/gtao_depth_prefilter.rs` | Depth prefilter render node |
| `assets/shaders/deferred_lighting.wgsl` | Has 7x7 blur (L84-138) - MUST REMOVE |
| `XeGTAO/Source/Rendering/Shaders/XeGTAO.hlsli` | Reference implementation |

---

## Current Progress

| Phase | Task | Status |
|-------|------|--------|
| 0 | Document differences | DONE |
| 0 | Write implementation plan | DONE |
| 1 | Wire GtaoConfig through | **DONE** |
| 2 | Implement depth MIP chain | **DONE** |
| 3 | Main pass XeGTAO compliance | TODO |
| 4 | Edge-aware denoiser | TODO |
| 5 | Edge packing | TODO |
| 6 | TAA noise index | TODO |

---

## Next Step

**Phase 3: Main pass XeGTAO compliance**

Update main GTAO shader to sample from the depth MIP chain and match XeGTAO behavior.

Tasks:
1. Update get_viewspace_depth() to sample from depth_mip0 instead of hardware depth
2. Add MIP level selection based on sample distance (XeGTAO_GetDepthMipLevel)
3. Add thin occluder compensation heuristic
4. Verify viewspaceZ precision matches fp16 range

Reference: `XeGTAO.hlsli` L454-462 (MIP sampling), L475-498 (thin occluder)

Test: `cargo run --example p20_gtao_test`

---

## Documents to Update on Progress

When you complete work, update these:

1. **`docs/GTAO_IMPLEMENTATION_PLAN.md`** - Mark phases complete, update checklist
2. **`docs/GTAO_SESSION_CHEATSHEET.md`** (this file) - Update "Current Progress" table
3. **Git commits** - One per logical change, descriptive messages

---

## Quick Commands

```bash
# Build and run GTAO test
cargo run --example p20_gtao_test

# Debug modes in deferred_lighting.wgsl:
# DEBUG_MODE = 0    Full lighting with GTAO
# DEBUG_MODE = 100  GTAO with blur
# DEBUG_MODE = 101  Raw GTAO (no blur) - best for checking quality

# Debug modes in gtao.wgsl:
# DEBUG_GTAO = 0    Normal output
# DEBUG_GTAO = 1    NDC depth
# DEBUG_GTAO = 2    Normal.z
# DEBUG_GTAO = 3    Linear depth
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
