// G-Buffer TEST shader - renders a fullscreen quad with solid colors to prove MRT works.
//
// This shader outputs to multiple render targets:
// - location 0: gColor - solid green with some emission
// - location 1: gNormal - normal pointing up
// - location 2: gPosition - center of screen at depth 10

// Fragment output - Multiple Render Targets
struct FragmentOutput {
    @location(0) g_color: vec4<f32>,      // RGB = albedo, A = emission
    @location(1) g_normal: vec4<f32>,     // RGB = world normal, A = unused
    @location(2) g_position: vec4<f32>,   // XYZ = world position, W = linear depth
}

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
    out.position = vec4<f32>(x, y, 0.5, 1.0);  // z=0.5 for mid-depth
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    
    // Create a gradient pattern to verify the output
    let pattern = sin(in.uv.x * 10.0) * sin(in.uv.y * 10.0) * 0.5 + 0.5;
    
    // G-Buffer output 0: Color + Emission
    // Green with varying emission based on pattern
    out.g_color = vec4<f32>(0.2, 0.8, 0.2, pattern * 0.5);  // emission 0-0.5
    
    // G-Buffer output 1: World-space normal pointing up
    out.g_normal = vec4<f32>(0.0, 1.0, 0.0, 0.0);
    
    // G-Buffer output 2: World position (based on UV) + depth
    // Simulate positions at z=0 plane, depth=10 units
    out.g_position = vec4<f32>(
        (in.uv.x - 0.5) * 20.0,  // x: -10 to 10
        0.0,                       // y: 0
        (in.uv.y - 0.5) * 20.0,  // z: -10 to 10
        10.0                       // depth: 10 units
    );
    
    return out;
}
