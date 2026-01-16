# Optimization & Benchmarking Roadmap

## Summary

Establish a comprehensive benchmarking infrastructure and identify performance bottlenecks in the voxel rendering pipeline. Create a systematic approach to profiling, measuring, and optimizing the codebase with focus on the latest example (`p34_sky_terrain_test`).

## Context

**Latest Example:** `p34_sky_terrain_test.rs`
- 800x800 voxel terrain (rolling hills)
- Dual moon shadows
- Sky dome with clouds/stars
- Character controller with physics
- Full deferred rendering pipeline

**Current State:**
- Basic `BenchmarkPlugin` exists (FPS/frame time reporting)
- No formal profiling infrastructure
- No performance regression testing
- CPU physics designed for GPU conversion (not yet implemented)

---

## Performance-Critical Systems Identified

### Render Pipeline (GPU-bound)
| System | File | Concern |
|--------|------|---------|
| G-Buffer Pass | `gbuffer_node.rs` | Draw call count per chunk |
| Shadow Pass (x2 moons) | `shadow_node.rs` | Dual shadow maps = 2x geometry submission |
| GTAO | `gtao_node.rs` | Compute dispatch, sample count |
| Lighting Pass | `lighting_node.rs` | Per-fragment point light iteration |
| Bloom (6 mip levels) | `bloom_node.rs` | 12 render passes total |
| Sky Dome | `sky_dome_node.rs` | Procedural cloud/star calculation |

### Mesh Generation (CPU-bound)
| System | File | Concern |
|--------|------|---------|
| Greedy Meshing | `voxel_mesh.rs` | O(n) per chunk, runs on main thread |
| Cross-Chunk Culling | `voxel_mesh.rs` | Border synchronization |
| Chunk Streaming | `chunk_streaming.rs` | Load/unload latency |

### Physics (CPU-bound, GPU-ready)
| System | File | Concern |
|--------|------|---------|
| Spring-Damper Physics | `physics_math.rs` | Per-particle force calculation |
| Terrain Collision | `physics_math.rs` | AABB checks against occupancy |
| Voxel Fragment Physics | `voxel_fragment.rs` | Multiple bodies with particles |
| Character Controller | `character_controller.rs` | Per-frame collision sweep |

### MarkovJunior (CPU-bound)
| System | File | Concern |
|--------|------|---------|
| Model Stepping | `markov_junior/` | Pattern matching O(grid_size) |
| Grid to VoxelWorld | `sync_generation_to_layer` | Full grid copy each step |

---

## Phase O1: Establish Benchmark Harness

**Priority:** HIGHEST  
**Outcome:** Automated performance measurement with reproducible results

### Tasks

1. **Create benchmark example** `examples/benchmark_p34.rs`:
   - Fixed camera positions (no player input)
   - Deterministic scene (fixed seed)
   - Configurable test duration
   - Output: JSON performance report

2. **Extend BenchmarkPlugin**:
   - Per-system timing (extract, prepare, render phases)
   - Memory usage tracking
   - Draw call count
   - Triangle/vertex counts

3. **Create baseline measurements**:
   ```bash
   cargo run --example benchmark_p34 --release -- --duration=60 --output=baseline.json
   ```

**Verification:**
```bash
cargo run --example benchmark_p34 --release
# Output: JSON file with avg FPS, frame times, per-system breakdown
```

**Files:**
- `examples/benchmark_p34.rs` (new)
- `crates/studio_core/src/benchmark.rs` (extend)

---

## Phase O2: GPU Profiling Integration

**Priority:** HIGH  
**Outcome:** Visibility into GPU-side performance

### Tasks

1. **Integrate wgpu profiling**:
   - Enable timestamp queries
   - Measure per-pass GPU time
   - Track GPU memory usage

2. **Create render graph timing**:
   - Wrap each node with timing
   - Report: Shadow, GBuffer, GTAO, Lighting, Bloom, Sky

3. **Output format**:
   ```json
   {
     "gpu_timing_ms": {
       "shadow_moon1": 1.2,
       "shadow_moon2": 1.1,
       "gbuffer": 3.4,
       "gtao_prefilter": 0.8,
       "gtao_main": 2.1,
       "gtao_denoise": 0.5,
       "lighting": 1.8,
       "bloom": 1.5,
       "sky_dome": 0.9
     }
   }
   ```

**Files:**
- `crates/studio_core/src/deferred/profiling.rs` (new)
- Modify all `*_node.rs` files to add timing

---

## Phase O3: CPU Profiling Points

**Priority:** HIGH  
**Outcome:** Identify CPU bottlenecks in mesh generation and physics

### Tasks

1. **Add tracing spans to critical paths**:
   ```rust
   #[tracing::instrument(skip_all)]
   fn build_merged_chunk_mesh(...) {
       // ...
   }
   ```

2. **Key functions to instrument**:
   - `build_chunk_mesh_greedy_with_borders`
   - `detect_terrain_collisions`
   - `compute_kinematic_correction`
   - `Model::step` (MarkovJunior)
   - `sync_generation_to_layer`

3. **Create Tracy/chrome tracing export**:
   - Use `tracing-chrome` or `tracing-tracy`
   - Visualize frame breakdown

**Files:**
- `Cargo.toml` - Add tracing dependencies
- Various source files - Add `#[tracing::instrument]`

---

## Phase O4: Mesh Generation Optimization

**Priority:** HIGH  
**Outcome:** Faster chunk mesh generation

### Current Analysis

```rust
// voxel_mesh.rs - build_chunk_mesh_greedy_with_borders
// Estimated complexity: O(32^3) = O(32768) per chunk
// Current: Runs on main thread, blocks frame
```

### Optimization Strategies

1. **Async Mesh Generation**:
   - Move to `AsyncComputeTaskPool`
   - Queue mesh builds, don't block main thread
   - Use double-buffering for mesh updates

2. **Incremental Updates**:
   - Track which chunks are dirty
   - Only rebuild changed chunks
   - Already partially implemented via `VoxelLayers`

3. **Greedy Meshing Improvements**:
   - Profile current greedy algorithm
   - Consider spatial hashing for faster neighbor lookup
   - Batch face emission to reduce allocations

**Verification:**
```bash
cargo run --example benchmark_p34 --release
# Compare mesh generation times before/after
```

**Files:**
- `crates/studio_core/src/voxel_mesh.rs`
- `crates/studio_core/src/chunk_streaming.rs`

---

## Phase O5: Physics GPU Migration

**Priority:** MEDIUM (Design exists, implementation needed)  
**Outcome:** Spring-damper physics runs on GPU compute

### Current State

- `physics_math.rs` has pure math functions designed for GPU
- `voxel_collision_gpu.rs` has GPU collision infrastructure
- `voxel_collision.wgsl` shader exists but may be incomplete

### Migration Plan

1. **Verify GPU collision shader**:
   - Audit `voxel_collision.wgsl` against `physics_math.rs`
   - Ensure all physics stages are implemented

2. **Create GPU physics pipeline**:
   - Upload particle positions to storage buffer
   - Run collision detection compute
   - Run force aggregation compute
   - Run integration compute
   - Read back positions

3. **Hybrid approach**:
   - Keep character controller on CPU (needs immediate response)
   - Move VoxelFragment physics to GPU

**Files:**
- `assets/shaders/voxel_collision.wgsl`
- `crates/studio_core/src/voxel_collision_gpu.rs`
- `crates/studio_core/src/deferred/collision_node.rs`

---

## Phase O6: Point Light Optimization

**Priority:** MEDIUM  
**Outcome:** Efficient handling of many point lights

### Current Analysis

```wgsl
// deferred_lighting.wgsl - point light loop
// Current: O(n) per fragment, iterates ALL lights
// With 256 lights on 1920x1080: 530 million light checks/frame
```

### Optimization Strategies

1. **Spatial Clustering**:
   - Divide screen into tiles
   - Assign lights to tiles
   - Only process lights affecting each tile

2. **Light Culling**:
   - Frustum cull lights before upload
   - Distance cull from camera

3. **Deferred Light Volumes**:
   - Render light spheres to stencil
   - Only shade pixels inside light radius

**Files:**
- `assets/shaders/deferred_lighting.wgsl`
- `crates/studio_core/src/deferred/point_light.rs`

---

## Phase O7: GTAO Optimization

**Priority:** MEDIUM  
**Outcome:** Faster ambient occlusion with similar quality

### Current State

- XeGTAO-compliant implementation
- Quality presets: Low (1 slice, 2 steps) to Ultra (9 slices, 3 steps)
- Half-resolution rendering
- Bilateral denoising

### Optimization Strategies

1. **Verify optimal quality preset**:
   - Benchmark Low vs Medium vs High
   - Find quality/performance sweet spot

2. **Temporal accumulation**:
   - Reuse previous frame's AO
   - Only compute new samples, blend with history
   - Significant quality boost with minimal cost

3. **Adaptive sampling**:
   - More samples near edges
   - Fewer samples in flat areas

**Files:**
- `crates/studio_core/src/deferred/gtao.rs`
- `assets/shaders/gtao.wgsl`

---

## Phase O8: Shadow Map Optimization

**Priority:** LOW  
**Outcome:** Efficient dual-moon shadows

### Current State

- Two 2048x2048 shadow maps (Moon1 + Moon2)
- Full scene rendered twice for shadows
- 3x3 PCF filtering

### Optimization Strategies

1. **Cascaded Shadow Maps**:
   - Multiple cascades for different distances
   - Higher resolution near camera

2. **Shadow Caching**:
   - Static geometry cached
   - Only re-render dynamic objects

3. **Single-Pass Dual Shadow**:
   - Render both moons in one pass with geometry shader
   - Reduces geometry submission

**Files:**
- `crates/studio_core/src/deferred/shadow_node.rs`
- `assets/shaders/shadow_depth.wgsl`

---

## Benchmark Metrics to Track

### Frame Budget (60 FPS = 16.67ms)

| Phase | Target | Current (estimate) |
|-------|--------|-------------------|
| CPU Update | < 2ms | ? |
| Shadow Passes | < 2ms | ? |
| G-Buffer | < 3ms | ? |
| GTAO | < 2ms | ? |
| Lighting | < 2ms | ? |
| Bloom | < 1ms | ? |
| Sky Dome | < 1ms | ? |
| **Total** | **< 13ms** | **?** |

### Key Metrics

- **FPS** (avg, min, max, 1% low)
- **Frame time variance** (stuttering)
- **Draw calls per frame**
- **Triangles rendered**
- **GPU memory usage**
- **CPU time per system**

---

## Implementation Order

```
O1 (Benchmark Harness) ─────────────────┐
        │                               │
        └──> O2 (GPU Profiling) ────────┤
                    │                   │
        ┌───────────┘                   │
        │                               │
O3 (CPU Profiling) ─────────────────────┴──> PROFILING COMPLETE
                                               │
                                               v
                    ┌─────────────────────────────────────────┐
                    │                                         │
          O4 (Mesh Gen) ─────> O5 (GPU Physics) ─────> O6 (Point Lights)
                    │                                         │
                    └──> O7 (GTAO) ──────> O8 (Shadows) ──────┘
                                               │
                                               v
                                    OPTIMIZATIONS COMPLETE
```

---

## Quick Profiling Commands

```bash
# Basic FPS benchmark
cargo run --example p34_sky_terrain_test --release

# With benchmark plugin (if added)
cargo run --example benchmark_p34 --release -- --duration=60

# Tracy profiling (requires tracy-client feature)
TRACY_NO_EXIT=1 cargo run --example p34_sky_terrain_test --release --features tracy

# Frame capture with RenderDoc
renderdoccmd capture cargo run --example p34_sky_terrain_test --release
```

---

## Success Criteria

- [ ] Benchmark harness produces reproducible measurements
- [ ] GPU timing visible per render pass
- [ ] CPU hotspots identified via tracing
- [ ] 60 FPS maintained on target hardware with full scene
- [ ] No frame time spikes > 5ms variance
- [ ] Memory usage stable (no leaks over time)

---

## References

- Existing benchmark: `crates/studio_core/src/benchmark.rs`
- Physics reference: `docs/physics/ARCHITECTURE.md`
- Render pipeline: `docs/ARCHITECTURE.md`
- GTAO docs: `docs/GTAO.md`
