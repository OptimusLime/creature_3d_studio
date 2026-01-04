// Point Light Shadow Depth Shader
//
// Renders the scene from a point light's perspective (one cube face at a time).
// The hardware depth buffer stores clip-space Z, but we manually override
// the fragment depth to store linear distance / radius for easier comparison
// in the lighting shader.

// View uniforms (per face)
struct ViewUniforms {
    view_proj: mat4x4<f32>,
    // xyz = light position, w = far plane (radius)
    light_pos_far: vec4<f32>,
}

@group(0) @binding(0) var<uniform> view: ViewUniforms;

// Mesh uniforms
struct MeshUniforms {
    model: mat4x4<f32>,
    model_inverse: mat4x4<f32>,
}

@group(1) @binding(0) var<uniform> mesh: MeshUniforms;

// Vertex input - matches GBufferVertex layout
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) emission: f32,
    @location(4) ao: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) clip_w: f32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform to world space
    let world_pos = (mesh.model * vec4<f32>(in.position, 1.0)).xyz;
    out.world_pos = world_pos;
    
    // Transform to clip space
    let clip = view.view_proj * vec4<f32>(world_pos, 1.0);
    out.clip_position = clip;
    out.clip_w = clip.w;
    
    return out;
}

struct FragmentOutput {
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    
    // Compute linear distance from light to this fragment
    let light_pos = view.light_pos_far.xyz;
    let far = view.light_pos_far.w;
    let distance = length(in.world_pos - light_pos);
    
    // Normalize to [0, 1] range using far plane (light radius)
    // Closer objects get smaller depth values
    out.depth = clamp(distance / far, 0.0, 1.0);
    
    return out;
}
