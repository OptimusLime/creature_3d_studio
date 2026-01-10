// Sky Dome Shader
//
// Fullscreen pass that renders sky where no geometry exists.
// Phase 1: Simple UV-based gradient
// Phase 2+: Will add cloud texture sampling

// Scene texture from previous pass (post-bloom)
@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

// G-buffer position texture for depth check
@group(0) @binding(2) var gPosition: texture_2d<f32>;
@group(0) @binding(3) var position_sampler: sampler;

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

// Simple hash for procedural patterns
fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

// Procedural cloud pattern (placeholder for texture)
// Returns alpha value: 0.0 = fully transparent, 1.0 = fully opaque
fn cloud_pattern(uv: vec2<f32>) -> f32 {
    // Checkerboard pattern - same as placeholder texture
    let checker_size = 0.125;  // 1/8 = 32px at 256px texture
    let checker_x = floor(uv.x / checker_size);
    let checker_y = floor(uv.y / checker_size);
    let is_cloud = (i32(checker_x) + i32(checker_y)) % 2 == 0;
    
    if is_cloud {
        return 0.4;  // 40% opacity - gradient clearly visible through clouds
    } else {
        return 0.0;  // Fully transparent - shows pure gradient
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let scene_color = textureSample(scene_texture, scene_sampler, in.uv);
    let position_sample = textureSample(gPosition, position_sampler, in.uv);
    let depth = position_sample.w;
    
    if depth > SKY_DEPTH_THRESHOLD {
        // Layer 0: Base gradient (dramatic colors for visibility)
        let t = in.uv.y;  // 0 = top, 1 = bottom
        // Dark blue at top (zenith), orange at bottom (horizon) - very obvious gradient
        let zenith = vec3<f32>(0.0, 0.0, 0.2);   // Dark blue
        let horizon = vec3<f32>(0.8, 0.4, 0.1);  // Orange
        let gradient = mix(zenith, horizon, t);
        
        // Layer 1: Cloud overlay (procedural placeholder)
        // Scale UV for tiling (4x4 tiles across screen)
        let cloud_uv = in.uv * 4.0;
        let cloud_alpha = cloud_pattern(cloud_uv);
        let cloud_color = vec3<f32>(1.0, 1.0, 1.0);  // Pure white clouds
        
        // Alpha blend: result = cloud * alpha + gradient * (1 - alpha)
        let sky_color = mix(gradient, cloud_color, cloud_alpha);
        
        return vec4<f32>(sky_color, 1.0);
    }
    
    return scene_color;
}
