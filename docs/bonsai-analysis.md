# Bonsai Voxel Engine Analysis

Analysis of the Bonsai C++ voxel engine for techniques to adapt to Creature 3D Studio.

**Source**: `/bonsai/` directory  
**Purpose**: Reference implementation for voxel rendering, emission/glow, and LOD systems

---

## Directory Structure

```
bonsai/
├── src/engine/
│   ├── world.h/cpp          # World management, octree traversal
│   ├── world_chunk.h/cpp    # Voxel storage, mesh generation
│   ├── render.cpp           # Rendering pipeline orchestration
│   ├── mesh.h/cpp           # Mesh data structures
│   ├── light.h              # Point lights, lighting data
│   ├── bloom.h              # Bloom system (mip-chain)
│   ├── graphics.h           # Draw lists, GPU state
│   └── lod.cpp              # LOD boundary detection
├── shaders/
│   ├── gBuffer.*            # Deferred geometry pass
│   ├── Lighting.*           # Deferred lighting pass
│   ├── 3DTransparency.*     # Order-Independent Transparency
│   ├── bloom_*.fragmentshader  # Downsample/upsample bloom
│   ├── composite.*          # Final compositing + tone mapping
│   ├── Ao.fragmentshader    # SSAO
│   └── terrain/             # GPU terrain generation
└── generated/
    └── freelist_allocator_octree_node.h  # Octree allocation
```

---

## Core Data Structures

### Voxel (32-bit packed)
```c
struct voxel {
  u16 Color;   // Packed HSV color
  u16 Normal;  // 15-bit packed normal
};

struct voxel_lighting {
  u8 Emission;  // Per-voxel emission (0-255)
};
```

### Vertex Material
```c
struct vertex_material {
  u16 ColorIndex;    // Packed HSV color
  u8 Transparency;   // 0-255
  u8 Emission;       // 0-255 (KEY FOR GLOW)
};
```

### Chunk Data
```c
struct chunk_data {
  v3i Dim;           // Usually 64x64x64
  u64 *Occupancy;    // Bit-packed filled state
  u64 *FaceMasks;    // 6 face directions for mesh gen
};

struct world_chunk {
  chunk_data Data;
  gpu_element_buffer_handles Handles;  // VAO/VBOs
  v3i WorldP;                          // World position
  v3i DimInChunks;                     // LOD resolution
};
```

### Octree Node
```c
struct octree_node {
  octree_node_type Type;    // Branch or Leaf
  v3i WorldP;               // World position
  v3i Resolution;           // LOD level (V3i(1) = full res)
  world_chunk *Chunk;       // Actual voxel data
  octree_node *Children[8]; // Branch children
};
```

### World
```c
struct world {
  octree_node Root;
  v3i ChunkDim;              // Default 64x64x64
  v3i Center;                // Camera-relative center
  s32 ChunksPerResolutionStep;  // LOD granularity
};
```

---

## Key Algorithms

### Mesh Generation (Bitwise Face Masks)

Not traditional greedy meshing - uses bitwise operations on 64-bit occupancy masks:

```c
// For each Y,Z slice, compute face visibility
u64 RightFaces = (Bits) & ~(Bits>>1);  // Right neighbor empty
u64 LeftFaces  = (Bits) & ~(Bits<<1);  // Left neighbor empty
u64 FrontFaces = Bits & (~yBits);      // Front neighbor empty
// ... 6 directions total
```

**Pros**: Very fast, parallel-friendly  
**Cons**: No face merging (more vertices than greedy)

### LOD System

Distance-based octree subdivision:

```c
v3i ComputeNodeDesiredResolution(octree_node *Node) {
  r32 Distance = DistanceToBox(CameraP, NodeRect);
  s32 DistanceInChunks = Distance / ChunkDim.x;
  return Max(V3i(1), V3i(DistanceInChunks / ChunksPerResolutionStep));
}
```

5 LOD levels: 32, 16, 8, 4, 2 voxels per chunk dimension.

### Octree Traversal

- `SplitOctreeNode_Recursive()` - Subdivide when camera approaches
- `OctreeBranchShouldCollapse()` - Merge when camera recedes
- `DispatchOctreeDrawJobs()` - Build sorted draw lists

---

## Rendering Pipeline

```
1. SHADOW PASS
   DepthRTT shaders -> shadowMap texture

2. G-BUFFER PASS (Deferred)
   gBuffer shaders -> gColor, gNormal, gPosition, gDepth

3. TRANSPARENCY PASS (OIT)
   3DTransparency.frag -> TransparencyAccum, TransparencyCount
   Uses Bravoil-McGuire weighted blended OIT

4. SSAO PASS
   Ao.frag -> Ssao texture (32-sample hemisphere)

5. LIGHTING PASS
   Lighting.frag -> LuminanceTex (HDR)
   Inputs: All g-buffers, shadow map, SSAO, transparency textures
   Features: Sun + key + back lights, point lights, fog, shadows

6. BLOOM PASS
   bloom_downsample.frag (3 mip levels)
   bloom_upsample.frag (tent filter)
   -> BloomTex

7. COMPOSITE PASS
   composite.frag -> Final output
   Features: AgX/Reinhard tone mapping, bloom add (5%), gamma correction
```

---

## Emission/Glow System (Critical for Our Use Case)

### Data Flow

1. **Voxel Storage**: `voxel_lighting.Emission` (u8)
2. **Mesh Generation**: `vertex_material.Emission` (u8)
3. **GPU Upload**: Packed into `VERTEX_TRANS_EMISS_LAYOUT_LOCATION`
4. **Vertex Shader**: Unpacks to `TransEmiss.y`, scales by `RENDERER_MAX_LIGHT_EMISSION_VALUE`
5. **Transparency Pass**: Stores emission in `TransparencyCount.g` (weighted by alpha)
6. **Lighting Pass**: Adds emission directly to light output:
   ```glsl
   float TransAccumEmission = texture(TransparencyCountTex, gBufferUV).g;
   TransparencyContrib *= V3(LightTransmission + LightEmission);
   ```
7. **Bloom**: High-luminance pixels (including emissive) bloom naturally
8. **Composite**: Bloom added at 5% intensity

### Key Insight

Emission **bypasses normal lighting** - emissive surfaces glow regardless of scene lighting. This is exactly what we need for:
- Mana = light = visibility = danger
- Creatures with bioluminescent parts
- Night scene with glowing elements

### Current Limitations

- Emission only works via OIT (transparent objects)
- Opaque emissive surfaces would need G-buffer emission channel
- No per-object bloom intensity control
- No volumetric light scattering (god rays)

---

## Techniques to Adapt

### Directly Portable (Reference Implementation)

| Technique | Bonsai Location | Our Approach |
|-----------|-----------------|--------------|
| Emission vertex attribute | `vertex_material.Emission` | Same - encode mana as emission |
| Deferred + OIT hybrid | `3DTransparency.frag` | Port to WGSL, simplify |
| Mip-chain bloom | `bloom_*.frag` | Port to WGSL compute shaders |
| Distance fog | `Lighting.frag:306-319` | Adapt for 6-moon atmosphere |
| Octree LOD | `world.cpp` | Simplify for smaller creatures |

### Adapt with Modifications

| Technique | Modification Needed |
|-----------|---------------------|
| G-buffer emission | Add emission channel for opaque emissive voxels |
| Bloom extraction | Add threshold/tinting for mana-type colors |
| Fog color | Dynamic based on ambient mana level |
| Point lights | Use for mana sources attracting predators |

### Reference Only (Won't Port Directly)

| Technique | Reason |
|-----------|--------|
| Bitwise mesh generation | Bevy has `block-mesh` / `building-blocks` crates |
| GPU terrain generation | Not needed for creature sculpting |
| 64-bit occupancy masks | Bevy uses different spatial structures |

---

## Recommendations for Creature Studio

### Voxel Storage
- Use Bevy-native crates (`block-mesh`, `ndshape`) instead of porting Bonsai's C++ structures
- Keep the emission concept but integrate with Bevy's material system

### Rendering Pipeline
1. **Phase 1**: Use Bevy's standard PBR with emissive materials
2. **Phase 2**: Add custom bloom pass for mana visualization
3. **Phase 3**: Full deferred pipeline if performance requires

### Emission System
```rust
// Proposed Bevy component
#[derive(Component)]
struct VoxelEmission {
    intensity: f32,      // 0.0 - 1.0+
    mana_type: ManaType, // For color tinting
    pulse_freq: f32,     // Danger indicator pulsing
}
```

### Bloom for "Glow = Mana = Danger"
- Port Bonsai's mip-chain bloom (efficient, proven)
- Add mana-type color tinting in upsample pass
- Dynamic bloom radius based on danger level

### Fog as Ambient Mana
```glsl
// Adapt from Lighting.fragmentshader:306-319
vec3 FogColor = mix(NightFogColor, ManaFogColor, AmbientManaLevel);
float FogPower = BasePower * (1.0 + AmbientManaLevel);
```

### Moon Lighting
- Use Bonsai's point light texture system for 6 colored moons
- Each moon = point light with colored emission
- Mana visibility tied to moon phase/color

---

## Files to Reference During Implementation

| Task | Reference File |
|------|----------------|
| Bloom shaders | `shaders/bloom_downsample.fragmentshader`, `bloom_upsample.fragmentshader` |
| Emission handling | `shaders/Lighting.fragmentshader:136, 382-402` |
| Tone mapping | `shaders/composite.fragmentshader:169-209` |
| OIT accumulation | `shaders/3DTransparency.fragmentshader:71-72, 116` |
| Fog calculation | `shaders/Lighting.fragmentshader:306-319` |
| Vertex emission packing | `src/engine/mesh.h:701-793` |
| LOD decision | `src/engine/world.cpp:546-604` |

---

## Summary

Bonsai provides a solid reference for:
1. **Emission/glow pipeline** - exactly what we need for mana visualization
2. **Bloom system** - efficient mip-chain approach
3. **Deferred + OIT hybrid** - handles both opaque and transparent glowing surfaces
4. **LOD octree** - proven approach for voxel worlds

**Recommended approach**: Reference Bonsai's shaders (port key concepts to WGSL) but use Bevy-native crates for voxel data structures and mesh generation. The emission→bloom→composite pipeline is the most valuable part to adapt.
