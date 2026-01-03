// Voxel Collision Compute Shader
// GPU-accelerated collision detection for voxel fragments against terrain
//
// This shader checks each voxel of a fragment against the world terrain occupancy
// and outputs collision contact points for physics response.
//
// Texture format: R32Uint per texel, where each texel stores 32 bits (one Z-column)
// Lookup: texel(x,y) contains bits for z=0..31

// ============================================================================
// Constants
// ============================================================================

const CHUNK_SIZE: i32 = 32;
const MAX_CONTACTS_PER_FRAGMENT: u32 = 64u;

// ============================================================================
// Bind Groups
// ============================================================================

// Group 0: World terrain occupancy
@group(0) @binding(0) var chunk_textures: texture_2d_array<u32>;
@group(0) @binding(1) var<storage, read> chunk_index: array<ChunkIndexEntry>;

// Group 1: Fragment data and output
@group(1) @binding(0) var<storage, read> fragments: array<FragmentData>;
@group(1) @binding(1) var<storage, read_write> contacts: array<Contact>;
@group(1) @binding(2) var<storage, read_write> contact_count: atomic<u32>;
@group(1) @binding(3) var<storage, read> fragment_occupancy: array<u32>;

// Group 2: Uniforms
@group(2) @binding(0) var<uniform> uniforms: CollisionUniforms;

// ============================================================================
// Structures
// ============================================================================

struct ChunkIndexEntry {
    coord_x: i32,
    coord_y: i32,
    coord_z: i32,
    layer: i32,  // -1 = not loaded
}

struct FragmentData {
    // Fragment transform
    position: vec3<f32>,
    _pad0: f32,
    rotation: vec4<f32>,  // Quaternion (x, y, z, w)
    
    // Fragment bounds (local space)
    size: vec3<u32>,      // Size in voxels
    fragment_index: u32,  // Which fragment this is
    
    // Bit-packed occupancy data offset in separate buffer
    occupancy_offset: u32,
    occupancy_size: u32,  // Number of u32s
    _pad1: u32,
    _pad2: u32,
}

struct Contact {
    // World position of contact
    position: vec3<f32>,
    penetration: f32,
    
    // Contact normal (pointing out of terrain)
    normal: vec3<f32>,
    fragment_index: u32,
}

struct CollisionUniforms {
    max_contacts: u32,
    chunk_index_size: u32,  // Size of hash table
    fragment_index: u32,    // Current fragment being processed (per dispatch)
    fragment_count: u32,    // Total fragments this frame
}

// ============================================================================
// Helper Functions
// ============================================================================

// Hash a chunk coordinate to an index in the chunk index buffer
fn hash_chunk_coord(coord: vec3<i32>, table_size: u32) -> u32 {
    var h = u32(coord.x);
    h = h * 31u + u32(coord.y);
    h = h * 31u + u32(coord.z);
    return h % table_size;
}

// Look up the texture layer for a chunk coordinate
// Returns -1 if chunk is not loaded
fn lookup_chunk_layer(chunk_coord: vec3<i32>) -> i32 {
    let hash = hash_chunk_coord(chunk_coord, uniforms.chunk_index_size);
    let entry = chunk_index[hash];
    
    // Check if this entry matches our chunk
    if entry.coord_x == chunk_coord.x && 
       entry.coord_y == chunk_coord.y && 
       entry.coord_z == chunk_coord.z {
        return entry.layer;
    }
    
    // Linear probe for collision resolution (check next few slots)
    for (var i = 1u; i < 4u; i++) {
        let probe_hash = (hash + i) % uniforms.chunk_index_size;
        let probe_entry = chunk_index[probe_hash];
        if probe_entry.coord_x == chunk_coord.x && 
           probe_entry.coord_y == chunk_coord.y && 
           probe_entry.coord_z == chunk_coord.z {
            return probe_entry.layer;
        }
    }
    
    return -1;
}

// Check if a world position is occupied in the terrain
fn is_terrain_occupied(world_pos: vec3<i32>) -> bool {
    // Convert world position to chunk coordinate and local position
    let chunk_coord = vec3<i32>(
        world_pos.x >> 5,  // divide by 32
        world_pos.y >> 5,
        world_pos.z >> 5
    );
    let local_pos = vec3<u32>(
        u32(world_pos.x & 31),  // mod 32
        u32(world_pos.y & 31),
        u32(world_pos.z & 31)
    );
    
    // Get texture layer for this chunk
    let layer = lookup_chunk_layer(chunk_coord);
    if layer < 0 {
        return false;  // Chunk not loaded, assume empty
    }
    
    // Sample the texture: texel(x,y) contains 32 bits for z=0..31
    let bits = textureLoad(chunk_textures, vec2<i32>(i32(local_pos.x), i32(local_pos.y)), layer, 0).r;
    
    // Check the bit for this Z position
    return (bits & (1u << local_pos.z)) != 0u;
}

// Rotate a vector by a quaternion
fn rotate_by_quat(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    // q = (x, y, z, w) where w is the scalar part
    let qv = vec3<f32>(q.x, q.y, q.z);
    let uv = cross(qv, v);
    let uuv = cross(qv, uv);
    return v + ((uv * q.w) + uuv) * 2.0;
}

// Calculate the contact normal for a collision
// Returns the direction to push the fragment out of the terrain voxel
//
// IMPORTANT: For floor contacts (penetrating from above), we ALWAYS push UP.
// This prevents the "normal flip" bug where deep penetration would cause
// the shortest exit to be through the bottom, leading to objects falling through.
//
// We determine "penetrating from above" by checking if there's empty space above
// the colliding voxel. If the voxel above is empty, this is a floor contact.
fn calculate_contact_normal(world_pos: vec3<f32>, voxel_pos: vec3<i32>) -> vec3<f32> {
    let voxel_min = vec3<f32>(f32(voxel_pos.x), f32(voxel_pos.y), f32(voxel_pos.z));
    let voxel_max = voxel_min + vec3<f32>(1.0, 1.0, 1.0);
    
    // Check if the voxel above is empty - if so, this is a floor contact
    // and we should ALWAYS push UP regardless of penetration depth
    let voxel_above = vec3<i32>(voxel_pos.x, voxel_pos.y + 1, voxel_pos.z);
    let is_floor_contact = !is_terrain_occupied(voxel_above);
    
    if is_floor_contact {
        // Floor contact: always push UP to prevent falling through
        return vec3<f32>(0.0, 1.0, 0.0);
    }
    
    // For non-floor contacts, use standard shortest-exit logic
    let dist_to_min_x = world_pos.x - voxel_min.x;
    let dist_to_max_x = voxel_max.x - world_pos.x;
    let dist_to_min_y = world_pos.y - voxel_min.y;
    let dist_to_max_y = voxel_max.y - world_pos.y;
    let dist_to_min_z = world_pos.z - voxel_min.z;
    let dist_to_max_z = voxel_max.z - world_pos.z;
    
    // Find minimum distance to determine exit direction
    var min_dist = dist_to_max_y;
    var normal = vec3<f32>(0.0, 1.0, 0.0);  // Default: push up
    
    if dist_to_min_y < min_dist {
        min_dist = dist_to_min_y;
        normal = vec3<f32>(0.0, -1.0, 0.0);
    }
    if dist_to_min_x < min_dist {
        min_dist = dist_to_min_x;
        normal = vec3<f32>(-1.0, 0.0, 0.0);
    }
    if dist_to_max_x < min_dist {
        min_dist = dist_to_max_x;
        normal = vec3<f32>(1.0, 0.0, 0.0);
    }
    if dist_to_min_z < min_dist {
        min_dist = dist_to_min_z;
        normal = vec3<f32>(0.0, 0.0, -1.0);
    }
    if dist_to_max_z < min_dist {
        min_dist = dist_to_max_z;
        normal = vec3<f32>(0.0, 0.0, 1.0);
    }
    
    return normal;
}

// Calculate penetration depth along the given normal direction
fn calculate_penetration(world_pos: vec3<f32>, voxel_pos: vec3<i32>, normal: vec3<f32>) -> f32 {
    let voxel_min = vec3<f32>(f32(voxel_pos.x), f32(voxel_pos.y), f32(voxel_pos.z));
    let voxel_max = voxel_min + vec3<f32>(1.0, 1.0, 1.0);
    
    // Calculate penetration along the normal direction
    // For floor contacts (pushing up), penetration is distance from point to voxel top
    if normal.y > 0.5 {
        // Pushing UP: penetration is how far below the voxel top we are
        // Always positive if we're inside the voxel
        return max(0.0, voxel_max.y - world_pos.y);
    } else if normal.y < -0.5 {
        // Pushing DOWN: penetration is how far above the voxel bottom we are
        return max(0.0, world_pos.y - voxel_min.y);
    } else if normal.x > 0.5 {
        return max(0.0, voxel_max.x - world_pos.x);
    } else if normal.x < -0.5 {
        return max(0.0, world_pos.x - voxel_min.x);
    } else if normal.z > 0.5 {
        return max(0.0, voxel_max.z - world_pos.z);
    } else {
        return max(0.0, world_pos.z - voxel_min.z);
    }
}

// Check if a voxel in a fragment is occupied.
// Uses linear indexing: linear = x + y * size.x + z * size.x * size.y
// Bit packing: u32_idx = linear / 32, bit_pos = linear % 32
fn is_fragment_voxel_occupied(fragment: FragmentData, local_pos: vec3<u32>) -> bool {
    // If occupancy_size is 0, assume all voxels within bounds are occupied (solid fragment)
    if fragment.occupancy_size == 0u {
        return true;
    }
    
    // Calculate linear index
    let linear = local_pos.x + local_pos.y * fragment.size.x + local_pos.z * fragment.size.x * fragment.size.y;
    let u32_idx = linear / 32u;
    let bit_pos = linear % 32u;
    
    // Check bounds
    if u32_idx >= fragment.occupancy_size {
        return false;
    }
    
    // Read from fragment occupancy buffer at the fragment's offset
    let data_idx = fragment.occupancy_offset + u32_idx;
    let bits = fragment_occupancy[data_idx];
    
    return (bits & (1u << bit_pos)) != 0u;
}

// ============================================================================
// Main Compute Shader
// ============================================================================

// Each thread processes one voxel of a fragment.
//
// Dispatch strategy (per fragment):
// - workgroups_x = ceil(size.x / 8)
// - workgroups_y = ceil(size.y / 8)
// - workgroups_z = size.z
//
// Each workgroup handles an 8x8 tile at a specific Z level.
// The fragment_index is passed via uniforms (updated per dispatch).

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
        @builtin(workgroup_id) workgroup_id: vec3<u32>) {
    
    // Fragment index comes from uniforms, set per dispatch
    let fragment_idx = uniforms.fragment_index;
    
    if fragment_idx >= arrayLength(&fragments) {
        return;
    }
    
    let fragment = fragments[fragment_idx];
    
    // Local position within fragment:
    // - X,Y from global_id (thread position in 8x8 tiles)
    // - Z from workgroup_id.z (each workgroup handles one Z slice)
    let local_pos = vec3<u32>(global_id.x, global_id.y, workgroup_id.z);
    
    // Check if this thread's position is within fragment bounds
    if local_pos.x >= fragment.size.x || 
       local_pos.y >= fragment.size.y ||
       local_pos.z >= fragment.size.z {
        return;
    }
    
    // Check if this voxel is occupied in the fragment
    // Skip empty voxels to avoid false collision contacts
    if !is_fragment_voxel_occupied(fragment, local_pos) {
        return;
    }
    
    // Convert local position to world position
    let half_size = vec3<f32>(f32(fragment.size.x), f32(fragment.size.y), f32(fragment.size.z)) * 0.5;
    let local_float = vec3<f32>(f32(local_pos.x) + 0.5, f32(local_pos.y) + 0.5, f32(local_pos.z) + 0.5);
    
    // Center the voxel relative to fragment center
    let centered = local_float - half_size;
    
    // Rotate by fragment rotation
    let rotated = rotate_by_quat(centered, fragment.rotation);
    
    // Translate to world position
    let world_pos = fragment.position + rotated;
    
    // Convert to voxel coordinates (floor to get the voxel we're in)
    let voxel_pos = vec3<i32>(
        i32(floor(world_pos.x)),
        i32(floor(world_pos.y)),
        i32(floor(world_pos.z))
    );
    
    // Check for collision with terrain
    if is_terrain_occupied(voxel_pos) {
        // Collision detected! Output a contact
        let normal = calculate_contact_normal(world_pos, voxel_pos);
        let penetration = calculate_penetration(world_pos, voxel_pos, normal);
        
        // Atomically allocate a slot in the output buffer
        let contact_idx = atomicAdd(&contact_count, 1u);
        
        if contact_idx < uniforms.max_contacts {
            var contact: Contact;
            contact.position = world_pos;
            contact.penetration = penetration;
            contact.normal = normal;
            contact.fragment_index = fragment.fragment_index;
            
            contacts[contact_idx] = contact;
        }
    }
}
