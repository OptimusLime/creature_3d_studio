# Greedy Meshing AO Interpolation Bug

## Discovery Date
2024-12-29

## Status: CRITICAL BUG - NEEDS FIX

## Summary
Greedy meshing causes severe ambient occlusion (AO) interpolation artifacts that appear as dark streaks/bands extending from objects across large merged floor/wall quads. This is a **fundamental flaw** in our current vertex-based AO approach when combined with greedy meshing.

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

### Example
A 16x16 floor gets merged into one quad. A pillar at position (8, 1, 8) sits on top of voxel (8, 0, 8).
- The floor quad vertex at (8, 0, 8) gets dark AO because the pillar is adjacent
- The floor quad vertices at (0, 0, 0), (16, 0, 0), (0, 0, 16), (16, 0, 16) get bright AO
- Interpolation creates a dark streak radiating from the pillar base across the entire floor

## Affected Systems
- Any scene using greedy meshing (enabled by default)
- Most visible on large flat surfaces (floors, walls, ceilings)
- The `p19_dual_moon_shadows` and `p17_chunk_streaming` examples clearly demonstrate this bug

## Current Workaround
Disable greedy meshing (sacrifices mesh optimization):
```rust
VoxelWorldApp::new("My Scene")
    .with_greedy_meshing(false)
    // ...
```

Examples `p17_chunk_streaming` and `p19_dual_moon_shadows` have been updated to disable greedy meshing as a temporary fix.

---

## Fix Options Analysis

### Option 1: Don't merge quads with differing AO values
During greedy mesh merging, check if all vertices would have the same AO value. Only merge if AO is uniform across the potential merged region.

**Pros**: Simple logic, preserves AO accuracy
**Cons**: Reduces mesh optimization effectiveness near geometry - defeats much of the purpose of greedy meshing

### Option 2: Use flat shading for AO on merged quads
Store AO per-face rather than per-vertex for greedy meshed quads. Use `flat` interpolation qualifier in the shader.

**Pros**: No interpolation artifacts
**Cons**: Loses smooth AO gradients, may look blocky - Minecraft-style but less refined

### Option 3: Screen-Space Ambient Occlusion (SSAO) - RECOMMENDED
Remove vertex AO entirely and compute ambient occlusion in a post-process pass using depth/normal buffers.

**Pros**: 
- No mesh-dependent artifacts
- Modern industry-standard approach
- Works regardless of mesh topology
- Can look better than vertex AO with proper tuning
- Already have depth/normal G-buffers from deferred pipeline

**Cons**: 
- More GPU cost (additional render pass)
- Requires careful tuning of sample radius, bias, etc.
- May need temporal filtering for noise reduction

### Option 4: Break merged quads at AO discontinuities
When a merged quad would span vertices with different AO values, split it into smaller quads along the discontinuity boundaries.

**Pros**: Preserves optimization where possible, accurate AO
**Cons**: Complex implementation, may not reduce vertex count much in practice

---

## Recommended Approach: SSAO

**We should study how Bonsai handles ambient occlusion before implementing a fix.**

Bonsai is a mature voxel renderer that likely faced similar issues. Key questions to research:

1. Does Bonsai use vertex AO, SSAO, or both?
2. How does Bonsai handle AO with greedy meshing?
3. What SSAO algorithm does Bonsai use (if any)?
4. What are Bonsai's performance characteristics for AO?

### Why SSAO is the Right Solution

1. **Decouples AO from mesh topology** - No more interpolation artifacts regardless of how we mesh
2. **We already have the infrastructure** - Our deferred pipeline has depth and normal G-buffers
3. **Industry standard** - Every modern game engine uses SSAO
4. **Future-proof** - Works with any mesh optimization we add later
5. **Better quality potential** - Can capture occlusion that vertex AO misses

### SSAO Implementation Plan (after Bonsai research)

1. Research Bonsai's AO approach in `docs/research/`
2. Choose SSAO algorithm (HBAO, GTAO, or simpler)
3. Add SSAO compute/render pass after G-buffer, before lighting
4. Remove vertex AO from mesh generation (or keep as fallback)
5. Tune SSAO parameters for voxel aesthetics
6. Add quality settings (low/medium/high)

---

## Files Involved
- `crates/studio_core/src/voxel_mesh.rs` - Current vertex AO calculation
- `assets/shaders/gbuffer.wgsl` - AO passed through from vertex to fragment
- `assets/shaders/deferred_lighting.wgsl` - AO applied to final lighting (line 1648)
- `crates/studio_core/src/deferred/plugin.rs` - Where SSAO pass would be added

## Test Cases

### p19_dual_moon_shadows
Run `cargo run --example p19_dual_moon_shadows` with and without `.with_greedy_meshing(false)` to see the difference.
- With greedy: Black north-south streaks on floor from pillar bases
- Without greedy: Clean shadows, only moon shadows visible

### p17_chunk_streaming  
Run `cargo run --example p17_chunk_streaming` - the bug is extremely visible here:
- With greedy: Dark diagonal streaks radiating from every pillar, stair-step AO patterns across entire terrain
- Without greedy: Clean terrain with proper localized AO at geometry edges only

## Impact
This bug affects ALL scenes using greedy meshing (the default). It makes the lighting look incorrect and muddy, with phantom shadows appearing everywhere. This is a **critical visual bug** that should be fixed before any release.

## Related Issues
- This IS the cause of the "cross-chunk shadow streaks" documented in `CHUNK_SHADOW_STREAKS_DEBUG.md` - confirmed.
- Bonsai research: `docs/research/bonsai-pipeline-analysis.md`

## Next Steps

### 1. Study Bonsai SSAO Implementation
Bonsai uses SSAO via `Ao.fragmentshader` (see `docs/bonsai-analysis.md:29` and `docs/bonsai-pipeline-analysis.md:244`).

Key points from existing analysis:
- Input: gNormal, depth buffers
- 32-sample hemisphere kernel
- Blur pass for noise reduction  
- Applied as: `TotalLight *= AO` (multiplicative)

**Action**: Clone/examine Bonsai repo to get the full `Ao.fragmentshader` implementation details.

### 2. Implement SSAO Pass
Based on `docs/bonsai-pipeline-analysis.md` Phase 12 plan:
1. Create `ssao.wgsl` shader (port from Bonsai's `Ao.fragmentshader`)
2. Add SSAO render pass between G-buffer and Lighting passes
3. Create SSAO texture (single channel, blurred)
4. Sample SSAO in lighting pass instead of vertex AO

### 3. Remove Vertex AO System
Once SSAO is working:
1. Remove `voxel_ao` vertex attribute from mesh generation
2. Remove AO from G-buffer normal.a channel
3. Update gbuffer.wgsl to not pass AO
4. Simplify vertex layout

### 4. Re-enable Greedy Meshing
After SSAO works:
1. Re-enable `use_greedy_meshing: true` as default
2. Update examples to remove workaround comments
3. Verify no AO artifacts with greedy meshing + SSAO
4. Performance test: greedy meshing should now be safe AND performant

### 5. Performance Tuning
- Add SSAO quality settings (sample count, radius)
- Consider temporal filtering for noise reduction
- Profile GPU cost and optimize if needed
