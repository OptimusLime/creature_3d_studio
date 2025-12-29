// Deferred Lighting Shader
// 
// Fullscreen pass that reads G-buffer textures and computes lighting.
// Based on Bonsai's Lighting.fragmentshader
//
// G-Buffer inputs:
// - gColor: RGB = albedo, A = emission intensity
// - gNormal: RGB = world-space normal (encoded as 0-1)
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

// Lighting constants
const AMBIENT_COLOR: vec3<f32> = vec3<f32>(0.1, 0.05, 0.15);
const AMBIENT_INTENSITY: f32 = 0.2;

const SUN_DIRECTION: vec3<f32> = vec3<f32>(0.408, -0.816, 0.408); // normalized (0.5, -1, 0.5)
const SUN_COLOR: vec3<f32> = vec3<f32>(0.8, 0.85, 1.0);
const SUN_INTENSITY: f32 = 0.8;

const FOG_COLOR: vec3<f32> = vec3<f32>(0.102, 0.039, 0.180); // #1a0a2e
const FOG_START: f32 = 10.0;
const FOG_END: f32 = 100.0;

// Debug mode: 0 = final lighting, 1 = show gNormal, 2 = show gPosition depth, 3 = bright red test
const DEBUG_MODE: i32 = 0;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample G-buffer
    let color_sample = textureSample(gColor, gbuffer_sampler, in.uv);
    let normal_sample = textureSample(gNormal, gbuffer_sampler, in.uv);
    let position_sample = textureSample(gPosition, gbuffer_sampler, in.uv);
    
    let albedo = color_sample.rgb;
    let emission = color_sample.a;
    
    // Normal is stored as 0.5 + N*0.5 in gbuffer, decode back
    // For now, gbuffer clears to (0.5, 0.5, 1.0) which is up vector
    let world_normal = normalize(normal_sample.rgb * 2.0 - 1.0);
    
    let world_pos = position_sample.xyz;
    let depth = position_sample.w;
    
    // Debug: Show g-buffer normal directly
    if DEBUG_MODE == 1 {
        return vec4<f32>(normal_sample.rgb, 1.0);
    }
    
    // Debug: Show depth
    if DEBUG_MODE == 2 {
        let d = clamp(depth / 100.0, 0.0, 1.0);
        return vec4<f32>(d, d, d, 1.0);
    }
    
    // Debug: Bright red test - should fill entire screen
    if DEBUG_MODE == 3 {
        return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }
    
    // Skip pixels with no geometry (far depth = 1000)
    // These should show fog color
    if depth > 999.0 {
        return vec4<f32>(FOG_COLOR, 1.0);
    }
    
    // --- Lighting Calculation ---
    
    // Ambient
    var total_light = AMBIENT_COLOR * AMBIENT_INTENSITY;
    
    // Directional light (sun)
    let sun_dir = normalize(-SUN_DIRECTION);
    let n_dot_l = max(dot(world_normal, sun_dir), 0.0);
    total_light += SUN_COLOR * SUN_INTENSITY * n_dot_l;
    
    // Apply lighting to albedo
    var final_color = albedo * total_light;
    
    // Add emission (emission intensity stored in alpha)
    final_color += albedo * emission;
    
    // --- Fog (Bonsai-style) ---
    let fog_factor = smoothstep(FOG_START, FOG_END, depth);
    final_color = mix(final_color, FOG_COLOR, fog_factor);
    
    return vec4<f32>(final_color, 1.0);
}
