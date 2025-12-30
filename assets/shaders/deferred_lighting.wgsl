// Deferred Lighting Shader
// 
// Fullscreen pass that reads G-buffer textures and computes lighting.
// Based on Bonsai's Lighting.fragmentshader
//
// G-Buffer inputs:
// - gColor: RGB = albedo, A = emission intensity (0-1)
// - gNormal: RGB = world-space normal, A = ambient occlusion (0-1)
// - gPosition: XYZ = world position, W = linear depth

// G-buffer textures (bind group 0)
@group(0) @binding(0) var gColor: texture_2d<f32>;
@group(0) @binding(1) var gNormal: texture_2d<f32>;
@group(0) @binding(2) var gPosition: texture_2d<f32>;
@group(0) @binding(3) var gbuffer_sampler: sampler;

// Dual directional shadow maps (bind group 1) - moon1 + moon2
@group(1) @binding(0) var moon1_shadow_map: texture_depth_2d;
@group(1) @binding(1) var moon2_shadow_map: texture_depth_2d;
@group(1) @binding(2) var shadow_sampler: sampler_comparison;

// Directional shadow uniforms (bind group 2) - both moon matrices and config
struct DirectionalShadowUniforms {
    moon1_view_proj: mat4x4<f32>,
    moon2_view_proj: mat4x4<f32>,
    moon1_direction: vec4<f32>,      // xyz = direction, w = unused
    moon1_color_intensity: vec4<f32>, // rgb = color, a = intensity
    moon2_direction: vec4<f32>,
    moon2_color_intensity: vec4<f32>,
    shadow_softness: vec4<f32>,      // x = directional, y = point, zw = unused
}
@group(2) @binding(0) var<uniform> shadow_uniforms: DirectionalShadowUniforms;

// Point lights (bind group 3) - Storage buffer for high volume lights
// Must match MAX_POINT_LIGHTS in point_light.rs
const MAX_POINT_LIGHTS: u32 = 256u;

struct PointLight {
    position: vec4<f32>,        // xyz = position, w = unused
    color_intensity: vec4<f32>, // rgb = color, a = intensity
    radius_padding: vec4<f32>,  // x = radius, yzw = unused
}

// Storage buffer layout: [header (16 bytes)] [lights array]
// Using storage buffer instead of uniform allows:
// - Much higher light counts (256+ vs ~32)
// - Dynamic array sizing
// - Better performance for sparse light iteration
struct PointLightsStorage {
    count: vec4<u32>,  // x = count, yzw = unused
    lights: array<PointLight>,  // Runtime-sized array (storage buffer feature)
}
@group(3) @binding(0) var<storage, read> point_lights: PointLightsStorage;

// Point light shadow maps (bind group 4) - 6 face textures for cube shadow map
// Face order: +X, -X, +Y, -Y, +Z, -Z
@group(4) @binding(0) var point_shadow_face_px: texture_depth_2d;
@group(4) @binding(1) var point_shadow_face_nx: texture_depth_2d;
@group(4) @binding(2) var point_shadow_face_py: texture_depth_2d;
@group(4) @binding(3) var point_shadow_face_ny: texture_depth_2d;
@group(4) @binding(4) var point_shadow_face_pz: texture_depth_2d;
@group(4) @binding(5) var point_shadow_face_nz: texture_depth_2d;
@group(4) @binding(6) var point_shadow_sampler: sampler_comparison;

// Point shadow matrices (bind group 5) - view-proj matrices for cube face sampling
struct PointShadowMatrices {
    face_matrices: array<mat4x4<f32>, 6>,  // +X, -X, +Y, -Y, +Z, -Z
    light_pos_radius: vec4<f32>,           // xyz = position, w = radius
}
@group(5) @binding(0) var<uniform> point_shadow_matrices: PointShadowMatrices;

// GTAO texture (bind group 6) - screen-space ambient occlusion
// This replaces per-vertex AO from the G-buffer when enabled
@group(6) @binding(0) var gtao_texture: texture_2d<f32>;
@group(6) @binding(1) var gtao_sampler: sampler;

// ============================================================================
// Edge-Aware Bilateral GTAO Blur
// ============================================================================
// Performs a 5x5 depth-weighted blur on the GTAO texture to reduce noise
// while preserving sharp edges at depth discontinuities.
// GTAO is at half-res, so this blur covers ~2.5 pixels in GTAO space.

fn sample_gtao_with_blur(uv: vec2<f32>, center_depth: f32) -> f32 {
    // Get texture dimensions for pixel offset calculation
    let tex_dims = vec2<f32>(textureDimensions(gtao_texture));
    let pixel_size = 1.0 / tex_dims;
    
    // Depth threshold for edge detection - relative to center depth
    // Larger = more blur across depth differences
    let depth_threshold = 0.08 * center_depth + 1.0;
    
    var total_ao = 0.0;
    var total_weight = 0.0;
    
    // 7x7 blur kernel for strong denoising
    // GTAO at half-res means this covers ~3.5 GTAO pixels
    let kernel_radius = 3;
    
    for (var dy = -kernel_radius; dy <= kernel_radius; dy++) {
        for (var dx = -kernel_radius; dx <= kernel_radius; dx++) {
            let offset = vec2<f32>(f32(dx), f32(dy));
            let sample_uv = uv + offset * pixel_size;
            
            // Bounds check
            if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
                continue;
            }
            
            // Sample AO
            let ao_sample = textureSample(gtao_texture, gtao_sampler, sample_uv).r;
            
            // Sample depth for edge detection
            let sample_depth = textureSample(gPosition, gbuffer_sampler, sample_uv).w;
            
            // Gaussian spatial weight (sigma ≈ 1.5)
            let dist_sq = f32(dx * dx + dy * dy);
            let spatial_weight = exp(-dist_sq / 4.5);
            
            // Edge-aware weight: reduce weight for depth discontinuities
            let depth_diff = abs(sample_depth - center_depth);
            let edge_weight = exp(-depth_diff * depth_diff / (depth_threshold * depth_threshold));
            
            // Combined weight
            let weight = spatial_weight * edge_weight;
            
            total_ao += ao_sample * weight;
            total_weight += weight;
        }
    }
    
    // Return weighted average
    if total_weight > 0.001 {
        return total_ao / total_weight;
    } else {
        return textureSample(gtao_texture, gtao_sampler, uv).r;
    }
}

// Point shadow map size (must match POINT_SHADOW_MAP_SIZE in Rust)
const POINT_SHADOW_MAP_SIZE: f32 = 512.0;

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
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// === LIGHTING CONSTANTS ===
// Dark World mode: Two colored moons instead of sun
// Set to 1 for dual moon mode, 0 for classic sun mode
const DARK_WORLD_MODE: i32 = 1;

// --- Classic Sun Mode (DARK_WORLD_MODE = 0) ---
const AMBIENT_COLOR: vec3<f32> = vec3<f32>(0.2, 0.15, 0.25);  // Slightly purple ambient
const AMBIENT_INTENSITY: f32 = 0.2;  // Base illumination

// Sun coming from upper-left-front - biased toward Y for clear top/side difference
const SUN_DIRECTION: vec3<f32> = vec3<f32>(0.3, -0.9, -0.3);
const SUN_COLOR: vec3<f32> = vec3<f32>(1.0, 0.95, 0.9);  // Warm white
const SUN_INTENSITY: f32 = 1.0;

// Fill light from lower-front-left - illuminates shadowed faces
const FILL_DIRECTION: vec3<f32> = vec3<f32>(-0.5, 0.3, 0.8);
const FILL_COLOR: vec3<f32> = vec3<f32>(0.5, 0.6, 0.8);  // Cool blue
const FILL_INTENSITY: f32 = 0.4;

// --- Dark World Mode (DARK_WORLD_MODE = 1) ---
// Purple Moon - from back-left, moderate height
const MOON1_DIRECTION: vec3<f32> = vec3<f32>(0.6, -0.6, 0.55);  // Back-left
const MOON1_COLOR: vec3<f32> = vec3<f32>(0.4, 0.15, 0.7);  // Deep purple
const MOON1_INTENSITY: f32 = 0.15;  // Very dim - let point lights dominate

// Orange Moon - from front-right, similar height (lights opposite faces)
const MOON2_DIRECTION: vec3<f32> = vec3<f32>(-0.6, -0.6, -0.55);  // Front-right
const MOON2_COLOR: vec3<f32> = vec3<f32>(1.0, 0.45, 0.1);  // Deep orange
const MOON2_INTENSITY: f32 = 0.12;  // Very dim

// Dark world ambient - near zero (very dark scene)
const DARK_AMBIENT_COLOR: vec3<f32> = vec3<f32>(0.01, 0.005, 0.02);
const DARK_AMBIENT_INTENSITY: f32 = 0.05;

// --- Fog Settings ---
const FOG_COLOR: vec3<f32> = vec3<f32>(0.02, 0.01, 0.03);  // Near black for dark world
const FOG_START: f32 = 30.0;
const FOG_END: f32 = 100.0;

// Shadow map constants
const SHADOW_MAP_SIZE: f32 = 2048.0;
const SHADOW_BIAS_MIN: f32 = 0.001;  // Minimum bias to prevent shadow acne
const SHADOW_BIAS_MAX: f32 = 0.01;   // Maximum bias for grazing angles

// Poisson disk samples for soft shadows (16 samples in a unit circle)
// These provide good coverage with minimal banding artifacts
const POISSON_DISK: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(-0.94201624, -0.39906216),
    vec2<f32>(0.94558609, -0.76890725),
    vec2<f32>(-0.094184101, -0.92938870),
    vec2<f32>(0.34495938, 0.29387760),
    vec2<f32>(-0.91588581, 0.45771432),
    vec2<f32>(-0.81544232, -0.87912464),
    vec2<f32>(-0.38277543, 0.27676845),
    vec2<f32>(0.97484398, 0.75648379),
    vec2<f32>(0.44323325, -0.97511554),
    vec2<f32>(0.53742981, -0.47373420),
    vec2<f32>(-0.26496911, -0.41893023),
    vec2<f32>(0.79197514, 0.19090188),
    vec2<f32>(-0.24188840, 0.99706507),
    vec2<f32>(-0.81409955, 0.91437590),
    vec2<f32>(0.19984126, 0.78641367),
    vec2<f32>(0.14383161, -0.14100790)
);

// Debug mode: 0 = final lighting, 1 = show gNormal, 2 = show gPosition depth, 3 = albedo only, 4 = shadow only, 5 = AO only, 6 = point lights only, 7 = world position XZ, 8 = light count, 9 = distance to light 0, 10 = first light color, 11 = first light radius, 20 = point shadow for light 0, 21 = raw -Y face shadow sample, 22 = face UV coords, 23 = which cube face, 24 = compare depth, 25 = show UV coords for -Y face, 26 = test fixed UV sample
// 50 = matrix-based UV, 51 = compare matrix vs manual UV, 52 = stored vs compare depth
// 61 = shadow depth difference visualization
// 34 = raw -Y shadow map contents displayed as fullscreen
// 62 = stored depth for selected face (all faces, not just -Y)
// 63 = raw +X face shadow map
// 70 = face_multiplier only (discrete bands), 71 = N·L only for moon/sun (R=moon1, G=moon2)
// 41 = moon1 shadow only, 42 = moon2 shadow only, 43 = both overlap (blue)
// 100 = GTAO only (raw GTAO texture output)
// 101 = GTAO raw center sample (no blur)
const DEBUG_MODE: i32 = 0;

// Calculate point light contribution at a world position.
// Uses smooth falloff and N·L for realistic colored lighting.
// shadow_factor: 0.0 = in shadow, 1.0 = lit
fn calculate_point_light_with_shadow(
    light: PointLight,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    shadow_factor: f32,
) -> vec3<f32> {
    let light_pos = light.position.xyz;
    let radius = light.radius_padding.x;
    
    // Vector from surface to light
    let to_light = light_pos - world_pos;
    let distance = length(to_light);
    
    // Outside radius = no contribution
    if distance > radius {
        return vec3<f32>(0.0);
    }
    
    let light_color = light.color_intensity.rgb;
    let intensity = light.color_intensity.a;
    
    // Normalize direction to light
    let light_dir = to_light / distance;
    
    // N·L factor - surfaces facing the light get more light
    let n_dot_l = max(dot(world_normal, light_dir), 0.0);
    
    // Smooth quadratic falloff (more realistic than linear)
    let falloff_linear = 1.0 - (distance / radius);
    let falloff = falloff_linear * falloff_linear;
    
    // Final contribution: color * intensity * falloff * N·L * shadow
    // Intensity is from the light source, typically 1-50
    // Scale factor tuned for visibility: 0.1 works well with intensity 10-50
    return light_color * intensity * falloff * n_dot_l * shadow_factor * 0.1;
}

// Calculate point light contribution without shadow.
fn calculate_point_light(
    light: PointLight,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
) -> vec3<f32> {
    return calculate_point_light_with_shadow(light, world_pos, world_normal, 1.0);
}

// Calculate total contribution from all point lights.
// First light (index 0) casts shadows, others do not.
fn calculate_all_point_lights(world_pos: vec3<f32>, world_normal: vec3<f32>) -> vec3<f32> {
    var total = vec3<f32>(0.0);
    let light_count = point_lights.count.x;
    
    for (var i = 0u; i < light_count && i < MAX_POINT_LIGHTS; i++) {
        let light = point_lights.lights[i];
        
        // First light casts shadows
        // NOTE: Point light shadows are temporarily disabled due to spurious shadow artifacts.
        // The shadow map rendering and sampling have a mismatch causing incorrect shadows
        // in areas that should be lit. This creates pink patches in Dark World mode.
        // TODO: Fix point light shadow sampling - see POINT_LIGHT_SHADOW_DEBUG.md
        if i == 0u {
            let shadow = 1.0;  // Disabled - was: calculate_point_shadow(...)
            total += calculate_point_light_with_shadow(light, world_pos, world_normal, shadow);
        } else {
            total += calculate_point_light(light, world_pos, world_normal);
        }
    }
    
    return total;
}

// Calculate soft shadow using Poisson disk sampling.
// shadow_map: the depth texture to sample
// view_proj: light-space view-projection matrix
// world_pos: fragment world position
// world_normal: fragment world normal
// light_dir: direction TO the light (normalized)
// softness: 0.0 = hard shadows, 1.0 = very soft
fn calculate_soft_directional_shadow(
    shadow_map: texture_depth_2d,
    view_proj: mat4x4<f32>,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    light_dir: vec3<f32>,
    softness: f32,
) -> f32 {
    // Transform world position to light clip space
    let light_space_pos = view_proj * vec4<f32>(world_pos, 1.0);
    let proj_coords = light_space_pos.xyz / light_space_pos.w;
    
    // Transform from NDC [-1,1] to texture UV [0,1]
    let shadow_uv = vec2<f32>(
        proj_coords.x * 0.5 + 0.5,
        proj_coords.y * -0.5 + 0.5  // Flip Y
    );
    
    let current_depth = proj_coords.z;
    
    // Check bounds
    if shadow_uv.x < 0.0 || shadow_uv.x > 1.0 || shadow_uv.y < 0.0 || shadow_uv.y > 1.0 {
        return 1.0;
    }
    if current_depth > 1.0 || current_depth < 0.0 {
        return 1.0;
    }
    
    // Slope-scaled bias
    let n_dot_l = max(dot(world_normal, light_dir), 0.0);
    let bias = max(SHADOW_BIAS_MAX * (1.0 - n_dot_l), SHADOW_BIAS_MIN);
    
    // Softness controls the sample radius (0-3 texels)
    let radius = softness * 3.0 / SHADOW_MAP_SIZE;
    
    // 16-sample Poisson disk for smooth soft shadows
    var shadow_sum = 0.0;
    for (var i = 0; i < 16; i++) {
        let offset = POISSON_DISK[i] * radius;
        shadow_sum += textureSampleCompare(
            shadow_map,
            shadow_sampler,
            shadow_uv + offset,
            current_depth - bias
        );
    }
    
    return shadow_sum / 16.0;
}

// Calculate Moon 1 (purple) shadow
fn calculate_moon1_shadow(world_pos: vec3<f32>, world_normal: vec3<f32>) -> f32 {
    let light_dir = normalize(-shadow_uniforms.moon1_direction.xyz);
    let softness = shadow_uniforms.shadow_softness.x;
    return calculate_soft_directional_shadow(
        moon1_shadow_map,
        shadow_uniforms.moon1_view_proj,
        world_pos,
        world_normal,
        light_dir,
        softness
    );
}

// Calculate Moon 2 (orange) shadow
fn calculate_moon2_shadow(world_pos: vec3<f32>, world_normal: vec3<f32>) -> f32 {
    let light_dir = normalize(-shadow_uniforms.moon2_direction.xyz);
    let softness = shadow_uniforms.shadow_softness.x;
    return calculate_soft_directional_shadow(
        moon2_shadow_map,
        shadow_uniforms.moon2_view_proj,
        world_pos,
        world_normal,
        light_dir,
        softness
    );
}

// Legacy shadow calculation (kept for compatibility, uses moon1 by default)
fn calculate_shadow(world_pos: vec3<f32>, world_normal: vec3<f32>) -> f32 {
    return calculate_moon1_shadow(world_pos, world_normal);
}

// Helper function to sample a single point from the appropriate shadow face
fn sample_point_shadow_face(face_idx: i32, uv: vec2<f32>, compare_depth: f32) -> f32 {
    switch face_idx {
        case 0: { return textureSampleCompare(point_shadow_face_px, point_shadow_sampler, uv, compare_depth); }
        case 1: { return textureSampleCompare(point_shadow_face_nx, point_shadow_sampler, uv, compare_depth); }
        case 2: { return textureSampleCompare(point_shadow_face_py, point_shadow_sampler, uv, compare_depth); }
        case 3: { return textureSampleCompare(point_shadow_face_ny, point_shadow_sampler, uv, compare_depth); }
        case 4: { return textureSampleCompare(point_shadow_face_pz, point_shadow_sampler, uv, compare_depth); }
        case 5: { return textureSampleCompare(point_shadow_face_nz, point_shadow_sampler, uv, compare_depth); }
        default: { return 1.0; }
    }
}

// Compute cube face UV directly from direction vector.
// This matches standard OpenGL cubemap conventions used by look_at with standard up vectors.
// Reference: https://learnopengl.com/Advanced-Lighting/Shadows/Point-Shadows
//
// The UV formulas are derived from how look_at matrices orient each face:
// - X and Z faces: up = -Y, so texture V maps to world Y
// - Y faces: up = +/-Z, so texture V maps to world Z
fn compute_cube_face_uv(dir: vec3<f32>, face_idx: i32) -> vec2<f32> {
    let abs_dir = abs(dir);
    var u: f32;
    var v: f32;
    
    switch face_idx {
        // +X face: looking from light toward +X
        // Camera right = -Z, camera up = -Y
        case 0: {
            u = -dir.z / abs_dir.x;
            v = -dir.y / abs_dir.x;
        }
        // -X face: looking from light toward -X
        // Camera right = +Z, camera up = -Y
        case 1: {
            u = dir.z / abs_dir.x;
            v = -dir.y / abs_dir.x;
        }
        // +Y face: looking from light toward +Y
        // Camera right = +X, camera up = +Z
        case 2: {
            u = dir.x / abs_dir.y;
            v = dir.z / abs_dir.y;
        }
        // -Y face: looking from light toward -Y
        // Camera right = +X, camera up = -Z
        case 3: {
            u = dir.x / abs_dir.y;
            v = -dir.z / abs_dir.y;
        }
        // +Z face: looking from light toward +Z
        // Camera right = +X, camera up = -Y
        case 4: {
            u = dir.x / abs_dir.z;
            v = -dir.y / abs_dir.z;
        }
        // -Z face: looking from light toward -Z
        // Camera right = -X, camera up = -Y
        case 5: {
            u = -dir.x / abs_dir.z;
            v = -dir.y / abs_dir.z;
        }
        default: {
            u = 0.0;
            v = 0.0;
        }
    }
    
    // Map from [-1, 1] to [0, 1]
    return vec2<f32>(u * 0.5 + 0.5, v * 0.5 + 0.5);
}

// Calculate shadow for a point light using cube shadow map with soft PCF.
// Returns 0.0 for fully in shadow, 1.0 for fully lit.
//
// Uses matrix-based UV calculation to match how shadow maps were rendered.
// Softness is controlled by shadow_uniforms.shadow_softness.y
fn calculate_point_shadow(light_pos: vec3<f32>, world_pos: vec3<f32>, radius: f32) -> f32 {
    // Use the light position from the shadow matrices for consistency
    let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
    let shadow_radius = point_shadow_matrices.light_pos_radius.w;
    
    // Vector from light to fragment
    let light_to_frag = world_pos - shadow_light_pos;
    let distance = length(light_to_frag);
    
    // Skip if outside light radius
    if distance > shadow_radius {
        return 1.0;
    }
    
    let abs_vec = abs(light_to_frag);
    
    // Select cube face based on dominant axis
    // Face order: 0=+X, 1=-X, 2=+Y, 3=-Y, 4=+Z, 5=-Z
    var face_idx: i32;
    if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
        face_idx = select(1, 0, light_to_frag.x > 0.0);
    } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
        face_idx = select(3, 2, light_to_frag.y > 0.0);
    } else {
        face_idx = select(5, 4, light_to_frag.z > 0.0);
    }
    
    // Use matrix-based UV calculation (matches how shadow maps were rendered)
    let view_proj = point_shadow_matrices.face_matrices[face_idx];
    let clip = view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    // NDC to UV: x [-1,1] -> [0,1], y [-1,1] -> [1,0] (flip Y for texture coords)
    let face_uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
    
    // Check if UV is valid - if outside [0,1] range, we're at the edge of the cube face
    // and shouldn't cast shadow (return lit)
    if face_uv.x < 0.0 || face_uv.x > 1.0 || face_uv.y < 0.0 || face_uv.y > 1.0 {
        return 1.0;
    }
    
    // Depth comparison: we store linear distance/radius
    // Bias to prevent self-shadowing (shadow acne)
    let compare_depth = (distance / shadow_radius) - 0.05;
    
    // Get softness from uniforms (0.0 = hard, 1.0 = very soft)
    let softness = shadow_uniforms.shadow_softness.y;
    let radius_scale = softness * 2.0 / POINT_SHADOW_MAP_SIZE;  // 0-2 texels based on softness
    
    // Use Poisson disk sampling for softer shadows
    var shadow_sum = 0.0;
    for (var i = 0; i < 16; i++) {
        let offset = POISSON_DISK[i] * radius_scale;
        // Clamp UV to valid range to avoid sampling garbage at edges
        let sample_uv = clamp(face_uv + offset, vec2<f32>(0.001), vec2<f32>(0.999));
        shadow_sum += sample_point_shadow_face(face_idx, sample_uv, compare_depth);
    }
    
    return shadow_sum / 16.0;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // EARLY DEBUG: Show light count immediately (before any G-buffer reads)
    if DEBUG_MODE == 99 {
        let count = f32(point_lights.count.x) / 32.0;
        return vec4<f32>(count, count, count, 1.0);
    }
    
    // EARLY DEBUG: Show first light's color (before depth check)
    if DEBUG_MODE == 98 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            return vec4<f32>(light.color_intensity.rgb, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);
    }
    
    // EARLY DEBUG: Show distance to light 0 as color gradient
    if DEBUG_MODE == 97 {
        let position_sample = textureSample(gPosition, gbuffer_sampler, in.uv);
        let world_pos = position_sample.xyz;
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let dist = distance(world_pos, light.position.xyz);
            let radius = light.radius_padding.x;
            // Red = distance/radius (>1 means outside), Green = in-range flag
            let in_range = select(0.0, 1.0, dist <= radius);
            return vec4<f32>(dist / radius, in_range, 0.0, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);
    }
    
    // EARLY DEBUG: Show calculate_point_light output directly for light 0
    if DEBUG_MODE == 96 {
        let position_sample = textureSample(gPosition, gbuffer_sampler, in.uv);
        let normal_sample = textureSample(gNormal, gbuffer_sampler, in.uv);
        let world_pos = position_sample.xyz;
        let world_normal = normalize(normal_sample.rgb);
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let result = calculate_point_light(light, world_pos, world_normal);
            // Scale up by 10 to make visible
            return vec4<f32>(result * 10.0, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);
    }
    
    // DEBUG: Show depth > 999 check result
    if DEBUG_MODE == 95 {
        let position_sample = textureSample(gPosition, gbuffer_sampler, in.uv);
        let depth = position_sample.w;
        if depth > 999.0 {
            return vec4<f32>(1.0, 0.0, 0.0, 1.0); // Red = fail depth check (no geometry)
        }
        return vec4<f32>(0.0, 1.0, 0.0, 1.0); // Green = pass depth check (has geometry)
    }
    
    // DEBUG: Show gPosition XYZ directly (scaled for visibility)
    if DEBUG_MODE == 94 {
        let position_sample = textureSample(gPosition, gbuffer_sampler, in.uv);
        // Scale: expect positions around -10 to +10
        let pos_norm = (position_sample.xyz + 10.0) / 20.0;
        return vec4<f32>(pos_norm, 1.0);
    }
    
    // DEBUG: Show world Y position only (ground should be Y≈0)
    if DEBUG_MODE == 93 {
        let position_sample = textureSample(gPosition, gbuffer_sampler, in.uv);
        // Show raw Y: positive = green, negative = red, scaled by /50
        let y = position_sample.y;
        if y >= 0.0 {
            return vec4<f32>(0.0, y / 50.0, 0.0, 1.0);
        } else {
            return vec4<f32>(-y / 50.0, 0.0, 0.0, 1.0);
        }
    }
    
    // Sample G-buffer
    let color_sample = textureSample(gColor, gbuffer_sampler, in.uv);
    let normal_sample = textureSample(gNormal, gbuffer_sampler, in.uv);
    let position_sample = textureSample(gPosition, gbuffer_sampler, in.uv);
    
    let albedo = color_sample.rgb;
    let emission = color_sample.a;  // 0-1 normalized emission
    
    // Normal is stored directly as world-space normal (-1 to +1)
    // NO encoding/decoding needed - just normalize to handle interpolation
    let world_normal = normalize(normal_sample.rgb);
    
    let world_pos = position_sample.xyz;
    let depth = position_sample.w;
    
    // Sample GTAO with edge-aware bilateral blur
    // This reduces noise at edges while preserving depth discontinuities
    let ao = sample_gtao_with_blur(in.uv, depth);
    
    // Debug: Show g-buffer normal as color (remap -1,1 to 0,1 for visualization)
    if DEBUG_MODE == 1 {
        return vec4<f32>(world_normal * 0.5 + 0.5, 1.0);
    }
    
    // Debug: Show depth
    if DEBUG_MODE == 2 {
        let d = clamp(depth / 50.0, 0.0, 1.0);
        return vec4<f32>(d, d, d, 1.0);
    }
    
    // Debug: Show albedo only (no lighting)
    if DEBUG_MODE == 3 {
        return vec4<f32>(albedo, 1.0);
    }
    
    // Skip pixels with no geometry (far depth = 1000)
    // These should show fog color
    if depth > 999.0 {
        return vec4<f32>(FOG_COLOR, 1.0);
    }
    
    // --- Shadow Calculation ---
    // Calculate shadows for both moons independently
    let moon1_shadow = calculate_moon1_shadow(world_pos, world_normal);
    let moon2_shadow = calculate_moon2_shadow(world_pos, world_normal);
    
    // Legacy shadow_factor for compatibility
    let shadow_factor = moon1_shadow;
    
    // Debug: Show shadow only (moon1 in red channel, moon2 in green)
    if DEBUG_MODE == 4 {
        return vec4<f32>(moon1_shadow, moon2_shadow, 0.0, 1.0);
    }
    
    // Debug: Show moon1 shadow only (grayscale)
    if DEBUG_MODE == 41 {
        return vec4<f32>(moon1_shadow, moon1_shadow, moon1_shadow, 1.0);
    }
    
    // Debug: Show moon2 shadow only (grayscale)
    if DEBUG_MODE == 42 {
        return vec4<f32>(moon2_shadow, moon2_shadow, moon2_shadow, 1.0);
    }
    
    // Debug: Show where BOTH shadows overlap (blue = both shadowed)
    if DEBUG_MODE == 43 {
        let both_shadowed = (1.0 - moon1_shadow) * (1.0 - moon2_shadow);
        return vec4<f32>(moon1_shadow, moon2_shadow, both_shadowed, 1.0);
    }
    
    // Debug: Show AO only (now shows GTAO)
    if DEBUG_MODE == 5 {
        return vec4<f32>(vec3<f32>(ao), 1.0);
    }
    
    // Debug: Show raw GTAO texture (after blur)
    if DEBUG_MODE == 100 {
        return vec4<f32>(vec3<f32>(ao), 1.0);
    }
    
    // Debug: Show raw GTAO center sample (no blur) - to debug GTAO output directly
    if DEBUG_MODE == 101 {
        let raw_ao = textureSample(gtao_texture, gtao_sampler, in.uv).r;
        // Check for NaN or invalid values
        if (raw_ao != raw_ao) { // NaN check
            return vec4<f32>(1.0, 0.0, 1.0, 1.0); // Magenta = NaN
        }
        if (raw_ao < 0.0) {
            return vec4<f32>(1.0, 0.0, 0.0, 1.0); // Red = negative
        }
        if (raw_ao > 1.0) {
            return vec4<f32>(0.0, 0.0, 1.0, 1.0); // Blue = > 1.0
        }
        return vec4<f32>(vec3<f32>(raw_ao), 1.0);
    }
    
    // Debug 102: Detailed point shadow breakdown
    // R = stored_depth, G = compare_depth, B = shadow result
    // Helps diagnose why spurious shadows appear
    if DEBUG_MODE == 102 {
        if point_lights.count.x > 0u {
            let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
            let shadow_radius = point_shadow_matrices.light_pos_radius.w;
            let light_to_frag = world_pos - shadow_light_pos;
            let distance = length(light_to_frag);
            
            if distance > shadow_radius {
                return vec4<f32>(0.0, 1.0, 0.0, 1.0); // Green = outside radius
            }
            
            let abs_vec = abs(light_to_frag);
            var face_idx: i32;
            if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
                face_idx = select(1, 0, light_to_frag.x > 0.0);
            } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
                face_idx = select(3, 2, light_to_frag.y > 0.0);
            } else {
                face_idx = select(5, 4, light_to_frag.z > 0.0);
            }
            
            let view_proj = point_shadow_matrices.face_matrices[face_idx];
            let clip = view_proj * vec4<f32>(world_pos, 1.0);
            let ndc = clip.xyz / clip.w;
            let face_uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
            
            // Check UV bounds
            if face_uv.x < 0.0 || face_uv.x > 1.0 || face_uv.y < 0.0 || face_uv.y > 1.0 {
                return vec4<f32>(1.0, 1.0, 0.0, 1.0); // Yellow = UV out of bounds
            }
            
            let tex_coord = vec2<i32>(face_uv * 512.0);
            var stored_depth: f32;
            switch face_idx {
                case 0: { stored_depth = textureLoad(point_shadow_face_px, tex_coord, 0); }
                case 1: { stored_depth = textureLoad(point_shadow_face_nx, tex_coord, 0); }
                case 2: { stored_depth = textureLoad(point_shadow_face_py, tex_coord, 0); }
                case 3: { stored_depth = textureLoad(point_shadow_face_ny, tex_coord, 0); }
                case 4: { stored_depth = textureLoad(point_shadow_face_pz, tex_coord, 0); }
                case 5: { stored_depth = textureLoad(point_shadow_face_nz, tex_coord, 0); }
                default: { stored_depth = 1.0; }
            }
            
            let compare_depth = distance / shadow_radius;
            
            // R = stored (darker = closer geometry in shadow map)
            // G = compare (darker = we're closer to light)
            // B = 0.5 if near boundary (|stored - compare| < 0.1)
            let near_boundary = select(0.0, 0.5, abs(stored_depth - compare_depth) < 0.1);
            return vec4<f32>(stored_depth, compare_depth, near_boundary, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0); // Blue = no lights
    }
    
    // Debug 103: Check clip.w and NDC validity
    if DEBUG_MODE == 103 {
        if point_lights.count.x > 0u {
            let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
            let shadow_radius = point_shadow_matrices.light_pos_radius.w;
            let light_to_frag = world_pos - shadow_light_pos;
            let distance = length(light_to_frag);
            
            if distance > shadow_radius {
                return vec4<f32>(0.5, 0.5, 0.5, 1.0); // Gray = outside radius
            }
            
            let abs_vec = abs(light_to_frag);
            var face_idx: i32;
            if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
                face_idx = select(1, 0, light_to_frag.x > 0.0);
            } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
                face_idx = select(3, 2, light_to_frag.y > 0.0);
            } else {
                face_idx = select(5, 4, light_to_frag.z > 0.0);
            }
            
            let view_proj = point_shadow_matrices.face_matrices[face_idx];
            let clip = view_proj * vec4<f32>(world_pos, 1.0);
            
            // R = clip.w (should be positive for points in front of camera)
            // G = clip.z / clip.w (NDC z, should be in [0,1] for valid depth)
            // B = face_idx / 6
            let ndc_z = clip.z / clip.w;
            let clip_w_norm = clamp(clip.w / shadow_radius, 0.0, 1.0);
            return vec4<f32>(clip_w_norm, clamp(ndc_z, 0.0, 1.0), f32(face_idx) / 5.0, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);
    }
    
    // Debug: Show world position XZ (scaled to 0-1 range for -20 to +20)
    if DEBUG_MODE == 7 {
        let x_norm = (world_pos.x + 20.0) / 40.0;
        let z_norm = (world_pos.z + 20.0) / 40.0;
        return vec4<f32>(x_norm, 0.0, z_norm, 1.0);
    }
    
    // Debug: Show light count as grayscale
    if DEBUG_MODE == 8 {
        let count = f32(point_lights.count.x) / 32.0;  // Normalize to 0-1 for ~32 lights
        return vec4<f32>(count, count, count, 1.0);
    }
    
    // Debug: Show distance to first light (should be gradient around light position)
    if DEBUG_MODE == 9 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let dist = distance(world_pos, light.position.xyz);
            let d = 1.0 - clamp(dist / 30.0, 0.0, 1.0);  // Visualize distance up to 30 units
            return vec4<f32>(d, d * 0.5, 0.0, 1.0);  // Orange gradient
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);  // Blue if no lights
    }
    
    // Debug: Show first light's color directly (should be the light's RGB)
    if DEBUG_MODE == 10 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            // Show the color_intensity RGB directly - if data is correct, should be the light color
            return vec4<f32>(light.color_intensity.rgb, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);  // Blue if no lights
    }
    
    // Debug: Show first light's radius as grayscale (expecting ~15.0 for test light)
    if DEBUG_MODE == 11 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            // Show radius normalized - expect 15/20 = 0.75 grayscale
            let r = light.radius_padding.x / 20.0;
            return vec4<f32>(r, r, r, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);  // Blue if no lights
    }
    
    // Debug: Show falloff value for first light (inline calculation)
    if DEBUG_MODE == 12 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_pos = light.position.xyz;
            let radius = light.radius_padding.x;
            let to_light = light_pos - world_pos;
            let distance = length(to_light);
            
            // Show: red = distance/30, green = in-range flag, blue = falloff
            let in_range = select(0.0, 1.0, distance <= radius);
            let falloff = max(0.0, 1.0 - (distance / radius));
            
            return vec4<f32>(distance / 30.0, in_range, falloff, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);
    }
    
    // Debug: Show light[0] position XYZ directly
    if DEBUG_MODE == 13 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            // Normalize position for display: expect (0, 5, 0) -> (0.5, 0.25, 0.5) with ±20 range
            let pos = light.position.xyz;
            return vec4<f32>(
                (pos.x + 20.0) / 40.0,  // R: x position
                pos.y / 20.0,            // G: y position (0-20)
                (pos.z + 20.0) / 40.0,  // B: z position
                1.0
            );
        }
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);  // Magenta if no lights
    }
    
    // Debug: Show point shadow for light 0
    if DEBUG_MODE == 20 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let shadow = calculate_point_shadow(light.position.xyz, world_pos, light.radius_padding.x);
            // Green = lit, Red = shadowed
            return vec4<f32>(1.0 - shadow, shadow, 0.0, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);  // Blue if no lights
    }
    
    // Debug: Show matrix-based UV for -Y face (ground)
    if DEBUG_MODE == 50 {
        let light_pos = point_shadow_matrices.light_pos_radius.xyz;
        let light_to_frag = world_pos - light_pos;
        let abs_vec = abs(light_to_frag);
        
        // Only for -Y face
        if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z && light_to_frag.y < 0.0 {
            // Use matrix directly
            let view_proj = point_shadow_matrices.face_matrices[3]; // -Y face
            let clip = view_proj * vec4<f32>(world_pos, 1.0);
            let ndc = clip.xyz / clip.w;
            let face_uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
            
            // R = U, G = V from matrix
            return vec4<f32>(face_uv.x, face_uv.y, 0.0, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 0.5, 1.0); // Blue if not -Y
    }
    
    // Debug: Compare matrix UV vs manual UV
    if DEBUG_MODE == 51 {
        let light_pos = point_shadow_matrices.light_pos_radius.xyz;
        let radius = point_shadow_matrices.light_pos_radius.w;
        let light_to_frag = world_pos - light_pos;
        let abs_vec = abs(light_to_frag);
        
        // Only for -Y face
        if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z && light_to_frag.y < 0.0 {
            // Matrix-based UV
            let view_proj = point_shadow_matrices.face_matrices[3];
            let clip = view_proj * vec4<f32>(world_pos, 1.0);
            let ndc = clip.xyz / clip.w;
            let matrix_uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
            
            // Old manual UV (from the original formula)
            let manual_uv = vec2<f32>(
                light_to_frag.x / abs_vec.y * 0.5 + 0.5,
                light_to_frag.z / abs_vec.y * 0.5 + 0.5
            );
            
            // Show difference: if they match, should be black
            let diff = abs(matrix_uv - manual_uv);
            return vec4<f32>(diff.x * 10.0, diff.y * 10.0, 0.0, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 0.5, 1.0);
    }
    
    // Debug: Show raw stored depth vs our compare depth
    if DEBUG_MODE == 52 {
        let light_pos = point_shadow_matrices.light_pos_radius.xyz;
        let radius = point_shadow_matrices.light_pos_radius.w;
        let light_to_frag = world_pos - light_pos;
        let distance = length(light_to_frag);
        let abs_vec = abs(light_to_frag);
        
        if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z && light_to_frag.y < 0.0 {
            let view_proj = point_shadow_matrices.face_matrices[3];
            let clip = view_proj * vec4<f32>(world_pos, 1.0);
            let ndc = clip.xyz / clip.w;
            let face_uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
            
            let tex_coord = vec2<i32>(face_uv * 512.0);
            let stored_depth = textureLoad(point_shadow_face_ny, tex_coord, 0);
            let compare_depth = distance / radius;
            
            // R = stored, G = compare, B = lit test (1 if stored >= compare)
            let is_lit = select(0.0, 1.0, stored_depth >= compare_depth - 0.02);
            return vec4<f32>(stored_depth, compare_depth, is_lit, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 0.5, 1.0);
    }
    
    // Debug: Sample -Y face shadow map directly (this is the floor-facing direction)
    if DEBUG_MODE == 21 {
        // Sample at a fixed UV, show the depth value
        let sample_uv = in.uv;  // Use screen UV as sample coord
        let depth = textureSampleCompare(point_shadow_face_ny, point_shadow_sampler, sample_uv, 0.5);
        return vec4<f32>(depth, depth, depth, 1.0);
    }
    
    // Debug: Show which cube face would be selected for each fragment
    // Uses point_shadow_matrices.light_pos_radius to match calculate_point_shadow
    if DEBUG_MODE == 23 {
        if point_lights.count.x > 0u {
            // Use shadow light position, not point_lights[0]
            let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
            let light_to_frag = world_pos - shadow_light_pos;
            let abs_vec = abs(light_to_frag);
            
            // Color code: R=X faces, G=Y faces, B=Z faces
            // Bright = positive, Dark = negative
            if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
                if light_to_frag.x > 0.0 {
                    return vec4<f32>(1.0, 0.0, 0.0, 1.0); // +X = bright red
                } else {
                    return vec4<f32>(0.5, 0.0, 0.0, 1.0); // -X = dark red
                }
            } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
                if light_to_frag.y > 0.0 {
                    return vec4<f32>(0.0, 1.0, 0.0, 1.0); // +Y = bright green
                } else {
                    return vec4<f32>(0.0, 0.5, 0.0, 1.0); // -Y = dark green
                }
            } else {
                if light_to_frag.z > 0.0 {
                    return vec4<f32>(0.0, 0.0, 1.0, 1.0); // +Z = bright blue
                } else {
                    return vec4<f32>(0.0, 0.0, 0.5, 1.0); // -Z = dark blue
                }
            }
        }
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    
    // Debug: Show compare_depth (R) - this is what we're testing against
    if DEBUG_MODE == 24 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_to_frag = world_pos - light.position.xyz;
            let distance = length(light_to_frag);
            let radius = light.radius_padding.x;
            let compare_depth = distance / radius;
            // Red channel = compare depth (0-1)
            // Should see gradient from center (low ~0.3) to edges (higher)
            return vec4<f32>(compare_depth, compare_depth, compare_depth, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);
    }
    
    // Debug: Show UV coords that would be used for -Y face sampling
    if DEBUG_MODE == 25 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_to_frag = world_pos - light.position.xyz;
            let abs_vec = abs(light_to_frag);
            
            // Only show for -Y dominant direction (ground looking up at light)
            if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z && light_to_frag.y < 0.0 {
                // -Y face UV calculation
                let face_uv = vec2<f32>(
                    light_to_frag.x / abs_vec.y * 0.5 + 0.5,
                    -light_to_frag.z / abs_vec.y * 0.5 + 0.5
                );
                return vec4<f32>(face_uv.x, face_uv.y, 0.0, 1.0);
            } else {
                // Not -Y face - show blue
                return vec4<f32>(0.0, 0.0, 0.5, 1.0);
            }
        }
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    
    // Debug: Test if rendering to shadow faces actually works
    // Shadow shader writes 0.25 to all faces. Sampler uses GreaterEqual.
    // Compare with 0.3: written faces (0.25 >= 0.3) = 0, cleared faces (1.0 >= 0.3) = 1
    // Expected if rendering works: value = 0 (black)
    // Expected if rendering broken: value = 1 (white)
    if DEBUG_MODE == 26 {
        let compare_val = 0.3;  // Just above 0.25 that shader writes
        let px = textureSampleCompare(point_shadow_face_px, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        let nx = textureSampleCompare(point_shadow_face_nx, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        let py = textureSampleCompare(point_shadow_face_py, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        let ny = textureSampleCompare(point_shadow_face_ny, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        let pz = textureSampleCompare(point_shadow_face_pz, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        let nz = textureSampleCompare(point_shadow_face_nz, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        // Show all 6 faces encoded: R = X faces avg, G = Y faces avg, B = Z faces avg
        // Broken face will push its channel toward 0.5 (one of two)
        // R: +X=0, -X=0 -> 0.0 if both work. +X=1, -X=0 -> 0.5 if +X broken.
        return vec4<f32>((px + nx) * 0.5, (py + ny) * 0.5, (pz + nz) * 0.5, 1.0);
    }
    
    // Debug mode 28: Use textureLoad to read raw depth values (no sampler)
    // This bypasses the comparison sampler to see actual depth values
    if DEBUG_MODE == 28 {
        let center = vec2<i32>(256, 256);  // Center of 512x512 texture
        let px_raw = textureLoad(point_shadow_face_px, center, 0);
        let nx_raw = textureLoad(point_shadow_face_nx, center, 0);
        let py_raw = textureLoad(point_shadow_face_py, center, 0);
        let ny_raw = textureLoad(point_shadow_face_ny, center, 0);
        let pz_raw = textureLoad(point_shadow_face_pz, center, 0);
        let nz_raw = textureLoad(point_shadow_face_nz, center, 0);
        // Expected: px=0.1, nx=0.2, py=0.3, ny=0.4, pz=0.5, nz=0.6
        // Show R=face0(+X), G=face3(-Y), B=face5(-Z)
        return vec4<f32>(px_raw, ny_raw, nz_raw, 1.0);
    }
    
    // Debug mode 29: Show all 6 faces as strips using textureLoad
    if DEBUG_MODE == 29 {
        let center = vec2<i32>(256, 256);
        let strip = i32(in.uv.x * 6.0);
        var depth = 0.0;
        if strip == 0 {
            depth = textureLoad(point_shadow_face_px, center, 0);
        } else if strip == 1 {
            depth = textureLoad(point_shadow_face_nx, center, 0);
        } else if strip == 2 {
            depth = textureLoad(point_shadow_face_py, center, 0);
        } else if strip == 3 {
            depth = textureLoad(point_shadow_face_ny, center, 0);
        } else if strip == 4 {
            depth = textureLoad(point_shadow_face_pz, center, 0);
        } else {
            depth = textureLoad(point_shadow_face_nz, center, 0);
        }
        // Show raw depth: 0.1=dark, 0.6=brighter
        return vec4<f32>(depth, depth, depth, 1.0);
    }
    
    // Debug mode 31: Show raw shadow map depth vs compare_depth using correct face selection
    if DEBUG_MODE == 31 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_pos = light.position.xyz;
            let radius = light.radius_padding.x;
            
            let light_to_frag = world_pos - light_pos;
            let distance = length(light_to_frag);
            let compare_depth = (distance / radius);
            
            let abs_vec = abs(light_to_frag);
            var stored_depth = 0.0;
            var face_uv: vec2<f32>;
            
            // Select correct face and compute UV (same logic as calculate_point_shadow)
            if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
                if light_to_frag.x > 0.0 {
                    face_uv = vec2<f32>(light_to_frag.z / abs_vec.x * 0.5 + 0.5, light_to_frag.y / abs_vec.x * 0.5 + 0.5);
                    stored_depth = textureLoad(point_shadow_face_px, vec2<i32>(face_uv * 512.0), 0);
                } else {
                    face_uv = vec2<f32>(-light_to_frag.z / abs_vec.x * 0.5 + 0.5, light_to_frag.y / abs_vec.x * 0.5 + 0.5);
                    stored_depth = textureLoad(point_shadow_face_nx, vec2<i32>(face_uv * 512.0), 0);
                }
            } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
                if light_to_frag.y > 0.0 {
                    face_uv = vec2<f32>(light_to_frag.x / abs_vec.y * 0.5 + 0.5, -light_to_frag.z / abs_vec.y * 0.5 + 0.5);
                    stored_depth = textureLoad(point_shadow_face_py, vec2<i32>(face_uv * 512.0), 0);
                } else {
                    face_uv = vec2<f32>(light_to_frag.x / abs_vec.y * 0.5 + 0.5, light_to_frag.z / abs_vec.y * 0.5 + 0.5);
                    stored_depth = textureLoad(point_shadow_face_ny, vec2<i32>(face_uv * 512.0), 0);
                }
            } else {
                if light_to_frag.z > 0.0 {
                    face_uv = vec2<f32>(light_to_frag.x / abs_vec.z * 0.5 + 0.5, light_to_frag.y / abs_vec.z * 0.5 + 0.5);
                    stored_depth = textureLoad(point_shadow_face_pz, vec2<i32>(face_uv * 512.0), 0);
                } else {
                    face_uv = vec2<f32>(-light_to_frag.x / abs_vec.z * 0.5 + 0.5, light_to_frag.y / abs_vec.z * 0.5 + 0.5);
                    stored_depth = textureLoad(point_shadow_face_nz, vec2<i32>(face_uv * 512.0), 0);
                }
            }
            
            // R = stored depth, G = compare depth, B = lit test
            let is_lit = select(0.0, 1.0, stored_depth >= compare_depth);
            return vec4<f32>(stored_depth, compare_depth, is_lit, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);
    }
    
    // Debug mode 34: Show raw -Y shadow map contents (not sampled by world pos, just display the texture)
    if DEBUG_MODE == 34 {
        // Map screen UV directly to shadow map UV to see what's actually stored
        let shadow_uv = in.uv;
        let tex_coord = vec2<i32>(shadow_uv * 512.0);
        let raw_depth = textureLoad(point_shadow_face_ny, tex_coord, 0);
        // Show depth: 0.0 = black (close), 1.0 = white (far/empty)
        return vec4<f32>(raw_depth, raw_depth, raw_depth, 1.0);
    }
    
    // Debug mode 63: Show raw +X shadow map contents
    if DEBUG_MODE == 63 {
        let shadow_uv = in.uv;
        let tex_coord = vec2<i32>(shadow_uv * 512.0);
        let raw_depth = textureLoad(point_shadow_face_px, tex_coord, 0);
        return vec4<f32>(raw_depth, raw_depth, raw_depth, 1.0);
    }
    
    // Debug mode 64: Show raw +Z shadow map contents
    if DEBUG_MODE == 64 {
        let shadow_uv = in.uv;
        let tex_coord = vec2<i32>(shadow_uv * 512.0);
        let raw_depth = textureLoad(point_shadow_face_pz, tex_coord, 0);
        return vec4<f32>(raw_depth, raw_depth, raw_depth, 1.0);
    }
    
    // Debug mode 80: COMPREHENSIVE shadow debug
    // Shows stored_depth (R), compare_depth (G), and difference (B)
    // for the ACTUAL face that would be sampled
    if DEBUG_MODE == 80 {
        let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
        let shadow_radius = point_shadow_matrices.light_pos_radius.w;
        let light_to_frag = world_pos - shadow_light_pos;
        let distance = length(light_to_frag);
        let abs_vec = abs(light_to_frag);
        
        // Select face (same logic as calculate_point_shadow)
        var face_idx: i32;
        if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
            face_idx = select(1, 0, light_to_frag.x > 0.0);
        } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
            face_idx = select(3, 2, light_to_frag.y > 0.0);
        } else {
            face_idx = select(5, 4, light_to_frag.z > 0.0);
        }
        
        // Compute UV
        let face_uv = compute_cube_face_uv(light_to_frag, face_idx);
        let tex_coord = vec2<i32>(face_uv * POINT_SHADOW_MAP_SIZE);
        
        // Load raw depth from correct face
        var stored_depth: f32;
        switch face_idx {
            case 0: { stored_depth = textureLoad(point_shadow_face_px, tex_coord, 0); }
            case 1: { stored_depth = textureLoad(point_shadow_face_nx, tex_coord, 0); }
            case 2: { stored_depth = textureLoad(point_shadow_face_py, tex_coord, 0); }
            case 3: { stored_depth = textureLoad(point_shadow_face_ny, tex_coord, 0); }
            case 4: { stored_depth = textureLoad(point_shadow_face_pz, tex_coord, 0); }
            case 5: { stored_depth = textureLoad(point_shadow_face_nz, tex_coord, 0); }
            default: { stored_depth = 1.0; }
        }
        
        // Compare depth (what we're testing against)
        let compare_depth = distance / shadow_radius;
        
        // R = stored depth from shadow map
        // G = compare depth (computed from fragment distance)
        // B = 1.0 if stored >= compare (lit), 0.0 if shadowed
        let is_lit = select(0.0, 1.0, stored_depth >= compare_depth - 0.02);
        return vec4<f32>(stored_depth, compare_depth, is_lit, 1.0);
    }
    
    // Debug mode 81: Show just the shadow light position info
    if DEBUG_MODE == 81 {
        let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
        let shadow_radius = point_shadow_matrices.light_pos_radius.w;
        
        // Show distance from shadow light as gradient
        let dist = distance(world_pos, shadow_light_pos);
        let normalized_dist = clamp(dist / shadow_radius, 0.0, 1.0);
        
        // Also show if we have valid light position (should not be 0,0,0)
        let has_valid_pos = select(0.0, 1.0, length(shadow_light_pos) > 0.1);
        
        return vec4<f32>(normalized_dist, has_valid_pos, shadow_radius / 50.0, 1.0);
    }
    
    // Debug mode 82: Show computed UV coordinates
    // R = U, G = V, B = face_idx / 6
    if DEBUG_MODE == 82 {
        let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
        let light_to_frag = world_pos - shadow_light_pos;
        let abs_vec = abs(light_to_frag);
        
        var face_idx: i32;
        if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
            face_idx = select(1, 0, light_to_frag.x > 0.0);
        } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
            face_idx = select(3, 2, light_to_frag.y > 0.0);
        } else {
            face_idx = select(5, 4, light_to_frag.z > 0.0);
        }
        
        let face_uv = compute_cube_face_uv(light_to_frag, face_idx);
        
        // R = U, G = V, B = face_idx normalized
        return vec4<f32>(face_uv.x, face_uv.y, f32(face_idx) / 5.0, 1.0);
    }
    
    // Debug mode 83: Compare manual UV vs matrix-based UV
    // Shows the DIFFERENCE - should be near black if they match
    if DEBUG_MODE == 83 {
        let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
        let light_to_frag = world_pos - shadow_light_pos;
        let abs_vec = abs(light_to_frag);
        
        var face_idx: i32;
        if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
            face_idx = select(1, 0, light_to_frag.x > 0.0);
        } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
            face_idx = select(3, 2, light_to_frag.y > 0.0);
        } else {
            face_idx = select(5, 4, light_to_frag.z > 0.0);
        }
        
        // Manual UV from compute_cube_face_uv
        let manual_uv = compute_cube_face_uv(light_to_frag, face_idx);
        
        // Matrix-based UV from view_proj transform
        let view_proj = point_shadow_matrices.face_matrices[face_idx];
        let clip = view_proj * vec4<f32>(world_pos, 1.0);
        let ndc = clip.xyz / clip.w;
        // Standard NDC to UV: x: [-1,1] -> [0,1], y: [-1,1] -> [1,0] (flip Y)
        let matrix_uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
        
        // Show difference scaled up 10x
        let diff = abs(manual_uv - matrix_uv) * 10.0;
        
        // R = U difference, G = V difference, B = face_idx
        return vec4<f32>(diff.x, diff.y, f32(face_idx) / 5.0, 1.0);
    }
    
    // Debug mode 35: Overlay - show shadow map with computed sample UV as crosshair
    if DEBUG_MODE == 35 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_pos = light.position.xyz;
            let light_to_frag = world_pos - light_pos;
            let abs_vec = abs(light_to_frag);
            
            // Show shadow map for -Y face
            let shadow_uv = in.uv;
            let tex_coord = vec2<i32>(shadow_uv * 512.0);
            let raw_depth = textureLoad(point_shadow_face_ny, tex_coord, 0);
            
            // Compute where THIS fragment would sample from
            var sample_uv = vec2<f32>(0.5, 0.5);
            if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z && light_to_frag.y < 0.0 {
                sample_uv = vec2<f32>(
                    light_to_frag.x / abs_vec.y * 0.5 + 0.5,
                    light_to_frag.z / abs_vec.y * 0.5 + 0.5
                );
            }
            
            // Draw crosshair at sample UV location
            let dist_to_sample = length(in.uv - sample_uv);
            if dist_to_sample < 0.01 {
                return vec4<f32>(1.0, 0.0, 0.0, 1.0); // Red dot at sample location
            }
            
            return vec4<f32>(raw_depth, raw_depth, raw_depth, 1.0);
        }
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    
    // Debug mode 36: Sample shadow map using world position UV, show as color on geometry
    // This shows where each ground point samples FROM - if correct, pillar shapes should appear
    // at the pillar BASE positions on the ground
    if DEBUG_MODE == 36 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_pos = light.position.xyz;
            let light_to_frag = world_pos - light_pos;
            let abs_vec = abs(light_to_frag);
            
            // Only for -Y face (ground)
            if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z && light_to_frag.y < 0.0 {
                let face_uv = vec2<f32>(
                    light_to_frag.x / abs_vec.y * 0.5 + 0.5,
                    light_to_frag.z / abs_vec.y * 0.5 + 0.5
                );
                let tex_coord = vec2<i32>(face_uv * 512.0);
                let stored_depth = textureLoad(point_shadow_face_ny, tex_coord, 0);
                
                // Compare with our depth
                let radius = light.radius_padding.x;
                let distance = length(light_to_frag);
                let compare_depth = distance / radius;
                
                // If stored < compare, we're in shadow (something closer blocks us)
                if stored_depth < compare_depth - 0.02 {
                    return vec4<f32>(0.2, 0.0, 0.0, 1.0); // Dark red = shadow
                } else {
                    return vec4<f32>(0.0, 0.5, 0.0, 1.0); // Green = lit
                }
            }
            return vec4<f32>(0.0, 0.0, 0.3, 1.0); // Blue = not -Y face
        }
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    
    // Debug mode 37: Same as 36 but with V flipped to test coordinate system
    if DEBUG_MODE == 37 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_pos = light.position.xyz;
            let light_to_frag = world_pos - light_pos;
            let abs_vec = abs(light_to_frag);
            
            // Only for -Y face (ground)
            if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z && light_to_frag.y < 0.0 {
                let face_uv = vec2<f32>(
                    light_to_frag.x / abs_vec.y * 0.5 + 0.5,
                    1.0 - (light_to_frag.z / abs_vec.y * 0.5 + 0.5)  // FLIP V
                );
                let tex_coord = vec2<i32>(face_uv * 512.0);
                let stored_depth = textureLoad(point_shadow_face_ny, tex_coord, 0);
                
                // Compare with our depth
                let radius = light.radius_padding.x;
                let distance = length(light_to_frag);
                let compare_depth = distance / radius;
                
                // If stored < compare, we're in shadow (something closer blocks us)
                if stored_depth < compare_depth - 0.02 {
                    return vec4<f32>(0.2, 0.0, 0.0, 1.0); // Dark red = shadow
                } else {
                    return vec4<f32>(0.0, 0.5, 0.0, 1.0); // Green = lit
                }
            }
            return vec4<f32>(0.0, 0.0, 0.3, 1.0); // Blue = not -Y face
        }
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    
    // Debug mode 38: Try both U and V flipped
    if DEBUG_MODE == 38 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_pos = light.position.xyz;
            let light_to_frag = world_pos - light_pos;
            let abs_vec = abs(light_to_frag);
            
            // Only for -Y face (ground)
            if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z && light_to_frag.y < 0.0 {
                let face_uv = vec2<f32>(
                    1.0 - (light_to_frag.x / abs_vec.y * 0.5 + 0.5),  // FLIP U
                    1.0 - (light_to_frag.z / abs_vec.y * 0.5 + 0.5)   // FLIP V
                );
                let tex_coord = vec2<i32>(face_uv * 512.0);
                let stored_depth = textureLoad(point_shadow_face_ny, tex_coord, 0);
                
                // Compare with our depth
                let radius = light.radius_padding.x;
                let distance = length(light_to_frag);
                let compare_depth = distance / radius;
                
                // If stored < compare, we're in shadow (something closer blocks us)
                if stored_depth < compare_depth - 0.02 {
                    return vec4<f32>(0.2, 0.0, 0.0, 1.0); // Dark red = shadow
                } else {
                    return vec4<f32>(0.0, 0.5, 0.0, 1.0); // Green = lit
                }
            }
            return vec4<f32>(0.0, 0.0, 0.3, 1.0); // Blue = not -Y face
        }
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    
    // Debug mode 39: Only flip U
    if DEBUG_MODE == 39 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_pos = light.position.xyz;
            let light_to_frag = world_pos - light_pos;
            let abs_vec = abs(light_to_frag);
            
            // Only for -Y face (ground)
            if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z && light_to_frag.y < 0.0 {
                let face_uv = vec2<f32>(
                    1.0 - (light_to_frag.x / abs_vec.y * 0.5 + 0.5),  // FLIP U only
                    light_to_frag.z / abs_vec.y * 0.5 + 0.5
                );
                let tex_coord = vec2<i32>(face_uv * 512.0);
                let stored_depth = textureLoad(point_shadow_face_ny, tex_coord, 0);
                
                // Compare with our depth
                let radius = light.radius_padding.x;
                let distance = length(light_to_frag);
                let compare_depth = distance / radius;
                
                // If stored < compare, we're in shadow (something closer blocks us)
                if stored_depth < compare_depth - 0.02 {
                    return vec4<f32>(0.2, 0.0, 0.0, 1.0); // Dark red = shadow
                } else {
                    return vec4<f32>(0.0, 0.5, 0.0, 1.0); // Green = lit
                }
            }
            return vec4<f32>(0.0, 0.0, 0.3, 1.0); // Blue = not -Y face
        }
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    
    // Debug mode 40: Show world XZ position to verify pillar locations
    if DEBUG_MODE == 40 {
        // R = X position (-8 to +8 mapped to 0-1), G = Z position, B = Y position
        let x_norm = (world_pos.x + 8.0) / 16.0;
        let z_norm = (world_pos.z + 8.0) / 16.0;
        let y_norm = world_pos.y / 8.0;
        return vec4<f32>(clamp(x_norm, 0.0, 1.0), clamp(z_norm, 0.0, 1.0), clamp(y_norm, 0.0, 1.0), 1.0);
    }
    
    // Debug mode 41: Show shadow map with markers at expected pillar positions
    // Pillar 1: world (-2, 1-4, 2-3) -> sample UV calc: (-2/6*0.5+0.5, 2.5/6*0.5+0.5) = (0.33, 0.71)
    // Pillar 2: world (2.5, 1-2, -3.5) -> sample UV calc: (2.5/6*0.5+0.5, -3.5/6*0.5+0.5) = (0.71, 0.21)
    if DEBUG_MODE == 41 {
        let tex_uv = in.uv;
        let tex_coord = vec2<i32>(tex_uv * 512.0);
        let raw_depth = textureLoad(point_shadow_face_ny, tex_coord, 0);
        
        // Expected pillar positions (where sampling shader would look)
        let p1_uv = vec2<f32>(0.33, 0.71);  // Pillar 1
        let p2_uv = vec2<f32>(0.71, 0.21);  // Pillar 2
        
        // Draw markers
        if length(tex_uv - p1_uv) < 0.02 {
            return vec4<f32>(1.0, 0.0, 0.0, 1.0);  // Red for pillar 1 expected
        }
        if length(tex_uv - p2_uv) < 0.02 {
            return vec4<f32>(0.0, 0.0, 1.0, 1.0);  // Blue for pillar 2 expected
        }
        
        // Center marker
        if length(tex_uv - vec2<f32>(0.5, 0.5)) < 0.01 {
            return vec4<f32>(1.0, 1.0, 0.0, 1.0);  // Yellow for center
        }
        
        return vec4<f32>(raw_depth, raw_depth, raw_depth, 1.0);
    }
    
    // Debug mode 33: Show light_to_frag vector components for ground
    if DEBUG_MODE == 33 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_pos = light.position.xyz;
            let light_to_frag = world_pos - light_pos;
            
            // Normalize to visible range: expect values -10 to +10, map to 0-1
            let ltf_norm = (light_to_frag + 10.0) / 20.0;
            return vec4<f32>(ltf_norm.x, ltf_norm.y, ltf_norm.z, 1.0);
        }
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    
    // Debug mode 32: Show computed face_uv for -Y face (ground)
    if DEBUG_MODE == 32 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_pos = light.position.xyz;
            let light_to_frag = world_pos - light_pos;
            let abs_vec = abs(light_to_frag);
            
            // Only show -Y face (ground looking up at light)
            if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z && light_to_frag.y < 0.0 {
                // Current -Y face UV calculation
                let face_uv = vec2<f32>(
                    light_to_frag.x / abs_vec.y * 0.5 + 0.5,
                    light_to_frag.z / abs_vec.y * 0.5 + 0.5
                );
                // R = U, G = V, B = 0
                return vec4<f32>(face_uv.x, face_uv.y, 0.0, 1.0);
            } else {
                // Not -Y face - show blue
                return vec4<f32>(0.0, 0.0, 0.5, 1.0);
            }
        }
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    
    // Debug mode 61: Simple shadow depth comparison - just show stored vs compare
    if DEBUG_MODE == 61 {
        let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
        let shadow_radius = point_shadow_matrices.light_pos_radius.w;
        let light_to_frag = world_pos - shadow_light_pos;
        let distance = length(light_to_frag);
        let abs_vec = abs(light_to_frag);
        
        var face_idx: i32;
        if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
            face_idx = select(1, 0, light_to_frag.x > 0.0);
        } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
            face_idx = select(3, 2, light_to_frag.y > 0.0);
        } else {
            face_idx = select(5, 4, light_to_frag.z > 0.0);
        }
        
        let view_proj = point_shadow_matrices.face_matrices[face_idx];
        let clip = view_proj * vec4<f32>(world_pos, 1.0);
        let ndc = clip.xyz / clip.w;
        let face_uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
        
        let tex_coord = vec2<i32>(face_uv * 512.0);
        var stored_depth: f32;
        switch face_idx {
            case 0: { stored_depth = textureLoad(point_shadow_face_px, tex_coord, 0); }
            case 1: { stored_depth = textureLoad(point_shadow_face_nx, tex_coord, 0); }
            case 2: { stored_depth = textureLoad(point_shadow_face_py, tex_coord, 0); }
            case 3: { stored_depth = textureLoad(point_shadow_face_ny, tex_coord, 0); }
            case 4: { stored_depth = textureLoad(point_shadow_face_pz, tex_coord, 0); }
            case 5: { stored_depth = textureLoad(point_shadow_face_nz, tex_coord, 0); }
            default: { stored_depth = 1.0; }
        }
        
        let compare_depth = distance / shadow_radius;
        
        // Simply show: stored - compare
        // Positive (green) = stored > compare = we're closer = LIT
        // Negative (red) = stored < compare = something blocks us = SHADOW
        let diff = stored_depth - compare_depth;
        if diff >= 0.0 {
            return vec4<f32>(0.0, diff * 5.0, 0.0, 1.0); // Green = lit
        } else {
            return vec4<f32>(-diff * 5.0, 0.0, 0.0, 1.0); // Red = shadow
        }
    }
    
    // Debug mode 62: Show raw stored depth from shadow map for each face
    // Useful to verify shadow map rendering is correct
    if DEBUG_MODE == 62 {
        let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
        let shadow_radius = point_shadow_matrices.light_pos_radius.w;
        let light_to_frag = world_pos - shadow_light_pos;
        let distance = length(light_to_frag);
        let abs_vec = abs(light_to_frag);
        
        var face_idx: i32;
        if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
            face_idx = select(1, 0, light_to_frag.x > 0.0);
        } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
            face_idx = select(3, 2, light_to_frag.y > 0.0);
        } else {
            face_idx = select(5, 4, light_to_frag.z > 0.0);
        }
        
        let view_proj = point_shadow_matrices.face_matrices[face_idx];
        let clip = view_proj * vec4<f32>(world_pos, 1.0);
        let ndc = clip.xyz / clip.w;
        let face_uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
        
        let tex_coord = vec2<i32>(face_uv * 512.0);
        var stored_depth: f32;
        switch face_idx {
            case 0: { stored_depth = textureLoad(point_shadow_face_px, tex_coord, 0); }
            case 1: { stored_depth = textureLoad(point_shadow_face_nx, tex_coord, 0); }
            case 2: { stored_depth = textureLoad(point_shadow_face_py, tex_coord, 0); }
            case 3: { stored_depth = textureLoad(point_shadow_face_ny, tex_coord, 0); }
            case 4: { stored_depth = textureLoad(point_shadow_face_pz, tex_coord, 0); }
            case 5: { stored_depth = textureLoad(point_shadow_face_nz, tex_coord, 0); }
            default: { stored_depth = 1.0; }
        }
        
        let compare_depth = distance / shadow_radius;
        
        // R = stored, G = compare, B = face index / 6
        return vec4<f32>(stored_depth, compare_depth, f32(face_idx) / 6.0, 1.0);
    }
    
    // Debug mode 60: Detailed shadow debug - show stored depth, compare depth, and result
    // R = stored depth at computed UV, G = compare depth, B = shadow result
    if DEBUG_MODE == 60 {
        let shadow_light_pos = point_shadow_matrices.light_pos_radius.xyz;
        let shadow_radius = point_shadow_matrices.light_pos_radius.w;
        let light_to_frag = world_pos - shadow_light_pos;
        let distance = length(light_to_frag);
        let abs_vec = abs(light_to_frag);
        
        // Select face (same logic as calculate_point_shadow)
        var face_idx: i32;
        if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
            face_idx = select(1, 0, light_to_frag.x > 0.0);
        } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
            face_idx = select(3, 2, light_to_frag.y > 0.0);
        } else {
            face_idx = select(5, 4, light_to_frag.z > 0.0);
        }
        
        // Get UV using matrix
        let view_proj = point_shadow_matrices.face_matrices[face_idx];
        let clip = view_proj * vec4<f32>(world_pos, 1.0);
        let ndc = clip.xyz / clip.w;
        let face_uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
        
        // Load stored depth
        let tex_coord = vec2<i32>(face_uv * 512.0);
        var stored_depth: f32;
        switch face_idx {
            case 0: { stored_depth = textureLoad(point_shadow_face_px, tex_coord, 0); }
            case 1: { stored_depth = textureLoad(point_shadow_face_nx, tex_coord, 0); }
            case 2: { stored_depth = textureLoad(point_shadow_face_py, tex_coord, 0); }
            case 3: { stored_depth = textureLoad(point_shadow_face_ny, tex_coord, 0); }
            case 4: { stored_depth = textureLoad(point_shadow_face_pz, tex_coord, 0); }
            case 5: { stored_depth = textureLoad(point_shadow_face_nz, tex_coord, 0); }
            default: { stored_depth = 1.0; }
        }
        
        let compare_depth = distance / shadow_radius;
        let is_lit = select(0.0, 1.0, compare_depth <= stored_depth + 0.02);
        
        // R = stored, G = compare, B = result
        return vec4<f32>(stored_depth, compare_depth, is_lit, 1.0);
    }
    
    // Debug mode 30: Compare textureLoad vs textureSampleCompare on face 3 (-Y)
    // Split into 4 quadrants to test different compare values
    if DEBUG_MODE == 30 {
        let center_i = vec2<i32>(256, 256);
        let center_f = vec2<f32>(0.5, 0.5);
        
        // Raw depth from textureLoad (should be ~0.4)
        let raw_depth = textureLoad(point_shadow_face_ny, center_i, 0);
        
        // Test different compare values:
        // Quadrant layout (y < 0.5 is top):
        //   Top-left: raw depth
        //   Top-right: compare vs 0.3 (stored 0.4 >= 0.3 = TRUE = 1.0 white)
        //   Bottom-left: compare vs 0.5 (stored 0.4 >= 0.5 = FALSE = 0.0 black)
        //   Bottom-right: compare vs 0.2 (stored 0.4 >= 0.2 = TRUE = 1.0 white)
        
        let cmp_03 = textureSampleCompare(point_shadow_face_ny, point_shadow_sampler, center_f, 0.3);
        let cmp_05 = textureSampleCompare(point_shadow_face_ny, point_shadow_sampler, center_f, 0.5);
        let cmp_02 = textureSampleCompare(point_shadow_face_ny, point_shadow_sampler, center_f, 0.2);
        
        if in.uv.y < 0.5 {
            if in.uv.x < 0.5 {
                // Top-left: raw depth (gray ~0.4)
                return vec4<f32>(raw_depth, raw_depth, raw_depth, 1.0);
            } else {
                // Top-right: compare vs 0.3 (expect WHITE)
                return vec4<f32>(cmp_03, cmp_03, cmp_03, 1.0);
            }
        } else {
            if in.uv.x < 0.5 {
                // Bottom-left: compare vs 0.5 (expect BLACK)
                return vec4<f32>(cmp_05, cmp_05, cmp_05, 1.0);
            } else {
                // Bottom-right: compare vs 0.2 (expect WHITE)
                return vec4<f32>(cmp_02, cmp_02, cmp_02, 1.0);
            }
        }
    }
    
    // Debug mode 27: Show each face individually - use screen position to pick
    // Each face is cleared to unique value: F0=0.1, F1=0.2, F2=0.3, F3=0.4, F4=0.5, F5=0.6
    // Compare at 0.7: all cleared faces should be BLACK (depth < 0.7)
    // If any face is WHITE, it means the clear didn't work (still 1.0)
    if DEBUG_MODE == 27 {
        let compare_val = 0.7;
        // Divide screen into 6 vertical strips, one per face
        let strip = i32(in.uv.x * 6.0);
        var face_val = 0.0;
        if strip == 0 {
            face_val = textureSampleCompare(point_shadow_face_px, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        } else if strip == 1 {
            face_val = textureSampleCompare(point_shadow_face_nx, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        } else if strip == 2 {
            face_val = textureSampleCompare(point_shadow_face_py, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        } else if strip == 3 {
            face_val = textureSampleCompare(point_shadow_face_ny, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        } else if strip == 4 {
            face_val = textureSampleCompare(point_shadow_face_pz, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        } else {
            face_val = textureSampleCompare(point_shadow_face_nz, point_shadow_sampler, vec2<f32>(0.5, 0.5), compare_val);
        }
        // Black = geometry rendered, White = cleared/empty
        // Labels: strip 0=+X, 1=-X, 2=+Y, 3=-Y, 4=+Z, 5=-Z
        return vec4<f32>(face_val, face_val, face_val, 1.0);
    }
    
    // --- Point Light Calculation ---
    let point_light_contribution = calculate_all_point_lights(world_pos, world_normal);
    
    // Debug: Show point lights only
    if DEBUG_MODE == 6 {
        return vec4<f32>(point_light_contribution, 1.0);
    }
    
    // --- Lighting Calculation ---
    var total_light = vec3<f32>(0.0);
    
    if DARK_WORLD_MODE == 1 {
        // === DARK WORLD MODE: Dual colored moons with independent shadows ===
        
        // Very dim ambient
        total_light = DARK_AMBIENT_COLOR * DARK_AMBIENT_INTENSITY;
        
        // Moon 1 (Purple) - uses uniform data from MoonConfig
        let moon1_dir = normalize(-shadow_uniforms.moon1_direction.xyz);
        let moon1_color = shadow_uniforms.moon1_color_intensity.rgb;
        let moon1_intensity = shadow_uniforms.moon1_color_intensity.a;
        let n_dot_moon1 = max(dot(world_normal, moon1_dir), 0.0);
        total_light += moon1_color * moon1_intensity * n_dot_moon1 * moon1_shadow;
        
        // Moon 2 (Orange) - now with its own shadow!
        let moon2_dir = normalize(-shadow_uniforms.moon2_direction.xyz);
        let moon2_color = shadow_uniforms.moon2_color_intensity.rgb;
        let moon2_intensity = shadow_uniforms.moon2_color_intensity.a;
        let n_dot_moon2 = max(dot(world_normal, moon2_dir), 0.0);
        total_light += moon2_color * moon2_intensity * n_dot_moon2 * moon2_shadow;
        
    } else {
        // === CLASSIC SUN MODE ===
        
        // Ambient - base illumination for all surfaces (not affected by shadow)
        total_light = AMBIENT_COLOR * AMBIENT_INTENSITY;
        
        // Main directional light (sun) - standard N dot L, modulated by shadow
        let sun_dir = normalize(-SUN_DIRECTION);  // Direction TO the light
        let n_dot_sun = max(dot(world_normal, sun_dir), 0.0);
        total_light += SUN_COLOR * SUN_INTENSITY * n_dot_sun * shadow_factor;
        
        // Fill light from opposite side - prevents pure black shadows
        let fill_dir = normalize(-FILL_DIRECTION);  // Direction TO the light
        let n_dot_fill = max(dot(world_normal, fill_dir), 0.0);
        total_light += FILL_COLOR * FILL_INTENSITY * n_dot_fill;
    }
    
    // --- Minecraft-style Face Shading ---
    // Apply fixed brightness multipliers per face direction.
    // This makes blocks distinguishable even on flat surfaces where all faces
    // point the same direction and would otherwise have identical lighting.
    // Values tuned to match Minecraft's classic look.
    var face_multiplier = 1.0;
    if abs(world_normal.y) > 0.9 {
        // Top (+Y) or Bottom (-Y) faces
        if world_normal.y > 0.0 {
            face_multiplier = 1.0;  // Top faces: full brightness
        } else {
            face_multiplier = 0.5;  // Bottom faces: half brightness
        }
    } else if abs(world_normal.z) > 0.9 {
        // Front (+Z) or Back (-Z) faces
        face_multiplier = 0.8;
    } else {
        // Left (-X) or Right (+X) faces
        face_multiplier = 0.6;
    }
    
    // Apply face multiplier to total light
    total_light *= face_multiplier;
    
    // Debug: Show face multiplier only (should show discrete bands, not smooth gradient)
    if DEBUG_MODE == 70 {
        return vec4<f32>(vec3<f32>(face_multiplier), 1.0);
    }
    
    // Debug: Show N·L only for moon/sun (should show smooth gradient based on light angle)
    if DEBUG_MODE == 71 {
        if DARK_WORLD_MODE == 1 {
            let moon1_dir = normalize(-MOON1_DIRECTION);
            let moon2_dir = normalize(-MOON2_DIRECTION);
            let n_dot_moon1 = max(dot(world_normal, moon1_dir), 0.0);
            let n_dot_moon2 = max(dot(world_normal, moon2_dir), 0.0);
            // Show both N·L values: R = moon1, G = moon2
            return vec4<f32>(n_dot_moon1, n_dot_moon2, 0.0, 1.0);
        } else {
            let sun_dir = normalize(-SUN_DIRECTION);
            let n_dot_sun = max(dot(world_normal, sun_dir), 0.0);
            return vec4<f32>(n_dot_sun, n_dot_sun, n_dot_sun, 1.0);
        }
    }
    
    // --- Point Lights ---
    // Add contribution from point lights (colored local illumination)
    // Point lights are NOT affected by face shading (they're local, not directional)
    // but ARE affected by AO
    total_light += point_light_contribution;
    
    // --- Per-Vertex Ambient Occlusion ---
    // AO darkens corners and edges where blocks meet.
    // This is the key visual feature that makes voxels "pop" like Minecraft.
    // Applied after all other lighting as a multiplier.
    total_light *= ao;
    
    // Apply lighting to albedo
    var final_color = albedo * total_light;
    
    // Add emission - emissive surfaces glow with their own color
    // With the hybrid tone mapping in bloom_composite, we can use simpler
    // emission handling - just add albedo scaled by emission.
    // The Reinhard luminance mapping preserves color saturation for bright areas.
    if emission > 0.01 {
        // Perceptual scaling - sqrt makes mid-emission values more visible
        let emit_factor = sqrt(emission);
        
        // For emissive surfaces, replace lit color with self-illuminated albedo
        // The albedo IS the emission color - we just need to make it bright
        let emissive_color = albedo * (0.5 + emit_factor * 0.6);  // 0.5 to 1.1 brightness
        
        // Blend: high emission = mostly self-lit, low = mostly lit by external
        final_color = mix(final_color, emissive_color, emit_factor);
        
        // Add HDR boost for bloom - moderate to preserve color through tone mapping
        final_color += albedo * emit_factor * 0.5;
    }
    
    // --- Fog (Bonsai-style) ---
    // Exponential fog for more natural falloff
    let fog_factor = smoothstep(FOG_START, FOG_END, depth);
    final_color = mix(final_color, FOG_COLOR, fog_factor);
    
    // HDR output - values can exceed 1.0 for bloom
    return vec4<f32>(final_color, 1.0);
}
