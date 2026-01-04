//! Bridge between MarkovJunior grids and VoxelWorld.
//!
//! Converts MjGrid output to VoxelWorld for rendering in Bevy.

use super::MjGrid;
use crate::voxel::{Voxel, VoxelWorld};
use std::collections::HashMap;

/// Palette mapping MjGrid values to voxel colors.
///
/// Value 0 is always treated as empty/transparent.
/// Other values map to specific colors.
#[derive(Debug, Clone)]
pub struct MjPalette {
    /// Mapping from grid value (1-255) to voxel
    colors: HashMap<u8, Voxel>,
}

impl Default for MjPalette {
    /// Default palette with basic colors.
    fn default() -> Self {
        let mut colors = HashMap::new();
        // Value 1 = white
        colors.insert(1, Voxel::solid(255, 255, 255));
        // Value 2 = red
        colors.insert(2, Voxel::solid(255, 0, 0));
        // Value 3 = green
        colors.insert(3, Voxel::solid(0, 255, 0));
        // Value 4 = blue
        colors.insert(4, Voxel::solid(0, 0, 255));
        // Value 5 = yellow
        colors.insert(5, Voxel::solid(255, 255, 0));
        // Value 6 = cyan
        colors.insert(6, Voxel::solid(0, 255, 255));
        // Value 7 = magenta
        colors.insert(7, Voxel::solid(255, 0, 255));
        // Value 8 = gray
        colors.insert(8, Voxel::solid(128, 128, 128));
        Self { colors }
    }
}

impl MjPalette {
    /// Create an empty palette.
    pub fn new() -> Self {
        Self {
            colors: HashMap::new(),
        }
    }

    /// Add a color mapping.
    pub fn set(&mut self, value: u8, voxel: Voxel) {
        if value > 0 {
            self.colors.insert(value, voxel);
        }
    }

    /// Get the voxel for a grid value.
    /// Returns None for value 0 (empty) or unmapped values.
    pub fn get(&self, value: u8) -> Option<Voxel> {
        if value == 0 {
            None
        } else {
            self.colors.get(&value).copied()
        }
    }

    /// Create the PICO-8 16-color palette.
    pub fn pico8() -> Self {
        let mut colors = HashMap::new();
        // PICO-8 colors (indices 1-15, 0 is transparent)
        colors.insert(1, Voxel::solid(29, 43, 83)); // dark-blue
        colors.insert(2, Voxel::solid(126, 37, 83)); // dark-purple
        colors.insert(3, Voxel::solid(0, 135, 81)); // dark-green
        colors.insert(4, Voxel::solid(171, 82, 54)); // brown
        colors.insert(5, Voxel::solid(95, 87, 79)); // dark-grey
        colors.insert(6, Voxel::solid(194, 195, 199)); // light-grey
        colors.insert(7, Voxel::solid(255, 241, 232)); // white
        colors.insert(8, Voxel::solid(255, 0, 77)); // red
        colors.insert(9, Voxel::solid(255, 163, 0)); // orange
        colors.insert(10, Voxel::solid(255, 236, 39)); // yellow
        colors.insert(11, Voxel::solid(0, 228, 54)); // green
        colors.insert(12, Voxel::solid(41, 173, 255)); // blue
        colors.insert(13, Voxel::solid(131, 118, 156)); // lavender
        colors.insert(14, Voxel::solid(255, 119, 168)); // pink
        colors.insert(15, Voxel::solid(255, 204, 170)); // light-peach
        Self { colors }
    }
}

/// Convert an MjGrid to a VoxelWorld using a palette.
///
/// Value 0 in the grid is treated as empty (no voxel).
/// Other values are looked up in the palette.
///
/// The grid is centered at the origin, so a 5x5x1 grid will
/// have voxels from (-2, -2, 0) to (2, 2, 0).
pub fn to_voxel_world(grid: &MjGrid, palette: &MjPalette) -> VoxelWorld {
    let mut world = VoxelWorld::new();

    // Center offset so the grid is centered at origin
    let offset_x = (grid.mx / 2) as i32;
    let offset_y = (grid.my / 2) as i32;
    let offset_z = (grid.mz / 2) as i32;

    for (x, y, z, value) in grid.iter_nonzero() {
        if let Some(voxel) = palette.get(value) {
            let world_x = x as i32 - offset_x;
            let world_y = y as i32 - offset_y;
            let world_z = z as i32 - offset_z;
            world.set_voxel(world_x, world_y, world_z, voxel);
        }
    }

    world
}

impl MjGrid {
    /// Convert this grid to a VoxelWorld using the given palette.
    pub fn to_voxel_world(&self, palette: &MjPalette) -> VoxelWorld {
        to_voxel_world(self, palette)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_default() {
        let palette = MjPalette::default();
        assert!(palette.get(0).is_none()); // 0 is always empty
        assert!(palette.get(1).is_some()); // 1 = white
        assert!(palette.get(2).is_some()); // 2 = red
    }

    #[test]
    fn test_palette_pico8_has_15_colors() {
        let palette = MjPalette::pico8();
        // PICO-8 has 16 colors, but 0 is transparent so 15 are mapped
        for i in 1..=15 {
            assert!(palette.get(i).is_some(), "Missing PICO-8 color {}", i);
        }
        assert!(palette.get(0).is_none());
        assert!(palette.get(16).is_none());
    }

    #[test]
    fn test_to_voxel_world_maps_values() {
        let mut grid = MjGrid::new(3, 3, 1);
        grid.set(1, 1, 0, 1); // center = white

        let palette = MjPalette::default();
        let world = grid.to_voxel_world(&palette);

        // Center of 3x3 grid at (1,1) with offset (-1,-1,0) -> world (0, 0, 0)
        assert!(world.get_voxel(0, 0, 0).is_some());
    }

    #[test]
    fn test_to_voxel_world_skips_zero() {
        let grid = MjGrid::new(3, 3, 1);
        // All zeros, nothing set

        let palette = MjPalette::default();
        let world = grid.to_voxel_world(&palette);

        assert_eq!(world.chunk_count(), 0);
    }

    #[test]
    fn test_to_voxel_world_cross_pattern() {
        let mut grid = MjGrid::new(5, 5, 1);
        // Cross pattern: center + 4 adjacent
        grid.set(2, 2, 0, 1); // center
        grid.set(1, 2, 0, 1); // left
        grid.set(3, 2, 0, 1); // right
        grid.set(2, 1, 0, 1); // down
        grid.set(2, 3, 0, 1); // up

        let palette = MjPalette::default();
        let world = grid.to_voxel_world(&palette);

        // 5x5 grid, offset is (-2, -2, 0)
        // Center (2,2) -> world (0, 0, 0)
        // Left (1,2) -> world (-1, 0, 0)
        // etc.
        assert!(world.get_voxel(0, 0, 0).is_some(), "center missing");
        assert!(world.get_voxel(-1, 0, 0).is_some(), "left missing");
        assert!(world.get_voxel(1, 0, 0).is_some(), "right missing");
        assert!(world.get_voxel(0, -1, 0).is_some(), "down missing");
        assert!(world.get_voxel(0, 1, 0).is_some(), "up missing");
        assert!(world.get_voxel(0, 0, 1).is_none(), "should be empty");
    }
}
