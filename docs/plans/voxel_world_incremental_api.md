# VoxelLayers: Layered Voxel Architecture for MarkovJunior Integration

## Summary

Integrate MarkovJunior procedural generation directly into VoxelWorld with proper collision, emissive lighting, and incremental mesh updates.

## Motivating Example

**`p30_markov_kinematic_animated`** - The ONE example that proves everything works:

```
Player walks on terrain → Presses G → Building generates at specified location →
Player cannot walk through building (collision) → Emissive voxels glow and cast light →
Building animates during generation → Press G again to regenerate elsewhere
```

**What success looks like:**
1. Building appears at world position (not hardcoded origin)
2. Player bounces off building walls (collision works)
3. Yellow/Orange MJ voxels emit light (emissive mapping)
4. Generation animates frame-by-frame (dirty tracking works)
5. No separate "GeneratedBuilding" mesh - it's part of VoxelWorld

**All infrastructure exists to support this single example.**

---

## Core Data Structures

### VoxelLayer

A single voxel layer with world offset and dirty tracking:

```rust
/// A layer of voxels that can be positioned anywhere in world space.
/// Multiple layers composite together for rendering and collision.
pub struct VoxelLayer {
    /// Human-readable name for debugging ("terrain", "generated", etc.)
    pub name: String,
    
    /// Priority for compositing. Higher priority layers override lower.
    /// terrain=0, generated=10, player_placed=20
    pub priority: i32,
    
    /// World offset - layer's local (0,0,0) maps to this world position.
    /// Allows placing generated content anywhere without coordinate math.
    pub offset: IVec3,
    
    /// The actual voxel data, stored in chunks.
    pub world: VoxelWorld,
    
    /// Whether this layer renders.
    pub visible: bool,
    
    /// Whether this layer participates in collision detection.
    pub collidable: bool,
    
    /// Chunks that have been modified since last mesh rebuild.
    /// Key insight: set_voxel() automatically marks chunk dirty.
    dirty_chunks: HashSet<ChunkPos>,
}

impl VoxelLayer {
    pub fn new(name: &str, priority: i32) -> Self {
        Self {
            name: name.to_string(),
            priority,
            offset: IVec3::ZERO,
            world: VoxelWorld::new(),
            visible: true,
            collidable: true,
            dirty_chunks: HashSet::new(),
        }
    }
    
    /// Set voxel at layer-local coordinates. Automatically marks chunk dirty.
    pub fn set_voxel(&mut self, x: i32, y: i32, z: i32, voxel: Voxel) {
        self.world.set_voxel(x, y, z, voxel);
        let chunk_pos = ChunkPos::from_world(x, y, z);
        self.dirty_chunks.insert(chunk_pos);
        // Also mark neighbors dirty if at chunk boundary (for face culling)
        self.mark_neighbors_if_boundary(x, y, z, chunk_pos);
    }
    
    /// Convert layer-local coords to world coords using offset.
    pub fn local_to_world(&self, local: IVec3) -> IVec3 {
        local + self.offset
    }
    
    /// Convert world coords to layer-local coords.
    pub fn world_to_local(&self, world: IVec3) -> IVec3 {
        world - self.offset
    }
    
    /// Take dirty chunks (returns set and clears internal tracking).
    pub fn take_dirty_chunks(&mut self) -> HashSet<ChunkPos> {
        std::mem::take(&mut self.dirty_chunks)
    }
    
    /// Clear a rectangular region efficiently.
    pub fn clear_region(&mut self, min: IVec3, max: IVec3) {
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    self.world.clear_voxel(x, y, z);
                    let chunk_pos = ChunkPos::from_world(x, y, z);
                    self.dirty_chunks.insert(chunk_pos);
                }
            }
        }
    }
}
```

### VoxelLayers

Resource holding all layers with priority-based merging:

```rust
/// Bevy resource containing all voxel layers.
/// Handles merging for render and collision.
#[derive(Resource)]
pub struct VoxelLayers {
    /// Layers sorted by priority (lowest first for iteration).
    layers: Vec<VoxelLayer>,
}

impl VoxelLayers {
    /// Create standard layer setup.
    pub fn new() -> Self {
        Self {
            layers: vec![
                VoxelLayer::new("terrain", 0),
                VoxelLayer::new("generated", 10),
            ],
        }
    }
    
    /// Get mutable reference to layer by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut VoxelLayer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }
    
    /// Get voxel at world position, checking layers by priority (highest first).
    /// Returns first non-empty voxel found.
    pub fn get_voxel(&self, world_x: i32, world_y: i32, world_z: i32) -> Option<Voxel> {
        // Iterate in reverse (highest priority first)
        for layer in self.layers.iter().rev() {
            if !layer.visible { continue; }
            let local = layer.world_to_local(IVec3::new(world_x, world_y, world_z));
            if let Some(voxel) = layer.world.get_voxel(local.x, local.y, local.z) {
                return Some(voxel);
            }
        }
        None
    }
    
    /// Check if position is solid in any collidable layer.
    pub fn is_solid(&self, world_x: i32, world_y: i32, world_z: i32) -> bool {
        for layer in self.layers.iter().rev() {
            if !layer.collidable { continue; }
            let local = layer.world_to_local(IVec3::new(world_x, world_y, world_z));
            if layer.world.is_solid(local.x, local.y, local.z) {
                return true;
            }
        }
        false
    }
    
    /// Collect all dirty chunk positions across all layers.
    /// Returns world-space chunk positions.
    pub fn collect_dirty_chunks(&mut self) -> HashSet<ChunkPos> {
        let mut all_dirty = HashSet::new();
        for layer in &mut self.layers {
            for local_chunk in layer.take_dirty_chunks() {
                // Convert chunk pos to world space
                let world_origin = layer.local_to_world(local_chunk.world_origin());
                all_dirty.insert(ChunkPos::from_world(world_origin.x, world_origin.y, world_origin.z));
            }
        }
        all_dirty
    }
}
```

### ChunkEntityMap

Tracks mesh entities for incremental updates:

```rust
/// Maps chunk positions to their mesh entities for incremental updates.
#[derive(Resource, Default)]
pub struct ChunkEntityMap {
    /// World chunk position → mesh entity
    chunks: HashMap<ChunkPos, Entity>,
    /// World chunk position → light entities from emissive voxels
    lights: HashMap<ChunkPos, Vec<Entity>>,
}

impl ChunkEntityMap {
    pub fn register(&mut self, pos: ChunkPos, entity: Entity) {
        self.chunks.insert(pos, entity);
    }
    
    pub fn get(&self, pos: ChunkPos) -> Option<Entity> {
        self.chunks.get(&pos).copied()
    }
    
    pub fn remove(&mut self, pos: ChunkPos) -> Option<Entity> {
        self.lights.remove(&pos);
        self.chunks.remove(&pos)
    }
}
```

### MjWriteTarget Trait

Allows MarkovJunior to write directly to VoxelLayer:

```rust
/// Trait for anything MarkovJunior can write to.
/// Implemented by MjGrid (existing) and VoxelLayerTarget (new).
pub trait MjWriteTarget {
    /// Set cell value at position.
    fn set(&mut self, x: i32, y: i32, z: i32, value: u8);
    
    /// Get cell value at position.
    fn get(&self, x: i32, y: i32, z: i32) -> u8;
    
    /// Clear all cells to 0.
    fn clear(&mut self);
    
    /// Grid dimensions (mx, my, mz).
    fn dimensions(&self) -> (usize, usize, usize);
}

/// Wrapper that makes VoxelLayer implement MjWriteTarget.
/// Handles coordinate transform (MJ Y/Z swap) and color mapping.
pub struct VoxelLayerTarget<'a> {
    layer: &'a mut VoxelLayer,
    palette: &'a RenderPalette,
    /// MJ character list for reverse lookup (value → char).
    characters: &'a [char],
    /// Grid dimensions for bounds checking.
    mx: usize,
    my: usize, 
    mz: usize,
}

impl<'a> MjWriteTarget for VoxelLayerTarget<'a> {
    fn set(&mut self, x: i32, y: i32, z: i32, value: u8) {
        if value == 0 {
            // MJ value 0 = empty
            // Swap Y/Z for MJ→VoxelWorld coordinate transform
            self.layer.world.clear_voxel(x, z, y);
        } else {
            let ch = self.characters[value as usize];
            let voxel = self.palette.to_voxel(ch);
            // Swap Y/Z: MJ uses Y as height, VoxelWorld uses Z as height
            self.layer.set_voxel(x, z, y, voxel);
        }
    }
    
    fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        // Reverse lookup: voxel → MJ value
        // Uses mj_value stored in Voxel, or 0 if empty
        self.layer.world.get_voxel(x, z, y)
            .map(|v| v.mj_value)
            .unwrap_or(0)
    }
    
    fn clear(&mut self) {
        self.layer.clear_region(
            IVec3::ZERO,
            IVec3::new(self.mx as i32 - 1, self.mz as i32 - 1, self.my as i32 - 1),
        );
    }
    
    fn dimensions(&self) -> (usize, usize, usize) {
        (self.mx, self.my, self.mz)
    }
}
```

### Extended Voxel Struct

Add MJ value storage for reverse lookup:

```rust
/// A single voxel with color, emission, and MJ value for reverse lookup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Voxel {
    pub color: [u8; 3],
    pub emission: u8,
    /// MarkovJunior cell value (for MjWriteTarget::get reverse lookup).
    /// Only used when voxel was written via MjWriteTarget.
    pub mj_value: u8,
}
```

### Extended RenderPalette

Add emission mapping:

```rust
/// MarkovJunior palette with color AND emission mapping.
pub struct RenderPalette {
    /// Character → RGBA color
    colors: HashMap<char, [u8; 4]>,
    /// Character → emission level (0-255). Missing = 0.
    emission: HashMap<char, u8>,
}

impl RenderPalette {
    /// Convert MJ character to Voxel with color and emission.
    pub fn to_voxel(&self, ch: char, mj_value: u8) -> Voxel {
        let rgba = self.colors.get(&ch).copied().unwrap_or([128, 128, 128, 255]);
        let emission = self.emission.get(&ch).copied().unwrap_or(0);
        Voxel {
            color: [rgba[0], rgba[1], rgba[2]],
            emission,
            mj_value,
        }
    }
    
    /// Set emission for a character.
    pub fn with_emission(mut self, ch: char, emission: u8) -> Self {
        self.emission.insert(ch, emission);
        self
    }
    
    /// Default palette with warm colors emissive.
    pub fn with_default_emission(self) -> Self {
        self.with_emission('Y', 200)  // Yellow glows
            .with_emission('O', 180)  // Orange glows
            .with_emission('W', 100)  // White slight glow
            .with_emission('R', 150)  // Red glows
    }
}
```

---

## Core Design Principles

### 1. Layers Have World Offsets
When MJ writes to layer position (5, 0, 3), and layer offset is (100, 10, 50), the world position is (105, 10, 53). This allows placing generated content anywhere without modifying MJ code.

### 2. Priority-Based Compositing
Higher priority layers override lower when merging for render/collision:
- terrain (priority 0) - base world
- generated (priority 10) - MJ output, overrides terrain
- player_placed (priority 20) - future: player modifications

### 3. Automatic Dirty Tracking
`layer.set_voxel()` automatically marks the chunk dirty. No manual tracking needed. The `update_dirty_chunks` system rebuilds only what changed.

### 4. MJ Writes Directly to VoxelWorld
No intermediate copy. MJ ExecutionContext uses `VoxelLayerTarget` which writes directly to the layer. Each `set()` call triggers dirty tracking.

### 5. Verification Drives Phases
Each phase ends with running p30 and observing specific, measurable behavior.

---

## Phase 1: VoxelLayer Struct with Dirty Tracking

**Outcome:** `VoxelLayer` struct exists with offset and dirty tracking. Unit tests prove coordinate transforms and dirty tracking work correctly.

### Tasks

1. Create `crates/studio_core/src/voxel_layer.rs`:
   - Define `VoxelLayer` struct (see Core Data Structures above)
   - Implement `new()`, `set_voxel()`, `clear_voxel()`
   - Implement `local_to_world()`, `world_to_local()`
   - Implement `take_dirty_chunks()`, `has_dirty_chunks()`
   - Implement `mark_neighbors_if_boundary()` helper

2. Add unit tests in `voxel_layer.rs`:
   ```rust
   #[test]
   fn test_layer_coordinate_transform() {
       let mut layer = VoxelLayer::new("test", 0);
       layer.offset = IVec3::new(100, 50, 200);
       
       // Local (0,0,0) → World (100,50,200)
       assert_eq!(layer.local_to_world(IVec3::ZERO), IVec3::new(100, 50, 200));
       
       // Local (5,3,7) → World (105,53,207)
       assert_eq!(layer.local_to_world(IVec3::new(5, 3, 7)), IVec3::new(105, 53, 207));
       
       // World (100,50,200) → Local (0,0,0)
       assert_eq!(layer.world_to_local(IVec3::new(100, 50, 200)), IVec3::ZERO);
   }
   
   #[test]
   fn test_set_voxel_marks_dirty() {
       let mut layer = VoxelLayer::new("test", 0);
       assert!(!layer.has_dirty_chunks());
       
       layer.set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
       assert!(layer.has_dirty_chunks());
       
       let dirty = layer.take_dirty_chunks();
       assert_eq!(dirty.len(), 1);
       assert!(dirty.contains(&ChunkPos::from_world(5, 5, 5)));
       
       // After take, should be empty
       assert!(!layer.has_dirty_chunks());
   }
   
   #[test]
   fn test_boundary_marks_neighbor_dirty() {
       let mut layer = VoxelLayer::new("test", 0);
       
       // Voxel at x=31 (chunk boundary, CHUNK_SIZE=32)
       layer.set_voxel(31, 16, 16, Voxel::solid(255, 0, 0));
       
       let dirty = layer.take_dirty_chunks();
       // Should mark both chunk (0,0,0) and neighbor chunk (1,0,0)
       assert!(dirty.contains(&ChunkPos::new(0, 0, 0)));
       assert!(dirty.contains(&ChunkPos::new(1, 0, 0)));
   }
   ```

3. Export from `lib.rs`: `pub mod voxel_layer; pub use voxel_layer::*;`

### Verification

```bash
cargo test -p studio_core voxel_layer
```

**Expected output:**
```
running 3 tests
test voxel_layer::test_layer_coordinate_transform ... ok
test voxel_layer::test_set_voxel_marks_dirty ... ok
test voxel_layer::test_boundary_marks_neighbor_dirty ... ok

test result: ok. 3 passed; 0 failed
```

**Phase is COMPLETE when:** All 3 tests pass. This proves:
- Coordinate transforms work correctly
- Dirty tracking triggers on set_voxel
- Boundary voxels mark neighbor chunks dirty

---

## Phase 2: VoxelLayers Resource + Merged Chunk Building

**Outcome:** `VoxelLayers` resource holds multiple layers. `build_merged_chunk()` composites voxels from all layers. Unit tests prove priority-based merging works.

### Tasks

1. Add `VoxelLayers` to `voxel_layer.rs`:
   - Define struct (see Core Data Structures)
   - Implement `new()` creating terrain + generated layers
   - Implement `get_mut(name)` for layer access
   - Implement `get_voxel(x,y,z)` with priority lookup
   - Implement `is_solid(x,y,z)` for collision
   - Implement `collect_dirty_chunks()`

2. Add `build_merged_chunk()` to `voxel_mesh.rs`:
   ```rust
   /// Build a single VoxelChunk by merging all visible layers at this position.
   /// Higher priority layers override lower ones.
   pub fn build_merged_chunk(
       layers: &VoxelLayers,
       chunk_pos: ChunkPos,  // World-space chunk position
   ) -> Option<VoxelChunk> {
       let mut merged = VoxelChunk::new();
       let mut has_voxels = false;
       
       // Iterate layers lowest priority first (so higher overwrites)
       for layer in layers.iter_by_priority() {
           if !layer.visible { continue; }
           
           // Convert world chunk pos to layer-local chunk pos
           let world_origin = chunk_pos.world_origin();
           let local_origin = layer.world_to_local(world_origin);
           let local_chunk_pos = ChunkPos::from_world(
               local_origin.x, local_origin.y, local_origin.z
           );
           
           if let Some(chunk) = layer.world.get_chunk(&local_chunk_pos) {
               for x in 0..CHUNK_SIZE {
                   for y in 0..CHUNK_SIZE {
                       for z in 0..CHUNK_SIZE {
                           if let Some(voxel) = chunk.get(x, y, z) {
                               merged.set(x, y, z, voxel);
                               has_voxels = true;
                           }
                       }
                   }
               }
           }
       }
       
       if has_voxels { Some(merged) } else { None }
   }
   ```

3. Add unit tests:
   ```rust
   #[test]
   fn test_merged_chunk_priority() {
       let mut layers = VoxelLayers::new();
       
       // Terrain layer (priority 0): red voxel at (5,5,5)
       layers.get_mut("terrain").unwrap()
           .set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
       
       // Generated layer (priority 10): blue voxel at same position
       layers.get_mut("generated").unwrap()
           .set_voxel(5, 5, 5, Voxel::solid(0, 0, 255));
       
       // Merged result should be blue (higher priority wins)
       let voxel = layers.get_voxel(5, 5, 5).unwrap();
       assert_eq!(voxel.color, [0, 0, 255]);
   }
   
   #[test]
   fn test_merged_chunk_offset() {
       let mut layers = VoxelLayers::new();
       
       // Generated layer offset at (100, 0, 0)
       let gen = layers.get_mut("generated").unwrap();
       gen.offset = IVec3::new(100, 0, 0);
       gen.set_voxel(5, 5, 5, Voxel::solid(0, 255, 0));  // Local coords
       
       // Should appear at world (105, 5, 5)
       assert!(layers.get_voxel(105, 5, 5).is_some());
       assert!(layers.get_voxel(5, 5, 5).is_none());  // Not at local coords
   }
   
   #[test]
   fn test_build_merged_chunk_combines_layers() {
       let mut layers = VoxelLayers::new();
       
       // Terrain: voxel at (0,0,0)
       layers.get_mut("terrain").unwrap()
           .set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
       
       // Generated: voxel at (1,0,0) 
       layers.get_mut("generated").unwrap()
           .set_voxel(1, 0, 0, Voxel::solid(0, 255, 0));
       
       let merged = build_merged_chunk(&layers, ChunkPos::new(0, 0, 0)).unwrap();
       
       // Both voxels should be present
       assert!(merged.get(0, 0, 0).is_some());
       assert!(merged.get(1, 0, 0).is_some());
   }
   ```

### Verification

```bash
cargo test -p studio_core merged_chunk
cargo test -p studio_core voxel_layers
```

**Expected output:**
```
running 3 tests
test voxel_layer::test_merged_chunk_priority ... ok
test voxel_layer::test_merged_chunk_offset ... ok
test voxel_mesh::test_build_merged_chunk_combines_layers ... ok

test result: ok. 3 passed; 0 failed
```

**Phase is COMPLETE when:** All tests pass, proving:
- Higher priority layers override lower
- Layer offsets correctly transform coordinates
- Merged chunk contains voxels from all layers

---

## Phase 3: Emissive Palette Mapping

**Outcome:** RenderPalette produces Voxels with emission values. Emissive MJ colors will glow when rendered.

### Tasks

1. Extend `RenderPalette` in `crates/studio_core/src/markov_junior/render.rs`:
   ```rust
   pub struct RenderPalette {
       /// Character to RGBA color mapping
       colors: HashMap<char, [u8; 4]>,
       /// Character to emission level (0-255). Missing = 0.
       emission: HashMap<char, u8>,
   }
   
   impl RenderPalette {
       /// Convert MJ character to Voxel with color and emission.
       /// This is the main method for MJ→VoxelWorld integration.
       pub fn to_voxel(&self, ch: char) -> Voxel {
           let rgba = self.colors.get(&ch).copied().unwrap_or([128, 128, 128, 255]);
           let emission = self.emission.get(&ch).copied().unwrap_or(0);
           Voxel::new(rgba[0], rgba[1], rgba[2], emission)
       }
       
       /// Set emission for a character. Builder pattern.
       pub fn with_emission(mut self, ch: char, emission: u8) -> Self {
           self.emission.insert(ch, emission);
           self
       }
       
       /// Apply default emission for warm colors (Y, O, R, W).
       /// Call after from_palette_xml() for standard glowing behavior.
       pub fn with_default_emission(mut self) -> Self {
           self.emission.insert('Y', 200);  // Yellow glows bright
           self.emission.insert('O', 180);  // Orange glows
           self.emission.insert('R', 150);  // Red glows
           self.emission.insert('W', 80);   // White slight glow
           self
       }
   }
   ```

2. Update `from_palette_xml()` to initialize empty emission HashMap:
   ```rust
   pub fn from_palette_xml() -> Self {
       let mut colors = HashMap::new();
       // ... existing color setup ...
       Self { 
           colors,
           emission: HashMap::new(),  // Add this line
       }
   }
   ```

3. Add unit tests in `render.rs`:
   ```rust
   #[test]
   fn test_palette_to_voxel_basic() {
       let palette = RenderPalette::from_palette_xml();
       
       // Y (Yellow) without emission set → emission 0
       let voxel = palette.to_voxel('Y');
       assert_eq!(voxel.color, [0xFF, 0xEC, 0x27]);  // Yellow from palette.xml
       assert_eq!(voxel.emission, 0);  // No emission by default
   }
   
   #[test]
   fn test_palette_with_emission() {
       let palette = RenderPalette::from_palette_xml()
           .with_emission('Y', 200)
           .with_emission('O', 180);
       
       let y_voxel = palette.to_voxel('Y');
       assert_eq!(y_voxel.emission, 200);
       
       let o_voxel = palette.to_voxel('O');
       assert_eq!(o_voxel.emission, 180);
       
       // B (Black) not set → emission 0
       let b_voxel = palette.to_voxel('B');
       assert_eq!(b_voxel.emission, 0);
   }
   
   #[test]
   fn test_palette_default_emission() {
       let palette = RenderPalette::from_palette_xml().with_default_emission();
       
       assert_eq!(palette.to_voxel('Y').emission, 200);
       assert_eq!(palette.to_voxel('O').emission, 180);
       assert_eq!(palette.to_voxel('R').emission, 150);
       assert_eq!(palette.to_voxel('W').emission, 80);
       assert_eq!(palette.to_voxel('B').emission, 0);  // Not a warm color
       assert_eq!(palette.to_voxel('G').emission, 0);  // Not a warm color
   }
   
   #[test]
   fn test_palette_unknown_char_fallback() {
       let palette = RenderPalette::from_palette_xml();
       
       // Unknown character → gray fallback, no emission
       let voxel = palette.to_voxel('?');
       assert_eq!(voxel.color, [128, 128, 128]);
       assert_eq!(voxel.emission, 0);
   }
   ```

4. Add `Voxel` import at top of render.rs:
   ```rust
   use crate::voxel::Voxel;
   ```

### Verification

```bash
cargo test -p studio_core palette_to_voxel
cargo test -p studio_core palette_with_emission
cargo test -p studio_core palette_default_emission
```

**Expected output:**
```
running 4 tests
test markov_junior::render::tests::test_palette_to_voxel_basic ... ok
test markov_junior::render::tests::test_palette_with_emission ... ok
test markov_junior::render::tests::test_palette_default_emission ... ok
test markov_junior::render::tests::test_palette_unknown_char_fallback ... ok

test result: ok. 4 passed; 0 failed
```

**Phase is COMPLETE when:** All 4 tests pass. This proves:
- RenderPalette can convert MJ characters to Voxels
- Emission can be set per-character
- Default emission applies warm colors correctly
- Unknown characters have sensible fallback

---

## Phase 4: Incremental Dirty Chunk Update

**Outcome:** Modifying a layer marks chunks dirty. Dirty chunks rebuild meshes next frame. This is the core of live voxel updates.

### Tasks

1. Add `ChunkEntityMap` resource to `crates/studio_core/src/voxel_layer.rs`:
   ```rust
   use bevy::prelude::*;
   use std::collections::{HashMap, HashSet};
   
   /// Maps world chunk positions to their mesh entities.
   /// Used by update_dirty_chunks to know which entities to rebuild.
   #[derive(Resource, Default)]
   pub struct ChunkEntityMap {
       /// World chunk position → mesh entity
       chunks: HashMap<ChunkPos, Entity>,
   }
   
   impl ChunkEntityMap {
       /// Register a chunk entity.
       pub fn register(&mut self, pos: ChunkPos, entity: Entity) {
           self.chunks.insert(pos, entity);
       }
       
       /// Get entity for chunk position.
       pub fn get(&self, pos: &ChunkPos) -> Option<Entity> {
           self.chunks.get(pos).copied()
       }
       
       /// Remove and return entity for chunk position.
       pub fn remove(&mut self, pos: &ChunkPos) -> Option<Entity> {
           self.chunks.remove(pos)
       }
       
       /// Iterate all registered chunks.
       pub fn iter(&self) -> impl Iterator<Item = (&ChunkPos, &Entity)> {
           self.chunks.iter()
       }
   }
   ```

2. Add `update_dirty_chunks` system to `crates/studio_core/src/voxel_world_plugin.rs`:
   ```rust
   use crate::voxel_layer::{VoxelLayers, ChunkEntityMap};
   use crate::voxel_mesh::{build_merged_chunk, build_chunk_mesh};
   
   /// System that rebuilds mesh for any dirty chunks.
   /// Runs every frame, only does work if chunks are dirty.
   pub fn update_dirty_chunks(
       mut layers: ResMut<VoxelLayers>,
       chunk_map: Res<ChunkEntityMap>,
       mut meshes: ResMut<Assets<Mesh>>,
       mesh_query: Query<&Handle<Mesh>>,
   ) {
       let dirty = layers.collect_dirty_chunks();
       if dirty.is_empty() {
           return;
       }
       
       info!("Rebuilding {} dirty chunks", dirty.len());
       
       for chunk_pos in dirty {
           // Get existing entity for this chunk
           let Some(entity) = chunk_map.get(&chunk_pos) else {
               // No entity yet - will be created by initial spawn
               continue;
           };
           
           // Get mesh handle from entity
           let Ok(mesh_handle) = mesh_query.get(entity) else {
               warn!("Chunk entity {:?} missing mesh handle", entity);
               continue;
           };
           
           // Build merged chunk from all layers
           if let Some(merged_chunk) = build_merged_chunk(&layers, chunk_pos) {
               // Rebuild mesh in place
               if let Some(mesh) = meshes.get_mut(mesh_handle) {
                   *mesh = build_chunk_mesh(&merged_chunk, chunk_pos);
               }
           } else {
               // Chunk is now empty - clear the mesh
               if let Some(mesh) = meshes.get_mut(mesh_handle) {
                   *mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList, 
                                    bevy::render::render_asset::RenderAssetUsages::default());
               }
           }
       }
   }
   ```

3. Register system in plugin:
   ```rust
   // In VoxelWorldPlugin::build()
   app.init_resource::<ChunkEntityMap>()
      .add_systems(Update, update_dirty_chunks);
   ```

4. Modify initial chunk spawn to register with ChunkEntityMap:
   ```rust
   // When spawning chunk mesh entities:
   let entity = commands.spawn(/* mesh bundle */).id();
   chunk_map.register(chunk_pos, entity);
   ```

5. Add integration test in `voxel_layer.rs`:
   ```rust
   #[test]
   fn test_dirty_tracking_flow() {
       let mut layers = VoxelLayers::new();
       
       // Initially no dirty chunks
       assert!(layers.collect_dirty_chunks().is_empty());
       
       // Set voxel in terrain layer
       layers.get_mut("terrain").unwrap()
           .set_voxel(5, 5, 5, Voxel::solid(255, 0, 0));
       
       // Now have dirty chunks
       let dirty = layers.collect_dirty_chunks();
       assert_eq!(dirty.len(), 1);
       assert!(dirty.contains(&ChunkPos::from_world(5, 5, 5)));
       
       // After collect, dirty is cleared
       assert!(layers.collect_dirty_chunks().is_empty());
       
       // Set voxel in generated layer with offset
       let gen = layers.get_mut("generated").unwrap();
       gen.offset = IVec3::new(100, 0, 0);
       gen.set_voxel(5, 5, 5, Voxel::solid(0, 255, 0));  // Local (5,5,5)
       
       // Dirty chunk should be at WORLD position (105, 5, 5)
       let dirty = layers.collect_dirty_chunks();
       assert_eq!(dirty.len(), 1);
       // World pos 105 / 32 = chunk 3
       assert!(dirty.contains(&ChunkPos::from_world(105, 5, 5)));
   }
   ```

### Verification

**Unit tests:**
```bash
cargo test -p studio_core dirty_tracking_flow
```

**Expected output:**
```
running 1 test
test voxel_layer::tests::test_dirty_tracking_flow ... ok

test result: ok. 1 passed; 0 failed
```

**Manual test in p30:**
Add temporary code to p30 that sets a voxel after 3 seconds:
```rust
// In p30 update system:
fn test_dirty_chunks(
    time: Res<Time>,
    mut layers: ResMut<VoxelLayers>,
    mut tested: Local<bool>,
) {
    if *tested { return; }
    if time.elapsed_secs() > 3.0 {
        *tested = true;
        // Add red voxel to generated layer
        layers.get_mut("generated").unwrap()
            .set_voxel(0, 10, 0, Voxel::solid(255, 0, 0));
        info!("Added test voxel at (0, 10, 0)");
    }
}
```

**Expected behavior:**
1. Run `cargo run --example p30_markov_kinematic_animated`
2. Wait 3 seconds
3. Console shows: "Added test voxel at (0, 10, 0)" then "Rebuilding 1 dirty chunks"
4. Red voxel APPEARS in the world without restart
5. Player can walk to the voxel and collide with it

**Phase is COMPLETE when:**
- Unit test passes
- Red voxel appears after 3 seconds in p30
- Console shows rebuild message
- Voxel has collision

---

## Phase 5: MJ Writes to VoxelLayer

**Outcome:** MarkovJunior writes directly to generated layer. Building appears in VoxelWorld with collision. No separate mesh entity.

### Tasks

1. Create `crates/studio_core/src/markov_junior/write_target.rs`:
   ```rust
   //! MjWriteTarget trait for MarkovJunior output destinations.
   
   use crate::voxel::{Voxel, VoxelWorld};
   use crate::voxel_layer::VoxelLayer;
   use super::render::RenderPalette;
   
   /// Trait for anything MarkovJunior can write to.
   /// Abstracts over MjGrid (testing) and VoxelLayer (production).
   pub trait MjWriteTarget {
       /// Set cell value at position. value=0 means empty.
       fn set(&mut self, x: i32, y: i32, z: i32, value: u8);
       
       /// Get cell value at position. Returns 0 if empty.
       fn get(&self, x: i32, y: i32, z: i32) -> u8;
       
       /// Clear all cells to empty (value 0).
       fn clear(&mut self);
       
       /// Grid dimensions (mx, my, mz).
       fn dimensions(&self) -> (usize, usize, usize);
   }
   
   /// Wrapper that makes VoxelLayer implement MjWriteTarget.
   /// Handles:
   /// - MJ character → Voxel conversion via RenderPalette
   /// - Y/Z coordinate swap (MJ uses Y as height, VoxelWorld uses Z)
   /// - Automatic dirty tracking via VoxelLayer::set_voxel
   pub struct VoxelLayerTarget<'a> {
       layer: &'a mut VoxelLayer,
       palette: &'a RenderPalette,
       /// MJ characters list for value → char lookup
       characters: Vec<char>,
       /// Grid dimensions for bounds checking
       mx: usize,
       my: usize,
       mz: usize,
   }
   
   impl<'a> VoxelLayerTarget<'a> {
       pub fn new(
           layer: &'a mut VoxelLayer,
           palette: &'a RenderPalette,
           characters: &str,
           mx: usize,
           my: usize,
           mz: usize,
       ) -> Self {
           Self {
               layer,
               palette,
               characters: characters.chars().collect(),
               mx,
               my,
               mz,
           }
       }
   }
   
   impl<'a> MjWriteTarget for VoxelLayerTarget<'a> {
       fn set(&mut self, x: i32, y: i32, z: i32, value: u8) {
           // Bounds check
           if x < 0 || y < 0 || z < 0 
               || x >= self.mx as i32 
               || y >= self.my as i32 
               || z >= self.mz as i32 {
               return;
           }
           
           // Coordinate transform: MJ Y is height, VoxelWorld Z is height
           // MJ (x, y, z) → VoxelWorld (x, z, y)
           let vx = x;
           let vy = z;  // MJ z → VoxelWorld y
           let vz = y;  // MJ y (height) → VoxelWorld z (height)
           
           if value == 0 {
               // Clear voxel
               self.layer.clear_voxel(vx, vy, vz);
           } else {
               // Convert MJ value to character, then to Voxel
               let ch = self.characters.get(value as usize)
                   .copied()
                   .unwrap_or('?');
               let voxel = self.palette.to_voxel(ch);
               self.layer.set_voxel(vx, vy, vz, voxel);
           }
       }
       
       fn get(&self, x: i32, y: i32, z: i32) -> u8 {
           // Coordinate transform
           let vx = x;
           let vy = z;
           let vz = y;
           
           // Get voxel and reverse-lookup MJ value
           // For simplicity, return 0 if no voxel (matches MJ "empty" convention)
           if self.layer.world.get_voxel(vx, vy, vz).is_some() {
               // Non-empty, but we don't track MJ value in Voxel
               // This is only used for rule matching, which needs exact value
               // TODO: Add mj_value to Voxel if needed for WFC/constraint nodes
               1  // Non-zero placeholder
           } else {
               0
           }
       }
       
       fn clear(&mut self) {
           // Clear the region that MJ is writing to
           self.layer.clear_region(
               bevy::prelude::IVec3::ZERO,
               bevy::prelude::IVec3::new(
                   self.mx as i32 - 1,
                   self.mz as i32 - 1,  // Swapped
                   self.my as i32 - 1,  // Swapped
               ),
           );
       }
       
       fn dimensions(&self) -> (usize, usize, usize) {
           (self.mx, self.my, self.mz)
       }
   }
   ```

2. Export from `markov_junior/mod.rs`:
   ```rust
   pub mod write_target;
   pub use write_target::{MjWriteTarget, VoxelLayerTarget};
   ```

3. Update p30 to use VoxelLayerTarget instead of separate mesh:

   **Before (current broken approach):**
   ```rust
   // Runs MJ, copies to separate mesh entity with no collision
   fn update_mj_building(
       mut mj: ResMut<MarkovState>,
       mut building_mesh: Query<&mut Handle<Mesh>, With<GeneratedBuilding>>,
   ) {
       // ... copies MjGrid to separate mesh ...
   }
   ```

   **After (new integrated approach):**
   ```rust
   // Runs MJ, writes directly to VoxelLayers
   fn update_mj_building(
       mut mj: ResMut<MarkovState>,
       mut layers: ResMut<VoxelLayers>,
       palette: Res<MjPalette>,
   ) {
       if !mj.running { return; }
       
       let model = &mut mj.model;
       let grid = model.grid();
       
       // Create write target for generated layer
       let gen_layer = layers.get_mut("generated").unwrap();
       let mut target = VoxelLayerTarget::new(
           gen_layer,
           &palette.0,
           &grid.values,  // Character list like "BWA"
           grid.mx, grid.my, grid.mz,
       );
       
       // Run MJ step - this writes directly to VoxelLayer
       // MJ internally calls target.set() which marks chunks dirty
       if model.step_with_target(&mut target) {
           // Still running
       } else {
           mj.running = false;
           info!("MJ generation complete");
       }
   }
   ```

4. Add `step_with_target` to Model/Interpreter (if not already):
   ```rust
   impl Interpreter {
       /// Run one step, writing to the provided target.
       pub fn step_with_target<T: MjWriteTarget>(&mut self, target: &mut T) -> bool {
           // ... existing step logic ...
           // When setting grid values, use target.set() instead of self.grid.set()
       }
   }
   ```

5. Remove GeneratedBuilding entity and mesh from p30:
   - Delete `GeneratedBuilding` component
   - Delete `spawn_building_mesh()` 
   - Delete `build_building_mesh()`
   - Delete the mesh copy loop

6. Add unit test:
   ```rust
   #[test]
   fn test_voxel_layer_target_coordinate_transform() {
       let mut layer = VoxelLayer::new("test", 0);
       let palette = RenderPalette::from_palette_xml().with_default_emission();
       
       {
           let mut target = VoxelLayerTarget::new(
               &mut layer,
               &palette,
               "BW",  // B=0 (empty), W=1 (white)
               8, 8, 8,
           );
           
           // Set voxel at MJ coords (2, 5, 3) where y=5 is height
           target.set(2, 5, 3, 1);  // W=1
       }
       
       // Should appear at VoxelWorld coords (2, 3, 5) where z=5 is height
       let voxel = layer.world.get_voxel(2, 3, 5);
       assert!(voxel.is_some(), "Voxel should exist after target.set()");
       
       // Should NOT be at MJ coords
       assert!(layer.world.get_voxel(2, 5, 3).is_none());
   }
   
   #[test]
   fn test_voxel_layer_target_marks_dirty() {
       let mut layer = VoxelLayer::new("test", 0);
       let palette = RenderPalette::from_palette_xml();
       
       assert!(!layer.has_dirty_chunks());
       
       {
           let mut target = VoxelLayerTarget::new(
               &mut layer, &palette, "BW", 8, 8, 8,
           );
           target.set(5, 5, 5, 1);
       }
       
       assert!(layer.has_dirty_chunks());
   }
   ```

### Verification

**Unit tests:**
```bash
cargo test -p studio_core voxel_layer_target
```

**Expected output:**
```
running 2 tests
test markov_junior::write_target::tests::test_voxel_layer_target_coordinate_transform ... ok
test markov_junior::write_target::tests::test_voxel_layer_target_marks_dirty ... ok

test result: ok. 2 passed; 0 failed
```

**Manual test in p30:**
```bash
cargo run --example p30_markov_kinematic_animated
```

**Expected behavior:**
1. Press G → Building generates (voxels appear incrementally)
2. Walk into building → **Player STOPS** (collision works!)
3. In Bevy inspector: NO "GeneratedBuilding" entity exists
4. Yellow/Orange voxels glow (emission from palette)
5. Building is part of the same mesh as terrain

**Phase is COMPLETE when:**
- Unit tests pass
- Building has collision (player bounces off walls)
- No separate GeneratedBuilding mesh entity
- Emissive voxels glow

---

## Phase 6: Layer Offset for Placement

**Outcome:** Can place generated building at any world position via layer offset. Building sits ON terrain, not through it.

### Tasks

1. In p30 setup, set generated layer offset to place building on platform:
   ```rust
   fn setup_voxel_world(mut layers: ResMut<VoxelLayers>) {
       // Terrain layer at origin (default)
       // ... spawn terrain platform ...
       
       // Generated layer offset: place building ON the platform
       // Platform is at y=0, so building starts at y=1 (one voxel above)
       let gen = layers.get_mut("generated").unwrap();
       gen.offset = IVec3::new(5, 1, 5);  // Offset from terrain
   }
   ```

2. Add keyboard controls to move building offset:
   ```rust
   /// Resource to track current building placement
   #[derive(Resource)]
   struct BuildingPlacement {
       offset: IVec3,
       positions: Vec<IVec3>,  // Preset positions
       current_index: usize,
   }
   
   impl Default for BuildingPlacement {
       fn default() -> Self {
           Self {
               offset: IVec3::new(5, 1, 5),
               positions: vec![
                   IVec3::new(5, 1, 5),    // Position 1
                   IVec3::new(20, 1, 5),   // Position 2
                   IVec3::new(5, 1, 20),   // Position 3
                   IVec3::new(20, 1, 20),  // Position 4
               ],
               current_index: 0,
           }
       }
   }
   
   fn handle_placement_keys(
       keyboard: Res<ButtonInput<KeyCode>>,
       mut placement: ResMut<BuildingPlacement>,
       mut layers: ResMut<VoxelLayers>,
       mut mj: ResMut<MarkovState>,
   ) {
       let new_index = if keyboard.just_pressed(KeyCode::Digit1) {
           Some(0)
       } else if keyboard.just_pressed(KeyCode::Digit2) {
           Some(1)
       } else if keyboard.just_pressed(KeyCode::Digit3) {
           Some(2)
       } else if keyboard.just_pressed(KeyCode::Digit4) {
           Some(3)
       } else {
           None
       };
       
       if let Some(idx) = new_index {
           if idx < placement.positions.len() && idx != placement.current_index {
               placement.current_index = idx;
               let new_offset = placement.positions[idx];
               
               // Clear old building location
               let gen = layers.get_mut("generated").unwrap();
               gen.clear_region(IVec3::ZERO, IVec3::new(15, 15, 15));
               
               // Update offset
               gen.offset = new_offset;
               placement.offset = new_offset;
               
               // Reset MJ to regenerate
               mj.running = false;
               info!("Building position changed to {:?}", new_offset);
           }
       }
   }
   ```

3. Add on-screen help text:
   ```rust
   fn spawn_help_text(mut commands: Commands) {
       commands.spawn(TextBundle::from_section(
           "G: Generate building | 1-4: Move building position",
           TextStyle { font_size: 18.0, color: Color::WHITE, ..default() },
       ).with_style(Style {
           position_type: PositionType::Absolute,
           bottom: Val::Px(10.0),
           left: Val::Px(10.0),
           ..default()
       }));
   }
   ```

4. Add unit test for offset behavior:
   ```rust
   #[test]
   fn test_layer_offset_affects_world_position() {
       let mut layers = VoxelLayers::new();
       
       // Set offset on generated layer
       let gen = layers.get_mut("generated").unwrap();
       gen.offset = IVec3::new(100, 50, 200);
       gen.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));  // Local origin
       
       // Voxel should appear at world position (100, 50, 200)
       assert!(layers.get_voxel(100, 50, 200).is_some());
       
       // Voxel should NOT appear at local coordinates in world space
       assert!(layers.get_voxel(0, 0, 0).is_none());
       
       // is_solid should also respect offset
       assert!(layers.is_solid(100, 50, 200));
       assert!(!layers.is_solid(0, 0, 0));
   }
   
   #[test]
   fn test_changing_offset_and_regenerating() {
       let mut layers = VoxelLayers::new();
       
       // Place building at first position
       let gen = layers.get_mut("generated").unwrap();
       gen.offset = IVec3::new(10, 0, 10);
       gen.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
       
       assert!(layers.get_voxel(10, 0, 10).is_some());
       
       // Clear and move to new position
       let gen = layers.get_mut("generated").unwrap();
       gen.clear_region(IVec3::ZERO, IVec3::new(10, 10, 10));
       gen.offset = IVec3::new(50, 0, 50);
       gen.set_voxel(0, 0, 0, Voxel::solid(0, 255, 0));
       
       // Old position should be empty
       assert!(layers.get_voxel(10, 0, 10).is_none());
       // New position should have voxel
       assert!(layers.get_voxel(50, 0, 50).is_some());
   }
   ```

### Verification

**Unit tests:**
```bash
cargo test -p studio_core layer_offset_affects
cargo test -p studio_core changing_offset
```

**Expected output:**
```
running 2 tests
test voxel_layer::tests::test_layer_offset_affects_world_position ... ok
test voxel_layer::tests::test_changing_offset_and_regenerating ... ok

test result: ok. 2 passed; 0 failed
```

**Manual test in p30:**
```bash
cargo run --example p30_markov_kinematic_animated
```

**Expected behavior:**
1. Press G → Building generates at position 1 (on platform)
2. Building sits ON the platform, not through it (offset y=1 works)
3. Press 2 → Building clears, regenerates at position 2
4. Press 3 → Building clears, regenerates at position 3
5. Press 4 → Building clears, regenerates at position 4
6. Walk to each position → Collision works at ALL positions
7. Console shows "Building position changed to [x, y, z]"

**Phase is COMPLETE when:**
- Unit tests pass
- Building sits on platform (not intersecting)
- Keys 1-4 move building to different positions
- Collision works at all positions
- Old building clears when moving to new position

---

## Phase 7: Animation Polish

**Outcome:** Generation animates smoothly with good performance. Clear visual feedback during generation.

### Tasks

1. Add configurable steps-per-frame to MarkovState:
   ```rust
   #[derive(Resource)]
   pub struct MarkovState {
       pub model: Model,
       pub running: bool,
       /// MJ steps to run per frame. Higher = faster, less smooth.
       pub steps_per_frame: usize,
       /// Max chunk rebuilds per frame (frame budget).
       pub max_rebuilds_per_frame: usize,
   }
   
   impl Default for MarkovState {
       fn default() -> Self {
           Self {
               model: Model::empty(),
               running: false,
               steps_per_frame: 50,       // Tune for visual smoothness
               max_rebuilds_per_frame: 4, // Prevent frame drops
           }
       }
   }
   ```

2. Update MJ update system to respect steps_per_frame:
   ```rust
   fn update_mj_building(
       mut mj: ResMut<MarkovState>,
       mut layers: ResMut<VoxelLayers>,
       palette: Res<MjPalette>,
   ) {
       if !mj.running { return; }
       
       let gen_layer = layers.get_mut("generated").unwrap();
       let grid = mj.model.grid();
       
       let mut target = VoxelLayerTarget::new(
           gen_layer,
           &palette.0,
           &grid.values,
           grid.mx, grid.my, grid.mz,
       );
       
       // Run multiple steps per frame for speed
       for _ in 0..mj.steps_per_frame {
           if !mj.model.step_with_target(&mut target) {
               mj.running = false;
               info!("MJ generation complete");
               break;
           }
       }
   }
   ```

3. Add frame budget to dirty chunk system:
   ```rust
   pub fn update_dirty_chunks(
       mut layers: ResMut<VoxelLayers>,
       mj: Res<MarkovState>,
       chunk_map: Res<ChunkEntityMap>,
       mut meshes: ResMut<Assets<Mesh>>,
       mesh_query: Query<&Handle<Mesh>>,
   ) {
       let dirty: Vec<_> = layers.collect_dirty_chunks().into_iter().collect();
       if dirty.is_empty() { return; }
       
       // Apply frame budget
       let to_rebuild = dirty.len().min(mj.max_rebuilds_per_frame);
       
       if dirty.len() > to_rebuild {
           // Re-mark chunks we couldn't rebuild this frame
           for chunk_pos in dirty.iter().skip(to_rebuild) {
               // These will be picked up next frame
               // Note: This requires adding a method to re-mark dirty
           }
           debug!("Frame budget: rebuilt {}/{} chunks", to_rebuild, dirty.len());
       }
       
       for chunk_pos in dirty.into_iter().take(to_rebuild) {
           // ... rebuild logic ...
       }
   }
   ```

4. Clear previous generation before starting new:
   ```rust
   fn start_generation(
       keyboard: Res<ButtonInput<KeyCode>>,
       mut mj: ResMut<MarkovState>,
       mut layers: ResMut<VoxelLayers>,
   ) {
       if keyboard.just_pressed(KeyCode::KeyG) {
           // Clear previous building
           let gen = layers.get_mut("generated").unwrap();
           gen.clear_region(IVec3::ZERO, IVec3::new(31, 31, 31));
           
           // Reset and start MJ
           mj.model.reset(rand::random());
           mj.running = true;
           info!("Starting new generation");
       }
   }
   ```

5. Add keyboard to adjust animation speed:
   ```rust
   fn adjust_animation_speed(
       keyboard: Res<ButtonInput<KeyCode>>,
       mut mj: ResMut<MarkovState>,
   ) {
       if keyboard.just_pressed(KeyCode::BracketLeft) {
           mj.steps_per_frame = (mj.steps_per_frame / 2).max(1);
           info!("Steps per frame: {}", mj.steps_per_frame);
       }
       if keyboard.just_pressed(KeyCode::BracketRight) {
           mj.steps_per_frame = (mj.steps_per_frame * 2).min(500);
           info!("Steps per frame: {}", mj.steps_per_frame);
       }
   }
   ```

6. Update help text:
   ```rust
   "G: Generate | 1-4: Position | [ ]: Speed | ESC: Quit"
   ```

7. Add FPS counter for verification:
   ```rust
   fn show_fps(
       diagnostics: Res<DiagnosticsStore>,
       mut fps_text: Query<&mut Text, With<FpsText>>,
   ) {
       if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
           if let Some(value) = fps.smoothed() {
               for mut text in &mut fps_text {
                   text.sections[0].value = format!("FPS: {:.0}", value);
               }
           }
       }
   }
   ```

### Verification

**Manual test in p30:**
```bash
cargo run --example p30_markov_kinematic_animated
```

**Expected behavior:**
1. Press G → Building animates visibly (voxels appear incrementally, not all at once)
2. FPS counter shows > 30 FPS during generation
3. Press `[` → Animation slows down (fewer steps per frame)
4. Press `]` → Animation speeds up (more steps per frame)
5. Press G again → Old building clears, new one generates
6. No flickering or visual glitches during animation
7. Building is recognizable mid-generation (not just noise)

**Performance targets:**
- FPS > 30 during active generation
- FPS > 55 when idle (not generating)
- No frame drops below 20 FPS

**Screenshot test:**
```bash
cargo run --example p30_markov_kinematic_animated -- --screenshot
```
Take screenshot 2 seconds after pressing G. Building should be visibly partial (mid-generation), not complete or empty.

**Phase is COMPLETE when:**
- Generation is visibly animated (not instant)
- FPS stays above 30 during generation
- Speed controls work (`[` and `]`)
- No flickering or visual artifacts
- Pressing G clears old building before generating new

---

## File Structure

```
crates/studio_core/src/
├── voxel_layer.rs           # NEW: VoxelLayer, VoxelLayers, ChunkEntityMap
├── voxel.rs                 # Unchanged (Voxel already has emission)
├── voxel_mesh.rs            # MODIFY: Add build_merged_chunk()
├── voxel_world_plugin.rs    # MODIFY: Add update_dirty_chunks system, register resources
├── lib.rs                   # MODIFY: Add `pub mod voxel_layer;`
└── markov_junior/
    ├── mod.rs               # MODIFY: Export write_target module
    ├── render.rs            # MODIFY: Add emission HashMap, to_voxel() method
    └── write_target.rs      # NEW: MjWriteTarget trait, VoxelLayerTarget

examples/
└── p30_markov_kinematic_animated.rs  # REFACTOR: Use VoxelLayers, remove separate mesh
```

### New Files (2)
- `voxel_layer.rs` - ~200 lines: VoxelLayer, VoxelLayers, ChunkEntityMap
- `write_target.rs` - ~100 lines: MjWriteTarget trait, VoxelLayerTarget impl

### Modified Files (5)
- `voxel_mesh.rs` - Add ~50 lines: build_merged_chunk() function
- `voxel_world_plugin.rs` - Add ~40 lines: update_dirty_chunks system
- `render.rs` - Add ~30 lines: emission HashMap, to_voxel(), with_emission()
- `lib.rs` - Add 1 line: module export
- `markov_junior/mod.rs` - Add 2 lines: module export

### Files NOT Changed
- `voxel.rs` - Voxel struct already has emission field
- `interpreter.rs` - MjGrid continues to work, MjWriteTarget is additive
- Other markov_junior files - No changes needed

---

## What We're NOT Doing

- **No p31, p32, p33 examples** - p30 is the only example
- **No MjGrid changes** - MjGrid still exists for tests/verification
- **No complex trait hierarchy** - Just MjWriteTarget with 2 implementors
- **No collision refactor** - TerrainOccupancy rebuilt when layers change (simple)

---

## Success Criteria (All Must Pass)

Run `cargo run --example p30_markov_kinematic_animated`:

1. [ ] Building generates when pressing G
2. [ ] Building has collision (player cannot walk through)
3. [ ] Emissive voxels glow (Y/O colors emit light)
4. [ ] Building position controlled by layer offset
5. [ ] Generation animates (see progress, not just final)
6. [ ] No separate mesh entity (building IS the voxel world)
7. [ ] Can regenerate without restart
8. [ ] FPS > 30 during generation

---

## Naming Conventions

| Name | Purpose |
|------|---------|
| `VoxelLayer` | Single layer with offset + dirty tracking |
| `VoxelLayers` | Resource holding all layers |
| `MjWriteTarget` | Trait for MJ write destinations |
| `VoxelLayerTarget` | Wrapper making VoxelLayer an MjWriteTarget |
| `ChunkEntityMap` | Maps ChunkPos → Entity |
| `update_dirty_chunks` | System that rebuilds dirty meshes |

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| MjWriteTarget breaks existing MJ | MjGrid still implements trait, tests pass |
| Performance regression | Frame budget, profile during Phase 7 |
| Coordinate confusion | Clear offset documentation, unit tests |

---

## How This Plan Follows HOW_WE_WORK

1. **One motivating example** - p30 drives all work
2. **Early end-to-end** - Phase 2 already renders merged view
3. **Verification at each phase** - Run p30, observe specific behavior
4. **Complexification** - Start with rendering, add collision, add emission, add animation
5. **No silent stubs** - Each phase has observable outcome
6. **Hypothesis-driven** - If phase fails, we debug specific issue in p30
