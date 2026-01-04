//! Shadow mapping for deferred rendering.
//!
//! Implements directional shadow mapping for dual moon lights:
//! 1. Render scene from each moon's perspective to depth textures
//! 2. Sample shadow maps in lighting pass to determine shadow visibility
//!
//! Also supports a single point light shadow (cube shadow map).
//!
//! Based on Bonsai's DepthRTT.* and Lighting.fragmentshader shadow sampling.

use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        BindGroupLayout, BindGroupLayoutEntry, BindingType, BufferBindingType,
        CachedRenderPipelineId, CompareFunction, DepthStencilState, Extent3d, PipelineCache,
        PrimitiveState, RenderPipelineDescriptor, ShaderStages, StencilState, TextureDescriptor,
        TextureDimension, TextureFormat, TextureUsages, VertexState,
    },
    renderer::RenderDevice,
    texture::{CachedTexture, TextureCache},
};

use super::gbuffer_geometry::GBufferVertex;

/// Shadow map resolution (2048x2048 is a good balance of quality vs performance).
pub const SHADOW_MAP_SIZE: u32 = 2048;

/// Shadow map texture format.
pub const SHADOW_DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// Configuration for dual moon lighting and shadow system.
///
/// Controls both moons' directions, colors, intensities, and shadow parameters.
/// This is the primary configuration for the dark fantasy lighting aesthetic.
#[derive(Resource, Clone)]
pub struct MoonConfig {
    // === Moon 1 (Purple) ===
    /// Direction FROM moon TO scene (normalized).
    pub moon1_direction: Vec3,
    /// Moon 1 color (linear RGB).
    pub moon1_color: Vec3,
    /// Moon 1 intensity multiplier.
    pub moon1_intensity: f32,

    // === Moon 2 (Orange) ===
    /// Direction FROM moon TO scene (normalized).
    pub moon2_direction: Vec3,
    /// Moon 2 color (linear RGB).
    pub moon2_color: Vec3,
    /// Moon 2 intensity multiplier.
    pub moon2_intensity: f32,

    // === Shadow Parameters (shared) ===
    /// Size of the orthographic shadow frustum (half-width in world units).
    pub shadow_size: f32,
    /// Near plane for shadow frustum.
    pub near: f32,
    /// Far plane for shadow frustum.
    pub far: f32,

    /// Shadow softness for directional lights (0.0 = hard, 1.0 = very soft).
    /// Controls the Poisson disk sampling radius.
    pub directional_shadow_softness: f32,

    /// Shadow softness for point lights (0.0 = hard, 1.0 = very soft).
    pub point_shadow_softness: f32,
}

impl Default for MoonConfig {
    fn default() -> Self {
        Self::dark_world()
    }
}

impl MoonConfig {
    /// Dark fantasy dual moon configuration.
    /// Purple moon from back-left, orange moon from front-right.
    ///
    /// Moon directions are set so shadows cast in opposite directions,
    /// making it easy to see which moon is lighting each surface.
    pub fn dark_world() -> Self {
        Self {
            // Purple moon - from the left side at moderate height
            // Shadows cast to the right
            moon1_direction: Vec3::new(0.7, -0.5, 0.3).normalize(),
            moon1_color: Vec3::new(0.5, 0.2, 0.9), // Rich purple
            moon1_intensity: 0.5,                  // Bright enough to see clearly

            // Orange moon - from the right side at moderate height
            // Shadows cast to the left
            moon2_direction: Vec3::new(-0.7, -0.5, -0.3).normalize(),
            moon2_color: Vec3::new(1.0, 0.5, 0.15), // Warm orange
            moon2_intensity: 0.45,                  // Slightly dimmer to prevent washout

            // Shadow parameters
            shadow_size: 50.0,
            near: 0.1,
            far: 200.0,

            // Soft shadows by default
            directional_shadow_softness: 0.4,
            point_shadow_softness: 0.3,
        }
    }

    /// Single bright sun configuration (classic daytime look).
    pub fn sun() -> Self {
        Self {
            // Sun from above-right
            moon1_direction: Vec3::new(0.3, -0.9, -0.3).normalize(),
            moon1_color: Vec3::new(1.0, 0.95, 0.9),
            moon1_intensity: 1.0,

            // Fill light from opposite side (no shadow)
            moon2_direction: Vec3::new(-0.5, -0.3, 0.8).normalize(),
            moon2_color: Vec3::new(0.5, 0.6, 0.8),
            moon2_intensity: 0.3,

            shadow_size: 50.0,
            near: 0.1,
            far: 200.0,

            directional_shadow_softness: 0.3,
            point_shadow_softness: 0.3,
        }
    }

    /// Calculate light-space view matrix for a given moon direction.
    ///
    /// For orthographic directional shadows, the view matrix must be constructed
    /// so that objects at different positions perpendicular to the light direction
    /// map to different UV coordinates in the shadow map.
    ///
    /// The standard look_at with Y-up doesn't work well when the light direction
    /// is in the XY plane (Z≈0), because it makes the shadow map X axis align
    /// with world Z, causing world X variations to only affect depth, not UV.
    fn light_view_matrix(&self, direction: Vec3, scene_center: Vec3) -> Mat4 {
        let light_distance = self.shadow_size * 2.0;
        let light_pos = scene_center - direction * light_distance;

        // Compute a good up vector that keeps shadow coordinates intuitive:
        // - For lights in the XY plane (Z≈0), use Z as up so that shadow X ≈ world X
        // - For lights with Z component, use Y as up (standard)
        let up = if direction.z.abs() < 0.01 {
            // Light is in XY plane - cross product with Y gives us the right vector
            // We want right to be along X, so we need up to be along Z
            if direction.x > 0.0 {
                Vec3::NEG_Z // Light from +X, shadow goes to -X, up is -Z
            } else {
                Vec3::Z // Light from -X, shadow goes to +X, up is +Z
            }
        } else {
            Vec3::Y
        };

        Mat4::look_at_rh(light_pos, scene_center, up)
    }

    /// Calculate orthographic projection matrix for shadows.
    fn light_projection_matrix(&self) -> Mat4 {
        Mat4::orthographic_rh(
            -self.shadow_size,
            self.shadow_size,
            -self.shadow_size,
            self.shadow_size,
            self.near,
            self.far,
        )
    }

    /// Get light-space view-projection matrix for Moon 1.
    pub fn moon1_view_projection(&self, scene_center: Vec3) -> Mat4 {
        self.light_projection_matrix() * self.light_view_matrix(self.moon1_direction, scene_center)
    }

    /// Get light-space view-projection matrix for Moon 2.
    pub fn moon2_view_projection(&self, scene_center: Vec3) -> Mat4 {
        self.light_projection_matrix() * self.light_view_matrix(self.moon2_direction, scene_center)
    }
}

/// Dual directional shadow map textures for moon lighting.
/// Contains separate shadow maps for Moon 1 and Moon 2.
#[derive(Component)]
pub struct ViewDirectionalShadowTextures {
    /// Moon 1 (purple) shadow depth texture.
    pub moon1: CachedTexture,
    /// Moon 2 (orange) shadow depth texture.
    pub moon2: CachedTexture,
    /// Size of each shadow map.
    pub size: Extent3d,
}

impl ViewDirectionalShadowTextures {
    /// Create shadow map textures for both moons.
    pub fn new(render_device: &RenderDevice, texture_cache: &mut TextureCache) -> Self {
        let size = Extent3d {
            width: SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
            depth_or_array_layers: 1,
        };

        let moon1 = texture_cache.get(
            render_device,
            TextureDescriptor {
                label: Some("moon1_shadow_depth"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: SHADOW_DEPTH_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        let moon2 = texture_cache.get(
            render_device,
            TextureDescriptor {
                label: Some("moon2_shadow_depth"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: SHADOW_DEPTH_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        Self { moon1, moon2, size }
    }
}

/// Shadow map uniform data passed to shadow depth shader.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowViewUniform {
    /// Light-space view-projection matrix.
    pub light_view_proj: [[f32; 4]; 4],
}

/// GPU uniform data for dual moon shadow system.
/// Passed to the lighting shader for shadow sampling and moon lighting.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DirectionalShadowUniforms {
    /// Moon 1 light-space view-projection matrix.
    pub moon1_view_proj: [[f32; 4]; 4],
    /// Moon 2 light-space view-projection matrix.
    pub moon2_view_proj: [[f32; 4]; 4],
    /// Moon 1 direction (xyz) + unused (w).
    pub moon1_direction: [f32; 4],
    /// Moon 1 color (rgb) + intensity (a).
    pub moon1_color_intensity: [f32; 4],
    /// Moon 2 direction (xyz) + unused (w).
    pub moon2_direction: [f32; 4],
    /// Moon 2 color (rgb) + intensity (a).
    pub moon2_color_intensity: [f32; 4],
    /// Shadow softness: x = directional, y = point, z = lighting_debug_mode, w = unused.
    pub shadow_softness: [f32; 4],
}

impl DirectionalShadowUniforms {
    /// Create uniforms from MoonConfig with debug mode.
    pub fn from_config(config: &MoonConfig, scene_center: Vec3, lighting_debug_mode: i32) -> Self {
        Self {
            moon1_view_proj: config
                .moon1_view_projection(scene_center)
                .to_cols_array_2d(),
            moon2_view_proj: config
                .moon2_view_projection(scene_center)
                .to_cols_array_2d(),
            moon1_direction: [
                config.moon1_direction.x,
                config.moon1_direction.y,
                config.moon1_direction.z,
                0.0,
            ],
            moon1_color_intensity: [
                config.moon1_color.x,
                config.moon1_color.y,
                config.moon1_color.z,
                config.moon1_intensity,
            ],
            moon2_direction: [
                config.moon2_direction.x,
                config.moon2_direction.y,
                config.moon2_direction.z,
                0.0,
            ],
            moon2_color_intensity: [
                config.moon2_color.x,
                config.moon2_color.y,
                config.moon2_color.z,
                config.moon2_intensity,
            ],
            shadow_softness: [
                config.directional_shadow_softness,
                config.point_shadow_softness,
                lighting_debug_mode as f32,
                0.0,
            ],
        }
    }
}

/// Shadow pipeline resources.
#[derive(Resource)]
pub struct ShadowPipeline {
    /// Shadow depth render pipeline ID.
    pub pipeline_id: CachedRenderPipelineId,
    /// Bind group layout for view uniforms.
    pub view_layout: BindGroupLayout,
    /// Bind group layout for mesh uniforms.
    pub mesh_layout: BindGroupLayout,
}

/// Initialize the shadow pipeline.
pub fn init_shadow_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<ShadowPipeline>>,
) {
    if existing.is_some() {
        return;
    }

    // View layout: light-space view-projection matrix
    let view_layout = render_device.create_bind_group_layout(
        "shadow_view_layout",
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    );

    // Mesh layout: per-mesh model transform
    let mesh_layout = render_device.create_bind_group_layout(
        "shadow_mesh_layout",
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    );

    // Load shadow depth shader
    let shader = asset_server.load("shaders/shadow_depth.wgsl");

    // Create shadow depth pipeline (depth-only, no color output)
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("shadow_depth_pipeline".into()),
        layout: vec![view_layout.clone(), mesh_layout.clone()],
        push_constant_ranges: vec![],
        vertex: VertexState {
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Some("vs_main".into()),
            buffers: vec![GBufferVertex::vertex_buffer_layout()],
        },
        primitive: PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            // Cull back faces - same as G-buffer pass
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(DepthStencilState {
            format: SHADOW_DEPTH_FORMAT,
            depth_write_enabled: true,
            // Standard depth test (not reverse-Z for shadow maps, simpler math)
            depth_compare: CompareFunction::LessEqual,
            stencil: StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,      // Small constant bias
                slope_scale: 2.0, // Slope-scaled bias for angled surfaces
                clamp: 0.0,
            },
        }),
        multisample: Default::default(),
        // No fragment shader needed - depth-only pass
        fragment: None,
        zero_initialize_workgroup_memory: false,
    });

    commands.insert_resource(ShadowPipeline {
        pipeline_id,
        view_layout,
        mesh_layout,
    });
}

/// Prepare dual directional shadow map textures for cameras.
pub fn prepare_directional_shadow_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    cameras: Query<
        Entity,
        (
            With<super::DeferredCamera>,
            Without<ViewDirectionalShadowTextures>,
        ),
    >,
) {
    for entity in cameras.iter() {
        let shadow_textures =
            ViewDirectionalShadowTextures::new(&render_device, &mut texture_cache);
        commands.entity(entity).insert(shadow_textures);
    }
}
