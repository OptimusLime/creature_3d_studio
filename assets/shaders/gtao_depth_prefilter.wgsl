// GTAO Depth Prefilter - Compute Shader
// Generates 5-level depth MIP pyramid for XeGTAO
// Based on XeGTAO_PrefilterDepths16x16 from XeGTAO.hlsli L617-684
//
// Input: NDC depth buffer (hardware depth)
// Output: 5 MIP levels of linearized viewspace depth
//
// Workgroup: 8x8 threads, each thread handles 2x2 pixels at MIP 0
// Total coverage: 16x16 pixels per workgroup

// Uniforms for depth linearization and MIP filtering
struct DepthPrefilterUniforms {
    viewport_size: vec2<f32>,          // Full resolution size
    viewport_pixel_size: vec2<f32>,    // 1.0 / viewport_size
    depth_unpack_consts: vec2<f32>,    // xy = depthLinearizeMul, depthLinearizeAdd
    effect_radius: f32,
    effect_falloff_range: f32,
    radius_multiplier: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> uniforms: DepthPrefilterUniforms;
@group(0) @binding(1) var source_depth: texture_depth_2d;

// Output MIP levels (R16Float for viewspace depth)
@group(1) @binding(0) var out_depth_mip0: texture_storage_2d<r16float, write>;
@group(1) @binding(1) var out_depth_mip1: texture_storage_2d<r16float, write>;
@group(1) @binding(2) var out_depth_mip2: texture_storage_2d<r16float, write>;
@group(1) @binding(3) var out_depth_mip3: texture_storage_2d<r16float, write>;
@group(1) @binding(4) var out_depth_mip4: texture_storage_2d<r16float, write>;

// Workgroup shared memory for MIP generation
// 8x8 threads, each writes one value
var<workgroup> scratch_depths: array<array<f32, 8>, 8>;

// Convert NDC depth to linear viewspace depth
// For Bevy's INFINITE REVERSE-Z: linear_z = near / ndc_depth
fn screen_to_viewspace_depth(ndc_depth: f32) -> f32 {
    let mul = uniforms.depth_unpack_consts.x;
    let add = uniforms.depth_unpack_consts.y;
    return mul / (add + ndc_depth);
}

// Clamp depth to valid range (for fp16 precision)
// XeGTAO L607-614
fn clamp_depth(depth: f32) -> f32 {
    return clamp(depth, 0.0, 65504.0);
}

// Weighted average depth filter - preserves depth edges better than simple average
// XeGTAO L579-603: XeGTAO_DepthMIPFilter
fn depth_mip_filter(depth0: f32, depth1: f32, depth2: f32, depth3: f32) -> f32 {
    let max_depth = max(max(depth0, depth1), max(depth2, depth3));
    
    // Scale factor found empirically by XeGTAO authors
    let depth_range_scale_factor = 0.75;
    let effect_radius = depth_range_scale_factor * uniforms.effect_radius * uniforms.radius_multiplier;
    let falloff_range = uniforms.effect_falloff_range * effect_radius;
    let falloff_from = effect_radius * (1.0 - uniforms.effect_falloff_range);
    
    // Precomputed falloff parameters
    let falloff_mul = -1.0 / falloff_range;
    let falloff_add = falloff_from / falloff_range + 1.0;
    
    // Weight based on distance from max depth
    let weight0 = saturate((max_depth - depth0) * falloff_mul + falloff_add);
    let weight1 = saturate((max_depth - depth1) * falloff_mul + falloff_add);
    let weight2 = saturate((max_depth - depth2) * falloff_mul + falloff_add);
    let weight3 = saturate((max_depth - depth3) * falloff_mul + falloff_add);
    
    let weight_sum = weight0 + weight1 + weight2 + weight3;
    return (weight0 * depth0 + weight1 * depth1 + weight2 * depth2 + weight3 * depth3) / weight_sum;
}

@compute @workgroup_size(8, 8, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    // Each thread handles a 2x2 block at MIP 0
    let base_coord = global_id.xy;
    let pix_coord = base_coord * 2u;
    
    // Load 2x2 block of depths directly using integer coordinates
    // textureLoad is the correct method for depth textures in compute shaders
    let ndc_depth0 = textureLoad(source_depth, vec2<i32>(pix_coord) + vec2<i32>(0, 0), 0);
    let ndc_depth1 = textureLoad(source_depth, vec2<i32>(pix_coord) + vec2<i32>(1, 0), 0);
    let ndc_depth2 = textureLoad(source_depth, vec2<i32>(pix_coord) + vec2<i32>(0, 1), 0);
    let ndc_depth3 = textureLoad(source_depth, vec2<i32>(pix_coord) + vec2<i32>(1, 1), 0);
    
    // Convert to linear viewspace depth and clamp
    let depth0 = clamp_depth(screen_to_viewspace_depth(ndc_depth0));
    let depth1 = clamp_depth(screen_to_viewspace_depth(ndc_depth1));
    let depth2 = clamp_depth(screen_to_viewspace_depth(ndc_depth2));
    let depth3 = clamp_depth(screen_to_viewspace_depth(ndc_depth3));
    
    // Write MIP 0 (full resolution linearized depth)
    textureStore(out_depth_mip0, vec2<i32>(pix_coord) + vec2<i32>(0, 0), vec4<f32>(depth0, 0.0, 0.0, 0.0));
    textureStore(out_depth_mip0, vec2<i32>(pix_coord) + vec2<i32>(1, 0), vec4<f32>(depth1, 0.0, 0.0, 0.0));
    textureStore(out_depth_mip0, vec2<i32>(pix_coord) + vec2<i32>(0, 1), vec4<f32>(depth2, 0.0, 0.0, 0.0));
    textureStore(out_depth_mip0, vec2<i32>(pix_coord) + vec2<i32>(1, 1), vec4<f32>(depth3, 0.0, 0.0, 0.0));
    
    // MIP 1: Filter 2x2 -> 1
    let dm1 = depth_mip_filter(depth0, depth1, depth2, depth3);
    textureStore(out_depth_mip1, vec2<i32>(base_coord), vec4<f32>(dm1, 0.0, 0.0, 0.0));
    scratch_depths[local_id.x][local_id.y] = dm1;
    
    workgroupBarrier();
    
    // MIP 2: Every 2x2 threads cooperate
    if (local_id.x % 2u == 0u && local_id.y % 2u == 0u) {
        let in_tl = scratch_depths[local_id.x + 0u][local_id.y + 0u];
        let in_tr = scratch_depths[local_id.x + 1u][local_id.y + 0u];
        let in_bl = scratch_depths[local_id.x + 0u][local_id.y + 1u];
        let in_br = scratch_depths[local_id.x + 1u][local_id.y + 1u];
        
        let dm2 = depth_mip_filter(in_tl, in_tr, in_bl, in_br);
        textureStore(out_depth_mip2, vec2<i32>(base_coord / 2u), vec4<f32>(dm2, 0.0, 0.0, 0.0));
        scratch_depths[local_id.x][local_id.y] = dm2;
    }
    
    workgroupBarrier();
    
    // MIP 3: Every 4x4 threads cooperate
    if (local_id.x % 4u == 0u && local_id.y % 4u == 0u) {
        let in_tl = scratch_depths[local_id.x + 0u][local_id.y + 0u];
        let in_tr = scratch_depths[local_id.x + 2u][local_id.y + 0u];
        let in_bl = scratch_depths[local_id.x + 0u][local_id.y + 2u];
        let in_br = scratch_depths[local_id.x + 2u][local_id.y + 2u];
        
        let dm3 = depth_mip_filter(in_tl, in_tr, in_bl, in_br);
        textureStore(out_depth_mip3, vec2<i32>(base_coord / 4u), vec4<f32>(dm3, 0.0, 0.0, 0.0));
        scratch_depths[local_id.x][local_id.y] = dm3;
    }
    
    workgroupBarrier();
    
    // MIP 4: Only one thread per workgroup
    if (local_id.x == 0u && local_id.y == 0u) {
        let in_tl = scratch_depths[0u][0u];
        let in_tr = scratch_depths[4u][0u];
        let in_bl = scratch_depths[0u][4u];
        let in_br = scratch_depths[4u][4u];
        
        let dm4 = depth_mip_filter(in_tl, in_tr, in_bl, in_br);
        textureStore(out_depth_mip4, vec2<i32>(base_coord / 8u), vec4<f32>(dm4, 0.0, 0.0, 0.0));
    }
}
