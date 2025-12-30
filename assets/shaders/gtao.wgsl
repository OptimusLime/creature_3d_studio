// GTAO - Ground Truth Ambient Occlusion
// Based on XeGTAO by Intel
// Using CORRECT depth reconstruction from hardware depth buffer

const PI: f32 = 3.14159265359;
const PI_HALF: f32 = 1.5707963267948966;

// ============================================================================
// Bind Groups
// ============================================================================

// Group 0: G-buffer textures
@group(0) @binding(0) var g_normal: texture_2d<f32>;
@group(0) @binding(1) var g_position: texture_2d<f32>;  // For sky detection (depth > 999)
@group(0) @binding(2) var gbuffer_sampler: sampler;
@group(0) @binding(3) var depth_texture: texture_depth_2d;  // Hardware depth buffer!

// Group 1: Noise
@group(1) @binding(0) var noise_texture: texture_2d<f32>;
@group(1) @binding(1) var noise_sampler: sampler;

// Group 2: Camera uniforms
// IMPORTANT: Layout must match Rust GtaoCameraUniform exactly!
struct CameraUniforms {
    view: mat4x4<f32>,               // 64 bytes
    projection: mat4x4<f32>,         // 64 bytes
    inv_projection: mat4x4<f32>,     // 64 bytes
    screen_size: vec4<f32>,          // 16 bytes - xy = size, zw = 1/size
    // Pack vec2s into vec4s for alignment
    depth_unpack_and_ndc_mul: vec4<f32>,  // xy = depth_unpack_consts, zw = ndc_to_view_mul
    ndc_add_and_params1: vec4<f32>,       // xy = ndc_to_view_add, z = effect_radius, w = effect_falloff_range
    params2: vec4<f32>,                   // x = radius_multiplier, y = final_value_power, z = sample_distribution_power, w = thin_occluder_compensation
}
@group(2) @binding(0) var<uniform> camera: CameraUniforms;

// ============================================================================
// Parameters (can be tuned)
// ============================================================================

const SLICE_COUNT: i32 = 9;       // Number of direction slices
const STEPS_PER_SLICE: i32 = 4;   // Steps per slice direction
// Total samples: 9 slices × 4 steps × 2 directions = 72 samples
// More slices = better angular coverage, helps with noise

// Debug modes: 0 = normal, 1 = show depth, 2 = show normal.z
const DEBUG_GTAO: i32 = 0;  // 0 = normal GTAO output

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
// XeGTAO-style Depth & Position Reconstruction
// ============================================================================

// Accessor functions for packed uniforms
fn get_depth_unpack_consts() -> vec2<f32> {
    return camera.depth_unpack_and_ndc_mul.xy;
}

fn get_ndc_to_view_mul() -> vec2<f32> {
    return camera.depth_unpack_and_ndc_mul.zw;
}

fn get_ndc_to_view_add() -> vec2<f32> {
    return camera.ndc_add_and_params1.xy;
}

fn get_effect_radius() -> f32 {
    return camera.ndc_add_and_params1.z;
}

fn get_effect_falloff_range() -> f32 {
    return camera.ndc_add_and_params1.w;
}

fn get_radius_multiplier() -> f32 {
    return camera.params2.x;
}

fn get_final_value_power() -> f32 {
    return camera.params2.y;
}

fn get_sample_distribution_power() -> f32 {
    return camera.params2.z;
}

// Convert NDC depth [0,1] to linear view-space depth
// For Bevy's INFINITE REVERSE-Z projection:
//   - Near plane = depth 1.0
//   - Far plane = depth 0.0
//   - linear_z = near / ndc_depth
fn screen_space_to_viewspace_depth(screen_depth: f32) -> f32 {
    let consts = get_depth_unpack_consts();
    // Formula: linear_z = mul / (add + ndc_depth)
    // With mul = near, add = small_epsilon
    return consts.x / (consts.y + screen_depth);
}

// Reconstruct view-space position from screen UV and viewspace depth
fn compute_viewspace_position(screen_uv: vec2<f32>, viewspace_depth: f32) -> vec3<f32> {
    // screen_uv is [0,1], convert to NDC [-1,1] style for XeGTAO math
    // XeGTAO uses: (NDCToViewMul * screenPos + NDCToViewAdd) * depth
    // Where screenPos is in [0,1] range
    let ndc_mul = get_ndc_to_view_mul();
    let ndc_add = get_ndc_to_view_add();
    var pos: vec3<f32>;
    pos.x = (ndc_mul.x * screen_uv.x + ndc_add.x) * viewspace_depth;
    pos.y = (ndc_mul.y * screen_uv.y + ndc_add.y) * viewspace_depth;
    pos.z = viewspace_depth;
    return pos;
}

// Sample depth at UV and convert to viewspace depth
fn get_viewspace_depth(uv: vec2<f32>) -> f32 {
    let ndc_depth = textureSample(depth_texture, gbuffer_sampler, uv);
    return screen_space_to_viewspace_depth(ndc_depth);
}

// Get view-space position at UV
fn get_viewspace_pos(uv: vec2<f32>) -> vec3<f32> {
    let depth = get_viewspace_depth(uv);
    return compute_viewspace_position(uv, depth);
}

// Get view-space normal from g-buffer normal (world-space -> view-space)
fn get_viewspace_normal(uv: vec2<f32>) -> vec3<f32> {
    let world_normal = textureSample(g_normal, gbuffer_sampler, uv).xyz;
    // Transform world normal to view space (use mat3 part of view matrix)
    let view_normal = (camera.view * vec4<f32>(world_normal, 0.0)).xyz;
    return normalize(view_normal);
}

// Check if this is sky (far plane)
fn is_sky(uv: vec2<f32>) -> bool {
    return textureSample(g_position, gbuffer_sampler, uv).w > 999.0;
}

// ============================================================================
// XeGTAO Edge Detection
// ============================================================================

fn calculate_edges(center_z: f32, left_z: f32, right_z: f32, top_z: f32, bottom_z: f32) -> vec4<f32> {
    var edges = vec4<f32>(left_z, right_z, top_z, bottom_z) - center_z;
    
    let slope_lr = (edges.y - edges.x) * 0.5;
    let slope_tb = (edges.w - edges.z) * 0.5;
    let edges_slope_adjusted = edges + vec4<f32>(slope_lr, -slope_lr, slope_tb, -slope_tb);
    edges = min(abs(edges), abs(edges_slope_adjusted));
    
    return saturate(vec4<f32>(1.25) - edges / (center_z * 0.011));
}

// ============================================================================
// Fast approximation of acos
// ============================================================================

fn fast_acos(x: f32) -> f32 {
    // Attempt to match XeGTAO's fast_acos
    let x_clamped = clamp(x, -1.0, 1.0);
    // Simple approximation: acos(x) ≈ PI/2 - asin(x), and asin(x) ≈ x + x^3/6 for small x
    // For better accuracy use polynomial approximation
    let abs_x = abs(x_clamped);
    let result = sqrt(1.0 - abs_x) * (PI_HALF - abs_x * (0.175394 + abs_x * 0.0421407));
    return select(result, PI - result, x_clamped < 0.0);
}

// ============================================================================
// GTAO Core Algorithm (XeGTAO-style horizon-based)
// ============================================================================

fn compute_gtao(uv: vec2<f32>, pixel_pos: vec2<f32>) -> f32 {
    // Check for sky
    if is_sky(uv) {
        return 1.0;
    }
    
    let pixel_size = camera.screen_size.zw;
    
    // Get center view-space position and normal
    let viewspace_depth = get_viewspace_depth(uv);
    let P = compute_viewspace_position(uv, viewspace_depth);
    let N = get_viewspace_normal(uv);
    
    // XeGTAO line 287: View direction from pixel to camera (normalized)
    // In view space, camera is at origin, so viewVec = normalize(-P)
    let view_vec = normalize(-P);
    
    // Compute effect radius in screen space
    let world_radius = get_effect_radius() * get_radius_multiplier();
    // Project radius to screen space (approximate)
    let radius_pixels = (world_radius / viewspace_depth) / pixel_size.x * 0.5;
    
    // Clamp radius to reasonable range
    let screenspace_radius = clamp(radius_pixels, 1.0, 256.0);
    
    // XeGTAO L342-343: fade out for small screen radii
    var visibility = saturate((10.0 - screenspace_radius) / 100.0) * 0.5;
    
    // XeGTAO L335, L367: minimum sample distance to avoid sampling center pixel
    let pixel_too_close_threshold = 1.3;
    let min_s = pixel_too_close_threshold / screenspace_radius;
    
    // XeGTAO L304-316: Precompute falloff parameters
    // falloffRange = effectFalloffRange * effectRadius (default effectFalloffRange = 0.615)
    let falloff_range = get_effect_falloff_range() * world_radius;
    let falloff_from = world_radius * (1.0 - get_effect_falloff_range());
    // Optimized: weight = saturate(dist * falloffMul + falloffAdd)
    let falloff_mul = -1.0 / falloff_range;
    let falloff_add = falloff_from / falloff_range + 1.0;
    
    // Get noise for randomizing slice directions to avoid banding
    let noise_uv = pixel_pos / 32.0;
    let noise = textureSample(noise_texture, noise_sampler, noise_uv);
    let noise_slice = noise.x;
    let noise_sample = noise.y;
    
    // For each slice direction
    for (var slice = 0; slice < SLICE_COUNT; slice++) {
        // XeGTAO lines 372-374: Compute slice angle with noise offset
        let slice_k = (f32(slice) + noise_slice) / f32(SLICE_COUNT);
        let phi = slice_k * PI;
        let cos_phi = cos(phi);
        let sin_phi = sin(phi);
        
        // XeGTAO line 377: omega for screen-space offset (note: -sinPhi for Y)
        let omega = vec2<f32>(cos_phi, -sin_phi);
        
        // XeGTAO line 383: direction vector in viewspace (XY plane)
        let direction_vec = vec3<f32>(cos_phi, sin_phi, 0.0);
        
        // XeGTAO line 386: orthoDirectionVec = directionVec - dot(directionVec, viewVec) * viewVec
        let ortho_direction_vec = direction_vec - dot(direction_vec, view_vec) * view_vec;
        
        // XeGTAO line 390: axisVec = normalize(cross(orthoDirectionVec, viewVec))
        let axis_vec = normalize(cross(ortho_direction_vec, view_vec));
        
        // XeGTAO line 396: projectedNormalVec = normal - axisVec * dot(normal, axisVec)
        let projected_normal_vec = N - axis_vec * dot(N, axis_vec);
        
        // XeGTAO line 399: signNorm = sign(dot(orthoDirectionVec, projectedNormalVec))
        let sign_norm = sign(dot(ortho_direction_vec, projected_normal_vec));
        
        // XeGTAO lines 402-403: projectedNormalVecLength and cosNorm
        let projected_normal_vec_length = length(projected_normal_vec);
        let cos_norm = saturate(dot(projected_normal_vec, view_vec) / (projected_normal_vec_length + 0.0001));
        
        // XeGTAO line 406: n = signNorm * FastACos(cosNorm)
        let n_angle = sign_norm * fast_acos(cos_norm);
        
        // XeGTAO lines 409-410: Low horizon cos values
        let low_horizon_cos = vec2<f32>(cos(n_angle + PI_HALF), cos(n_angle - PI_HALF));
        
        // XeGTAO lines 413-414: Initialize horizon cos with low horizon values
        var horizon_cos0 = low_horizon_cos.x;
        var horizon_cos1 = low_horizon_cos.y;
        
        // XeGTAO line 380: convert omega to screen units (pixels)
        let omega_screen = omega * screenspace_radius;
        
        // Sample along the slice in both directions
        for (var step = 0; step < STEPS_PER_SLICE; step++) {
            // XeGTAO L419-421: R1 quasi-random sequence for step noise
            // Golden ratio conjugate = 0.6180339887498948482
            let step_base_noise = f32(slice + step * STEPS_PER_SLICE) * 0.6180339887498948482;
            let step_noise = fract(noise_sample + step_base_noise);
            
            // XeGTAO L423-424: Sample distance with noise
            var s = (f32(step) + step_noise) / f32(STEPS_PER_SLICE);
            
            // XeGTAO L427: additional distribution modifier
            s = pow(s, get_sample_distribution_power());
            
            // XeGTAO L430: avoid sampling center pixel
            s = s + min_s;
            
            // XeGTAO line 433: sample offset in screen space (pixels)
            let sample_offset_pixels = s * omega_screen;
            
            // XeGTAO L440-442: Snap to pixel center for more correct direction math
            let sample_offset = round(sample_offset_pixels) * pixel_size;
            
            // Positive direction (XeGTAO lines 458-493)
            {
                let sample_uv = uv + sample_offset;
                if sample_uv.x >= 0.0 && sample_uv.x <= 1.0 && sample_uv.y >= 0.0 && sample_uv.y <= 1.0 {
                    if !is_sky(sample_uv) {
                        let S = get_viewspace_pos(sample_uv);
                        let delta = S - P;
                        let delta_len = length(delta);
                        
                        if delta_len > 0.001 {
                            // XeGTAO line 472: sampleHorizonVec = delta / delta_len
                            let sample_horizon_vec = delta / delta_len;
                            
                            // XeGTAO line 488: horizon cos = dot(sampleHorizonVec, viewVec)
                            let shc = dot(sample_horizon_vec, view_vec);
                            
                            // XeGTAO L477-478: Falloff weight using precomputed mul/add
                            let weight = saturate(delta_len * falloff_mul + falloff_add);
                            
                            // XeGTAO L492: lerp between low horizon and sample horizon
                            let weighted_shc = mix(low_horizon_cos.x, shc, weight);
                            
                            // XeGTAO line 505: max update
                            horizon_cos0 = max(horizon_cos0, weighted_shc);
                        }
                    }
                }
            }
            
            // Negative direction (XeGTAO lines 462-464)
            {
                let sample_uv = uv - sample_offset;
                if sample_uv.x >= 0.0 && sample_uv.x <= 1.0 && sample_uv.y >= 0.0 && sample_uv.y <= 1.0 {
                    if !is_sky(sample_uv) {
                        let S = get_viewspace_pos(sample_uv);
                        let delta = S - P;
                        let delta_len = length(delta);
                        
                        if delta_len > 0.001 {
                            let sample_horizon_vec = delta / delta_len;
                            let shc = dot(sample_horizon_vec, view_vec);
                            
                            // XeGTAO L478: Falloff weight using precomputed mul/add
                            let weight = saturate(delta_len * falloff_mul + falloff_add);
                            
                            // XeGTAO L493: lerp between low horizon and sample horizon
                            let weighted_shc = mix(low_horizon_cos.y, shc, weight);
                            
                            horizon_cos1 = max(horizon_cos1, weighted_shc);
                        }
                    }
                }
            }
        }
        
        // XeGTAO L531-532: fudge factor for slight overdarkening on high slopes
        let proj_normal_len_adjusted = mix(projected_normal_vec_length, 1.0, 0.05);
        
        // XeGTAO lines 536-537: Convert horizon cos to angles
        // IMPORTANT: h0 uses horizonCos1 (negative direction), h1 uses horizonCos0 (positive direction)
        let h0 = -fast_acos(horizon_cos1);
        let h1 = fast_acos(horizon_cos0);
        
        // XeGTAO lines 542-543: Visibility integration formula
        // IMPORTANT: Uses cosNorm (the saturated dot product), NOT cos(n_angle)
        let n = n_angle;
        let sin_n = sin(n);
        
        let iarc0 = (cos_norm + 2.0 * h0 * sin_n - cos(2.0 * h0 - n)) / 4.0;
        let iarc1 = (cos_norm + 2.0 * h1 * sin_n - cos(2.0 * h1 - n)) / 4.0;
        
        // XeGTAO line 544: localVisibility = projectedNormalVecLength * (iarc0 + iarc1)
        let local_visibility = proj_normal_len_adjusted * (iarc0 + iarc1);
        
        visibility += local_visibility;
    }
    
    // XeGTAO line 556: visibility /= sliceCount
    visibility = visibility / f32(SLICE_COUNT);
    
    // XeGTAO line 557: apply power
    visibility = pow(visibility, get_final_value_power());
    
    // XeGTAO line 558: disallow total occlusion
    visibility = max(0.03, visibility);
    
    return visibility;
}

// ============================================================================
// Fragment Shader
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) f32 {
    // Debug: show reconstructed depth
    if DEBUG_GTAO == 1 {
        if is_sky(in.uv) { return 0.0; }
        // First, show raw NDC depth to verify sampling works
        let ndc_depth = textureSample(depth_texture, gbuffer_sampler, in.uv);
        // For reverse-Z, near=1, far=0, so multiply by 1000 to see small values
        return clamp(ndc_depth * 100.0, 0.0, 1.0);
    }
    
    // Debug: show linear depth
    if DEBUG_GTAO == 3 {
        if is_sky(in.uv) { return 0.0; }
        let depth = get_viewspace_depth(in.uv);
        // Normalize depth for visualization (assuming 0-100 range)
        return clamp(depth / 50.0, 0.0, 1.0);
    }
    
    // Debug: show normal.z (should be mostly positive for upward-facing surfaces)
    if DEBUG_GTAO == 2 {
        if is_sky(in.uv) { return 0.0; }
        let N = get_viewspace_normal(in.uv);
        return N.z * 0.5 + 0.5;
    }
    
    let pixel_pos = in.uv * camera.screen_size.xy;
    return compute_gtao(in.uv, pixel_pos);
}
