// G-Buffer shader for deferred rendering.
//
// This shader writes geometry data to multiple render targets (MRT):
// - location 0: gColor (RGBA16F) - RGB = albedo, A = emission
// - location 1: gNormal (RGBA16F) - RGB = world-space normal
// - location 2: gPosition (RGBA16F) - XYZ = world position, W = linear depth
//
// NO LIGHTING is computed here - that happens in the lighting pass.
//
// Reference: bonsai/shaders/gBuffer.fragmentshader

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    mesh_view_bindings::view,
}

// Near/far clip planes for linear depth calculation
const NEAR_CLIP: f32 = 0.1;
const FAR_CLIP: f32 = 1000.0;

// Material uniform
struct GBufferMaterialUniform {
    _padding: f32,  // Placeholder, add material properties as needed
}

@group(2) @binding(0)
var<uniform> material: GBufferMaterialUniform;

// Vertex input (same as forward voxel shader)
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) voxel_color: vec3<f32>,
    @location(3) voxel_emission: f32,
}

// Vertex output
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) emission: f32,
}

// Fragment output - Multiple Render Targets
struct FragmentOutput {
    @location(0) g_color: vec4<f32>,      // RGB = albedo, A = emission
    @location(1) g_normal: vec4<f32>,     // RGB = world normal, A = unused
    @location(2) g_position: vec4<f32>,   // XYZ = world position, W = linear depth
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    
    out.clip_position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position.xyz;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
    out.color = vertex.voxel_color;
    out.emission = vertex.voxel_emission;
    
    return out;
}

// Linearize depth from clip-space z to [0,1] range
// Based on Bonsai's Linearize() function
fn linearize_depth(clip_z: f32) -> f32 {
    return (2.0 * NEAR_CLIP) / (FAR_CLIP + NEAR_CLIP - clip_z * (FAR_CLIP - NEAR_CLIP));
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    
    // G-Buffer output 0: Color + Emission
    out.g_color = vec4<f32>(in.color, in.emission);
    
    // G-Buffer output 1: World-space normal (normalized)
    let normal = normalize(in.world_normal);
    out.g_normal = vec4<f32>(normal, 0.0);
    
    // G-Buffer output 2: World position + linear depth
    // Linear depth is stored in W for use in fog/SSAO calculations
    let linear_depth = linearize_depth(in.clip_position.z / in.clip_position.w);
    out.g_position = vec4<f32>(in.world_position, linear_depth);
    
    return out;
}
