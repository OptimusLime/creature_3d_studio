// XeGTAO (Ground Truth Ambient Occlusion) Port to WGSL
// Based on Intel's XeGTAO implementation

// Constants
const PI: f32 = 3.1415926535897932384626433832795;
const PI_HALF: f32 = 1.5707963267948966192313216916398;
const SLICE_COUNT: f32 = 3.0;     // Number of search directions (slices)
const STEPS_per_SLICE: f32 = 3.0; // Samples per direction

// ============================================================================
// Bind Groups
// ============================================================================

// Group 0: G-buffer textures
@group(0) @binding(0) var g_normal: texture_2d<f32>;     // World-space normals in RGB
@group(0) @binding(1) var g_position: texture_2d<f32>;   // World-space position in XYZ, linear depth in W
@group(0) @binding(2) var gbuffer_sampler: sampler;

// Group 1: Noise (kernel unused but kept for binding layout compatibility)
@group(1) @binding(0) var<uniform> unused_kernel: array<vec4<f32>, 64>;
@group(1) @binding(1) var noise_texture: texture_2d<f32>;               // Random rotation vectors
@group(1) @binding(2) var noise_sampler: sampler;

// Group 2: Camera uniforms
struct CameraUniforms {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    inv_projection: mat4x4<f32>,
    screen_size: vec4<f32>,      // (width, height, 1/width, 1/height)
    params1: vec4<f32>,          // x: Radius, y: Falloff, z: RadiusMul, w: SampleDistPower
    params2: vec4<f32>,          // x: ThinComp, y: FinalPower, z: DepthMIP, w: Unused
    tan_half_fov: vec2<f32>,
    padding: vec2<f32>,
}
@group(2) @binding(0) var<uniform> camera: CameraUniforms;

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
// Helper Functions
// ============================================================================

// Fast acos approximation (from XeGTAO)
fn fast_acos(inX: f32) -> f32 {
    let x = abs(inX);
    var res = -0.156583 * x + PI_HALF;
    res *= sqrt(1.0 - x);
    if (inX >= 0.0) { return res; } else { return PI - res; }
}

// Spatio-temporal noise (spatial only for now)
fn spatio_temporal_noise(uv: vec2<f32>) -> vec2<f32> {
    // Sample the 32x32 noise texture
    let noise_scale = camera.screen_size.xy / 32.0;
    let noise_uv = uv * noise_scale;
    let noise_val = textureSample(noise_texture, noise_sampler, noise_uv).xy;
    return noise_val; 
}

// Compute viewspace position from screen UV and View Depth
fn compute_viewspace_position(uv: vec2<f32>, view_depth: f32) -> vec3<f32> {
    // Reconstruct using NDCToViewMul/Add logic from XeGTAO, but adapted for Bevy's camera uniforms
    // Bevy: NDC (-1 to 1). UV (0 to 1).
    // NDC.x = uv.x * 2 - 1
    // NDC.y = (1 - uv.y) * 2 - 1  (Flip Y for Bevy/WGPU standard?) -> Verify this. Bevy UV (0,0) is Top-Left. NDC Y is up.
    // So uv.y=0 -> NDC.y=1. uv.y=1 -> NDC.y=-1. Correct.
    
    let ndc_x = uv.x * 2.0 - 1.0;
    let ndc_y = (1.0 - uv.y) * 2.0 - 1.0; // Invert Y for NDC
    
    // View Space:
    // x = ndc_x * view_depth * tan(fovX/2)
    // y = ndc_y * view_depth * tan(fovY/2)
    // z = -view_depth (Bevy view space: -Z is forward)
    
    // However, XeGTAO code often uses positive Z for calculation logic.
    // Let's stick to Bevy's coordinate system (Right Handed, -Z forward) but handle the math carefully.
    
    let x = ndc_x * camera.tan_half_fov.x * view_depth;
    let y = ndc_y * camera.tan_half_fov.y * view_depth;
    return vec3<f32>(x, y, -view_depth);
}

// ============================================================================
// Fragment Shader - Main Pass
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) f32 {
    let uv = in.uv;
    
    // 1. Get Depth and View Z
    let pos_sample = textureSample(g_position, gbuffer_sampler, uv);
    let world_pos = pos_sample.xyz;
    let linear_depth = pos_sample.w; // Assumed positive distance
    
    // Early exit for sky
    if linear_depth > 999.0 {
        return 1.0;
    }
    
    let view_z = linear_depth; // Use positive depth for radius calcs
    
    // 2. Get Normal (View Space)
    let world_normal = textureSample(g_normal, gbuffer_sampler, uv).xyz;
    // Transform to view space (normal matrix is rotational part of view matrix)
    let view_normal = normalize((camera.view * vec4<f32>(world_normal, 0.0)).xyz);
    
    // 3. Parameters
    let effect_radius = camera.params1.x * camera.params1.z; // Radius * RadiusMultiplier
    let falloff_range = camera.params1.y * effect_radius;
    let sample_dist_power = camera.params1.w;
    let thin_occluder_comp = camera.params2.x;
    let final_value_power = camera.params2.y;
    
    // Falloff calculation constants
    let falloff_from = effect_radius * (1.0 - camera.params1.y);
    let falloff_mul = -1.0 / falloff_range;
    let falloff_add = falloff_from / falloff_range + 1.0;
    
    // 4. Viewspace Position
    // We can use the reconstructed one or transform world_pos.
    // Using transform is safer given we have world_pos
    let view_pos = (camera.view * vec4<f32>(world_pos, 1.0)).xyz;
    // Note: view_pos.z should be approx -view_z
    
    let view_vec = normalize(-view_pos); // Vector from pixel to camera
    
    // 5. Screen Space Radius
    // Approximate pixels size at this depth
    // tan_half_fov.y * 2.0 is the height of the view frustum at depth 1.0
    let frustum_height_at_depth = camera.tan_half_fov.y * 2.0 * view_z;
    let pixel_height_at_depth = frustum_height_at_depth * camera.screen_size.w; // .w is 1/height? No, .w is 1/h.
    // Actually screen_size is (w, h, 1/w, 1/h)
    // pixel_size_view = (tan_half_fov * 2 * z) / resolution
    
    let pixel_dir_view_size = view_z * camera.tan_half_fov.x * 2.0 * camera.screen_size.z; // Just using X axis approx
    
    let screenspace_radius = effect_radius / pixel_dir_view_size;
    
    // Fade out for small radii
    var visibility = clamp((10.0 - screenspace_radius) / 100.0, 0.0, 1.0) * 0.5;
    
    // 6. Horizon Search
    let noise = spatio_temporal_noise(uv);
    let noise_slice = noise.x;
    let noise_sample = noise.y;
    
    let min_s = 1.3 / screenspace_radius; // PixelTooCloseThreshold / screenspace_radius
    
    for (var slice: f32 = 0.0; slice < SLICE_COUNT; slice = slice + 1.0) {
        let slice_k = (slice + noise_slice) / SLICE_COUNT;
        let phi = slice_k * PI;
        let cos_phi = cos(phi);
        let sin_phi = sin(phi);
        let omega = vec2<f32>(cos_phi, -sin_phi) * screenspace_radius; // Screen space direction
        
        // Direction in view space (projected on view plane)
        let dir_vec = vec3<f32>(cos_phi, sin_phi, 0.0);
        let ortho_dir_vec = dir_vec - (dot(dir_vec, view_vec) * view_vec);
        let axis_vec = normalize(cross(ortho_dir_vec, view_vec));
        let proj_normal_vec = view_normal - axis_vec * dot(view_normal, axis_vec);
        
        let proj_normal_len = length(proj_normal_vec);
        let cos_norm = clamp(dot(proj_normal_vec, view_vec) / proj_normal_len, -1.0, 1.0);
        let n = sign(dot(ortho_dir_vec, proj_normal_vec)) * fast_acos(cos_norm);
        
        // Initial horizons (hemisphere boundaries)
        let low_horizon_cos0 = cos(n + PI_HALF);
        let low_horizon_cos1 = cos(n - PI_HALF);
        var horizon_cos0 = low_horizon_cos0;
        var horizon_cos1 = low_horizon_cos1;
        
        for (var step: f32 = 0.0; step < STEPS_per_SLICE; step = step + 1.0) {
            let step_base_noise = (slice + step * STEPS_per_SLICE) * 0.6180339887498948482;
            let step_noise = fract(noise_sample + step_base_noise);
            
            var s = (step + step_noise) / STEPS_per_SLICE;
            s = pow(s, sample_dist_power);
            s = s + min_s;
            
            let sample_offset = s * omega;
            let sample_offset_len = length(sample_offset);
            
            // Sample 0 (Positive Direction)
            let sample_uv0 = uv + sample_offset * camera.screen_size.zw; // .zw is pixel size
            
            // Bounds check
            if (sample_uv0.x >= 0.0 && sample_uv0.x <= 1.0 && sample_uv0.y >= 0.0 && sample_uv0.y <= 1.0) {
                 // In fragment shader, no MIP chain, so just sample base level
                 // This is the simplified "No MIP" fallback
                 let s_pos = textureSample(g_position, gbuffer_sampler, sample_uv0);
                 let s_z = s_pos.w;
                 
                 // Reconstruct sample view pos
                 // Use compute_viewspace_position for consistency with logic
                 // But wait, compute_viewspace_position depends on UV.
                 let s_view_pos = compute_viewspace_position(sample_uv0, s_z);
                 
                 let sample_delta = s_view_pos - view_pos;
                 let sample_dist = length(sample_delta);
                 let sample_horizon_vec = sample_delta / sample_dist;
                 
                 // Falloff/Weight
                 let weight = clamp(sample_dist * falloff_mul + falloff_add, 0.0, 1.0);
                 
                 // Horizon Cosine
                 var shc = dot(sample_horizon_vec, view_vec);
                 shc = mix(low_horizon_cos0, shc, weight);
                 
                 horizon_cos0 = max(horizon_cos0, shc);
            }
            
            // Sample 1 (Negative Direction)
            let sample_uv1 = uv - sample_offset * camera.screen_size.zw;
            if (sample_uv1.x >= 0.0 && sample_uv1.x <= 1.0 && sample_uv1.y >= 0.0 && sample_uv1.y <= 1.0) {
                 let s_pos = textureSample(g_position, gbuffer_sampler, sample_uv1);
                 let s_z = s_pos.w;
                 let s_view_pos = compute_viewspace_position(sample_uv1, s_z);
                 
                 let sample_delta = s_view_pos - view_pos;
                 let sample_dist = length(sample_delta);
                 let sample_horizon_vec = sample_delta / sample_dist;
                 
                 let weight = clamp(sample_dist * falloff_mul + falloff_add, 0.0, 1.0);
                 
                 var shc = dot(sample_horizon_vec, view_vec);
                 shc = mix(low_horizon_cos1, shc, weight);
                 
                 horizon_cos1 = max(horizon_cos1, shc);
            }
        }
        
        // Integration
        // projectedNormalVecLength = lerp( projectedNormalVecLength, 1, 0.05 ); // Fudge from XeGTAO
        let proj_norm_len_biased = mix(proj_normal_len, 1.0, 0.05);
        
        let h0 = -fast_acos(horizon_cos1);
        let h1 = fast_acos(horizon_cos0);
        
        let iarc0 = (cos_norm + 2.0 * h0 * sin(n) - cos(2.0 * h0 - n)) / 4.0;
        let iarc1 = (cos_norm + 2.0 * h1 * sin(n) - cos(2.0 * h1 - n)) / 4.0;
        
        let local_vis = proj_norm_len_biased * (iarc0 + iarc1);
        visibility = visibility + local_vis;
    }
    
    visibility = visibility / SLICE_COUNT;
    visibility = pow(visibility, final_value_power);
    visibility = max(0.03, visibility); // Disallow total occlusion
    
    // Output AO (1.0 = visible, 0.0 = occluded)
    // XeGTAO outputs visibility directly. 
    // Standard AO maps: 1=white=visible, 0=black=occluded.
    return clamp(visibility, 0.0, 1.0);
}
