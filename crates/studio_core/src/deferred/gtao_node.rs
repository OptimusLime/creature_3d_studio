//! GTAO (Ground Truth Ambient Occlusion) render graph node.
//!
//! This node performs a fullscreen pass that computes ambient occlusion
//! using Intel's XeGTAO algorithm - a horizon-based approach that provides
//! ground-truth quality AO with excellent performance.
//!
//! All parameters come from GtaoConfig - NO HARDCODED VALUES.
//!
//! Reference: https://github.com/GameTechDev/XeGTAO

use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
        BufferBindingType, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId,
        ColorTargetState, ColorWrites, Extent3d, FilterMode, FragmentState, LoadOp,
        MultisampleState, Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment,
        RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
        SamplerDescriptor, ShaderStages, StoreOp, TextureDescriptor, TextureDimension,
        TextureFormat, TextureSampleType, TextureUsages, TextureViewDimension, VertexState,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue},
    view::ExtractedView,
};

use super::gbuffer::ViewGBufferTextures;
use super::gtao::{GtaoConfig, ViewGtaoTexture};
use super::gtao_depth_prefilter::ViewDepthMipTextures;

/// GPU uniform for GTAO camera and algorithm parameters.
/// Layout matches the shader's CameraUniforms struct exactly.
/// All vec2s packed into vec4s for proper alignment.
///
/// All values come from GtaoConfig - NO HARDCODED VALUES in this struct.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GtaoCameraUniform {
    /// View matrix (world to view space) - 64 bytes
    pub view: [[f32; 4]; 4],
    /// Projection matrix (view to clip space) - 64 bytes
    pub projection: [[f32; 4]; 4],
    /// Inverse projection matrix (clip to view space) - 64 bytes
    pub inv_projection: [[f32; 4]; 4],
    /// Screen dimensions (width, height, 1/width, 1/height) - 16 bytes
    pub screen_size: [f32; 4],
    /// xy = depth_unpack_consts (depthLinearizeMul, depthLinearizeAdd)
    /// zw = ndc_to_view_mul - 16 bytes
    pub depth_unpack_and_ndc_mul: [f32; 4],
    /// xy = ndc_to_view_add
    /// z = effect_radius, w = effect_falloff_range - 16 bytes
    pub ndc_add_and_params1: [f32; 4],
    /// xy = ndc_to_view_mul_x_pixel_size (XeGTAO L70: NDCToViewMul * ViewportPixelSize)
    /// z = radius_multiplier, w = final_value_power - 16 bytes
    pub ndc_mul_pixel_and_params: [f32; 4],
    /// x = sample_distribution_power, y = thin_occluder_compensation
    /// z = depth_mip_sampling_offset, w = denoise_blur_beta - 16 bytes
    pub params2: [f32; 4],
    /// x = slice_count, y = steps_per_slice, z = unused, w = unused - 16 bytes
    pub params3: [f32; 4],
}
// Total: 64*3 + 16*6 = 192 + 96 = 288 bytes

/// Render graph node that computes GTAO.
#[derive(Default)]
pub struct GtaoPassNode;

impl ViewNode for GtaoPassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ExtractedView,
        &'static ViewGBufferTextures,
        &'static ViewGtaoTexture,
        Option<&'static ViewDepthMipTextures>,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, view, gbuffer, gtao_texture, depth_mips): bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let gtao_pipeline = world.get_resource::<GtaoPipeline>();
        let noise_texture = world.get_resource::<GtaoNoiseTexture>();

        let Some(gtao_pipeline) = gtao_pipeline else {
            return Ok(());
        };

        let Some(pipeline) = pipeline_cache.get_render_pipeline(gtao_pipeline.pipeline_id) else {
            return Ok(());
        };

        let Some(noise_texture) = noise_texture else {
            return Ok(());
        };

        // Get GTAO config from render world (extracted from main world)
        let gtao_config = world
            .get_resource::<GtaoConfig>()
            .cloned()
            .unwrap_or_default();

        // Early out if GTAO is disabled
        if !gtao_config.enabled {
            return Ok(());
        }

        // Create bind group for G-buffer textures (group 0)
        let gbuffer_bind_group = render_context.render_device().create_bind_group(
            "gtao_gbuffer_bind_group",
            &gtao_pipeline.gbuffer_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&gbuffer.normal.default_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&gbuffer.position.default_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&gtao_pipeline.gbuffer_sampler),
                },
                // Hardware depth buffer for proper GTAO depth reconstruction
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&gbuffer.depth.default_view),
                },
            ],
        );

        // Create noise bind group (group 1)
        let noise_bind_group = render_context.render_device().create_bind_group(
            "gtao_noise_bind_group",
            &gtao_pipeline.noise_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&noise_texture.view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&gtao_pipeline.noise_sampler),
                },
            ],
        );

        // Compute matrices
        let world_from_view = view.world_from_view.to_matrix();
        let view_from_world = world_from_view.inverse();
        let projection = view.clip_from_view;
        let inv_projection = projection.inverse();

        let full_screen_size = camera
            .physical_viewport_size
            .unwrap_or(UVec2::new(1920, 1080));
        
        // GTAO renders at half resolution
        let half_screen_size = UVec2::new(
            (full_screen_size.x / 2).max(1),
            (full_screen_size.y / 2).max(1),
        );

        // Bevy uses INFINITE REVERSE-Z projection!
        // For reverse-Z: near maps to depth=1, far maps to depth=0
        // To get linear depth: linear_z = near / ndc_depth
        let proj_cols = projection.to_cols_array_2d();
        let near = proj_cols[3][2];  // Near plane value from projection matrix
        
        // Encoding for shader: linear_z = mul / (add + ndc_depth)
        let depth_linearize_mul = near;
        let depth_linearize_add = 0.0001;  // Small epsilon to prevent div by zero

        // XeGTAO NDC to view-space constants (XeGTAO.h L177-184)
        let tan_half_fov_y = 1.0 / proj_cols[1][1];  // 1/proj[1][1]
        let tan_half_fov_x = 1.0 / proj_cols[0][0];  // 1/proj[0][0]
        
        // NDCToViewMul = { tanHalfFOVX * 2.0, tanHalfFOVY * -2.0 }
        // NDCToViewAdd = { tanHalfFOVX * -1.0, tanHalfFOVY * 1.0 }
        let ndc_to_view_mul = [tan_half_fov_x * 2.0, tan_half_fov_y * -2.0];
        let ndc_to_view_add = [tan_half_fov_x * -1.0, tan_half_fov_y * 1.0];
        
        // XeGTAO.h L184: NDCToViewMul_x_PixelSize = NDCToViewMul * ViewportPixelSize
        let pixel_size = [1.0 / half_screen_size.x as f32, 1.0 / half_screen_size.y as f32];
        let ndc_to_view_mul_x_pixel_size = [
            ndc_to_view_mul[0] * pixel_size[0],
            ndc_to_view_mul[1] * pixel_size[1],
        ];

        // All values from GtaoConfig - NO HARDCODED VALUES
        let camera_uniform = GtaoCameraUniform {
            view: view_from_world.to_cols_array_2d(),
            projection: proj_cols,
            inv_projection: inv_projection.to_cols_array_2d(),
            screen_size: [
                half_screen_size.x as f32,
                half_screen_size.y as f32,
                pixel_size[0],
                pixel_size[1],
            ],
            // Pack: xy = depth_unpack_consts, zw = ndc_to_view_mul
            depth_unpack_and_ndc_mul: [
                depth_linearize_mul,
                depth_linearize_add,
                ndc_to_view_mul[0],
                ndc_to_view_mul[1],
            ],
            // Pack: xy = ndc_to_view_add, z = effect_radius, w = effect_falloff_range
            // FROM CONFIG - not hardcoded
            ndc_add_and_params1: [
                ndc_to_view_add[0],
                ndc_to_view_add[1],
                gtao_config.effect_radius,
                gtao_config.effect_falloff_range,
            ],
            // Pack: xy = ndc_to_view_mul_x_pixel_size, z = radius_multiplier, w = final_value_power
            // XeGTAO.h L70, L184 for NDCToViewMul_x_PixelSize
            ndc_mul_pixel_and_params: [
                ndc_to_view_mul_x_pixel_size[0],
                ndc_to_view_mul_x_pixel_size[1],
                gtao_config.radius_multiplier,
                gtao_config.final_value_power,
            ],
            // Pack: x = sample_dist_power, y = thin_occluder_comp, z = depth_mip_offset, w = denoise_blur_beta
            // FROM CONFIG - not hardcoded
            params2: [
                gtao_config.sample_distribution_power,
                gtao_config.thin_occluder_compensation,
                gtao_config.depth_mip_sampling_offset,
                gtao_config.denoise_blur_beta(),
            ],
            // Pack: x = slice_count, y = steps_per_slice, z = unused, w = unused
            // FROM CONFIG - not hardcoded
            params3: [
                gtao_config.slice_count() as f32,
                gtao_config.steps_per_slice() as f32,
                0.0,
                0.0,
            ],
        };

        let camera_buffer =
            render_context
                .render_device()
                .create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("gtao_camera_buffer"),
                    contents: bytemuck::bytes_of(&camera_uniform),
                    usage: BufferUsages::UNIFORM,
                });

        let camera_bind_group = render_context.render_device().create_bind_group(
            "gtao_camera_bind_group",
            &gtao_pipeline.camera_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        );

        // Create depth MIP bind group if available (group 3)
        let depth_mip_bind_group = depth_mips.map(|mips| {
            render_context.render_device().create_bind_group(
                "gtao_depth_mip_bind_group",
                &gtao_pipeline.depth_mip_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&mips.mip0.default_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&mips.mip1.default_view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::TextureView(&mips.mip2.default_view),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::TextureView(&mips.mip3.default_view),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: BindingResource::TextureView(&mips.mip4.default_view),
                    },
                    BindGroupEntry {
                        binding: 5,
                        resource: BindingResource::Sampler(&gtao_pipeline.depth_mip_sampler),
                    },
                ],
            )
        });

        // Begin render pass writing to GTAO texture
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("gtao_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &gtao_texture.texture.default_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color::WHITE), // Default to fully lit (1.0 = no occlusion)
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Set viewport to half resolution
        render_pass.set_viewport(
            0.0,
            0.0,
            half_screen_size.x as f32,
            half_screen_size.y as f32,
            0.0,
            1.0,
        );

        // Draw fullscreen triangle
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &gbuffer_bind_group, &[]);
        render_pass.set_bind_group(1, &noise_bind_group, &[]);
        render_pass.set_bind_group(2, &camera_bind_group, &[]);
        if let Some(ref depth_mip_bg) = depth_mip_bind_group {
            render_pass.set_bind_group(3, depth_mip_bg, &[]);
        }
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

/// Pipeline resources for GTAO.
#[derive(Resource)]
pub struct GtaoPipeline {
    pub pipeline_id: CachedRenderPipelineId,
    /// G-buffer textures layout (group 0)
    pub gbuffer_layout: BindGroupLayout,
    /// G-buffer sampler
    pub gbuffer_sampler: Sampler,
    /// Noise texture layout (group 1)
    pub noise_layout: BindGroupLayout,
    /// Noise sampler (repeating)
    pub noise_sampler: Sampler,
    /// Camera matrices layout (group 2)
    pub camera_layout: BindGroupLayout,
    /// Depth MIP chain layout (group 3) - optional, for XeGTAO MIP sampling
    pub depth_mip_layout: BindGroupLayout,
    /// Depth MIP sampler (linear filtering for MIP sampling)
    pub depth_mip_sampler: Sampler,
}

/// Noise texture for GTAO slice direction randomization.
#[derive(Resource)]
pub struct GtaoNoiseTexture {
    #[allow(dead_code)]
    pub texture: bevy::render::render_resource::Texture,
    pub view: bevy::render::render_resource::TextureView,
}

/// System to initialize the GTAO pipeline.
pub fn init_gtao_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<GtaoPipeline>>,
) {
    if existing.is_some() {
        return;
    }

    // Group 0: G-buffer textures (normal, position, depth, sampler)
    let gbuffer_layout = render_device.create_bind_group_layout(
        "gtao_gbuffer_layout",
        &[
            // Normal texture
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Position texture (still needed for world-space normal transform)
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Sampler for float textures
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                count: None,
            },
            // Hardware depth buffer (Depth32Float) - for proper GTAO depth reconstruction
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    );

    // Group 1: Noise texture
    let noise_layout = render_device.create_bind_group_layout(
        "gtao_noise_layout",
        &[
            // Noise texture
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Noise sampler
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    );

    // Group 2: Camera matrices
    let camera_layout = render_device.create_bind_group_layout(
        "gtao_camera_layout",
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    );

    // Group 3: Depth MIP chain (5 levels of pre-filtered viewspace depth)
    let depth_mip_layout = render_device.create_bind_group_layout(
        "gtao_depth_mip_layout",
        &[
            // MIP 0 (full resolution)
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // MIP 1 (half resolution)
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // MIP 2 (quarter resolution)
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // MIP 3 (1/8 resolution)
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // MIP 4 (1/16 resolution)
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // Depth MIP sampler
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    );

    // Create samplers
    let gbuffer_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("gtao_gbuffer_sampler"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..default()
    });

    let noise_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("gtao_noise_sampler"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        address_mode_u: bevy::render::render_resource::AddressMode::Repeat,
        address_mode_v: bevy::render::render_resource::AddressMode::Repeat,
        ..default()
    });

    // Depth MIP sampler - uses linear filtering for smooth MIP sampling
    let depth_mip_sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("gtao_depth_mip_sampler"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });

    // Load shader
    let shader = asset_server.load("shaders/gtao.wgsl");

    // Queue pipeline creation
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("gtao_pipeline".into()),
        layout: vec![
            gbuffer_layout.clone(),
            noise_layout.clone(),
            camera_layout.clone(),
            depth_mip_layout.clone(),
        ],
        push_constant_ranges: vec![],
        vertex: VertexState {
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Some("vs_main".into()),
            buffers: vec![],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            shader,
            shader_defs: vec![],
            entry_point: Some("fs_main".into()),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::R8Unorm,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        zero_initialize_workgroup_memory: false,
    });

    commands.insert_resource(GtaoPipeline {
        pipeline_id,
        gbuffer_layout,
        gbuffer_sampler,
        noise_layout,
        noise_sampler,
        camera_layout,
        depth_mip_layout,
        depth_mip_sampler,
    });
}

/// Noise texture size for slice direction randomization
const NOISE_TEXTURE_SIZE: u32 = 32;

/// System to create the noise texture with random direction vectors.
pub fn init_gtao_noise_texture(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    existing: Option<Res<GtaoNoiseTexture>>,
) {
    if existing.is_some() {
        return;
    }

    use rand::prelude::*;
    let mut rng = rand::thread_rng();

    // Generate 32x32 noise texture with random angles for slice direction rotation
    let pixel_count = (NOISE_TEXTURE_SIZE * NOISE_TEXTURE_SIZE) as usize;
    let mut noise_data = Vec::with_capacity(pixel_count * 4); // RGBA8

    for _ in 0..pixel_count {
        // Two independent random values in [0, 1] for:
        // R: Slice direction offset (rotates which directions we sample)
        // G: Sample step offset (jitters sample positions along slice)
        let slice_noise: f32 = rng.gen();   // [0, 1]
        let sample_noise: f32 = rng.gen();  // [0, 1]

        // Store as RGBA8: [0,1] -> [0,255]
        noise_data.push((slice_noise * 255.0) as u8);
        noise_data.push((sample_noise * 255.0) as u8);
        noise_data.push(128); // Z unused
        noise_data.push(255); // A = 1
    }

    // Create the texture
    let texture = render_device.create_texture(&TextureDescriptor {
        label: Some("gtao_noise_texture"),
        size: Extent3d {
            width: NOISE_TEXTURE_SIZE,
            height: NOISE_TEXTURE_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Write data to texture
    render_queue.0.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &noise_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * NOISE_TEXTURE_SIZE),
            rows_per_image: Some(NOISE_TEXTURE_SIZE),
        },
        wgpu::Extent3d {
            width: NOISE_TEXTURE_SIZE,
            height: NOISE_TEXTURE_SIZE,
            depth_or_array_layers: 1,
        },
    );

    // Create texture view
    let view = texture.create_view(&bevy::render::render_resource::TextureViewDescriptor::default());

    commands.insert_resource(GtaoNoiseTexture { texture, view });
}
