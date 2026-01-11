//! GPU collision compute system.
//!
//! This module runs the voxel collision compute shader and copies results
//! to staging buffers for readback.
//!
//! ## Dispatch Strategy
//!
//! Each fragment is processed with a separate dispatch. The fragment_index
//! is written to a uniform buffer before each dispatch, so the shader knows
//! which fragment to process.
//!
//! Workgroup layout per fragment:
//! - X: ceil(fragment.size.x / 8)
//! - Y: ceil(fragment.size.y / 8)  
//! - Z: fragment.size.z (each workgroup handles one Z slice)
//!
//! Each thread within a workgroup handles one (x,y) position in the fragment.

use bevy::prelude::*;
use bevy::render::{
    render_graph::RenderLabel,
    render_resource::PipelineCache,
    renderer::{RenderDevice, RenderQueue},
};

use super::collision_extract::ExtractedFragments;
use super::collision_prepare::{CollisionBindGroups, CollisionFragmentBuffer, GpuCollisionState};
use super::collision_readback::GpuCollisionContacts;
use crate::voxel::VoxelScaleConfig;
use crate::voxel_collision_gpu::{
    CollisionUniforms, GpuCollisionPipeline, GpuCollisionResult, GpuContact, HASH_GRID_TOTAL_CELLS,
    MAX_GPU_CHUNKS, MAX_GPU_CONTACTS,
};

/// Label for the collision compute node (for render graph, if used).
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct CollisionComputeLabel;

/// System that runs the collision compute shader.
/// This runs as a system in the Render schedule, after bind groups are prepared.
///
/// ## Dispatch Strategy
///
/// The collision pipeline runs in three phases:
///
/// 1. **Clear Hash Grid**: Single dispatch to reset the spatial hash grid
///    - Workgroups: ceil(HASH_GRID_TOTAL_CELLS / 64)
///
/// 2. **Populate Hash Grid**: One dispatch per fragment to insert voxels
///    - Same workgroup layout as collision: 8x8xZ per fragment
///
/// 3. **Collision Detection**: One dispatch per fragment
///    - Checks terrain occupancy AND spatial hash grid
///    - Outputs contacts for both terrain and fragment collisions
///
/// For each fragment dispatch:
/// - workgroups_x = ceil(size.x / 8)
/// - workgroups_y = ceil(size.y / 8)
/// - workgroups_z = size.z
///
/// The shader uses `global_invocation_id` for local voxel position and reads
/// `fragment_index` from uniforms to know which fragment data to use.
pub fn run_collision_compute_system(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline_cache: Res<PipelineCache>,
    collision_pipeline: Option<Res<GpuCollisionPipeline>>,
    collision_state: Option<Res<GpuCollisionState>>,
    bind_groups: Option<Res<CollisionBindGroups>>,
    fragment_buffer: Option<Res<CollisionFragmentBuffer>>,
    extracted_fragments: Option<Res<ExtractedFragments>>,
    contacts: Option<Res<GpuCollisionContacts>>,
    scale_config: Option<Res<VoxelScaleConfig>>,
) {
    // Get voxel scale (default to 1.0 if not configured)
    let voxel_scale = scale_config.map(|c| c.scale).unwrap_or(1.0);

    // Get required resources
    let Some(collision_pipeline) = collision_pipeline else {
        return;
    };
    let Some(collision_state) = collision_state else {
        return;
    };
    let Some(bind_groups) = bind_groups else {
        return;
    };
    let Some(fragment_buffer) = fragment_buffer else {
        return;
    };
    let Some(extracted_fragments) = extracted_fragments else {
        return;
    };

    // Skip if not initialized or no fragments
    if !collision_state.initialized {
        trace!("Collision compute: not initialized");
        return;
    }
    if fragment_buffer.count == 0 {
        return; // This is normal when no fragments exist
    }

    // Get all required pipelines
    let Some(main_pipeline) = pipeline_cache.get_compute_pipeline(collision_pipeline.pipeline_id)
    else {
        // Pipeline not ready yet
        return;
    };
    let Some(clear_grid_pipeline) =
        pipeline_cache.get_compute_pipeline(collision_pipeline.clear_grid_pipeline_id)
    else {
        return;
    };
    let Some(populate_grid_pipeline) =
        pipeline_cache.get_compute_pipeline(collision_pipeline.populate_grid_pipeline_id)
    else {
        return;
    };

    trace!(
        "Running collision compute for {} fragments",
        fragment_buffer.count
    );

    // Create command encoder
    let mut encoder = render_device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("collision_compute_encoder"),
    });

    // ========================================================================
    // Phase 1: Clear Hash Grid
    // ========================================================================
    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("clear_hash_grid_pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(clear_grid_pipeline);
        // Bind all groups even though clear only uses group 3
        compute_pass.set_bind_group(0, &bind_groups.occupancy_bind_group, &[]);
        compute_pass.set_bind_group(1, &bind_groups.fragment_bind_group, &[]);
        compute_pass.set_bind_group(2, &bind_groups.uniform_bind_group, &[]);
        compute_pass.set_bind_group(3, &bind_groups.hash_grid_bind_group, &[]);

        // Dispatch: ceil(HASH_GRID_TOTAL_CELLS * 4 / 64) workgroups
        // Each cell has 4 slots to clear
        let total_slots = HASH_GRID_TOTAL_CELLS * 4;
        let workgroups = (total_slots + 63) / 64;
        compute_pass.dispatch_workgroups(workgroups, 1, 1);
    }

    // ========================================================================
    // Phase 2: Populate Hash Grid (one dispatch per fragment)
    // ========================================================================
    for (frag_idx, frag) in extracted_fragments.fragments.iter().enumerate() {
        // Update uniforms with current fragment index
        let uniforms = CollisionUniforms::with_scale(
            MAX_GPU_CONTACTS,
            MAX_GPU_CHUNKS * 4,
            frag_idx as u32,
            fragment_buffer.count,
            voxel_scale,
        );
        render_queue.write_buffer(
            &collision_pipeline.uniform_buffer,
            0,
            bytemuck::bytes_of(&uniforms),
        );

        // Calculate dispatch size for this fragment
        let workgroups_x = (frag.size.x + 7) / 8;
        let workgroups_y = (frag.size.y + 7) / 8;
        let workgroups_z = frag.size.z;

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("populate_hash_grid_pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(populate_grid_pipeline);
            // Bind all groups even though populate only uses groups 1, 2, 3
            compute_pass.set_bind_group(0, &bind_groups.occupancy_bind_group, &[]);
            compute_pass.set_bind_group(1, &bind_groups.fragment_bind_group, &[]);
            compute_pass.set_bind_group(2, &bind_groups.uniform_bind_group, &[]);
            compute_pass.set_bind_group(3, &bind_groups.hash_grid_bind_group, &[]);
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, workgroups_z);
        }
    }

    // ========================================================================
    // Phase 3: Collision Detection (one dispatch per fragment)
    // ========================================================================
    for (frag_idx, frag) in extracted_fragments.fragments.iter().enumerate() {
        // Update uniforms with current fragment index and scale
        let uniforms = CollisionUniforms::with_scale(
            MAX_GPU_CONTACTS,
            MAX_GPU_CHUNKS * 4,
            frag_idx as u32,
            fragment_buffer.count,
            voxel_scale,
        );
        render_queue.write_buffer(
            &collision_pipeline.uniform_buffer,
            0,
            bytemuck::bytes_of(&uniforms),
        );

        // Calculate dispatch size for this fragment
        // Workgroup size is 8x8x1, so we need:
        // - ceil(size.x / 8) workgroups in X
        // - ceil(size.y / 8) workgroups in Y
        // - size.z workgroups in Z (each handles one Z slice)
        let workgroups_x = (frag.size.x + 7) / 8;
        let workgroups_y = (frag.size.y + 7) / 8;
        let workgroups_z = frag.size.z;

        trace!(
            "Fragment {}: size {:?}, dispatch {}x{}x{}",
            frag_idx,
            frag.size,
            workgroups_x,
            workgroups_y,
            workgroups_z
        );

        // Begin compute pass for this fragment
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("collision_compute_pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(main_pipeline);
            compute_pass.set_bind_group(0, &bind_groups.occupancy_bind_group, &[]);
            compute_pass.set_bind_group(1, &bind_groups.fragment_bind_group, &[]);
            compute_pass.set_bind_group(2, &bind_groups.uniform_bind_group, &[]);
            compute_pass.set_bind_group(3, &bind_groups.hash_grid_bind_group, &[]);
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, workgroups_z);
        }
    }

    // Copy results to staging buffers
    let contact_buffer_size = MAX_GPU_CONTACTS as u64 * std::mem::size_of::<GpuContact>() as u64;

    encoder.copy_buffer_to_buffer(
        &collision_pipeline.contact_buffer,
        0,
        &collision_pipeline.readback_buffer,
        0,
        contact_buffer_size,
    );

    encoder.copy_buffer_to_buffer(
        &collision_pipeline.contact_count_buffer,
        0,
        &collision_pipeline.count_readback_buffer,
        0,
        4,
    );

    // Submit commands
    render_queue.submit(std::iter::once(encoder.finish()));

    // Synchronous readback (blocking - TODO: Phase 4 will make this async)
    // For now, this is fine for testing correctness
    if let Some(contacts) = contacts {
        // Also store entity mapping for Phase 5
        let entity_map: Vec<Entity> = extracted_fragments
            .fragments
            .iter()
            .map(|f| f.entity)
            .collect();

        // Map the count buffer
        let count_slice = collision_pipeline.count_readback_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        count_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        // Poll until ready
        let _ = render_device.wgpu_device().poll(wgpu::PollType::wait());

        if rx.recv().ok().and_then(|r| r.ok()).is_some() {
            let count_data = count_slice.get_mapped_range();
            let contact_count = *bytemuck::from_bytes::<u32>(&count_data);
            drop(count_data);
            collision_pipeline.count_readback_buffer.unmap();

            trace!("GPU collision readback: {} contacts", contact_count);

            if contact_count > 0 {
                // Map the contacts buffer
                let contacts_size = (contact_count as usize).min(MAX_GPU_CONTACTS as usize)
                    * std::mem::size_of::<GpuContact>();
                let contacts_slice = collision_pipeline
                    .readback_buffer
                    .slice(..contacts_size as u64);
                let (tx2, rx2) = std::sync::mpsc::channel();
                contacts_slice.map_async(wgpu::MapMode::Read, move |result| {
                    tx2.send(result).ok();
                });

                let _ = render_device.wgpu_device().poll(wgpu::PollType::wait());

                if rx2.recv().ok().and_then(|r| r.ok()).is_some() {
                    let contacts_data = contacts_slice.get_mapped_range();
                    let gpu_contacts: &[GpuContact] = bytemuck::cast_slice(&contacts_data);

                    let result = GpuCollisionResult {
                        contacts: gpu_contacts.to_vec(),
                        fragment_entities: entity_map,
                    };

                    trace!(
                        "GPU collision: {} contacts for {} entities",
                        result.contacts.len(),
                        result.fragment_entities.len()
                    );
                    contacts.set(result);

                    drop(contacts_data);
                }
                collision_pipeline.readback_buffer.unmap();
            } else {
                // No contacts - still pass entity map for consistency
                contacts.set(GpuCollisionResult {
                    contacts: Vec::new(),
                    fragment_entities: entity_map,
                });
            }
        }
    }
}
