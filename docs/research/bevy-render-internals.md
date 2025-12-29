# Bevy 0.17 Render Internals Research

This document captures our discoveries about Bevy's rendering architecture and how it influenced our deferred rendering implementation.

## Key Discoveries

### 1. Render World Extraction

Bevy uses a **two-world architecture**:
- **Main World**: Game logic, entities, components
- **Render World**: GPU resources, render-specific data

**Extraction** happens every frame via `ExtractSchedule`. Components marked with `ExtractComponent` are automatically copied. For custom data, write extraction systems that query `Extract<Query<...>>`.

```rust
// Automatic extraction
#[derive(Component, ExtractComponent)]
pub struct DeferredRenderable;

// Manual extraction for complex data
pub fn extract_deferred_meshes(
    mut commands: Commands,
    meshes_query: Extract<Query<(RenderEntity, &GlobalTransform, &Mesh3d), With<DeferredRenderable>>>,
) { ... }
```

**Key insight**: `RenderEntity` maps main world entities to render world entities. Use this in Extract queries.

### 2. Mesh GPU Buffer Access

Bevy stores mesh vertex/index data in shared GPU buffers managed by `MeshAllocator`. Multiple meshes share the same buffer (slab allocation).

```rust
// Get GPU buffers for a mesh
let mesh_allocator = world.resource::<MeshAllocator>();
let vertex_slice = mesh_allocator.mesh_vertex_slice(&mesh_asset_id);
let index_slice = mesh_allocator.mesh_index_slice(&mesh_asset_id);

// vertex_slice.buffer is the GPU buffer
// vertex_slice.range is the ELEMENT range (not bytes!)
```

**Key insight**: When drawing, use `vertex_slice.range.start` as `base_vertex` for indexed draws.

### 3. Mesh Vertex Attribute Order

Bevy stores vertex attributes in **sorted order by attribute ID**, not insertion order.

| Attribute | ID | Offset |
|-----------|-----|--------|
| POSITION | 0 | 0 |
| NORMAL | 1 | 12 |
| UV_0 | 2 | 24 |
| ... | ... | ... |
| Custom (VoxelColor) | 988540917 | after built-ins |
| Custom (VoxelEmission) | 988540918 | after VoxelColor |

Our voxel mesh layout (40 bytes/vertex):
```
Offset 0:  Position (Float32x3, 12 bytes)
Offset 12: Normal (Float32x3, 12 bytes)
Offset 24: VoxelColor (Float32x3, 12 bytes)
Offset 36: VoxelEmission (Float32, 4 bytes)
```

**Key insight**: `at_shader_location()` only affects shader binding, not buffer layout.

### 4. RenderMesh vs Mesh

- `Mesh` (main world): CPU-side mesh data, attributes as vectors
- `RenderMesh` (render world): GPU metadata only (vertex count, index format, layout ref)

The actual GPU buffers are in `MeshAllocator`, not `RenderMesh`.

```rust
let render_meshes = world.resource::<RenderAssets<RenderMesh>>();
let gpu_mesh = render_meshes.get(mesh_asset_id);
// gpu_mesh.buffer_info tells us indexed vs non-indexed
// gpu_mesh.vertex_count, etc.
```

### 5. Custom Render Graph Nodes

ViewNodes run per-camera and have access to view-specific queries:

```rust
impl ViewNode for GBufferPassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        &'static ViewGBufferTextures,  // Our custom component
    );
    
    fn run<'w>(&self, ..., query_item: QueryItem<Self::ViewQuery>, world: &'w World) {
        // query_item has our per-view data
        // world gives access to global resources
    }
}
```

**Key insight**: ViewNodes can't mutate resources. Prepare all bind groups in Prepare phase.

### 6. Render Graph Scheduling

Nodes are scheduled via edges in the render graph:

```rust
render_app.add_render_graph_edges(
    Core3d,
    (Node3d::StartMainPass, DeferredLabel::GBufferPass),
);
```

Our deferred pipeline runs:
1. `StartMainPass` (Bevy)
2. `GBufferPass` (ours - renders to MRT)
3. `MainOpaquePass` (Bevy - we skip this for deferred objects)
4. `LightingPass` (ours - fullscreen deferred lighting)
5. `MainTransparentPass` (Bevy)

### 7. Pipeline Caching

Bevy caches render pipelines. Queue them once, retrieve by ID:

```rust
let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor { ... });

// Later, in render node:
let pipeline = pipeline_cache.get_render_pipeline(pipeline_id);
```

**Key insight**: Pipeline may not be ready on first frame. Always check `Option`.

## Decisions Made Based on Research

### Decision: Custom Extraction vs Reusing RenderMeshInstances

**Options considered**:
1. Query `RenderMeshInstances` (Bevy's extracted mesh data)
2. Custom extraction with `DeferredRenderable` marker

**Chose option 2** because:
- We need transform matrices, not just mesh IDs
- `RenderMeshInstances` is optimized for Bevy's PBR pipeline
- Marker component gives explicit control over what renders in deferred pass

### Decision: Per-Mesh Bind Groups vs Dynamic Offsets

**Options considered**:
1. Create bind group per mesh with transform uniform
2. Use dynamic uniform buffer with offsets

**Chose option 1** because:
- Simpler implementation
- Good enough for small mesh counts
- Can optimize to option 2 later if needed

### Decision: Fixed Vertex Layout vs Specialization

**Options considered**:
1. Fixed vertex layout matching our voxel format
2. Per-mesh pipeline specialization based on `MeshVertexBufferLayoutRef`

**Chose option 1** because:
- All our voxel meshes use identical vertex format
- Avoids pipeline permutation explosion
- Faster startup (single pipeline)

## Open Questions

1. **GPU culling**: Should we implement frustum culling on GPU for large voxel worlds?
2. **Instancing**: Can we batch multiple chunks with same mesh but different transforms?
3. **Async mesh upload**: How to handle mesh changes without stalling?
