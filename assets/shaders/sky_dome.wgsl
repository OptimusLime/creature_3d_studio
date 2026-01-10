// Sky Dome Shader
//
// Fullscreen pass that renders sky where no geometry exists.
// Uses spherical projection for world-space cloud texture sampling.

// Scene texture from previous pass (post-bloom)
@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

// G-buffer position texture for depth check
@group(0) @binding(2) var gPosition: texture_2d<f32>;
@group(0) @binding(3) var position_sampler: sampler;

// Cloud texture (MarkovJunior-generated or placeholder)
@group(0) @binding(4) var cloud_texture: texture_2d<f32>;
@group(0) @binding(5) var cloud_sampler: sampler;

// Sky dome uniforms (bind group 1)
// MUST match SkyDomeUniform in sky_dome_node.rs exactly!
struct SkyDomeUniforms {
    inv_view_proj: mat4x4<f32>,
    horizon_color: vec4<f32>,
    zenith_color: vec4<f32>,
    // x = blend_power, y = moons_enabled, z = sun_intensity (unused), w = time_of_day
    params: vec4<f32>,
    sun_direction: vec4<f32>,  // unused
    sun_color: vec4<f32>,      // unused
    // Moon 1: xyz = direction, w = size (radians)
    moon1_direction: vec4<f32>,
    // Moon 1: rgb = color, a = glow_intensity
    moon1_color: vec4<f32>,
    // Moon 1: x = glow_falloff, y = limb_darkening, z = surface_detail, w = unused
    moon1_params: vec4<f32>,
    // Moon 2: xyz = direction, w = size (radians)
    moon2_direction: vec4<f32>,
    // Moon 2: rgb = color, a = glow_intensity
    moon2_color: vec4<f32>,
    // Moon 2: x = glow_falloff, y = limb_darkening, z = surface_detail, w = unused
    moon2_params: vec4<f32>,
}
@group(1) @binding(0) var<uniform> sky: SkyDomeUniforms;

const SKY_DEPTH_THRESHOLD: f32 = 999.0;
const PI: f32 = 3.14159265359;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Convert screen UV to world-space ray direction
// Since inv_view_proj has numerical issues with infinite far plane,
// we use a simpler approach: just unproject a single point at z=0 (far plane in reverse-Z)
fn get_world_ray_direction(uv: vec2<f32>) -> vec3<f32> {
    // Convert UV to NDC [-1, 1]
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0);
    
    // For infinite reverse-Z, we need to handle the singularity differently
    // Unproject a point at the far plane (z=0 in reverse-Z, but use small epsilon)
    let clip_pos = vec4<f32>(ndc.x, ndc.y, 0.0001, 1.0);
    
    let world_pos = sky.inv_view_proj * clip_pos;
    
    // Perspective divide
    let world_point = world_pos.xyz / world_pos.w;
    
    // Ray direction is from camera to this point
    // For sky, we just normalize the direction (camera at origin conceptually)
    return normalize(world_point);
}

// Convert world-space direction to spherical UV coordinates
// This maps the sky dome onto the cloud texture using equirectangular projection
fn direction_to_spherical_uv(dir: vec3<f32>) -> vec2<f32> {
    // Spherical coordinates:
    // theta (azimuth) = atan2(z, x) -> maps to U [0, 1]
    // phi (elevation) = asin(y) -> maps to V [0, 1]
    
    let theta = atan2(dir.z, dir.x);  // [-PI, PI]
    let phi = asin(clamp(dir.y, -1.0, 1.0));  // [-PI/2, PI/2]
    
    // Map to [0, 1] UV space
    let u = (theta + PI) / (2.0 * PI);  // [0, 1]
    let v = (phi + PI * 0.5) / PI;       // [0, 1] - 0 = bottom (nadir), 1 = top (zenith)
    
    return vec2<f32>(u, v);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let scene_color = textureSample(scene_texture, scene_sampler, in.uv);
    let position_sample = textureSample(gPosition, position_sampler, in.uv);
    let depth = position_sample.w;
    
    if depth > SKY_DEPTH_THRESHOLD {
        // Get world-space ray direction for this pixel
        let ray_dir = get_world_ray_direction(in.uv);
        
        // Compute elevation: ray_dir.y goes from -1 (down) to +1 (up)
        // Map to [0,1]: 0 = horizon, 1 = zenith
        let elevation = clamp((ray_dir.y + 1.0) * 0.5, 0.0, 1.0);
        
        // Use config colors for gradient
        let horizon = sky.horizon_color.rgb;
        let zenith = sky.zenith_color.rgb;
        
        // Apply blend power for sharper/softer horizon transition
        let blend_power = sky.params.x;
        let t = pow(elevation, blend_power);
        let gradient = mix(horizon, zenith, t);
        
        // Sample cloud texture using spherical UV mapping
        let cloud_uv = direction_to_spherical_uv(ray_dir);
        let cloud_sample = textureSample(cloud_texture, cloud_sampler, cloud_uv);
        let cloud_alpha = cloud_sample.a;
        
        // Blend clouds over gradient
        let cloud_color = vec3<f32>(1.0, 1.0, 1.0); // White clouds
        let sky_color = mix(gradient, cloud_color, cloud_alpha);
        
        return vec4<f32>(sky_color, 1.0);
    }
    
    return scene_color;
}
