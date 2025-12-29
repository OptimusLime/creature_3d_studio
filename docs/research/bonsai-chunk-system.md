# Bonsai Chunk & World Streaming Architecture

Research document for implementing multi-chunk support in Creature 3D Studio.

## Overview

Bonsai uses an octree-based world system with 64x64x64 voxel chunks. Key features:
- Bit-packed occupancy for memory efficiency
- Pre-computed face masks for fast mesh generation
- Border buffers for cross-chunk face culling
- Priority queue streaming based on camera distance
- LOD via variable-resolution octree nodes
- Freelist memory management

---

## 1. Chunk Structure

### Core Chunk Data

```cpp
struct world_chunk {
  world_chunk *Next;              // Freelist pointer
  
  // Chunk data
  v3i  Dim;                       // Typically 64x64x64 or 64x66x66 (with apron)
  u64 *Occupancy;                 // Bit-packed occupancy (1 bit per voxel)
  u64 *xOccupancyBorder;          // Border occupancy for cross-chunk face culling
  u64 *FaceMasks;                 // Pre-computed face visibility (6 masks per row)
  
  b32 IsOnFreelist;
  gpu_element_buffer_handles Handles;  // GPU mesh handles
  
  v3i WorldP;                     // Position in chunk-space coordinates
  s32 FilledCount;                // Number of filled voxels
};
```

### Chunk Size

```cpp
WORLD_CHUNK_DIM = Chunk_Dimension(64, 64, 64);  // Base chunk size

// With apron for cross-chunk face culling:
Global_ChunkApronDim = V3i(2, 2, 4);
Global_ChunkApronMinDim = V3i(1, 1, 1);
Global_ChunkApronMaxDim = V3i(1, 1, 3);
```

**Why 64?** Aligns with u64 bit operations - one X-row of 64 voxels fits in a single u64.

---

## 2. Bit-Packed Occupancy

The key insight: occupancy is stored as **1 bit per voxel**, not full voxel structs.

```cpp
// 64 voxels in X-direction = 1 u64
// For a 64x64x64 chunk: 64*64 = 4096 u64 values = 32KB

u64 *Occupancy;  // [y * 64 + z] indexes a row of 64 X-voxels
```

### Benefits
- 32KB for occupancy vs 16MB for dense voxel array
- Bit operations enable fast face culling
- Cache-friendly access patterns

### Access Pattern
```cpp
// Get occupancy for X-row at (y, z)
u64 row = Occupancy[y * ChunkDim.z + z];

// Check if voxel at (x, y, z) is solid
bool solid = (row >> x) & 1;
```

---

## 3. Cross-Chunk Face Culling

### The Problem
When generating a mesh, faces at chunk boundaries need neighbor data from adjacent chunks.

### Bonsai's Solution: xOccupancyBorder

A minimal border buffer storing just the edge voxels needed for face culling:

```cpp
#define xOccupancyBorder_Dim V3i(2, 1, 66)
#define xOccupancyBorder_ElementCount 132  // Just 132 u64s = 1KB

u64 *xOccupancyBorder;  // Stores -X and +X neighbor edges
```

The border stores:
- Left edge (X=-1) from the -X neighbor chunk
- Right edge (X=64) from the +X neighbor chunk

### Face Mask Generation

```cpp
void MakeFaceMasks_NoExteriorFaces(world_chunk *Chunk) {
  for (z = 1; z < ChunkDim.z-1; ++z) {
    for (y = 1; y < ChunkDim.y-1; ++y) {
      u64 Bits = Occupancy[y * ChunkDim.z + z];
      
      // Get border bits from neighbor chunks
      u64 RightYRow = xOccupancyBorder[((z-1)*2)+1];
      u64 LeftYRow  = xOccupancyBorder[(z-1)*2];
      u64 RightBit = ((RightYRow >> (y-1)) & 1) << 63;
      u64 LeftBit  = ((LeftYRow  >> (y-1)) & 1);
      
      // Neighbor rows within chunk
      u64 yBits  = Occupancy[(y+1) * ChunkDim.z + z];  // +Y neighbor
      u64 nyBits = Occupancy[(y-1) * ChunkDim.z + z];  // -Y neighbor
      u64 zBits  = Occupancy[y * ChunkDim.z + (z+1)];  // +Z neighbor
      u64 nzBits = Occupancy[y * ChunkDim.z + (z-1)];  // -Z neighbor
      
      // Face = solid AND neighbor empty
      u64 RightFaces = Bits & ~(RightBit | (Bits >> 1));
      u64 LeftFaces  = Bits & ~(LeftBit  | (Bits << 1));
      u64 FrontFaces = Bits & ~yBits;
      u64 BackFaces  = Bits & ~nyBits;
      u64 TopFaces   = Bits & ~zBits;
      u64 BotFaces   = Bits & ~nzBits;
      
      // Store 6 face masks per row
      s32 idx = (y * ChunkDim.z + z) * 6;
      FaceMasks[idx + 0] = LeftFaces;
      FaceMasks[idx + 1] = RightFaces;
      FaceMasks[idx + 2] = BackFaces;
      FaceMasks[idx + 3] = FrontFaces;
      FaceMasks[idx + 4] = BotFaces;
      FaceMasks[idx + 5] = TopFaces;
    }
  }
}
```

### Key Insight

The magic is `Bits & ~NeighborBits`:
- `Bits` = which voxels are solid in this row
- `NeighborBits` = which neighbors exist (shifted appropriately)
- `~NeighborBits` = where neighbors are empty
- `Bits & ~NeighborBits` = solid voxels with empty neighbors = visible faces

---

## 4. World Structure

### Octree-Based World

```cpp
struct world {
  v3i Center;                          // World chunk position at center
  visible_region_size VisibleRegionSize;
  
  octree_node Root;                    // Octree root for spatial queries
  memory_arena *OctreeMemory;
  octree_node_freelist OctreeNodeFreelist;
  
  world_chunk ChunkFreelistSentinal;   // Freelist for chunk recycling
  s32 FreeChunkCount;
  
  v3i ChunkDim = V3i(64);
};
```

### Octree Node

```cpp
struct octree_node {
  chunk_flag Flags;
  octree_node_type Type;   // Branch or Leaf
  
  b32 Dirty;
  v3i WorldP;              // Chunk coordinates
  v3i Resolution;          // Size in chunks (for LOD)
  
  world_chunk *Chunk;
  octree_node *Children[8];
  octree_node *Next;       // Freelist pointer
};
```

---

## 5. Streaming System

### Priority Queue

```cpp
#define OCTREE_PRIORITY_QUEUE_LIST_COUNT (512)
#define OCTREE_PRIORITY_QUEUE_LIST_LENGTH (512)

struct octree_node_priority_queue {
  octree_node_ptr_cursor Lists[512];  // 512 priority levels
};
```

### Priority Calculation

Priority is computed from:
1. **Node resolution** - smaller nodes (higher detail) get lower priority index (higher priority)
2. **Frustum visibility** - nodes outside frustum get +128 penalty
3. **Parent geometry** - nodes with parent already having GPU mesh get -256 boost
4. **Distance** - farther nodes get higher priority index

```cpp
s32 ComputePriorityIndex(engine_resources *Engine, octree_node *Node) {
  s32 PriorityIndex = 0;
  
  // Resolution factor
  PriorityIndex += Node->Resolution.x * 4;
  
  // Frustum culling
  if (!IsInFrustum(Camera, NodeBounds)) {
    PriorityIndex += 128;
  }
  
  // Parent has mesh - prioritize children
  if (Parent && Parent->Chunk && HasGpuMesh(Parent->Chunk)) {
    PriorityIndex -= 256;
  }
  
  // Distance factor
  r32 Distance = DistanceToBox(CameraP, NodeRect);
  PriorityIndex += s32(Distance / ChunkDim.x);
  
  return Clamp(0, PriorityIndex, 511);
}
```

### Rate Limiting

```cpp
#define MAX_OCTREE_NODES_QUEUED_TOTAL (64)
#define MAX_OCTREE_NODES_QUEUED_PER_FRAME (32)
```

Only 32 chunks queued per frame, 64 total in flight - prevents frame spikes.

---

## 6. LOD System

### LOD Levels

```cpp
enum world_chunk_mesh_index {
  MeshIndex_Lod0,  // 32 voxels per unit (full detail)
  MeshIndex_Lod1,  // 16 voxels per unit  
  MeshIndex_Lod2,  // 8 voxels per unit
  MeshIndex_Lod3,  // 4 voxels per unit
  MeshIndex_Lod4,  // 2 voxels per unit
};
```

### Resolution-Based LOD

Octree nodes have variable resolution - a single node can represent 1x1x1 chunks (full detail) or NxNxN chunks (low detail):

```cpp
v3i ComputeNodeDesiredResolution(engine_resources *Engine, octree_node *Node) {
  r32 Distance = DistanceToBox(CameraP, NodeRect);
  s32 DistanceInChunks = s32(Distance) / s32(World->ChunkDim.x);
  
  // Further = higher resolution value = lower detail
  v3i Result = Max(V3i(1), V3i(DistanceInChunks / ChunksPerResolutionStep));
  return Result;
}
```

### Split/Collapse

- **Split**: When camera approaches, divide into 8 children with higher detail
- **Collapse**: When camera recedes, merge children back into single low-detail node

```cpp
b32 OctreeLeafShouldSplit(engine_resources *Engine, octree_node *Node) {
  if (Node->Resolution > V3i(1)) {
    v3i DesiredRes = ComputeNodeDesiredResolution(Engine, Node);
    return DesiredRes < Node->Resolution;  // Want more detail
  }
  return False;
}
```

---

## 7. Memory Management

### Chunk Freelist

Chunks are recycled via a freelist to avoid allocation:

```cpp
world_chunk* GetFreeWorldChunk(world *World) {
  if (World->ChunkFreelistSentinal.Next) {
    AcquireFutex(&World->ChunkFreelistFutex);
    world_chunk *Next = World->ChunkFreelistSentinal.Next;
    World->ChunkFreelistSentinal.Next = Next->Next;
    World->FreeChunkCount -= 1;
    ReleaseFutex(&World->ChunkFreelistFutex);
    return Next;
  } else {
    return AllocateWorldChunk(...);
  }
}

void FreeWorldChunk(world *World, world_chunk *Chunk) {
  AcquireFutex(&World->ChunkFreelistFutex);
  Chunk->Next = World->ChunkFreelistSentinal.Next;
  World->ChunkFreelistSentinal.Next = Chunk;
  World->FreeChunkCount += 1;
  ReleaseFutex(&World->ChunkFreelistFutex);
}
```

### Mesh Buffers

GPU mesh buffers also use tiered freelists based on size:

```cpp
#define TIERED_MESH_FREELIST_MAX_ELEMENTS (128)
#define WORLD_CHUNK_MESH_MIN_SIZE (2048)
```

---

## 8. Canonical Position

Handles positions that cross chunk boundaries:

```cpp
struct canonical_position {
  v3 Offset;             // Position within chunk (0 to ChunkDim)
  world_position WorldP; // Chunk coordinates
};

canonical_position Canonicalize(v3 Offset, world_position WorldP) {
  canonical_position Result = { Offset, WorldP };
  
  // Wrap X
  if (Result.Offset.x >= ChunkDim.x) {
    s32 ChunkWidths = (s32)Result.Offset.x / ChunkDim.x;
    Result.Offset.x -= ChunkDim.x * ChunkWidths;
    Result.WorldP.x += ChunkWidths;
  } else if (Result.Offset.x < 0) {
    s32 ChunkWidths = (s32)(-Result.Offset.x / ChunkDim.x) + 1;
    Result.Offset.x += ChunkDim.x * ChunkWidths;
    Result.WorldP.x -= ChunkWidths;
  }
  // Similar for Y, Z...
  
  return Result;
}
```

---

## 9. Key Design Decisions

### What Bonsai Does Well

1. **Bit-packed occupancy** - 64:1 compression for occupancy data
2. **Pre-computed face masks** - Mesh generation is just bit iteration
3. **Minimal border buffers** - Only 1KB per chunk for cross-chunk culling
4. **Octree LOD** - Graceful degradation at distance
5. **Rate-limited streaming** - Predictable frame times
6. **Freelist recycling** - Zero steady-state allocation

### Complexity We May Not Need

1. **Octree spatial structure** - HashMap may suffice for our scale
2. **5-level LOD** - May only need 1-2 levels initially
3. **Async threading with futexes** - Can start single-threaded
4. **Complex priority queue** - Simple distance sort may suffice

---

## 10. Recommended Approach for Creature Studio

### Phase 1: Basic Multi-Chunk (Minimal)

```rust
struct ChunkPos(i32, i32, i32);

struct VoxelWorld {
    chunks: HashMap<ChunkPos, VoxelChunk>,
    chunk_size: usize,  // 32 for now, match our CHUNK_SIZE
}
```

Keep it simple:
- HashMap lookup by chunk position
- No streaming yet - load all chunks upfront
- No cross-chunk face culling yet (accept seams at boundaries)

### Phase 2: Cross-Chunk Face Culling

Add border buffers like Bonsai:
```rust
struct VoxelChunk {
    voxels: Box<[[[Option<Voxel>; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE]>,
    // Border occupancy from neighbors (just edge slices)
    border_neg_x: Option<[bool; CHUNK_SIZE * CHUNK_SIZE]>,
    border_pos_x: Option<[bool; CHUNK_SIZE * CHUNK_SIZE]>,
    // ... etc for Y, Z
}
```

When generating mesh, check borders for boundary faces.

### Phase 3: Distance-Based Streaming

```rust
struct StreamingConfig {
    load_radius: f32,    // Load chunks within this distance
    unload_radius: f32,  // Unload chunks beyond this distance
    max_loads_per_frame: usize,
}

fn update_streaming(
    world: &mut VoxelWorld,
    camera_chunk: ChunkPos,
    config: &StreamingConfig,
) {
    // 1. Find chunks to unload (beyond unload_radius)
    // 2. Find chunks to load (within load_radius, not loaded)
    // 3. Sort by distance, load up to max_loads_per_frame
}
```

### Phase 4: LOD (If Needed)

Only if performance requires:
- Generate low-detail meshes for distant chunks
- 2x2x2 voxels merged into 1 for LOD1, etc.

---

## References

- Bonsai source: `src/engine/world_chunk.cpp`, `world.cpp`
- Face mask generation: `MakeFaceMasks_NoExteriorFaces()`
- Mesh building: `BuildWorldChunkMeshFromMarkedVoxels_Naieve_v3()`
- Streaming: `SplitAndQueueOctreeNodesForInit()`
