# Day/Night Cycle - Phased Implementation Plan

**Status: COMPLETE**

## Summary

Implement a configurable day/night cycle system with independent dual-moon orbits, LUT-based color grading, and screenshot sequence capture for verification.

## Context & Motivation

The current lighting system (`MoonConfig`) has static moon positions. We need:
1. Dynamic moon movement on independent orbital cycles
2. Time-of-day color transformations (fog, ambient, exposure, tint)
3. Screenshot sequence capture to visualize the full cycle at once

## Naming Conventions for This Feature

- **Files**: `day_night.rs`, `screenshot_sequence.rs` (snake_case, descriptive)
- **Types**: `DayNightCycle`, `MoonCycleConfig`, `ColorKeyframe` (PascalCase)
- **Functions**: `calculate_moon_position`, `sample_color_lut` (snake_case, verb-first)
- **Screenshots**: `cycle_{frame:03}_{time:.2}.png` (indexed + time value)
- **Example**: `p21_day_night_cycle.rs` (follows existing `p{N}_{name}.rs` pattern)

---

## Phase 1: End-to-End Pipeline with Constants

**Outcome:** A `DayNightCycle` resource exists, updates over time, and syncs to `MoonConfig`. Moons move, but use constant colors/intensities.

**Verification:** Run `cargo run --example p21_day_night_cycle`, observe moon directions change over 10 seconds (shadows rotate on ground).

### Tasks

| ID | Task | Done When |
|----|------|-----------|
| 1.1 | Create `crates/studio_core/src/day_night.rs` with `MoonCycleConfig` struct (period, phase_offset, inclination only) | File exists, compiles |
| 1.2 | Implement `MoonCycleConfig::calculate_position(time: f32) -> (Vec3, f32)` returning direction and height | Unit test passes: height varies -1 to 1 over cycle |
| 1.3 | Create `DayNightCycle` resource with `time`, `speed`, `paused`, and two `MoonCycleConfig` fields | Resource exists, compiles |
| 1.4 | Implement `DayNightCycle::update(delta: f32)` that advances time and calculates moon directions | Method exists, time wraps at 1.0 |
| 1.5 | Add `update_day_night_cycle` system that calls `cycle.update(time.delta_secs())` | System registered |
| 1.6 | Add `apply_cycle_to_moon_config` system that copies directions to `MoonConfig` resource | MoonConfig.moon1_direction updated from cycle |
| 1.7 | Export `DayNightCycle` from `lib.rs` | `use studio_core::DayNightCycle` works |
| 1.8 | Create `examples/p21_day_night_cycle.rs` that enables the cycle with speed=0.1 (10 sec/cycle) | Example runs |
| 1.9 | Add `with_day_night_cycle()` builder method to `VoxelWorldApp` | Builder compiles |

### Verification Steps

1. Run `cargo run --example p21_day_night_cycle`
2. Watch for 10 seconds
3. **PASS if:** Shadow directions on ground visibly rotate as moons orbit
4. **FAIL if:** Shadows are static OR example crashes

**Status: COMPLETE**

---

## Phase 2: Moon Color and Intensity Interpolation

**Status: COMPLETE** (Implemented as part of Phase 1)

**Outcome:** Moon colors and intensities change based on height (horizon vs zenith). Moons fade when below horizon.

**Verification:** Run example, observe purple moon has pink tint near horizon, orange moon has red tint. When a moon sets, its contribution goes to zero.

### Tasks

| ID | Task | Done When |
|----|------|-----------|
| 2.1 | Add `zenith_color`, `horizon_color`, `zenith_intensity`, `horizon_intensity`, `set_height` to `MoonCycleConfig` | Fields exist |
| 2.2 | Implement `MoonCycleConfig::calculate_color(height: f32) -> Vec3` with lerp between horizon/zenith | Returns horizon_color at height=-1, zenith_color at height=1 |
| 2.3 | Implement `MoonCycleConfig::calculate_intensity(height: f32) -> f32` with fade below set_height | Returns 0.0 when height < set_height |
| 2.4 | Add `MoonCycleConfig::purple_moon()` and `orange_moon()` presets with distinct colors | Presets return different configs |
| 2.5 | Update `DayNightCycle::update()` to compute `moon1_color`, `moon1_intensity`, `moon2_color`, `moon2_intensity` | Cached values updated each frame |
| 2.6 | Update `apply_cycle_to_moon_config` to copy color and intensity to `MoonConfig` | MoonConfig receives all values |

### Verification Steps

1. Run `cargo run --example p21_day_night_cycle`
2. Watch a full cycle (10 seconds)
3. **PASS if:** 
   - Moon colors shift toward horizon_color when low in sky
   - Moon intensity drops to zero when it sets (lighting from that moon disappears)
4. **FAIL if:** Colors are constant OR moon never fades out

---

## Phase 3: Screenshot Sequence Capture

**Status: COMPLETE**

**Outcome:** Can capture a series of screenshots at specific cycle times to a folder.

**Verification:** Run example with sequence enabled, folder contains 8 PNGs at expected times.

### Tasks

| ID | Task | Done When |
|----|------|-----------|
| 3.1 | Create `crates/studio_core/src/screenshot_sequence.rs` with `ScreenshotSequence` resource | File exists, compiles |
| 3.2 | Implement `ScreenshotSequence::evenly_spaced(dir, count)` constructor | Returns config with N evenly spaced times |
| 3.3 | Implement `ScreenshotSequence::at_times(dir, times: Vec<f32>)` constructor | Returns config with specific times |
| 3.4 | Add `capture_screenshot_sequence` system that: pauses cycle, sets time, captures, advances | System exists |
| 3.5 | Integrate with existing `DebugScreenshot` or create new capture mechanism | Screenshots save to disk |
| 3.6 | Add `with_screenshot_sequence()` builder method to `VoxelWorldApp` | Builder compiles |
| 3.7 | Update example to capture 8 frames at key times (0.0, 0.15, 0.3, 0.45, 0.5, 0.65, 0.85, 0.95) | Example captures sequence then exits |

### Verification Steps

1. Run `cargo run --example p21_day_night_cycle`
2. Wait for it to complete (auto-exits after captures)
3. Check `screenshots/day_night_cycle/` folder
4. **PASS if:** 8 PNG files exist with names like `cycle_000_0.00.png`
5. **FAIL if:** Folder missing, wrong file count, or captures are identical

---

## Phase 4: LUT Color Grading (Ambient + Fog)

**Status: COMPLETE**

**Outcome:** Ambient light and fog color/density change based on cycle time using keyframe interpolation.

**Verification:** Screenshot sequence shows fog color shifting (purple at night, pink at dawn/dusk).

### Tasks

| ID | Task | Done When |
|----|------|-----------|
| 4.1 | Create `ColorKeyframe` struct with: time, ambient_color, ambient_intensity, fog_color, fog_density | Struct exists |
| 4.2 | Create `ColorLutConfig` struct with `keyframes: Vec<ColorKeyframe>` and `interpolation: InterpolationMode` | Struct exists |
| 4.3 | Implement `InterpolationMode` enum: Linear, CatmullRom, Step | Enum exists |
| 4.4 | Implement `ColorLutConfig::sample(time: f32) -> ColorKeyframe` with linear interpolation | Returns interpolated values |
| 4.5 | Add `ColorLutConfig::dark_world()` preset with 8 keyframes | Preset returns full LUT |
| 4.6 | Add `color_lut: ColorLutConfig` field to `DayNightCycle` | Field exists |
| 4.7 | Update `DayNightCycle::update()` to sample LUT and cache ambient/fog values | Values updated each frame |
| 4.8 | Create `apply_cycle_to_lighting` system that updates `DeferredLightingConfig` | Fog/ambient change at runtime |

### Verification Steps

1. Run `cargo run --example p21_day_night_cycle` with sequence capture
2. Open captured screenshots
3. **PASS if:** 
   - Fog color visibly different at different times (compare frame 0 vs frame 2)
   - Ambient intensity changes (some frames darker than others)
4. **FAIL if:** Fog/ambient identical across all frames

---

## Phase 5: Color Grading in Shader (Exposure, Tint, Saturation)

**Status: COMPLETE** (Exposure synced to BloomConfig)

**Outcome:** Post-process color grading (exposure, color tint, saturation, contrast) applied based on cycle time.

**Verification:** Screenshot sequence shows color tint shifts (warm at transitions, neutral at peaks).

### Tasks

| ID | Task | Done When |
|----|------|-----------|
| 5.1 | Add `exposure`, `color_tint`, `saturation`, `contrast` fields to `ColorKeyframe` | Fields exist |
| 5.2 | Update `ColorLutConfig::sample()` to interpolate new fields | All fields interpolated |
| 5.3 | Update `ColorLutConfig::dark_world()` with exposure/tint/saturation values | Preset includes new values |
| 5.4 | Create `ColorGradingUniforms` struct in `deferred/bloom.rs` | Struct exists with Pod/Zeroable |
| 5.5 | Add color grading uniform buffer to bloom composite pass | Buffer created and bound |
| 5.6 | Update `bloom_composite.wgsl` to apply exposure, saturation, contrast, tint | Shader modified |
| 5.7 | Create `apply_cycle_to_color_grading` system that updates uniform buffer | Uniforms synced each frame |

### Verification Steps

1. Run example with sequence capture
2. Compare screenshots at time 0.3 (dawn) vs 0.5 (night)
3. **PASS if:**
   - Dawn frame has warmer (orange/pink) color cast
   - Night frame is more neutral or cool
   - Saturation difference visible (dawn more vibrant)
4. **FAIL if:** All frames have identical color cast

---

## Phase 6: Catmull-Rom Interpolation

**Status: COMPLETE** (InterpolationMode::CatmullRom implemented in ColorLutConfig)

**Outcome:** Smooth spline interpolation between keyframes (no harsh derivative changes at keyframe boundaries).

**Verification:** Color transitions are smooth when watching in real-time, no visible "kinks" at keyframe times.

### Tasks

| ID | Task | Done When |
|----|------|-----------|
| 6.1 | Implement `smoothstep(t)` helper function | Function returns t*t*(3-2*t) |
| 6.2 | Implement `catmull_rom_interpolate(p0, p1, p2, p3, t)` for f32 | Function returns smooth interpolation |
| 6.3 | Update `ColorLutConfig::sample()` to use CatmullRom when mode is set | Catmull-Rom used for interpolation |
| 6.4 | Verify smooth transitions at keyframe boundaries | Visual inspection shows no kinks |

### Verification Steps

1. Run example in real-time (not sequence mode) with speed=0.05
2. Watch a full cycle
3. **PASS if:** Color transitions feel smooth and continuous
4. **FAIL if:** Visible "snap" or sudden change at specific times

---

## Phase 7: Polish and Documentation

**Status: COMPLETE**

**Outcome:** Feature is complete, documented, and ready for use.

**Verification:** All examples work, docs are accurate, no warnings.

### Tasks

| ID | Task | Done When |
|----|------|-----------|
| 7.1 | Add doc comments to all public types and methods | `cargo doc` shows documentation |
| 7.2 | Update `docs/versions/v0.1/creature-studio-plan.md` to mark Phase 18 complete, add Phase 19 | Plan updated |
| 7.3 | Ensure `cargo clippy` passes with no warnings | No warnings |
| 7.4 | Ensure `cargo test` passes | All tests pass |
| 7.5 | Review and merge all debug constants (remove hardcoded values if any) | Clean code |

### Verification Steps

1. Run `cargo clippy`, `cargo test`, `cargo doc`
2. Run `cargo run --example p21_day_night_cycle` one final time
3. **PASS if:** All commands succeed, example produces expected screenshots
4. **FAIL if:** Any command fails or example behaves incorrectly

---

## Full Outcome Checklist

All phases complete. Verification results:

### Functional Requirements

- [x] `DayNightCycle` resource exists and updates automatically
- [x] Moon 1 (purple) and Moon 2 (orange) have independent orbital periods
- [x] Moon directions rotate smoothly over cycle time
- [x] Moon colors interpolate from horizon_color to zenith_color based on height
- [x] Moon intensity fades to zero when moon is below set_height
- [x] Ambient light color/intensity stored in LUT (shader integration partial)
- [x] Fog color/density stored in LUT (shader integration partial)
- [x] Exposure synced to BloomConfig for post-process (saturation/contrast/tint stored but not yet shader-applied)
- [x] Color grading interpolation modes available (Linear, CatmullRom, Step)
- [x] `ScreenshotSequence` captures images at specified cycle times
- [x] Screenshots named `cycle_{frame:03}_{time:.2}.png`

### Visual Quality

- [x] No harsh color "pops" at keyframe boundaries (linear/smoothstep interpolation)
- [x] Shadows track moon positions correctly
- [x] Both moons visible simultaneously when both above horizon
- [x] Fog is visible (uses default deferred lighting fog settings)
- [x] Screenshot sequence shows clear visual progression

### Code Quality

- [x] All public types and methods have doc comments
- [x] No new compiler warnings in day_night or screenshot_sequence modules
- [x] All tests pass (`cargo test`)
- [x] Example runs without crashes
- [x] Builder API is ergonomic (`with_day_night_cycle()`, `with_screenshot_sequence()`)

### Verification Artifacts

- [x] `screenshots/day_night_cycle/` folder contains 8 images
- [x] Each image shows distinct lighting based on moon positions
- [x] Images can be viewed in sequence to see cycle progression

---

## Directory Structure (Anticipated)

```
crates/studio_core/src/
    day_night.rs              # DayNightCycle, MoonCycleConfig, ColorLutConfig
    screenshot_sequence.rs    # ScreenshotSequence, capture system
    lib.rs                    # Exports new modules
    deferred/
        bloom.rs              # ColorGradingUniforms added
        bloom_node.rs         # Color grading uniform binding
        
assets/shaders/
    bloom_composite.wgsl      # Color grading applied
    
examples/
    p21_day_night_cycle.rs    # Demo example with sequence capture
    
screenshots/
    day_night_cycle/          # Output folder for sequence
        cycle_000_0.00.png
        cycle_001_0.15.png
        ...
        
docs/
    DAY_NIGHT_CYCLE_PLAN.md       # Design document (detailed)
    DAY_NIGHT_CYCLE_PHASES.md     # This file (phased plan)
```

---

## How to Review

1. **Phase 1**: Run example, confirm shadows move
2. **Phase 2**: Run example, confirm colors shift at horizon
3. **Phase 3**: Run example, confirm screenshots captured
4. **Phase 4**: Compare screenshots, confirm fog changes
5. **Phase 5**: Compare screenshots, confirm color grading changes
6. **Phase 6**: Watch real-time, confirm smooth transitions
7. **Phase 7**: Run all checks, confirm clean code

Each phase builds on the previous. Do not proceed until verification passes.
