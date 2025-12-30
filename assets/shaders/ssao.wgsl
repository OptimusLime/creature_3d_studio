// SSAO (Screen-Space Ambient Occlusion) shader
// 
// Uses normal-based edge detection to darken corners and edges where
// surface normals change. This approach works well with our world-space
// G-buffer and produces clean results for voxel geometry.

// ============================================================================
// Bind Groups
// ============================================================================

@group(0) @binding(0) var g_normal: texture_2d<f32>;
@group(0) @binding(1) var g_position: texture_2d<f32>;
@group(0) @binding(2) var gbuffer_sampler: sampler;

@group(1) @binding(0) var<uniform> ssao_kernel: array<vec4<f32>, 32>;
@group(1) @binding(1) var noise_texture: texture_2d<f32>;
@group(1) @binding(2) var noise_sampler: sampler;

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    screen_size: vec4<f32>,
}
@group(2) @binding(0) var<uniform> camera: CameraUniforms;

// ============================================================================
// Constants
// ============================================================================

const AO_RADIUS: f32 = 3.0;        // Sample radius in pixels
const AO_INTENSITY: f32 = 0.8;     // Overall darkening intensity
const DEPTH_THRESHOLD: f32 = 2.0;  // Max world-space depth difference to consider

// ============================================================================
// Vertex Shader
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    
    var out: VertexOutput;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// ============================================================================
// Fragment Shader
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) f32 {
    let uv = in.uv;
    let texel_size = camera.screen_size.zw;
    
    // Sample center pixel
    let center_normal = textureSample(g_normal, gbuffer_sampler, uv).xyz;
    let center_pos = textureSample(g_position, gbuffer_sampler, uv);
    let center_depth = center_pos.w;
    
    // Early out for sky/background
    if center_depth > 999.0 || length(center_normal) < 0.5 {
        return 1.0;
    }
    
    let normal = normalize(center_normal);
    
    // Multi-scale sampling for smoother AO
    var ao: f32 = 0.0;
    var total_weight: f32 = 0.0;
    
    // Sample at multiple radii for better coverage
    let radii = array<f32, 3>(1.0, 2.0, 4.0);
    let weights = array<f32, 3>(0.5, 0.3, 0.2);
    
    for (var r: i32 = 0; r < 3; r++) {
        let radius = radii[r] * AO_RADIUS;
        let weight = weights[r];
        
        // 8-direction sampling pattern
        let offsets = array<vec2<f32>, 8>(
            vec2<f32>(1.0, 0.0),
            vec2<f32>(-1.0, 0.0),
            vec2<f32>(0.0, 1.0),
            vec2<f32>(0.0, -1.0),
            vec2<f32>(0.707, 0.707),
            vec2<f32>(-0.707, 0.707),
            vec2<f32>(0.707, -0.707),
            vec2<f32>(-0.707, -0.707)
        );
        
        var ring_ao: f32 = 0.0;
        
        for (var i: i32 = 0; i < 8; i++) {
            let offset = offsets[i] * radius * texel_size;
            let sample_uv = uv + offset;
            
            // Bounds check
            if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
                continue;
            }
            
            let sample_normal = textureSample(g_normal, gbuffer_sampler, sample_uv).xyz;
            let sample_pos = textureSample(g_position, gbuffer_sampler, sample_uv);
            let sample_depth = sample_pos.w;
            
            // Skip sky samples
            if sample_depth > 999.0 || length(sample_normal) < 0.5 {
                continue;
            }
            
            // Normal difference - corners have different normals meeting
            let normal_diff = 1.0 - max(dot(normal, normalize(sample_normal)), 0.0);
            
            // Depth difference - nearby geometry at different depths can occlude
            let depth_diff = abs(center_depth - sample_depth);
            let depth_factor = saturate(1.0 - depth_diff / DEPTH_THRESHOLD);
            
            // Position-based check: is the sample position "below" our surface?
            // This helps detect concave corners
            let to_sample = sample_pos.xyz - center_pos.xyz;
            let behind_surface = -dot(normalize(to_sample), normal);
            let concavity = saturate(behind_surface);
            
            // Combine factors
            // Normal changes at nearby depths indicate corners/edges
            let sample_ao = normal_diff * depth_factor * 0.5 + concavity * depth_factor * 0.5;
            ring_ao += sample_ao;
        }
        
        ring_ao /= 8.0;
        ao += ring_ao * weight;
        total_weight += weight;
    }
    
    if total_weight > 0.0 {
        ao /= total_weight;
    }
    
    // Apply intensity and return (1.0 = fully lit, 0.0 = fully occluded)
    return saturate(1.0 - ao * AO_INTENSITY);
}
