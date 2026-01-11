// Sky Sphere Material Shader
//
// Renders a sky gradient with cloud texture overlay.
// UV coordinates come from the sphere mesh (proper spherical mapping).

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
}

// Vertex input - standard Bevy mesh attributes
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

// Vertex output
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var cloud_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var cloud_sampler: sampler;

struct SkyUniforms {
    time_of_day: f32,
    cloud_opacity: f32,
    _padding: vec2<f32>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> sky: SkyUniforms;

const PI: f32 = 3.14159265359;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.clip_position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position.xyz;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
    out.uv = vertex.uv;
    return out;
}

// Compute sky gradient based on vertical position (V coordinate)
fn compute_sky_gradient(v: f32, time_of_day: f32) -> vec3<f32> {
    // v=0 is bottom (horizon), v=1 is top (zenith) on a UV sphere
    // But sphere UV often has v=0 at one pole, v=1 at other
    // We'll use v to determine height: 0.5 = horizon, 0 or 1 = poles
    
    // Convert v to height: 0 at horizon (v=0.5), 1 at zenith (v=0 or v=1)
    let height = abs(v - 0.5) * 2.0;
    
    // Time of day affects colors
    let is_night = time_of_day < 0.2 || time_of_day > 0.8;
    let is_sunset = time_of_day > 0.7 && time_of_day < 0.85;
    let is_sunrise = time_of_day > 0.15 && time_of_day < 0.3;
    
    var zenith: vec3<f32>;
    var horizon: vec3<f32>;
    
    if is_night {
        zenith = vec3<f32>(0.02, 0.02, 0.08);
        horizon = vec3<f32>(0.05, 0.05, 0.12);
    } else if is_sunset {
        let t = (time_of_day - 0.7) / 0.15;
        zenith = mix(vec3<f32>(0.2, 0.3, 0.5), vec3<f32>(0.1, 0.05, 0.15), t);
        horizon = mix(vec3<f32>(0.6, 0.5, 0.4), vec3<f32>(0.9, 0.4, 0.2), t);
    } else if is_sunrise {
        let t = (time_of_day - 0.15) / 0.15;
        zenith = mix(vec3<f32>(0.1, 0.05, 0.15), vec3<f32>(0.2, 0.4, 0.7), t);
        horizon = mix(vec3<f32>(0.8, 0.3, 0.2), vec3<f32>(0.5, 0.6, 0.7), t);
    } else {
        // Day
        zenith = vec3<f32>(0.15, 0.4, 0.8);
        horizon = vec3<f32>(0.6, 0.75, 0.9);
    }
    
    // Blend based on height with power curve
    let blend = pow(height, 1.5);
    return mix(horizon, zenith, blend);
}

// Sample clouds with tinting
fn sample_clouds(uv: vec2<f32>, sky_color: vec3<f32>, height: f32) -> vec4<f32> {
    let cloud_sample = textureSample(cloud_texture, cloud_sampler, uv);
    let raw_alpha = cloud_sample.a;
    
    if raw_alpha < 0.01 {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    
    // Cloud color: slightly gray-white
    let base_cloud = vec3<f32>(0.95, 0.95, 0.97);
    
    // Tint toward sky color near horizon
    let tint_strength = 0.15 * (1.0 - height);
    let cloud_color = mix(base_cloud, sky_color, tint_strength);
    
    return vec4<f32>(cloud_color, raw_alpha);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // DEBUG: Output UV as color to verify it's working
    // return vec4<f32>(uv.x, uv.y, 0.0, 1.0);
    
    // Sky gradient based on V coordinate (vertical on sphere)
    let sky_gradient = compute_sky_gradient(uv.y, sky.time_of_day);
    
    // Sample clouds using sphere UV
    let height = abs(uv.y - 0.5) * 2.0;
    let clouds = sample_clouds(uv, sky_gradient, height);
    
    // Blend clouds over gradient
    let cloud_alpha = clouds.a * sky.cloud_opacity;
    let final_color = mix(sky_gradient, clouds.rgb, cloud_alpha);
    
    return vec4<f32>(final_color, 1.0);
}
