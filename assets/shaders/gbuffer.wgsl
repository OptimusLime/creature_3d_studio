// G-Buffer shader for deferred rendering.
//
// This shader writes geometry data to multiple render targets (MRT):
// - location 0: gColor (RGBA16F) - RGB = albedo, A = emission
// - location 1: gNormal (RGBA16F) - RGB = world-space normal, A = ambient occlusion
// - location 2: gPosition (RGBA32F) - XYZ = world position, W = linear depth
//
// NO LIGHTING is computed here - that happens in the lighting pass.
//
// FULLY CUSTOM - no bevy_pbr imports, we handle everything ourselves.

// Near/far clip planes for linear depth calculation
const NEAR_CLIP: f32 = 0.1;
const FAR_CLIP: f32 = 1000.0;

// View uniforms (bind group 0)
struct ViewUniform {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    inverse_view: mat4x4<f32>,
    projection: mat4x4<f32>,
    inverse_projection: mat4x4<f32>,
    world_position: vec3<f32>,
    viewport: vec4<f32>,  // x, y, width, height
}

@group(0) @binding(0)
var<uniform> view: ViewUniform;

// Mesh uniforms (bind group 1)
struct MeshUniform {
    world_from_local: mat4x4<f32>,
    local_from_world: mat4x4<f32>,
}

@group(1) @binding(0)
var<uniform> mesh: MeshUniform;

// Vertex input - matches our voxel mesh vertex format
struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) voxel_color: vec3<f32>,
    @location(3) voxel_emission: f32,
    @location(4) voxel_ao: f32,
}

// Vertex output
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) emission: f32,
    @location(4) ao: f32,
}

// Fragment output - Multiple Render Targets
struct FragmentOutput {
    @location(0) g_color: vec4<f32>,      // RGB = albedo, A = emission
    @location(1) g_normal: vec4<f32>,     // RGB = world normal, A = ambient occlusion
    @location(2) g_position: vec4<f32>,   // XYZ = world position, W = linear depth
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform to world space
    let world_position = mesh.world_from_local * vec4<f32>(vertex.position, 1.0);
    
    // Transform to clip space
    out.clip_position = view.view_proj * world_position;
    out.world_position = world_position.xyz;
    
    // Transform normal to world space (using inverse transpose for correct normals)
    // For uniform scaling, we can use the upper-left 3x3 of world_from_local
    let normal_matrix = mat3x3<f32>(
        mesh.world_from_local[0].xyz,
        mesh.world_from_local[1].xyz,
        mesh.world_from_local[2].xyz
    );
    out.world_normal = normalize(normal_matrix * vertex.normal);
    
    // Pass through color, emission, and AO
    out.color = vertex.voxel_color;
    out.emission = vertex.voxel_emission;
    out.ao = vertex.voxel_ao;
    
    return out;
}

// Linearize depth from clip-space z to linear distance
// Reverse-Z: near = 1, far = 0, so we need to handle that
fn linearize_depth(ndc_z: f32) -> f32 {
    // For reverse-Z with infinite far plane:
    // linear_depth = near / ndc_z
    // But we use finite far, so:
    let z = ndc_z;
    return (2.0 * NEAR_CLIP * FAR_CLIP) / (FAR_CLIP + NEAR_CLIP - z * (FAR_CLIP - NEAR_CLIP));
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    
    // G-Buffer output 0: Color + Emission
    // Emission is stored as 0-1 normalized value
    out.g_color = vec4<f32>(in.color, in.emission);
    
    // G-Buffer output 1: World-space normal + Ambient Occlusion
    // AO is stored in the alpha channel (0.0 = fully occluded, 1.0 = fully lit)
    let normal = normalize(in.world_normal);
    out.g_normal = vec4<f32>(normal, in.ao);
    
    // G-Buffer output 2: World position + linear depth
    // Linear depth is calculated from NDC z for use in fog/lighting
    let ndc_z = in.clip_position.z / in.clip_position.w;
    let linear_depth = linearize_depth(ndc_z);
    out.g_position = vec4<f32>(in.world_position, linear_depth);
    
    return out;
}
