// SSAO (Screen-Space Ambient Occlusion) shader
// 
// Proper hemisphere sampling implementation in view-space.
// Based on the LearnOpenGL/John Chapman SSAO algorithm.
//
// Algorithm:
// 1. For each pixel, get its view-space position and normal
// 2. Build a TBN matrix to orient the hemisphere along the normal
// 3. Sample random points in the hemisphere
// 4. Project each sample to screen space
// 5. Compare sample depth with actual depth at that screen position
// 6. If actual depth is closer, that sample is occluded

// ============================================================================
// Constants
// ============================================================================

const KERNEL_SIZE: u32 = 64u;

// ============================================================================
// Bind Groups
// ============================================================================

// Group 0: G-buffer textures
@group(0) @binding(0) var g_normal: texture_2d<f32>;     // World-space normals in RGB
@group(0) @binding(1) var g_position: texture_2d<f32>;   // World-space position in XYZ, linear depth in W
@group(0) @binding(2) var gbuffer_sampler: sampler;

// Group 1: SSAO kernel and noise
@group(1) @binding(0) var<uniform> ssao_kernel: array<vec4<f32>, 64>;  // Hemisphere samples (tangent space)
@group(1) @binding(1) var noise_texture: texture_2d<f32>;               // 4x4 random rotation vectors
@group(1) @binding(2) var noise_sampler: sampler;

// Group 2: Camera uniforms
struct CameraUniforms {
    view: mat4x4<f32>,           // World to view space
    projection: mat4x4<f32>,     // View to clip space
    inv_projection: mat4x4<f32>, // Clip to view space (for depth reconstruction)
    screen_size: vec4<f32>,      // (width, height, 1/width, 1/height)
    params: vec4<f32>,           // (radius, bias, intensity, unused)
}
@group(2) @binding(0) var<uniform> camera: CameraUniforms;

// ============================================================================
// Vertex Shader (Fullscreen Triangle)
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
    
    // Get SSAO parameters
    let radius = camera.params.x;
    let base_bias = camera.params.y;
    let intensity = camera.params.z;
    
    // Sample G-buffer
    let world_normal = textureSample(g_normal, gbuffer_sampler, uv).xyz;
    let position_sample = textureSample(g_position, gbuffer_sampler, uv);
    let world_position = position_sample.xyz;
    let linear_depth = position_sample.w;
    
    // Early out for sky/background (depth > 999 means no geometry)
    if linear_depth > 999.0 || length(world_normal) < 0.5 {
        return 1.0;
    }
    
    // Transform world position and normal to view space
    let view_position = (camera.view * vec4<f32>(world_position, 1.0)).xyz;
    let view_normal = normalize((camera.view * vec4<f32>(world_normal, 0.0)).xyz);
    
    // Depth-proportional bias like Bonsai: bias = depth * 0.0005
    // This prevents artifacts at varying distances
    let bias = base_bias + abs(view_position.z) * 0.0005;
    
    // Sample noise texture - contains random rotation vectors in XY plane
    // Tile across the screen for variation
    let noise_scale = camera.screen_size.xy / 32.0;
    let noise_uv = uv * noise_scale;
    let noise_raw = textureSample(noise_texture, noise_sampler, noise_uv).xyz;
    // Decode from [0,1] to [-1,1]
    let random_vec = normalize(noise_raw * 2.0 - 1.0);
    
    // Create TBN matrix to orient hemisphere along the normal
    // Gram-Schmidt process: make tangent perpendicular to normal
    var tangent = random_vec - view_normal * dot(random_vec, view_normal);
    let tangent_len = length(tangent);
    if tangent_len < 0.001 {
        // Fallback if random vector is parallel to normal
        if abs(view_normal.y) < 0.9 {
            tangent = vec3<f32>(0.0, 1.0, 0.0);
        } else {
            tangent = vec3<f32>(1.0, 0.0, 0.0);
        }
        tangent = tangent - view_normal * dot(tangent, view_normal);
    }
    tangent = normalize(tangent);
    let bitangent = cross(view_normal, tangent);
    
    // TBN transforms from tangent space to view space
    // Kernel samples are in tangent space with Z pointing along normal
    let tbn = mat3x3<f32>(tangent, bitangent, view_normal);
    
    // Accumulate occlusion
    var occlusion: f32 = 0.0;
    
    for (var i: u32 = 0u; i < KERNEL_SIZE; i = i + 1u) {
        // Get sample offset in tangent space (Z is along normal)
        let kernel_sample = ssao_kernel[i].xyz;
        
        // Transform sample to view space and add to current position
        let sample_view_pos = view_position + tbn * kernel_sample * radius;
        
        // Project sample position to clip space
        let sample_clip = camera.projection * vec4<f32>(sample_view_pos, 1.0);
        
        // Perspective divide to get NDC
        var sample_ndc = sample_clip.xyz / sample_clip.w;
        
        // Convert NDC [-1,1] to UV [0,1]
        // Note: Y is flipped for texture coordinates
        let sample_uv = vec2<f32>(
            sample_ndc.x * 0.5 + 0.5,
            -sample_ndc.y * 0.5 + 0.5
        );
        
        // Bounds check - skip samples outside screen
        if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
            continue;
        }
        
        // Sample the G-buffer at the projected position to get actual geometry there
        let sampled_pos = textureSample(g_position, gbuffer_sampler, sample_uv);
        let sampled_world_pos = sampled_pos.xyz;
        let sampled_depth = sampled_pos.w;
        
        // Skip sky samples
        if sampled_depth > 999.0 {
            continue;
        }
        
        // Transform sampled world position to view space
        let sampled_view_pos = (camera.view * vec4<f32>(sampled_world_pos, 1.0)).xyz;
        
        // Compare Z values in view space
        // sample_view_pos = where our hemisphere sample ended up
        // sampled_view_pos = actual geometry at that screen position
        //
        // If actual geometry Z > sample Z (less negative = closer to camera),
        // then there's something blocking that sample direction
        let sample_z = sample_view_pos.z;
        let actual_z = sampled_view_pos.z;
        
        // Range check to avoid counting distant geometry
        let z_diff = actual_z - sample_z;
        
        // Occlusion: actual geometry is closer to camera (higher Z) than sample point
        // AND within a reasonable range (not a depth discontinuity)
        if z_diff > bias && z_diff < radius {
            occlusion += 1.0;
        }
    }
    
    // Normalize occlusion
    occlusion = occlusion / f32(KERNEL_SIZE);
    
    // Apply intensity and invert (1.0 = fully lit, 0.0 = fully occluded)
    let ao = saturate(1.0 - occlusion * intensity);
    
    return ao;
}

// ============================================================================
// Blur Pass (for smoothing SSAO noise)
// This would be a separate render pass in production, but for now we do
// a simple edge-aware blur inline by sampling neighbors
// ============================================================================

// Helper: Sample AO with depth-aware weighting
fn sample_ao_weighted(base_uv: vec2<f32>, offset: vec2<f32>, base_depth: f32, texel_size: vec2<f32>) -> vec2<f32> {
    let sample_uv = base_uv + offset * texel_size;
    
    // Bounds check
    if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
        return vec2<f32>(0.0, 0.0); // (weighted_ao, weight)
    }
    
    let sample_pos = textureSample(g_position, gbuffer_sampler, sample_uv);
    let sample_depth = sample_pos.w;
    
    // Skip sky
    if sample_depth > 999.0 {
        return vec2<f32>(0.0, 0.0);
    }
    
    // Depth-based weight: reduce weight for samples at very different depths
    let depth_diff = abs(base_depth - sample_depth);
    let depth_weight = exp(-depth_diff * 2.0); // Exponential falloff
    
    // Would sample SSAO texture here if this were a separate pass
    // For now, return placeholder
    return vec2<f32>(depth_weight, depth_weight);
}
