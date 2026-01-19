//! Base render layer that converts voxels to pixels.
//!
//! This is the foundational layer that reads voxel data and renders
//! materials as solid colors. Other layers can be composited on top.

use super::{PixelBuffer, RenderContext, RenderLayer};

/// Base render layer that renders voxels as solid material colors.
///
/// This layer:
/// - Fills empty voxels (ID 0) with a background color
/// - Looks up material colors from the palette
/// - Renders unknown materials as magenta (for debugging)
pub struct BaseRenderLayer {
    /// Background color for empty voxels.
    pub background_color: [u8; 4],
    /// Color for unknown/missing materials.
    pub unknown_color: [u8; 4],
}

impl Default for BaseRenderLayer {
    fn default() -> Self {
        Self {
            background_color: [30, 30, 30, 255], // Dark gray
            unknown_color: [255, 0, 255, 255],   // Magenta
        }
    }
}

impl BaseRenderLayer {
    /// Create a new base render layer with default colors.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the background color for empty voxels.
    pub fn with_background(mut self, color: [u8; 4]) -> Self {
        self.background_color = color;
        self
    }

    /// Set the color for unknown materials.
    pub fn with_unknown_color(mut self, color: [u8; 4]) -> Self {
        self.unknown_color = color;
        self
    }
}

impl RenderLayer for BaseRenderLayer {
    fn name(&self) -> &str {
        "base"
    }

    fn render(&self, ctx: &RenderContext, pixels: &mut PixelBuffer) {
        for y in 0..ctx.height() {
            for x in 0..ctx.width() {
                let mat_id = ctx.get_voxel(x, y);

                let color = if mat_id == 0 {
                    self.background_color
                } else if let Some(mat_color) = ctx.get_material_color(mat_id) {
                    [
                        (mat_color[0] * 255.0) as u8,
                        (mat_color[1] * 255.0) as u8,
                        (mat_color[2] * 255.0) as u8,
                        255,
                    ]
                } else {
                    self.unknown_color
                };

                pixels.set_pixel(x, y, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_editor::material::{Material, MaterialPalette};
    use crate::map_editor::voxel_buffer::VoxelBuffer;

    #[test]
    fn test_base_layer_renders_materials() {
        let buffer = VoxelBuffer::new_2d(2, 2);
        buffer.set_2d(0, 0, 1); // stone
        buffer.set_2d(1, 0, 2); // dirt
        buffer.set_2d(0, 1, 0); // empty
        buffer.set_2d(1, 1, 99); // unknown

        let palette = MaterialPalette::new(vec![
            Material::new(1, "stone", [0.5, 0.5, 0.5]),
            Material::new(2, "dirt", [0.6, 0.4, 0.2]),
        ]);

        let ctx = RenderContext::new(&buffer, &palette);
        let layer = BaseRenderLayer::new();
        let mut pixels = PixelBuffer::new(2, 2);

        layer.render(&ctx, &mut pixels);

        // Stone at (0,0) - should be gray
        let stone = pixels.get_pixel(0, 0);
        assert_eq!(stone[0], 127); // 0.5 * 255 ≈ 127

        // Dirt at (1,0) - should be brownish
        let dirt = pixels.get_pixel(1, 0);
        assert_eq!(dirt[0], 153); // 0.6 * 255 ≈ 153

        // Empty at (0,1) - should be background
        let empty = pixels.get_pixel(0, 1);
        assert_eq!(empty, [30, 30, 30, 255]);

        // Unknown at (1,1) - should be magenta
        let unknown = pixels.get_pixel(1, 1);
        assert_eq!(unknown, [255, 0, 255, 255]);
    }

    #[test]
    fn test_layer_name() {
        let layer = BaseRenderLayer::new();
        assert_eq!(layer.name(), "base");
    }
}
