// Bloom Downsample Shader
//
// 13-tap filter for high-quality downsampling.
// Based on Bonsai's bloom_downsample.fragmentshader
//
// This shader takes an input texture and downsamples it to half resolution
// while applying a blur filter to prevent aliasing artifacts.

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;

struct PushConstants {
    texel_size: vec2<f32>,  // 1.0 / input_resolution
    threshold: f32,         // Brightness threshold (first pass only)
    is_first_pass: f32,     // 1.0 for first pass (apply threshold), 0.0 otherwise
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

// Soft threshold - smoother falloff than hard cutoff
fn soft_threshold(color: vec3<f32>, threshold: f32) -> vec3<f32> {
    let brightness = max(max(color.r, color.g), color.b);
    let soft = brightness - threshold + 0.1;
    let contribution = max(soft, 0.0) / max(brightness, 0.0001);
    return color * contribution;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = pc.texel_size;
    
    // 13-tap filter (Jimenez 2014)
    // Sample pattern:
    //   A   B   C
    //     D   E
    //   F   G   H
    //     I   J
    //   K   L   M
    
    let a = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-2.0, -2.0) * texel).rgb;
    let b = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 0.0, -2.0) * texel).rgb;
    let c = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 2.0, -2.0) * texel).rgb;
    
    let d = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-1.0, -1.0) * texel).rgb;
    let e = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 1.0, -1.0) * texel).rgb;
    
    let f = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-2.0,  0.0) * texel).rgb;
    let g = textureSample(input_texture, input_sampler, in.uv).rgb;  // Center
    let h = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 2.0,  0.0) * texel).rgb;
    
    let i = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-1.0,  1.0) * texel).rgb;
    let j = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 1.0,  1.0) * texel).rgb;
    
    let k = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-2.0,  2.0) * texel).rgb;
    let l = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 0.0,  2.0) * texel).rgb;
    let m = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 2.0,  2.0) * texel).rgb;
    
    // Weighted average
    // Center group (higher weight)
    var color = (d + e + i + j) * 0.25 * 0.5;
    // Outer groups
    color += (a + b + f + g) * 0.25 * 0.125;
    color += (b + c + g + h) * 0.25 * 0.125;
    color += (f + g + k + l) * 0.25 * 0.125;
    color += (g + h + l + m) * 0.25 * 0.125;
    
    // Apply threshold on first pass only
    if pc.is_first_pass > 0.5 {
        color = soft_threshold(color, pc.threshold);
    }
    
    return vec4<f32>(color, 1.0);
}
