# Rendering Debug: p23 vs p9 Island

## Current Issue

**p9_island** renders correctly with deferred pipeline (shadows, GTAO, proper lighting).
**p23_kinematic_controller** renders like garbage - flat, no proper lighting.

Both examples use `VoxelWorldApp` with identical deferred pipeline setup.

## What We've Verified

### Both pipelines ARE running
Debug output confirms both examples run the deferred nodes:
```
>>> GBufferPassNode::run - camera viewport: Some(UVec2(1280, 720))
>>> LightingPassNode::run - rendering to main texture
```

### Both have identical setup
- `VoxelWorldApp` with `.with_deferred(true)`
- `DeferredRenderingPlugin` added
- `DeferredCamera` component on camera
- `DeferredRenderable` component on meshes (via `spawn_world_with_lights_config`)
- `DeferredPointLight` for shadow light and emissive voxels
- `VoxelMaterial` for mesh materials

### Screenshot comparison
- p9_island.png: 112KB (rich detail, proper lighting)
- p23_kinematic_controller.png: 24KB (flat, washed out)

## Hypothesis

The deferred pipeline runs for both BUT something else is also rendering and overwriting/interfering.

### Possible causes:

1. **Render order issue** - Something renders AFTER LightingPassNode and overwrites
   - Bevy's forward pass (`MainOpaquePass`) runs between GBuffer and Lighting
   - If forward pass clears depth/color incorrectly, could cause issues

2. **Multiple cameras** - p23 might have 2 cameras (one from VoxelWorldApp, one from setup)
   - Unlikely since we use `CameraConfig::Custom` now, not spawn our own

3. **Different clear colors/operations** - LoadOp::Clear in lighting pass might differ

4. **Mesh extraction issue** - Meshes might not be extracted to render world properly for p23
   - `ExtractedDeferredMesh` query requires `DeferredRenderable` + `VoxelMaterial`

5. **Bloom/post-processing difference** - p9 might have different bloom settings

## Next Steps to Investigate

1. **Compare exactly what entities exist** in both examples at runtime
   - Number of cameras, their components
   - Number of meshes with `DeferredRenderable`
   - Number of `DeferredPointLight` entities

2. **Check if forward pass is interfering**
   - Add debug output to see if `MainOpaquePass` renders anything
   - Check if meshes are being rendered twice (forward + deferred)

3. **Disable forward rendering entirely** for meshes with `DeferredRenderable`
   - The meshes use `VoxelMaterial` which has a forward shader
   - Bevy might be rendering them in forward pass, then deferred overwrites partially

4. **Compare ViewGBufferTextures** content between the two
   - Screenshot the GBuffer intermediate textures
   - See if geometry is being written correctly

## Key Files

| File | Purpose |
|------|---------|
| `deferred/plugin.rs` | Render graph setup, node ordering |
| `deferred/gbuffer_node.rs` | Writes geometry to GBuffer |
| `deferred/lighting_node.rs` | Reads GBuffer, writes final image |
| `deferred/extract.rs` | Extracts meshes to render world |
| `voxel_mesh.rs` | VoxelMaterial with forward shader |
| `scene_utils.rs` | spawn_world_with_lights_config |

## Render Graph Order (from plugin.rs)

```
StartMainPass
  → Moon1ShadowPass
  → Moon2ShadowPass  
  → PointShadowPass
  → GBufferPass
  → MainOpaquePass  ← BEVY'S FORWARD RENDERING
  
GBufferPass
  → GtaoDepthPrefilter
  → GtaoPass
  → GtaoDenoise
  → MainOpaquePass

MainOpaquePass
  → LightingPass    ← CLEARS and renders deferred result
  → BloomPass
  → MainTransparentPass
```

The `LightingPass` does `LoadOp::Clear` so it SHOULD overwrite whatever forward rendered.
Unless the issue is in the GBuffer content itself.
