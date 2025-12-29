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

// Shadow map (bind group 1)
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;

// Shadow uniforms (bind group 2) - light-space matrix
struct ShadowUniforms {
    light_view_proj: mat4x4<f32>,
}
@group(2) @binding(0) var<uniform> shadow: ShadowUniforms;

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

// Debug mode: 0 = final lighting, 1 = show gNormal, 2 = show gPosition depth, 3 = albedo only, 4 = shadow only, 5 = AO only, 6 = point lights only, 7 = world position XZ, 8 = light count, 9 = distance to light 0, 10 = first light color, 11 = first light radius, 20 = point shadow for light 0, 21 = raw -Y face shadow sample, 22 = face UV coords, 23 = which cube face, 24 = compare depth, 25 = show UV coords for -Y face, 26 = test fixed UV sample
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
        if i == 0u {
            let shadow = calculate_point_shadow(light.position.xyz, world_pos, light.radius_padding.x);
            total += calculate_point_light_with_shadow(light, world_pos, world_normal, shadow);
        } else {
            total += calculate_point_light(light, world_pos, world_normal);
        }
    }
    
    return total;
}

// Calculate shadow factor using PCF (Percentage Closer Filtering).
// Returns 0.0 for fully in shadow, 1.0 for fully lit.
fn calculate_shadow(world_pos: vec3<f32>, world_normal: vec3<f32>) -> f32 {
    // Transform world position to light clip space
    let light_space_pos = shadow.light_view_proj * vec4<f32>(world_pos, 1.0);
    
    // Perspective divide (orthographic projection, but still needed for proper coords)
    let proj_coords = light_space_pos.xyz / light_space_pos.w;
    
    // Transform from NDC [-1,1] to texture UV [0,1]
    // Note: Y is flipped because texture coordinates go top-to-bottom
    let shadow_uv = vec2<f32>(
        proj_coords.x * 0.5 + 0.5,
        proj_coords.y * -0.5 + 0.5  // Flip Y
    );
    
    // Current fragment depth in light space
    let current_depth = proj_coords.z;
    
    // Check if outside shadow map bounds
    if shadow_uv.x < 0.0 || shadow_uv.x > 1.0 || shadow_uv.y < 0.0 || shadow_uv.y > 1.0 {
        return 1.0;  // Outside shadow map = not in shadow
    }
    
    // Check if behind the light's far plane
    if current_depth > 1.0 || current_depth < 0.0 {
        return 1.0;  // Outside frustum = not in shadow
    }
    
    // Calculate slope-scaled bias based on surface angle to light
    // Surfaces perpendicular to light need less bias, grazing angles need more
    // Use the primary shadow-casting light direction
    var primary_light_dir: vec3<f32>;
    if DARK_WORLD_MODE == 1 {
        primary_light_dir = normalize(-MOON1_DIRECTION);  // Purple moon casts shadows
    } else {
        primary_light_dir = normalize(-SUN_DIRECTION);
    }
    let n_dot_l = max(dot(world_normal, primary_light_dir), 0.0);
    let bias = max(SHADOW_BIAS_MAX * (1.0 - n_dot_l), SHADOW_BIAS_MIN);
    
    // PCF 3x3 sampling for soft shadow edges
    var shadow_sum = 0.0;
    let texel_size = 1.0 / SHADOW_MAP_SIZE;
    
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            // textureSampleCompare returns 1.0 if current_depth - bias < shadow_depth
            shadow_sum += textureSampleCompare(
                shadow_map,
                shadow_sampler,
                shadow_uv + offset,
                current_depth - bias
            );
        }
    }
    
    return shadow_sum / 9.0;
}

// Calculate shadow for a point light using cube shadow map.
// Returns 0.0 for fully in shadow, 1.0 for fully lit.
// light_pos: world position of the light
// world_pos: world position of the fragment being shaded
// radius: light's maximum range (used as far plane)
fn calculate_point_shadow(light_pos: vec3<f32>, world_pos: vec3<f32>, radius: f32) -> f32 {
    // Vector from light to fragment (NOT normalized - we need the actual components for projection)
    let light_to_frag = world_pos - light_pos;
    let distance = length(light_to_frag);
    
    // Skip if outside light radius
    if distance > radius {
        return 1.0;
    }
    
    // Use raw vector components for cube face UV calculation
    // This gives us proper perspective projection matching the shadow render pass
    let abs_vec = abs(light_to_frag);
    var face_uv: vec2<f32>;
    var shadow_depth: f32;
    
    // Add small bias to prevent shadow acne
    let bias = 0.02;
    let compare_depth = (distance / radius) - bias;
    
    // Find dominant axis and compute UV + sample shadow
    // The UV calculation must match the view matrices in CubeFaceMatrices::new()
    // For look_at_rh looking in direction D with up U:
    //   Right = normalize(cross(D, U))
    //   Up = cross(Right, D)  
    //   View transforms world to (Right, Up, -D) basis
    // Then perspective divides by -view_z to get clip coords
    
    // UV mapping derived from look_to_rh view matrices:
    // For look_to_rh(eye, dir, up): Right = cross(dir, up), TrueUp = cross(Right, dir)
    // view = (dot(ltf, Right), dot(ltf, TrueUp), -dot(ltf, dir))
    // clip.w = -view.z = dot(ltf, dir)
    // NDC = (view.x / clip.w, view.y / clip.w)
    // UV = (NDC.x * 0.5 + 0.5, -NDC.y * 0.5 + 0.5)  <- Y flipped for texture coords
    
    if abs_vec.x >= abs_vec.y && abs_vec.x >= abs_vec.z {
        // X is dominant axis
        if light_to_frag.x > 0.0 {
            // +X face: look_to_rh(pos, X, -Y)
            // Right = Z, TrueUp = -Y, view = (ltf.z, -ltf.y, -ltf.x)
            // clip.w = ltf.x, NDC = (ltf.z/ltf.x, -ltf.y/ltf.x)
            // UV = (ltf.z/abs_x * 0.5 + 0.5, ltf.y/abs_x * 0.5 + 0.5)
            face_uv = vec2<f32>(
                light_to_frag.z / abs_vec.x * 0.5 + 0.5,
                light_to_frag.y / abs_vec.x * 0.5 + 0.5
            );
            shadow_depth = textureSampleCompare(
                point_shadow_face_px, point_shadow_sampler,
                face_uv, compare_depth
            );
        } else {
            // -X face: look_to_rh(pos, -X, -Y)
            // Right = -Z, TrueUp = -Y, view = (-ltf.z, -ltf.y, ltf.x)
            // clip.w = -ltf.x = abs_x, NDC = (-ltf.z/abs_x, -ltf.y/abs_x)
            // UV = (-ltf.z/abs_x * 0.5 + 0.5, ltf.y/abs_x * 0.5 + 0.5)
            face_uv = vec2<f32>(
                -light_to_frag.z / abs_vec.x * 0.5 + 0.5,
                light_to_frag.y / abs_vec.x * 0.5 + 0.5
            );
            shadow_depth = textureSampleCompare(
                point_shadow_face_nx, point_shadow_sampler,
                face_uv, compare_depth
            );
        }
    } else if abs_vec.y >= abs_vec.x && abs_vec.y >= abs_vec.z {
        // Y is dominant axis
        if light_to_frag.y > 0.0 {
            // +Y face: look_to_rh(pos, Y, Z)
            // Right = X, TrueUp = Z, view = (ltf.x, ltf.z, -ltf.y)
            // clip.w = ltf.y, NDC = (ltf.x/ltf.y, ltf.z/ltf.y)
            // UV = (ltf.x/abs_y * 0.5 + 0.5, -ltf.z/abs_y * 0.5 + 0.5)
            face_uv = vec2<f32>(
                light_to_frag.x / abs_vec.y * 0.5 + 0.5,
                -light_to_frag.z / abs_vec.y * 0.5 + 0.5
            );
            shadow_depth = textureSampleCompare(
                point_shadow_face_py, point_shadow_sampler,
                face_uv, compare_depth
            );
        } else {
            // -Y face: look_to_rh(pos, -Y, -Z)
            // Right = X, TrueUp = -Z, view = (ltf.x, -ltf.z, ltf.y)
            // clip.w = -ltf.y = abs_y, NDC = (ltf.x/abs_y, -ltf.z/abs_y)
            // UV = (ltf.x/abs_y * 0.5 + 0.5, ltf.z/abs_y * 0.5 + 0.5)
            face_uv = vec2<f32>(
                light_to_frag.x / abs_vec.y * 0.5 + 0.5,
                light_to_frag.z / abs_vec.y * 0.5 + 0.5
            );
            shadow_depth = textureSampleCompare(
                point_shadow_face_ny, point_shadow_sampler,
                face_uv, compare_depth
            );
        }
    } else {
        // Z is dominant axis
        if light_to_frag.z > 0.0 {
            // +Z face: look_to_rh(pos, Z, -Y)
            // Right = X, TrueUp = -Y, view = (ltf.x, -ltf.y, -ltf.z)
            // clip.w = ltf.z, NDC = (ltf.x/ltf.z, -ltf.y/ltf.z)
            // UV = (ltf.x/abs_z * 0.5 + 0.5, ltf.y/abs_z * 0.5 + 0.5)
            face_uv = vec2<f32>(
                light_to_frag.x / abs_vec.z * 0.5 + 0.5,
                light_to_frag.y / abs_vec.z * 0.5 + 0.5
            );
            shadow_depth = textureSampleCompare(
                point_shadow_face_pz, point_shadow_sampler,
                face_uv, compare_depth
            );
        } else {
            // -Z face: look_to_rh(pos, -Z, -Y)
            // Right = -X, TrueUp = -Y, view = (-ltf.x, -ltf.y, ltf.z)
            // clip.w = -ltf.z = abs_z, NDC = (-ltf.x/abs_z, -ltf.y/abs_z)
            // UV = (-ltf.x/abs_z * 0.5 + 0.5, ltf.y/abs_z * 0.5 + 0.5)
            face_uv = vec2<f32>(
                -light_to_frag.x / abs_vec.z * 0.5 + 0.5,
                light_to_frag.y / abs_vec.z * 0.5 + 0.5
            );
            shadow_depth = textureSampleCompare(
                point_shadow_face_nz, point_shadow_sampler,
                face_uv, compare_depth
            );
        }
    }
    
    return shadow_depth;
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
    
    // Ambient occlusion is stored in normal.a (0 = fully occluded, 1 = fully lit)
    let ao = normal_sample.a;
    
    let world_pos = position_sample.xyz;
    let depth = position_sample.w;
    
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
    let shadow_factor = calculate_shadow(world_pos, world_normal);
    
    // Debug: Show shadow only
    if DEBUG_MODE == 4 {
        return vec4<f32>(vec3<f32>(shadow_factor), 1.0);
    }
    
    // Debug: Show AO only
    if DEBUG_MODE == 5 {
        return vec4<f32>(vec3<f32>(ao), 1.0);
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
    
    // Debug: Sample -Y face shadow map directly (this is the floor-facing direction)
    if DEBUG_MODE == 21 {
        // Sample at a fixed UV, show the depth value
        let sample_uv = in.uv;  // Use screen UV as sample coord
        let depth = textureSampleCompare(point_shadow_face_ny, point_shadow_sampler, sample_uv, 0.5);
        return vec4<f32>(depth, depth, depth, 1.0);
    }
    
    // Debug: Show which cube face would be selected for each fragment
    if DEBUG_MODE == 23 {
        if point_lights.count.x > 0u {
            let light = point_lights.lights[0];
            let light_to_frag = world_pos - light.position.xyz;
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
        // === DARK WORLD MODE: Dual colored moons ===
        
        // Very dim ambient
        total_light = DARK_AMBIENT_COLOR * DARK_AMBIENT_INTENSITY;
        
        // Purple Moon (primary light, uses shadow map)
        let moon1_dir = normalize(-MOON1_DIRECTION);
        let n_dot_moon1 = max(dot(world_normal, moon1_dir), 0.0);
        total_light += MOON1_COLOR * MOON1_INTENSITY * n_dot_moon1 * shadow_factor;
        
        // Orange Moon (secondary light, no shadow map yet - TODO: multi-shadow)
        // Orange moon lights faces that purple moon doesn't hit well
        let moon2_dir = normalize(-MOON2_DIRECTION);
        let n_dot_moon2 = max(dot(world_normal, moon2_dir), 0.0);
        // No shadow for orange moon yet - it provides fill lighting
        total_light += MOON2_COLOR * MOON2_INTENSITY * n_dot_moon2;
        
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
