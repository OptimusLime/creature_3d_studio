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
//! - `RenderLayerStack`: Ordered collection of layers with compositing
//!
//! # Example
//!
//! ```ignore
//! let mut stack = RenderLayerStack::new(32, 32);
//! stack.add_layer(Box::new(BaseRenderLayer::new()));
//! stack.add_layer(Box::new(lua_layer));
//!
//! let pixels = stack.render_all(&ctx);
//! ```

mod base;
mod lua_layer;
mod pixel_buffer;
mod visualizer;

pub use base::BaseRenderLayer;
pub use lua_layer::{LuaRenderLayer, RENDERER_LUA_PATH};
pub use pixel_buffer::PixelBuffer;
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

/// Ordered stack of render layers for compositing.
#[derive(bevy::prelude::Resource)]
pub struct RenderLayerStack {
    layers: Vec<Box<dyn RenderLayer>>,
    width: usize,
    height: usize,
}

impl RenderLayerStack {
    /// Create a new layer stack for the given dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            layers: Vec::new(),
            width,
            height,
        }
    }

    /// Add a layer to the top of the stack.
    pub fn add_layer(&mut self, layer: Box<dyn RenderLayer>) {
        self.layers.push(layer);
    }

    /// Get the names of all layers.
    pub fn list_layers(&self) -> Vec<&str> {
        self.layers.iter().map(|l| l.name()).collect()
    }

    /// Render all enabled layers in order.
    pub fn render_all(&self, ctx: &RenderContext) -> PixelBuffer {
        let mut pixels = PixelBuffer::new(self.width, self.height);

        for layer in &self.layers {
            if layer.enabled() {
                layer.render(ctx, &mut pixels);
            }
        }

        pixels
    }

    /// Render only layers whose names are in the filter list.
    pub fn render_filtered(&self, ctx: &RenderContext, names: &[&str]) -> PixelBuffer {
        let mut pixels = PixelBuffer::new(self.width, self.height);

        for layer in &self.layers {
            if layer.enabled() && names.contains(&layer.name()) {
                layer.render(ctx, &mut pixels);
            }
        }

        pixels
    }

    /// Update dimensions (clears layers - they need to be re-added).
    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    /// Get current width.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get current height.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get mutable access to a layer by name.
    pub fn get_layer_mut(&mut self, name: &str) -> Option<&mut Box<dyn RenderLayer>> {
        self.layers.iter_mut().find(|l| l.name() == name)
    }

    /// Remove all layers.
    pub fn clear(&mut self) {
        self.layers.clear();
    }

    /// Replace a layer by name, or add it if not found.
    pub fn replace_layer(&mut self, layer: Box<dyn RenderLayer>) {
        let name = layer.name();
        if let Some(pos) = self.layers.iter().position(|l| l.name() == name) {
            self.layers[pos] = layer;
        } else {
            self.layers.push(layer);
        }
    }

    /// Remove a layer by name.
    pub fn remove_layer(&mut self, name: &str) {
        self.layers.retain(|l| l.name() != name);
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
    fn test_layer_stack_basic() {
        let buffer = VoxelBuffer2D::new(4, 4);
        let palette = MaterialPalette::default_palette();
        let ctx = RenderContext::new(&buffer, &palette);

        let mut stack = RenderLayerStack::new(4, 4);
        stack.add_layer(Box::new(TestLayer {
            name: "red".to_string(),
            color: [255, 0, 0, 255],
        }));

        let pixels = stack.render_all(&ctx);
        assert_eq!(pixels.get_pixel(0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn test_layer_compositing() {
        let buffer = VoxelBuffer2D::new(4, 4);
        let palette = MaterialPalette::default_palette();
        let ctx = RenderContext::new(&buffer, &palette);

        let mut stack = RenderLayerStack::new(4, 4);
        stack.add_layer(Box::new(TestLayer {
            name: "red".to_string(),
            color: [255, 0, 0, 255],
        }));
        stack.add_layer(Box::new(TestLayer {
            name: "blue".to_string(),
            color: [0, 0, 255, 255],
        }));

        // Blue layer renders last, so it overwrites red
        let pixels = stack.render_all(&ctx);
        assert_eq!(pixels.get_pixel(0, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn test_layer_filtering() {
        let buffer = VoxelBuffer2D::new(4, 4);
        let palette = MaterialPalette::default_palette();
        let ctx = RenderContext::new(&buffer, &palette);

        let mut stack = RenderLayerStack::new(4, 4);
        stack.add_layer(Box::new(TestLayer {
            name: "red".to_string(),
            color: [255, 0, 0, 255],
        }));
        stack.add_layer(Box::new(TestLayer {
            name: "blue".to_string(),
            color: [0, 0, 255, 255],
        }));

        // Filter to only red
        let pixels = stack.render_filtered(&ctx, &["red"]);
        assert_eq!(pixels.get_pixel(0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn test_list_layers() {
        let mut stack = RenderLayerStack::new(4, 4);
        stack.add_layer(Box::new(TestLayer {
            name: "base".to_string(),
            color: [0, 0, 0, 255],
        }));
        stack.add_layer(Box::new(TestLayer {
            name: "overlay".to_string(),
            color: [255, 255, 255, 255],
        }));

        let names = stack.list_layers();
        assert_eq!(names, vec!["base", "overlay"]);
    }
}
