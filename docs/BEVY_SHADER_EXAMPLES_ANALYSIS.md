# Bevy 0.17 Shader Examples Analysis

## Overview

This document provides a comprehensive analysis of all Bevy 0.17 shader examples, with a focus on understanding how textures are bound and sampled in custom shaders. This is directly relevant to debugging our sky dome cloud texture sampling issue.

---

## Example Index

### Basic Shader Examples (`examples/shader/`)

| # | Example | Relevance | Read | Key Learnings |
|---|---------|-----------|------|---------------|
| 1 | `shader_material.rs` | HIGH | YES | Simple texture binding with `AsBindGroup` derive macro |
| 2 | `shader_material_screenspace_texture.rs` | HIGH | YES | Screen-space texture sampling |
| 3 | `fallback_image.rs` | MEDIUM | YES | How fallback textures work for all dimensions |
| 4 | `array_texture.rs` | MEDIUM | YES | 2D array textures |
| 5 | `animate_shader.rs` | LOW | YES | Time-based animation, no texture |
| 6 | `extended_material.rs` | LOW | YES | Extending StandardMaterial |
| 7 | `shader_defs.rs` | LOW | NO | Shader preprocessor |
| 8 | `shader_material_2d.rs` | LOW | NO | 2D specific |
| 9 | `shader_material_glsl.rs` | LOW | NO | GLSL instead of WGSL |
| 10 | `shader_material_bindless.rs` | MEDIUM | NO | Bindless textures |
| 11 | `shader_material_wesl.rs` | LOW | NO | WESL shader |
| 12 | `shader_prepass.rs` | MEDIUM | NO | Prepass textures |
| 13 | `compute_shader_game_of_life.rs` | LOW | NO | Compute shaders |
| 14 | `gpu_readback.rs` | LOW | NO | GPU readback |
| 15 | `storage_buffer.rs` | LOW | NO | Storage buffers |
| 16 | `extended_material_bindless.rs` | LOW | NO | Bindless + extension |
| 17 | `automatic_instancing.rs` | LOW | NO | Instancing |

### Advanced Shader Examples (`examples/shader_advanced/`)

| # | Example | Relevance | Read | Key Learnings |
|---|---------|-----------|------|---------------|
| 1 | `custom_post_processing.rs` | **CRITICAL** | YES | Most similar to our sky dome - fullscreen post-process with texture |
| 2 | `texture_binding_array.rs` | HIGH | YES | Manual texture binding via `as_bind_group` |
| 3 | `fullscreen_material.rs` | HIGH | YES | New simpler fullscreen material API |
| 4 | `manual_material.rs` | HIGH | YES | Manual material binding without `AsBindGroup` derive |
| 5 | `render_depth_to_texture.rs` | HIGH | YES | Custom render node with texture copy |
| 6 | `custom_render_phase.rs` | MEDIUM | YES | Full custom render phase |
| 7 | `custom_phase_item.rs` | MEDIUM | NO | Phase item basics |
| 8 | `custom_shader_instancing.rs` | LOW | NO | Custom instancing |
| 9 | `custom_vertex_attribute.rs` | LOW | NO | Custom vertex data |
| 10 | `specialized_mesh_pipeline.rs` | MEDIUM | NO | Pipeline specialization |

---

## Critical Findings

### 1. Texture Binding Patterns

#### Pattern A: `AsBindGroup` Derive Macro (High-Level)

**Used in:** `shader_material.rs`, `shader_material_screenspace_texture.rs`

```rust
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct CustomMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
}
```

**Shader side:**
```wgsl
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var texture_sampler: sampler;
```

**Key insight:** The `#{MATERIAL_BIND_GROUP}` placeholder is automatically replaced by Bevy.

---

#### Pattern B: Manual Bind Group Creation (Low-Level)

**Used in:** `custom_post_processing.rs`, `manual_material.rs`

```rust
// In Bevy 0.17, use BindGroupLayoutDescriptor instead of direct layout creation
let layout = BindGroupLayoutDescriptor::new(
    "post_process_bind_group_layout",
    &BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
            uniform_buffer::<PostProcessSettings>(true),
        ),
    ),
);
```

**Bind group creation (in node run):**
```rust
let bind_group = render_context.render_device().create_bind_group(
    "post_process_bind_group",
    &pipeline_cache.get_bind_group_layout(&post_process_pipeline.layout),
    &BindGroupEntries::sequential((
        post_process.source,
        &post_process_pipeline.sampler,
        settings_binding.clone(),
    )),
);
```

**Shader side:**
```wgsl
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
```

---

### 2. Critical Differences Between Bevy 0.15 and 0.17

#### Bind Group Layout Changes

**Bevy 0.15:**
```rust
let layout = render_device.create_bind_group_layout(
    "my_layout",
    &[BindGroupLayoutEntry { ... }],
);
```

**Bevy 0.17:**
```rust
let layout = BindGroupLayoutDescriptor::new(
    "my_layout",
    &BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
        ),
    ),
);

// When creating bind group, get layout from pipeline cache:
let bind_group = render_device.create_bind_group(
    "my_bind_group",
    &pipeline_cache.get_bind_group_layout(&layout),
    &BindGroupEntries::sequential((...)),
);
```

#### Pipeline Layout Changes

**Bevy 0.17:** Pipeline descriptor uses `BindGroupLayoutDescriptor` directly:
```rust
let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
    layout: vec![layout.clone()],  // BindGroupLayoutDescriptor
    ...
});
```

---

### 3. Texture Sampling Requirements

#### TextureSampleType Must Match

| Texture Format | TextureSampleType | Sampler Type |
|----------------|-------------------|--------------|
| Rgba8Unorm | `Float { filterable: true }` | `Filtering` |
| Rgba8UnormSrgb | `Float { filterable: true }` | `Filtering` |
| Rgba32Float | `Float { filterable: false }` | `NonFiltering` |
| Depth32Float | `Depth` | `Comparison` |

**Critical:** If you use `filterable: true` in the layout, the sampler MUST be `SamplerBindingType::Filtering`.

---

### 4. Post-Processing Pattern (Most Relevant to Sky Dome)

From `custom_post_processing.rs`:

```rust
// 1. Create layout descriptor
let layout = BindGroupLayoutDescriptor::new(
    "post_process_bind_group_layout",
    &BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
            uniform_buffer::<PostProcessSettings>(true),
        ),
    ),
);

// 2. Create sampler (can be done once)
let sampler = render_device.create_sampler(&SamplerDescriptor::default());

// 3. Queue pipeline with layout
let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
    layout: vec![layout.clone()],
    vertex: fullscreen_shader.to_vertex_state(),
    fragment: Some(FragmentState {
        shader,
        targets: vec![Some(ColorTargetState {
            format: TextureFormat::bevy_default(),
            ...
        })],
        ..default()
    }),
    ..default()
});

// 4. In ViewNode::run(), create bind group:
let bind_group = render_context.render_device().create_bind_group(
    "post_process_bind_group",
    &pipeline_cache.get_bind_group_layout(&post_process_pipeline.layout),
    &BindGroupEntries::sequential((
        post_process.source,  // TextureView from view_target.post_process_write()
        &post_process_pipeline.sampler,
        settings_binding.clone(),
    )),
);
```

---

### 5. Manual Material Pattern (Relevant for Custom Textures)

From `manual_material.rs`:

```rust
// Create unprepared bind group with explicit bindings
let unprepared = UnpreparedBindGroup {
    bindings: BindingResources(vec![
        (
            0,
            OwnedBindingResource::TextureView(
                TextureViewDimension::D2,
                image.texture_view.clone(),
            ),
        ),
        (
            1,
            OwnedBindingResource::Sampler(
                SamplerBindingType::NonFiltering,
                sampler.clone(),
            ),
        ),
    ]),
};
```

---

### 6. Fullscreen Vertex Shader

Bevy provides a built-in fullscreen vertex shader. In Bevy 0.17:

```rust
use bevy::core_pipeline::FullscreenShader;

// In RenderStartup system:
fn init_pipeline(fullscreen_shader: Res<FullscreenShader>, ...) {
    let vertex_state = fullscreen_shader.to_vertex_state();
    // Use in pipeline descriptor
}
```

**Shader import:**
```wgsl
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // in.uv contains 0-1 screen UVs
}
```

---

## Comparison: Our Sky Dome vs Examples

### Our Current Implementation

```rust
// sky_dome_node.rs (our code)

// 1. Layout creation - OLD STYLE (Bevy 0.15)
let textures_layout = render_device.create_bind_group_layout(
    "sky_dome_textures_layout",
    &[
        BindGroupLayoutEntry { binding: 0, ... },
        BindGroupLayoutEntry { binding: 1, ... },
        // etc
    ],
);

// 2. Bind group creation
let textures_bind_group = render_context.render_device().create_bind_group(
    Some("sky_dome_textures_bind_group"),
    &sky_pipeline.textures_layout,
    &[
        BindGroupEntry { binding: 0, resource: ... },
        BindGroupEntry { binding: 1, resource: ... },
        // etc
    ],
);
```

### Example Implementation (Bevy 0.17 Pattern)

```rust
// custom_post_processing.rs (example)

// 1. Layout descriptor - NEW STYLE (Bevy 0.17)
let layout = BindGroupLayoutDescriptor::new(
    "post_process_bind_group_layout",
    &BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
        ),
    ),
);

// 2. Bind group creation - uses pipeline_cache.get_bind_group_layout()
let bind_group = render_context.render_device().create_bind_group(
    "post_process_bind_group",
    &pipeline_cache.get_bind_group_layout(&layout),
    &BindGroupEntries::sequential((
        source_texture,
        &sampler,
    )),
);
```

---

## Hypothesis: What's Wrong With Our Code

### Potential Issues

1. **CRITICAL: Bind Group Layout API Changed in Bevy 0.17**
   - We're using `render_device.create_bind_group_layout()` - this is the OLD Bevy 0.15 API
   - Bevy 0.17 uses `BindGroupLayoutDescriptor::new()` 
   - When creating bind groups, 0.17 uses `pipeline_cache.get_bind_group_layout(&descriptor)`
   - **Our bind group layout might not be properly linked to the pipeline**

2. **Texture/Sampler Type Mismatch**
   - Our cloud texture is Rgba8UnormSrgb (typical PNG)
   - We declare `TextureSampleType::Float { filterable: true }` in layout
   - We use `SamplerBindingType::Filtering` in layout
   - **This should be correct**, but we should verify

3. **Bind Group Entry Order**
   - We use explicit `BindGroupEntry { binding: N, ... }` with indices
   - Examples use `BindGroupEntries::sequential(...)` which auto-assigns bindings
   - If there's a mismatch between Rust binding indices and WGSL, texture would be black

### CONFIRMED: Root Cause

**We are using the deprecated Bevy 0.15 bind group layout API.**

Evidence from grep search:
- **Zero** uses of `create_bind_group_layout` in Bevy 0.17 examples
- **All** examples use `BindGroupLayoutDescriptor::new()` 
- **All** bind group creations use `pipeline_cache.get_bind_group_layout(&descriptor)`

Our code:
```rust
// OLD API (Bevy 0.15) - WE USE THIS
let textures_layout = render_device.create_bind_group_layout("...", &[...]);
// ...
let bind_group = render_device.create_bind_group(..., &sky_pipeline.textures_layout, ...);
```

Bevy 0.17 examples:
```rust
// NEW API (Bevy 0.17) - EXAMPLES USE THIS
let layout = BindGroupLayoutDescriptor::new("...", &BindGroupLayoutEntries::sequential(...));
// ...
let bind_group = render_device.create_bind_group(
    "...",
    &pipeline_cache.get_bind_group_layout(&layout),  // KEY DIFFERENCE!
    &BindGroupEntries::sequential((...)),
);
```

**The `pipeline_cache.get_bind_group_layout()` call is critical** - it retrieves the actual GPU layout that was created when the pipeline was compiled. Our old approach creates a separate layout that may not be compatible.

---

## Recommended Fix

Update `sky_dome_node.rs` to use Bevy 0.17 patterns:

```rust
// Store BindGroupLayoutDescriptor instead of BindGroupLayout
#[derive(Resource)]
pub struct SkyDomePipeline {
    pub pipeline_id: CachedRenderPipelineId,
    pub textures_layout: BindGroupLayoutDescriptor,  // Changed!
    pub uniforms_layout: BindGroupLayoutDescriptor,  // Changed!
    pub scene_sampler: Sampler,
    pub position_sampler: Sampler,
    pub cloud_sampler: Sampler,
}

// In init_sky_dome_pipeline:
let textures_layout = BindGroupLayoutDescriptor::new(
    "sky_dome_textures_layout",
    &BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d(TextureSampleType::Float { filterable: true }),  // scene
            sampler(SamplerBindingType::Filtering),                     // scene sampler
            texture_2d(TextureSampleType::Float { filterable: false }), // gPosition (Rgba32Float!)
            sampler(SamplerBindingType::NonFiltering),                  // position sampler
            texture_2d(TextureSampleType::Float { filterable: true }),  // cloud
            sampler(SamplerBindingType::Filtering),                     // cloud sampler
        ),
    ),
);

// In ViewNode::run():
let textures_bind_group = render_context.render_device().create_bind_group(
    "sky_dome_textures_bind_group",
    &pipeline_cache.get_bind_group_layout(&sky_pipeline.textures_layout),
    &BindGroupEntries::sequential((
        post_process.source,
        &sky_pipeline.scene_sampler,
        &gbuffer.position.default_view,
        &sky_pipeline.position_sampler,
        cloud_texture_view,
        &sky_pipeline.cloud_sampler,
    )),
);
```

---

## Additional Notes

### Texture View Creation

When getting a texture from `RenderAssets<GpuImage>`:
```rust
let gpu_image = gpu_images.get(handle.id())?;
// gpu_image.texture_view is the TextureView to bind
```

### Debug Strategy

1. First verify scene_texture sampling works (binding 0)
2. Then verify gPosition sampling works (binding 2)
3. Finally debug cloud_texture (binding 4)

If scene_texture works but cloud_texture doesn't, the issue is specific to how we load/bind the cloud texture.

---

## Files to Modify

1. `crates/studio_core/src/deferred/sky_dome_node.rs`
   - Update to use `BindGroupLayoutDescriptor`
   - Update bind group creation to use `pipeline_cache.get_bind_group_layout()`
   - Use `BindGroupEntries::sequential()` instead of explicit `BindGroupEntry`

2. `assets/shaders/sky_dome.wgsl`
   - Keep as-is for now, bindings look correct

---

## References

- Bevy 0.17 examples: `/Users/paul/coding/creatures/creature_3d_studio/bevy/examples/`
- Key example: `shader_advanced/custom_post_processing.rs`
- Shader assets: `/Users/paul/coding/creatures/creature_3d_studio/bevy/assets/shaders/`
