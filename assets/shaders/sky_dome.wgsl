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
struct SkyDomeUniforms {
    // Inverse view-projection matrix for reconstructing view direction
    inv_view_proj: mat4x4<f32>,
    // Gradient colors (vec4 for alignment, only rgb used)
    horizon_color: vec4<f32>,
    zenith_color: vec4<f32>,
    // x = blend_power, y = moons_enabled (0 or 1), zw = unused
    params: vec4<f32>,
    // Moon 1: xyz = direction (normalized), w = size (radians)
    moon1_direction: vec4<f32>,
    // Moon 1: rgb = color, a = glow_intensity
    moon1_color: vec4<f32>,
    // Moon 1: x = glow_falloff, yzw = unused
    moon1_params: vec4<f32>,
    // Moon 2: xyz = direction (normalized), w = size (radians)
    moon2_direction: vec4<f32>,
    // Moon 2: rgb = color, a = glow_intensity
    moon2_color: vec4<f32>,
    // Moon 2: x = glow_falloff, yzw = unused
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
    
    // Moon disc - sharp edge
    let disc_factor = 1.0 - smoothstep(moon_size * 0.8, moon_size, angle);
    
    // Glow - soft falloff around moon
    let glow_size = moon_size * 3.0; // Glow extends 3x moon size
    let glow_factor = exp(-pow(angle / glow_size, glow_falloff)) * glow_intensity;
    
    // Combine disc and glow
    let total_factor = disc_factor + glow_factor * (1.0 - disc_factor);
    
    // Apply horizon fade - moons near horizon are dimmer
    let horizon_fade = smoothstep(-0.05, 0.2, moon_dir.y);
    
    return moon_color * total_factor * horizon_fade;
}

// Compute full sky color including gradient and moons
fn compute_sky_color(view_dir: vec3<f32>) -> vec3<f32> {
    // Start with gradient
    var sky_color = compute_sky_gradient(view_dir);
    
    // Add moons if enabled
    let moons_enabled = sky.params.y > 0.5;
    if moons_enabled {
        // Moon 1
        let moon1_contrib = render_moon(
            view_dir,
            sky.moon1_direction.xyz,
            sky.moon1_direction.w,
            sky.moon1_color.rgb,
            sky.moon1_color.a,
            sky.moon1_params.x,
        );
        sky_color += moon1_contrib;
        
        // Moon 2
        let moon2_contrib = render_moon(
            view_dir,
            sky.moon2_direction.xyz,
            sky.moon2_direction.w,
            sky.moon2_color.rgb,
            sky.moon2_color.a,
            sky.moon2_params.x,
        );
        sky_color += moon2_contrib;
    }
    
    return sky_color;
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
