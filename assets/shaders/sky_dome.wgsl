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

// Moon textures (MarkovJunior-generated)
@group(0) @binding(6) var moon1_texture: texture_2d<f32>;
@group(0) @binding(7) var moon1_sampler: sampler;
@group(0) @binding(8) var moon2_texture: texture_2d<f32>;
@group(0) @binding(9) var moon2_sampler: sampler;

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
// Reduced intensity for subtler sky effect
// NOTE: In sky_dome.wgsl, light_dir points TO the moon (positive Y = above)
fn mie_scatter(ray_dir: vec3<f32>, light_dir: vec3<f32>, light_color: vec3<f32>, intensity: f32) -> vec3<f32> {
    // Mie scattering creates a subtle halo around the light source
    // Strongest when looking toward the light
    let cos_angle = max(dot(ray_dir, light_dir), 0.0);
    
    // Henyey-Greenstein phase function approximation
    // g = 0.76 gives forward-peaked scattering typical of atmospheric aerosols
    let g = 0.76;
    let g2 = g * g;
    let phase = (1.0 - g2) / pow(1.0 + g2 - 2.0 * g * cos_angle, 1.5);
    
    // Subtle color tinting around moon
    return light_color * phase * 0.02;
}

// Phase 2: Calculate moon contribution to cloud lighting
// Focus on COLOR TINTING rather than brightness increase
// NOTE: In sky_dome.wgsl, moon_dir points TO the moon (positive Y = moon above)
fn moon_cloud_lighting(ray_dir: vec3<f32>, moon_dir: vec3<f32>, moon_color: vec3<f32>, moon_intensity: f32) -> vec3<f32> {
    // Moon is above horizon if y > 0 (pointing up = moon above)
    let moon_altitude = moon_dir.y;
    let moon_visible = smoothstep(-0.1, 0.2, moon_altitude);
    
    // How much this cloud patch faces the moon (diffuse-like term)
    // Using ray_dir as surface normal approximation
    let facing = max(dot(ray_dir, moon_dir), 0.0);
    
    // Color tinting - visible but not overpowering
    // pow(facing, 1.5) gives moderately focused falloff near moon
    let tint_factor = pow(facing, 1.5) * 0.25 + 0.05;
    
    return moon_color * tint_factor * moon_visible;
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
    // Clouds pick up ambient sky color - visible but moody
    let ambient_cloud = sky_gradient * 0.5;
    
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
    // Base cloud color - visible in darkness
    var cloud_color = ambient_cloud;
    
    // Add moon color tinting (focused on color, not excessive brightness)
    cloud_color += moon1_light + moon2_light;
    
    // Atmospheric scattering effects
    cloud_color += rayleigh * 0.15;
    cloud_color += (mie1 + mie2) * cloud_alpha;
    
    // Edge glow / silver lining effect
    cloud_color *= edge_brightness;
    
    // Clamp to reasonable range - moody but visible
    return clamp(cloud_color, vec3<f32>(0.0), vec3<f32>(1.2));
}

// ============================================================================
// MOON RENDERING
// Renders stylized moons using MarkovJunior-generated textures
// ============================================================================

// Calculate horizon proximity factor (0 = zenith, 1 = horizon)
fn horizon_proximity(moon_dir: vec3<f32>) -> f32 {
    // moon_dir.y is altitude: 1 = zenith, 0 = horizon, -1 = nadir
    // Convert to horizon proximity: 0 at zenith, 1 at horizon
    return 1.0 - clamp(moon_dir.y, 0.0, 1.0);
}

// Sample moon texture and apply coloring with horizon effects
fn sample_moon_texture(
    ray_dir: vec3<f32>,
    moon_dir: vec3<f32>,
    moon_size: f32,
    moon_color: vec3<f32>,
    glow_intensity: f32,
    glow_falloff: f32,
    is_moon1: bool,
) -> vec4<f32> {
    // Moon only visible if above horizon
    if moon_dir.y < -0.1 {
        return vec4<f32>(0.0);
    }
    
    // === HORIZON EFFECTS ===
    let horizon_factor = horizon_proximity(moon_dir);
    
    // Scale moon larger near horizon (moon illusion effect) - up to 30% bigger
    let horizon_scale = 1.0 + horizon_factor * 0.3;
    let scaled_size = moon_size * horizon_scale;
    
    // Increase glow near horizon (atmospheric scattering effect) - up to 2.5x more
    let horizon_glow_boost = 1.0 + horizon_factor * 1.5;
    let boosted_glow = glow_intensity * horizon_glow_boost;
    
    // Softer glow falloff near horizon (atmosphere disperses light)
    let horizon_falloff_factor = 1.0 - horizon_factor * 0.4;
    let adjusted_falloff = glow_falloff * horizon_falloff_factor;
    
    // Warm color shift near horizon (atmospheric reddening)
    let horizon_warmth = horizon_factor * 0.3;
    let warmed_color = moon_color + vec3<f32>(horizon_warmth * 0.2, horizon_warmth * 0.05, -horizon_warmth * 0.1);
    
    // Calculate angle between ray and moon direction
    let cos_angle = dot(ray_dir, moon_dir);
    let angle = acos(clamp(cos_angle, -1.0, 1.0));
    
    let disc_radius = scaled_size;
    
    var moon_alpha = 0.0;
    var moon_col = vec3<f32>(0.0);
    
    // Inside moon disc - sample texture
    if angle < disc_radius {
        // Calculate UV coordinates on the moon disc
        // Project ray onto plane perpendicular to moon direction
        let to_ray = ray_dir - moon_dir * cos_angle;
        let dist_from_center = length(to_ray);
        
        // Create local coordinate system on moon surface
        let up = vec3<f32>(0.0, 1.0, 0.0);
        let right = normalize(cross(up, moon_dir));
        let local_up = normalize(cross(moon_dir, right));
        
        // Project to get UV (-1 to 1 range)
        let local_x = dot(to_ray, right) / disc_radius;
        let local_y = dot(to_ray, local_up) / disc_radius;
        
        // Map to texture UV (0 to 1 range, centered)
        let uv = vec2<f32>(local_x * 0.5 + 0.5, local_y * 0.5 + 0.5);
        
        // Sample the appropriate moon texture
        var tex_sample: vec4<f32>;
        if is_moon1 {
            tex_sample = textureSample(moon1_texture, moon1_sampler, uv);
        } else {
            tex_sample = textureSample(moon2_texture, moon2_sampler, uv);
        }
        
        // Moon texture is BRIGHT and emissive - the texture itself glows
        let base_color = tex_sample.rgb * warmed_color;
        
        // Strong emissive on the texture itself - moon surface is self-luminous
        let luminance = dot(tex_sample.rgb, vec3<f32>(0.299, 0.587, 0.114));
        let emissive_boost = 1.5 + boosted_glow * 2.0 * luminance;
        moon_col = base_color * emissive_boost;
        
        // Brighter center for that glowing orb look
        let center_dist = length(vec2<f32>(local_x, local_y));
        let inner_glow = pow(1.0 - center_dist, 2.0) * boosted_glow * 0.6;
        moon_col += warmed_color * inner_glow;
        
        moon_alpha = tex_sample.a;
        
        // Crisp edge - not too soft
        let edge_softness = disc_radius * 0.06;
        let edge_factor = smoothstep(disc_radius, disc_radius - edge_softness, angle);
        moon_alpha *= edge_factor;
    }
    
    // Tight outer glow - steep falloff, doesn't bleed into clouds
    let glow_radius = disc_radius * 1.6; // Much tighter radius
    if angle < glow_radius && angle > disc_radius * 0.95 {
        let glow_t = (angle - disc_radius * 0.95) / (glow_radius - disc_radius * 0.95);
        // Steep falloff - pow 3.0 for rapid dropoff
        let glow = pow(1.0 - glow_t, 3.0) * boosted_glow * 0.5;
        
        let glow_col = warmed_color * 0.7;
        
        // Only add glow outside the disc, blend gently
        if moon_alpha < 0.3 {
            moon_col = mix(moon_col, glow_col * glow, 1.0 - moon_alpha);
            moon_alpha = max(moon_alpha, glow * 0.3);
        }
    }
    
    return vec4<f32>(moon_col, moon_alpha);
}

// Render both moons using textures
fn render_moons(ray_dir: vec3<f32>) -> vec4<f32> {
    let moons_enabled = sky.params.y;
    if moons_enabled < 0.5 {
        return vec4<f32>(0.0);
    }
    
    // Moon 1 (purple) - using texture
    let moon1_dir = normalize(sky.moon1_direction.xyz);
    let moon1_size = sky.moon1_direction.w;
    let moon1_col = sky.moon1_color.rgb;
    let moon1_glow = sky.moon1_color.a;
    let moon1_falloff = sky.moon1_params.x;
    
    let moon1 = sample_moon_texture(
        ray_dir, moon1_dir, moon1_size, moon1_col,
        moon1_glow, moon1_falloff, true
    );
    
    // Moon 2 (orange) - using texture
    let moon2_dir = normalize(sky.moon2_direction.xyz);
    let moon2_size = sky.moon2_direction.w;
    let moon2_col = sky.moon2_color.rgb;
    let moon2_glow = sky.moon2_color.a;
    let moon2_falloff = sky.moon2_params.x;
    
    let moon2 = sample_moon_texture(
        ray_dir, moon2_dir, moon2_size, moon2_col,
        moon2_glow, moon2_falloff, false
    );
    
    // Composite moons (alpha blend)
    var result = moon1;
    result = vec4<f32>(
        mix(result.rgb, moon2.rgb, moon2.a * (1.0 - result.a * 0.3)),
        max(result.a, moon2.a)
    );
    
    return result;
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
        
        // === Layer 0: Sky gradient (base) ===
        var sky_color = gradient;
        
        // === Layer 1: Moons (behind clouds) ===
        let moons = render_moons(ray_dir);
        sky_color = mix(sky_color, moons.rgb, moons.a);
        
        // === Layer 2: Clouds (in front of moons) ===
        // Sample cloud texture using spherical UV mapping
        let cloud_uv = direction_to_spherical_uv(ray_dir);
        let cloud_sample = textureSample(cloud_texture, cloud_sampler, cloud_uv);
        let cloud_alpha = cloud_sample.a;
        
        // Calculate physically-based cloud color using all 4 phases
        let cloud_color = calculate_cloud_color(ray_dir, gradient, cloud_alpha, elevation);
        
        // Blend clouds over sky+moons
        sky_color = mix(sky_color, cloud_color, cloud_alpha * 0.85); // Slightly translucent clouds
        
        // Add moon glow that bleeds through clouds
        let glow_bleed = moons.a * (1.0 - cloud_alpha * 0.7) * 0.3;
        sky_color += moons.rgb * glow_bleed;
        
        return vec4<f32>(sky_color, 1.0);
    }
    
    return scene_color;
}
