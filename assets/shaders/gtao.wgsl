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

const SLICE_COUNT: i32 = 6;       // Number of direction slices
const STEPS_PER_SLICE: i32 = 6;   // Steps per slice direction
// Total samples: 6 slices × 6 steps × 2 directions = 72 samples

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
    
    // Compute effect radius in screen space
    let world_radius = get_effect_radius() * get_radius_multiplier();
    // Project radius to screen space (approximate)
    let radius_pixels = (world_radius / viewspace_depth) / pixel_size.x * 0.5;
    
    // Clamp radius
    let clamped_radius = clamp(radius_pixels, 1.0, 256.0);
    
    if clamped_radius < 2.0 {
        return 1.0;
    }
    
    // Get noise for randomizing slice directions
    let noise_uv = pixel_pos / 4.0;  // Noise texture tiles every 4 pixels
    let noise = textureSample(noise_texture, noise_sampler, noise_uv / 8.0);
    let noise_slice = noise.x;
    let noise_sample = noise.y;
    
    var visibility = 0.0;
    
    // For each slice direction
    for (var slice = 0; slice < SLICE_COUNT; slice++) {
        // Compute slice angle with noise offset
        let slice_angle = (PI / f32(SLICE_COUNT)) * (f32(slice) + noise_slice);
        let direction = vec2<f32>(cos(slice_angle), sin(slice_angle));
        
        // Orthogonal direction for hemisphere test
        let ortho_dir = vec3<f32>(direction.y, -direction.x, 0.0);
        
        // Project normal onto slice plane (the plane containing view direction and slice direction)
        // This gives us the "projected normal" in the slice
        let slice_normal = N - ortho_dir * dot(N, ortho_dir);
        let slice_normal_len = length(slice_normal);
        let slice_n = slice_normal / (slice_normal_len + 0.0001);
        
        // Angle of normal in the slice (from view direction)
        let n_angle = fast_acos(clamp(slice_n.z, -1.0, 1.0)) * sign(slice_n.x * direction.x + slice_n.y * direction.y);
        
        // Initialize horizon angles
        var horizon_cos = vec2<f32>(-1.0, -1.0);  // cos of horizon angle for +/- directions
        
        // Sample along the slice in both directions
        for (var step = 0; step < STEPS_PER_SLICE; step++) {
            // Sample distance with power distribution (more samples closer to center)
            let step_base = (f32(step) + noise_sample) / f32(STEPS_PER_SLICE);
            let step_dist = pow(step_base, get_sample_distribution_power());
            let sample_offset = direction * step_dist * clamped_radius;
            
            // Positive direction
            {
                let sample_uv = uv + sample_offset * pixel_size;
                if sample_uv.x >= 0.0 && sample_uv.x <= 1.0 && sample_uv.y >= 0.0 && sample_uv.y <= 1.0 {
                    if !is_sky(sample_uv) {
                        let S = get_viewspace_pos(sample_uv);
                        let delta = S - P;
                        let delta_len = length(delta);
                        
                        if delta_len > 0.001 {
                            // Horizon angle: angle from view direction to sample
                            let horizon = delta.z / delta_len;
                            
                            // Falloff based on distance
                            let falloff = saturate(1.0 - delta_len / (world_radius + 0.001));
                            let weighted_horizon = mix(-1.0, horizon, falloff);
                            
                            horizon_cos.x = max(horizon_cos.x, weighted_horizon);
                        }
                    }
                }
            }
            
            // Negative direction
            {
                let sample_uv = uv - sample_offset * pixel_size;
                if sample_uv.x >= 0.0 && sample_uv.x <= 1.0 && sample_uv.y >= 0.0 && sample_uv.y <= 1.0 {
                    if !is_sky(sample_uv) {
                        let S = get_viewspace_pos(sample_uv);
                        let delta = S - P;
                        let delta_len = length(delta);
                        
                        if delta_len > 0.001 {
                            let horizon = delta.z / delta_len;
                            
                            let falloff = saturate(1.0 - delta_len / (world_radius + 0.001));
                            let weighted_horizon = mix(-1.0, horizon, falloff);
                            
                            horizon_cos.y = max(horizon_cos.y, weighted_horizon);
                        }
                    }
                }
            }
        }
        
        // Convert horizon cos to angles
        let horizon_angle_pos = fast_acos(horizon_cos.x);
        let horizon_angle_neg = -fast_acos(horizon_cos.y);
        
        // Clamp horizon angles to hemisphere around normal
        let h0 = max(horizon_angle_neg, n_angle - PI_HALF);
        let h1 = min(horizon_angle_pos, n_angle + PI_HALF);
        
        // Compute visibility for this slice
        // Integration of cos over the visible arc
        let slice_visibility = (slice_normal_len + cos(n_angle) * 0.5) * 
                               (h1 - h0 + sin(h0) * cos(h0) - sin(h1) * cos(h1));
        
        visibility += slice_visibility;
    }
    
    // Normalize by number of slices and apply power
    visibility = visibility / (f32(SLICE_COUNT) * PI);
    visibility = saturate(visibility);
    visibility = pow(visibility, get_final_value_power());
    
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
