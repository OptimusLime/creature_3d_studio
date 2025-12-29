# Bonsai Rendering Pipeline Analysis

This document analyzes Bonsai's rendering approach as our reference implementation.

## Bonsai's Deferred Rendering Pipeline

### G-Buffer Layout

Bonsai uses a classic deferred rendering setup with these G-buffer textures:

| Texture | Format | Contents |
|---------|--------|----------|
| gColor | RGBA16F | RGB = albedo, A = emission |
| gNormal | RGBA16F | RGB = world-space normal |
| gPosition | RGBA32F | XYZ = world position, W = linear depth |

**Our implementation matches this exactly.**

### Lighting Pass

From `Lighting.fragmentshader`:
- Sun directional light with hardcoded direction
- Distance-based fog (exponential falloff)
- Emission bypass (high emission = skip lighting, go straight to output)
- Ambient term for unlit areas

Key fog calculation (lines 306-319):
```glsl
float fogAmount = 1.0 - exp(-distance * fogDensity);
vec3 finalColor = mix(litColor, fogColor, fogAmount);
```

### Bloom Pipeline

From `bloom_downsample.fragmentshader` and `bloom_upsample.fragmentshader`:

1. **Threshold**: Extract pixels above brightness threshold
2. **Downsample**: 13-tap filter, halving resolution each pass (6 mip levels)
3. **Upsample**: Tent filter, doubling resolution, additive blend
4. **Composite**: Add bloom to final image

### Vertex Material System

From `mesh.h` (lines 701-793):

Bonsai packs material data into vertex attributes:
```cpp
struct vertex_material {
    v3 Color;       // RGB color
    f32 Emission;   // 0-1 emission intensity
    // Transparency handled separately via flags
};
```

**Our `VoxelMaterial` with custom vertex attributes mirrors this.**

## Voxel World Structure

### Chunks

Bonsai uses a chunk-based world:
- Chunk size: 32x32x32 voxels (configurable)
- Chunks loaded/unloaded based on camera distance
- Each chunk generates a single mesh

### Mesh Generation

From the voxel mesher:
1. Iterate all filled voxels
2. For each voxel, check 6 neighbors
3. Only generate face if neighbor is empty (face culling)
4. Group adjacent same-material faces (greedy meshing)

**We haven't implemented face culling or greedy meshing yet.**

### Scene Graph

Bonsai supports:
- Multiple chunks at different positions
- Entity placement within chunks
- Dynamic voxel modification

## What Bonsai Has That We Don't (Yet)

| Feature | Bonsai | Us | Priority |
|---------|--------|-----|----------|
| G-buffer rendering | Yes | Yes | Done |
| Deferred lighting | Yes | Yes | Done |
| Distance fog | Yes | Yes | Done |
| Emission | Yes | Yes | Done |
| Bloom | Yes | No | Phase 10 |
| Face culling | Yes | No | High |
| Greedy meshing | Yes | No | High |
| Multiple chunks | Yes | No | Medium |
| SSAO | Yes | No | Low |
| Shadows | Yes | No | Low |
| Transparency (OIT) | Yes | No | Low |

## Visual Reference

Bonsai's signature look:
- Dark void background (black or deep purple)
- Glowing emission on edges/highlights
- Bloom halo around bright objects
- Fog fading distant objects to background color
- Chunky voxel aesthetic with hard edges

## Key Shader Files

| File | Purpose |
|------|---------|
| `gBuffer.fragmentshader` | G-buffer output |
| `Lighting.fragmentshader` | Deferred lighting + fog |
| `bloom_downsample.fragmentshader` | Bloom mip generation |
| `bloom_upsample.fragmentshader` | Bloom reconstruction |
| `composite.fragmentshader` | Final compositing |

## Porting Notes

### Differences from Bonsai

1. **API**: Bonsai uses OpenGL, we use wgpu/WebGPU
2. **Coordinate system**: Bonsai is Y-up, we're Y-up (same)
3. **Depth**: Bonsai uses standard depth, we use reverse-Z (better precision)
4. **Asset format**: Bonsai has custom formats, we use Bevy's asset system

### Things We Changed

1. **Fog color**: We use purple (0.102, 0.039, 0.180) for the 80s aesthetic
2. **Reverse-Z depth**: Better depth precision for large scenes
3. **Bevy integration**: Using Bevy's mesh system, not custom buffers
