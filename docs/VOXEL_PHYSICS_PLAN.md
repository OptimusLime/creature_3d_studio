# Voxel Physics & Dynamic Fragments Plan

## Executive Summary

We are extending our existing `VoxelWorld` system to support **dynamic voxel fragments** - pieces of the world that can be broken off, moved through physics, and merged back. This enables destruction, cut/paste, and eventually character interaction.

**Key Insight**: We already have excellent voxel data structures and greedy meshing. We need to add:
1. Region extraction/merge operations on `VoxelWorld`
2. A `VoxelFragment` wrapper for physics-active pieces
3. Trimesh collider generation from our existing greedy mesh output
4. Settle/merge lifecycle for fragments

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CURRENT STATE                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  VoxelWorld (HashMap<ChunkPos, VoxelChunk>)                                 │
│       │                                                                      │
│       ▼                                                                      │
│  build_world_meshes_cross_chunk() ──► Mesh + VoxelMaterial ──► GPU          │
│                                                                              │
│  (No physics, no dynamic pieces)                                            │
└─────────────────────────────────────────────────────────────────────────────┘

                                    ▼▼▼

┌─────────────────────────────────────────────────────────────────────────────┐
│                              TARGET STATE                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  VoxelWorld (static terrain)          VoxelFragment (dynamic piece)         │
│       │                                     │                                │
│       │                                     ├── VoxelWorld (small, sparse)   │
│       │                                     ├── Transform (physics position) │
│       │                                     ├── RigidBody::Dynamic           │
│       │                                     └── Collider (Trimesh)           │
│       │                                                                      │
│       ▼                                     ▼                                │
│  Mesh + Collider(Trimesh)             Mesh + Collider(Trimesh)              │
│  RigidBody::Fixed                     RigidBody::Dynamic                    │
│                                                                              │
│  ◄──────────── settle_fragment() merges back ────────────►                  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Model

### Existing (No Changes Needed)

```rust
// voxel.rs - KEEP AS IS
pub struct Voxel { color: [u8; 3], emission: u8 }
pub struct VoxelChunk { voxels: Box<[Option<Voxel>; 32768]> }
pub struct VoxelWorld { chunks: HashMap<ChunkPos, VoxelChunk> }
```

### New Types

```rust
// voxel.rs - ADD these methods to VoxelWorld
impl VoxelWorld {
    /// Extract voxels within a sphere, removing them from self.
    /// Returns a new VoxelWorld containing only the extracted voxels,
    /// with coordinates relative to the sphere center.
    pub fn split_sphere(&mut self, center: IVec3, radius: i32) -> VoxelWorld;
    
    /// Extract voxels within an AABB, removing them from self.
    /// Returns a new VoxelWorld with coordinates relative to min corner.
    pub fn split_aabb(&mut self, min: IVec3, max: IVec3) -> VoxelWorld;
    
    /// Merge another world into self at the given offset.
    /// Overwrites existing voxels at overlapping positions.
    pub fn merge_from(&mut self, other: &VoxelWorld, offset: IVec3);
    
    /// Check if merging would cause any overlaps.
    /// Returns list of positions that would collide.
    pub fn check_merge_collisions(&self, other: &VoxelWorld, offset: IVec3) -> Vec<IVec3>;
    
    /// Shift all voxels by offset (for recentering after split).
    pub fn translate(&mut self, offset: IVec3);
    
    /// Get the centroid of all voxels (for physics center of mass).
    pub fn centroid(&self) -> Option<Vec3>;
}
```

```rust
// voxel_fragment.rs - NEW FILE
use bevy::prelude::*;
use crate::voxel::VoxelWorld;

/// A dynamic piece of voxel geometry that exists in the physics world.
/// 
/// Fragments are created by breaking/cutting pieces from the main world.
/// They have their own physics body and can move, rotate, and collide.
/// Eventually they "settle" and merge back into a static VoxelWorld.
#[derive(Component)]
pub struct VoxelFragment {
    /// The voxel data for this fragment (coordinates relative to entity origin)
    pub data: VoxelWorld,
    /// Whether this fragment is settling (velocity near zero for N frames)
    pub settling_frames: u32,
    /// Original world position when broken off (for debugging)
    pub origin: IVec3,
}

/// Marker for fragments that are in "preview" mode (clipboard paste preview).
/// These render with transparency and don't have physics.
#[derive(Component)]
pub struct FragmentPreview;

/// Marker for the main static world entity.
#[derive(Component)]
pub struct StaticVoxelWorld;

/// Configuration for fragment behavior.
#[derive(Resource)]
pub struct FragmentConfig {
    /// Frames of low velocity before settling
    pub settle_threshold_frames: u32,
    /// Velocity magnitude below which we count as "still"
    pub settle_velocity_threshold: f32,
    /// Maximum fragments before forcing oldest to settle
    pub max_active_fragments: usize,
}

impl Default for FragmentConfig {
    fn default() -> Self {
        Self {
            settle_threshold_frames: 60,  // 1 second at 60fps
            settle_velocity_threshold: 0.1,
            max_active_fragments: 32,
        }
    }
}
```

---

## Physics Collider Strategy

### Why Trimesh (Not Rapier's Voxels Shape)

After analysis, we will use **Trimesh colliders** generated from our greedy mesh output:

1. **We already have greedy meshing** - `build_chunk_mesh_greedy()` outputs optimized quads
2. **Trimesh is well-supported** - Rapier handles trimesh colliders efficiently
3. **Consistency** - Same geometry for rendering and physics (no desync)
4. **Dynamic updates** - Rebuild trimesh when voxels change (acceptable for fragments which are small)

### Collider Generation

```rust
// voxel_physics.rs - NEW FILE

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::voxel::VoxelWorld;
use crate::voxel_mesh::{build_world_meshes_cross_chunk, ChunkMesh};

/// Generate a single merged Trimesh collider from a VoxelWorld.
/// 
/// This combines all chunk meshes into one collider for physics.
/// For small fragments, this is efficient. For the main world,
/// we may want per-chunk colliders instead.
pub fn generate_trimesh_collider(world: &VoxelWorld) -> Option<Collider> {
    let chunk_meshes = build_world_meshes_cross_chunk(world);
    if chunk_meshes.is_empty() {
        return None;
    }
    
    let mut all_vertices: Vec<Vec3> = Vec::new();
    let mut all_indices: Vec<[u32; 3]> = Vec::new();
    
    for chunk_mesh in chunk_meshes {
        let base_idx = all_vertices.len() as u32;
        let offset = Vec3::from_array(chunk_mesh.world_offset);
        
        // Extract positions from mesh
        if let Some(positions) = chunk_mesh.mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            if let bevy::mesh::VertexAttributeValues::Float32x3(verts) = positions {
                for v in verts {
                    all_vertices.push(Vec3::from_array(*v) + offset);
                }
            }
        }
        
        // Extract indices
        if let Some(Indices::U32(indices)) = chunk_mesh.mesh.indices() {
            for chunk in indices.chunks(3) {
                all_indices.push([
                    chunk[0] + base_idx,
                    chunk[1] + base_idx,
                    chunk[2] + base_idx,
                ]);
            }
        }
    }
    
    if all_vertices.is_empty() || all_indices.is_empty() {
        return None;
    }
    
    Some(Collider::trimesh(all_vertices, all_indices))
}

/// Generate per-chunk colliders for the main world (better for large worlds).
pub fn generate_chunk_colliders(world: &VoxelWorld) -> Vec<(ChunkPos, Collider, Vec3)> {
    // Returns (chunk_pos, collider, world_offset) for each chunk
    // Implementation: same as above but per-chunk
    todo!()
}
```

---

## Phase 22: Core Data Operations

**Goal**: Implement `split_sphere`, `split_aabb`, `merge_from`, and `translate` on `VoxelWorld` with comprehensive unit tests.

### 22.1: Implement `VoxelWorld::split_aabb`

**File**: `crates/studio_core/src/voxel.rs`

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 22.1.1 | Add `split_aabb(&mut self, min: IVec3, max: IVec3) -> VoxelWorld` method | Compiles |
| 22.1.2 | Iterate world voxels in AABB range | N/A |
| 22.1.3 | Remove matching voxels from self | N/A |
| 22.1.4 | Insert into new VoxelWorld with coordinates relative to `min` | N/A |
| 22.1.5 | Handle negative coordinates correctly | N/A |
| 22.1.6 | Prune empty chunks from self after extraction | N/A |

**Unit Tests** (add to `voxel.rs` `#[cfg(test)]` module):

```rust
#[test]
fn test_split_aabb_basic() {
    let mut world = VoxelWorld::new();
    // Create 4x4x4 cube at origin
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
            }
        }
    }
    assert_eq!(world.total_voxel_count(), 64);
    
    // Split out 2x2x2 corner
    let fragment = world.split_aabb(IVec3::ZERO, IVec3::new(2, 2, 2));
    
    // Fragment has 8 voxels (2x2x2)
    assert_eq!(fragment.total_voxel_count(), 8);
    // Original has 64 - 8 = 56 voxels
    assert_eq!(world.total_voxel_count(), 56);
    // Fragment coordinates are relative to min (0,0,0)
    assert!(fragment.get_voxel(0, 0, 0).is_some());
    assert!(fragment.get_voxel(1, 1, 1).is_some());
    assert!(fragment.get_voxel(2, 2, 2).is_none()); // exclusive upper bound
}

#[test]
fn test_split_aabb_empty_region() {
    let mut world = VoxelWorld::new();
    world.set_voxel(10, 10, 10, Voxel::solid(255, 0, 0));
    
    // Split empty region
    let fragment = world.split_aabb(IVec3::ZERO, IVec3::new(5, 5, 5));
    
    assert_eq!(fragment.total_voxel_count(), 0);
    assert_eq!(world.total_voxel_count(), 1); // Original unchanged
}

#[test]
fn test_split_aabb_negative_coordinates() {
    let mut world = VoxelWorld::new();
    // Voxels spanning negative to positive
    for x in -2..2 {
        for y in -2..2 {
            for z in -2..2 {
                world.set_voxel(x, y, z, Voxel::solid(100, 100, 100));
            }
        }
    }
    assert_eq!(world.total_voxel_count(), 64);
    
    // Split negative quadrant
    let fragment = world.split_aabb(IVec3::new(-2, -2, -2), IVec3::ZERO);
    
    assert_eq!(fragment.total_voxel_count(), 8);
    // Fragment coords relative to min (-2,-2,-2), so (0,0,0) in fragment = (-2,-2,-2) in world
    assert!(fragment.get_voxel(0, 0, 0).is_some());
    assert!(fragment.get_voxel(1, 1, 1).is_some());
}

#[test]
fn test_split_aabb_preserves_voxel_data() {
    let mut world = VoxelWorld::new();
    world.set_voxel(5, 5, 5, Voxel::new(100, 150, 200, 128));
    
    let fragment = world.split_aabb(IVec3::new(5, 5, 5), IVec3::new(6, 6, 6));
    
    let voxel = fragment.get_voxel(0, 0, 0).unwrap();
    assert_eq!(voxel.color, [100, 150, 200]);
    assert_eq!(voxel.emission, 128);
}

#[test]
fn test_split_aabb_cross_chunk_boundary() {
    let mut world = VoxelWorld::new();
    // Voxels spanning chunk boundary (chunk size = 32)
    for x in 30..34 {
        world.set_voxel(x, 0, 0, Voxel::solid(255, 0, 0));
    }
    assert_eq!(world.chunk_count(), 2); // Chunks (0,0,0) and (1,0,0)
    
    let fragment = world.split_aabb(IVec3::new(30, 0, 0), IVec3::new(34, 1, 1));
    
    assert_eq!(fragment.total_voxel_count(), 4);
    assert_eq!(world.total_voxel_count(), 0);
}
```

### 22.2: Implement `VoxelWorld::split_sphere`

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 22.2.1 | Add `split_sphere(&mut self, center: IVec3, radius: i32) -> VoxelWorld` | Compiles |
| 22.2.2 | Compute AABB from center ± radius | N/A |
| 22.2.3 | Iterate voxels in AABB, check distance² ≤ radius² | N/A |
| 22.2.4 | Extract matching voxels, coordinates relative to center | N/A |

**Unit Tests**:

```rust
#[test]
fn test_split_sphere_basic() {
    let mut world = VoxelWorld::new();
    // Create solid 10x10x10 cube centered at (5,5,5)
    for x in 0..10 {
        for y in 0..10 {
            for z in 0..10 {
                world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
            }
        }
    }
    
    // Split sphere of radius 2 at center (5,5,5)
    let fragment = world.split_sphere(IVec3::new(5, 5, 5), 2);
    
    // Sphere of radius 2: approximately 33 voxels (4/3 * π * 2³ ≈ 33)
    // Exact count depends on discrete sampling
    assert!(fragment.total_voxel_count() > 20);
    assert!(fragment.total_voxel_count() < 50);
    
    // Center voxel should be at (0,0,0) in fragment (relative to center)
    assert!(fragment.get_voxel(0, 0, 0).is_some());
    
    // Original should have hole
    assert!(world.get_voxel(5, 5, 5).is_none());
}

#[test]
fn test_split_sphere_at_edge() {
    let mut world = VoxelWorld::new();
    // Floor at y=0
    for x in 0..20 {
        for z in 0..20 {
            world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
        }
    }
    
    // Sphere at floor level - should only get hemisphere
    let fragment = world.split_sphere(IVec3::new(10, 0, 10), 3);
    
    // Should get roughly half a sphere (floor cuts it)
    assert!(fragment.total_voxel_count() > 10);
    assert!(fragment.total_voxel_count() < 60);
}
```

### 22.3: Implement `VoxelWorld::merge_from`

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 22.3.1 | Add `merge_from(&mut self, other: &VoxelWorld, offset: IVec3)` | Compiles |
| 22.3.2 | Iterate other's voxels | N/A |
| 22.3.3 | Add to self at position + offset | N/A |
| 22.3.4 | Overwrite mode: replace existing voxels | N/A |

**Unit Tests**:

```rust
#[test]
fn test_merge_from_empty_into_empty() {
    let mut world = VoxelWorld::new();
    let other = VoxelWorld::new();
    
    world.merge_from(&other, IVec3::ZERO);
    
    assert_eq!(world.total_voxel_count(), 0);
}

#[test]
fn test_merge_from_basic() {
    let mut world = VoxelWorld::new();
    let mut other = VoxelWorld::new();
    
    other.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
    other.set_voxel(1, 0, 0, Voxel::solid(0, 255, 0));
    
    world.merge_from(&other, IVec3::new(10, 10, 10));
    
    assert_eq!(world.total_voxel_count(), 2);
    assert!(world.get_voxel(10, 10, 10).is_some());
    assert!(world.get_voxel(11, 10, 10).is_some());
}

#[test]
fn test_merge_from_overwrites() {
    let mut world = VoxelWorld::new();
    world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0)); // Red
    
    let mut other = VoxelWorld::new();
    other.set_voxel(0, 0, 0, Voxel::solid(0, 0, 255)); // Blue
    
    world.merge_from(&other, IVec3::new(5, 5, 5));
    
    let voxel = world.get_voxel(5, 5, 5).unwrap();
    assert_eq!(voxel.color, [0, 0, 255]); // Should be blue (overwritten)
}

#[test]
fn test_merge_from_negative_offset() {
    let mut world = VoxelWorld::new();
    let mut other = VoxelWorld::new();
    
    other.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
    
    world.merge_from(&other, IVec3::new(-10, -10, -10));
    
    assert!(world.get_voxel(-5, -5, -5).is_some());
}

#[test]
fn test_split_then_merge_roundtrip() {
    let mut world = VoxelWorld::new();
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
            }
        }
    }
    let original_count = world.total_voxel_count();
    
    // Split out a piece
    let fragment = world.split_aabb(IVec3::new(1, 1, 1), IVec3::new(3, 3, 3));
    let fragment_count = fragment.total_voxel_count();
    
    // Merge back at same location
    world.merge_from(&fragment, IVec3::new(1, 1, 1));
    
    assert_eq!(world.total_voxel_count(), original_count);
}
```

### 22.4: Implement `VoxelWorld::translate` and `centroid`

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 22.4.1 | Add `translate(&mut self, offset: IVec3)` | Compiles |
| 22.4.2 | Add `centroid(&self) -> Option<Vec3>` | Compiles |

**Unit Tests**:

```rust
#[test]
fn test_translate_basic() {
    let mut world = VoxelWorld::new();
    world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
    world.set_voxel(1, 0, 0, Voxel::solid(0, 255, 0));
    
    world.translate(IVec3::new(10, 20, 30));
    
    assert!(world.get_voxel(0, 0, 0).is_none());
    assert!(world.get_voxel(10, 20, 30).is_some());
    assert!(world.get_voxel(11, 20, 30).is_some());
}

#[test]
fn test_centroid_single_voxel() {
    let mut world = VoxelWorld::new();
    world.set_voxel(10, 20, 30, Voxel::solid(255, 0, 0));
    
    let centroid = world.centroid().unwrap();
    
    // Centroid should be center of voxel (10.5, 20.5, 30.5)
    assert!((centroid.x - 10.5).abs() < 0.01);
    assert!((centroid.y - 20.5).abs() < 0.01);
    assert!((centroid.z - 30.5).abs() < 0.01);
}

#[test]
fn test_centroid_symmetric() {
    let mut world = VoxelWorld::new();
    // 2x2x2 cube at origin
    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
            }
        }
    }
    
    let centroid = world.centroid().unwrap();
    
    // Centroid should be at (1, 1, 1)
    assert!((centroid.x - 1.0).abs() < 0.01);
    assert!((centroid.y - 1.0).abs() < 0.01);
    assert!((centroid.z - 1.0).abs() < 0.01);
}
```

### 22.5: Implement `check_merge_collisions`

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 22.5.1 | Add `check_merge_collisions(&self, other: &VoxelWorld, offset: IVec3) -> Vec<IVec3>` | Compiles |

**Unit Tests**:

```rust
#[test]
fn test_check_merge_collisions_none() {
    let mut world = VoxelWorld::new();
    world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
    
    let mut other = VoxelWorld::new();
    other.set_voxel(0, 0, 0, Voxel::solid(0, 255, 0));
    
    // No collision - different positions
    let collisions = world.check_merge_collisions(&other, IVec3::new(10, 0, 0));
    assert!(collisions.is_empty());
}

#[test]
fn test_check_merge_collisions_overlap() {
    let mut world = VoxelWorld::new();
    world.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
    
    let mut other = VoxelWorld::new();
    other.set_voxel(0, 0, 0, Voxel::solid(0, 255, 0));
    
    // Collision at (5,5,5)
    let collisions = world.check_merge_collisions(&other, IVec3::new(5, 5, 5));
    assert_eq!(collisions.len(), 1);
    assert_eq!(collisions[0], IVec3::new(5, 5, 5));
}
```

---

## Phase 23: Physics Integration

**Goal**: Generate Trimesh colliders from `VoxelWorld` and integrate with bevy_rapier3d.

### 23.1: Add bevy_rapier3d Dependency

**File**: `Cargo.toml` (workspace)

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 23.1.1 | Add `bevy_rapier3d = "0.32"` to workspace dependencies | `cargo check` passes |
| 23.1.2 | Add feature flag `physics` to studio_core | Compiles with feature |

### 23.2: Implement Trimesh Collider Generation

**File**: `crates/studio_core/src/voxel_physics.rs` (NEW)

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 23.2.1 | Create `voxel_physics.rs` module | Compiles |
| 23.2.2 | Implement `generate_trimesh_collider(world: &VoxelWorld) -> Option<Collider>` | Unit test |
| 23.2.3 | Handle empty world (return None) | Unit test |

**Unit Tests**:

```rust
#[test]
fn test_generate_trimesh_empty_world() {
    let world = VoxelWorld::new();
    let collider = generate_trimesh_collider(&world);
    assert!(collider.is_none());
}

#[test]
fn test_generate_trimesh_single_voxel() {
    let mut world = VoxelWorld::new();
    world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
    
    let collider = generate_trimesh_collider(&world);
    assert!(collider.is_some());
    
    // Single voxel = 6 faces = 12 triangles = 36 indices
    // (Verify via collider shape inspection if possible)
}

#[test]
fn test_generate_trimesh_greedy_merged() {
    let mut world = VoxelWorld::new();
    // 4x4x4 same-color cube should greedy merge to 6 quads
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                world.set_voxel(x, y, z, Voxel::solid(128, 128, 128));
            }
        }
    }
    
    let collider = generate_trimesh_collider(&world);
    assert!(collider.is_some());
    
    // 6 quads = 12 triangles (greedy meshing benefit applies to physics too)
}
```

### 23.3: VoxelFragment Component and Systems

**File**: `crates/studio_core/src/voxel_fragment.rs` (NEW)

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 23.3.1 | Define `VoxelFragment` component | Compiles |
| 23.3.2 | Define `FragmentPreview` marker component | Compiles |
| 23.3.3 | Define `StaticVoxelWorld` marker component | Compiles |
| 23.3.4 | Define `FragmentConfig` resource | Compiles |
| 23.3.5 | Implement `spawn_fragment(commands, data, transform) -> Entity` | Integration test |

**Integration Test** (requires Bevy app):

```rust
#[test]
fn test_spawn_fragment_creates_entity() {
    // Create minimal Bevy app
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
    
    // Spawn fragment
    let mut world_data = VoxelWorld::new();
    world_data.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
    
    app.world.spawn_fragment(world_data, Transform::from_xyz(10.0, 10.0, 10.0));
    
    app.update();
    
    // Verify entity exists with components
    let fragment_query = app.world.query::<(&VoxelFragment, &RigidBody, &Collider)>();
    assert_eq!(fragment_query.iter(&app.world).count(), 1);
}
```

### 23.4: Fragment Mesh Synchronization

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 23.4.1 | System: `sync_fragment_meshes` - generates Mesh from VoxelFragment.data | Visual test |
| 23.4.2 | System: `sync_fragment_colliders` - regenerates Collider when data changes | Integration test |

**Visual Test**: Example `p22_voxel_fragment.rs`

```rust
// Spawns a fragment, verifies it renders and falls due to gravity
fn main() {
    VoxelWorldApp::new("Fragment Test")
        .with_world_builder(|world| {
            // Ground plane
            for x in -10..10 {
                for z in -10..10 {
                    world.set_voxel(x, 0, z, Voxel::solid(100, 100, 100));
                }
            }
        })
        .with_setup(|commands, _world| {
            // Spawn a floating fragment
            let mut fragment_data = VoxelWorld::new();
            for x in 0..3 {
                for y in 0..3 {
                    for z in 0..3 {
                        fragment_data.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
                    }
                }
            }
            spawn_fragment(commands, fragment_data, Transform::from_xyz(0.0, 10.0, 0.0));
        })
        .with_screenshot("screenshots/p22_fragment.png")
        .run();
}
```

**Verification**: Screenshot shows red 3x3x3 cube falling onto gray ground, coming to rest.

---

## Phase 24: Break and Settle Lifecycle

**Goal**: Implement the full lifecycle: break piece from world → physics simulation → settle back into world.

### 24.1: Break Event and System

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 24.1.1 | Define `BreakVoxelEvent { center: IVec3, radius: i32, impulse: Vec3 }` | Compiles |
| 24.1.2 | System: `handle_break_events` - reads events, calls split_sphere, spawns fragment | Integration test |
| 24.1.3 | Apply initial impulse to fragment RigidBody | Visual test |

**Integration Test**:

```rust
#[test]
fn test_break_event_creates_fragment() {
    let mut app = /* setup */;
    
    // Insert world with voxels
    let mut world = VoxelWorld::new();
    for x in 0..10 { for y in 0..10 { for z in 0..10 {
        world.set_voxel(x, y, z, Voxel::solid(255, 0, 0));
    }}}
    app.insert_resource(MainVoxelWorld(world));
    
    // Send break event
    app.world.send_event(BreakVoxelEvent {
        center: IVec3::new(5, 5, 5),
        radius: 2,
        impulse: Vec3::new(0.0, 10.0, 0.0),
    });
    
    app.update();
    
    // Verify fragment created
    let fragments: Vec<_> = app.world.query::<&VoxelFragment>().iter(&app.world).collect();
    assert_eq!(fragments.len(), 1);
    assert!(fragments[0].data.total_voxel_count() > 0);
    
    // Verify main world has hole
    let main_world = app.world.resource::<MainVoxelWorld>();
    assert!(main_world.0.get_voxel(5, 5, 5).is_none());
}
```

### 24.2: Settle Detection System

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 24.2.1 | System: `detect_settling_fragments` - increments settling_frames when velocity low | Unit test |
| 24.2.2 | Reset settling_frames when velocity exceeds threshold | Unit test |

### 24.3: Merge Settled Fragments

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 24.3.1 | System: `merge_settled_fragments` - when settling_frames > threshold | Integration test |
| 24.3.2 | Convert fragment Transform to world IVec3 offset (round to nearest) | Unit test |
| 24.3.3 | Call `main_world.merge_from(&fragment.data, offset)` | Integration test |
| 24.3.4 | Despawn fragment entity | Integration test |
| 24.3.5 | Trigger main world mesh/collider rebuild | Visual test |

**Integration Test**:

```rust
#[test]
fn test_fragment_settles_and_merges() {
    let mut app = /* setup with physics */;
    
    // Ground + fragment above it
    /* ... */
    
    // Run physics until fragment settles
    for _ in 0..120 { // 2 seconds at 60fps
        app.update();
    }
    
    // Fragment should be merged back
    let fragments: Vec<_> = app.world.query::<&VoxelFragment>().iter(&app.world).collect();
    assert_eq!(fragments.len(), 0, "Fragment should have merged");
    
    // Main world should have fragment's voxels
    let main_world = app.world.resource::<MainVoxelWorld>();
    assert!(main_world.0.total_voxel_count() > initial_count);
}
```

---

## Phase 25: Rendering Integration

**Goal**: Ensure fragments render correctly alongside the main world, including transparency for preview mode.

### 25.1: Fragment Mesh Generation

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 25.1.1 | Fragments use same `VoxelMaterial` as main world | Visual test |
| 25.1.2 | Fragment mesh centered at entity origin (not world origin) | Visual test |
| 25.1.3 | Mesh rebuilds when `VoxelFragment.data` changes | Visual test |

### 25.2: Preview Transparency (Clipboard Paste Preview)

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 25.2.1 | Add `VoxelMaterialTransparent` variant with alpha support | Compiles |
| 25.2.2 | `FragmentPreview` entities use transparent material | Visual test |
| 25.2.3 | Preview fragments have no physics (no Collider/RigidBody) | Unit test |

### 25.3: Main World Collider Updates

**Tasks**:
| ID | Task | Verification |
|----|------|--------------|
| 25.3.1 | `StaticVoxelWorld` entity has Trimesh Collider | Integration test |
| 25.3.2 | Collider rebuilds when world voxels change (break/merge) | Integration test |
| 25.3.3 | Consider chunked colliders for large worlds (optimization) | Design decision |

---

## Phase 26: Input and Interaction (Future)

**Goal**: User-facing tools for breaking, copying, and pasting.

### 26.1: Raycast Selection

- Raycast from camera through mouse cursor
- Identify hit voxel position
- Highlight targeted voxel/region

### 26.2: Break Tool

- Click to break sphere at cursor
- Configurable radius
- Fragment flies away with impulse

### 26.3: Copy/Paste (Clipboard)

- Select region (AABB or sphere)
- Copy to clipboard (no modification to world)
- Paste preview (transparent, follows cursor)
- Confirm paste (merge into world)

---

## File Structure

```
crates/studio_core/src/
├── voxel.rs                 # ADD: split_sphere, split_aabb, merge_from, translate, centroid
├── voxel_mesh.rs            # NO CHANGES (already complete)
├── voxel_fragment.rs        # NEW: VoxelFragment component, spawn/despawn
├── voxel_physics.rs         # NEW: Trimesh collider generation
├── voxel_interaction.rs     # NEW (Phase 26): Break/Copy/Paste tools
└── lib.rs                   # Export new modules

examples/
├── p22_voxel_fragment.rs    # Fragment spawning and physics
├── p23_break_settle.rs      # Full break → settle → merge cycle
└── p24_copy_paste.rs        # Clipboard preview and paste
```

---

## Test Summary

| Phase | Unit Tests | Integration Tests | Visual Tests |
|-------|------------|-------------------|--------------|
| 22.1 split_aabb | 5 | 0 | 0 |
| 22.2 split_sphere | 2 | 0 | 0 |
| 22.3 merge_from | 5 | 0 | 0 |
| 22.4 translate/centroid | 3 | 0 | 0 |
| 22.5 check_collisions | 2 | 0 | 0 |
| 23.2 trimesh collider | 3 | 0 | 0 |
| 23.3 fragment component | 0 | 1 | 0 |
| 23.4 fragment sync | 0 | 1 | 1 |
| 24.1 break event | 0 | 1 | 1 |
| 24.2 settle detection | 2 | 0 | 0 |
| 24.3 merge settled | 0 | 2 | 1 |
| 25.1-25.3 rendering | 0 | 2 | 3 |
| **Total** | **22** | **7** | **6** |

---

## Risk Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| bevy_rapier3d incompatible with Bevy 0.17 | Medium | High | Check compatibility first; fall back to raw rapier3d |
| Trimesh rebuild too slow for large worlds | Medium | Medium | Use per-chunk colliders; throttle rebuilds |
| Fragment physics unstable (jitter) | Low | Medium | Tune Rapier solver settings; use CCD |
| Greedy mesh doesn't match physics mesh | Low | High | Use same mesh source for both (already planned) |
| Transparent material breaks deferred pipeline | Medium | Medium | Use forward pass for previews; separate render layer |

---

## Success Criteria

**Phase 22 Complete**:
- `cargo test -p studio_core split` passes (all split tests)
- `cargo test -p studio_core merge` passes (all merge tests)
- No changes to rendering behavior (data-only phase)

**Phase 23 Complete**:
- `cargo run --example p22_voxel_fragment` shows fragment falling
- Fragment collides with ground and stops
- Screenshot captured successfully

**Phase 24 Complete**:
- `cargo run --example p23_break_settle` shows break → fall → settle → merge
- Main world collider updates when fragment merges
- No visual seams at merge location

**Phase 25 Complete**:
- Transparent preview renders correctly
- Deferred pipeline still works for opaque voxels
- Copy/paste workflow is intuitive

---

## Next Steps

1. **Implement Phase 22.1**: Add `split_aabb` to `VoxelWorld` with tests
2. **Run tests**: `cargo test -p studio_core split_aabb`
3. **Proceed incrementally** through each task

Ready to begin implementation.
