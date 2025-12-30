# XeGTAO Implementation Plan

## Executive Summary

Replace our current SSAO implementation with Intel's XeGTAO (Ground Truth Ambient Occlusion) algorithm to eliminate banding artifacts and improve visual quality. This document defines a phased approach with **specific, measurable verification criteria** for each phase.

**Current Problem**: Visible banding/noise artifacts in SSAO output (see `screenshots/p18_cross_chunk_culling.png`, `screenshots/p19_dual_moon_shadows.png`)

**Target Outcome**: Clean, artifact-free ambient occlusion matching XeGTAO reference quality

---

## Phase Overview

| Phase | Name | Duration | Verification |
|-------|------|----------|--------------|
| 0 | Baseline & Test Infrastructure | 1 session | Test example runs, baseline screenshots captured |
| 1 | Depth Linearization | 1 session | Depth visualization test passes |
| 2 | Core GTAO Algorithm | 2 sessions | AO gradient test passes, no banding |
| 3 | Spatial Denoiser | 1 session | Noise measurement below threshold |
| 4 | Depth MIP Chain | 1 session | Performance + quality metrics met |

---

## Phase 0: Baseline & Test Infrastructure

### Objective
Establish measurable baseline and create dedicated test examples for SSAO verification.

### Tasks

#### Task 0.1: Create SSAO Test Scene
**File**: `assets/worlds/ssao_test.voxworld`

Create a test world with known geometry for AO verification:
- Flat ground plane (tests: no false occlusion on flat surfaces)
- 90-degree corner (tests: proper corner darkening)
- Stairs/steps (tests: gradient without banding)
- Floating cube (tests: contact shadows)
- Thin pillar (tests: thin occluder handling)

**Acceptance Criteria**:
- [ ] World file loads without errors
- [ ] Contains all 5 test geometries listed above

#### Task 0.2: Create SSAO Test Example
**File**: `examples/p20_ssao_test.rs`

```rust
// Test modes:
// - Mode 0: Full render (AO applied to lighting)
// - Mode 1: AO buffer only (grayscale visualization)
// - Mode 2: Depth buffer visualization
// - Mode 3: Normals visualization
```

**Acceptance Criteria**:
- [ ] `cargo run --example p20_ssao_test` completes without panic
- [ ] Generates `screenshots/p20_ssao_test.png`
- [ ] Can toggle between visualization modes via command line arg

#### Task 0.3: Capture Baseline Screenshots
Run current implementation and document artifacts.

**Acceptance Criteria**:
- [ ] `screenshots/baseline_ssao_full.png` - Current full render
- [ ] `screenshots/baseline_ssao_ao_only.png` - Current AO buffer
- [ ] `docs/XeGTAO_BASELINE_ARTIFACTS.md` - Document of visible issues

#### Task 0.4: Create AO Quality Metrics Function
**File**: `crates/studio_core/src/deferred/ssao_metrics.rs`

Implement pixel analysis for automated verification:
```rust
pub struct SsaoMetrics {
    pub banding_score: f32,      // 0.0 = no banding, 1.0 = severe
    pub noise_variance: f32,     // Lower is better
    pub flat_surface_ao: f32,    // Should be ~1.0 (no occlusion)
    pub corner_ao: f32,          // Should be ~0.3-0.5 (occluded)
}
```

**Acceptance Criteria**:
- [ ] Function compiles and returns metrics struct
- [ ] Baseline banding_score documented (expected: high)
- [ ] Baseline noise_variance documented

### Phase 0 Verification Command
```bash
cargo run --example p20_ssao_test -- --mode ao_only
# Must produce: screenshots/p20_ssao_ao_only.png
# Must complete in < 5 seconds
```

---

## Phase 1: Depth Linearization

### Objective
Fix depth buffer handling to match XeGTAO's viewspace depth requirements.

### Background
XeGTAO requires **viewspace linear depth** (positive Z distance from camera). Our current implementation uses the position G-buffer's W component, which may have precision issues.

### Tasks

#### Task 1.1: Add Depth Unpacking Constants to Uniforms
**File**: `crates/studio_core/src/deferred/ssao_node.rs`

Add to `SsaoCameraUniform`:
```rust
/// Depth unpacking: viewZ = DepthUnpackConsts.x / (DepthUnpackConsts.y - screenDepth)
pub depth_unpack_consts: [f32; 2],
```

Compute from projection matrix:
```rust
let depth_linearize_mul = -proj[3][2];  // clipFar * clipNear / (clipFar - clipNear)
let depth_linearize_add = proj[2][2];   // clipFar / (clipFar - clipNear)
```

**Acceptance Criteria**:
- [ ] Uniforms compile without errors
- [ ] Values are non-zero when logged

#### Task 1.2: Create Depth Visualization Shader
**File**: `assets/shaders/debug_depth.wgsl`

Output linearized depth as grayscale:
```wgsl
let linear_z = depth_unpack_consts.x / (depth_unpack_consts.y - raw_depth);
let normalized = saturate(linear_z / 50.0); // 50 units max
return vec4(normalized, normalized, normalized, 1.0);
```

**Acceptance Criteria**:
- [ ] Near objects appear dark gray
- [ ] Far objects appear light gray
- [ ] No visible discontinuities or bands
- [ ] Sky/background is white (max depth)

#### Task 1.3: Update SSAO Shader Depth Sampling
**File**: `assets/shaders/ssao.wgsl`

Replace position buffer sampling with proper depth reconstruction:
```wgsl
fn linearize_depth(screen_depth: f32) -> f32 {
    return camera.depth_unpack_consts.x / (camera.depth_unpack_consts.y - screen_depth);
}

fn compute_viewspace_position(uv: vec2<f32>, linear_depth: f32) -> vec3<f32> {
    let ndc = vec2(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0);
    return vec3(
        ndc.x * camera.tan_half_fov.x * linear_depth,
        ndc.y * camera.tan_half_fov.y * linear_depth,
        -linear_depth
    );
}
```

**Acceptance Criteria**:
- [ ] Shader compiles without errors
- [ ] AO output is not all white or all black
- [ ] Corners show darkening

### Phase 1 Verification Command
```bash
cargo run --example p20_ssao_test -- --mode depth
# Must produce: screenshots/p20_depth_linear.png
# Visual check: Smooth gradient from dark (near) to light (far)
# No stair-stepping or banding in depth visualization
```

### Phase 1 Automated Test
```rust
#[test]
fn depth_linearization_produces_gradient() {
    let img = load_image("screenshots/p20_depth_linear.png");
    
    // Sample vertical line down center of image
    let samples: Vec<f32> = (0..img.height())
        .map(|y| img.get_pixel(img.width()/2, y).luminance())
        .collect();
    
    // Verify monotonic increase (far objects lighter)
    let is_monotonic = samples.windows(2).all(|w| w[1] >= w[0] - 0.01);
    assert!(is_monotonic, "Depth should increase monotonically with distance");
}
```

---

## Phase 2: Core GTAO Algorithm

### Objective
Implement the horizon-based AO calculation correctly, eliminating banding artifacts.

### Tasks

#### Task 2.1: Implement Proper Noise Generation
**File**: `crates/studio_core/src/deferred/ssao_node.rs`

Replace random noise with Hilbert curve + R2 sequence:
```rust
fn hilbert_index(x: u32, y: u32) -> u32 {
    // 64x64 Hilbert curve lookup
    // ... (port from XeGTAO.h)
}

fn spatio_temporal_noise(x: u32, y: u32, frame: u32) -> [f32; 2] {
    let index = hilbert_index(x % 64, y % 64);
    let index = index + 288 * (frame % 64);
    [
        fract(0.5 + index as f32 * 0.75487766624669276),
        fract(0.5 + index as f32 * 0.56984029099805326),
    ]
}
```

**Acceptance Criteria**:
- [ ] Noise pattern is not visible as regular grid
- [ ] R2 sequence produces low-discrepancy values

#### Task 2.2: Fix Horizon Search Loop
**File**: `assets/shaders/ssao.wgsl`

Port exact XeGTAO main pass logic:
- Proper slice angle calculation
- Correct horizon cos tracking per direction
- Falloff weight calculation
- Integration formula

Key formulas to verify:
```wgsl
// Horizon angle integration (line 542-543 in XeGTAO.hlsli)
let h0 = -fast_acos(horizon_cos1);
let h1 = fast_acos(horizon_cos0);
let iarc0 = (cos_norm + 2.0 * h0 * sin(n) - cos(2.0 * h0 - n)) / 4.0;
let iarc1 = (cos_norm + 2.0 * h1 * sin(n) - cos(2.0 * h1 - n)) / 4.0;
```

**Acceptance Criteria**:
- [ ] Shader compiles
- [ ] No NaN outputs (check with debug visualization)
- [ ] AO values in range [0.03, 1.0] as per XeGTAO spec

#### Task 2.3: Add Quality Level Presets
**File**: `crates/studio_core/src/deferred/ssao.rs`

```rust
pub enum SsaoQuality {
    Low,      // 1 slice, 2 steps
    Medium,   // 2 slices, 2 steps  
    High,     // 3 slices, 3 steps
    Ultra,    // 9 slices, 3 steps
}
```

**Acceptance Criteria**:
- [ ] All 4 quality levels produce valid output
- [ ] Higher quality = more AO detail (visually verifiable)
- [ ] Performance scales: Low < Medium < High < Ultra

#### Task 2.4: Implement Edge Detection Output
**File**: `assets/shaders/ssao.wgsl`

Add edges calculation for denoiser:
```wgsl
fn calculate_edges(center_z: f32, left_z: f32, right_z: f32, top_z: f32, bottom_z: f32) -> vec4<f32> {
    // Port XeGTAO_CalculateEdges
}

fn pack_edges(edges: vec4<f32>) -> f32 {
    // Pack 4 edge values into single R8
}
```

**Acceptance Criteria**:
- [ ] Edges texture is created (R8Unorm format)
- [ ] Edges are bright at depth discontinuities
- [ ] Edges are dark on smooth surfaces

### Phase 2 Verification Commands
```bash
# Test 1: Flat surface should have AO ~1.0 (no occlusion)
cargo run --example p20_ssao_test -- --mode ao_only --scene flat
# Sample center pixel: should be > 0.95

# Test 2: Corner should have AO ~0.3-0.5 (occluded)
cargo run --example p20_ssao_test -- --mode ao_only --scene corner
# Sample corner pixel: should be 0.2-0.6

# Test 3: Stairs should show smooth gradient (no banding)
cargo run --example p20_ssao_test -- --mode ao_only --scene stairs
# Banding score should be < 0.1
```

### Phase 2 Automated Tests
```rust
#[test]
fn flat_surface_has_no_occlusion() {
    let metrics = analyze_ssao_region("screenshots/p20_ao_flat.png", center_region);
    assert!(metrics.mean_ao > 0.95, "Flat surface should be ~1.0, got {}", metrics.mean_ao);
}

#[test]
fn corner_has_occlusion() {
    let metrics = analyze_ssao_region("screenshots/p20_ao_corner.png", corner_region);
    assert!(metrics.mean_ao < 0.6, "Corner should be occluded, got {}", metrics.mean_ao);
    assert!(metrics.mean_ao > 0.2, "Corner should not be black, got {}", metrics.mean_ao);
}

#[test]
fn no_banding_on_stairs() {
    let metrics = analyze_ssao_region("screenshots/p20_ao_stairs.png", stair_region);
    assert!(metrics.banding_score < 0.1, "Banding detected: {}", metrics.banding_score);
}
```

---

## Phase 3: Spatial Denoiser

### Objective
Implement edge-aware blur to clean up noise while preserving edges.

### Tasks

#### Task 3.1: Create Denoise Shader
**File**: `assets/shaders/ssao_denoise.wgsl`

Port `XeGTAO_Denoise` function:
```wgsl
fn unpack_edges(packed: f32) -> vec4<f32> { ... }

fn denoise_sample(ao: f32, edge: f32, sum: ptr<f32>, weight: ptr<f32>) {
    *sum += edge * ao;
    *weight += edge;
}

@fragment
fn fs_denoise(in: VertexOutput) -> @location(0) f32 {
    // Gather 3x3 neighborhood
    // Weight by edge values
    // Return blurred result
}
```

**Acceptance Criteria**:
- [ ] Shader compiles
- [ ] Output is smoother than input
- [ ] Edges are preserved (depth discontinuities not blurred)

#### Task 3.2: Add Denoise Render Pass
**File**: `crates/studio_core/src/deferred/ssao_denoise_node.rs`

Create new render graph node for denoising:
- Input: Raw AO texture, Edges texture
- Output: Denoised AO texture
- Support 1-3 passes (configurable)

**Acceptance Criteria**:
- [ ] Node integrates into render graph
- [ ] Pass count is configurable
- [ ] Output texture is correct size

#### Task 3.3: Integrate Denoise into Pipeline
**File**: `crates/studio_core/src/deferred/plugin.rs`

Wire up: Main SSAO -> Denoise -> Lighting

**Acceptance Criteria**:
- [ ] Full pipeline renders without errors
- [ ] Denoised output used in final lighting

### Phase 3 Verification Commands
```bash
# Compare noise before/after denoise
cargo run --example p20_ssao_test -- --mode ao_raw
cargo run --example p20_ssao_test -- --mode ao_denoised

# Noise variance should decrease significantly
```

### Phase 3 Automated Tests
```rust
#[test]
fn denoise_reduces_noise() {
    let raw = analyze_noise("screenshots/p20_ao_raw.png");
    let denoised = analyze_noise("screenshots/p20_ao_denoised.png");
    
    assert!(denoised.variance < raw.variance * 0.5, 
        "Denoise should reduce variance by >50%: raw={}, denoised={}", 
        raw.variance, denoised.variance);
}

#[test]
fn denoise_preserves_edges() {
    let raw = analyze_edges("screenshots/p20_ao_raw.png");
    let denoised = analyze_edges("screenshots/p20_ao_denoised.png");
    
    // Edge strength should remain similar
    assert!((denoised.edge_strength - raw.edge_strength).abs() < 0.1,
        "Edges should be preserved: raw={}, denoised={}",
        raw.edge_strength, denoised.edge_strength);
}
```

---

## Phase 4: Depth MIP Chain (Performance Optimization)

### Objective
Implement depth prefiltering for better cache performance and quality at large radii.

### Tasks

#### Task 4.1: Create Depth MIP Texture
**File**: `crates/studio_core/src/deferred/ssao.rs`

Allocate texture with 5 MIP levels:
```rust
pub struct ViewSsaoDepthMips {
    pub texture: CachedTexture,  // R16Float with 5 MIPs
}
```

**Acceptance Criteria**:
- [ ] Texture created with 5 MIP levels
- [ ] Format is R16Float or R32Float
- [ ] All MIP levels are writable (UAV)

#### Task 4.2: Create Depth Prefilter Compute Shader
**File**: `assets/shaders/ssao_prefilter_depth.wgsl`

Port `XeGTAO_PrefilterDepths16x16`:
```wgsl
@compute @workgroup_size(8, 8, 1)
fn cs_prefilter_depths(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>
) {
    // Read 2x2 raw depth
    // Convert to viewspace
    // Write MIP 0
    // Compute weighted average for MIPs 1-4
}
```

**Acceptance Criteria**:
- [ ] Compute shader compiles
- [ ] All 5 MIP levels populated
- [ ] Weighted average preserves depth edges

#### Task 4.3: Update Main Pass to Use MIPs
**File**: `assets/shaders/ssao.wgsl`

Sample from appropriate MIP based on sample offset:
```wgsl
let mip_level = clamp(log2(sample_offset_length) - depth_mip_sampling_offset, 0.0, 4.0);
let sample_depth = textureSampleLevel(depth_mips, sampler, uv, mip_level);
```

**Acceptance Criteria**:
- [ ] MIP selection is correct (visualize MIP level as color)
- [ ] Large radius samples use higher MIPs
- [ ] No visible quality regression

### Phase 4 Verification Commands
```bash
# Performance comparison
cargo run --example p20_ssao_test -- --mode benchmark --no-mips
cargo run --example p20_ssao_test -- --mode benchmark --with-mips

# MIP usage should improve frame time by >10% at high quality
```

### Phase 4 Automated Tests
```rust
#[test]
fn depth_mips_improve_performance() {
    let time_no_mips = benchmark_ssao(use_mips: false);
    let time_with_mips = benchmark_ssao(use_mips: true);
    
    let improvement = (time_no_mips - time_with_mips) / time_no_mips;
    assert!(improvement > 0.10, 
        "MIPs should improve perf by >10%: no_mips={}ms, with_mips={}ms",
        time_no_mips, time_with_mips);
}

#[test]
fn depth_mips_maintain_quality() {
    let quality_no_mips = analyze_ssao_quality("screenshots/p20_no_mips.png");
    let quality_with_mips = analyze_ssao_quality("screenshots/p20_with_mips.png");
    
    // Quality should not degrade significantly
    assert!((quality_no_mips.banding_score - quality_with_mips.banding_score).abs() < 0.05);
}
```

---

## Success Criteria Summary

### Must Pass (Blockers)
1. **No panics**: All test examples run to completion
2. **No banding**: Banding score < 0.1 on stair test
3. **Correct AO values**: Flat surface > 0.95, corners 0.2-0.6
4. **Edges preserved**: Denoise does not blur depth discontinuities

### Should Pass (Quality Gates)
1. **Noise reduction**: Denoise reduces variance by >50%
2. **Performance**: MIPs improve High quality by >10%
3. **Visual match**: Side-by-side with XeGTAO reference is acceptable

### Nice to Have
1. **Temporal stability**: No visible flickering when camera moves
2. **Bent normals**: Optional output for specular occlusion

---

## File Inventory

### New Files to Create
```
examples/p20_ssao_test.rs                    # Test harness
assets/worlds/ssao_test.voxworld             # Test geometry
assets/shaders/debug_depth.wgsl              # Depth visualization
assets/shaders/ssao_denoise.wgsl             # Denoise pass
assets/shaders/ssao_prefilter_depth.wgsl     # Depth MIP compute
crates/studio_core/src/deferred/ssao_metrics.rs      # Quality analysis
crates/studio_core/src/deferred/ssao_denoise_node.rs # Denoise render node
docs/XeGTAO_BASELINE_ARTIFACTS.md            # Baseline documentation
```

### Files to Modify
```
assets/shaders/ssao.wgsl                     # Main algorithm
crates/studio_core/src/deferred/ssao.rs      # Resources
crates/studio_core/src/deferred/ssao_node.rs # Render node
crates/studio_core/src/deferred/mod.rs       # Module exports
crates/studio_core/src/deferred/plugin.rs    # Pipeline integration
```

---

## Reference Materials

- **XeGTAO Source**: `XeGTAO/Source/Rendering/Shaders/XeGTAO.hlsli`
- **Constants Header**: `XeGTAO/Source/Rendering/Shaders/XeGTAO.h`
- **Entry Points**: `XeGTAO/Source/Rendering/Shaders/vaGTAO.hlsl`
- **Algorithm Paper**: [Practical Real-Time Strategies for Accurate Indirect Occlusion](https://www.activision.com/cdn/research/Practical_Real_Time_Strategies_for_Accurate_Indirect_Occlusion_NEW%20VERSION_COLOR.pdf)
