// Sky Dome Shader
//
// Fullscreen pass that renders procedural sky where no geometry exists.
// Runs after bloom pass, reads the scene color and G-buffer depth to determine
// where to render sky (depth > 999.0 means no geometry).
//
// Phase 3: Gradient sky with dual moon rendering.

// Scene texture from previous pass (post-bloom)
@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

// G-buffer position texture for depth check
// gPosition.w contains linear depth (999+ for sky pixels)
// Uses separate non-filtering sampler since Rgba32Float is not filterable
@group(0) @binding(2) var gPosition: texture_2d<f32>;
@group(0) @binding(3) var position_sampler: sampler;

// Sky dome uniforms (bind group 1)
// MUST match SkyDomeUniform in sky_dome_node.rs exactly!
struct SkyDomeUniforms {
    // Inverse view-projection matrix for reconstructing view direction
    inv_view_proj: mat4x4<f32>,
    // Gradient colors (vec4 for alignment, only rgb used)
    horizon_color: vec4<f32>,
    zenith_color: vec4<f32>,
    // x = blend_power, y = moons_enabled, z = sun_intensity, w = time_of_day
    params: vec4<f32>,
    // Sun: xyz = direction (normalized), w = angular_size
    sun_direction: vec4<f32>,
    // Sun: rgb = color, a = unused
    sun_color: vec4<f32>,
    // Moon 1: xyz = direction (normalized), w = size (radians)
    moon1_direction: vec4<f32>,
    // Moon 1: rgb = color, a = glow_intensity
    moon1_color: vec4<f32>,
    // Moon 1: x = glow_falloff, y = limb_darkening, z = surface_detail, w = unused
    moon1_params: vec4<f32>,
    // Moon 2: xyz = direction (normalized), w = size (radians)
    moon2_direction: vec4<f32>,
    // Moon 2: rgb = color, a = glow_intensity
    moon2_color: vec4<f32>,
    // Moon 2: x = glow_falloff, y = limb_darkening, z = surface_detail, w = unused
    moon2_params: vec4<f32>,
}
@group(1) @binding(0) var<uniform> sky: SkyDomeUniforms;

// Depth threshold for sky detection
// Pixels with depth > this are considered sky (no geometry)
const SKY_DEPTH_THRESHOLD: f32 = 999.0;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Fullscreen triangle vertex shader
// Generates a triangle that covers the entire screen using vertex indices 0, 1, 2
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate positions: (-1,-1), (3,-1), (-1,3) - covers screen with one triangle
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    // UV coordinates: (0,1), (2,1), (0,-1) -> after clamp covers (0,0) to (1,1)
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Reconstruct world-space view direction from UV coordinates
fn get_view_direction(uv: vec2<f32>) -> vec3<f32> {
    // Convert UV to NDC (-1 to 1)
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0);
    
    // Unproject to world space using inverse view-projection
    // Use z=1 (far plane) since we're computing direction for sky
    let world_pos = sky.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    
    // Perspective divide and normalize to get direction
    return normalize(world_pos.xyz / world_pos.w);
}

// Compute sky gradient based on view direction
fn compute_sky_gradient(view_dir: vec3<f32>) -> vec3<f32> {
    // Get vertical component of view direction
    // view_dir.y = 1.0 at zenith (looking straight up)
    // view_dir.y = 0.0 at horizon
    // view_dir.y = -1.0 at nadir (looking straight down)
    
    // Clamp to [0, 1] range - below horizon uses horizon color
    let height = clamp(view_dir.y, 0.0, 1.0);
    
    // Apply blend power for gradient curve control
    // Higher power = more zenith color at top, sharper transition
    let blend_power = sky.params.x;
    let blend_factor = pow(height, blend_power);
    
    // Blend from horizon to zenith
    return mix(sky.horizon_color.rgb, sky.zenith_color.rgb, blend_factor);
}

// Render a moon disc with glow
// Returns: rgb = moon contribution to add to sky
fn render_moon(
    view_dir: vec3<f32>,
    moon_dir: vec3<f32>,
    moon_size: f32,
    moon_color: vec3<f32>,
    glow_intensity: f32,
    glow_falloff: f32,
) -> vec3<f32> {
    // Skip if moon is below horizon
    if moon_dir.y < -0.05 {
        return vec3<f32>(0.0);
    }
    
    // Calculate angle between view direction and moon direction
    let cos_angle = dot(view_dir, moon_dir);
    let angle = acos(clamp(cos_angle, -1.0, 1.0));
    
    var result = vec3<f32>(0.0);
    
    // Apply horizon fade - moons near horizon are dimmer
    let horizon_fade = smoothstep(-0.05, 0.2, moon_dir.y);
    
    // Moon DISC - solid bright circle
    // This should be clearly visible as a distinct shape
    if angle < moon_size {
        // Inside the moon disc
        let t = angle / moon_size;  // 0 at center, 1 at edge
        let edge_softness = 1.0 - smoothstep(0.85, 1.0, t);
        
        // Disc is BRIGHT - the actual moon surface
        result = moon_color * edge_softness * 1.5 * horizon_fade;
    } else {
        // GLOW - soft halo OUTSIDE the disc only
        let glow_size = moon_size * 2.5;
        if angle < glow_size {
            let glow_t = (angle - moon_size) / (glow_size - moon_size);
            let glow_factor = exp(-glow_t * glow_t * 2.0) * glow_intensity * 0.3;
            result = moon_color * glow_factor * horizon_fade;
        }
    }
    
    return result;
}

// Compute full sky color including gradient and moons
fn compute_sky_color(view_dir: vec3<f32>) -> vec3<f32> {
    // DEBUG: Output view_dir.y as brightness to verify view direction reconstruction
    // If looking UP (zenith), view_dir.y should be ~1.0 (BRIGHT)
    // If looking at HORIZON, view_dir.y should be ~0.0 (DARK)
    // If looking DOWN, view_dir.y should be negative (BLACK)
    //
    // This tests Hypothesis 1: Is view_dir correct?
    let debug_y = clamp(view_dir.y, 0.0, 1.0);
    return vec3<f32>(debug_y, debug_y, debug_y);  // Grayscale based on vertical look direction
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the scene color from previous pass
    let scene_color = textureSample(scene_texture, scene_sampler, in.uv);
    
    // Sample depth from G-buffer position texture (w component = linear depth)
    // Uses non-filtering sampler since Rgba32Float is not filterable
    let position_sample = textureSample(gPosition, position_sampler, in.uv);
    let depth = position_sample.w;
    
    // If depth > threshold, this is a sky pixel - render sky with moons
    // Otherwise, pass through the scene color unchanged
    if depth > SKY_DEPTH_THRESHOLD {
        let view_dir = get_view_direction(in.uv);
        let sky_color = compute_sky_color(view_dir);
        return vec4<f32>(sky_color, 1.0);
    }
    
    return scene_color;
}
