# Handoff: Visual Fidelity Improvements Feature Branch

## Branch Information
- **Branch:** `feature/visual-fidelity-improvements`
- **Base:** `main`
- **Status:** Phases 0-3 complete, Phase 4 next

---

## Critical Documents to Read First

### 1. How We Work (MANDATORY)
**File:** `docs/HOW_WE_WORK.md`

This document defines our collaboration process. Key principles:
- **Verification is first-class**: Every phase must have simple, automated verification
- **Facade pattern**: Build end-to-end pipeline first with trivial output, then complexify
- **No manual verification**: Use automated screenshots, not "run and look around"
- **Hypothesis-driven debugging**: When something fails, form hypothesis, test, iterate
- **Never abandon tasks**: Debug systematically, don't substitute simpler alternatives

### 2. The Plan Document (MANDATORY)
**File:** `docs/plans/visual_fidelity_improvements.md`

This is the implementation plan with 9 phases (0-8). Each phase has:
- **Outcome**: What will be true when complete
- **Verification**: How to prove it's done (must be simple bash commands + screenshot checks)
- **Tasks**: Specific file paths and changes

---

## Current State

### Completed Phases

#### Phase 0: Visual Verification Test Harness
**File:** `examples/p31_visual_fidelity_test.rs`

Test harness captures screenshots from multiple camera angles with time-of-day control:
- Output: `screenshots/visual_fidelity_test/`
- Captures: `sky_up.png`, `sky_horizon.png`, `moon_time_00.png`, `moon_time_25.png`, `building_front.png`, `building_aerial.png`, `terrain_distance.png`
- Each capture can specify `time_of_day: Option<f32>` to position moons

#### Phase 1: Sky Dome Pipeline (Facade)
- Created `sky_dome.rs`, `sky_dome_node.rs`, `sky_dome.wgsl`
- Constant purple sky where depth > 999.0 (no geometry)
- Render graph: `BloomPass -> SkyDomePass -> MainTransparentPass`

#### Phase 2: Sky Gradient
- Horizon-to-zenith gradient based on view direction
- Uses inverse view-projection matrix to reconstruct view direction from UV
- `SkyDomeConfig`: `horizon_color`, `zenith_color`, `horizon_blend_power`

#### Phase 3: Moon Rendering with Time-of-Day Control
- Dual moons (purple and orange) with configurable appearance
- `time_of_day` parameter (0.0-1.0) controls moon orbital positions
- `MoonAppearance`: size, color, glow_intensity, glow_falloff
- Moon disc rendering with soft glow falloff
- Horizon fade effect dims moons near horizon

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test
# Check screenshots/visual_fidelity_test/
# - sky_up.png: dark zenith
# - sky_horizon.png: gradient with moon glow
# - moon_time_00.png: moons at time 0.0
# - moon_time_25.png: moons at time 0.25 (different position)
```

---

## Next Phase: Phase 4 - Moon Horizon Effects

**Goal:** Moons near horizon show atmospheric color shift (warmer/more saturated).

**Tasks:**
1. Add horizon tinting to moon rendering in `sky_dome.wgsl`:
   - Blend moon color toward horizon_color based on moon altitude
   - Increase saturation near horizon
2. Add `horizon_tint_strength` parameter to `SkyDomeConfig`
3. Update test to capture moon at different altitudes

**Verification:**
```bash
cargo run --example p31_visual_fidelity_test
# Compare moon color at zenith vs near horizon
# Horizon moon should appear more saturated/warm
```

---

## Remaining Phases

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Test Harness | DONE |
| 1 | Sky Dome Pipeline (Facade) | DONE |
| 2 | Sky Gradient | DONE |
| 3 | Moon Rendering | DONE |
| 4 | Moon Horizon Effects | **NEXT** |
| 5 | Mystery Palette | Pending |
| 6 | Voxel Scale | Pending |
| 7 | Extended Terrain | Pending |
| 8 | Height Fog | Pending |

---

## Key Files

### Sky Dome Implementation
- `crates/studio_core/src/deferred/sky_dome.rs` - `SkyDomeConfig`, `MoonAppearance`
- `crates/studio_core/src/deferred/sky_dome_node.rs` - `SkyDomeNode`, `SkyDomeUniform`, `MoonOrbit`
- `assets/shaders/sky_dome.wgsl` - Gradient + moon rendering shader

### Integration Points
- `crates/studio_core/src/deferred/plugin.rs` - Node registration, render graph edges
- `crates/studio_core/src/deferred/labels.rs` - `DeferredLabel::SkyDomePass`
- `crates/studio_core/src/deferred/mod.rs` - Module exports

### Day/Night Cycle (Reference)
- `crates/studio_core/src/day_night.rs` - `MoonCycleConfig` orbital math (copied into `sky_dome_node.rs`)

---

## Key Code Patterns

### SkyDomeConfig (Main App Resource)
```rust
#[derive(Resource, Clone, Debug, ExtractResource)]
pub struct SkyDomeConfig {
    pub enabled: bool,
    pub horizon_color: Color,
    pub zenith_color: Color,
    pub horizon_blend_power: f32,
    pub time_of_day: f32,  // 0.0-1.0 controls moon positions
    pub moon1: MoonAppearance,
    pub moon2: MoonAppearance,
    pub moons_enabled: bool,
}
```

### SkyDomeUniform (GPU Struct)
```rust
#[repr(C)]
pub struct SkyDomeUniform {
    pub inv_view_proj: [[f32; 4]; 4],
    pub horizon_color: [f32; 4],
    pub zenith_color: [f32; 4],
    pub params: [f32; 4],  // x=blend_power, y=moons_enabled
    pub moon1_direction: [f32; 4],  // xyz=dir, w=size
    pub moon1_color: [f32; 4],      // rgb=color, a=glow_intensity
    pub moon1_params: [f32; 4],     // x=glow_falloff
    pub moon2_direction: [f32; 4],
    pub moon2_color: [f32; 4],
    pub moon2_params: [f32; 4],
}
```

### Moon Orbital Calculation
```rust
// From sky_dome_node.rs - matches day_night.rs logic
fn calculate_direction(&self, cycle_time: f32) -> Vec3 {
    let moon_time = (cycle_time / self.period + self.phase_offset).fract();
    let angle = moon_time * TAU;
    let incline_rad = self.inclination.to_radians();
    let x = angle.cos();
    let y_base = angle.sin();
    let y = y_base * incline_rad.cos();
    let z = y_base * incline_rad.sin();
    Vec3::new(x, y, z).normalize()
}
```

### Test Harness Time Control
```rust
CapturePosition {
    name: "moon_time_00",
    position: Vec3::new(0.0, 5.0, 0.0),
    look_at: Vec3::new(1.0, 0.5, 0.0),
    time_of_day: Some(0.0),  // Set sky_config.time_of_day before capture
}
```

---

## Verification Commands

```bash
# Build and run test harness
cargo run --example p31_visual_fidelity_test

# Check screenshots
ls screenshots/visual_fidelity_test/

# Build only (faster iteration)
cargo build --example p31_visual_fidelity_test
```

---

## Git History

```
b09f9ad feat(p31): add dual moon rendering with time-of-day control (Phase 3)
9e2d156 feat(p31): add sky gradient from horizon to zenith (Phase 2)
11da607 feat(p31): implement sky dome pipeline facade (Phase 1)
0fa057e feat(p31): add visual fidelity test harness (Phase 0)
```

---

## Common Pitfalls

1. **Non-filterable textures**: G-buffer position is `Rgba32Float` - use `NonFiltering` sampler
2. **Shader async loading**: Pipeline may not be ready on first frame; check before drawing
3. **ExtractResource derive**: Config resources need `#[derive(ExtractResource)]` for render world
4. **Linear color space**: Convert `Color` to linear with `.to_linear()` before passing to shader
5. **Render graph order**: Sky dome runs after bloom, before transparent

---

## Visual Goals (Reminder)

The end goal is screenshots that look compelling:
- Procedural sky with dual moons (purple + orange)
- Dark fantasy / mysterious aesthetic
- Time-of-day control for positioning moons in screenshots
- Buildings that don't look oversized
- Terrain extending to misty horizon
