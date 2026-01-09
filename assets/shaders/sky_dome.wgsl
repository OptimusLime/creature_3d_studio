// Sky Dome Shader
//
// Fullscreen pass that renders procedural sky where no geometry exists.
// Runs after bloom pass, reads the scene color and G-buffer depth to determine
// where to render sky (depth > 999.0 means no geometry).
//
// Phase 1 (Facade): Outputs constant purple to prove pipeline works.
// Phase 2+: Will add gradient, moons, atmospheric effects.

// Scene texture from previous pass (post-bloom)
@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

// G-buffer position texture for depth check
// gPosition.w contains linear depth (999+ for sky pixels)
// Uses separate non-filtering sampler since Rgba32Float is not filterable
@group(0) @binding(2) var gPosition: texture_2d<f32>;
@group(0) @binding(3) var position_sampler: sampler;

// Sky dome constants
// Phase 1: Constant purple - distinct from fog color (#1a0a2e = 0.102, 0.039, 0.180)
const SKY_COLOR: vec3<f32> = vec3<f32>(0.2, 0.1, 0.3);  // Purple, noticeably different from fog

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the scene color from previous pass
    let scene_color = textureSample(scene_texture, scene_sampler, in.uv);
    
    // Sample depth from G-buffer position texture (w component = linear depth)
    // Uses non-filtering sampler since Rgba32Float is not filterable
    let position_sample = textureSample(gPosition, position_sampler, in.uv);
    let depth = position_sample.w;
    
    // If depth > threshold, this is a sky pixel - render sky color
    // Otherwise, pass through the scene color unchanged
    if depth > SKY_DEPTH_THRESHOLD {
        return vec4<f32>(SKY_COLOR, 1.0);
    }
    
    return scene_color;
}
