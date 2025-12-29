// Deferred Lighting Shader
// 
// Fullscreen pass that reads G-buffer textures and computes lighting.
// Based on Bonsai's Lighting.fragmentshader
//
// G-Buffer inputs:
// - gColor: RGB = albedo, A = emission intensity (0-1)
// - gNormal: RGB = world-space normal (raw -1 to +1, NOT encoded)
// - gPosition: XYZ = world position, W = linear depth

// G-buffer textures
@group(0) @binding(0) var gColor: texture_2d<f32>;
@group(0) @binding(1) var gNormal: texture_2d<f32>;
@group(0) @binding(2) var gPosition: texture_2d<f32>;
@group(0) @binding(3) var gbuffer_sampler: sampler;

// Fullscreen triangle vertices
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Generate fullscreen triangle
    // Vertex 0: (-1, -1), Vertex 1: (3, -1), Vertex 2: (-1, 3)
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Lighting constants - improved for better contrast
const AMBIENT_COLOR: vec3<f32> = vec3<f32>(0.15, 0.1, 0.2);  // Slightly purple ambient
const AMBIENT_INTENSITY: f32 = 0.15;  // Lower ambient for more contrast

// Sun coming from upper-right-front for good face differentiation
const SUN_DIRECTION: vec3<f32> = vec3<f32>(-0.577, -0.577, -0.577); // normalized (-1, -1, -1)
const SUN_COLOR: vec3<f32> = vec3<f32>(1.0, 0.95, 0.9);  // Warm white
const SUN_INTENSITY: f32 = 1.2;  // Brighter sun for more contrast

// Fill light from opposite side (dimmer)
const FILL_DIRECTION: vec3<f32> = vec3<f32>(0.707, 0.0, 0.707); // from left-back
const FILL_COLOR: vec3<f32> = vec3<f32>(0.4, 0.5, 0.7);  // Cool blue
const FILL_INTENSITY: f32 = 0.3;

const FOG_COLOR: vec3<f32> = vec3<f32>(0.102, 0.039, 0.180); // #1a0a2e - deep purple
const FOG_START: f32 = 15.0;
const FOG_END: f32 = 80.0;

// Debug mode: 0 = final lighting, 1 = show gNormal, 2 = show gPosition depth, 3 = albedo only
const DEBUG_MODE: i32 = 0;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample G-buffer
    let color_sample = textureSample(gColor, gbuffer_sampler, in.uv);
    let normal_sample = textureSample(gNormal, gbuffer_sampler, in.uv);
    let position_sample = textureSample(gPosition, gbuffer_sampler, in.uv);
    
    let albedo = color_sample.rgb;
    let emission = color_sample.a;  // 0-1 normalized emission
    
    // Normal is stored directly as world-space normal (-1 to +1)
    // NO encoding/decoding needed - just normalize to handle interpolation
    let world_normal = normalize(normal_sample.rgb);
    
    let world_pos = position_sample.xyz;
    let depth = position_sample.w;
    
    // Debug: Show g-buffer normal as color (remap -1,1 to 0,1 for visualization)
    if DEBUG_MODE == 1 {
        return vec4<f32>(world_normal * 0.5 + 0.5, 1.0);
    }
    
    // Debug: Show depth
    if DEBUG_MODE == 2 {
        let d = clamp(depth / 50.0, 0.0, 1.0);
        return vec4<f32>(d, d, d, 1.0);
    }
    
    // Debug: Show albedo only (no lighting)
    if DEBUG_MODE == 3 {
        return vec4<f32>(albedo, 1.0);
    }
    
    // Skip pixels with no geometry (far depth = 1000)
    // These should show fog color
    if depth > 999.0 {
        return vec4<f32>(FOG_COLOR, 1.0);
    }
    
    // --- Lighting Calculation ---
    
    // Ambient - base illumination
    var total_light = AMBIENT_COLOR * AMBIENT_INTENSITY;
    
    // Main directional light (sun) - from upper-right-front
    let sun_dir = normalize(-SUN_DIRECTION);
    let n_dot_sun = max(dot(world_normal, sun_dir), 0.0);
    // Add slight wraparound for softer shadows
    let sun_wrap = n_dot_sun * 0.8 + 0.2 * max(dot(world_normal, sun_dir) + 0.5, 0.0);
    total_light += SUN_COLOR * SUN_INTENSITY * sun_wrap;
    
    // Fill light from opposite side - prevents pure black shadows
    let fill_dir = normalize(-FILL_DIRECTION);
    let n_dot_fill = max(dot(world_normal, fill_dir), 0.0);
    total_light += FILL_COLOR * FILL_INTENSITY * n_dot_fill;
    
    // Apply lighting to albedo
    var final_color = albedo * total_light;
    
    // Add emission - emission makes the surface glow beyond its lit color
    // Higher emission = more of the albedo color added as self-illumination
    // Scale emission contribution (emission is 0-1, we want visible glow)
    let emission_strength = emission * 2.0;  // Boost emission visibility
    final_color += albedo * emission_strength;
    
    // --- Fog (Bonsai-style) ---
    // Exponential fog for more natural falloff
    let fog_factor = smoothstep(FOG_START, FOG_END, depth);
    final_color = mix(final_color, FOG_COLOR, fog_factor);
    
    // HDR output - values can exceed 1.0 for bloom
    return vec4<f32>(final_color, 1.0);
}
