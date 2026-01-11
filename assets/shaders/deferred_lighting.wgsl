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
// GTAO Sampling
// ============================================================================
// XeGTAO edge-aware denoising is now done in a separate compute pass.
// The gtao_texture input is already denoised, so we just sample directly.
// Simple bilinear upsampling (GTAO is at half-res).

fn sample_gtao(uv: vec2<f32>) -> f32 {
    return textureSample(gtao_texture, gtao_sampler, uv).r;
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
// Moon direction, color, and intensity now come from uniforms (shadow_uniforms)
// These constants are kept as fallbacks but should not be used

// Dark world ambient - base values, modified by moon positions at runtime
const DARK_AMBIENT_BASE_COLOR: vec3<f32> = vec3<f32>(0.02, 0.015, 0.03);
const DARK_AMBIENT_BASE_INTENSITY: f32 = 0.08;  // Base visibility even when both moons are down

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

// ============================================================================
// Debug Mode - controlled via uniform for runtime switching
// ============================================================================
// Debug modes are passed via uniform from Rust code (shadow_uniforms.shadow_softness.z).
// This allows runtime debug switching without shader recompilation.
//
// Available modes:
//   0 = Final lit scene (default)
//   1 = G-buffer normals (world space, remapped 0-1)
//   2 = G-buffer depth (linear, normalized)
//   3 = Albedo only (no lighting)
//   4 = Shadow factor (R=moon1, G=moon2)
//   5 = GTAO (ambient occlusion) - RAW OUTPUT, no lighting
//   6 = Point lights only
//   7 = World position XZ

// Debug mode accessor - reads from shadow_uniforms.shadow_softness.z
fn get_debug_mode() -> i32 {
    return i32(shadow_uniforms.shadow_softness.z);
}

// ============================================================================
// Moon Environment Lighting Functions
// These provide dynamic terrain lighting based on moon positions
// ============================================================================

// Calculate moon altitude factor for intensity scaling
// Returns 0.0 when moon is well below horizon, scales up as moon rises
// NOTE: moon_dir is direction FROM moon TO scene, so negative Y means moon is ABOVE
fn moon_altitude_factor(moon_dir: vec3<f32>) -> f32 {
    // moon_dir.y is negative when moon is above horizon (light pointing down)
    // -moon_dir.y gives us: +1 when moon at zenith, -1 when at nadir
    let altitude = -moon_dir.y;
    // Moon lighting should persist until moon disc fully sets below horizon
    // -0.15 = moon center well below horizon (disc gone)
    // 0.1 = moon just above horizon (still rising, full intensity)
    // This keeps lighting effects strong while moon is near/at horizon
    return smoothstep(-0.15, 0.1, altitude);
}

// Calculate horizon warmth - color shifts warmer when moon is near horizon
// NOTE: moon_dir is direction FROM moon TO scene, so negative Y means moon is ABOVE
fn horizon_warmth(moon_dir: vec3<f32>, base_color: vec3<f32>) -> vec3<f32> {
    let altitude = -moon_dir.y;  // Flip sign: positive = above horizon
    // Horizon proximity: 1.0 at horizon, 0.0 at zenith/nadir
    let horizon_proximity = 1.0 - abs(altitude);
    let warmth = horizon_proximity * horizon_proximity;
    
    // Shift toward warm (add red/yellow, reduce blue)
    let warm_shift = vec3<f32>(0.15, 0.05, -0.1) * warmth;
    return base_color + warm_shift;
}

// Calculate zenith-darkness: how dark should the scene be based on moon positions
// Returns 1.0 when at least one moon is visible, drops toward 0.25 when both are below
// NOTE: moon_dir.y values passed here - negative Y means moon ABOVE (light pointing down)
fn calculate_darkness_factor(moon1_dir_y: f32, moon2_dir_y: f32) -> f32 {
    // Convert to altitude (positive = above horizon)
    let moon1_alt = -moon1_dir_y;
    let moon2_alt = -moon2_dir_y;
    
    // How far below horizon is each moon? 
    // Use -0.15 as the "gone" threshold (matches moon_altitude_factor)
    // 0 if still providing light, positive if truly below
    let moon1_below = max(0.0, -0.15 - moon1_alt);
    let moon2_below = max(0.0, -0.15 - moon2_alt);
    
    // Night depth: only deep when BOTH moons are well below horizon
    // Use minimum because we need both to be down for true darkness
    let night_depth = min(moon1_below, moon2_below) * 3.0;  // Scale so 0.33 below = full night
    let clamped_depth = clamp(night_depth, 0.0, 1.0);
    
    // Darkness factor: 1.0 = normal brightness, 0.25 = dim but visible
    // We want enough base visibility to see the terrain even in full darkness
    return 1.0 - clamped_depth * 0.75;
}

// Calculate dynamic ambient color based on which moon is more visible
// NOTE: moon_dir is direction FROM moon TO scene, so negative Y means moon is ABOVE
fn calculate_dynamic_ambient(moon1_dir: vec3<f32>, moon1_color: vec3<f32>,
                             moon2_dir: vec3<f32>, moon2_color: vec3<f32>) -> vec3<f32> {
    // Convert to altitude (positive = above horizon)
    let moon1_alt = -moon1_dir.y;
    let moon2_alt = -moon2_dir.y;
    
    // Contribution based on visibility - use -0.15 threshold to match altitude_factor
    // Moons contribute until they're well below horizon
    let moon1_contrib = max(0.0, moon1_alt + 0.15);
    let moon2_contrib = max(0.0, moon2_alt + 0.15);
    let total = moon1_contrib + moon2_contrib + 0.001;  // Avoid div by zero
    
    // Blend colors based on relative visibility
    let blend = moon1_contrib / total;
    let blended_color = mix(moon2_color, moon1_color, blend);
    
    // Desaturate and dim for ambient (ambient shouldn't be as saturated as direct light)
    let ambient_color = blended_color * 0.3 + DARK_AMBIENT_BASE_COLOR;
    
    // Intensity scales with highest visible moon
    // Use smoothstep for gradual falloff matching moon_altitude_factor
    let max_visible = max(moon1_alt, moon2_alt);
    let visibility = smoothstep(-0.15, 0.1, max_visible);
    let intensity = DARK_AMBIENT_BASE_INTENSITY + visibility * 0.1;
    
    return ambient_color * intensity;
}

// Full moon lighting contribution with altitude scaling and horizon warmth
fn calculate_moon_light(
    moon_dir: vec3<f32>,
    moon_color: vec3<f32>,
    moon_intensity: f32,
    world_normal: vec3<f32>,
    shadow: f32
) -> vec3<f32> {
    // Altitude-based intensity scaling
    let alt_factor = moon_altitude_factor(moon_dir);
    
    // Skip if moon not contributing (below horizon)
    if alt_factor < 0.01 {
        return vec3<f32>(0.0);
    }
    
    // Apply horizon warmth to color
    let warmed_color = horizon_warmth(moon_dir, moon_color);
    
    // Standard N.L lighting (direction TO the light)
    let light_dir = normalize(-moon_dir);
    let n_dot_l = max(dot(world_normal, light_dir), 0.0);
    
    // Scale down intensity slightly - moons should be moody, not blinding
    // Base intensity comes from MoonConfig (0.5 for moon1, 0.45 for moon2)
    let scaled_intensity = moon_intensity * 0.7;
    
    // Combine: color * base_intensity * altitude_scale * N.L * shadow
    return warmed_color * scaled_intensity * alt_factor * n_dot_l * shadow;
}

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
    
    // Sample denoised GTAO (XeGTAO edge-aware denoising done in compute pass)
    // Simple bilinear upsampling since GTAO is at half resolution
    let ao = sample_gtao(in.uv);
    
    // =========================================================================
    // Debug outputs - controlled via uniform (shadow_uniforms.shadow_softness.z)
    // =========================================================================
    
    // Get debug mode from uniform
    let debug_mode = get_debug_mode();
    
    // Mode 1: G-buffer normals (world space, remapped 0-1)
    if debug_mode == 1 {
        return vec4<f32>(world_normal * 0.5 + 0.5, 1.0);
    }
    
    // Mode 2: G-buffer depth (linear, normalized)
    if debug_mode == 2 {
        let d = clamp(depth / 50.0, 0.0, 1.0);
        return vec4<f32>(d, d, d, 1.0);
    }
    
    // Mode 3: Albedo only (no lighting)
    if debug_mode == 3 {
        return vec4<f32>(albedo, 1.0);
    }
    
    // Skip pixels with no geometry (far depth = 1000)
    if depth > 999.0 {
        return vec4<f32>(FOG_COLOR, 1.0);
    }
    
    // --- Shadow Calculation ---
    let moon1_shadow = calculate_moon1_shadow(world_pos, world_normal);
    let moon2_shadow = calculate_moon2_shadow(world_pos, world_normal);
    let shadow_factor = moon1_shadow;
    
    // Mode 4: Shadow factor (R=moon1, G=moon2)
    if debug_mode == 4 {
        return vec4<f32>(moon1_shadow, moon2_shadow, 0.0, 1.0);
    }
    
    // Mode 5: GTAO (ambient occlusion) - RAW GTAO OUTPUT
    if debug_mode == 5 {
        return vec4<f32>(vec3<f32>(ao), 1.0);
    }
    
    // Mode 7: World position XZ
    if debug_mode == 7 {
        let x_norm = (world_pos.x + 20.0) / 40.0;
        let z_norm = (world_pos.z + 20.0) / 40.0;
        return vec4<f32>(x_norm, 0.0, z_norm, 1.0);
    }
    
    // --- Point Light Calculation ---
    let point_light_contribution = calculate_all_point_lights(world_pos, world_normal);
    
    // Mode 6: Point lights only
    if debug_mode == 6 {
        return vec4<f32>(point_light_contribution, 1.0);
    }
    
    // --- Lighting Calculation ---
    var total_light = vec3<f32>(0.0);
    
    if DARK_WORLD_MODE == 1 {
        // === DARK WORLD MODE: Dual colored moons with dynamic environment lighting ===
        // Moon positions now affect terrain lighting intensity, color, and ambient
        
        // Get moon data from uniforms (dynamic, changes with T/Y keys)
        let moon1_dir = normalize(shadow_uniforms.moon1_direction.xyz);
        let moon1_color = shadow_uniforms.moon1_color_intensity.rgb;
        let moon1_intensity = shadow_uniforms.moon1_color_intensity.a;
        
        let moon2_dir = normalize(shadow_uniforms.moon2_direction.xyz);
        let moon2_color = shadow_uniforms.moon2_color_intensity.rgb;
        let moon2_intensity = shadow_uniforms.moon2_color_intensity.a;
        
        // Dynamic ambient based on which moon is more visible
        // Blends moon colors and scales with altitude
        total_light = calculate_dynamic_ambient(moon1_dir, moon1_color, moon2_dir, moon2_color);
        
        // Moon 1 (Purple) - with altitude scaling and horizon warmth
        total_light += calculate_moon_light(
            moon1_dir, moon1_color, moon1_intensity,
            world_normal, moon1_shadow
        );
        
        // Moon 2 (Orange) - with altitude scaling and horizon warmth
        total_light += calculate_moon_light(
            moon2_dir, moon2_color, moon2_intensity,
            world_normal, moon2_shadow
        );
        
        // Apply zenith-darkness: when both moons are below horizon, go nearly black
        let darkness_factor = calculate_darkness_factor(moon1_dir.y, moon2_dir.y);
        total_light *= darkness_factor;
        
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
