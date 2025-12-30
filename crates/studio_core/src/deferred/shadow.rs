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
        CachedRenderPipelineId, CompareFunction, DepthStencilState, Extent3d,
        PipelineCache, PrimitiveState, RenderPipelineDescriptor, ShaderStages,
        StencilState, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        VertexState,
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
            moon1_color: Vec3::new(0.5, 0.2, 0.9),  // Rich purple
            moon1_intensity: 0.5,  // Bright enough to see clearly
            
            // Orange moon - from the right side at moderate height
            // Shadows cast to the left
            moon2_direction: Vec3::new(-0.7, -0.5, -0.3).normalize(),
            moon2_color: Vec3::new(1.0, 0.5, 0.15),  // Warm orange
            moon2_intensity: 0.45,  // Slightly dimmer to prevent washout
            
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
    fn light_view_matrix(&self, direction: Vec3, scene_center: Vec3) -> Mat4 {
        let light_distance = self.shadow_size * 2.0;
        let light_pos = scene_center - direction * light_distance;
        Mat4::look_at_rh(light_pos, scene_center, Vec3::Y)
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

// === Legacy ShadowConfig for backward compatibility ===
// TODO: Remove once all code migrated to MoonConfig

/// Shadow mapping configuration (legacy - use MoonConfig instead).
#[derive(Resource, Clone)]
pub struct ShadowConfig {
    /// Sun direction (normalized, pointing FROM the sun TO the scene).
    pub sun_direction: Vec3,
    /// Size of the orthographic shadow frustum (half-width in world units).
    pub shadow_size: f32,
    /// Near plane for shadow frustum.
    pub near: f32,
    /// Far plane for shadow frustum.
    pub far: f32,
    /// Depth bias to prevent shadow acne.
    pub depth_bias_constant: f32,
    pub depth_bias_slope: f32,
    /// PCF kernel size.
    pub pcf_kernel_size: u32,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self::dark_world()
    }
}

impl ShadowConfig {
    /// Classic sun configuration.
    pub fn sun() -> Self {
        Self {
            sun_direction: Vec3::new(0.3, -0.9, -0.3).normalize(),
            shadow_size: 50.0,
            near: 0.1,
            far: 200.0,
            depth_bias_constant: 0.0,
            depth_bias_slope: 0.0,
            pcf_kernel_size: 3,
        }
    }
    
    /// Dark world purple moon configuration.
    pub fn dark_world() -> Self {
        Self {
            sun_direction: Vec3::new(0.6, -0.6, 0.55).normalize(),
            shadow_size: 50.0,
            near: 0.1,
            far: 200.0,
            depth_bias_constant: 0.0,
            depth_bias_slope: 0.0,
            pcf_kernel_size: 3,
        }
    }
}

impl ShadowConfig {    
    /// Calculate the light-space view matrix.
    pub fn light_view_matrix(&self, scene_center: Vec3) -> Mat4 {
        let light_distance = self.shadow_size * 2.0;
        let light_pos = scene_center - self.sun_direction * light_distance;
        Mat4::look_at_rh(light_pos, scene_center, Vec3::Y)
    }
    
    /// Calculate the light-space orthographic projection matrix.
    pub fn light_projection_matrix(&self) -> Mat4 {
        Mat4::orthographic_rh(
            -self.shadow_size,
            self.shadow_size,
            -self.shadow_size,
            self.shadow_size,
            self.near,
            self.far,
        )
    }
    
    /// Calculate the combined light-space view-projection matrix.
    pub fn light_view_projection(&self, scene_center: Vec3) -> Mat4 {
        self.light_projection_matrix() * self.light_view_matrix(scene_center)
    }
}

/// Shadow map textures for a camera view (legacy single shadow map).
#[derive(Component)]
pub struct ViewShadowTextures {
    /// Depth texture for shadow map rendering.
    pub depth: CachedTexture,
    /// Size of the shadow map.
    pub size: Extent3d,
}

impl ViewShadowTextures {
    /// Create shadow map texture for shadow rendering.
    pub fn new(
        render_device: &RenderDevice,
        texture_cache: &mut TextureCache,
    ) -> Self {
        let size = Extent3d {
            width: SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
            depth_or_array_layers: 1,
        };
        
        let depth = texture_cache.get(
            render_device,
            TextureDescriptor {
                label: Some("shadow_depth_texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: SHADOW_DEPTH_FORMAT,
                // RENDER_ATTACHMENT for writing depth
                // TEXTURE_BINDING for sampling in lighting pass
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        
        Self { depth, size }
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
    pub fn new(
        render_device: &RenderDevice,
        texture_cache: &mut TextureCache,
    ) -> Self {
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
    /// Shadow softness: x = directional, y = point, zw = unused.
    pub shadow_softness: [f32; 4],
}

impl DirectionalShadowUniforms {
    /// Create uniforms from MoonConfig.
    pub fn from_config(config: &MoonConfig, scene_center: Vec3) -> Self {
        Self {
            moon1_view_proj: config.moon1_view_projection(scene_center).to_cols_array_2d(),
            moon2_view_proj: config.moon2_view_projection(scene_center).to_cols_array_2d(),
            moon1_direction: [config.moon1_direction.x, config.moon1_direction.y, config.moon1_direction.z, 0.0],
            moon1_color_intensity: [config.moon1_color.x, config.moon1_color.y, config.moon1_color.z, config.moon1_intensity],
            moon2_direction: [config.moon2_direction.x, config.moon2_direction.y, config.moon2_direction.z, 0.0],
            moon2_color_intensity: [config.moon2_color.x, config.moon2_color.y, config.moon2_color.z, config.moon2_intensity],
            shadow_softness: [config.directional_shadow_softness, config.point_shadow_softness, 0.0, 0.0],
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
                constant: 2,   // Small constant bias
                slope_scale: 2.0,  // Slope-scaled bias for angled surfaces
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

/// Prepare shadow map textures for cameras.
pub fn prepare_shadow_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    cameras: Query<Entity, (With<super::DeferredCamera>, Without<ViewShadowTextures>)>,
) {
    for entity in cameras.iter() {
        let shadow_textures = ViewShadowTextures::new(&render_device, &mut texture_cache);
        
        commands.entity(entity).insert(shadow_textures);
    }
}

/// Prepare dual directional shadow map textures for cameras.
pub fn prepare_directional_shadow_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    cameras: Query<Entity, (With<super::DeferredCamera>, Without<ViewDirectionalShadowTextures>)>,
) {
    for entity in cameras.iter() {
        let shadow_textures = ViewDirectionalShadowTextures::new(&render_device, &mut texture_cache);
        commands.entity(entity).insert(shadow_textures);
    }
}
