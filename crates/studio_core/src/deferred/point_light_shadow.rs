//! Point light shadow mapping using cube shadow maps.
//!
//! Each shadow-casting point light renders the scene 6 times (one per cube face)
//! from the light's perspective. The resulting cube map stores the distance from
//! the light to the nearest surface in each direction.
//!
//! ## Architecture
//!
//! ```text
//! For each shadow-casting point light:
//!   Render 6 cube faces (+X, -X, +Y, -Y, +Z, -Z)
//!   Each face: 90° FOV perspective projection
//!   Store: linear distance from light
//!
//! In lighting pass:
//!   Sample cube map using direction from light to fragment
//!   Compare fragment distance vs shadow distance
//!   Apply shadow factor to point light contribution
//! ```
//!
//! ## Performance Considerations
//!
//! - Limited to MAX_SHADOW_CASTING_LIGHTS (4-8) to bound render passes
//! - Uses 512x512 per face (lower than directional shadows)
//! - Only nearest lights to camera cast shadows
//! - Lights outside view frustum don't cast shadows

use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingType,
        BufferBindingType, BufferInitDescriptor, BufferUsages,
        CachedRenderPipelineId, CompareFunction, DepthStencilState, Extent3d,
        PipelineCache, PrimitiveState, RenderPipelineDescriptor, SamplerBindingType,
        ShaderStages, StencilState, TextureDescriptor, TextureDimension, TextureFormat,
        TextureSampleType, TextureUsages, TextureViewDimension, VertexState,
    },
    renderer::RenderDevice,
    texture::{CachedTexture, TextureCache},
};

use super::gbuffer_geometry::GBufferVertex;

/// Maximum number of point lights that can cast shadows.
/// Each light requires 6 render passes, so keep this low.
pub const MAX_SHADOW_CASTING_LIGHTS: usize = 4;

/// Resolution of each cube face shadow map.
/// Lower than directional shadows since point lights are typically closer to objects.
pub const POINT_SHADOW_MAP_SIZE: u32 = 512;

/// Depth format for point light shadow maps.
pub const POINT_SHADOW_DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// Marker component for point lights that cast shadows.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CastsShadow;

/// Configuration for a shadow-casting point light.
#[derive(Clone, Debug)]
pub struct ShadowCastingLight {
    /// World position of the light.
    pub position: Vec3,
    /// Light color.
    pub color: Vec3,
    /// Light intensity.
    pub intensity: f32,
    /// Maximum radius.
    pub radius: f32,
    /// Index into shadow cube array (0..MAX_SHADOW_CASTING_LIGHTS).
    pub shadow_index: u32,
}

/// View matrices for rendering all 6 faces of a cube shadow map.
/// 
/// Each face has a 90° FOV perspective projection looking outward
/// from the light position in the corresponding direction.
#[derive(Clone, Copy, Debug)]
pub struct CubeFaceMatrices {
    /// View-projection matrices for each face (+X, -X, +Y, -Y, +Z, -Z).
    pub view_proj: [Mat4; 6],
}

impl CubeFaceMatrices {
    /// Create view-projection matrices for a point light at the given position.
    pub fn new(light_pos: Vec3, near: f32, far: f32) -> Self {
        // Debug: test the full view-projection
        {
            let ground = Vec3::new(0.0, 0.0, 0.0);
            let view = Mat4::look_to_rh(light_pos, Vec3::NEG_Y, Vec3::NEG_Z);
            let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, near, far);
            let view_pos = view.transform_point3(ground);
            let clip = proj * Vec4::new(view_pos.x, view_pos.y, view_pos.z, 1.0);
            let ndc = clip / clip.w;
            bevy::log::info_once!("DEBUG -Y face: ground(0,0,0) -> view {:?} -> clip {:?} -> ndc {:?}", view_pos, clip, ndc);
            bevy::log::info_once!("  clip.w={}, ndc.z={} (should be 0-1 for visible)", clip.w, ndc.z);
        }
        let proj = Mat4::perspective_rh(
            std::f32::consts::FRAC_PI_2, // 90 degrees FOV
            1.0,                          // Square aspect ratio
            near,
            far,
        );
        
        // Define look directions and up vectors for each face.
        // Order: +X, -X, +Y, -Y, +Z, -Z
        // 
        // For cube shadow maps, each face captures what's visible in that direction from the light.
        // look_to_rh(eye, dir, up) creates a view matrix where camera at eye looks in direction dir.
        let faces: [(Vec3, Vec3); 6] = [
            (Vec3::X, Vec3::NEG_Y),     // +X face: look right, up is down
            (Vec3::NEG_X, Vec3::NEG_Y), // -X face: look left, up is down
            (Vec3::Y, Vec3::Z),         // +Y face: look up, up is forward
            (Vec3::NEG_Y, Vec3::NEG_Z), // -Y face: look down, up is backward
            (Vec3::Z, Vec3::NEG_Y),     // +Z face: look forward, up is down
            (Vec3::NEG_Z, Vec3::NEG_Y), // -Z face: look backward, up is down
        ];
        
        let mut view_proj = [Mat4::IDENTITY; 6];
        static DEBUG_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let do_debug = !DEBUG_DONE.swap(true, std::sync::atomic::Ordering::Relaxed);
        
        for (i, (dir, up)) in faces.iter().enumerate() {
            let view = Mat4::look_to_rh(light_pos, *dir, *up);
            view_proj[i] = proj * view;
            
            // Debug: test multiple points through face 3 (-Y) view-proj
            if do_debug && i == 3 {
                for test_point in [
                    Vec3::new(0.0, 0.0, 0.0),   // center
                    Vec3::new(5.0, 0.0, 5.0),   // off-center
                    Vec3::new(-5.0, 1.0, -5.0), // pillar area
                    Vec3::new(0.0, 1.0, 0.0),   // slightly above ground
                ] {
                    let clip = view_proj[i] * Vec4::new(test_point.x, test_point.y, test_point.z, 1.0);
                    let ndc = if clip.w.abs() > 0.001 { clip / clip.w } else { clip };
                    bevy::log::info!("-Y face: point {:?} -> clip {:?}, ndc {:?}", test_point, clip, ndc);
                }
            }
        }
        
        Self { view_proj }
    }
}

/// GPU uniform data for point light shadow rendering.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointShadowUniform {
    /// View-projection matrix for the current cube face.
    pub view_proj: [[f32; 4]; 4],
    /// Light position (xyz) and far plane (w).
    pub light_pos_far: [f32; 4],
}

/// Cube shadow map textures for a single point light.
/// 
/// We use separate 2D textures for each face rather than a cube texture
/// because it's simpler to render to and we're doing our own cube sampling.
pub struct PointLightShadowMap {
    /// Depth textures for each face (+X, -X, +Y, -Y, +Z, -Z).
    pub faces: [CachedTexture; 6],
}

impl PointLightShadowMap {
    /// Create a cube shadow map for a point light.
    pub fn new(
        render_device: &RenderDevice,
        texture_cache: &mut TextureCache,
        light_index: usize,
    ) -> Self {
        let size = Extent3d {
            width: POINT_SHADOW_MAP_SIZE,
            height: POINT_SHADOW_MAP_SIZE,
            depth_or_array_layers: 1,
        };
        
        // Create 6 separate textures for each cube face.
        // IMPORTANT: Use unique descriptors (via unique labels that encode light+face)
        // to prevent texture_cache from returning the same texture for all faces.
        let face_names = ["+X", "-X", "+Y", "-Y", "+Z", "-Z"];
        let faces = std::array::from_fn(|face_idx| {
            // Create a truly unique label including light index and face index
            // This ensures texture_cache treats each as distinct
            let label = format!("point_shadow_L{}_F{}_{}", light_index, face_idx, face_names[face_idx]);
            texture_cache.get(
                render_device,
                TextureDescriptor {
                    label: Some(Box::leak(label.into_boxed_str())),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: POINT_SHADOW_DEPTH_FORMAT,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        });
        
        Self { faces }
    }
}

/// Point shadow map textures for all shadow-casting lights.
#[derive(Component)]
pub struct ViewPointShadowTextures {
    /// Shadow maps for each shadow-casting light.
    pub shadow_maps: Vec<PointLightShadowMap>,
}

impl ViewPointShadowTextures {
    /// Create point shadow textures for the maximum number of shadow-casting lights.
    pub fn new(render_device: &RenderDevice, texture_cache: &mut TextureCache) -> Self {
        let shadow_maps = (0..MAX_SHADOW_CASTING_LIGHTS)
            .map(|i| PointLightShadowMap::new(render_device, texture_cache, i))
            .collect();
        
        Self { shadow_maps }
    }
    
    /// Get the face texture view for a specific light and face.
    pub fn get_face_view(
        &self,
        light_idx: usize,
        face_idx: usize,
    ) -> Option<&bevy::render::render_resource::TextureView> {
        self.shadow_maps
            .get(light_idx)
            .and_then(|sm| sm.faces.get(face_idx))
            .map(|tex| &tex.default_view)
    }
}

/// Pipeline for rendering point light shadow depth.
#[derive(Resource)]
pub struct PointShadowPipeline {
    /// Render pipeline for shadow depth.
    pub pipeline_id: CachedRenderPipelineId,
    /// Bind group layout for view uniforms (view-proj matrix, light pos).
    pub view_layout: BindGroupLayout,
    /// Bind group layout for mesh uniforms (model matrix).
    pub mesh_layout: BindGroupLayout,
}

/// Initialize the point light shadow pipeline.
pub fn init_point_shadow_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<PointShadowPipeline>>,
) {
    if existing.is_some() {
        return;
    }
    
    // View layout: view-projection matrix + light position/far
    let view_layout = render_device.create_bind_group_layout(
        "point_shadow_view_layout",
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    );
    
    // Mesh layout: model transform
    let mesh_layout = render_device.create_bind_group_layout(
        "point_shadow_mesh_layout",
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
    let shader = asset_server.load("shaders/point_shadow_depth.wgsl");
    
    // Create pipeline - outputs linear distance to depth buffer
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("point_shadow_depth_pipeline".into()),
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
            cull_mode: None,  // Disable culling for shadow pass - we want all faces
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(DepthStencilState {
            format: POINT_SHADOW_DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: CompareFunction::LessEqual,
            stencil: StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 0,  // No hardware bias - we compute linear depth ourselves
                slope_scale: 0.0,
                clamp: 0.0,
            },
        }),
        multisample: Default::default(),
        // Fragment shader writes linear distance to frag_depth
        fragment: Some(bevy::render::render_resource::FragmentState {
            shader,
            shader_defs: vec![],
            entry_point: Some("fs_main".into()),
            targets: vec![],  // No color targets, depth-only
        }),
        zero_initialize_workgroup_memory: false,
    });
    
    commands.insert_resource(PointShadowPipeline {
        pipeline_id,
        view_layout,
        mesh_layout,
    });
}

/// Resource holding the list of shadow-casting lights for the current frame.
#[derive(Resource, Default)]
pub struct ShadowCastingLights {
    pub lights: Vec<ShadowCastingLight>,
}

/// Prepare point shadow textures for cameras.
pub fn prepare_point_shadow_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    cameras: Query<Entity, (With<super::DeferredCamera>, Without<ViewPointShadowTextures>)>,
) {
    for entity in cameras.iter() {
        let shadow_textures = ViewPointShadowTextures::new(&render_device, &mut texture_cache);
        commands.entity(entity).insert(shadow_textures);
    }
}

/// Select which point lights will cast shadows.
/// 
/// Selects the nearest MAX_SHADOW_CASTING_LIGHTS lights to the camera
/// that have the CastsShadow component.
pub fn prepare_shadow_casting_lights(
    mut commands: Commands,
    extracted_lights: Option<Res<super::point_light::ExtractedPointLights>>,
    // TODO: Get camera position for distance-based sorting
) {
    let mut shadow_lights = ShadowCastingLights::default();
    
    // For now, just take the first MAX_SHADOW_CASTING_LIGHTS lights
    // TODO: Sort by distance to camera and filter by CastsShadow component
    if let Some(extracted) = extracted_lights {
        for (idx, light) in extracted.lights.iter().enumerate() {
            if idx >= MAX_SHADOW_CASTING_LIGHTS {
                break;
            }
            
            shadow_lights.lights.push(ShadowCastingLight {
                position: light.position,
                color: light.color,
                intensity: light.intensity,
                radius: light.radius,
                shadow_index: idx as u32,
            });
        }
    }
    
    commands.insert_resource(shadow_lights);
}

/// Pre-built bind groups for point shadow rendering.
#[derive(Resource, Default)]
pub struct PointShadowBindGroups {
    /// View bind groups for each light/face combination.
    /// Layout: [light_0_face_0, light_0_face_1, ..., light_0_face_5, light_1_face_0, ...]
    pub view_bind_groups: Vec<BindGroup>,
    /// Mesh bind groups (same as directional shadow pass).
    pub mesh_bind_groups: Vec<BindGroup>,
    /// Fallback mesh bind group for test cube.
    pub fallback_mesh: Option<BindGroup>,
}

/// Prepare bind groups for point shadow rendering.
pub fn prepare_point_shadow_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline: Option<Res<PointShadowPipeline>>,
    shadow_lights: Option<Res<ShadowCastingLights>>,
    geometry_pipeline: Option<Res<super::gbuffer_geometry::GBufferGeometryPipeline>>,
) {
    let Some(pipeline) = pipeline else { return };
    let Some(shadow_lights) = shadow_lights else { return };
    let Some(geometry_pipeline) = geometry_pipeline else { return };
    
    let mut bind_groups = PointShadowBindGroups::default();
    
    // Create view bind groups for each light/face combination
    for light in &shadow_lights.lights {
        bevy::log::info_once!("Shadow render using light at {:?}, radius {}", light.position, light.radius);
        let matrices = CubeFaceMatrices::new(light.position, 0.1, light.radius);
        
        for face_idx in 0..6 {
            let uniform = PointShadowUniform {
                view_proj: matrices.view_proj[face_idx].to_cols_array_2d(),
                light_pos_far: [light.position.x, light.position.y, light.position.z, light.radius],
            };
            
            let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("point_shadow_view_uniform"),
                contents: bytemuck::bytes_of(&uniform),
                usage: BufferUsages::UNIFORM,
            });
            
            let bind_group = render_device.create_bind_group(
                Some("point_shadow_view_bind_group"),
                &pipeline.view_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            );
            
            bind_groups.view_bind_groups.push(bind_group);
        }
    }
    
    // Create mesh bind groups (reuse from geometry pipeline data)
    static MESH_DEBUG_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let do_mesh_debug = !MESH_DEBUG_DONE.swap(true, std::sync::atomic::Ordering::Relaxed);
    
    for mesh_data in &geometry_pipeline.meshes_to_render {
        if do_mesh_debug {
            bevy::log::info!("Point shadow mesh transform: {:?}", mesh_data.transform);
        }
        let mesh_uniform = super::gbuffer_geometry::GBufferMeshUniform {
            world_from_local: mesh_data.transform.to_cols_array_2d(),
            local_from_world: mesh_data.transform.inverse().to_cols_array_2d(),
        };
        
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("point_shadow_mesh_uniform"),
            contents: bytemuck::bytes_of(&mesh_uniform),
            usage: BufferUsages::UNIFORM,
        });
        
        let bind_group = render_device.create_bind_group(
            Some("point_shadow_mesh_bind_group"),
            &pipeline.mesh_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        );
        
        bind_groups.mesh_bind_groups.push(bind_group);
    }
    
    // Create fallback mesh bind group
    let fallback_uniform = super::gbuffer_geometry::GBufferMeshUniform {
        world_from_local: Mat4::IDENTITY.to_cols_array_2d(),
        local_from_world: Mat4::IDENTITY.to_cols_array_2d(),
    };
    
    let fallback_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("point_shadow_fallback_mesh_uniform"),
        contents: bytemuck::bytes_of(&fallback_uniform),
        usage: BufferUsages::UNIFORM,
    });
    
    let fallback = render_device.create_bind_group(
        Some("point_shadow_fallback_mesh_bind_group"),
        &pipeline.mesh_layout,
        &[BindGroupEntry {
            binding: 0,
            resource: fallback_buffer.as_entire_binding(),
        }],
    );
    bind_groups.fallback_mesh = Some(fallback);
    
    commands.insert_resource(bind_groups);
}

/// GPU data for shadow-casting lights, passed to lighting shader.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuShadowLight {
    /// World position (xyz) and shadow index (w as u32 bits).
    pub position_shadow_idx: [f32; 4],
    /// Color (rgb) and intensity (a).
    pub color_intensity: [f32; 4],
    /// Radius (x) and padding.
    pub radius_padding: [f32; 4],
}

/// Storage buffer header for shadow-casting lights.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowLightsHeader {
    /// Number of shadow-casting lights (x) and padding.
    pub count: [u32; 4],
}

/// Create bind group layout entries for point shadow sampling in lighting shader.
pub fn point_shadow_bind_group_layout_entries() -> Vec<BindGroupLayoutEntry> {
    vec![
        // Cube shadow map array
        BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Depth,
                view_dimension: TextureViewDimension::CubeArray,
                multisampled: false,
            },
            count: None,
        },
        // Comparison sampler
        BindGroupLayoutEntry {
            binding: 1,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Sampler(SamplerBindingType::Comparison),
            count: None,
        },
        // Shadow-casting light data
        BindGroupLayoutEntry {
            binding: 2,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
    ]
}
