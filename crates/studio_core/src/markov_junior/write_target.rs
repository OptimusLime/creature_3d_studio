//! MjWriteTarget trait for MarkovJunior output destinations.
//!
//! This module provides an abstraction for MarkovJunior grid output, allowing
//! MJ to write directly to VoxelLayer instead of just MjGrid.
//!
//! ## Key Types
//!
//! - `MjWriteTarget`: Trait for anything MarkovJunior can write to
//! - `VoxelLayerTarget`: Wrapper that makes VoxelLayer implement MjWriteTarget
//!
//! ## Coordinate Transform
//!
//! MJ uses Y as height (vertical axis), while VoxelWorld uses Z as height.
//! The `VoxelLayerTarget` handles this transform:
//! - MJ (x, y, z) → VoxelWorld (x, z, y)

use crate::markov_junior::render::RenderPalette;
use crate::voxel::Voxel;
use crate::voxel_layer::VoxelLayer;
use bevy::prelude::IVec3;

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
    /// Create a new VoxelLayerTarget.
    ///
    /// # Arguments
    /// * `layer` - The VoxelLayer to write to
    /// * `palette` - RenderPalette for char→Voxel conversion
    /// * `characters` - MJ character string (e.g., "BWA")
    /// * `mx`, `my`, `mz` - Grid dimensions
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

    /// Get the underlying layer (for inspection).
    pub fn layer(&self) -> &VoxelLayer {
        self.layer
    }
}

impl<'a> MjWriteTarget for VoxelLayerTarget<'a> {
    fn set(&mut self, x: i32, y: i32, z: i32, value: u8) {
        // Bounds check
        if x < 0
            || y < 0
            || z < 0
            || x >= self.mx as i32
            || y >= self.my as i32
            || z >= self.mz as i32
        {
            return;
        }

        // Coordinate transform: MJ Y is height, VoxelWorld Z is height
        // MJ (x, y, z) → VoxelWorld (x, z, y)
        let vx = x;
        let vy = z; // MJ z → VoxelWorld y
        let vz = y; // MJ y (height) → VoxelWorld z (height)

        if value == 0 {
            // Clear voxel
            self.layer.clear_voxel(vx, vy, vz);
        } else {
            // Convert MJ value to character, then to Voxel
            let ch = self.characters.get(value as usize).copied().unwrap_or('?');
            let voxel = self.palette.to_voxel(ch);
            self.layer.set_voxel(vx, vy, vz, voxel);
        }
    }

    fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        // Bounds check
        if x < 0
            || y < 0
            || z < 0
            || x >= self.mx as i32
            || y >= self.my as i32
            || z >= self.mz as i32
        {
            return 0;
        }

        // Coordinate transform
        let vx = x;
        let vy = z;
        let vz = y;

        // Get voxel and check if present
        // For simplicity, return 1 for any non-empty voxel, 0 for empty
        // This is sufficient for basic rule matching
        // TODO: Add mj_value to Voxel if needed for exact value matching in WFC
        if self.layer.get_voxel(vx, vy, vz).is_some() {
            1 // Non-zero placeholder
        } else {
            0
        }
    }

    fn clear(&mut self) {
        // Clear the region that MJ is writing to
        // Transform dimensions: MJ (mx, my, mz) → VoxelWorld (mx, mz, my)
        self.layer.clear_region(
            IVec3::ZERO,
            IVec3::new(
                self.mx as i32 - 1,
                self.mz as i32 - 1, // Swapped
                self.my as i32 - 1, // Swapped
            ),
        );
    }

    fn dimensions(&self) -> (usize, usize, usize) {
        (self.mx, self.my, self.mz)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voxel_layer_target_coordinate_transform() {
        let mut layer = VoxelLayer::new("test", 0);
        let palette = RenderPalette::from_palette_xml().with_default_emission();

        {
            let mut target = VoxelLayerTarget::new(
                &mut layer, &palette, "BW", // B=0 (empty), W=1 (white)
                8, 8, 8,
            );

            // Set voxel at MJ coords (2, 5, 3) where y=5 is height
            target.set(2, 5, 3, 1); // W=1
        }

        // Should appear at VoxelWorld coords (2, 3, 5) where z=5 is height
        // MJ (x=2, y=5, z=3) → VoxelWorld (x=2, y=3, z=5)
        let voxel = layer.get_voxel(2, 3, 5);
        assert!(voxel.is_some(), "Voxel should exist after target.set()");

        // Should NOT be at MJ coords interpreted as VoxelWorld coords
        assert!(
            layer.get_voxel(2, 5, 3).is_none(),
            "Voxel should not be at untransformed coords"
        );
    }

    #[test]
    fn test_voxel_layer_target_marks_dirty() {
        let mut layer = VoxelLayer::new("test", 0);
        let palette = RenderPalette::from_palette_xml();

        assert!(!layer.has_dirty_chunks());

        {
            let mut target = VoxelLayerTarget::new(&mut layer, &palette, "BW", 8, 8, 8);
            target.set(5, 5, 5, 1);
        }

        assert!(layer.has_dirty_chunks());
    }

    #[test]
    fn test_voxel_layer_target_clear() {
        let mut layer = VoxelLayer::new("test", 0);
        let palette = RenderPalette::from_palette_xml();

        // Set some voxels
        {
            let mut target = VoxelLayerTarget::new(&mut layer, &palette, "BW", 4, 4, 4);
            target.set(1, 1, 1, 1);
            target.set(2, 2, 2, 1);
        }

        // Verify they exist (transformed coords)
        assert!(layer.get_voxel(1, 1, 1).is_some());
        assert!(layer.get_voxel(2, 2, 2).is_some());

        // Clear
        {
            let mut target = VoxelLayerTarget::new(&mut layer, &palette, "BW", 4, 4, 4);
            target.clear();
        }

        // Should be empty now
        assert!(layer.get_voxel(1, 1, 1).is_none());
        assert!(layer.get_voxel(2, 2, 2).is_none());
    }

    #[test]
    fn test_voxel_layer_target_get() {
        let mut layer = VoxelLayer::new("test", 0);
        let palette = RenderPalette::from_palette_xml();

        // Set a voxel at MJ coords (3, 2, 1)
        {
            let mut target = VoxelLayerTarget::new(&mut layer, &palette, "BW", 8, 8, 8);
            target.set(3, 2, 1, 1);

            // Get should return non-zero at same coords
            assert_ne!(target.get(3, 2, 1), 0, "Should find voxel at set location");

            // Get should return 0 at empty location
            assert_eq!(
                target.get(0, 0, 0),
                0,
                "Should not find voxel at empty location"
            );
        }
    }

    #[test]
    fn test_voxel_layer_target_bounds() {
        let mut layer = VoxelLayer::new("test", 0);
        let palette = RenderPalette::from_palette_xml();

        {
            let mut target = VoxelLayerTarget::new(&mut layer, &palette, "BW", 4, 4, 4);

            // Out of bounds - should not panic or set
            target.set(-1, 0, 0, 1);
            target.set(0, -1, 0, 1);
            target.set(0, 0, -1, 1);
            target.set(4, 0, 0, 1);
            target.set(0, 4, 0, 1);
            target.set(0, 0, 4, 1);

            // Get out of bounds should return 0
            assert_eq!(target.get(-1, 0, 0), 0);
            assert_eq!(target.get(4, 0, 0), 0);
        }

        // Layer should still be empty
        assert!(!layer.has_dirty_chunks() || layer.world.total_voxel_count() == 0);
    }

    #[test]
    fn test_voxel_layer_target_emission() {
        let mut layer = VoxelLayer::new("test", 0);
        let palette = RenderPalette::from_palette_xml().with_default_emission();

        {
            // Use characters that have emission: Y=200 emission
            let mut target = VoxelLayerTarget::new(
                &mut layer, &palette, "BY", // B=0 (empty), Y=1 (yellow with emission)
                8, 8, 8,
            );

            target.set(0, 0, 0, 1); // Set Y (yellow)
        }

        // Should have emission from palette
        let voxel = layer.get_voxel(0, 0, 0).unwrap();
        assert_eq!(voxel.emission, 200, "Yellow should have emission 200");
    }

    #[test]
    fn test_voxel_layer_target_dimensions() {
        let mut layer = VoxelLayer::new("test", 0);
        let palette = RenderPalette::from_palette_xml();

        let target = VoxelLayerTarget::new(&mut layer, &palette, "BW", 10, 20, 30);

        assert_eq!(target.dimensions(), (10, 20, 30));
    }
}
