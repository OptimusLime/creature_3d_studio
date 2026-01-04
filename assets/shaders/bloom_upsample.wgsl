// Bloom Upsample Shader
//
// 9-tap tent filter for high-quality upsampling.
// Based on Bonsai's bloom_upsample.fragmentshader
//
// This shader takes a downsampled bloom texture and upsamples it,
// then blends it with the previous mip level.

@group(0) @binding(0) var input_texture: texture_2d<f32>;  // Current mip (smaller)
@group(0) @binding(1) var blend_texture: texture_2d<f32>;  // Previous mip (larger) to blend with
@group(0) @binding(2) var input_sampler: sampler;

struct PushConstants {
    texel_size: vec2<f32>,  // 1.0 / output_resolution
    blend_factor: f32,      // How much to blend with previous mip (0.5 - 0.7 typical)
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = pc.texel_size;
    
    // 9-tap tent filter (3x3 with bilinear sampling)
    // Sample pattern:
    //   A B C
    //   D E F
    //   G H I
    // But we sample at half-texel offsets to leverage bilinear filtering
    
    let a = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-1.0, -1.0) * texel).rgb;
    let b = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 0.0, -1.0) * texel).rgb;
    let c = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 1.0, -1.0) * texel).rgb;
    
    let d = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-1.0,  0.0) * texel).rgb;
    let e = textureSample(input_texture, input_sampler, in.uv).rgb;  // Center
    let f = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 1.0,  0.0) * texel).rgb;
    
    let g = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-1.0,  1.0) * texel).rgb;
    let h = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 0.0,  1.0) * texel).rgb;
    let i = textureSample(input_texture, input_sampler, in.uv + vec2<f32>( 1.0,  1.0) * texel).rgb;
    
    // Tent filter weights: corners=1, edges=2, center=4, total=16
    var upsampled = e * 4.0;
    upsampled += (b + d + f + h) * 2.0;
    upsampled += (a + c + g + i) * 1.0;
    upsampled /= 16.0;
    
    // Blend with the previous (larger) mip level
    let blend = textureSample(blend_texture, input_sampler, in.uv).rgb;
    let result = mix(blend, upsampled, pc.blend_factor);
    
    return vec4<f32>(result, 1.0);
}
