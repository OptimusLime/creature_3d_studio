# Voxel Shading Techniques

## Current State

Our deferred pipeline now correctly implements basic directional lighting:
- N·L (normal dot light) calculation per fragment
- Fill light from opposite direction
- Ambient base illumination
- Emission for glowing voxels

**Problem**: All faces pointing the same direction have identical shading, making it hard to distinguish individual blocks in large flat areas (e.g., grass plains).

## Why Minecraft Blocks Are Distinguishable

Minecraft uses several techniques beyond basic N·L lighting:

### 1. Face-Based Shading Multipliers (Simplest)

Minecraft applies fixed brightness multipliers per face direction:
- Top (+Y): 1.0 (full brightness)
- Bottom (-Y): 0.5
- North/South (±Z): 0.8
- East/West (±X): 0.6

This is baked into the vertex colors or applied in the shader. It's NOT physically accurate but gives immediate visual distinction between faces.

**Implementation**: In `deferred_lighting.wgsl`, after N·L calculation:
```wgsl
// Minecraft-style face shading
var face_multiplier = 1.0;
if (abs(world_normal.y) > 0.9) {
    face_multiplier = select(0.5, 1.0, world_normal.y > 0.0); // top=1.0, bottom=0.5
} else if (abs(world_normal.z) > 0.9) {
    face_multiplier = 0.8; // north/south
} else {
    face_multiplier = 0.6; // east/west
}
final_color *= face_multiplier;
```

### 2. Ambient Occlusion (AO)

AO darkens areas where geometry is close together:
- Corners where 3 blocks meet → darkest
- Edges where 2 blocks meet → dark
- Open faces → brightest

**Implementation approaches**:

a) **Per-vertex AO** (Minecraft's approach):
   - During mesh generation, check neighboring voxels
   - For each vertex, count how many of the 3 adjacent corners are solid
   - Store AO value (0-3) as vertex attribute
   - Interpolate across face for smooth gradients

b) **Screen-space AO (SSAO)**:
   - Post-process pass reading depth/normal buffer
   - Sample nearby depths to detect occlusion
   - More expensive but works with any geometry

### 3. Shadow Mapping

Cast shadows from the sun:
- Render scene from sun's perspective to depth texture
- During lighting, check if fragment is in shadow
- Soft shadows via PCF (percentage closer filtering)

**Complexity**: Requires additional render pass, shadow map texture, bias handling.

### 4. Subtle Noise/Variation

Some games add slight per-block variation:
- Random brightness offset per block
- Texture variation
- Edge highlighting

## Recommended Implementation Order

### Phase 10.8: Minecraft-style Face Shading (Quick Win)
Add fixed multipliers per face direction in the lighting shader.
**Effort**: 10 minutes, immediate visual improvement.

### Phase 11.5: Per-Vertex Ambient Occlusion
Modify `build_chunk_mesh()` to calculate AO per vertex.
**Effort**: 2-4 hours, significant visual improvement.

### Phase 14: Shadow Mapping (Future)
Full shadow map implementation for sun shadows.
**Effort**: 1-2 days, requires new render pass infrastructure.

## References

- [0fps: Ambient Occlusion for Minecraft-like Worlds](https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/)
- [Minecraft Wiki: Lighting](https://minecraft.wiki/w/Light)
- Bonsai's approach: Uses SSAO + shadow mapping for high-quality results
