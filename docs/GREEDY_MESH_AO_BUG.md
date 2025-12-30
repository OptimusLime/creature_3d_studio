# Greedy Meshing AO Interpolation Bug

## Discovery Date
2024-12-29

## Status: RESOLVED - Using Screen-Space AO (GTAO)

## Summary
Greedy meshing caused severe ambient occlusion (AO) interpolation artifacts when using vertex-based AO. The solution is to use screen-space ambient occlusion instead, which computes AO per-pixel rather than per-vertex.

## Original Problem
When greedy meshing merges multiple voxel faces into a single large quad:
1. Each vertex of the merged quad gets an AO value based on its immediate voxel neighbors
2. If one corner is adjacent to geometry, that corner gets dark AO
3. GPU interpolates AO across the entire quad surface
4. This creates streaks/gradients across large merged surfaces

## Resolution
**Implemented GTAO (Ground Truth Ambient Occlusion)** based on Intel's XeGTAO algorithm.

Screen-space AO computes occlusion per-pixel by sampling the depth buffer, completely avoiding the vertex interpolation problem.

See `docs/GTAO_IMPLEMENTATION_PLAN.md` for implementation details.

## Relevant Files
- `assets/shaders/gtao.wgsl` - GTAO shader
- `crates/studio_core/src/deferred/gtao.rs` - GTAO resources
- `crates/studio_core/src/deferred/gtao_node.rs` - GTAO render node
