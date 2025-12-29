# Chunk Streaming Design

Research and implementation notes for Phase 17: Distance-based chunk streaming.

## Overview

Chunk streaming enables infinite voxel worlds by dynamically loading/unloading chunks based on camera position. Only chunks near the player are kept in memory and rendered.

## Design Goals

1. **Memory efficiency**: Only load chunks the player can see
2. **Smooth performance**: Rate-limit loading to prevent frame spikes
3. **No popping**: Hysteresis prevents rapid load/unload at boundaries
4. **Integration**: Works with existing deferred rendering pipeline

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     CHUNK STREAMING                              │
└─────────────────────────────────────────────────────────────────┘

                   Camera at world position
                           │
                           ▼
    ┌──────────────────────┴──────────────────────┐
    │     Convert to chunk coordinates            │
    │     camera_chunk = floor(camera_pos / 32)   │
    └──────────────────────┬──────────────────────┘
                           │
       ┌───────────────────┼───────────────────┐
       ▼                   ▼                   ▼
  ┌─────────┐        ┌─────────┐        ┌─────────┐
  │ UNLOAD  │        │ Current │        │  LOAD   │
  │ beyond  │◀──────▶│ camera  │◀──────▶│ within  │
  │ radius  │        │  chunk  │        │ radius  │
  └─────────┘        └─────────┘        └─────────┘
      │                                      │
      ▼                                      ▼
  Despawn entity                        Spawn entity
  Remove from loaded                    Add to loaded
```

## Key Components

### ChunkStreamingConfig

Configuration resource controlling streaming behavior:

```rust
pub struct ChunkStreamingConfig {
    pub load_radius: i32,           // Load within this chunk distance
    pub unload_radius: i32,         // Unload beyond this (> load for hysteresis)
    pub max_loads_per_frame: usize, // Rate limit to prevent spikes
    pub max_unloads_per_frame: usize,
    pub use_greedy_meshing: bool,   // Mesh optimization
    pub y_range: Option<(i32, i32)>, // Optional Y constraint
}
```

**Hysteresis**: `unload_radius > load_radius` creates a buffer zone where chunks stay loaded. This prevents thrashing when camera is at boundary:

```
        unload_radius
    ┌───────────────────┐
    │                   │
    │   load_radius     │
    │  ┌───────────┐    │
    │  │           │    │
    │  │  camera   │    │
    │  │           │    │
    │  └───────────┘    │
    │    (loaded)       │
    │                   │
    └───────────────────┘
        (stay loaded)
```

### ChunkManager

Resource that owns the `VoxelWorld` and tracks loaded state:

```rust
pub struct ChunkManager {
    world: VoxelWorld,                      // Source voxel data
    loaded_chunks: HashMap<ChunkPos, Entity>, // Loaded chunk entities
    load_queue: Vec<ChunkPos>,              // Prioritized load queue
    last_camera_chunk: Option<ChunkPos>,    // For movement detection
    pub stats: StreamingStats,              // Debug info
}
```

### Load Queue Priority

Chunks are sorted by distance from camera (nearest first):

```rust
// Sort descending so pop() returns nearest
self.load_queue.sort_by_key(|pos| {
    let dx = pos.x - camera_chunk.x;
    let dy = pos.y - camera_chunk.y;
    let dz = pos.z - camera_chunk.z;
    Reverse(dx * dx + dy * dy + dz * dz)
});
```

### ChunkMaterialHandle

The deferred rendering pipeline requires `MeshMaterial3d<VoxelMaterial>` component for extraction. The streaming system needs access to a shared material:

```rust
#[derive(Resource)]
pub struct ChunkMaterialHandle(pub Handle<VoxelMaterial>);
```

## Streaming Algorithm

### Per-Frame Update

```rust
fn chunk_streaming_system() {
    // 1. Get camera chunk position
    let camera_chunk = world_pos_to_chunk(camera_pos);
    
    // 2. If camera moved to new chunk, rebuild load queue
    if camera_moved {
        rebuild_load_queue(camera_chunk);
    }
    
    // 3. UNLOAD: Find chunks beyond unload_radius
    for chunk in loaded_chunks {
        if distance(chunk, camera_chunk) > unload_radius {
            despawn(chunk);
            // Rate limited to max_unloads_per_frame
        }
    }
    
    // 4. LOAD: Pop from load queue
    while loads < max_loads_per_frame {
        if let Some(pos) = load_queue.pop() {
            spawn_chunk_entity(pos);
        }
    }
}
```

### Spherical vs Cubic Radius

We use spherical distance check for more natural viewing:

```rust
let dist_sq = dx * dx + dy * dy + dz * dz;
if dist_sq <= radius * radius {
    // Within spherical radius
}
```

## Integration with Deferred Rendering

### Required Components

For a chunk to render through the deferred pipeline:

1. `Mesh3d` - The mesh handle
2. `MeshMaterial3d<VoxelMaterial>` - Required for extraction query
3. `Transform` - World position
4. `DeferredRenderable` - Marker for deferred pipeline

### Extraction Query

The deferred pipeline extracts meshes with this filter:

```rust
Query<..., (With<DeferredRenderable>, With<MeshMaterial3d<VoxelMaterial>>)>
```

This is why `ChunkMaterialHandle` is essential.

## Performance Considerations

### Rate Limiting

Loading a chunk involves:
1. Building mesh (greedy meshing, AO calculation)
2. Uploading to GPU
3. Adding to render world

Rate limiting (e.g., 2 chunks/frame) spreads this cost:

| max_loads_per_frame | Behavior |
|---------------------|----------|
| 1 | Very smooth, slow loading |
| 2-4 | Good balance |
| 8+ | Fast loading, potential stutters |

### Memory Usage

Per chunk memory (32x32x32):
- Voxel data: ~128KB (Option<Voxel> = 4 bytes * 32K)
- Mesh vertices: Variable (depends on complexity)
- GPU buffers: Automatic via Bevy

With load_radius=3, maximum ~343 chunks could be loaded (7x7x7 sphere), but typical flat worlds load far fewer.

## Comparison with Bonsai

| Feature | Bonsai | Our Implementation |
|---------|--------|-------------------|
| Spatial structure | Octree | HashMap |
| Priority queue | 512-level priority | Simple distance sort |
| Rate limiting | 32/frame | Configurable |
| LOD | 5 levels | Not yet |
| Threading | Async with futexes | Single-threaded |
| Chunk size | 64x64x64 | 32x32x32 |

Our implementation is simpler but sufficient for current needs. LOD and async loading can be added later if needed.

## Usage Example

```rust
// Setup
fn setup(mut commands: Commands, mut materials: ResMut<Assets<VoxelMaterial>>) {
    // Create world
    let world = create_voxel_world();
    
    // Configure streaming
    commands.insert_resource(ChunkStreamingConfig {
        load_radius: 3,
        unload_radius: 5,
        max_loads_per_frame: 2,
        ..default()
    });
    
    // Material for deferred rendering
    let material = materials.add(VoxelMaterial::default());
    commands.insert_resource(ChunkMaterialHandle(material));
    
    // Chunk manager owns world data
    commands.insert_resource(ChunkManager::new(world));
}

// Add streaming system
app.add_systems(Update, chunk_streaming_system);
```

## Future Improvements

1. **Async Loading**: Move mesh building to background thread
2. **LOD System**: Lower detail for distant chunks
3. **Cross-Chunk Culling**: Border buffers for seamless boundaries (Phase 18)
4. **Chunk Caching**: Keep recently unloaded meshes in memory
5. **Predictive Loading**: Load chunks in camera's movement direction

## Files

| File | Purpose |
|------|---------|
| `crates/studio_core/src/chunk_streaming.rs` | Streaming system implementation |
| `examples/p17_chunk_streaming.rs` | Demo with 8x8 chunk world |
| `docs/research/bonsai-chunk-system.md` | Reference analysis |

## Test Results

- 64 chunk world (8x8)
- load_radius=3 loads ~28 chunks
- Smooth 60fps with rate-limited loading
- Correct fog falloff at chunk edges
- All deferred features work (shadows, AO, bloom)
