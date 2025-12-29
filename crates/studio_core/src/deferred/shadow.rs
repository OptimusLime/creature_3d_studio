//! Shadow mapping for deferred rendering.
//!
//! Implements directional shadow mapping for the sun light:
//! 1. Render scene from light's perspective to depth texture
//! 2. Sample shadow map in lighting pass to determine shadow visibility
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

/// Shadow mapping configuration.
#[derive(Resource, Clone)]
pub struct ShadowConfig {
    /// Sun direction (normalized, pointing FROM the sun TO the scene).
    /// Default: (0.3, -0.9, -0.3) - mostly from above, slightly from back-right.
    pub sun_direction: Vec3,
    
    /// Size of the orthographic shadow frustum (half-width in world units).
    /// Objects within [-shadow_size, shadow_size] from the frustum center are shadowed.
    pub shadow_size: f32,
    
    /// Near plane for shadow frustum.
    pub near: f32,
    
    /// Far plane for shadow frustum.
    pub far: f32,
    
    /// Depth bias to prevent shadow acne.
    /// Applied as constant + slope-scaled bias.
    pub depth_bias_constant: f32,
    pub depth_bias_slope: f32,
    
    /// PCF (Percentage Closer Filtering) kernel size.
    /// 1 = no filtering (hard shadows), 3 = 3x3 PCF, 5 = 5x5 PCF
    pub pcf_kernel_size: u32,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self::dark_world()  // Default to dark world mode
    }
}

impl ShadowConfig {
    /// Classic sun configuration (bright day scene).
    pub fn sun() -> Self {
        Self {
            // Match the sun direction from deferred_lighting.wgsl
            sun_direction: Vec3::new(0.3, -0.9, -0.3).normalize(),
            shadow_size: 50.0,  // Covers a 100x100 world unit area
            near: 0.1,
            far: 200.0,
            depth_bias_constant: 0.0,  // GPU-level bias (applied in rasterizer)
            depth_bias_slope: 0.0,     // We use shader-based bias instead
            pcf_kernel_size: 3,        // 3x3 PCF for soft shadows
        }
    }
    
    /// Dark world purple moon configuration.
    /// Matches MOON1_DIRECTION in deferred_lighting.wgsl.
    pub fn dark_world() -> Self {
        Self {
            // Purple moon direction: back-left, moderate height
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
    /// 
    /// The camera is positioned along the light direction, looking at the scene center.
    pub fn light_view_matrix(&self, scene_center: Vec3) -> Mat4 {
        // Position the light camera far enough back along the light direction
        let light_distance = self.shadow_size * 2.0;
        let light_pos = scene_center - self.sun_direction * light_distance;
        
        // Create look-at matrix: camera at light_pos, looking toward scene_center
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

/// Shadow map textures for a camera view.
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

/// Shadow map uniform data passed to shadow depth shader.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowViewUniform {
    /// Light-space view-projection matrix.
    pub light_view_proj: [[f32; 4]; 4],
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
    
    info!("ShadowPipeline initialized ({}x{} shadow map)", SHADOW_MAP_SIZE, SHADOW_MAP_SIZE);
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
        
        info!(
            "Created shadow map texture for camera {:?} ({}x{})",
            entity, SHADOW_MAP_SIZE, SHADOW_MAP_SIZE
        );
    }
}
