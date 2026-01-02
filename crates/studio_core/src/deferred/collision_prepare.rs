//! GPU collision preparation systems.
//!
//! This module uploads terrain occupancy and fragment data to the GPU
//! for the collision compute shader.

use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        BindGroup, BindGroupEntry, BindingResource, Buffer,
        BufferInitDescriptor, BufferUsages,
    },
    renderer::{RenderDevice, RenderQueue},
};

use super::collision_extract::{ExtractedFragments, ExtractedTerrainChunks};
use crate::voxel_collision_gpu::{
    CollisionUniforms, GpuCollisionPipeline, GpuFragmentData, GpuWorldOccupancy,
    MAX_FRAGMENT_OCCUPANCY_U32S, MAX_GPU_CHUNKS, MAX_GPU_CONTACTS,
};

/// Resource tracking whether GPU collision is initialized.
#[derive(Resource, Default)]
pub struct GpuCollisionState {
    /// Whether the GPU occupancy has been initialized
    pub initialized: bool,
    /// Number of fragments uploaded this frame
    pub fragment_count: u32,
}

/// Bind groups for the collision compute shader.
#[derive(Resource)]
pub struct CollisionBindGroups {
    /// Group 0: World occupancy (textures + index buffer)
    pub occupancy_bind_group: BindGroup,
    /// Group 1: Fragment data + output contacts
    pub fragment_bind_group: BindGroup,
    /// Group 2: Uniforms
    pub uniform_bind_group: BindGroup,
}

/// Buffer holding fragment data for the current frame.
#[derive(Resource)]
pub struct CollisionFragmentBuffer {
    pub buffer: Buffer,
    pub count: u32,
}

/// System to initialize GPU world occupancy.
/// Runs once when terrain is first extracted.
pub fn init_gpu_occupancy(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    existing: Option<Res<GpuWorldOccupancy>>,
) {
    if existing.is_some() {
        return;
    }

    info!("Initializing GPU world occupancy with {} chunk capacity", MAX_GPU_CHUNKS);
    let gpu_occupancy = GpuWorldOccupancy::new(&render_device, MAX_GPU_CHUNKS);
    commands.insert_resource(gpu_occupancy);
    commands.insert_resource(GpuCollisionState::default());
}

/// System to upload terrain chunks to GPU.
/// Only uploads when terrain is dirty (changed since last frame).
pub fn upload_terrain_to_gpu(
    render_queue: Res<RenderQueue>,
    extracted_terrain: Res<ExtractedTerrainChunks>,
    mut gpu_occupancy: Option<ResMut<GpuWorldOccupancy>>,
    mut collision_state: Option<ResMut<GpuCollisionState>>,
) {
    let Some(gpu_occupancy) = gpu_occupancy.as_mut() else {
        return;
    };
    let Some(collision_state) = collision_state.as_mut() else {
        return;
    };

    // Only upload if terrain is dirty
    if !extracted_terrain.dirty {
        return;
    }

    info!(
        "Uploading {} terrain chunks to GPU",
        extracted_terrain.chunks.len()
    );

    for (coord, chunk_occ) in &extracted_terrain.chunks {
        if let Some(layer) = gpu_occupancy.upload_chunk(&render_queue, *coord, chunk_occ) {
            trace!("Uploaded chunk {:?} to GPU layer {}", coord, layer);
        } else {
            warn!("Failed to upload chunk {:?} - no free GPU layers", coord);
        }
    }

    collision_state.initialized = true;
}

/// System to prepare fragment data for GPU collision.
/// Creates the fragment buffer and bind groups for the compute shader.
pub fn prepare_collision_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    extracted_fragments: Res<ExtractedFragments>,
    gpu_occupancy: Option<Res<GpuWorldOccupancy>>,
    collision_pipeline: Option<Res<GpuCollisionPipeline>>,
    mut collision_state: Option<ResMut<GpuCollisionState>>,
) {
    let Some(gpu_occupancy) = gpu_occupancy else {
        return;
    };
    let Some(collision_pipeline) = collision_pipeline else {
        return;
    };
    let Some(collision_state) = collision_state.as_mut() else {
        return;
    };

    // Skip if no fragments to process
    if extracted_fragments.fragments.is_empty() {
        collision_state.fragment_count = 0;
        return;
    }

    // Build fragment occupancy buffer:
    // Layout: For each fragment, store occupancy data consecutively
    // Fragment i's data starts at occupancy_offset stored in GpuFragmentData
    let mut occupancy_buffer_data: Vec<u32> = Vec::new();
    let mut gpu_fragments: Vec<GpuFragmentData> = Vec::with_capacity(extracted_fragments.fragments.len());
    
    for (idx, frag) in extracted_fragments.fragments.iter().enumerate() {
        let occupancy_offset = occupancy_buffer_data.len() as u32;
        let occupancy_data = &frag.occupancy_data;
        let occupancy_size = occupancy_data.len() as u32;
        
        // Check if we have room
        if occupancy_buffer_data.len() + occupancy_data.len() > MAX_FRAGMENT_OCCUPANCY_U32S as usize {
            warn!(
                "Fragment occupancy buffer overflow at fragment {}. Truncating.",
                idx
            );
            break;
        }
        
        // Append this fragment's occupancy data
        occupancy_buffer_data.extend_from_slice(occupancy_data);
        
        // Create GPU fragment with correct offset/size
        gpu_fragments.push(GpuFragmentData::new_with_occupancy(
            frag.position,
            frag.rotation,
            frag.size,
            idx as u32,
            occupancy_offset,
            occupancy_size,
        ));
    }

    collision_state.fragment_count = gpu_fragments.len() as u32;

    // Ensure occupancy buffer has at least one element (GPU doesn't like empty buffers)
    if occupancy_buffer_data.is_empty() {
        occupancy_buffer_data.push(0);
    }

    // Upload fragment occupancy data
    render_queue.write_buffer(
        &collision_pipeline.fragment_occupancy_buffer,
        0,
        bytemuck::cast_slice(&occupancy_buffer_data),
    );

    // Create fragment buffer
    let fragment_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("collision_fragment_buffer"),
        contents: bytemuck::cast_slice(&gpu_fragments),
        usage: BufferUsages::STORAGE,
    });

    // Store fragment buffer
    commands.insert_resource(CollisionFragmentBuffer {
        buffer: fragment_buffer.clone(),
        count: gpu_fragments.len() as u32,
    });

    // Reset contact count to 0
    render_queue.write_buffer(
        &collision_pipeline.contact_count_buffer,
        0,
        bytemuck::bytes_of(&0u32),
    );

    // Write initial uniforms (fragment_index will be updated per dispatch in collision_node)
    let uniforms = CollisionUniforms {
        max_contacts: MAX_GPU_CONTACTS,
        chunk_index_size: MAX_GPU_CHUNKS * 4, // Hash table size
        fragment_index: 0,
        fragment_count: gpu_fragments.len() as u32,
    };
    render_queue.write_buffer(
        &collision_pipeline.uniform_buffer,
        0,
        bytemuck::bytes_of(&uniforms),
    );

    // Create bind groups
    // Group 0: World occupancy
    let occupancy_bind_group = render_device.create_bind_group(
        "collision_occupancy_bind_group",
        &gpu_occupancy.bind_group_layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&gpu_occupancy.chunk_texture_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: gpu_occupancy.chunk_index_buffer.as_entire_binding(),
            },
        ],
    );

    // Group 1: Fragment data + contacts + occupancy
    let fragment_bind_group = render_device.create_bind_group(
        "collision_fragment_bind_group",
        &collision_pipeline.fragment_layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: fragment_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: collision_pipeline.contact_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: collision_pipeline.contact_count_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: collision_pipeline.fragment_occupancy_buffer.as_entire_binding(),
            },
        ],
    );

    // Group 2: Uniforms
    let uniform_bind_group = render_device.create_bind_group(
        "collision_uniform_bind_group",
        &collision_pipeline.uniform_layout,
        &[BindGroupEntry {
            binding: 0,
            resource: collision_pipeline.uniform_buffer.as_entire_binding(),
        }],
    );

    commands.insert_resource(CollisionBindGroups {
        occupancy_bind_group,
        fragment_bind_group,
        uniform_bind_group,
    });
}

/// System to initialize the collision compute pipeline.
pub fn init_collision_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<bevy::render::render_resource::PipelineCache>,
    asset_server: Res<AssetServer>,
    gpu_occupancy: Option<Res<GpuWorldOccupancy>>,
    existing: Option<Res<GpuCollisionPipeline>>,
) {
    if existing.is_some() {
        return;
    }
    
    let Some(gpu_occupancy) = gpu_occupancy else {
        return;
    };

    info!("Initializing GPU collision pipeline");
    let pipeline = GpuCollisionPipeline::new(
        &render_device,
        &pipeline_cache,
        &asset_server,
        &gpu_occupancy.bind_group_layout,
    );
    commands.insert_resource(pipeline);
}
