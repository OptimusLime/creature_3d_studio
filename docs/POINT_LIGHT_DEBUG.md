# Point Light Shadow Debugging Report

## Current Status: PARTIALLY WORKING - SHADOW POSITION BUG

### Date: 2024-12-29

---

## Problem Statement

Point light shadows are now rendering, but **shadows are detached from the objects casting them**. The shadows appear in roughly the right direction from the light, but they are NOT connected to the base of the pillars.

---

## What Was Fixed (Session 2024-12-29)

### Root Cause 1: Render Graph Order (FIXED)
The render graph had two separate edge chains that didn't connect properly:
```rust
// Chain 1: StartMainPass → ShadowPass → PointShadowPass → GBufferPass
// Chain 2: MainOpaquePass → LightingPass → BloomPass → MainTransparentPass
```

**Problem**: No edge between `GBufferPass` and `MainOpaquePass` meant the lighting pass could run BEFORE the shadow pass completed.

**Fix** (in `plugin.rs`):
```rust
render_app.add_render_graph_edges(
    Core3d,
    (
        Node3d::StartMainPass,
        DeferredLabel::ShadowPass,
        DeferredLabel::PointShadowPass,
        DeferredLabel::GBufferPass,
        Node3d::MainOpaquePass,  // Connect to Bevy's chain
    ),
);
```

### Root Cause 2: Comparison Sampler Direction (FIXED)
The shadow comparison sampler used `GreaterEqual` but should use `LessEqual`.

**Why**: `textureSampleCompare` compares `reference >= sample`, not `sample >= reference`.
- With stored_depth=0.3 (ground) and compare_depth=0.28:
  - `GreaterEqual`: `0.28 >= 0.3` = FALSE → shadow (wrong!)
  - `LessEqual`: `0.28 <= 0.3` = TRUE → lit (correct!)

**Fix** (in `lighting_node.rs`):
```rust
compare: Some(CompareFunction::LessEqual),  // Was GreaterEqual
```

---

## Current Bug: Shadows Detached From Objects

### Symptom
Looking at the final render, the shadows:
1. Are in roughly the right direction (away from light at center)
2. Are NOT connected to the base of the pillars
3. Appear to float on the ground, offset from the objects

### Hypothesis: UV Coordinate Mismatch

The UV calculation in the lighting shader (`calculate_point_shadow`) may not match the view-projection used when rendering the shadow map.

**Evidence**: Debug mode 31 showed moiré patterns in the center area, indicating UV sampling misalignment.

**Specific concern**: The UV formulas use `light_to_frag / abs_vec.dominant_axis` but the shadow render pass uses `Mat4::look_to_rh()` which may have different coordinate conventions.

### Files Involved

| File | What it does |
|------|--------------|
| `point_light_shadow.rs:110-117` | Defines view matrices for each cube face |
| `deferred_lighting.wgsl:290-374` | UV calculation for sampling shadow map |
| `point_shadow_depth.wgsl` | Outputs `distance / radius` as depth |

### View Matrix Definitions (Rust)
```rust
let faces: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::NEG_Y),     // +X face
    (Vec3::NEG_X, Vec3::NEG_Y), // -X face
    (Vec3::Y, Vec3::Z),         // +Y face
    (Vec3::NEG_Y, Vec3::NEG_Z), // -Y face
    (Vec3::Z, Vec3::NEG_Y),     // +Z face
    (Vec3::NEG_Z, Vec3::NEG_Y), // -Z face
];
```

### UV Calculation (WGSL) - Example for -Y face
```wgsl
// -Y face: look_at(-Y, up=-Z)
face_uv = vec2<f32>(
    light_to_frag.x / abs_vec.y * 0.5 + 0.5,
    -light_to_frag.z / abs_vec.y * 0.5 + 0.5
);
```

---

## Next Steps to Fix Shadow Position

1. **Verify UV math manually**: Pick a known world point, compute what UV it should map to in the shadow pass, then verify the lighting shader computes the same UV.

2. **Add debug visualization**: Show the UV coordinates being used for shadow sampling overlaid on the scene.

3. **Compare with reference**: Look at how standard cube shadow map implementations handle the UV-to-face mapping.

4. **Check perspective division**: The shadow pass uses perspective projection but the UV calculation may assume orthographic.

---

## Debug Modes Reference

| Mode | Output |
|------|--------|
| 0 | Final render |
| 20 | Point shadow (green=lit, red=shadowed) |
| 23 | Which cube face is selected |
| 28 | Raw textureLoad values (R=+X, G=-Y, B=-Z) |
| 29 | All 6 faces as strips via textureLoad |
| 30 | Quadrant test: textureLoad vs textureSampleCompare |
| 31 | Debug: stored_depth vs compare_depth with face selection |

---

## Key Insight from Debugging

The textures ARE being written correctly (verified via `textureLoad`). The comparison sampler IS working correctly (verified via quadrant test). The issue is specifically in **where** we sample - the UV coordinates don't match between render and sample.
