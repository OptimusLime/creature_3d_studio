//! G-Buffer Material - renders geometry to multiple render targets.
//!
//! This module provides a custom render pipeline that outputs to the G-buffer:
//! - gColor (Rgba16Float): RGB = albedo, A = emission
//! - gNormal (Rgba16Float): RGB = world-space normal
//! - gPosition (Rgba32Float): XYZ = world position, W = linear depth

use bevy::asset::AssetServer;
use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        BindGroupLayout, BindGroupLayoutEntry, BindingType, BufferBindingType, ColorTargetState,
        ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, FragmentState, FrontFace,
        MultisampleState, PolygonMode, PrimitiveState, RenderPipelineDescriptor, ShaderStages,
        SpecializedMeshPipeline, SpecializedMeshPipelineError, StencilState, TextureFormat,
        VertexState,
    },
    renderer::RenderDevice,
};
use bevy_mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexFormat};

use super::gbuffer::{GBUFFER_COLOR_FORMAT, GBUFFER_NORMAL_FORMAT, GBUFFER_POSITION_FORMAT};

/// Resource holding the G-buffer render pipeline configuration.
#[derive(Resource)]
pub struct GBufferPipeline {
    /// Bind group layout for view uniforms (group 0)
    pub view_layout: BindGroupLayout,
    /// Bind group layout for mesh uniforms (group 1)  
    pub mesh_layout: BindGroupLayout,
    /// The G-buffer shader
    pub shader: Handle<Shader>,
}

impl GBufferPipeline {
    /// Create the G-buffer pipeline resource.
    pub fn new(render_device: &RenderDevice, asset_server: &AssetServer) -> Self {
        // View bind group layout (group 0) - view uniforms
        let view_layout = render_device.create_bind_group_layout(
            "gbuffer_view_layout",
            &[
                // View uniform buffer
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        // Mesh bind group layout (group 1) - per-instance transforms
        let mesh_layout = render_device.create_bind_group_layout(
            "gbuffer_mesh_layout",
            &[
                // Mesh uniform buffer (transforms)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        // Load the G-buffer shader
        let shader = asset_server.load("shaders/gbuffer.wgsl");

        Self {
            view_layout,
            mesh_layout,
            shader,
        }
    }
}

/// Key for specializing the G-buffer pipeline based on mesh properties.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct GBufferPipelineKey {
    /// Whether the mesh has vertex colors
    pub has_vertex_colors: bool,
    /// Primitive topology (triangles, lines, etc.)
    pub primitive_topology: PrimitiveTopology,
}

impl Default for GBufferPipelineKey {
    fn default() -> Self {
        Self {
            has_vertex_colors: true,
            primitive_topology: PrimitiveTopology::TriangleList,
        }
    }
}

impl SpecializedMeshPipeline for GBufferPipeline {
    type Key = GBufferPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        // Define vertex attributes based on our voxel vertex format
        let mut vertex_attributes = vec![];

        // Position (required)
        vertex_attributes.push(Mesh::ATTRIBUTE_POSITION.at_shader_location(0));

        // Normal (required for G-buffer)
        vertex_attributes.push(Mesh::ATTRIBUTE_NORMAL.at_shader_location(1));

        // Our custom voxel attributes - color and emission
        // These are custom attributes defined in voxel_mesh.rs
        let voxel_color_attr =
            MeshVertexAttribute::new("Voxel_Color", 988540918, VertexFormat::Float32x3);
        let voxel_emission_attr =
            MeshVertexAttribute::new("Voxel_Emission", 988540919, VertexFormat::Float32);

        vertex_attributes.push(voxel_color_attr.at_shader_location(2));
        vertex_attributes.push(voxel_emission_attr.at_shader_location(3));

        let vertex_buffer_layout = layout.0.get_layout(&vertex_attributes)?;

        // G-buffer outputs - 3 color targets (MRT)
        let targets = vec![
            // @location(0): gColor - RGB = albedo, A = emission
            Some(ColorTargetState {
                format: GBUFFER_COLOR_FORMAT,
                blend: None,
                write_mask: ColorWrites::ALL,
            }),
            // @location(1): gNormal - RGB = world normal
            Some(ColorTargetState {
                format: GBUFFER_NORMAL_FORMAT,
                blend: None,
                write_mask: ColorWrites::ALL,
            }),
            // @location(2): gPosition - XYZ = world pos, W = depth
            Some(ColorTargetState {
                format: GBUFFER_POSITION_FORMAT,
                blend: None,
                write_mask: ColorWrites::ALL,
            }),
        ];

        Ok(RenderPipelineDescriptor {
            label: Some("gbuffer_pipeline".into()),
            layout: vec![self.view_layout.clone(), self.mesh_layout.clone()],
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: vec![vertex_buffer_layout],
            },
            primitive: PrimitiveState {
                topology: key.primitive_topology,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(bevy::render::render_resource::Face::Back),
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::GreaterEqual, // Reverse-Z
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets,
            }),
            zero_initialize_workgroup_memory: false,
        })
    }
}

/// System to initialize the G-buffer pipeline.
pub fn init_gbuffer_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<GBufferPipeline>>,
) {
    if existing.is_some() {
        return;
    }

    let pipeline = GBufferPipeline::new(&render_device, &asset_server);
    commands.insert_resource(pipeline);
}
