# SSAO Quality Investigation Plan

## Problem Statement
Our SSAO implementation produces visible noise/dithering patterns that look cheap and unprofessional compared to Bonsai's smooth SSAO output. We need to systematically investigate Bonsai's implementation and identify what we're doing wrong.

## Current State
- SSAO is implemented and functional
- Greedy meshing artifacts are fixed
- **Quality is unacceptable** - visible dithering/noise pattern
- Bilateral blur helps but doesn't solve the problem

---

## Phase 1: Bonsai Deep Dive - COMPLETE

### 1.1 Bonsai SSAO Code Analysis

**Files analyzed:**
- `bonsai/shaders/Ao.fragmentshader` - Main SSAO shader
- `bonsai/shaders/Lighting.fragmentshader` - How AO is applied + blur
- `bonsai/src/engine/render/render_init.cpp` - Kernel and noise generation
- `bonsai/src/engine/render.cpp` - Render at half resolution

### 1.2 Key Findings from Bonsai

#### Kernel Generation (render_init.cpp:3-15)
```cpp
void InitSsaoKernel(v3 *Kernel, s32 Count, random_series *Entropy)
{
  for (int KernelIndex = 0; KernelIndex < Count; ++KernelIndex)
  {
    r32 Scale = (r32)KernelIndex/(r32)Count;
    Scale = Lerp(Scale * Scale, 0.1f, 1.0f);
    
    Kernel[KernelIndex] = V3(RandomBilateral(Entropy), RandomBilateral(Entropy), RandomUnilateral(Entropy));
    Kernel[KernelIndex] = Normalize(Kernel[KernelIndex]*Scale);
  }
}
```
**Key difference: Bonsai uses random hemisphere vectors, NOT cosine-weighted sampling.**

#### Noise Texture (render_init.cpp:17-37)
```cpp
v2i SsaoNoiseDim = V2i(32,32);  // 32x32 noise texture!
// ...
SsaoNoise[NoiseIndex] = Normalize(V3(RandomBilateral(&SsaoEntropy), RandomBilateral(&SsaoEntropy), 0.0f));
// ...
texture SsaoNoiseTexture = MakeTexture_RGB(SsaoNoiseDim, SsaoNoise, CSz("SsaoNoiseTexture"), 1);
```
**CRITICAL: Bonsai uses 32x32 noise texture, we use 4x4!**

#### SSAO Shader Parameters (Ao.fragmentshader)
```glsl
const int SSAO_KERNEL_SIZE = 32;  // 32 samples, not 64!
float SsaoRadius = 0.0005f;       // Very small radius in world space
float DepthThreshold = 0.0013f;   // Depth threshold for range check
float Bias = FragDepth*0.0005f;   // DEPTH-PROPORTIONAL bias!
```

#### Depth Comparison Method (Ao.fragmentshader:96-99)
```glsl
if ( BiasedFragDepth>SampleDepth && (BiasedFragDepth-SampleDepth < DepthThreshold) )
  AO -= OccluderContribution;
```
**Simple binary test with threshold, NO range smoothstep.**

#### Blur in Lighting Pass (Lighting.fragmentshader:212-230)
```glsl
int AoBlurSize = 2;  // Only 2x2 box blur!
for (int i = 0; i < AoBlurSize; ++i) {
   for (int j = 0; j < AoBlurSize; ++j) {
       vec2 TexOffset = (hlim + vec2(float(i), float(j))) * texelSize;
       AccumAO += texture(Ssao, gBufferUV + TexOffset).r;
   }
}
BlurredAO = AccumAO / float(AoBlurSize * AoBlurSize);
```
**Simple 2x2 box blur, NO depth-aware/bilateral filtering.**

#### Render Resolution (render.cpp:6)
```cpp
SetViewport(ApplicationResolution/2);  // Half resolution!
```
**SSAO is rendered at HALF RESOLUTION - this naturally hides noise when upsampled.**

---

## Phase 2: Detailed Comparison Table - COMPLETE

| Aspect | Bonsai | Our Implementation | Impact |
|--------|--------|-------------------|--------|
| **Sample Count** | 32 | 64 | Ours is 2x more, should be better |
| **Kernel Distribution** | Random uniform hemisphere | Cosine-weighted hemisphere | Different distribution |
| **Kernel Scale** | `lerp(scale^2, 0.1, 1.0)` | `0.1 + 0.9 * scale^2` | Similar |
| **Radius** | 0.0005 (view-space) | 1.5 (world-space) | **HUGE DIFFERENCE** |
| **Bias** | `FragDepth * 0.0005` (proportional) | 0.01 (constant) | **Depth-proportional vs constant** |
| **Intensity** | 1.0 (implicit) | 2.5 | We boost intensity |
| **Noise Texture Size** | **32x32** | 4x4 | **8x bigger - less tiling** |
| **Noise Content** | Normalized XY random vectors | Random XYZ | Similar |
| **Depth Source** | Linear depth from nonlinear | World pos.w (linear depth) | Similar |
| **Normal Source** | Model-space from G-buffer | World-space from G-buffer | Similar |
| **TBN Construction** | `mat3(Right, Front, Up) * SsaoRadius` | Standard Gram-Schmidt TBN | Similar |
| **Depth Compare** | Binary: `if (fragDepth > sampleDepth && diff < threshold)` | Binary with smoothstep range | Similar |
| **Range Check** | Hard threshold: `diff < DepthThreshold` | Smooth: `smoothstep(0,1, r/diff)` | Different |
| **Output Resolution** | **HALF** | Full | **Major - hides noise!** |
| **Blur Type** | 2x2 box blur | 3x3 bilateral | Similar size |
| **Blur Depth-Aware** | No | Yes (exp weight) | Ours is fancier |
| **Temporal Filtering** | None | None | Same |

---

## Phase 3: Root Cause Analysis - KEY FINDINGS

### Most Likely Causes of Noise (Priority Order)

#### 1. NOISE TEXTURE SIZE (HIGH CONFIDENCE)
**Bonsai: 32x32, Ours: 4x4**

A 4x4 noise texture tiles every 4 pixels, creating a very visible repeating pattern. Bonsai's 32x32 texture means the pattern repeats every 32 pixels - much less noticeable.

**Fix: Increase noise texture to 32x32**

#### 2. HALF-RESOLUTION RENDERING (HIGH CONFIDENCE)
**Bonsai renders SSAO at half resolution and upsamples.**

This naturally blurs/smooths the noise when the half-res texture is sampled at full resolution. It's like getting a free blur pass.

**Fix: Render SSAO at half resolution**

#### 3. DEPTH-PROPORTIONAL BIAS (MEDIUM CONFIDENCE)
**Bonsai: `Bias = FragDepth * 0.0005f`**

Bonsai's bias increases with distance, preventing artifacts at far distances. Our constant bias may be too large near or too small far.

**Fix: Use depth-proportional bias**

#### 4. VERY SMALL RADIUS (MEDIUM CONFIDENCE)
**Bonsai: 0.0005 (appears to be view-space)**
**Ours: 1.5 (world-space)**

The scales are wildly different. If Bonsai's is in some normalized space, 0.0005 would be much smaller than our 1.5 world units. Smaller radius = tighter occlusion = less noise spread.

**Fix: Experiment with much smaller radius values**

---

## Phase 4: Implementation Plan

### Priority 1: Quick Wins
1. **Increase noise texture to 32x32** - Easy change in ssao_node.rs
2. **Render at half resolution** - Modify texture size in prepare_ssao_textures

### Priority 2: Parameter Tuning
3. **Implement depth-proportional bias** - `bias = linear_depth * 0.0005`
4. **Experiment with smaller radius** - Try 0.1, 0.05, 0.01

### Priority 3: If Still Noisy
5. **Add dedicated blur pass** - Separate shader, larger kernel
6. **Try uniform kernel distribution** - Match Bonsai's random hemisphere

---

## Success Criteria

SSAO is acceptable when:
1. No visible dithering/noise pattern at normal viewing distance
2. Smooth gradients in occluded areas
3. Clean edges at depth discontinuities
4. Performance within 1ms on target hardware
5. Looks comparable to Bonsai's output

---

## Files to Modify

### Phase 1 Changes
- `crates/studio_core/src/deferred/ssao_node.rs` - 32x32 noise texture
- `crates/studio_core/src/deferred/ssao.rs` - Half-res texture option
- `assets/shaders/ssao.wgsl` - Depth-proportional bias

### Phase 2 Changes (if needed)
- `assets/shaders/ssao_blur.wgsl` - Dedicated blur shader
- `crates/studio_core/src/deferred/ssao_blur_node.rs` - Blur pass

---

## Key Bonsai Code Snippets for Reference

### Kernel Generation
```cpp
// bonsai/src/engine/render/render_init.cpp:3-15
void InitSsaoKernel(v3 *Kernel, s32 Count, random_series *Entropy)
{
  for (int KernelIndex = 0; KernelIndex < Count; ++KernelIndex)
  {
    r32 Scale = (r32)KernelIndex/(r32)Count;
    Scale = Lerp(Scale * Scale, 0.1f, 1.0f);
    Kernel[KernelIndex] = V3(RandomBilateral(Entropy), RandomBilateral(Entropy), RandomUnilateral(Entropy));
    Kernel[KernelIndex] = Normalize(Kernel[KernelIndex]*Scale);
  }
}
```

### SSAO Core Loop
```glsl
// bonsai/shaders/Ao.fragmentshader:61-100
float Bias = FragDepth*0.0005f;
float BiasedFragDepth = FragDepth - Bias;

for (int KernelIndex = 0; KernelIndex < SSAO_KERNEL_SIZE; ++KernelIndex)
{
  vec3 KernelP = Reorientation * SsaoKernel[KernelIndex];
  vec3 SampleP = KernelP + FragPosition;
  
  vec4 Projected = ViewProjection * vec4(SampleP, 1);
  vec2 SampleUV = (Projected.xy / Projected.w) * 0.5f + 0.5f;
  
  float SampleDepth = Linearize(texture(gDepth, SampleUV).r, 5000.f, 0.1f);
  
  if (BiasedFragDepth > SampleDepth && (BiasedFragDepth - SampleDepth < DepthThreshold))
    AO -= OccluderContribution;
}
```

### Half-Resolution Render
```cpp
// bonsai/src/engine/render.cpp:6
SetViewport(ApplicationResolution/2);
```
