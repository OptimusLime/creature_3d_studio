//! G-Buffer geometry rendering.
//!
//! This module provides the infrastructure to render mesh geometry to the G-buffer.
//! It extracts mesh data from the main world and renders it using our custom MRT pipeline.

use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry,
        BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferInitDescriptor,
        BufferUsages, CachedRenderPipelineId, ColorTargetState, ColorWrites, CompareFunction,
        DepthStencilState, FragmentState, MultisampleState, PipelineCache, PrimitiveState,
        RenderPipelineDescriptor, ShaderStages, StencilState, TextureFormat,
        VertexState,
    },
    renderer::{RenderDevice, RenderQueue},
};
use bytemuck::{Pod, Zeroable};
use bevy_mesh::{VertexBufferLayout, VertexFormat};

use super::gbuffer::{GBUFFER_COLOR_FORMAT, GBUFFER_NORMAL_FORMAT, GBUFFER_POSITION_FORMAT};

/// Vertex format for G-buffer rendering.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct GBufferVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub emission: f32,
}

impl GBufferVertex {
    pub fn vertex_buffer_layout() -> VertexBufferLayout {
        VertexBufferLayout::from_vertex_formats(
            wgpu::VertexStepMode::Vertex,
            [
                VertexFormat::Float32x3, // Position
                VertexFormat::Float32x3, // Normal
                VertexFormat::Float32x3, // Color
                VertexFormat::Float32,   // Emission
            ],
        )
    }
}

/// View uniform data for the G-buffer pass.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct GBufferViewUniform {
    pub view_proj: [[f32; 4]; 4],
    pub inverse_view_proj: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub inverse_view: [[f32; 4]; 4],
    pub projection: [[f32; 4]; 4],
    pub inverse_projection: [[f32; 4]; 4],
    pub world_position: [f32; 3],
    pub _padding: f32,
    pub viewport: [f32; 4],
}

/// Mesh uniform data for transforms.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct GBufferMeshUniform {
    pub world_from_local: [[f32; 4]; 4],
    pub local_from_world: [[f32; 4]; 4],
}

/// Resource containing G-buffer geometry pipeline and buffers.
#[derive(Resource)]
pub struct GBufferGeometryPipeline {
    pub pipeline_id: CachedRenderPipelineId,
    pub view_layout: BindGroupLayout,
    pub mesh_layout: BindGroupLayout,
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub view_uniform_buffer: Buffer,
    pub mesh_uniform_buffer: Buffer,
    pub view_bind_group: Option<BindGroup>,
    pub mesh_bind_group: Option<BindGroup>,
    pub index_count: u32,
}

/// Generate a test cube mesh.
fn generate_test_cube() -> (Vec<GBufferVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Cube from -1 to +1 in all axes (2x2x2 cube)
    let size = 1.0f32;
    
    // Define colors for each face (for debugging)
    let colors: [[f32; 3]; 6] = [
        [1.0, 0.3, 0.3], // +X: red-ish
        [0.3, 1.0, 0.3], // -X: green-ish
        [0.3, 0.3, 1.0], // +Y: blue-ish
        [1.0, 1.0, 0.3], // -Y: yellow-ish
        [1.0, 0.3, 1.0], // +Z: magenta-ish
        [0.3, 1.0, 1.0], // -Z: cyan-ish
    ];
    
    let emission = 0.3; // Some emission for all faces

    // Face definitions: normal, then 4 corners
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        // +X
        ([1.0, 0.0, 0.0], [
            [size, -size, -size],
            [size, size, -size],
            [size, size, size],
            [size, -size, size],
        ]),
        // -X
        ([-1.0, 0.0, 0.0], [
            [-size, -size, size],
            [-size, size, size],
            [-size, size, -size],
            [-size, -size, -size],
        ]),
        // +Y
        ([0.0, 1.0, 0.0], [
            [-size, size, -size],
            [-size, size, size],
            [size, size, size],
            [size, size, -size],
        ]),
        // -Y
        ([0.0, -1.0, 0.0], [
            [-size, -size, size],
            [-size, -size, -size],
            [size, -size, -size],
            [size, -size, size],
        ]),
        // +Z
        ([0.0, 0.0, 1.0], [
            [-size, -size, size],
            [size, -size, size],
            [size, size, size],
            [-size, size, size],
        ]),
        // -Z
        ([0.0, 0.0, -1.0], [
            [size, -size, -size],
            [-size, -size, -size],
            [-size, size, -size],
            [size, size, -size],
        ]),
    ];

    for (face_idx, (normal, corners)) in faces.iter().enumerate() {
        let base_index = vertices.len() as u32;
        let color = colors[face_idx];

        for corner in corners {
            vertices.push(GBufferVertex {
                position: *corner,
                normal: *normal,
                color,
                emission,
            });
        }

        // Two triangles per face
        indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);
    }

    (vertices, indices)
}

/// System to initialize the G-buffer geometry pipeline.
pub fn init_gbuffer_geometry_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<GBufferGeometryPipeline>>,
) {
    if existing.is_some() {
        return;
    }

    // Create bind group layouts
    let view_layout = render_device.create_bind_group_layout(
        "gbuffer_view_layout",
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    );

    let mesh_layout = render_device.create_bind_group_layout(
        "gbuffer_mesh_layout",
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

    // Generate test cube
    let (vertices, indices) = generate_test_cube();
    let index_count = indices.len() as u32;

    // Create buffers
    let vertex_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("gbuffer_vertex_buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: BufferUsages::VERTEX,
    });

    let index_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("gbuffer_index_buffer"),
        contents: bytemuck::cast_slice(&indices),
        usage: BufferUsages::INDEX,
    });

    // Create uniform buffers (will be filled each frame)
    let view_uniform_buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("gbuffer_view_uniform"),
        size: std::mem::size_of::<GBufferViewUniform>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mesh_uniform_buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("gbuffer_mesh_uniform"),
        size: std::mem::size_of::<GBufferMeshUniform>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Load shader
    let shader = asset_server.load("shaders/gbuffer.wgsl");

    // Create pipeline
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("gbuffer_geometry_pipeline".into()),
        layout: vec![view_layout.clone(), mesh_layout.clone()],
        push_constant_ranges: vec![],
        vertex: VertexState {
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Some("vertex".into()),
            buffers: vec![GBufferVertex::vertex_buffer_layout()],
        },
        primitive: PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: CompareFunction::GreaterEqual, // Reverse-Z
            stencil: StencilState::default(),
            bias: Default::default(),
        }),
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            shader,
            shader_defs: vec![],
            entry_point: Some("fragment".into()),
            targets: vec![
                Some(ColorTargetState {
                    format: GBUFFER_COLOR_FORMAT,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                }),
                Some(ColorTargetState {
                    format: GBUFFER_NORMAL_FORMAT,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                }),
                Some(ColorTargetState {
                    format: GBUFFER_POSITION_FORMAT,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                }),
            ],
        }),
        zero_initialize_workgroup_memory: false,
    });

    commands.insert_resource(GBufferGeometryPipeline {
        pipeline_id,
        view_layout,
        mesh_layout,
        vertex_buffer,
        index_buffer,
        view_uniform_buffer,
        mesh_uniform_buffer,
        view_bind_group: None,
        mesh_bind_group: None,
        index_count,
    });

    info!("GBufferGeometryPipeline initialized with test cube");
}

/// System to update G-buffer uniforms each frame.
pub fn update_gbuffer_uniforms(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Option<ResMut<GBufferGeometryPipeline>>,
) {
    let Some(mut pipeline) = pipeline else {
        return;
    };
    // Create view uniform with simple camera looking at origin
    let eye = Vec3::new(0.0, 5.0, 10.0);
    let center = Vec3::ZERO;
    let up = Vec3::Y;
    
    let view = Mat4::look_at_rh(eye, center, up);
    let projection = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 800.0 / 600.0, 0.1, 1000.0);
    let view_proj = projection * view;
    
    let view_uniform = GBufferViewUniform {
        view_proj: view_proj.to_cols_array_2d(),
        inverse_view_proj: view_proj.inverse().to_cols_array_2d(),
        view: view.to_cols_array_2d(),
        inverse_view: view.inverse().to_cols_array_2d(),
        projection: projection.to_cols_array_2d(),
        inverse_projection: projection.inverse().to_cols_array_2d(),
        world_position: eye.to_array(),
        _padding: 0.0,
        viewport: [0.0, 0.0, 800.0, 600.0],
    };
    
    render_queue.write_buffer(
        &pipeline.view_uniform_buffer,
        0,
        bytemuck::bytes_of(&view_uniform),
    );

    // Create mesh uniform (identity transform for now)
    let mesh_uniform = GBufferMeshUniform {
        world_from_local: Mat4::IDENTITY.to_cols_array_2d(),
        local_from_world: Mat4::IDENTITY.to_cols_array_2d(),
    };
    
    render_queue.write_buffer(
        &pipeline.mesh_uniform_buffer,
        0,
        bytemuck::bytes_of(&mesh_uniform),
    );

    // Create bind groups
    let view_bind_group = render_device.create_bind_group(
        "gbuffer_view_bind_group",
        &pipeline.view_layout,
        &[BindGroupEntry {
            binding: 0,
            resource: pipeline.view_uniform_buffer.as_entire_binding(),
        }],
    );

    let mesh_bind_group = render_device.create_bind_group(
        "gbuffer_mesh_bind_group",
        &pipeline.mesh_layout,
        &[BindGroupEntry {
            binding: 0,
            resource: pipeline.mesh_uniform_buffer.as_entire_binding(),
        }],
    );

    pipeline.view_bind_group = Some(view_bind_group);
    pipeline.mesh_bind_group = Some(mesh_bind_group);
}
