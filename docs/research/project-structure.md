# Creature 3D Studio - Project Structure

## Directory Tree (Key Files)

```
creature_3d_studio/
├── Cargo.toml                          # Workspace root
├── src/
│   └── main.rs                         # Main application entry
│
├── crates/
│   └── studio_core/                    # Core rendering library
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                  # Public exports
│           │
│           ├── voxel.rs                # Voxel + VoxelChunk data structures
│           ├── voxel_mesh.rs           # build_chunk_mesh(), VoxelMaterial
│           │
│           ├── deferred/               # CUSTOM DEFERRED PIPELINE
│           │   ├── mod.rs              # Module exports
│           │   ├── plugin.rs           # DeferredRenderingPlugin setup
│           │   │
│           │   ├── gbuffer.rs          # DeferredCamera, ViewGBufferTextures
│           │   ├── gbuffer_node.rs     # GBufferPassNode (renders geometry to MRT)
│           │   ├── gbuffer_geometry.rs # GBufferVertex, pipeline, uniforms
│           │   ├── gbuffer_material.rs # SpecializedMeshPipeline (future use)
│           │   │
│           │   ├── lighting.rs         # DeferredLightingConfig
│           │   ├── lighting_node.rs    # LightingPassNode (fullscreen deferred)
│           │   │
│           │   ├── extract.rs          # DeferredRenderable, mesh extraction
│           │   ├── prepare.rs          # G-buffer texture creation
│           │   └── labels.rs           # Render graph node labels
│           │
│           ├── creature_script.rs      # Lua script loading
│           ├── orbit_camera.rs         # Orbit camera controller
│           └── screenshot.rs           # Screenshot utilities
│
├── assets/
│   ├── shaders/
│   │   ├── gbuffer.wgsl                # G-buffer geometry shader (MRT output)
│   │   ├── deferred_lighting.wgsl      # Lighting pass (fog, sun, emission)
│   │   ├── voxel.wgsl                  # Forward voxel shader (legacy)
│   │   └── gbuffer_test.wgsl           # Debug fullscreen shader
│   │
│   └── scripts/
│       ├── test_creature.lua           # 5-voxel cross pattern
│       ├── test_emission.lua           # 4 voxels with emission gradient
│       └── test_fog.lua                # Voxels at various distances
│
├── examples/
│   ├── p0_screenshot_test.rs           # Phase 0: Screenshot infrastructure
│   ├── p1_black_void_test.rs           # Phase 1: Black background
│   ├── p2_single_cube_test.rs          # Phase 2: Basic cube rendering
│   ├── p3_lua_voxels.rs                # Phase 3: Lua-driven voxels
│   ├── p4_custom_mesh.rs               # Phase 4: Custom vertex format
│   ├── p5_emission.rs                  # Phase 5: Emission brightness
│   ├── p6_bloom.rs                     # Phase 6: Bloom post-process
│   ├── p7_fog.rs                       # Phase 7: Distance fog
│   └── p8_gbuffer.rs                   # Phase 8-9: Full deferred pipeline
│
├── tests/
│   └── screenshot_tests.rs             # Automated screenshot tests
│
├── screenshots/                        # Generated screenshots (gitignored)
│
├── docs/
│   ├── versions/v0.1/
│   │   └── creature-studio-plan.md     # Master development plan
│   │
│   └── research/                       # THIS FOLDER
│       ├── bevy-render-internals.md    # Bevy rendering research
│       ├── bonsai-pipeline-analysis.md # Bonsai reference analysis
│       └── project-structure.md        # This file
│
└── bonsai/                             # Reference implementation (git submodule)
    └── shaders/
        ├── gBuffer.fragmentshader
        ├── Lighting.fragmentshader
        ├── bloom_downsample.fragmentshader
        └── bloom_upsample.fragmentshader
```

## Data Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           MAIN WORLD                                     │
│                                                                         │
│  Lua Script ──► VoxelChunk ──► build_chunk_mesh() ──► Mesh Asset        │
│                                                            │            │
│  Entity: Mesh3d + MeshMaterial3d<VoxelMaterial> + DeferredRenderable    │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ Extract Schedule
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          RENDER WORLD                                    │
│                                                                         │
│  ExtractedDeferredMesh ──► prepare_deferred_meshes() ──► GBufferMeshDrawData
│                                                                         │
│  MeshAllocator: GPU vertex/index buffers                                │
│  RenderAssets<RenderMesh>: Mesh metadata                                │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ Render Schedule
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         RENDER GRAPH                                     │
│                                                                         │
│  ┌──────────────┐      ┌──────────────┐      ┌──────────────┐          │
│  │ GBufferPass  │ ──►  │    (MRT)     │ ──►  │ LightingPass │          │
│  │   Node       │      │ gColor       │      │    Node      │          │
│  └──────────────┘      │ gNormal      │      └──────────────┘          │
│         │              │ gPosition    │             │                   │
│         │              │ gDepth       │             │                   │
│         ▼              └──────────────┘             ▼                   │
│   Geometry ──────────────────────────────────► ViewTarget (screen)      │
└─────────────────────────────────────────────────────────────────────────┘
```

## Key Components

### Marker Components

| Component | Purpose |
|-----------|---------|
| `DeferredCamera` | Camera uses deferred rendering pipeline |
| `DeferredRenderable` | Entity renders in G-buffer pass |

### Resources (Render World)

| Resource | Purpose |
|----------|---------|
| `GBufferGeometryPipeline` | Pipeline, bind group layouts, test cube |
| `LightingPipeline` | Fullscreen lighting pipeline |
| `DeferredLightingConfig` | Sun direction, fog settings |

### Per-View Components (Render World)

| Component | Purpose |
|-----------|---------|
| `ViewGBufferTextures` | G-buffer textures for this camera |
| `ExtractedCamera` | Camera projection/viewport data |

## Shader Uniforms

### View Uniform (Bind Group 0)

```rust
struct ViewUniform {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    inverse_view: mat4x4<f32>,
    projection: mat4x4<f32>,
    inverse_projection: mat4x4<f32>,
    world_position: vec3<f32>,
    viewport: vec4<f32>,
}
```

### Mesh Uniform (Bind Group 1)

```rust
struct MeshUniform {
    world_from_local: mat4x4<f32>,
    local_from_world: mat4x4<f32>,
}
```

### Lighting Uniform (Lighting Pass)

```rust
struct LightingUniform {
    sun_direction: vec3<f32>,
    sun_color: vec3<f32>,
    sun_intensity: f32,
    fog_color: vec3<f32>,
    fog_density: f32,
    fog_start: f32,
    fog_end: f32,
    ambient_color: vec3<f32>,
    camera_position: vec3<f32>,
}
```
