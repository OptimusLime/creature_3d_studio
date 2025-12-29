// Bloom Composite Shader
//
// Final compositing pass that combines the original HDR image with bloom.
// Also applies tone mapping to bring HDR values back to LDR range.

@group(0) @binding(0) var scene_texture: texture_2d<f32>;   // Original HDR scene
@group(0) @binding(1) var bloom_texture: texture_2d<f32>;   // Blurred bloom
@group(0) @binding(2) var tex_sampler: sampler;

struct PushConstants {
    bloom_intensity: f32,   // How much bloom to add (0.5 - 1.5 typical)
    bloom_threshold: f32,   // Minimum brightness for bloom (unused here, for reference)
    exposure: f32,          // Exposure adjustment
    _padding: f32,
}
var<push_constant> pc: PushConstants;

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

// ACES filmic tone mapping
// Good for games - maintains saturation, nice highlight rolloff
fn aces_tonemap(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((color * (a * color + b)) / (color * (c * color + d) + e));
}

// Simple Reinhard tone mapping (alternative)
fn reinhard_tonemap(color: vec3<f32>) -> vec3<f32> {
    return color / (color + vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample original scene
    let scene = textureSample(scene_texture, tex_sampler, in.uv).rgb;
    
    // Sample bloom (already blurred)
    let bloom = textureSample(bloom_texture, tex_sampler, in.uv).rgb;
    
    // Combine scene + bloom
    var hdr_color = scene + bloom * pc.bloom_intensity;
    
    // Apply exposure
    hdr_color *= pc.exposure;
    
    // Tone map to LDR
    let ldr_color = aces_tonemap(hdr_color);
    
    // Gamma correction (if not handled by sRGB framebuffer)
    // let gamma_color = pow(ldr_color, vec3<f32>(1.0 / 2.2));
    
    return vec4<f32>(ldr_color, 1.0);
}
