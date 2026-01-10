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
// This maps the sky dome (hemisphere) onto the cloud texture
fn direction_to_spherical_uv(dir: vec3<f32>) -> vec2<f32> {
    // Spherical coordinates:
    // theta (azimuth) = atan2(z, x) -> maps to U [0, 1]
    // phi (elevation) = asin(y) -> maps to V [0, 1]
    
    let theta = atan2(dir.z, dir.x);  // [-PI, PI]
    let phi = asin(clamp(dir.y, -1.0, 1.0));  // [-PI/2, PI/2]
    
    // Map to [0, 1] UV space
    let u = (theta + PI) / (2.0 * PI);  // [0, 1]
    
    // HEMISPHERE mapping: only use upper half (horizon to zenith)
    // phi goes from 0 (horizon) to PI/2 (zenith)
    // Clamp negative elevations (below horizon) to horizon
    let phi_clamped = max(phi, 0.0);  // [0, PI/2]
    let v = phi_clamped / (PI * 0.5);  // [0, 1] - 0 = horizon, 1 = zenith
    
    return vec2<f32>(u, v);
}

// ============================================================================
// CLOUD LIGHTING FUNCTIONS
// Implements physically-inspired cloud shading with dual moon support
// ============================================================================

// Phase 3: Rayleigh scattering approximation (blue sky, colored horizon)
fn rayleigh_scatter(elevation: f32, time_of_day: f32) -> vec3<f32> {
    // Rayleigh scattering causes blue light to scatter more
    // At horizon (low elevation), path length is longer = more scattering = warmer colors
    // At zenith (high elevation), path length is shorter = more blue
    let scatter_strength = 1.0 - elevation;
    
    // Night time has much less scattering (no sun)
    let night_factor = 1.0 - abs(time_of_day - 0.5) * 2.0; // 0 at noon, 1 at midnight
    let scatter_amount = scatter_strength * (1.0 - night_factor * 0.8);
    
    // Wavelength-dependent scattering (blue scatters most)
    return vec3<f32>(0.05, 0.1, 0.2) * scatter_amount;
}

// Phase 3: Mie scattering (forward scatter / halo around light source)
fn mie_scatter(ray_dir: vec3<f32>, light_dir: vec3<f32>, light_color: vec3<f32>, intensity: f32) -> vec3<f32> {
    // Mie scattering creates a bright halo around the light source
    // Strongest when looking toward the light
    let cos_angle = max(dot(ray_dir, light_dir), 0.0);
    
    // Henyey-Greenstein phase function approximation
    // g = 0.76 gives forward-peaked scattering typical of atmospheric aerosols
    let g = 0.76;
    let g2 = g * g;
    let phase = (1.0 - g2) / pow(1.0 + g2 - 2.0 * g * cos_angle, 1.5);
    
    return light_color * phase * intensity * 0.05;
}

// Phase 2: Calculate moon contribution to cloud lighting
fn moon_cloud_lighting(ray_dir: vec3<f32>, moon_dir: vec3<f32>, moon_color: vec3<f32>, moon_intensity: f32) -> vec3<f32> {
    // Moon is above horizon if y > 0
    let moon_visible = step(0.0, moon_dir.y);
    
    // How much this cloud patch faces the moon (diffuse-like term)
    // Using ray_dir as surface normal approximation
    let facing = max(dot(ray_dir, moon_dir), 0.0);
    
    // Softer falloff for more natural look
    let diffuse = pow(facing, 0.5) * 0.4 + 0.1; // Ambient + directional
    
    return moon_color * moon_intensity * diffuse * moon_visible;
}

// Phase 4: Cloud density affects brightness (silver lining effect)
fn cloud_edge_glow(cloud_alpha: f32, moon1_contrib: vec3<f32>, moon2_contrib: vec3<f32>) -> f32 {
    // Thin cloud edges (low alpha) catch more light = brighter
    // Thick cloud centers (high alpha) are darker
    let edge_factor = 1.0 - smoothstep(0.2, 0.7, cloud_alpha);
    
    // More pronounced effect when moon is bright
    let moon_brightness = length(moon1_contrib) + length(moon2_contrib);
    let glow_strength = 0.3 + edge_factor * 0.7 * min(moon_brightness, 1.0);
    
    return glow_strength;
}

// Main cloud color calculation combining all phases
fn calculate_cloud_color(
    ray_dir: vec3<f32>,
    sky_gradient: vec3<f32>,
    cloud_alpha: f32,
    elevation: f32
) -> vec3<f32> {
    let time_of_day = sky.params.w;
    
    // === Phase 1: Base cloud color from sky gradient ===
    // Clouds pick up ambient sky color (darker at night)
    let ambient_cloud = sky_gradient * 0.3;
    
    // === Phase 2: Moon lighting (both moons) ===
    let moon1_dir = normalize(sky.moon1_direction.xyz);
    let moon1_col = sky.moon1_color.rgb;
    let moon1_intensity = sky.moon1_color.a; // glow_intensity doubles as light intensity
    
    let moon2_dir = normalize(sky.moon2_direction.xyz);
    let moon2_col = sky.moon2_color.rgb;
    let moon2_intensity = sky.moon2_color.a;
    
    let moon1_light = moon_cloud_lighting(ray_dir, moon1_dir, moon1_col, moon1_intensity);
    let moon2_light = moon_cloud_lighting(ray_dir, moon2_dir, moon2_col, moon2_intensity);
    
    // === Phase 3: Atmospheric scattering ===
    let rayleigh = rayleigh_scatter(elevation, time_of_day);
    let mie1 = mie_scatter(ray_dir, moon1_dir, moon1_col, moon1_intensity);
    let mie2 = mie_scatter(ray_dir, moon2_dir, moon2_col, moon2_intensity);
    
    // === Phase 4: Edge glow / silver lining ===
    let edge_brightness = cloud_edge_glow(cloud_alpha, moon1_light, moon2_light);
    
    // === Combine all contributions ===
    var cloud_color = ambient_cloud;
    cloud_color += moon1_light + moon2_light;           // Moon illumination
    cloud_color += rayleigh * 0.2;                       // Atmospheric tint
    cloud_color += (mie1 + mie2) * cloud_alpha;         // Forward scatter on dense clouds
    cloud_color *= edge_brightness;                      // Silver lining effect
    
    // Clamp to prevent over-bright
    return clamp(cloud_color, vec3<f32>(0.0), vec3<f32>(1.5));
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
        
        // Calculate physically-based cloud color using all 4 phases
        let cloud_color = calculate_cloud_color(ray_dir, gradient, cloud_alpha, elevation);
        
        // Blend clouds over sky gradient based on cloud density
        let sky_color = mix(gradient, cloud_color, cloud_alpha);
        
        return vec4<f32>(sky_color, 1.0);
    }
    
    return scene_color;
}
