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

// Point lights (bind group 3)
// Must match MAX_POINT_LIGHTS in point_light.rs
const MAX_POINT_LIGHTS: u32 = 32u;

struct PointLight {
    position: vec4<f32>,        // xyz = position, w = unused
    color_intensity: vec4<f32>, // rgb = color, a = intensity
    radius_padding: vec4<f32>,  // x = radius, yzw = unused
}

struct PointLightsUniform {
    count: vec4<u32>,  // x = count, yzw = unused
    lights: array<PointLight, 32>,
}
@group(3) @binding(0) var<uniform> point_lights: PointLightsUniform;

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
const MOON1_INTENSITY: f32 = 0.35;  // Dimmer - let point lights dominate

// Orange Moon - from front-right, similar height (lights opposite faces)
const MOON2_DIRECTION: vec3<f32> = vec3<f32>(-0.6, -0.6, -0.55);  // Front-right
const MOON2_COLOR: vec3<f32> = vec3<f32>(1.0, 0.45, 0.1);  // Deep orange
const MOON2_INTENSITY: f32 = 0.3;  // Dimmer

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

// Debug mode: 0 = final lighting, 1 = show gNormal, 2 = show gPosition depth, 3 = albedo only, 4 = shadow only, 5 = AO only, 6 = point lights only
const DEBUG_MODE: i32 = 0;

// Calculate point light contribution at a world position.
// Uses smooth quadratic falloff to zero at the light's radius.
fn calculate_point_light(
    light: PointLight,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
) -> vec3<f32> {
    let light_pos = light.position.xyz;
    let light_color = light.color_intensity.rgb;
    let intensity = light.color_intensity.a;
    let radius = light.radius_padding.x;
    
    // Vector from surface to light
    let to_light = light_pos - world_pos;
    let distance = length(to_light);
    
    // Skip if outside radius
    if distance > radius {
        return vec3<f32>(0.0);
    }
    
    // Normalize direction
    let light_dir = to_light / distance;
    
    // N dot L for diffuse lighting
    let n_dot_l = max(dot(world_normal, light_dir), 0.0);
    
    // Smooth quadratic attenuation: (1 - (d/r)^2)^2
    // This gives a nice smooth falloff that reaches exactly zero at radius
    let distance_ratio = distance / radius;
    let attenuation = pow(1.0 - distance_ratio * distance_ratio, 2.0);
    
    return light_color * intensity * n_dot_l * attenuation;
}

// Calculate total contribution from all point lights
fn calculate_all_point_lights(world_pos: vec3<f32>, world_normal: vec3<f32>) -> vec3<f32> {
    var total = vec3<f32>(0.0);
    let light_count = point_lights.count.x;
    
    for (var i = 0u; i < light_count && i < MAX_POINT_LIGHTS; i++) {
        total += calculate_point_light(point_lights.lights[i], world_pos, world_normal);
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
    
    // Add emission - emission makes the surface glow beyond its lit color
    // Higher emission = more of the albedo color added as self-illumination
    // Scale emission contribution: emission is 0-1 normalized from 0-255 input
    // We want high emission values (like 200) to produce HDR values > 1.0 for bloom
    let emission_strength = emission * 5.0;  // Strong emission for visible bloom
    final_color += albedo * emission_strength;
    
    // --- Fog (Bonsai-style) ---
    // Exponential fog for more natural falloff
    let fog_factor = smoothstep(FOG_START, FOG_END, depth);
    final_color = mix(final_color, FOG_COLOR, fog_factor);
    
    // HDR output - values can exceed 1.0 for bloom
    return vec4<f32>(final_color, 1.0);
}
