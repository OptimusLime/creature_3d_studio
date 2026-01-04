# Bonsai Rendering Pipeline Analysis

## Overview

Bonsai uses a **deferred rendering** pipeline with multiple passes. This document analyzes each stage and maps out what we need to implement to achieve Bonsai-quality output.

**UPDATE (Phase 7.5)**: We are pivoting from forward to deferred rendering. The game requires many dynamic lights (torches, glowing creatures, crafting stations) which forward rendering cannot handle efficiently. This document now contains the detailed Bonsai implementation analysis and our deferred rendering plan.

## Bonsai Pipeline - Detailed Implementation

### G-Buffer Pass

**Vertex Shader** (`gBuffer.vertexshader`):
```glsl
// Vertex inputs
layout(location = 0) in vec3 vertexPosition_modelspace;
layout(location = 1) in vec3 vertexNormal_modelspace;
layout(location = 2) in uint ColorIndex;          // Palette lookup
layout(location = 3) in ivec2 in_TransEmiss;      // (Transparency, Emission) 8-bit each

// Outputs to fragment
out vec3 vertexP_worldspace;
out vec3 vertexN_worldspace;
out vec3 MaterialColor;
out vec2 TransEmiss;  // (Transparency, Emission) normalized to [0,1]
```

**Fragment Shader** (`gBuffer.fragmentshader`):
```glsl
// Multiple Render Targets (MRT)
layout (location = 0) out vec4 gColor;      // RGB = albedo, A = unused
layout (location = 1) out vec3 gNormal;     // World-space normal
layout (location = 2) out vec4 gPosition;   // XYZ = world pos, W = linear depth

// Linear depth calculation
float Linearize(float Depth) {
  return (2.0 * NearClip) / (FarClip + NearClip - Depth * (FarClip - NearClip));
}

void main() {
  gColor.rgb = MaterialColor;
  gNormal = normalize(vertexN_worldspace);
  gPosition.xyz = vertexP_worldspace;
  gPosition.w = Linearize(gl_FragCoord.z);
}
```

### Lighting Pass

**Fullscreen Quad Vertex** (`Lighting.vertexshader`):
```glsl
layout(location = 0) in vec3 vertexPosition_modelspace;
out vec2 gBufferUV;

void main() {
  gBufferUV = (vertexPosition_modelspace.xy + vec2(1,1)) / 2.0;
  gl_Position = vec4(vertexPosition_modelspace, 1);
}
```

**Lighting Fragment** (`Lighting.fragmentshader`):

Input textures:
```glsl
uniform sampler2D gColor;
uniform sampler2D gPosition;
uniform sampler2D gNormal;
uniform sampler2D gDepth;
uniform sampler2D shadowMap;
uniform sampler2D Ssao;
```

**Point Lights via 1D Textures** (NOT uniform buffers - scales better):
```glsl
uniform s32 LightCount;
uniform sampler2D LightColors;      // 1D texture: RGB per light
uniform sampler2D LightPositions;   // 1D texture: XYZ per light

// In light loop:
for (int i = 0; i < LightCount; ++i) {
  vec3 LightPos = texelFetch(LightPositions, ivec2(i, 0), 0).rgb;
  vec3 LightCol = texelFetch(LightColors, ivec2(i, 0), 0).rgb;
  
  float dist = distance(FragWorldP, LightPos);
  float att = 1.0 / (dist * dist);  // 1/r² falloff
  
  // Diffuse + Specular accumulation
  PointLightsContrib += LightCol * att * (diffuse + specular);
}
```

**Fog (Additive, applied LAST)**:
```glsl
float DistanceToFrag = distance(Camera, FragWorldP);
float FogContrib = clamp(DistanceToFrag / MaxFogDist, 0.0, 1.0);
FogContrib *= FogContrib * 1.2;  // Squared falloff
vec3 Fog = FogContrib * FogColor * FogPower;

// Final compositing - fog is ADDED, not mixed
TotalLight = (Diffuse * KeyLight * Shadow) +
             (Diffuse * AmbientLight) +
             (Diffuse * BackLight) +
             (PointLightsContrib) +
             Fog;

TotalLight *= BlurredAO;  // SSAO multiplies everything
```

### C++ Data Structures

**G-Buffer Setup** (`render.h`):
```cpp
struct g_buffer_textures {
  texture Color;    // RGBA16F
  texture Normal;   // RGBA16F  
  texture Depth;    // Depth texture
};

struct g_buffer_render_group {
  framebuffer FBO;
  g_buffer_textures Textures;
  shader gBufferShader;
};
```

**Point Light Storage** (`light.h`):
```cpp
struct light {
  v3 Position;
  v3 Color;
};

struct game_lights {
  texture ColorTex;       // 1D: MAX_LIGHTS x 1, RGB32F
  texture PositionTex;    // 1D: MAX_LIGHTS x 1, RGB32F
  s32 Count;
  light *Lights;          // CPU array
};
```

---

## Why Deferred Rendering is Required

### Forward Rendering Scaling Problem

Forward rendering computes lighting PER OBJECT PER LIGHT:
- Cost = O(objects × lights × pixels)
- 100 objects × 50 lights = 5000 lighting calculations per pixel

### Deferred Rendering Solution

Deferred computes lighting PER PIXEL:
- Cost = O(pixels × lights)  
- 50 lights = 50 lighting calculations per pixel (regardless of object count)

### Our Game Requirements

- Dark fantasy aesthetic with glowing lights piercing darkness
- Player-placed torches, crafting stations, magical effects
- Creatures with glowing eyes/parts
- **Estimate: 50-100+ dynamic lights in complex scenes**

Forward rendering will DIE at this scale. Deferred is mandatory.

---

## Key Bonsai Design Decisions to Adopt

1. **G-Buffer Layout**: 3 textures (Color+Emission, Normal, Position+Depth)
2. **Light Storage**: 1D textures, not uniform buffers (no size limits)
3. **Fog is Additive**: `Total = AllLighting + Fog` (objects fade INTO fog)
4. **SSAO Multiplies Final**: `Total *= AO` (applied once, not per-light)
5. **Simple Light Loop**: No tiled/clustered complexity for ~100 lights

---

## DEFERRED RENDERING PHASE PLAN v2.0

### What We Keep
- Existing code (examples p0-p7, voxel.rs, creature_script.rs, Lua integration)
- Screenshot test infrastructure  
- Bevy's bloom (post-process, still works)
- Bevy's tone mapping (post-process, still works)

### Phase 8: G-Buffer Pass (8 hrs)
**Goal**: Render geometry to multiple render targets.

**G-Buffer Layout** (matching Bonsai):
- `gColor` (RGBA16F): RGB = albedo, A = emission
- `gNormal` (RGBA16F): RGB = world-space normal
- `gPosition` (RGBA16F): XYZ = world position, W = linear depth
- Depth buffer

**Implementation**:
1. Create custom render pass with MRT output
2. G-buffer shader writes color/normal/position (NO lighting)
3. Verify by dumping G-buffer textures

**Test**: `p8_gbuffer.png` - tiled view of all G-buffer textures

---

### Phase 9: Basic Lighting Pass (4 hrs)
**Goal**: Fullscreen quad reads G-buffer, computes lighting.

**Implementation**:
1. Fullscreen shader samples gColor, gNormal, gPosition
2. Single directional light (sun) + ambient
3. Output to HDR texture

**Test**: `p9_lighting.png` - should look identical to p7

---

### Phase 10: Point Lights (6 hrs)
**Goal**: Many dynamic lights, efficiently.

**Implementation** (Bonsai approach):
1. Light storage: 1D textures OR storage buffer
2. Loop in lighting shader
3. 1/r² attenuation + specular
4. Test with 10, 50, 100 lights

**Test**: `p10_point_lights.png` - scene with multiple colored lights

---

### Phase 11: Fog + Back Light (2 hrs)
**Goal**: Complete atmospheric lighting.

**Implementation**:
1. Back light: opposite sun, 15% intensity
2. Fog: ADDITIVE, applied after all lighting
3. `FinalLight = AllLighting + Fog`

**Test**: `p11_fog_backlight.png` - proper fog fade into background

---

### Phase 12: SSAO (8 hrs)
**Goal**: Screen-space ambient occlusion.

**Implementation** (port Bonsai's `Ao.fragmentshader`):
1. Input: gNormal, depth
2. 32-sample hemisphere kernel
3. Blur pass
4. Apply: `TotalLight *= AO`

**Test**: `p12_ssao.png` - visible contact shadows in corners

---

### Phase 13: Bloom Integration (2 hrs)
**Goal**: Verify Bevy bloom works with our pipeline.

**Test**: `p13_bloom.png` - emissive objects glow

---

### Phase 14: Rich Test Scene (4 hrs)
**Goal**: Full quality evaluation.

**Implementation**:
1. Multi-voxel creature with emissive parts
2. Ground with torches (point lights)
3. Full atmosphere

**Test**: `p14_poster.png` - the money shot

---

## Timeline

| Phase | Description | Hours | Running Total |
|-------|-------------|-------|---------------|
| 8 | G-Buffer Pass | 8 | 8 |
| 9 | Basic Lighting Pass | 4 | 12 |
| 10 | Point Lights | 6 | 18 |
| 11 | Fog + Back Light | 2 | 20 |
| 12 | SSAO | 8 | 28 |
| 13 | Bloom Integration | 2 | 30 |
| 14 | Rich Test Scene | 4 | 34 |

**Total: ~34 hours**

---

## Critical Path

```
Phase 8 (G-Buffer) → Phase 9 (Lighting) → Phase 10 (Point Lights)
```

Once Phase 10 is complete, we have the core feature: **many lights that scale**.

---

## Bonsai Shader File Reference

| File | Purpose | Our Phase |
|------|---------|-----------|
| `gBuffer.vertexshader` | G-buffer vertex transform | Phase 8 |
| `gBuffer.fragmentshader` | G-buffer MRT output | Phase 8 |
| `Lighting.vertexshader` | Fullscreen quad | Phase 9 |
| `Lighting.fragmentshader` | All lighting + fog | Phase 9-11 |
| `Ao.fragmentshader` | SSAO | Phase 12 |
| `bloom_downsample.fragmentshader` | Bloom | ✓ Bevy built-in |
| `bloom_upsample.fragmentshader` | Bloom | ✓ Bevy built-in |
| `composite.fragmentshader` | Tone mapping | ✓ Bevy built-in |
