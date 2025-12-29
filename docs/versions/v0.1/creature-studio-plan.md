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

| Phase | Goal | Verification Test | Key Output |
|-------|------|-------------------|------------|
| 0 | Screenshot infrastructure | `test_screenshot_capture` | `p0_test.png` (magenta) |
| 1 | Black void + camera | `test_p1_black_void` | `p1_black_void.png` (black) |
| 2 | Single lit cube | `test_p2_single_cube` | `p2_single_cube.png` |
| 3 | Lua voxel placement | `test_p3_lua_voxels` | `p3_lua_voxels.png` (5 cubes) |
| 4 | Custom mesh + vertex format | `test_p4_custom_mesh` | `p4_custom_mesh.png` |
| 5 | Emission brightness | `test_p5_emission` | `p5_emission.png` (gradient) |
| 6 | Bloom post-process | `test_p6_bloom` | `p6_bloom.png` (glow halo) |
| 7 | Distance fog | `test_p7_fog` | `p7_fog.png` (depth fade) |
| 8 | Tone mapping + final | `test_p8_final_aesthetic` | `p8_final_aesthetic.png` |

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

## Out of Scope

Documented separately in `creature-systems-plan.md` (to be created after Phase 8):

- Creature entity structure
- Lua behavior scripts
- Animation system
- Physics integration
- Face culling / greedy meshing optimization
- Deferred rendering / G-buffer
- OIT transparency
- SSAO
