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
| 9 | Voxel mesh integration | `p9_island` | Lua voxels through deferred | ✅ Done |
| 9.5 | Test scene - Island | `p9_island` | Floating island with crystals | ✅ Done |
| 10 | Bloom post-process | `p9_island` | Ping-pong bloom working | ✅ Done |
| 10.8 | Face shading multipliers | `p9_island` | Minecraft-style face shading | ✅ Done |
| 11 | Shadow Mapping | `p9_island` | Directional shadows with PCF | ✅ Done |
| 12 | Per-Vertex AO | `p9_island` | Minecraft-style corner darkening | ✅ Done |
| **13** | **Face Culling** | TBD | 16x geometry reduction | **NEXT** |
| 14 | Greedy Meshing | TBD | Merged faces | Planned |
| 15 | Multi-Chunk World | TBD | Larger scenes | Planned |

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

## Phase 11: Shadow Mapping (CURRENT PRIORITY)

**Goal**: Add directional shadow mapping for the sun light, creating proper cast shadows.

**Why**: Shadows are critical for visual quality and depth perception. Without shadows, objects appear to float and the scene lacks grounding. This is the #1 visual improvement after bloom.

**Reference**: Bonsai `DepthRTT.*` and `Lighting.fragmentshader:241-299`

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    SHADOW MAPPING PIPELINE                       │
└─────────────────────────────────────────────────────────────────┘

                    SHADOW PASS (runs first)
                           │
    ┌──────────────────────┴──────────────────────┐
    │                                              │
    │  Light-Space MVP                             │
    │  (Orthographic from sun direction)          │
    │           │                                  │
    │           ▼                                  │
    │  ┌─────────────────┐                        │
    │  │ Shadow Depth    │  Render scene from     │
    │  │ Texture         │  light's POV           │
    │  │ (2048x2048)     │  Store linear depth    │
    │  └─────────────────┘                        │
    │                                              │
    └──────────────────────────────────────────────┘
                           │
                           ▼
                    LIGHTING PASS
                           │
    ┌──────────────────────┴──────────────────────┐
    │                                              │
    │  For each fragment:                          │
    │  1. Transform to light space (ShadowMVP)    │
    │  2. Sample shadow map at light-space XY     │
    │  3. Compare fragment depth vs shadow depth  │
    │  4. If fragment deeper → in shadow          │
    │  5. Apply shadow factor to sun lighting     │
    │                                              │
    └──────────────────────────────────────────────┘
```

### Shadow Map Projection

For directional lights (sun), use **orthographic projection**:
- Fits the visible scene bounds
- Shadow map covers the area the camera can see
- No perspective distortion

```
Sun Direction: (0.3, -0.9, -0.3) (from above-right)

Light View Matrix:
  lookAt(lightPos, lightPos + sunDir, up)
  
Light Projection Matrix:
  ortho(-size, size, -size, size, near, far)
  size = scene bounds (e.g., 50 units)
```

### Shadow Quality Techniques

1. **Depth Bias** - Prevent shadow acne (self-shadowing artifacts)
   ```wgsl
   let bias = 0.005 * (1.0 - dot(normal, light_dir));
   ```

2. **PCF (Percentage Closer Filtering)** - Soft shadow edges
   ```wgsl
   // Sample 3x3 grid and average
   for dx in -1..2 {
     for dy in -1..2 {
       shadow += compare(depth, shadow_map[uv + offset]);
     }
   }
   shadow /= 9.0;
   ```

3. **Cascaded Shadow Maps (CSM)** - Future: multiple shadow maps at different distances

### Tasks

| ID | Task | Done When | Status |
|----|------|-----------|--------|
| 11.1 | Create `shadow_depth.wgsl` vertex/fragment shader | Renders depth from light POV | Pending |
| 11.2 | Create `ShadowPipeline` with orthographic projection | Pipeline compiles | Pending |
| 11.3 | Create `ViewShadowTextures` component (2048x2048 Depth32Float) | Texture allocated per camera | Pending |
| 11.4 | Create `ShadowPassNode` render graph node | Node runs before GBuffer | Pending |
| 11.5 | Calculate light-space MVP from sun direction | Correct projection | Pending |
| 11.6 | Render all meshes to shadow depth texture | Depth texture populated | Pending |
| 11.7 | Add shadow map sampler to lighting shader | Shadow uniform bound | Pending |
| 11.8 | Implement shadow sampling in `deferred_lighting.wgsl` | Basic hard shadows | Pending |
| 11.9 | Add depth bias to fix shadow acne | No self-shadowing artifacts | Pending |
| 11.10 | Implement PCF for soft shadows | Smooth shadow edges | Pending |
| 11.11 | Verify: Island scene shows tree shadow on grass | Screenshot review | Pending |

### Key Files to Create

| File | Purpose |
|------|---------|
| `deferred/shadow.rs` | ShadowPipeline, ViewShadowTextures, shadow config |
| `deferred/shadow_node.rs` | ShadowPassNode ViewNode implementation |
| `shaders/shadow_depth.wgsl` | Shadow pass vertex/fragment shaders |
| Updated `deferred_lighting.wgsl` | Shadow sampling in lighting |

### Shader Snippets

**shadow_depth.wgsl** (vertex shader):
```wgsl
struct ShadowUniforms {
    light_view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
}

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    let world_pos = uniforms.model * vec4(position, 1.0);
    return uniforms.light_view_proj * world_pos;
}

@fragment
fn fs_main() {
    // Depth written automatically to depth attachment
    // No color output needed
}
```

**deferred_lighting.wgsl** (shadow sampling):
```wgsl
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;

fn calculate_shadow(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    // Transform to light space
    let light_space_pos = shadow_mvp * vec4(world_pos, 1.0);
    let proj_coords = light_space_pos.xyz / light_space_pos.w;
    
    // Map from [-1,1] to [0,1] for texture sampling
    let shadow_uv = proj_coords.xy * 0.5 + 0.5;
    let current_depth = proj_coords.z;
    
    // Depth bias based on surface angle
    let bias = max(0.005 * (1.0 - dot(normal, sun_direction)), 0.001);
    
    // PCF 3x3 sampling
    var shadow = 0.0;
    let texel_size = 1.0 / 2048.0;
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow += textureSampleCompare(
                shadow_map, shadow_sampler,
                shadow_uv + offset, current_depth - bias
            );
        }
    }
    return shadow / 9.0;
}
```

### Verification

**Test**: `cargo run --example p9_island`

**Pass Criteria**:
1. Tree casts shadow on grass below
2. Island casts shadow on lower parts
3. No shadow acne (flickering on lit surfaces)
4. Shadow edges are soft (not hard pixelated)
5. Shadows align with sun direction
6. Performance acceptable (<16ms frame time)

### Reference: Bonsai Shadow Implementation

From `Lighting.fragmentshader:241-299`:
```glsl
if (UseShadowMapping) {
    f32 LinearDepth = Linearize(Depth, 5000.f, 0.1f);
    float acneBias = 0.045f * LinearDepth; // Fix acne - scales with distance

    v4 FragPShadowSpace = ShadowMVP * vec4(FragWorldP, 1.f);
    f32 FragDepth = FragPShadowSpace.z - acneBias;

    float ShadowSampleDepth = texture(shadowMap, FragPShadowSpace.xy).x;
    if (FragDepth > ShadowSampleDepth) { 
        ShadowVisibility -= vec3(1.f); 
    }
}
```

Key insights from Bonsai:
- Uses linear depth for shadow comparison
- Bias scales with fragment distance (prevents distant acne)
- Simple single-sample shadow (we'll improve with PCF)
- Shadow affects only key light (sun), not ambient/back light

---

## Phase 12: Per-Vertex Ambient Occlusion

**Goal**: Darken corners and edges where blocks meet for depth and block separation.

**Why**: AO is the primary technique Minecraft uses to make individual blocks distinguishable on flat surfaces. Corners get darker, giving visual "weight" to each block.

**Status**: After Shadows

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

## Phase 13: Face Culling

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

## Phase 14: Greedy Meshing

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
| 14.1 | Research greedy meshing algorithms | Document chosen approach | Pending |
| 14.2 | Implement per-face-direction greedy merge | Fewer quads for flat surfaces | Pending |
| 14.3 | Handle material boundaries correctly | Different colors don't merge | Pending |
| 14.4 | Benchmark improvement | Significant reduction in faces | Pending |
| 14.5 | Verify visual correctness | No artifacts at merged edges | Pending |

### Reference

- "Greedy Meshing Voxels" by Mikola Lysenko
- Bonsai's implementation in mesh generation code

---

## Phase 15: Multiple Chunks

**Goal**: Support a world with multiple chunk positions.

### Tasks

| ID | Task | Done When | Status |
|----|------|-----------|--------|
| 15.1 | Create `VoxelWorld` with HashMap<ChunkPos, VoxelChunk> | World struct exists | Pending |
| 15.2 | Spawn multiple mesh entities for chunks | Each chunk is separate entity | Pending |
| 15.3 | Position chunks at correct world coordinates | Chunks tile correctly | Pending |
| 15.4 | Create multi-chunk test scene | 3x3 chunk world | Pending |
| 15.5 | Verify chunk boundaries render correctly | No seams between chunks | Pending |

---

## Phase 11: Shadow Mapping (COMPLETE)

**Goal**: Add directional shadow mapping for the sun light, creating proper cast shadows.

**Status**: ✅ COMPLETE

### Implementation

**Architecture**:
```
SHADOW PASS (runs first)                    LIGHTING PASS (updated)
       │                                           │
       ▼                                           │
┌─────────────────┐                               │
│ Shadow Depth    │  Orthographic from sun       │
│ Texture         │  2048x2048 Depth32Float      │
│ (light-space)   │                               │
└────────┬────────┘                               │
         │                                         │
         └────────────────────────────────────────┤
                                                   ▼
                                          Sample shadow map
                                          Compare depths + bias
                                          PCF 3x3 for soft edges
```

**Files Created**:
| File | Purpose |
|------|---------|
| `deferred/shadow.rs` | ShadowConfig, ViewShadowTextures, ShadowPipeline |
| `deferred/shadow_node.rs` | ShadowPassNode ViewNode, ViewShadowUniforms |
| `shaders/shadow_depth.wgsl` | Depth-only vertex shader for shadow pass |

**Key Features**:
- 2048x2048 shadow map resolution
- Orthographic projection from sun direction
- Slope-scaled depth bias to prevent shadow acne
- PCF 3x3 filtering for soft shadow edges
- Shadow only affects sun light (ambient/fill unaffected)

**Verification**: Tree casts shadow on grass, visible shadow patches on terrain.

---

## Phase 12: Per-Vertex Ambient Occlusion (COMPLETE)

**Goal**: Darken corners and edges where blocks meet for visual depth and block separation.

**Status**: ✅ COMPLETE

### Implementation

**Algorithm** (from [0fps.net](https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/)):

For each vertex of a face, check 3 corner-adjacent voxels:
- side1: adjacent along one axis of the face
- side2: adjacent along the other axis
- corner: diagonally adjacent

```
AO = 1.0 - (side1 + side2 + corner) * 0.3
Special case: if both sides solid → AO = 0.0 (fully occluded)
```

**Files Modified**:
| File | Changes |
|------|---------|
| `voxel.rs` | Added `is_solid()`, `is_neighbor_solid()` helpers |
| `voxel_mesh.rs` | Added `ATTRIBUTE_VOXEL_AO`, `calculate_vertex_ao()`, `get_ao_offsets()` |
| `gbuffer_geometry.rs` | Added `ao` field to `GBufferVertex`, updated stride to 44 bytes |
| `gbuffer.wgsl` | Added `voxel_ao` input, stored in `g_normal.a` |
| `deferred_lighting.wgsl` | Read AO from `normal_sample.a`, apply as final multiplier |

**Key Features**:
- Per-vertex AO calculated at mesh generation time (zero runtime cost)
- Smooth interpolation across faces (GPU hardware interpolates vertex attributes)
- AO stored in G-buffer normal.a channel
- Applied as multiplier to all lighting (Minecraft-style)
- Debug mode 5 shows AO visualization

**Verification**: Corners and edges visibly darker, blocks "pop" with depth.

---

## Phase 10.8: Minecraft-Style Face Shading (COMPLETE)

**Goal**: Add fixed brightness multipliers per face direction so blocks are distinguishable even on flat surfaces.

**Status**: ✅ COMPLETE

Implementation in `deferred_lighting.wgsl`:
```wgsl
var face_multiplier = 1.0;
if abs(world_normal.y) > 0.9 {
    face_multiplier = select(0.5, 1.0, world_normal.y > 0.0); // top=1.0, bottom=0.5
} else if abs(world_normal.z) > 0.9 {
    face_multiplier = 0.8; // north/south
} else {
    face_multiplier = 0.6; // east/west
}
total_light *= face_multiplier;
```

---

## Roadmap Summary - Lighting Focus

**Current State**: Dark World scene with dual moons and basic point lights working.

**Priority Order** (based on user preference):
1. **A. High Volume Lights** - Many lights in same space
2. **B. Point Light Shadows** - Lights cast shadows
3. **C. Auto-gen Point Lights** - Generate from emissive voxels
4. **D. Multi-Shadow (Moons)** - Both moons cast shadows
5. **E. Day/Night Cycle** - Time-based moon transitions
6. **F. Multi-frame Render** - Capture day/night sequence

```
Current State (Phase 13 Complete - Point Lights Working!)
        │
        ▼
┌────────────────────────┐
│ Phase 14: HIGH VOLUME  │  ◀◀◀ CURRENT PRIORITY
│   POINT LIGHTS         │  256+ lights, GPU optimization
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Phase 15: POINT LIGHT  │  
│   SHADOWS              │  Cube shadow maps per light
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Phase 16: AUTO-GEN     │
│   POINT LIGHTS         │  From emissive voxels
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Phase 17: MULTI-SHADOW │
│   (BOTH MOONS)         │  Second shadow map for orange moon
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Phase 18: DAY/NIGHT    │
│   CYCLE                │  Time-based moon colors, dusk/dawn
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Phase 19: MULTI-FRAME  │
│   RENDER               │  Capture day/night sequence
└────────┬───────────────┘
         │
         ▼
    Future: Face Culling, Greedy Meshing, SSAO...
```

### Phase 13: Point Lights (COMPLETE)

**Status**: ✅ COMPLETE

**What was built**:
- `DeferredPointLight` component with color, intensity, radius
- Point light extraction to render world via `ExtractedPointLights` resource
- Uniform buffer with up to 32 lights (MAX_POINT_LIGHTS)
- Smooth quadratic attenuation: `(1 - (d/r)²)²`
- Point light contribution in `deferred_lighting.wgsl`
- Debug mode 6 for point lights visualization

**Verification**: Dark world scene shows colored illumination from crystals on nearby surfaces.

---

### Phase 14: High Volume Point Lights (NEXT)

**Goal**: Support 256+ point lights efficiently for dense light fields.

**Why**: Current MAX_POINT_LIGHTS=32 is too low for scenes with many glowing voxels.

**Approach**:
1. Increase MAX_POINT_LIGHTS to 256 (or use storage buffer for unlimited)
2. Implement light culling (only process lights near fragment)
3. Consider clustered deferred lighting for better scaling

**Tasks**:
| ID | Task | Status |
|----|------|--------|
| 14.1 | Increase MAX_POINT_LIGHTS to 256 | Pending |
| 14.2 | Switch from uniform to storage buffer | Pending |
| 14.3 | Add distance culling in shader | Pending |
| 14.4 | Test with 100+ lights | Pending |
| 14.5 | Profile GPU performance | Pending |

---

### Phase 15: Point Light Shadows

**Goal**: Point lights cast shadows using cube shadow maps.

**Why**: Without shadows, lights "bleed" through walls unrealistically.

**Architecture**:
```
For each shadow-casting point light:
  Render 6 faces of cube shadow map
  Sample cube map in lighting pass
  Compare distance to light vs shadow depth
```

**Complexity**: HIGH - 6 render passes per shadow-casting light, expensive!

**Optimization**: Limit shadow-casting lights (nearest 4-8 only)

---

### Phase 16: Auto-Generate Point Lights from Emissive Voxels

**Goal**: Automatically create point lights at emissive voxel positions.

**Algorithm**:
1. During mesh build, collect emissive voxel positions
2. For each emissive voxel:
   - Create `DeferredPointLight` at voxel center
   - Color = voxel color
   - Intensity = emission * scale_factor
   - Radius = emission-based (brighter = farther reach)

**Why**: No manual placement needed. Glowing crystals automatically illuminate.

---

### Phase 17: Multi-Shadow (Both Moons)

**Goal**: Both purple and orange moons cast independent shadows.

**Implementation**:
- Second 2048x2048 shadow map
- Second light-space matrix
- Sample both in lighting shader
- Each moon's shadow only affects its own contribution

---

### Phase 18: Day/Night Cycle

**Goal**: Time-based moon positions and colors with dusk/dawn transitions.

**Concept**:
```
Time 0.0  (Midnight): Both moons high, full intensity
Time 0.25 (Dawn):     Purple moon setting (orange tint), orange rising
Time 0.5  (Midday):   Moons below horizon, only ambient (or sun mode?)
Time 0.75 (Dusk):     Orange moon setting (red tint), purple rising
Time 1.0  (Midnight): Back to start
```

**Implementation**:
- `DayNightCycle` resource with time (0.0-1.0)
- System updates moon direction, color, intensity
- Pass to shader via uniforms

---

### Phase 19: Multi-Frame Render

**Goal**: Capture day/night cycle as image sequence.

**Implementation**:
- Loop time from 0.0 to 1.0
- Render and save screenshot at each step
- Output: `screenshots/cycle_000.png` through `cycle_100.png`

---

## Deferred Optimization Phases

These are lower priority than lighting features:

### Phase 20: Face Culling
Only generate mesh faces not occluded by adjacent voxels. 16x geometry reduction.

### Phase 21: Greedy Meshing  
Merge adjacent same-material faces into larger quads.

### Phase 22: Multi-Chunk World
Support worlds larger than single 16x16x16 chunk.

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
