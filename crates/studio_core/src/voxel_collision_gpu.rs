//! GPU-based voxel collision system.
//!
//! This module provides GPU-accelerated collision detection for voxel worlds
//! using compute shaders. It uploads chunk occupancy data to GPU textures
//! and runs collision queries in parallel on the GPU.
//!
//! ## Architecture
//!
//! ```text
//! CPU                                  GPU
//! ────                                 ───
//! ChunkOccupancy ──upload──► Chunk Texture Array (R32Uint per layer)
//! FragmentOccupancy ──────► Fragment Texture
//!                                      │
//!                           Collision Compute Shader
//!                           - For each fragment voxel:
//!                             - Transform to world space
//!                             - Sample terrain occupancy
//!                             - If collision: emit contact
//!                                      │
//! CollisionResult ◄──readback── Contact Output Buffer
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! // Initialize (once)
//! let gpu_occupancy = GpuWorldOccupancy::new(&render_device, 64);
//!
//! // Upload terrain chunks
//! gpu_occupancy.upload_chunk(&queue, IVec3::new(0, 0, 0), &chunk_data);
//!
//! // Run collision query (per frame)
//! let contacts = gpu_collision.query_fragment(&fragment, position, rotation);
//! ```

use bevy::asset::AssetServer;
use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        BindGroupLayout, BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType,
        BufferDescriptor, BufferUsages, CachedComputePipelineId, ComputePipelineDescriptor,
        PipelineCache, ShaderStages, TextureSampleType, TextureViewDimension,
    },
    renderer::{RenderDevice, RenderQueue},
};
use std::collections::HashMap;

use crate::voxel_collision::ChunkOccupancy;

/// Maximum number of chunks that can be loaded on GPU simultaneously.
pub const MAX_GPU_CHUNKS: u32 = 64;

/// Size of one chunk layer in the texture array (32x32x32 bits = 32x32 u32s per Z slice = 32 rows of 32 u32s).
/// We store each chunk as a 32x1024 R32Uint texture (32 Z-slices × 32*32 = 1024 bits per slice).
/// Actually, let's use a 3D texture approach: 32x32x32 where each texel is 1 bit packed into u32.
/// Simpler: store as 2D array where each layer is 32x32 (1024 texels), with 32 layers per chunk.
///
/// Actually, the simplest GPU-friendly format for bit-packed occupancy:
/// - Each chunk is 32x32x32 = 32768 bits = 1024 u32s = 4096 bytes
/// - Store as a 1D buffer per chunk, index = x + y*32 + z*32*32, bit = linear_index % 32, word = linear_index / 32
/// - Or use a 3D texture of R32Uint with size 32x32x32 where each texel stores 1 bit? No, wasteful.
///
/// Best approach: Store as 2D texture array
/// - Each layer = one chunk
/// - Texture format: R32Uint
/// - Dimensions: 32 x 32 (1024 texels per layer)
/// - Each texel stores 32 bits (one Z-column of 32 voxels)
/// - Lookup: layer = chunk_index, uv = (x, y), bit = z
pub const CHUNK_TEXTURE_WIDTH: u32 = 32;
pub const CHUNK_TEXTURE_HEIGHT: u32 = 32;

/// GPU-resident world occupancy data.
///
/// Stores chunk occupancy in a 2D texture array for fast GPU sampling.
#[derive(Resource)]
pub struct GpuWorldOccupancy {
    /// 2D texture array storing chunk occupancy.
    /// Format: R32Uint, size: 32x32 per layer, layers = max_chunks
    /// Each texel stores 32 bits representing a column of voxels in Z.
    pub chunk_texture: wgpu::Texture,
    pub chunk_texture_view: wgpu::TextureView,

    /// Storage buffer mapping chunk coordinates to texture layer indices.
    /// Uses open addressing with linear probing for collision resolution.
    /// Format: array of ChunkIndexEntry, where layer = -1 means not loaded
    pub chunk_index_buffer: wgpu::Buffer,

    /// Maps chunk coordinate to layer index
    loaded_chunks: HashMap<IVec3, u32>,

    /// Maps hash table slot to chunk coordinate (for probing)
    slot_to_coord: HashMap<u32, IVec3>,

    /// Free layer indices
    free_layers: Vec<u32>,

    /// Maximum chunks that can be loaded
    max_chunks: u32,

    /// Bind group layout for the occupancy data
    pub bind_group_layout: BindGroupLayout,
}

impl GpuWorldOccupancy {
    /// Create a new GPU world occupancy with capacity for `max_chunks` chunks.
    pub fn new(render_device: &RenderDevice, max_chunks: u32) -> Self {
        let max_chunks = max_chunks.min(MAX_GPU_CHUNKS);

        // Create 2D texture array for chunk occupancy
        let chunk_texture = render_device
            .wgpu_device()
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("gpu_world_occupancy_chunks"),
                size: wgpu::Extent3d {
                    width: CHUNK_TEXTURE_WIDTH,
                    height: CHUNK_TEXTURE_HEIGHT,
                    depth_or_array_layers: max_chunks,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

        let chunk_texture_view = chunk_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..default()
        });

        // Create chunk index buffer
        // We use a simple hash table approach: hash(chunk_coord) -> layer_index
        // Size = max_chunks * 4 for collision tolerance
        let index_buffer_size = (max_chunks as usize * 4) * std::mem::size_of::<ChunkIndexEntry>();
        let _initial_data = vec![ChunkIndexEntry::empty(); max_chunks as usize * 4];

        let chunk_index_buffer =
            render_device
                .wgpu_device()
                .create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gpu_world_occupancy_index"),
                    size: index_buffer_size as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

        // Create bind group layout
        let bind_group_layout = render_device.create_bind_group_layout(
            "gpu_world_occupancy_layout",
            &[
                // Chunk texture array
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Uint,
                        view_dimension: TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // Chunk index buffer
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        // Initialize free layers
        let free_layers: Vec<u32> = (0..max_chunks).rev().collect();

        Self {
            chunk_texture,
            chunk_texture_view,
            chunk_index_buffer,
            loaded_chunks: HashMap::new(),
            slot_to_coord: HashMap::new(),
            free_layers,
            max_chunks,
            bind_group_layout,
        }
    }

    /// Upload a chunk's occupancy data to the GPU.
    ///
    /// Returns the layer index, or None if no space available.
    pub fn upload_chunk(
        &mut self,
        queue: &RenderQueue,
        coord: IVec3,
        occupancy: &ChunkOccupancy,
    ) -> Option<u32> {
        // Check if already loaded
        if let Some(&layer) = self.loaded_chunks.get(&coord) {
            // Update existing layer (no need to update index buffer, coord is same)
            self.write_chunk_to_layer(queue, layer, occupancy);
            return Some(layer);
        }

        // Get a free layer
        let layer = self.free_layers.pop()?;

        // Write occupancy data
        self.write_chunk_to_layer(queue, layer, occupancy);

        // Update index buffer with linear probing
        self.update_chunk_index(queue, coord, layer as i32);

        // Track loaded chunk
        self.loaded_chunks.insert(coord, layer);

        Some(layer)
    }

    /// Remove a chunk from the GPU.
    pub fn unload_chunk(&mut self, queue: &RenderQueue, coord: IVec3) {
        if let Some(layer) = self.loaded_chunks.remove(&coord) {
            // Mark layer as free
            self.free_layers.push(layer);

            // Update index buffer to mark as unloaded (also removes from slot_to_coord)
            self.update_chunk_index(queue, coord, -1);
        }
    }

    /// Write chunk occupancy data to a texture layer.
    fn write_chunk_to_layer(&self, queue: &RenderQueue, layer: u32, occupancy: &ChunkOccupancy) {
        // Convert occupancy to the texture format
        // Our texture is 32x32 R32Uint, where each texel is a 32-bit column in Z
        // ChunkOccupancy stores data as: index = x + y*32 + z*32*32
        // We need to reorganize to: texel(x,y) = bits for z=0..31

        let mut texture_data = vec![0u32; 32 * 32];

        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    if occupancy.get(UVec3::new(x, y, z)) {
                        // Set bit z in texel (x, y)
                        texture_data[(y * 32 + x) as usize] |= 1 << z;
                    }
                }
            }
        }

        // Upload to GPU
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.chunk_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&texture_data),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(32 * 4), // 32 u32s per row
                rows_per_image: Some(32),
            },
            wgpu::Extent3d {
                width: 32,
                height: 32,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Update the chunk index buffer entry for a coordinate.
    /// Uses open addressing with linear probing to handle hash collisions.
    /// This must match the lookup logic in the shader (lookup_chunk_layer).
    fn update_chunk_index(&mut self, queue: &RenderQueue, coord: IVec3, layer: i32) {
        let table_size = self.max_chunks * 4;
        let base_hash = self.hash_chunk_coord(coord);

        // Linear probing to find a slot
        // We probe up to 4 slots (matching shader's probe limit)
        for i in 0..4u32 {
            let slot = (base_hash + i) % table_size;

            // Check if this slot is available or already has this coord
            // We maintain a local cache to track what's in each slot
            if let Some(&existing_coord) = self.slot_to_coord.get(&slot) {
                if existing_coord == coord {
                    // Found existing entry for this coord, update it
                    let entry = ChunkIndexEntry {
                        coord_x: coord.x,
                        coord_y: coord.y,
                        coord_z: coord.z,
                        layer,
                    };
                    let offset = (slot as usize * std::mem::size_of::<ChunkIndexEntry>()) as u64;
                    queue.write_buffer(
                        &self.chunk_index_buffer,
                        offset,
                        bytemuck::bytes_of(&entry),
                    );

                    if layer == -1 {
                        // Unloading: remove from tracking
                        self.slot_to_coord.remove(&slot);
                    }
                    return;
                }
                // Slot occupied by different coord, try next slot
            } else {
                // Empty slot found, write new entry
                let entry = ChunkIndexEntry {
                    coord_x: coord.x,
                    coord_y: coord.y,
                    coord_z: coord.z,
                    layer,
                };
                let offset = (slot as usize * std::mem::size_of::<ChunkIndexEntry>()) as u64;
                queue.write_buffer(&self.chunk_index_buffer, offset, bytemuck::bytes_of(&entry));

                if layer != -1 {
                    // Track that this slot now has this coord
                    self.slot_to_coord.insert(slot, coord);
                }
                return;
            }
        }

        // All 4 probe slots full - this shouldn't happen with a properly sized table
        warn!(
            "Hash table collision: no free slot for chunk {:?} (hash {})",
            coord, base_hash
        );
    }

    /// Hash a chunk coordinate to an index in the index buffer.
    fn hash_chunk_coord(&self, coord: IVec3) -> u32 {
        // Simple hash for chunk coordinates
        let mut h = coord.x as u32;
        h = h.wrapping_mul(31).wrapping_add(coord.y as u32);
        h = h.wrapping_mul(31).wrapping_add(coord.z as u32);
        h % (self.max_chunks * 4)
    }

    /// Get the number of loaded chunks.
    pub fn loaded_chunk_count(&self) -> usize {
        self.loaded_chunks.len()
    }

    /// Check if a chunk is loaded.
    pub fn is_chunk_loaded(&self, coord: IVec3) -> bool {
        self.loaded_chunks.contains_key(&coord)
    }
}

/// Entry in the chunk index buffer.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkIndexEntry {
    coord_x: i32,
    coord_y: i32,
    coord_z: i32,
    layer: i32, // -1 = not loaded
}

impl ChunkIndexEntry {
    fn empty() -> Self {
        Self {
            coord_x: i32::MAX,
            coord_y: i32::MAX,
            coord_z: i32::MAX,
            layer: -1,
        }
    }
}

/// Contact type discriminator for GPU contacts.
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ContactType {
    /// Contact with terrain voxels
    Terrain = 0,
    /// Contact with another fragment
    Fragment = 1,
}

/// Contact point output from GPU collision.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuContact {
    /// World position of contact
    pub position: [f32; 3],
    /// Penetration depth
    pub penetration: f32,
    /// Contact normal (pointing out of terrain/other fragment)
    pub normal: [f32; 3],
    /// Fragment index that generated this contact
    pub fragment_index: u32,
    /// Contact type: 0 = terrain, 1 = fragment
    pub contact_type: u32,
    /// Other fragment index (only valid if contact_type == 1)
    pub other_fragment: u32,
    /// Padding to maintain 16-byte alignment
    pub _pad0: u32,
    pub _pad1: u32,
}

/// Result of a GPU collision query.
#[derive(Debug, Default, Clone)]
pub struct GpuCollisionResult {
    /// Contact points generated by the query
    pub contacts: Vec<GpuContact>,
    /// Entity mapping: fragment_index -> Entity
    /// Used to apply collision forces to correct entities regardless of query order.
    pub fragment_entities: Vec<Entity>,
}

impl GpuCollisionResult {
    /// Check if there are any collisions.
    pub fn has_collision(&self) -> bool {
        !self.contacts.is_empty()
    }

    /// Get contacts for a specific fragment.
    pub fn contacts_for_fragment(&self, fragment_index: u32) -> impl Iterator<Item = &GpuContact> {
        self.contacts
            .iter()
            .filter(move |c| c.fragment_index == fragment_index)
    }

    /// Get terrain contacts for a specific fragment.
    pub fn terrain_contacts_for_fragment(
        &self,
        fragment_index: u32,
    ) -> impl Iterator<Item = &GpuContact> {
        self.contacts.iter().filter(move |c| {
            c.fragment_index == fragment_index && c.contact_type == ContactType::Terrain as u32
        })
    }

    /// Get fragment-to-fragment contacts for a specific fragment.
    pub fn fragment_contacts_for_fragment(
        &self,
        fragment_index: u32,
    ) -> impl Iterator<Item = &GpuContact> {
        self.contacts.iter().filter(move |c| {
            c.fragment_index == fragment_index && c.contact_type == ContactType::Fragment as u32
        })
    }

    /// Count terrain vs fragment contacts for a specific fragment.
    /// Returns (terrain_count, fragment_count).
    pub fn contact_counts_for_fragment(&self, fragment_index: u32) -> (usize, usize) {
        let mut terrain = 0;
        let mut fragment = 0;
        for contact in self.contacts_for_fragment(fragment_index) {
            if contact.contact_type == ContactType::Terrain as u32 {
                terrain += 1;
            } else {
                fragment += 1;
            }
        }
        (terrain, fragment)
    }

    /// Calculate a resolution vector (like CPU version) for a specific fragment.
    /// Uses maximum penetration per direction (not sum) to avoid over-correction.
    pub fn resolution_vector_for_fragment(&self, fragment_index: u32) -> Vec3 {
        let contacts: Vec<_> = self.contacts_for_fragment(fragment_index).collect();
        if contacts.is_empty() {
            return Vec3::ZERO;
        }

        // Track maximum penetration for each direction
        let mut max_push = [0.0f32; 6]; // +X, -X, +Y, -Y, +Z, -Z

        for contact in contacts {
            let n = Vec3::from(contact.normal);
            let p = contact.penetration;

            if n.x > 0.7 {
                max_push[0] = max_push[0].max(p);
            } else if n.x < -0.7 {
                max_push[1] = max_push[1].max(p);
            } else if n.y > 0.7 {
                max_push[2] = max_push[2].max(p);
            } else if n.y < -0.7 {
                max_push[3] = max_push[3].max(p);
            } else if n.z > 0.7 {
                max_push[4] = max_push[4].max(p);
            } else if n.z < -0.7 {
                max_push[5] = max_push[5].max(p);
            }
        }

        Vec3::new(
            max_push[0] - max_push[1],
            max_push[2] - max_push[3],
            max_push[4] - max_push[5],
        )
    }

    /// Check if a fragment has floor contact (normal pointing up).
    pub fn has_floor_contact_for_fragment(&self, fragment_index: u32) -> bool {
        self.contacts_for_fragment(fragment_index)
            .any(|c| c.normal[1] > 0.7)
    }
}

// ============================================================================
// GPU Fragment Data (matches shader struct)
// ============================================================================

/// Fragment data for GPU collision, matching the shader struct layout.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuFragmentData {
    /// World position of fragment center
    pub position: [f32; 3],
    pub _pad0: f32,

    /// Rotation quaternion (x, y, z, w)
    pub rotation: [f32; 4],

    /// Size in voxels
    pub size: [u32; 3],
    /// Fragment index
    pub fragment_index: u32,

    /// Offset into occupancy buffer (unused for now - we assume solid)
    pub occupancy_offset: u32,
    /// Number of u32s in occupancy data
    pub occupancy_size: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}

impl GpuFragmentData {
    /// Create fragment data from position, rotation, and size.
    /// Sets occupancy_offset and occupancy_size to 0 (assumes solid).
    pub fn new(position: Vec3, rotation: Quat, size: UVec3, fragment_index: u32) -> Self {
        Self {
            position: position.into(),
            _pad0: 0.0,
            rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
            size: size.into(),
            fragment_index,
            occupancy_offset: 0,
            occupancy_size: 0,
            _pad1: 0,
            _pad2: 0,
        }
    }

    /// Create fragment data with occupancy buffer offset and size.
    pub fn new_with_occupancy(
        position: Vec3,
        rotation: Quat,
        size: UVec3,
        fragment_index: u32,
        occupancy_offset: u32,
        occupancy_size: u32,
    ) -> Self {
        Self {
            position: position.into(),
            _pad0: 0.0,
            rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
            size: size.into(),
            fragment_index,
            occupancy_offset,
            occupancy_size,
            _pad1: 0,
            _pad2: 0,
        }
    }
}

/// Collision uniform data.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CollisionUniforms {
    /// Maximum number of contacts to generate
    pub max_contacts: u32,
    /// Size of the chunk index hash table
    pub chunk_index_size: u32,
    /// Current fragment index being processed (updated per dispatch)
    pub fragment_index: u32,
    /// Total number of fragments this frame
    pub fragment_count: u32,
}

// ============================================================================
// GPU Collision Pipeline
// ============================================================================

/// Maximum contacts that can be generated per frame.
pub const MAX_GPU_CONTACTS: u32 = 4096;

/// Size of the spatial hash grid for fragment-to-fragment collision.
/// Grid covers a cube of this many cells per side.
/// Total cells = HASH_GRID_SIZE^3
pub const HASH_GRID_SIZE: u32 = 64;

/// Total number of cells in the spatial hash grid.
pub const HASH_GRID_TOTAL_CELLS: u32 = HASH_GRID_SIZE * HASH_GRID_SIZE * HASH_GRID_SIZE;

/// Origin of the spatial hash grid in world space.
/// Grid covers from this point to (origin + HASH_GRID_SIZE * cell_size).
pub const HASH_GRID_ORIGIN: [f32; 3] = [-32.0, -32.0, -32.0];

/// GPU collision pipeline for running the voxel collision compute shader.
#[derive(Resource)]
pub struct GpuCollisionPipeline {
    /// The cached compute pipeline for main collision detection
    pub pipeline_id: CachedComputePipelineId,

    /// Cached compute pipeline for clearing the hash grid
    pub clear_grid_pipeline_id: CachedComputePipelineId,

    /// Cached compute pipeline for populating the hash grid
    pub populate_grid_pipeline_id: CachedComputePipelineId,

    /// Bind group layout for fragment data (group 1)
    pub fragment_layout: BindGroupLayout,

    /// Bind group layout for uniforms (group 2)
    pub uniform_layout: BindGroupLayout,

    /// Bind group layout for spatial hash grid (group 3)
    pub hash_grid_layout: BindGroupLayout,

    /// Output contact buffer (GPU side)
    pub contact_buffer: Buffer,

    /// Contact count buffer (atomic counter)
    pub contact_count_buffer: Buffer,

    /// Staging buffer for reading back results
    pub readback_buffer: Buffer,

    /// Staging buffer for reading back contact count
    pub count_readback_buffer: Buffer,

    /// Uniform buffer
    pub uniform_buffer: Buffer,

    /// Fragment occupancy buffer (stores bit-packed occupancy for all fragments)
    /// Layout: [header0, data0..., header1, data1..., ...]
    /// Each header: offset (u32), size_x (u32), size_y (u32), size_z (u32)
    pub fragment_occupancy_buffer: Buffer,

    /// Spatial hash grid buffer for fragment-to-fragment collision.
    /// Each cell stores up to 4 particle IDs as int4 (fragment_index << 16 | local_voxel_index).
    /// -1 indicates empty slot.
    pub hash_grid_buffer: Buffer,
}

/// Maximum total u32s for fragment occupancy data across all fragments.
/// This limits total occupancy data to ~256KB per frame.
pub const MAX_FRAGMENT_OCCUPANCY_U32S: u32 = 65536;

impl GpuCollisionPipeline {
    /// Initialize the collision pipeline.
    pub fn new(
        render_device: &RenderDevice,
        pipeline_cache: &PipelineCache,
        asset_server: &AssetServer,
        occupancy_layout: &BindGroupLayout,
    ) -> Self {
        // Fragment data layout (group 1)
        let fragment_layout = render_device.create_bind_group_layout(
            "collision_fragment_layout",
            &[
                // Fragment data buffer
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Output contacts buffer
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Contact count (atomic)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Fragment occupancy buffer (bit-packed occupancy data)
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        // Uniform layout (group 2)
        let uniform_layout = render_device.create_bind_group_layout(
            "collision_uniform_layout",
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        );

        // Hash grid layout (group 3) - for fragment-to-fragment collision
        let hash_grid_layout = render_device.create_bind_group_layout(
            "collision_hash_grid_layout",
            &[
                // Spatial hash grid buffer (read-write for populate, read for detect)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        // Create output buffers
        let contact_buffer_size =
            MAX_GPU_CONTACTS as u64 * std::mem::size_of::<GpuContact>() as u64;

        let contact_buffer = render_device
            .wgpu_device()
            .create_buffer(&BufferDescriptor {
                label: Some("collision_contact_buffer"),
                size: contact_buffer_size,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

        let contact_count_buffer = render_device
            .wgpu_device()
            .create_buffer(&BufferDescriptor {
                label: Some("collision_contact_count"),
                size: 4, // Single u32
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

        let readback_buffer = render_device
            .wgpu_device()
            .create_buffer(&BufferDescriptor {
                label: Some("collision_readback"),
                size: contact_buffer_size,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

        let count_readback_buffer = render_device
            .wgpu_device()
            .create_buffer(&BufferDescriptor {
                label: Some("collision_count_readback"),
                size: 4,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

        let uniform_buffer = render_device
            .wgpu_device()
            .create_buffer(&BufferDescriptor {
                label: Some("collision_uniforms"),
                size: std::mem::size_of::<CollisionUniforms>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

        // Fragment occupancy buffer for storing bit-packed occupancy data
        let fragment_occupancy_buffer =
            render_device
                .wgpu_device()
                .create_buffer(&BufferDescriptor {
                    label: Some("collision_fragment_occupancy"),
                    size: MAX_FRAGMENT_OCCUPANCY_U32S as u64 * 4,
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

        // Spatial hash grid buffer for fragment-to-fragment collision
        // Each cell stores 4 particle IDs as separate i32 values (for atomics compatibility)
        // Layout: hash_grid[cell_idx * 4 + slot]
        // Particle ID encoding: (fragment_index << 16) | local_voxel_index
        // -1 = empty slot
        let hash_grid_buffer_size = HASH_GRID_TOTAL_CELLS as u64 * 4 * 4; // 4 slots × i32 per cell
        let hash_grid_buffer = render_device
            .wgpu_device()
            .create_buffer(&BufferDescriptor {
                label: Some("collision_hash_grid"),
                size: hash_grid_buffer_size,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

        // Load shader and create pipelines
        let shader = asset_server.load("shaders/voxel_collision.wgsl");

        // Main collision detection pipeline
        let pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("voxel_collision_pipeline".into()),
            layout: vec![
                occupancy_layout.clone(),
                fragment_layout.clone(),
                uniform_layout.clone(),
                hash_grid_layout.clone(),
            ],
            push_constant_ranges: vec![],
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Some("main".into()),
            zero_initialize_workgroup_memory: true,
        });

        // Clear hash grid pipeline
        // Uses the same full layout as main pipeline - WGSL requires consistent bind groups
        let clear_grid_pipeline_id =
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("voxel_collision_clear_grid".into()),
                layout: vec![
                    occupancy_layout.clone(),
                    fragment_layout.clone(),
                    uniform_layout.clone(),
                    hash_grid_layout.clone(),
                ],
                push_constant_ranges: vec![],
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some("clear_hash_grid".into()),
                zero_initialize_workgroup_memory: true,
            });

        // Populate hash grid pipeline
        // Uses the same full layout as main pipeline - WGSL requires consistent bind groups
        let populate_grid_pipeline_id =
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("voxel_collision_populate_grid".into()),
                layout: vec![
                    occupancy_layout.clone(),
                    fragment_layout.clone(),
                    uniform_layout.clone(),
                    hash_grid_layout.clone(),
                ],
                push_constant_ranges: vec![],
                shader,
                shader_defs: vec![],
                entry_point: Some("populate_hash_grid".into()),
                zero_initialize_workgroup_memory: true,
            });

        Self {
            pipeline_id,
            clear_grid_pipeline_id,
            populate_grid_pipeline_id,
            fragment_layout,
            uniform_layout,
            hash_grid_layout,
            contact_buffer: contact_buffer.into(),
            contact_count_buffer: contact_count_buffer.into(),
            readback_buffer: readback_buffer.into(),
            count_readback_buffer: count_readback_buffer.into(),
            uniform_buffer: uniform_buffer.into(),
            fragment_occupancy_buffer: fragment_occupancy_buffer.into(),
            hash_grid_buffer: hash_grid_buffer.into(),
        }
    }

    /// Check if the pipeline is ready.
    pub fn is_ready(&self, pipeline_cache: &PipelineCache) -> bool {
        pipeline_cache
            .get_compute_pipeline(self.pipeline_id)
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_index_entry_size() {
        // Ensure our struct is properly aligned for GPU
        assert_eq!(std::mem::size_of::<ChunkIndexEntry>(), 16);
    }

    #[test]
    fn test_gpu_contact_size() {
        // Ensure contact struct is properly aligned
        // Fields: position (12) + penetration (4) + normal (12) + fragment_index (4) +
        //         contact_type (4) + other_fragment (4) + pad0 (4) + pad1 (4) = 48 bytes
        assert_eq!(std::mem::size_of::<GpuContact>(), 48);
    }

    #[test]
    fn test_chunk_to_texture_data_conversion() {
        // Test that the conversion from ChunkOccupancy to GPU texture format is correct
        // GPU texture format: 32x32 R32Uint, each texel is a 32-bit column in Z
        // texel(x,y) = bitmask for z=0..31

        let mut chunk = ChunkOccupancy::new();

        // Set some known voxels
        chunk.set(UVec3::new(0, 0, 0), true); // texel(0,0) bit 0
        chunk.set(UVec3::new(0, 0, 31), true); // texel(0,0) bit 31
        chunk.set(UVec3::new(5, 10, 15), true); // texel(5,10) bit 15
        chunk.set(UVec3::new(31, 31, 0), true); // texel(31,31) bit 0

        // Convert to texture format (same logic as write_chunk_to_layer)
        let mut texture_data = vec![0u32; 32 * 32];

        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    if chunk.get(UVec3::new(x, y, z)) {
                        texture_data[(y * 32 + x) as usize] |= 1 << z;
                    }
                }
            }
        }

        // Verify the conversion
        assert_eq!(
            texture_data[0 * 32 + 0],
            (1 << 0) | (1 << 31),
            "texel(0,0) should have bits 0 and 31"
        );
        assert_eq!(
            texture_data[10 * 32 + 5],
            1 << 15,
            "texel(5,10) should have bit 15"
        );
        assert_eq!(
            texture_data[31 * 32 + 31],
            1 << 0,
            "texel(31,31) should have bit 0"
        );

        // Verify other texels are 0
        assert_eq!(texture_data[1 * 32 + 0], 0, "texel(0,1) should be 0");
        assert_eq!(texture_data[0 * 32 + 1], 0, "texel(1,0) should be 0");
    }

    #[test]
    fn test_texture_lookup_formula() {
        // Verify the GPU lookup formula:
        // is_occupied(world_pos) {
        //   chunk_coord = world_pos >> 5  (divide by 32)
        //   local_pos = world_pos & 31    (mod 32)
        //   layer = lookup_chunk_layer(chunk_coord)
        //   bits = textureLoad(chunk_textures, vec2(local_pos.x, local_pos.y), layer).r
        //   return (bits & (1u << local_pos.z)) != 0
        // }

        // Test the formula at various positions
        let test_cases = [
            (
                IVec3::new(0, 0, 0),
                IVec3::new(0, 0, 0),
                UVec3::new(0, 0, 0),
            ),
            (
                IVec3::new(31, 31, 31),
                IVec3::new(0, 0, 0),
                UVec3::new(31, 31, 31),
            ),
            (
                IVec3::new(32, 0, 0),
                IVec3::new(1, 0, 0),
                UVec3::new(0, 0, 0),
            ),
            (
                IVec3::new(33, 34, 35),
                IVec3::new(1, 1, 1),
                UVec3::new(1, 2, 3),
            ),
            (
                IVec3::new(-1, 0, 0),
                IVec3::new(-1, 0, 0),
                UVec3::new(31, 0, 0),
            ),
            (
                IVec3::new(-32, 0, 0),
                IVec3::new(-1, 0, 0),
                UVec3::new(0, 0, 0),
            ),
        ];

        for (world_pos, expected_chunk, expected_local) in test_cases {
            let chunk_coord = IVec3::new(
                world_pos.x.div_euclid(32),
                world_pos.y.div_euclid(32),
                world_pos.z.div_euclid(32),
            );
            let local_pos = UVec3::new(
                world_pos.x.rem_euclid(32) as u32,
                world_pos.y.rem_euclid(32) as u32,
                world_pos.z.rem_euclid(32) as u32,
            );

            assert_eq!(
                chunk_coord, expected_chunk,
                "Chunk coord mismatch for {:?}",
                world_pos
            );
            assert_eq!(
                local_pos, expected_local,
                "Local pos mismatch for {:?}",
                world_pos
            );
        }
    }

    #[test]
    fn test_hash_distribution() {
        // Test that hash function distributes chunks reasonably
        let max_chunks = 64u32;
        let mut hit_count = vec![0u32; (max_chunks * 4) as usize];

        // Hash a bunch of nearby chunk coordinates
        for x in -5..5 {
            for y in -2..2 {
                for z in -5..5 {
                    let coord = IVec3::new(x, y, z);
                    let mut h = coord.x as u32;
                    h = h.wrapping_mul(31).wrapping_add(coord.y as u32);
                    h = h.wrapping_mul(31).wrapping_add(coord.z as u32);
                    let idx = h % (max_chunks * 4);
                    hit_count[idx as usize] += 1;
                }
            }
        }

        // Check that no slot is hit more than a reasonable number of times
        let max_hits = *hit_count.iter().max().unwrap();
        let total_chunks = 10 * 4 * 10; // 400 chunks
        let avg_hits = total_chunks as f32 / (max_chunks * 4) as f32;

        // Allow 5x the average as max (reasonable for a hash function)
        assert!(
            max_hits <= (avg_hits * 5.0) as u32 + 1,
            "Hash has too many collisions: max={}, avg={:.1}",
            max_hits,
            avg_hits
        );
    }

    #[test]
    fn test_gpu_fragment_data_size() {
        // Ensure GpuFragmentData matches shader expectations
        // position: vec3<f32> + pad = 16 bytes
        // rotation: vec4<f32> = 16 bytes
        // size: vec3<u32> + fragment_index: u32 = 16 bytes
        // occupancy_offset, occupancy_size, pad1, pad2 = 16 bytes
        // Total: 64 bytes
        assert_eq!(std::mem::size_of::<GpuFragmentData>(), 64);
    }

    #[test]
    fn test_collision_uniforms_size() {
        // Ensure CollisionUniforms is properly sized (16 bytes, vec4 aligned)
        // Fields: max_contacts, chunk_index_size, fragment_index, fragment_count
        assert_eq!(std::mem::size_of::<CollisionUniforms>(), 16);
    }

    #[test]
    fn test_gpu_collision_result_resolution_vector() {
        let mut result = GpuCollisionResult {
            contacts: Vec::new(),
            fragment_entities: vec![Entity::PLACEHOLDER, Entity::PLACEHOLDER], // Two fragments
        };

        // Add some terrain contacts for fragment 0
        result.contacts.push(GpuContact {
            position: [0.0, 0.0, 0.0],
            penetration: 0.3,
            normal: [0.0, 1.0, 0.0], // Push up
            fragment_index: 0,
            contact_type: ContactType::Terrain as u32,
            other_fragment: 0,
            _pad0: 0,
            _pad1: 0,
        });
        result.contacts.push(GpuContact {
            position: [1.0, 0.0, 0.0],
            penetration: 0.5,
            normal: [0.0, 1.0, 0.0], // Push up (more)
            fragment_index: 0,
            contact_type: ContactType::Terrain as u32,
            other_fragment: 0,
            _pad0: 0,
            _pad1: 0,
        });

        // Add a terrain contact for fragment 1
        result.contacts.push(GpuContact {
            position: [0.0, 0.0, 0.0],
            penetration: 0.2,
            normal: [1.0, 0.0, 0.0], // Push +X
            fragment_index: 1,
            contact_type: ContactType::Terrain as u32,
            other_fragment: 0,
            _pad0: 0,
            _pad1: 0,
        });

        // Add a fragment-to-fragment contact for fragment 0
        result.contacts.push(GpuContact {
            position: [2.0, 0.0, 0.0],
            penetration: 0.1,
            normal: [0.0, -1.0, 0.0], // Push down (collision with fragment above)
            fragment_index: 0,
            contact_type: ContactType::Fragment as u32,
            other_fragment: 1,
            _pad0: 0,
            _pad1: 0,
        });

        // Check resolution for fragment 0
        let res0 = result.resolution_vector_for_fragment(0);
        assert!((res0.x).abs() < 0.001, "No X push for fragment 0");
        // Y push should be max(0.3, 0.5) - 0.1 = 0.4 (up - down)
        assert!(
            (res0.y - 0.4).abs() < 0.001,
            "Y push should be 0.5 - 0.1 = 0.4, got {}",
            res0.y
        );
        assert!((res0.z).abs() < 0.001, "No Z push for fragment 0");

        // Check resolution for fragment 1
        let res1 = result.resolution_vector_for_fragment(1);
        assert!((res1.x - 0.2).abs() < 0.001, "X push should be 0.2");
        assert!((res1.y).abs() < 0.001, "No Y push for fragment 1");

        // Check has_floor_contact
        assert!(result.has_floor_contact_for_fragment(0));
        assert!(!result.has_floor_contact_for_fragment(1));

        // Check contact type counts
        let (terrain0, frag0) = result.contact_counts_for_fragment(0);
        assert_eq!(terrain0, 2, "Fragment 0 should have 2 terrain contacts");
        assert_eq!(frag0, 1, "Fragment 0 should have 1 fragment contact");

        let (terrain1, frag1) = result.contact_counts_for_fragment(1);
        assert_eq!(terrain1, 1, "Fragment 1 should have 1 terrain contact");
        assert_eq!(frag1, 0, "Fragment 1 should have 0 fragment contacts");

        // Check entity mapping
        assert_eq!(result.fragment_entities.len(), 2);
    }
}
