# Debug Log: Sky Dome Cloud Texture Not Rendering

## Date: 2026-01-10

## Goal
Sample a cloud texture in world-space so clouds don't move when camera rotates.

## Technique Being Implemented
1. For each sky pixel, compute the world-space ray direction from camera
2. Convert that direction to spherical UV coordinates (longitude/latitude)  
3. Sample cloud texture using those UVs

---

## Problem #1: Sky Color Changes When Moving Camera

**Observation:** Moving the camera forward/backward changed the sky gradient color.

**Root Cause:** The `inv_view_proj` matrix was being passed to the shader incorrectly - row/column order was wrong.

**Fix:** Added `.transpose()` before `.to_cols_array_2d()` in sky_dome_node.rs:
```rust
inv_view_proj: inv_view_proj.transpose().to_cols_array_2d(),
```

**Status:** FIXED

---

## Problem #2: Cloud Texture Not Visible

**Observation:** Sky renders gradient but no clouds appear.

### Tests Performed

| Test | Code | Output | Result |
|------|------|--------|--------|
| 1. Screen UV | `return vec4(in.uv.x, in.uv.y, 0.0, 1.0)` | Red-green gradient across screen | ✅ PASS - input UVs correct |
| 2. Matrix col0 (no transpose) | `return vec4(abs(col0.x), abs(col0.y)*10, abs(col0.z)*10, 1.0)` | Solid red | ❌ FAIL - matrix wrong |
| 3. Matrix col0 (with transpose) | Same as above | Magenta (red+blue) | ✅ PASS - matrix now has data |
| 4. Ray direction | `return vec4(ray_dir * 0.5 + 0.5, 1.0)` | Yellow/cyan gradient | ✅ PASS - ray varies per pixel |
| 5. Sphere UV | `return vec4(sphere_uv.x, sphere_uv.y, 0.0, 1.0)` | Green with yellow band at horizon | ✅ PASS - spherical mapping works |
| 6. Cloud alpha | `return vec4(cloud_sample.a, cloud_sample.a, cloud_sample.a, 1.0)` | Black with tiny white dots at horizon | ❌ FAIL - alpha ~0 everywhere |

### Texture File Verification

```bash
python3 -c "
from PIL import Image
import numpy as np
img = Image.open('assets/textures/generated/mj_clouds_001.png')
arr = np.array(img)
print(f'Shape: {arr.shape}')        # (1024, 2048, 4)
print(f'Channels: {img.mode}')       # RGBA
alpha = arr[:,:,3]
print(f'Alpha min: {alpha.min()}, max: {alpha.max()}')  # 0, 204
print(f'Non-zero alpha pixels: {np.count_nonzero(alpha)}')  # 114,684
"
```

**Result:** File on disk HAS valid alpha data (114,684 non-zero pixels, max 204).

### Current Hypothesis

**The cloud texture is not being bound to the shader.** Either:

1. `CloudTextureHandle` resource is never populated in main world
2. The extraction to render world fails silently
3. The texture handle exists but the GPU texture isn't ready when we bind

### Evidence For This Hypothesis

- Sphere UV mapping is correct (verified visually)
- `textureSample()` returns ~0 for all channels
- No "fallback" warnings printed (but fallback is 8x8 checkerboard with alpha=200, we'd see pattern)
- The file exists and has data

### Code Path For Texture Loading

1. `load_cloud_texture()` in sky_dome_node.rs - runs in PreUpdate
   - Checks `config.cloud_texture_path`
   - Loads via `asset_server.load(path)`
   - Stores in `CloudTextureHandle` resource

2. `extract_cloud_texture()` - runs in ExtractSchedule
   - Copies handle from main world to render world's `ExtractedCloudTexture`

3. In `SkyDomeNode::run()`:
   - Gets `ExtractedCloudTexture` 
   - Looks up GPU image via `gpu_images.get(handle.id())`
   - Falls back to `FallbackCloudTexture` if not found

### Debug Step: Check Texture Loading

Added logging. Results:
```
ExtractedCloudTexture handle exists: AssetId<bevy_image::image::Image>{ index: 7, generation: 0}
Cloud texture found! size: 2048x1024
```

**The texture IS loaded and bound correctly. Size matches the file.**

### New Hypothesis

The texture is bound, but sampling returns 0. Possible causes:

1. **UV coordinates are wrong** - but we verified sphere_uv looks correct
2. **Sampler configuration** - maybe filtering/addressing mode is wrong
3. **Texture format mismatch** - maybe alpha channel isn't being read correctly
4. **The texture data uploaded to GPU lost the alpha** - maybe Bevy's image loader strips alpha

### Next Step

Check the sampler configuration and texture format.

---

## Files Modified During Debug

- `assets/shaders/sky_dome.wgsl` - added world-space ray direction calculation, debug outputs
- `crates/studio_core/src/deferred/sky_dome_node.rs` - added matrix transpose, debug logging

## Related Files

- `assets/textures/generated/mj_clouds_001.png` - the cloud texture (verified has data)
- `examples/p33_mj_cloud_gen.rs` - generates the cloud texture
- `examples/p34_sky_terrain_test.rs` - test scene using sky dome
