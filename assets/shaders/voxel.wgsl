// Voxel shader with per-vertex color, emission, and distance fog.
//
// Emission approach adapted from Bonsai:
// - Bonsai stores emission per-vertex and adds it to light contribution
// - For forward rendering, we use: final = lit_color * (1.0 + emission * EMISSION_MULTIPLIER)
// - This means emission bypasses normal lighting attenuation (Bonsai's key insight)
//
// Fog approach:
// - Blend fog: objects fade INTO the fog color at distance
// - Squared falloff: fog_factor = (dist/max)^2 for nonlinear transition
// - FOG_COLOR matches ClearColor so distant objects disappear into background
// - Inspired by Bonsai but adapted for forward rendering
//
// Vertex attributes:
// - position (location 0): world position
// - normal (location 1): face normal
// - voxel_color (location 2): RGB color [0,1]
// - voxel_emission (location 3): emission intensity [0,1]
//
// Reference: bonsai/shaders/Lighting.fragmentshader:306-319, 394-402

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    mesh_view_bindings::view,
}

// Emission multiplier: emission=1.0 multiplies final color by (1 + EMISSION_MULTIPLIER)
// With value 2.0, full emission triples the brightness
const EMISSION_MULTIPLIER: f32 = 2.0;

// Fog parameters (from Bonsai Lighting.fragmentshader:306-319)
// - FOG_MAX_DISTANCE: distance at which fog reaches full intensity
// - FOG_POWER: multiplier for fog contribution
// - FOG_COLOR: deep purple for dark fantasy aesthetic
const FOG_MAX_DISTANCE: f32 = 50.0;
const FOG_POWER: f32 = 1.0;
const FOG_COLOR: vec3<f32> = vec3<f32>(0.102, 0.039, 0.180);  // #1a0a2e deep purple

// Material uniform - use Bevy's material bind group
struct VoxelMaterialUniform {
    ambient: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: VoxelMaterialUniform;

// Vertex input
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) voxel_color: vec3<f32>,
    @location(3) voxel_emission: f32,
}

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) emission: f32,
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform position and normal to world space
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    
    out.clip_position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position.xyz;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
    out.color = vertex.voxel_color;
    out.emission = vertex.voxel_emission;
    
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Light direction matching Phase 3: from (4, 8, 4) toward origin
    let light_dir = normalize(vec3<f32>(4.0, 8.0, 4.0));
    let normal = normalize(in.world_normal);
    
    // Lambertian diffuse
    let ndotl = max(dot(normal, light_dir), 0.0);
    
    // Combine ambient and diffuse lighting
    let lighting = material.ambient + (1.0 - material.ambient) * ndotl;
    
    // Apply lighting to base color
    let lit_color = in.color * lighting;
    
    // Emission multiplier approach (from plan, inspired by Bonsai):
    // Emissive surfaces bypass normal lighting attenuation
    // final = lit_color * (1.0 + emission * EMISSION_MULTIPLIER)
    // 
    // With EMISSION_MULTIPLIER = 2.0:
    // - emission = 0.0 -> multiplier = 1.0 (normal lighting)
    // - emission = 0.5 -> multiplier = 2.0 (double brightness)
    // - emission = 1.0 -> multiplier = 3.0 (triple brightness)
    let emission_boost = 1.0 + in.emission * EMISSION_MULTIPLIER;
    let emissive_color = lit_color * emission_boost;
    
    // Distance fog - blend toward fog color so objects disappear into background
    // Bonsai's additive fog works in deferred, but for forward rendering we need
    // actual blending so objects fade INTO the fog color (which matches ClearColor)
    //
    // Formula: fog_factor = clamp(dist/max, 0, 1)^2 (squared for nonlinear falloff)
    // Final: mix(object_color, fog_color, fog_factor)
    let camera_pos = view.world_position;
    let distance_to_frag = distance(camera_pos, in.world_position);
    var fog_factor = clamp(distance_to_frag / FOG_MAX_DISTANCE, 0.0, 1.0);
    fog_factor = fog_factor * fog_factor;  // squared falloff
    
    // Blend object color toward fog color - at max distance, object = fog = background
    let final_color = mix(emissive_color, FOG_COLOR, fog_factor);
    
    return vec4<f32>(final_color, 1.0);
}
