# Greedy Meshing AO Interpolation Bug

## Discovery Date
2024-12-29

## Status: SSAO IMPLEMENTED - QUALITY ISSUES REMAIN

## Summary
Greedy meshing causes severe ambient occlusion (AO) interpolation artifacts that appear as dark streaks/bands extending from objects across large merged floor/wall quads. This is a **fundamental flaw** in our current vertex-based AO approach when combined with greedy meshing.

**Update 2024-12-30**: SSAO has been implemented to replace vertex AO. The greedy meshing artifacts are fixed, but the SSAO implementation has significant noise/dithering quality issues that need investigation.

## Symptoms
- Dark streaks/shadows appearing in directions unrelated to light sources
- Streaks extend from pillars, walls, or any geometry adjacent to large flat surfaces
- Most visible on floors where many voxels get merged into large quads
- Appears as a "third shadow direction" that doesn't match any light in the scene
- Stair-step dark patterns across terrain in chunk streaming scenes

## Root Cause
When greedy meshing merges multiple voxel faces into a single large quad:

1. Each vertex of the merged quad gets an AO value based on its immediate voxel neighbors
2. If one corner of the merged quad is adjacent to a pillar/wall, that corner gets a dark AO value (e.g., 0.4)
3. The other corners far from the pillar get bright AO values (1.0)
4. GPU interpolates AO across the entire quad surface
5. This creates a gradient/streak from the dark corner across the entire merged surface

---

## SSAO Implementation Status

### What We Built
1. **SSAO Pass** - Runs after G-buffer, before lighting
2. **64-sample hemisphere kernel** - Cosine-weighted distribution
3. **View-space transforms** - World positions/normals converted to view space
4. **4x4 noise texture** - Random rotation vectors for kernel rotation
5. **Bilateral blur** - In lighting shader to reduce noise

### Current Quality Issues: SEVERE NOISE/DITHERING

The SSAO output has visible noise patterns that look "cheap" and "dog shit". The blur pass reduces but does not eliminate this.

**Symptoms:**
- Visible dithering pattern in AO
- Grainy appearance especially at distance
- Not the smooth, subtle AO seen in professional implementations like Bonsai

### Root Cause Investigation Needed

We need to compare our implementation against Bonsai's to understand:
1. Why Bonsai's SSAO looks smooth and ours looks noisy
2. What parameters Bonsai uses (radius, bias, sample count, kernel distribution)
3. How Bonsai's blur pass works (separate pass vs inline, kernel size)
4. Any additional techniques Bonsai uses (temporal filtering, depth-aware blur, etc.)

---

## Next Steps: Bonsai SSAO Investigation

### Phase 1: Bonsai Code Analysis
Examine the following Bonsai files:
- `shaders/Ao.fragmentshader` - Main SSAO shader
- `src/engine/render/render_init.cpp` - SSAO kernel generation (lines 3-38)
- Any blur pass shaders
- Uniform/parameter definitions

### Phase 2: Detailed Comparison Table
Create a table comparing:

| Aspect | Bonsai | Our Implementation |
|--------|--------|-------------------|
| Sample count | ? | 64 |
| Kernel distribution | ? | Cosine-weighted hemisphere |
| Radius (world units) | ? | 1.5 |
| Bias | ? | 0.01 |
| Intensity | ? | 2.5 |
| Noise texture size | ? | 4x4 |
| Blur pass | ? | 3x3 bilateral inline |
| Depth comparison method | ? | View-space Z comparison |
| Range check | ? | smoothstep(0, 1, radius / depth_diff) |

### Phase 3: Systematic Testing
1. Match Bonsai's parameters exactly
2. Test each parameter in isolation
3. Identify which differences cause the noise

---

## Files Involved

### SSAO Implementation
- `assets/shaders/ssao.wgsl` - SSAO shader (hemisphere sampling)
- `crates/studio_core/src/deferred/ssao.rs` - Kernel generation, texture prep
- `crates/studio_core/src/deferred/ssao_node.rs` - Render node, uniforms, noise texture
- `crates/studio_core/src/deferred/plugin.rs` - Render graph integration

### Lighting Integration
- `assets/shaders/deferred_lighting.wgsl` - SSAO sampling + blur (group 6)
- `crates/studio_core/src/deferred/lighting_node.rs` - SSAO bind group

### Bonsai Reference
- `bonsai/shaders/Ao.fragmentshader` - Reference SSAO implementation
- `bonsai/src/engine/render/render_init.cpp` - Kernel initialization

---

## Test Cases

### p19_dual_moon_shadows
- **Before SSAO**: Black north-south streaks on floor from pillar bases
- **After SSAO**: Streaks gone, but noisy AO around geometry

### p17_chunk_streaming
- **Before SSAO**: Dark diagonal streaks from every pillar
- **After SSAO**: Streaks gone, noisy AO visible on stepped pillars

### p15_greedy_mesh
- Single 8x8x8 cube with greedy meshing
- Good test for large flat faces + corner AO

---

## Related Documentation
- `docs/research/bonsai-pipeline-analysis.md` - Initial Bonsai analysis
- `docs/bonsai-analysis.md` - Bonsai overview
- `docs/CHUNK_SHADOW_STREAKS_DEBUG.md` - Original symptom investigation
