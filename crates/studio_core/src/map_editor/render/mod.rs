//! Render layer system for the map editor.
//!
//! Provides a compositable rendering system where multiple layers can be stacked
//! to produce the final output. Each layer implements `RenderLayer` and renders
//! into a shared `PixelBuffer`.
//!
//! # Architecture
//!
//! - `RenderLayer` trait: Interface for anything that can render pixels
//! - `PixelBuffer`: RGBA pixel buffer for rendering
//! - `RenderContext`: Shared context passed to layers (voxel data, materials)
//! - `RenderSurface`: A named render target with its own layer stack
//! - `RenderSurfaceManager`: Manages multiple surfaces with compositing
//!
//! # Example
//!
//! ```ignore
//! let mut manager = RenderSurfaceManager::new();
//! manager.add_surface("grid", 32, 32);
//! manager.add_layer("grid", Box::new(BaseRenderLayer::new()));
//! manager.add_layer("grid", Box::new(visualizer));
//!
//! let pixels = manager.render_composite(&ctx);
//! ```

mod base;
mod frame_capture;
mod lua_layer;
mod pixel_buffer;
mod surface;
mod visualizer;

pub use base::BaseRenderLayer;
pub use frame_capture::FrameCapture;
pub use lua_layer::{LuaRenderLayer, RENDERER_LUA_PATH};
pub use pixel_buffer::PixelBuffer;
pub use surface::{RenderSurface, RenderSurfaceManager, SurfaceInfo, SurfaceLayout};
pub use visualizer::{LuaVisualizer, SharedVisualizer, VISUALIZER_LUA_PATH};

use super::material::MaterialPalette;
use super::voxel_buffer_2d::VoxelBuffer2D;

/// Trait for render layers that can draw into a pixel buffer.
///
/// Layers are composited in order - later layers draw over earlier ones.
/// Each layer can read the current pixel buffer state and modify it.
pub trait RenderLayer: Send + Sync {
    /// Unique name for this layer (used for filtering).
    fn name(&self) -> &str;

    /// Whether this layer is currently enabled.
    fn enabled(&self) -> bool {
        true
    }

    /// Render this layer into the pixel buffer.
    ///
    /// The layer can read from `ctx` to get voxel/material data,
    /// and write to `pixels` to produce output.
    fn render(&self, ctx: &RenderContext, pixels: &mut PixelBuffer);
}

/// Context passed to render layers containing shared data.
pub struct RenderContext<'a> {
    /// The voxel buffer to render.
    pub buffer: &'a VoxelBuffer2D,
    /// Material palette for color lookups.
    pub palette: &'a MaterialPalette,
}

impl<'a> RenderContext<'a> {
    /// Create a new render context.
    pub fn new(buffer: &'a VoxelBuffer2D, palette: &'a MaterialPalette) -> Self {
        Self { buffer, palette }
    }

    /// Get the width of the buffer.
    pub fn width(&self) -> usize {
        self.buffer.width
    }

    /// Get the height of the buffer.
    pub fn height(&self) -> usize {
        self.buffer.height
    }

    /// Get the voxel (material ID) at a position.
    pub fn get_voxel(&self, x: usize, y: usize) -> u32 {
        self.buffer.get(x, y)
    }

    /// Get material color by ID, returns None if not found.
    pub fn get_material_color(&self, id: u32) -> Option<[f32; 3]> {
        self.palette.get_by_id(id).map(|m| m.color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestLayer {
        name: String,
        color: [u8; 4],
    }

    impl RenderLayer for TestLayer {
        fn name(&self) -> &str {
            &self.name
        }

        fn render(&self, ctx: &RenderContext, pixels: &mut PixelBuffer) {
            // Fill entire buffer with our color
            for y in 0..ctx.height() {
                for x in 0..ctx.width() {
                    pixels.set_pixel(x, y, self.color);
                }
            }
        }
    }

    #[test]
    fn test_surface_layer_basic() {
        let buffer = VoxelBuffer2D::new(4, 4);
        let palette = MaterialPalette::default_palette();
        let ctx = RenderContext::new(&buffer, &palette);

        let mut surface = RenderSurface::new("test", 4, 4);
        surface.add_layer(Box::new(TestLayer {
            name: "red".to_string(),
            color: [255, 0, 0, 255],
        }));

        let pixels = surface.render(&ctx);
        assert_eq!(pixels.get_pixel(0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn test_layer_compositing() {
        let buffer = VoxelBuffer2D::new(4, 4);
        let palette = MaterialPalette::default_palette();
        let ctx = RenderContext::new(&buffer, &palette);

        let mut surface = RenderSurface::new("test", 4, 4);
        surface.add_layer(Box::new(TestLayer {
            name: "red".to_string(),
            color: [255, 0, 0, 255],
        }));
        surface.add_layer(Box::new(TestLayer {
            name: "blue".to_string(),
            color: [0, 0, 255, 255],
        }));

        // Blue layer renders last, so it overwrites red
        let pixels = surface.render(&ctx);
        assert_eq!(pixels.get_pixel(0, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn test_layer_filtering() {
        let buffer = VoxelBuffer2D::new(4, 4);
        let palette = MaterialPalette::default_palette();
        let ctx = RenderContext::new(&buffer, &palette);

        let mut surface = RenderSurface::new("test", 4, 4);
        surface.add_layer(Box::new(TestLayer {
            name: "red".to_string(),
            color: [255, 0, 0, 255],
        }));
        surface.add_layer(Box::new(TestLayer {
            name: "blue".to_string(),
            color: [0, 0, 255, 255],
        }));

        // Filter to only red
        let pixels = surface.render_filtered(&ctx, &["red"]);
        assert_eq!(pixels.get_pixel(0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn test_list_layers() {
        let mut surface = RenderSurface::new("test", 4, 4);
        surface.add_layer(Box::new(TestLayer {
            name: "base".to_string(),
            color: [0, 0, 0, 255],
        }));
        surface.add_layer(Box::new(TestLayer {
            name: "overlay".to_string(),
            color: [255, 255, 255, 255],
        }));

        let names = surface.list_layers();
        assert_eq!(names, vec!["base", "overlay"]);
    }
}
