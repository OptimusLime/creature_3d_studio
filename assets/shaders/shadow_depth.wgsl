// Shadow Depth Shader
//
// Simple depth-only shader for shadow map rendering.
// Renders the scene from the light's perspective, writing only depth.
//
// The shadow map is then sampled in the lighting pass to determine
// whether fragments are in shadow.

// View uniforms (bind group 0) - light-space matrices
struct ShadowViewUniform {
    light_view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> shadow_view: ShadowViewUniform;

// Mesh uniforms (bind group 1) - per-mesh transform
struct MeshUniform {
    world_from_local: mat4x4<f32>,
    local_from_world: mat4x4<f32>,
}

@group(1) @binding(0)
var<uniform> mesh: MeshUniform;

// Vertex input - same as G-buffer shader but we only use position
struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,      // unused but part of vertex format
    @location(2) voxel_color: vec3<f32>, // unused
    @location(3) voxel_emission: f32,    // unused
}

@vertex
fn vs_main(vertex: Vertex) -> @builtin(position) vec4<f32> {
    // Transform to world space
    let world_position = mesh.world_from_local * vec4<f32>(vertex.position, 1.0);
    
    // Transform to light clip space
    return shadow_view.light_view_proj * world_position;
}

// No fragment shader needed - depth is written automatically
// But WGSL requires an entry point, so we provide an empty one
// Actually for depth-only passes with no color attachments,
// we don't need a fragment shader at all in wgpu.
