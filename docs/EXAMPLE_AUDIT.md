# Example Files Audit

## Status Key
- **PASS**: Working correctly, uses library properly
- **NEEDS FIX**: Has issues that need addressing  
- **CONSOLIDATE**: Should be merged with another example

## Current Examples (18 files)

### Infrastructure Tests
| Example | Status | Issues | Action Needed |
|---------|--------|--------|---------------|
| p0_screenshot_test | PASS | None | Keep as-is |
| p1_black_void_test | PASS | None | Keep as-is |
| p2_single_cube_test | PASS | None | Keep as-is |

### Lua Integration
| Example | Status | Issues | Action Needed |
|---------|--------|--------|---------------|
| p3_lua_voxels | PASS | Spawns many entities (inefficient) | Document as "naive approach" |
| p4_custom_mesh | PASS | Good single-mesh approach | Keep as reference |

### Rendering Features
| Example | Status | Issues | Action Needed |
|---------|--------|--------|---------------|
| p5_emission | PASS | None | Keep |
| p6_bloom | PASS | None | Keep |
| p7_fog | PASS | None | Keep |
| p8_gbuffer | PASS | None | Keep |

### Scene Tests
| Example | Status | Issues | Action Needed |
|---------|--------|--------|---------------|
| p9_island | NEEDS FIX | Camera too far, shadows hard to see | Adjust camera, verify shadows visible |
| p10_dark_world | PASS | Good example of deferred + emissive lights | Keep |

### Point Light Tests
| Example | Status | Issues | Action Needed |
|---------|--------|--------|---------------|
| p12_point_light_voxel | CONSOLIDATE | Similar to p13 | Merge into single point light example |
| p13_point_light_shadow | CONSOLIDATE | Similar to p12 | Merge into single point light example |

### Mesh Optimization
| Example | Status | Issues | Action Needed |
|---------|--------|--------|---------------|
| p14_face_culling | CONSOLIDATE | Similar to p15 | Merge into mesh_optimization example |
| p15_greedy_mesh | CONSOLIDATE | Similar to p14 | Merge into mesh_optimization example |

### Multi-Chunk & Streaming
| Example | Status | Issues | Action Needed |
|---------|--------|--------|---------------|
| p16_multi_chunk | PASS | Main multi-chunk example | Keep |
| p17_chunk_streaming | PASS | Chunk streaming demo | Keep |
| p18_cross_chunk_culling | PASS | Cross-chunk optimization | Keep |

---

## Deleted Files (This Session)
- p16_ao_debug.rs - Debug utility, not example
- p16_multi_chunk_greedy.rs - Duplicate diagnostic
- p16_multi_chunk_simple.rs - Duplicate diagnostic
- p16_multi_chunk_stepped.rs - Duplicate diagnostic
- p16_simple_stepped.rs - Duplicate diagnostic
- p16_uniform_stepped.rs - Duplicate diagnostic

---

## Code Duplication to Fix

### High Priority - Screenshot Capture (ALL 18 examples)
Every example has this duplicated:
```rust
#[derive(Resource)]
struct FrameCount(u32);

fn capture_and_exit(...) { ... }
```

**Fix**: Create `ScreenshotTestPlugin` in library

### High Priority - App Setup (ALL 18 examples)
```rust
App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin { ... }))
    .add_plugins(VoxelMaterialPlugin)
    .add_plugins(DeferredRenderingPlugin)
```

**Fix**: Create `VoxelTestApp` builder

### Medium Priority - DirectionalLight (15+ examples)
```rust
commands.spawn((
    DirectionalLight { illuminance: 10000.0, ... },
    Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
));
```

**Fix**: Add `spawn_default_light()` to scene_utils

### Medium Priority - Camera Setup
Many examples manually set up cameras instead of using `CameraPreset`.

**Fix**: Extend `CameraPreset` and use consistently

---

## Required Library Additions

1. **ScreenshotTestPlugin** - Handles frame counting and screenshot capture
2. **VoxelTestApp** - Builder for standard test app setup
3. **Test World Generators** - Functions to create standard test worlds
4. **Default Light Spawner** - scene_utils function for standard lighting
5. **Render Pipeline Config** - System to enable/disable features (bloom, shadows, AO, greedy)

---

## Checklist for Each Example Fix

When fixing an example:
- [ ] Remove duplicate screenshot capture code (use plugin)
- [ ] Remove duplicate app setup (use builder)
- [ ] Use library functions for world/scene setup
- [ ] Use CameraPreset for camera setup
- [ ] Verify all features work (bloom, shadows, AO)
- [ ] Test screenshot output matches expected
- [ ] Update Cargo.toml if needed
