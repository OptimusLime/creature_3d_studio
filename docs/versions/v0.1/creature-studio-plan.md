# v0.1 Creature Studio - Rendering Pipeline

## Vision

Bonsai-quality voxel rendering in Bevy. Dark fantasy 80s aesthetic: magenta/cyan/purple glow on black void.

**Gold Standard**: Bonsai's output. We bend Bevy to match it.

---

## Development Philosophy

**Verification Is First-Class**: Every phase ends with a test that generates a screenshot. Work needed for verification (headless rendering, screenshot capture, output folders) is done upfront in Phase 0.

**SMART Tasks**: Each task is Specific, Measurable, Achievable, Relevant, Time-bound. "Done" means a concrete artifact exists.

**Manual Review Workflow**: Tests generate labeled screenshots → I report what's in them → you review the folder → we iterate.

**Bonsai Is The Reference**: We port Bonsai's pipeline. When uncertain, we look at Bonsai's code.

---

## Verification Infrastructure

**Screenshot Output**: `screenshots/` folder (gitignored)
- Naming: `{phase}_{step}_{description}.png`
- Example: `p1_01_black_void.png`, `p2_03_emission_visible.png`

**Verification Tests**: `cargo test --features screenshot-tests`
- Each test renders a scene and saves a screenshot
- Test passes if screenshot is generated (manual review for correctness)
- Later: automated pixel comparison against reference images

**Headless Rendering**: Required for CI and automated tests
- Bevy with `WinitSettings` for headless or `bevy_headless_render`
- Must capture framebuffer to PNG

---

## Phase 0: Verification Infrastructure

**Goal**: Ability to run a test that renders a scene and saves a screenshot.

**Why First**: All future phases depend on this. Can't verify anything without it.

### Tasks

| ID | Task | Done When | Estimate |
|----|------|-----------|----------|
| 0.1 | Create `screenshots/` folder, add to `.gitignore` | Folder exists, `git status` shows it ignored | 5 min |
| 0.2 | Add `screenshot-tests` feature flag to `Cargo.toml` | `cargo build --features screenshot-tests` succeeds | 10 min |
| 0.3 | Implement `save_screenshot(path: &str)` function in `studio_core` | Function compiles, takes path argument | 30 min |
| 0.4 | Create minimal Bevy app that clears to magenta (#FF00FF) | App runs, window shows solid magenta | 15 min |
| 0.5 | Write test `test_screenshot_capture` that renders 1 frame and saves to `screenshots/p0_test.png` | Running test creates file, file is valid PNG | 1 hr |
| 0.6 | Verify headless mode: test runs without opening a window | `cargo test test_screenshot_capture --features screenshot-tests` completes on headless CI | 1 hr |

### Verification

**Test Command**: `cargo test test_screenshot_capture --features screenshot-tests`

**Pass Criteria**:
1. Test exits with success
2. File `screenshots/p0_test.png` exists
3. File is a valid PNG (can be opened)
4. Image is solid magenta (RGB 255, 0, 255)

**Manual Check**: Open `screenshots/p0_test.png`, confirm solid magenta.

---

## Phase 1: Black Void + Camera

**Goal**: Render a black void with an orbiting camera. Screenshot shows black with no artifacts.

**Depends On**: Phase 0

### Tasks

| ID | Task | Done When | Estimate |
|----|------|-----------|----------|
| 1.1 | Set Bevy clear color to black RGB(0,0,0) | `ClearColor` resource set, window background is black not gray | 10 min |
| 1.2 | Add 3D perspective camera at position (0, 5, 10) looking at origin (0,0,0) | `Camera3d` entity exists with correct transform | 15 min |
| 1.3 | Implement orbit camera: mouse drag rotates camera around origin | Left-drag rotates azimuth, vertical drag rotates elevation, camera stays focused on origin | 45 min |
| 1.4 | Write test `test_p1_black_void` that saves `screenshots/p1_black_void.png` | File created, test passes | 15 min |

### Verification

**Test Command**: `cargo test test_p1_black_void --features screenshot-tests`

**Pass Criteria**:
1. File `screenshots/p1_black_void.png` exists
2. Image is 100% black (RGB 0,0,0 for all pixels)
3. No gray, no default Bevy background color, no artifacts

**Manual Check**: Open image, confirm pure black.

---

## Phase 2: Single Cube Rendering

**Goal**: Render one cube at origin. Screenshot shows a lit cube on black background.

**Depends On**: Phase 1

### Tasks

| ID | Task | Done When | Estimate |
|----|------|-----------|----------|
| 2.1 | Spawn a cube mesh at origin with dimensions 1x1x1 | `Mesh` entity with `Cuboid::new(1.0, 1.0, 1.0)` visible in scene | 15 min |
| 2.2 | Apply `StandardMaterial` with base_color white RGB(255,255,255) | Cube renders with solid color, not wireframe | 10 min |
| 2.3 | Add `DirectionalLight` pointing at origin from above-right | Cube shows shading (different brightness on different faces) | 15 min |
| 2.4 | Write test `test_p2_single_cube` that saves `screenshots/p2_single_cube.png` | File created, test passes | 15 min |

### Verification

**Test Command**: `cargo test test_p2_single_cube --features screenshot-tests`

**Pass Criteria**:
1. File `screenshots/p2_single_cube.png` exists
2. White/gray cube visible at center of image
3. Background is black (not gray)
4. Cube has visible shading (faces have different brightness)

**Manual Check**: Open image, confirm cube visible, lit, on black background.

---

## Phase 3: Voxel Data Structure + Lua Placement

**Goal**: Define voxel struct, place voxels via Lua, render them as cubes (one cube per voxel, naive).

**Depends On**: Phase 2, existing mlua integration from v0.1 bootstrap

### Tasks

| ID | Task | Done When | Estimate |
|----|------|-----------|----------|
| 3.1 | Define `Voxel` struct in `studio_core/src/voxel.rs`: `pub struct Voxel { pub color: [u8; 3], pub emission: u8 }` | Struct exists, compiles | 15 min |
| 3.2 | Define `VoxelChunk` struct: 16x16x16 dense `Option<Voxel>` array with `get(x,y,z)` and `set(x,y,z,voxel)` methods | Struct exists, methods work (unit test) | 30 min |
| 3.3 | Register Lua function `place_voxel(x, y, z, r, g, b, emission)` that inserts into global `VoxelChunk` | Lua call succeeds, voxel stored in chunk | 1 hr |
| 3.4 | Create `assets/scripts/test_creature.lua` that places 5 voxels in cross pattern: center (red), +x (green), -x (blue), +z (yellow), -z (cyan) | Script file exists, executes without error | 20 min |
| 3.5 | Implement system that iterates filled voxels and spawns cube entity per voxel at correct world position | Cubes appear at grid positions matching voxel coordinates | 1 hr |
| 3.6 | Set each cube's `StandardMaterial.base_color` from voxel RGB | Cubes show correct colors from script | 30 min |
| 3.7 | Write test `test_p3_lua_voxels` that runs the Lua script and saves `screenshots/p3_lua_voxels.png` | File created, test passes | 30 min |

### Verification

**Test Command**: `cargo test test_p3_lua_voxels --features screenshot-tests`

**Pass Criteria**:
1. File `screenshots/p3_lua_voxels.png` exists
2. Exactly 5 cubes visible
3. Cross pattern: 1 center, 4 adjacent on axes
4. Colors match: red center, green +x, blue -x, yellow +z, cyan -z
5. Black background

**Manual Check**: Open image, count 5 cubes, verify cross pattern, verify colors match expectations.

---

## Phase 4: Custom Vertex Format with Emission

**Goal**: Replace naive cube-per-voxel with single mesh using custom vertex format that includes emission.

**Reference**: `bonsai/src/engine/mesh.h:701-793`

**Depends On**: Phase 3

### Tasks

| ID | Task | Done When | Estimate |
|----|------|-----------|----------|
| 4.1 | Define `VoxelVertex` struct: `position: [f32; 3], normal: [f32; 3], color: [f32; 3], emission: f32` | Struct exists, implements `Vertex` trait for Bevy | 30 min |
| 4.2 | Create shader `voxel.wgsl` that reads all vertex attributes including emission | Shader compiles, emission accessible in fragment shader | 1 hr |
| 4.3 | Implement `fn build_chunk_mesh(chunk: &VoxelChunk) -> Mesh`: generate 6 quads (12 triangles) per filled voxel | Function returns valid `Mesh` with correct vertex count | 2 hr |
| 4.4 | Replace per-voxel cube entities with single mesh entity using custom material | Scene renders same visual output as Phase 3 | 1 hr |
| 4.5 | Add log statement printing entity count at startup | Log shows "1 mesh entity" not "5 cube entities" | 10 min |
| 4.6 | Write test `test_p4_custom_mesh` that saves `screenshots/p4_custom_mesh.png` | File created, test passes | 30 min |

### Verification

**Test Command**: `cargo test test_p4_custom_mesh --features screenshot-tests`

**Pass Criteria**:
1. File `screenshots/p4_custom_mesh.png` exists
2. Visual output matches Phase 3 exactly (same 5 cubes, same colors, same positions)
3. Log confirms single mesh entity (not 5 separate entities)

**Manual Check**: Compare `p4_custom_mesh.png` to `p3_lua_voxels.png` - should look identical.

---

## Phase 5: Emission Affects Brightness

**Goal**: Voxels with emission > 0 render brighter than base color. No bloom yet.

**Reference**: Bonsai emission bypass concept

**Depends On**: Phase 4

### Tasks

| ID | Task | Done When | Estimate |
|----|------|-----------|----------|
| 5.1 | Update `test_creature.lua`: place 4 voxels with same color (white) but emission values 0, 64, 128, 255 | Script places 4 white voxels with varying emission | 15 min |
| 5.2 | Update `voxel.wgsl` fragment shader: output `color * (1.0 + emission * EMISSION_MULTIPLIER)` | Shader compiles, emission affects output brightness | 30 min |
| 5.3 | Set `EMISSION_MULTIPLIER = 2.0` so emission=1.0 triples brightness | Emission=255 voxel is noticeably brighter than emission=0 | 15 min |
| 5.4 | Verify high-emission voxels don't clip to pure white (may need HDR camera) | Brightest voxel shows color, not pure white | 30 min |
| 5.5 | Write test `test_p5_emission` that saves `screenshots/p5_emission.png` | File created, test passes | 30 min |

### Verification

**Test Command**: `cargo test test_p5_emission --features screenshot-tests`

**Pass Criteria**:
1. File `screenshots/p5_emission.png` exists
2. 4 white voxels visible
3. Clear brightness gradient: leftmost darkest, rightmost brightest
4. Brightest voxel is not pure white (still has visible shading/color)

**Manual Check**: Open image, verify brightness progression, verify no pure white clipping.

---

## Phase 6: Bloom Post-Processing

**Goal**: High-emission voxels have visible bloom halo. Port Bonsai's mip-chain bloom.

**Reference**: 
- `bonsai/shaders/bloom_downsample.fragmentshader`
- `bonsai/shaders/bloom_upsample.fragmentshader`

**Depends On**: Phase 5

### Tasks

| ID | Task | Done When | Estimate |
|----|------|-----------|----------|
| 6.1 | Research Bevy post-processing: document how to add custom post-process pass in `docs/bevy-postprocess-notes.md` | Document exists with working example reference | 1 hr |
| 6.2 | Create `bloom_downsample.wgsl`: port Bonsai's 13-tap COD-style filter | Shader compiles | 2 hr |
| 6.3 | Create `bloom_upsample.wgsl`: port Bonsai's 9-tap tent filter | Shader compiles | 1 hr |
| 6.4 | Implement bloom render node: 3-level mip-chain downsample, then upsample back | Bloom textures generated, visible in debug | 3 hr |
| 6.5 | Create `composite.wgsl`: blend bloom (5%) onto main image | Final image includes bloom contribution | 1 hr |
| 6.6 | Write test `test_p6_bloom` that saves `screenshots/p6_bloom.png` | File created, test passes | 30 min |

### Verification

**Test Command**: `cargo test test_p6_bloom --features screenshot-tests`

**Pass Criteria**:
1. File `screenshots/p6_bloom.png` exists
2. Highest emission voxel has visible glow halo (color extends beyond cube edges)
3. Lower emission voxels have less or no bloom
4. Bloom is soft (blurred), not sharp edges
5. Background remains black (bloom doesn't affect non-emissive areas)

**Manual Check**: Open image, verify bloom halo around bright voxel, compare to Phase 5 (bloom should be new).

---

## Phase 7: Distance Fog

**Goal**: Voxels farther from camera fade toward fog color.

**Reference**: `bonsai/shaders/Lighting.fragmentshader:306-319`

**Depends On**: Phase 6

### Tasks

| ID | Task | Done When | Estimate |
|----|------|-----------|----------|
| 7.1 | Update test scene: place voxels at z = 0, -5, -15, -30 (increasing distance from camera) | 4 voxels at varying depths | 15 min |
| 7.2 | Add fog uniforms to shader: `fog_color: vec3`, `fog_max_dist: f32`, `fog_power: f32` | Uniforms accessible in fragment shader | 30 min |
| 7.3 | Port Bonsai fog calculation: `fog_contrib = pow(clamp(dist / max_dist, 0, 1), power)` | Fragment shader applies fog based on distance | 45 min |
| 7.4 | Set fog parameters: `fog_color = deep_purple (#1a0a2e)`, `fog_max_dist = 50.0`, `fog_power = 2.0` | Far voxels fade to purple | 15 min |
| 7.5 | Write test `test_p7_fog` that saves `screenshots/p7_fog.png` | File created, test passes | 30 min |

### Verification

**Test Command**: `cargo test test_p7_fog --features screenshot-tests`

**Pass Criteria**:
1. File `screenshots/p7_fog.png` exists
2. Near voxel (z=0): full color, no fog
3. Far voxel (z=-30): heavily faded toward purple
4. Clear depth gradient visible
5. Fog color is deep purple, not gray

**Manual Check**: Open image, verify distance-based fog gradient.

---

## Phase 8: Tone Mapping + Final Aesthetic

**Goal**: HDR tone mapping, final color grading. Match Bonsai's composite pass.

**Reference**: `bonsai/shaders/composite.fragmentshader:169-209`

**Depends On**: Phase 7

### Tasks

| ID | Task | Done When | Estimate |
|----|------|-----------|----------|
| 8.1 | Enable HDR camera in Bevy (`Camera { hdr: true, .. }`) | Scene renders in linear color space | 30 min |
| 8.2 | Port Bonsai tone mapping to `composite.wgsl` (AgX preferred, Reinhard fallback) | Tone mapping applied before final output | 2 hr |
| 8.3 | Ensure compositing order: main image → add bloom → tone map → gamma correct | Pipeline order verified in code comments | 30 min |
| 8.4 | Apply gamma correction (linear → sRGB) | Output matches expected sRGB | 30 min |
| 8.5 | Create final test scene in Lua: creature shape with dim body (emission=0), bright core (emission=200), bright eyes (emission=255) | Multi-part creature with varying emission | 30 min |
| 8.6 | Write test `test_p8_final` that saves `screenshots/p8_final_aesthetic.png` | File created, test passes | 30 min |

### Verification

**Test Command**: `cargo test test_p8_final --features screenshot-tests`

**Pass Criteria**:
1. File `screenshots/p8_final_aesthetic.png` exists
2. Creature shape visible: dim body, glowing core, bright eyes
3. Bloom halos around core and eyes
4. Background is void black
5. Colors are rich: not washed out, not oversaturated
6. Fog visible on any distant parts
7. **Image quality suitable for promotional screenshot**

**Manual Check**: This is the "poster test" - would you put this screenshot on a website?

---

## Phase Summary

| Phase | Goal | Verification Test | Key Output | Status |
|-------|------|-------------------|------------|--------|
| 0 | Screenshot infrastructure | `test_screenshot_capture` | `p0_test.png` (magenta) | ✅ Done |
| 1 | Black void + camera | `test_p1_black_void` | `p1_black_void.png` (black) | ✅ Done |
| 2 | Single lit cube | `test_p2_single_cube` | `p2_single_cube.png` | ✅ Done |
| 3 | Lua voxel placement | `test_p3_lua_voxels` | `p3_lua_voxels.png` (5 cubes) | ✅ Done |
| 4 | Custom mesh + vertex format | `test_p4_custom_mesh` | `p4_custom_mesh.png` | ✅ Done |
| 5 | Emission brightness | `test_p5_emission` | `p5_emission.png` (gradient) | ✅ Done |
| 6 | Bloom post-process | `test_p6_bloom` | `p6_bloom.png` (glow halo) | ✅ Done |
| 7 | Distance fog | `test_p7_fog` | `p7_fog.png` (depth fade) | ✅ Done |
| 8 | Forward render final | `p8_gbuffer` | Forward rendering complete | ✅ Done |
| 8b | Deferred pipeline proof | `p8_gbuffer` | Test cube through deferred | ✅ Done |
| **9** | **Voxel mesh integration** | `p8_gbuffer` | Lua voxels through deferred | **Current** |
| 10 | Deferred + bloom | TBD | Full deferred with bloom | Planned |

---

## Phase Dependencies

```
Phase 0: Verification Infrastructure
    ↓
Phase 1: Black Void + Camera
    ↓
Phase 2: Single Cube Rendering
    ↓
Phase 3: Voxel Data + Lua Placement
    ↓
Phase 4: Custom Vertex Format
    ↓
Phase 5: Emission Brightness
    ↓
Phase 6: Bloom Post-Processing
    ↓
Phase 7: Distance Fog
    ↓
Phase 8: Tone Mapping + Final
```

---

## Manual Review Workflow

1. I complete a phase's tasks
2. I run: `cargo test test_pN_xxx --features screenshot-tests`
3. I confirm screenshot saved: `ls screenshots/`
4. I describe what's in the screenshot (colors, shapes, any anomalies)
5. You review `screenshots/` folder
6. We discuss pass/fail
7. Iterate or move to next phase

---

## Files Created By This Plan

| Path | Purpose | Created In |
|------|---------|------------|
| `screenshots/` | Gitignored output folder | Phase 0 |
| `studio_core/src/screenshot.rs` | `save_screenshot()` function | Phase 0 |
| `studio_core/src/voxel.rs` | `Voxel`, `VoxelChunk` structs | Phase 3 |
| `assets/scripts/test_creature.lua` | Voxel placement test script | Phase 3 |
| `assets/shaders/voxel.wgsl` | Custom voxel shader | Phase 4 |
| `assets/shaders/bloom_downsample.wgsl` | Bloom pass 1 | Phase 6 |
| `assets/shaders/bloom_upsample.wgsl` | Bloom pass 2 | Phase 6 |
| `assets/shaders/composite.wgsl` | Final compositing | Phase 6/8 |
| `docs/bevy-postprocess-notes.md` | Post-process research | Phase 6 |
| `tests/screenshot_tests.rs` | All verification tests | Phase 0+ |

---

## Bonsai Reference Files

| Feature | File | Lines |
|---------|------|-------|
| Vertex material | `src/engine/mesh.h` | 701-793 |
| Bloom downsample | `shaders/bloom_downsample.fragmentshader` | all |
| Bloom upsample | `shaders/bloom_upsample.fragmentshader` | all |
| Fog calculation | `shaders/Lighting.fragmentshader` | 306-319 |
| Tone mapping | `shaders/composite.fragmentshader` | 169-209 |
| Emission handling | `shaders/Lighting.fragmentshader` | 382-402 |

---

## Phase 8b: Full Custom Deferred Rendering

**Goal**: Complete the deferred rendering pipeline - render actual geometry to G-buffer.

**Status**: ✅ MILESTONE ACHIEVED - Test cube renders through full deferred pipeline!

### Architecture

```
GBufferPassNode (MRT)              LightingPassNode (fullscreen)
     │                                      │
     ├─→ gColor (Rgba16Float)  ────────────┼─→ Sample & compute lighting
     ├─→ gNormal (Rgba16Float) ────────────┤   - Sun directional light
     ├─→ gPosition (Rgba32Float) ──────────┤   - Distance fog
     └─→ gDepth (Depth32Float) ────────────┘   - Emission support
                                                │
                                                └─→ ViewTarget (final output)
```

### Completed Tasks

| ID | Task | Status |
|----|------|--------|
| 8b.1 | Create `GBufferMaterial` struct with pipeline layout | ✅ Done |
| 8b.2 | Implement `GBufferGeometryPipeline` for MRT output | ✅ Done |
| 8b.3 | Create `gbuffer.wgsl` shader with MRT fragment output | ✅ Done |
| 8b.4 | Create `GBufferVertex` format (pos, normal, color, emission) | ✅ Done |
| 8b.5 | Create view/mesh uniform buffers and bind groups | ✅ Done |
| 8b.6 | Render test cube in GBufferPassNode | ✅ Done |
| 8b.7 | Verify: cube visible with proper lighting/fog | ✅ Done |

### Files Created

| File | Purpose |
|------|---------|
| `deferred/gbuffer_geometry.rs` | GBufferVertex, uniforms, test cube, pipeline |
| `deferred/gbuffer_material.rs` | SpecializedMeshPipeline (for future mesh integration) |
| `shaders/gbuffer.wgsl` | Vertex/fragment shader for MRT output |
| `shaders/gbuffer_test.wgsl` | Fullscreen test shader (proved MRT works) |

### What Works

- ✅ MRT rendering to 3 color targets + depth
- ✅ Custom vertex format with emission
- ✅ View/projection matrices
- ✅ Deferred lighting with sun direction
- ✅ Distance fog from linear depth
- ✅ Emission stored in albedo alpha

---

## Phase 9: Render Actual Voxel Meshes (CURRENT)

**Goal**: Replace test cube with actual voxel meshes from Bevy entities.

**Status**: In Progress

### Problem

Currently we render a hardcoded test cube. We need to:
1. Find entities with `Mesh3d` + `MeshMaterial3d<VoxelMaterial>` + camera visibility
2. Get their mesh GPU buffers from Bevy's mesh allocator
3. Get their transforms
4. Render them with our G-buffer pipeline

### Approach Options

**Option A: Extract from Bevy's RenderMeshInstances**
- Bevy already extracts mesh instances to render world
- Query `RenderMeshInstances` and `RenderAssets<RenderMesh>`
- Use existing GPU buffers, just different pipeline

**Option B: Custom Extraction**
- Add marker component `GBufferMesh` to entities
- Extract our own mesh data each frame
- More control, more code

**Option C: Hybrid - Use Bevy extraction, custom marker**
- Mark entities with `DeferredRenderable`
- In render world, query visible entities with this marker
- Get mesh handles from `RenderMeshInstances`
- Look up GPU buffers from `RenderAssets<RenderMesh>`

**Chosen**: Option C - Hybrid approach

### Tasks

| ID | Task | Done When | Status |
|----|------|-----------|--------|
| 9.1 | Add `DeferredRenderable` marker component | Component exists, extracted to render world | Pending |
| 9.2 | Query `RenderMeshInstances` for entities with marker | Can iterate deferred mesh instances | Pending |
| 9.3 | Look up `RenderMesh` GPU buffers | Have vertex/index buffer references | Pending |
| 9.4 | Look up entity transforms | Have world transform matrices | Pending |
| 9.5 | Update `GBufferPassNode` to iterate and draw each mesh | Multiple meshes rendered | Pending |
| 9.6 | Update uniform buffer to support per-mesh transforms | Each mesh at correct position | Pending |
| 9.7 | Verify: voxel meshes render through deferred pipeline | Screenshot shows Lua-defined voxels | Pending |

### Key References

- `bevy_pbr/src/render/mesh.rs` - `RenderMeshInstances`, `extract_meshes`
- `bevy_render/src/mesh/mod.rs` - `RenderMesh`, `RenderMeshBufferInfo`
- `bevy_render/src/mesh/allocator.rs` - `MeshAllocator` for GPU buffer lookup

### Verification

**Test**: `cargo run --example p8_gbuffer`

**Pass Criteria**:
1. Voxels from Lua script visible (not test cube)
2. Different colored faces (from voxel colors)
3. Proper lighting and fog
4. Multiple voxels at correct positions

---

## Phase 9: Render Actual Voxel Meshes

**Status**: COMPLETE

**What was built**:
- `DeferredRenderable` marker component for entities
- `ExtractedDeferredMesh` in render world with transforms
- Integration with `MeshAllocator` for GPU buffer access
- Per-mesh bind groups with transform uniforms
- `GBufferPassNode` iterates and renders all extracted meshes

**Verification**: 4 white voxels with emission gradient render correctly through deferred pipeline.

---

## Phase 9.5: Test Scene - "Mini World" (NEXT)

**Goal**: Create a meaningful test scene to validate the pipeline before optimization.

**Why**: We need visual feedback beyond 4 test boxes. A small voxel scene will:
- Reveal rendering issues (Z-fighting, culling bugs, etc.)
- Test performance with more geometry
- Provide a better demo for the deferred pipeline
- Give us something to compare before/after face culling

### Scene Concept: "Floating Island"

A small 16x16x16 chunk with:
- Grassy top layer (green, no emission)
- Dirt middle layer (brown)
- Stone base (gray)
- Glowing crystals (cyan/magenta, high emission)
- A small tree (brown trunk, green leaves)
- Sky void (purple fog)

### Tasks

| ID | Task | Done When | Status |
|----|------|-----------|--------|
| 9.5.1 | Create `test_island.lua` with procedural terrain | Script creates 200+ voxels | Pending |
| 9.5.2 | Add voxel colors: grass, dirt, stone, crystal | At least 5 distinct colors used | Pending |
| 9.5.3 | Add emission: crystals glow with 0.8+ emission | Visible emission on crystals | Pending |
| 9.5.4 | Create `p9_island.rs` example | Example runs, screenshot generated | Pending |
| 9.5.5 | Verify: Scene looks correct with fog + lighting | Manual review of screenshot | Pending |

### Verification

**Test**: `cargo run --example p9_island`

**Pass Criteria**:
1. Island visible with multiple colors
2. Fog fades distant parts to purple
3. Crystals glow brighter than terrain
4. No obvious rendering artifacts

---

## Phase 10: Bloom Post-Processing

**Goal**: Add bloom to the deferred pipeline for that signature Bonsai glow.

**Status**: Pending

### Architecture

```
Lighting Pass Output (HDR)
         │
         ▼
┌─────────────────┐
│ Bloom Threshold │  Extract pixels > threshold
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Downsample x6  │  13-tap filter, halve each pass
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Upsample x6   │  Tent filter, add to previous mip
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Composite     │  Add bloom to original, tone map
└─────────────────┘
```

### Tasks

| ID | Task | Done When | Status |
|----|------|-----------|--------|
| 10.1 | Create bloom threshold extraction shader | Bright pixels extracted to texture | Pending |
| 10.2 | Implement 6-level mip chain generation | Mip textures created in prepare | Pending |
| 10.3 | Create `bloom_downsample.wgsl` | Shader compiles, 13-tap filter | Pending |
| 10.4 | Create `bloom_upsample.wgsl` | Shader compiles, tent filter | Pending |
| 10.5 | Add BloomNode to render graph | Node runs after LightingPass | Pending |
| 10.6 | Composite bloom onto final output | Bloom visible on bright areas | Pending |
| 10.7 | Verify: Crystals have bloom halo | Screenshot shows glow | Pending |

### Reference

- Bonsai: `bloom_downsample.fragmentshader`, `bloom_upsample.fragmentshader`
- Technique: Dual Kawase blur (fast approximation)

---

## Phase 11: Face Culling

**Goal**: Only generate mesh faces that are visible (not occluded by adjacent voxels).

**Why**: A solid 16x16x16 chunk has 4096 voxels = 24576 faces. With face culling, a cube surface has ~1536 faces (6x16x16). **16x reduction** in geometry!

### Current State

```rust
// voxel_mesh.rs - generates ALL 6 faces per voxel
for (x, y, z, voxel) in chunk.iter() {
    add_cube_faces(...);  // Always adds 6 faces
}
```

### New Algorithm

```rust
for (x, y, z, voxel) in chunk.iter() {
    // Only add face if neighbor is empty
    if chunk.get(x+1, y, z).is_none() { add_face(+X); }
    if chunk.get(x-1, y, z).is_none() { add_face(-X); }
    if chunk.get(x, y+1, z).is_none() { add_face(+Y); }
    if chunk.get(x, y-1, z).is_none() { add_face(-Y); }
    if chunk.get(x, y, z+1).is_none() { add_face(+Z); }
    if chunk.get(x, y, z-1).is_none() { add_face(-Z); }
}
```

### Tasks

| ID | Task | Done When | Status |
|----|------|-----------|--------|
| 11.1 | Add `VoxelChunk::get_neighbor()` method | Can query adjacent voxels safely | Pending |
| 11.2 | Modify `build_chunk_mesh()` to cull occluded faces | Only visible faces generated | Pending |
| 11.3 | Add vertex/face count logging | Log shows reduced counts | Pending |
| 11.4 | Compare before/after screenshots | Visually identical | Pending |
| 11.5 | Benchmark: measure frame time improvement | Faster with culling | Pending |

### Verification

**Before** (Phase 9.5 island):
- Log: "Generated 24000 vertices, 36000 indices"
- Frame time: X ms

**After**:
- Log: "Generated 3000 vertices, 4500 indices" (example)
- Frame time: <X ms
- Screenshot: Identical to before

---

## Phase 12: Greedy Meshing

**Goal**: Merge adjacent same-material faces into larger quads.

**Why**: Face culling reduces hidden faces, but visible surfaces still have many small quads. Greedy meshing merges them.

Example: A 16x16 flat grass surface:
- Without greedy: 256 quads (16x16)
- With greedy: 1 quad (if all same material)

### Algorithm (Simplified)

For each face direction (e.g., +Y top faces):
1. Create 2D slice of exposed faces with materials
2. Sweep rows, grouping adjacent same-material faces
3. Extend groups vertically while material matches
4. Emit one quad per group

### Tasks

| ID | Task | Done When | Status |
|----|------|-----------|--------|
| 12.1 | Research greedy meshing algorithms | Document chosen approach | Pending |
| 12.2 | Implement per-face-direction greedy merge | Fewer quads for flat surfaces | Pending |
| 12.3 | Handle material boundaries correctly | Different colors don't merge | Pending |
| 12.4 | Benchmark improvement | Significant reduction in faces | Pending |
| 12.5 | Verify visual correctness | No artifacts at merged edges | Pending |

### Reference

- "Greedy Meshing Voxels" by Mikola Lysenko
- Bonsai's implementation in mesh generation code

---

## Phase 13: Multiple Chunks

**Goal**: Support a world with multiple chunk positions.

### Tasks

| ID | Task | Done When | Status |
|----|------|-----------|--------|
| 13.1 | Create `VoxelWorld` with HashMap<ChunkPos, VoxelChunk> | World struct exists | Pending |
| 13.2 | Spawn multiple mesh entities for chunks | Each chunk is separate entity | Pending |
| 13.3 | Position chunks at correct world coordinates | Chunks tile correctly | Pending |
| 13.4 | Create multi-chunk test scene | 3x3 chunk world | Pending |
| 13.5 | Verify chunk boundaries render correctly | No seams between chunks | Pending |

---

## Phase 10.8: Minecraft-Style Face Shading (QUICK WIN)

**Goal**: Add fixed brightness multipliers per face direction so blocks are distinguishable even on flat surfaces.

**Why**: Currently all faces pointing the same direction have identical shading. In Minecraft, faces have different base multipliers (top=1.0, bottom=0.5, sides=0.6-0.8) making blocks distinguishable.

**Status**: Pending

### Implementation

In `deferred_lighting.wgsl`, after N·L calculation:
```wgsl
// Minecraft-style face shading multipliers
var face_multiplier = 1.0;
if (abs(world_normal.y) > 0.9) {
    face_multiplier = select(0.5, 1.0, world_normal.y > 0.0); // top=1.0, bottom=0.5
} else if (abs(world_normal.z) > 0.9) {
    face_multiplier = 0.8; // north/south
} else {
    face_multiplier = 0.6; // east/west
}
total_light *= face_multiplier;
```

### Tasks

| ID | Task | Done When | Status |
|----|------|-----------|--------|
| 10.8.1 | Add face multiplier logic to lighting shader | Compile succeeds | Pending |
| 10.8.2 | Test: different faces have different brightness | Screenshot shows variation | Pending |
| 10.8.3 | Tune multiplier values for best visual result | Manual review approval | Pending |

**Estimate**: 15 minutes

---

## Phase 10.9: Per-Vertex Ambient Occlusion

**Goal**: Darken corners and edges where blocks meet for depth and block separation.

**Why**: AO is the primary technique Minecraft uses to make individual blocks distinguishable on flat surfaces. Corners get darker, giving visual "weight" to each block.

**Status**: Future

### Algorithm

During mesh generation in `build_chunk_mesh()`:
1. For each vertex, check the 3 corner-adjacent voxels
2. Count how many are solid (0-3)
3. Store AO value as vertex attribute (or bake into color)
4. Interpolate across face for smooth corner darkening

### Reference

- [0fps: Ambient Occlusion for Minecraft-like Worlds](https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/)

**Estimate**: 2-4 hours

---

## Roadmap Summary

```
Current State (Phase 9.5 Complete - Island renders)
        │
        ▼
┌────────────────────────┐
│ Phase 10: Bloom        │  Fix BloomNode borrow issue
│   (In Progress)        │  Glow on crystals
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Phase 10.8: Face       │  QUICK WIN - 15 min
│   Shading Multipliers  │  Block differentiation
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Phase 10.9: Vertex AO  │  Corner/edge darkening
│   (Optional)           │  Full Minecraft-style look
└────────┬───────────────┘
         │
         ▼
┌───────────────────┐
│ Phase 11: Face    │  Performance
│   Culling         │  16x geometry reduction
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│ Phase 12: Greedy  │  More performance
│   Meshing         │  Merge adjacent faces
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│ Phase 13: Multi   │  Voxel world
│   Chunk World     │  Larger scenes
└────────┬──────────┘
         │
         ▼
    Future: Shadow Mapping, SSAO, OIT, Creatures...
```

---

## Out of Scope (v0.1)

Deferred to future versions:

- **Creature entity system**: Behavior, animation, AI
- **Physics integration**: Collision, movement
- **SSAO**: Ambient occlusion for depth
- **Shadows**: Shadow mapping
- **OIT Transparency**: Order-independent transparency
- **Level of Detail**: Distant chunk simplification
- **Chunk streaming**: Load/unload based on camera
