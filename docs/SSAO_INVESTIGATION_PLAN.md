# SSAO Quality Investigation Plan

## Problem Statement
Our SSAO implementation produces visible noise/dithering patterns that look cheap and unprofessional compared to Bonsai's smooth SSAO output. We need to systematically investigate Bonsai's implementation and identify what we're doing wrong.

## Current State
- SSAO is implemented and functional
- Greedy meshing artifacts are fixed
- **Quality is unacceptable** - visible dithering/noise pattern
- Bilateral blur helps but doesn't solve the problem

---

## Phase 1: Bonsai Deep Dive

### 1.1 Locate and Read Bonsai SSAO Code
Files to examine:
```
bonsai/shaders/Ao.fragmentshader       # Main SSAO shader
bonsai/shaders/*blur*                  # Any blur shaders
bonsai/src/engine/render/render_init.cpp  # Kernel generation (lines 3-38 known)
bonsai/src/engine/render/render.cpp    # SSAO pass setup
```

### 1.2 Document Bonsai's Approach
Answer these questions:
1. What SSAO algorithm does Bonsai use? (SSAO, HBAO, GTAO, custom?)
2. How many samples per pixel?
3. What is the kernel distribution? (uniform, cosine-weighted, other?)
4. What is the sample radius in world/view units?
5. What bias value is used?
6. How is depth comparison done? (linear depth, Z-buffer, view-space Z?)
7. What is the range check formula?
8. Is there a separate blur pass? What kernel size?
9. Is temporal filtering used?
10. What noise texture is used? Size? Format?

### 1.3 Extract Key Code Snippets
Copy relevant shader code and kernel generation code into investigation notes.

---

## Phase 2: Detailed Comparison Table

Create comprehensive comparison:

| Aspect | Bonsai | Our Implementation | Notes |
|--------|--------|-------------------|-------|
| **Algorithm** | | Hemisphere sampling | |
| **Sample Count** | | 64 | |
| **Kernel Distribution** | | Cosine-weighted | |
| **Kernel Generation** | | Random hemisphere + scale | See ssao.rs |
| **Radius** | | 1.5 world units | |
| **Bias** | | 0.01 | |
| **Intensity** | | 2.5 | |
| **Noise Texture Size** | | 4x4 | |
| **Noise Content** | | Random rotation vectors | |
| **Depth Source** | | G-buffer position.w | Linear depth |
| **Normal Source** | | G-buffer normal (world-space) | |
| **View Transform** | | World -> View via matrix | |
| **Projection** | | View -> Clip for UV | |
| **Depth Compare** | | actual_z >= sample_z + bias | |
| **Range Check** | | smoothstep(0,1, r/diff) | |
| **Blur Pass** | | 3x3 bilateral inline | In lighting shader |
| **Blur Depth Threshold** | | exp(-diff * 5.0) | |
| **Output Format** | | R8Unorm | Single channel |
| **Temporal Filtering** | | None | |

---

## Phase 3: Hypothesis Testing

### Potential Causes of Noise

#### H1: Insufficient Samples
- Bonsai might use more samples
- Test: Increase to 128, 256 samples

#### H2: Poor Kernel Distribution
- Our kernel might cluster samples unevenly
- Test: Compare kernel visualization with Bonsai's

#### H3: Inadequate Blur
- Inline blur might not be enough
- Test: Add dedicated blur pass with larger kernel

#### H4: Wrong Noise Pattern
- 4x4 might tile visibly
- Test: Try 8x8 or different noise generation

#### H5: Incorrect Depth Comparison
- View-space Z comparison might be wrong
- Test: Try different comparison methods

#### H6: Parameter Mismatch
- Radius/bias/intensity might be wrong scale
- Test: Match Bonsai's exact parameters

#### H7: Missing Temporal Filtering
- Bonsai might accumulate over frames
- Test: Add temporal filter

---

## Phase 4: Implementation Fixes

Based on investigation findings, implement fixes in priority order:
1. Match Bonsai parameters exactly
2. Fix any algorithmic differences
3. Add proper blur pass if needed
4. Add temporal filtering if needed

---

## Success Criteria

SSAO is acceptable when:
1. No visible dithering/noise pattern at normal viewing distance
2. Smooth gradients in occluded areas
3. Clean edges at depth discontinuities
4. Performance within 1ms on target hardware
5. Looks comparable to Bonsai's output

---

## Files to Create/Modify

### Investigation
- `docs/research/bonsai-ssao-analysis.md` - Detailed Bonsai code analysis
- `docs/SSAO_INVESTIGATION_PLAN.md` - This document

### Implementation (after investigation)
- `assets/shaders/ssao.wgsl` - Updated SSAO shader
- `assets/shaders/ssao_blur.wgsl` - Dedicated blur shader (if needed)
- `crates/studio_core/src/deferred/ssao.rs` - Updated kernel generation
- `crates/studio_core/src/deferred/ssao_node.rs` - Updated uniforms/parameters
- `crates/studio_core/src/deferred/ssao_blur_node.rs` - Blur pass (if needed)

---

## Timeline

1. **Phase 1**: Bonsai analysis (1-2 hours)
2. **Phase 2**: Comparison table (30 min)
3. **Phase 3**: Hypothesis testing (1-2 hours per hypothesis)
4. **Phase 4**: Implementation (varies based on findings)

---

## Notes

The key insight from the reference SSAO code provided by the user:
```wgsl
// The reference uses:
// - View-space position reconstructed from depth
// - Kernel samples transformed via TBN matrix
// - Projection to find sample UV
// - Range check: smoothstep(0.0, 1.0, radius / abs(view_pos.z - sample_view_pos.z))
// - Compare: sample_view_pos.z >= sample_pos.z + bias
```

Our implementation follows this pattern but something is causing noise. The investigation should identify whether it's:
- Parameter values
- Kernel quality
- Blur quality
- Noise texture quality
- Some other algorithmic difference
