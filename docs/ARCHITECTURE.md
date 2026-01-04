# Architecture

## Overview

Creature 3D Studio is a voxel-based 3D rendering engine built on Bevy 0.17. It implements a **fully custom deferred rendering pipeline** inspired by Bonsai, targeting an 80s dark fantasy aesthetic with colored lighting, bloom, and shadows.

## Render Pipeline

```
                          RENDER GRAPH FLOW
                                
┌─────────────────────────────────────────────────────────────────────┐
│                         SHADOW PASS                                  │
│  ShadowPassNode renders all DeferredRenderable meshes               │
│  from light's perspective to depth-only texture                     │
│  Output: shadow_depth (2048x2048 Depth32Float)                      │
└─────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        G-BUFFER PASS                                 │
│  GBufferPassNode renders all DeferredRenderable meshes              │
│  to Multiple Render Targets (MRT)                                    │
│                                                                      │
│  Outputs:                                                            │
│  ├─ gColor (Rgba16Float): RGB = albedo, A = emission                │
│  ├─ gNormal (Rgba16Float): RGB = world normal, A = ambient occlusion│
│  └─ gPosition (Rgba32Float): XYZ = world position, W = linear depth │
└─────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       LIGHTING PASS                                  │
│  LightingPassNode samples G-buffer and computes lighting            │
│                                                                      │
│  Features:                                                           │
│  ├─ Dual moon directional lights (purple + orange)                  │
│  ├─ Point lights (up to 256 via storage buffer)                     │
│  ├─ Shadow mapping with PCF soft shadows                            │
│  ├─ Minecraft-style face shading                                     │
│  ├─ Per-vertex ambient occlusion                                     │
│  ├─ Emission with color preservation                                 │
│  └─ Distance fog                                                     │
│                                                                      │
│  Output: HDR lit scene to ViewTarget                                │
└─────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         BLOOM PASS                                   │
│  BloomNode applies multi-pass bloom post-processing                 │
│                                                                      │
│  1. Downsample (6 mip levels, 13-tap filter)                        │
│  2. Upsample (tent filter, ping-pong between mip_a/mip_b)           │
│  3. Composite: scene + bloom * intensity                             │
│  4. Hybrid tone mapping (ACES + Reinhard for color preservation)    │
│                                                                      │
│  Output: Final LDR image                                             │
└─────────────────────────────────────────────────────────────────────┘
```

## Key Components

### Voxel Data (`studio_core/src/voxel.rs`)

```rust
pub struct Voxel {
    pub color: [u8; 3],   // RGB 0-255
    pub emission: u8,      // 0 = no glow, 255 = full glow
}

pub struct VoxelChunk {
    voxels: Box<[Option<Voxel>; 32*32*32]>,  // 32x32x32 dense array
}
```

### Vertex Format (`studio_core/src/voxel_mesh.rs`)

```
Position (12 bytes) + Normal (12 bytes) + Color (12 bytes) + Emission (4 bytes) + AO (4 bytes) = 44 bytes/vertex
```

Custom vertex attributes:
- `ATTRIBUTE_VOXEL_COLOR`: Per-vertex RGB color
- `ATTRIBUTE_VOXEL_EMISSION`: Per-vertex emission intensity (0-1)
- `ATTRIBUTE_VOXEL_AO`: Per-vertex ambient occlusion (0-1, computed at mesh generation)

### Point Lights (`studio_core/src/deferred/point_light.rs`)

```rust
pub struct DeferredPointLight {
    pub color: Color,      // Light color
    pub intensity: f32,    // Brightness multiplier
    pub radius: f32,       // Maximum range (smooth falloff)
}
```

Point lights are:
- Extracted to render world each frame via `ExtractedPointLights`
- Stored in GPU storage buffer (up to 256 lights)
- Use quadratic falloff: `(1 - (d/r)²)²`
- Can be auto-generated from emissive voxels

### Emissive Light Generation (`studio_core/src/voxel.rs`)

```rust
pub fn extract_emissive_lights(chunk: &VoxelChunk, min_emission: u8) -> Vec<EmissiveLight>
pub fn extract_clustered_emissive_lights(chunk: &VoxelChunk, min_emission: u8, color_tolerance: f32) -> Vec<EmissiveLight>
```

Auto-generates point lights from emissive voxels with matching colors.

## Shader Architecture

### G-Buffer Layout

| Target | Format | Contents |
|--------|--------|----------|
| gColor | Rgba16Float | RGB = albedo, A = emission (0-1) |
| gNormal | Rgba16Float | RGB = world normal, A = ambient occlusion |
| gPosition | Rgba32Float | XYZ = world position, W = linear depth |

### Lighting Shader Bind Groups

| Group | Binding | Content |
|-------|---------|---------|
| 0 | 0-3 | G-buffer textures + sampler |
| 1 | 0-1 | Shadow map + comparison sampler |
| 2 | 0 | Shadow uniforms (light view-proj matrix) |
| 3 | 0 | Point lights storage buffer |

### Tone Mapping

Uses **hybrid tone mapping** to preserve color saturation:
- ACES for dark/mid-tones (good contrast)
- Reinhard luminance for bright areas (preserves saturation)
- Blend based on brightness: `mix(aces, reinhard, smoothstep(0.5, 2.0, brightness))`

## File Structure

```
crates/studio_core/src/
├── deferred/
│   ├── mod.rs              # Module exports
│   ├── plugin.rs           # DeferredRenderingPlugin, render graph setup
│   ├── gbuffer.rs          # G-buffer texture creation
│   ├── gbuffer_geometry.rs # GBufferVertex, pipeline, test cube
│   ├── gbuffer_node.rs     # GBufferPassNode
│   ├── lighting_node.rs    # LightingPassNode, lighting pipeline
│   ├── shadow.rs           # Shadow textures, config
│   ├── shadow_node.rs      # ShadowPassNode
│   ├── bloom.rs            # Bloom textures, config
│   ├── bloom_node.rs       # BloomNode (downsample/upsample/composite)
│   ├── point_light.rs      # Point light component, extraction, storage buffer
│   ├── extract.rs          # Main world → render world extraction
│   ├── prepare.rs          # GPU resource preparation
│   └── labels.rs           # Render graph node labels
├── voxel.rs                # Voxel, VoxelChunk, emissive light extraction
├── voxel_mesh.rs           # Mesh generation, vertex attributes, AO
├── creature_script.rs      # Lua scripting integration
├── orbit_camera.rs         # Camera controller
└── lib.rs                  # Public API exports

assets/shaders/
├── gbuffer.wgsl            # G-buffer vertex/fragment (MRT output)
├── deferred_lighting.wgsl  # Fullscreen lighting pass
├── shadow_depth.wgsl       # Shadow map depth pass
├── bloom_downsample.wgsl   # 13-tap downsample filter
├── bloom_upsample.wgsl     # Tent filter upsample
├── bloom_composite.wgsl    # Final composite + tone mapping
└── voxel.wgsl              # Forward rendering material (unused in deferred)
```

## Marker Components

| Component | Purpose |
|-----------|---------|
| `DeferredCamera` | Marks camera for deferred pipeline |
| `DeferredRenderable` | Marks mesh for deferred rendering |
| `DeferredPointLight` | Point light source |

## Current Limitations

- Single 32x32x32 chunk (no multi-chunk world yet)
- No face culling (all 6 faces generated per voxel)
- No greedy meshing (each voxel face is separate quad)
- Point lights don't cast shadows (only directional light shadows)
- Single shadow map (purple moon only, orange moon unshadowed)

## Performance Characteristics

- **Point lights**: O(n) per fragment, but with early distance culling
- **Shadow mapping**: Single 2048x2048 depth texture, 3x3 PCF
- **Bloom**: 6-level mip chain, ~12 render passes total
- **Vertex format**: 44 bytes/vertex (could be optimized with packed formats)
