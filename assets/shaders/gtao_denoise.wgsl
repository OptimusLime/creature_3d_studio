// GTAO Edge-Aware Denoiser
// Based on XeGTAO.hlsli L686-826
//
// This compute shader implements XeGTAO's edge-aware spatial denoiser.
// It uses packed edge values to preserve depth discontinuities while
// smoothing out noise in the AO.
//
// Key differences from our old 7x7 bilateral blur:
// - Uses precomputed packed edges (2 bits per direction = 4 gradient levels)
// - 3x3 kernel with diagonal weighting (not 7x7 gaussian)
// - Edge symmetry enforcement for sharper blur
// - AO leaking prevention for edge cases

const PI: f32 = 3.14159265359;

// ============================================================================
// Bind Groups
// ============================================================================

// Group 0: Input textures
@group(0) @binding(0) var input_ao: texture_2d<f32>;      // Raw noisy AO
@group(0) @binding(1) var input_edges: texture_2d<f32>;   // Packed edges
@group(0) @binding(2) var input_sampler: sampler;

// Group 1: Output texture (storage image)
@group(1) @binding(0) var output_ao: texture_storage_2d<r8unorm, write>;

// Group 2: Uniforms
struct DenoiseUniforms {
    viewport_size: vec2<f32>,       // Width, height
    viewport_pixel_size: vec2<f32>, // 1/width, 1/height
    denoise_blur_beta: f32,         // XeGTAO default: 1.2
    is_final_pass: u32,             // 1 if final pass, 0 otherwise
    debug_mode: u32,                // 0=normal, 1=sum_weight, 2=edges_c, 3=blur_amount
    padding: f32,
}
@group(2) @binding(0) var<uniform> uniforms: DenoiseUniforms;

// ============================================================================
// XeGTAO Edge Unpacking (L686-696)
// ============================================================================

// Unpack 4 edge values from single packed float
// 2 bits per edge = 4 gradient values (0, 0.33, 0.66, 1.0)
fn unpack_edges(packed_val: f32) -> vec4<f32> {
    // XeGTAO L688: Convert [0,1] to [0,255]
    let packed_int = u32(packed_val * 255.5);
    
    // XeGTAO L689-693: Extract 2 bits each for L, R, T, B
    var edges_lrtb: vec4<f32>;
    edges_lrtb.x = f32((packed_int >> 6u) & 0x03u) / 3.0; // Left
    edges_lrtb.y = f32((packed_int >> 4u) & 0x03u) / 3.0; // Right
    edges_lrtb.z = f32((packed_int >> 2u) & 0x03u) / 3.0; // Top
    edges_lrtb.w = f32((packed_int >> 0u) & 0x03u) / 3.0; // Bottom
    
    return saturate(edges_lrtb);
}

// ============================================================================
// XeGTAO Denoise Helper (L704-710)
// ============================================================================

// Add weighted sample to running sum
fn add_sample(ao_value: f32, edge_value: f32, sum: ptr<function, f32>, sum_weight: ptr<function, f32>) {
    let weight = edge_value;
    *sum += weight * ao_value;
    *sum_weight += weight;
}

// ============================================================================
// Main Denoise Kernel (L734-826)
// ============================================================================

// Workgroup size matches XeGTAO: 8x8 threads, each processes 2 pixels horizontally
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // XeGTAO processes 2 pixels per thread horizontally
    let pix_coord_base = vec2<i32>(i32(global_id.x) * 2, i32(global_id.y));
    
    // Check bounds (we process 2 pixels: pix_coord_base and pix_coord_base + (1,0))
    let viewport_size = vec2<i32>(uniforms.viewport_size);
    if pix_coord_base.x >= viewport_size.x || pix_coord_base.y >= viewport_size.y {
        return;
    }
    

    
    // XeGTAO L736-737: Blur amount based on pass
    let blur_amount = select(
        uniforms.denoise_blur_beta / 5.0,  // Intermediate passes
        uniforms.denoise_blur_beta,         // Final pass
        uniforms.is_final_pass == 1u
    );
    
    // XeGTAO L737: Diagonal weight constant
    let diag_weight = 0.85 * 0.5;
    
    // XeGTAO L747: Gather center for UV calculation
    let gather_center = vec2<f32>(pix_coord_base) * uniforms.viewport_pixel_size;
    
    // For WGSL we can't use GatherRed, so we sample individual pixels
    // Sample edges for 3x3 neighborhood around pix_coord_base and pix_coord_base + (1,0)
    
    // Process both pixels
    for (var side = 0; side < 2; side++) {
        let pix_coord = vec2<i32>(pix_coord_base.x + side, pix_coord_base.y);
        
        // Skip if out of bounds (second pixel might be beyond viewport)
        if pix_coord.x >= viewport_size.x {
            continue;
        }
        
        let center_uv = (vec2<f32>(pix_coord) + 0.5) * uniforms.viewport_pixel_size;
        let pixel_size = uniforms.viewport_pixel_size;
        
        // Denoiser is now active - passthrough debug removed
        
        // Sample edges at neighboring pixels
        let edges_l = unpack_edges(textureSampleLevel(input_edges, input_sampler, center_uv + vec2<f32>(-pixel_size.x, 0.0), 0.0).r);
        let edges_r = unpack_edges(textureSampleLevel(input_edges, input_sampler, center_uv + vec2<f32>(pixel_size.x, 0.0), 0.0).r);
        let edges_t = unpack_edges(textureSampleLevel(input_edges, input_sampler, center_uv + vec2<f32>(0.0, -pixel_size.y), 0.0).r);
        let edges_b = unpack_edges(textureSampleLevel(input_edges, input_sampler, center_uv + vec2<f32>(0.0, pixel_size.y), 0.0).r);
        
        // XeGTAO L766: Center pixel edges
        var edges_c = unpack_edges(textureSampleLevel(input_edges, input_sampler, center_uv, 0.0).r);
        
        // XeGTAO L769-770: Enforce edge symmetry for sharper blur
        // This ensures left neighbor's right edge matches our left edge, etc.
        edges_c *= vec4<f32>(edges_l.y, edges_r.x, edges_t.w, edges_b.z);
        
        // XeGTAO L772-776: AO leaking prevention for 3-4 edge cases
        // When most edges are present (corners/edges of geometry),
        // allow small amount of neighbor bleeding to reduce aliasing
        let leak_threshold = 2.5;
        let leak_strength = 0.5;
        let edginess = (saturate(4.0 - leak_threshold - dot(edges_c, vec4<f32>(1.0))) / (4.0 - leak_threshold)) * leak_strength;
        edges_c = saturate(edges_c + edginess);
        
        // XeGTAO L785-788: Diagonal weights
        // Weight = diagWeight * (center_edge_to_corner * corner_neighbor_edge_from_corner)
        let weight_tl = diag_weight * (edges_c.x * edges_l.z + edges_c.z * edges_t.x);
        let weight_tr = diag_weight * (edges_c.z * edges_t.y + edges_c.y * edges_r.z);
        let weight_bl = diag_weight * (edges_c.w * edges_b.x + edges_c.x * edges_l.w);
        let weight_br = diag_weight * (edges_c.y * edges_r.w + edges_c.w * edges_b.y);
        
        // Sample AO values in 3x3 neighborhood
        let ao_c = textureSampleLevel(input_ao, input_sampler, center_uv, 0.0).r;
        let ao_l = textureSampleLevel(input_ao, input_sampler, center_uv + vec2<f32>(-pixel_size.x, 0.0), 0.0).r;
        let ao_r = textureSampleLevel(input_ao, input_sampler, center_uv + vec2<f32>(pixel_size.x, 0.0), 0.0).r;
        let ao_t = textureSampleLevel(input_ao, input_sampler, center_uv + vec2<f32>(0.0, -pixel_size.y), 0.0).r;
        let ao_b = textureSampleLevel(input_ao, input_sampler, center_uv + vec2<f32>(0.0, pixel_size.y), 0.0).r;
        let ao_tl = textureSampleLevel(input_ao, input_sampler, center_uv + vec2<f32>(-pixel_size.x, -pixel_size.y), 0.0).r;
        let ao_tr = textureSampleLevel(input_ao, input_sampler, center_uv + vec2<f32>(pixel_size.x, -pixel_size.y), 0.0).r;
        let ao_bl = textureSampleLevel(input_ao, input_sampler, center_uv + vec2<f32>(-pixel_size.x, pixel_size.y), 0.0).r;
        let ao_br = textureSampleLevel(input_ao, input_sampler, center_uv + vec2<f32>(pixel_size.x, pixel_size.y), 0.0).r;
        
        // XeGTAO L801-814: Weighted sum
        var sum_weight = blur_amount;
        var sum = ao_c * blur_amount;
        
        // Cardinal directions (use center's edges)
        add_sample(ao_l, edges_c.x, &sum, &sum_weight);
        add_sample(ao_r, edges_c.y, &sum, &sum_weight);
        add_sample(ao_t, edges_c.z, &sum, &sum_weight);
        add_sample(ao_b, edges_c.w, &sum, &sum_weight);
        
        // Diagonal directions (use computed diagonal weights)
        add_sample(ao_tl, weight_tl, &sum, &sum_weight);
        add_sample(ao_tr, weight_tr, &sum, &sum_weight);
        add_sample(ao_bl, weight_bl, &sum, &sum_weight);
        add_sample(ao_br, weight_br, &sum, &sum_weight);
        
        // XeGTAO L814: Normalize
        let denoised_ao = sum / sum_weight;
        
        // Debug output modes
        var output_value = denoised_ao;
        if uniforms.debug_mode == 1u {
            // Visualize sum_weight: should be ~blur_amount + 4*edge_weights + 4*diag_weights
            // For blur_beta=1.2 with full edges, sum_weight ~ 1.2 + 4*1 + 4*0.425 = 6.9
            // Normalize to [0,1] by dividing by 8
            output_value = sum_weight / 8.0;
        } else if uniforms.debug_mode == 2u {
            // Visualize edges_c after symmetry (min of all 4)
            output_value = min(min(edges_c.x, edges_c.y), min(edges_c.z, edges_c.w));
        } else if uniforms.debug_mode == 3u {
            // Visualize blur_amount
            output_value = blur_amount / 2.0;
        } else if uniforms.debug_mode == 4u {
            // Visualize difference: abs(denoised - input)
            // Multiply by 10 to make small differences visible
            output_value = abs(denoised_ao - ao_c) * 10.0;
        }
        
        // Write output
        textureStore(output_ao, pix_coord, vec4<f32>(output_value, 0.0, 0.0, 1.0));
    }
}
