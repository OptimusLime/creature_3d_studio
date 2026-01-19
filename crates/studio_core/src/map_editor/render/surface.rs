//! Multi-surface rendering system.
//!
//! Provides `RenderSurface` for independent render targets and `RenderSurfaceManager`
//! for managing multiple surfaces with compositing support.
//!
//! # Architecture
//!
//! Each `RenderSurface` has its own dimensions and layer stack, enabling:
//! - Multiple independent render targets (e.g., "grid" and "mj_structure")
//! - Per-surface layer management
//! - Composited output for screenshots and video export
//!
//! # Example
//!
//! ```ignore
//! let mut manager = RenderSurfaceManager::new();
//! manager.add_surface("grid", 100, 100);
//! manager.add_surface("mj_structure", 100, 100);
//! manager.set_layout(SurfaceLayout::Horizontal(vec!["mj_structure".into(), "grid".into()]));
//!
//! // Add layers to specific surfaces
//! manager.add_layer("grid", Box::new(base_layer));
//!
//! // Render and composite
//! let composite = manager.render_all(&ctx);
//! ```

use super::pixel_buffer::PixelBuffer;
use super::{RenderContext, RenderLayer};
use bevy::prelude::Resource;
use serde::Serialize;
use std::collections::HashMap;

/// A render target with its own pixel buffer and layer stack.
pub struct RenderSurface {
    /// Unique name for this surface.
    pub name: String,
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Layer stack for this surface.
    layers: Vec<Box<dyn RenderLayer>>,
}

impl RenderSurface {
    /// Create a new render surface with the given dimensions.
    pub fn new(name: impl Into<String>, width: usize, height: usize) -> Self {
        Self {
            name: name.into(),
            width,
            height,
            layers: Vec::new(),
        }
    }

    /// Add a layer to this surface's layer stack.
    pub fn add_layer(&mut self, layer: Box<dyn RenderLayer>) {
        self.layers.push(layer);
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

    /// Check if a layer with the given name exists.
    pub fn has_layer(&self, name: &str) -> bool {
        self.layers.iter().any(|l| l.name() == name)
    }

    /// Get the names of all layers.
    pub fn list_layers(&self) -> Vec<&str> {
        self.layers.iter().map(|l| l.name()).collect()
    }

    /// Render all enabled layers and return the pixel buffer.
    pub fn render(&self, ctx: &RenderContext) -> PixelBuffer {
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

    /// Clear all layers from this surface.
    pub fn clear_layers(&mut self) {
        self.layers.clear();
    }

    /// Get mutable access to a layer by name.
    pub fn get_layer_mut(&mut self, name: &str) -> Option<&mut Box<dyn RenderLayer>> {
        self.layers.iter_mut().find(|l| l.name() == name)
    }
}

/// How surfaces are arranged in the final composite.
#[derive(Clone, Debug, Serialize)]
pub enum SurfaceLayout {
    /// Single surface (no compositing needed).
    Single(String),
    /// Surfaces arranged left-to-right.
    Horizontal(Vec<String>),
    /// Surfaces arranged top-to-bottom.
    Vertical(Vec<String>),
    /// Grid arrangement with specified column count.
    Grid {
        columns: usize,
        surfaces: Vec<String>,
    },
}

impl Default for SurfaceLayout {
    fn default() -> Self {
        SurfaceLayout::Single("grid".to_string())
    }
}

/// Information about the current surface configuration.
#[derive(Clone, Debug, Serialize)]
pub struct SurfaceInfo {
    /// Names of all surfaces.
    pub surfaces: Vec<String>,
    /// Current layout configuration.
    pub layout: SurfaceLayout,
    /// Total composite dimensions [width, height].
    pub total_size: [usize; 2],
}

/// Manages multiple render surfaces and composites them.
#[derive(Resource)]
pub struct RenderSurfaceManager {
    surfaces: HashMap<String, RenderSurface>,
    layout: SurfaceLayout,
}

impl Default for RenderSurfaceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderSurfaceManager {
    /// Create a new empty surface manager.
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            layout: SurfaceLayout::default(),
        }
    }

    /// Add a new surface with the given dimensions.
    ///
    /// If a surface with this name already exists, it is replaced.
    pub fn add_surface(&mut self, name: impl Into<String>, width: usize, height: usize) {
        let name = name.into();
        self.surfaces
            .insert(name.clone(), RenderSurface::new(name, width, height));
    }

    /// Remove a surface by name.
    pub fn remove_surface(&mut self, name: &str) {
        self.surfaces.remove(name);
    }

    /// Check if a surface exists.
    pub fn has_surface(&self, name: &str) -> bool {
        self.surfaces.contains_key(name)
    }

    /// Get a reference to a surface.
    pub fn get_surface(&self, name: &str) -> Option<&RenderSurface> {
        self.surfaces.get(name)
    }

    /// Get a mutable reference to a surface.
    pub fn get_surface_mut(&mut self, name: &str) -> Option<&mut RenderSurface> {
        self.surfaces.get_mut(name)
    }

    /// Add a layer to a specific surface.
    pub fn add_layer(&mut self, surface: &str, layer: Box<dyn RenderLayer>) {
        if let Some(s) = self.surfaces.get_mut(surface) {
            s.add_layer(layer);
        }
    }

    /// Replace a layer on a specific surface.
    pub fn replace_layer(&mut self, surface: &str, layer: Box<dyn RenderLayer>) {
        if let Some(s) = self.surfaces.get_mut(surface) {
            s.replace_layer(layer);
        }
    }

    /// Set the layout for compositing surfaces.
    pub fn set_layout(&mut self, layout: SurfaceLayout) {
        self.layout = layout;
    }

    /// Get the current layout.
    pub fn layout(&self) -> &SurfaceLayout {
        &self.layout
    }

    /// List all surface names.
    pub fn list_surfaces(&self) -> Vec<&str> {
        self.surfaces.keys().map(|s| s.as_str()).collect()
    }

    /// Calculate the total composite dimensions based on layout.
    pub fn composite_dimensions(&self) -> (usize, usize) {
        match &self.layout {
            SurfaceLayout::Single(name) => {
                if let Some(s) = self.surfaces.get(name) {
                    (s.width, s.height)
                } else {
                    (0, 0)
                }
            }
            SurfaceLayout::Horizontal(names) => {
                let mut total_width = 0;
                let mut max_height = 0;
                for name in names {
                    if let Some(s) = self.surfaces.get(name) {
                        total_width += s.width;
                        max_height = max_height.max(s.height);
                    }
                }
                (total_width, max_height)
            }
            SurfaceLayout::Vertical(names) => {
                let mut max_width = 0;
                let mut total_height = 0;
                for name in names {
                    if let Some(s) = self.surfaces.get(name) {
                        max_width = max_width.max(s.width);
                        total_height += s.height;
                    }
                }
                (max_width, total_height)
            }
            SurfaceLayout::Grid { columns, surfaces } => {
                if surfaces.is_empty() || *columns == 0 {
                    return (0, 0);
                }
                // Assume all surfaces have the same dimensions (simplification)
                let cell_width = surfaces
                    .iter()
                    .filter_map(|n| self.surfaces.get(n))
                    .map(|s| s.width)
                    .max()
                    .unwrap_or(0);
                let cell_height = surfaces
                    .iter()
                    .filter_map(|n| self.surfaces.get(n))
                    .map(|s| s.height)
                    .max()
                    .unwrap_or(0);
                let rows = (surfaces.len() + columns - 1) / columns;
                (cell_width * *columns, cell_height * rows)
            }
        }
    }

    /// Get information about the current surface configuration.
    pub fn info(&self) -> SurfaceInfo {
        let (width, height) = self.composite_dimensions();
        SurfaceInfo {
            surfaces: self.surfaces.keys().cloned().collect(),
            layout: self.layout.clone(),
            total_size: [width, height],
        }
    }

    /// Render a single surface and return its pixel buffer.
    pub fn render_surface(&self, name: &str, ctx: &RenderContext) -> Option<PixelBuffer> {
        self.surfaces.get(name).map(|s| s.render(ctx))
    }

    /// Render a single surface with layer filtering.
    pub fn render_surface_filtered(
        &self,
        name: &str,
        ctx: &RenderContext,
        layers: &[&str],
    ) -> Option<PixelBuffer> {
        self.surfaces
            .get(name)
            .map(|s| s.render_filtered(ctx, layers))
    }

    /// Render all surfaces and composite them according to the layout.
    pub fn render_composite(&self, ctx: &RenderContext) -> PixelBuffer {
        let (width, height) = self.composite_dimensions();
        if width == 0 || height == 0 {
            return PixelBuffer::new(0, 0);
        }

        let mut composite = PixelBuffer::new(width, height);

        match &self.layout {
            SurfaceLayout::Single(name) => {
                if let Some(s) = self.surfaces.get(name) {
                    return s.render(ctx);
                }
            }
            SurfaceLayout::Horizontal(names) => {
                let mut x_offset = 0;
                for name in names {
                    if let Some(surface) = self.surfaces.get(name) {
                        let pixels = surface.render(ctx);
                        self.blit(&pixels, &mut composite, x_offset, 0);
                        x_offset += surface.width;
                    }
                }
            }
            SurfaceLayout::Vertical(names) => {
                let mut y_offset = 0;
                for name in names {
                    if let Some(surface) = self.surfaces.get(name) {
                        let pixels = surface.render(ctx);
                        self.blit(&pixels, &mut composite, 0, y_offset);
                        y_offset += surface.height;
                    }
                }
            }
            SurfaceLayout::Grid { columns, surfaces } => {
                let cell_width = surfaces
                    .iter()
                    .filter_map(|n| self.surfaces.get(n))
                    .map(|s| s.width)
                    .max()
                    .unwrap_or(0);
                let cell_height = surfaces
                    .iter()
                    .filter_map(|n| self.surfaces.get(n))
                    .map(|s| s.height)
                    .max()
                    .unwrap_or(0);

                for (i, name) in surfaces.iter().enumerate() {
                    if let Some(surface) = self.surfaces.get(name) {
                        let col = i % columns;
                        let row = i / columns;
                        let pixels = surface.render(ctx);
                        self.blit(&pixels, &mut composite, col * cell_width, row * cell_height);
                    }
                }
            }
        }

        composite
    }

    /// Blit (copy) a source pixel buffer onto a destination at the given offset.
    fn blit(&self, src: &PixelBuffer, dst: &mut PixelBuffer, x_offset: usize, y_offset: usize) {
        for y in 0..src.height {
            for x in 0..src.width {
                let pixel = src.get_pixel(x, y);
                dst.set_pixel(x + x_offset, y + y_offset, pixel);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_editor::material::MaterialPalette;
    use crate::map_editor::voxel_buffer_2d::VoxelBuffer2D;

    struct FillLayer {
        name: String,
        color: [u8; 4],
    }

    impl RenderLayer for FillLayer {
        fn name(&self) -> &str {
            &self.name
        }

        fn render(&self, ctx: &RenderContext, pixels: &mut PixelBuffer) {
            for y in 0..ctx.height() {
                for x in 0..ctx.width() {
                    pixels.set_pixel(x, y, self.color);
                }
            }
        }
    }

    #[test]
    fn test_single_surface() {
        let buffer = VoxelBuffer2D::new(4, 4);
        let palette = MaterialPalette::default_palette();
        let ctx = RenderContext::new(&buffer, &palette);

        let mut manager = RenderSurfaceManager::new();
        manager.add_surface("grid", 4, 4);
        manager.add_layer(
            "grid",
            Box::new(FillLayer {
                name: "red".to_string(),
                color: [255, 0, 0, 255],
            }),
        );

        let pixels = manager.render_surface("grid", &ctx).unwrap();
        assert_eq!(pixels.get_pixel(0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn test_horizontal_composite() {
        let buffer = VoxelBuffer2D::new(4, 4);
        let palette = MaterialPalette::default_palette();
        let ctx = RenderContext::new(&buffer, &palette);

        let mut manager = RenderSurfaceManager::new();
        manager.add_surface("left", 4, 4);
        manager.add_surface("right", 4, 4);

        manager.add_layer(
            "left",
            Box::new(FillLayer {
                name: "red".to_string(),
                color: [255, 0, 0, 255],
            }),
        );
        manager.add_layer(
            "right",
            Box::new(FillLayer {
                name: "blue".to_string(),
                color: [0, 0, 255, 255],
            }),
        );

        manager.set_layout(SurfaceLayout::Horizontal(vec![
            "left".to_string(),
            "right".to_string(),
        ]));

        let composite = manager.render_composite(&ctx);
        assert_eq!(composite.width, 8);
        assert_eq!(composite.height, 4);
        assert_eq!(composite.get_pixel(0, 0), [255, 0, 0, 255]); // Left side is red
        assert_eq!(composite.get_pixel(4, 0), [0, 0, 255, 255]); // Right side is blue
    }

    #[test]
    fn test_vertical_composite() {
        let buffer = VoxelBuffer2D::new(4, 4);
        let palette = MaterialPalette::default_palette();
        let ctx = RenderContext::new(&buffer, &palette);

        let mut manager = RenderSurfaceManager::new();
        manager.add_surface("top", 4, 4);
        manager.add_surface("bottom", 4, 4);

        manager.add_layer(
            "top",
            Box::new(FillLayer {
                name: "red".to_string(),
                color: [255, 0, 0, 255],
            }),
        );
        manager.add_layer(
            "bottom",
            Box::new(FillLayer {
                name: "blue".to_string(),
                color: [0, 0, 255, 255],
            }),
        );

        manager.set_layout(SurfaceLayout::Vertical(vec![
            "top".to_string(),
            "bottom".to_string(),
        ]));

        let composite = manager.render_composite(&ctx);
        assert_eq!(composite.width, 4);
        assert_eq!(composite.height, 8);
        assert_eq!(composite.get_pixel(0, 0), [255, 0, 0, 255]); // Top is red
        assert_eq!(composite.get_pixel(0, 4), [0, 0, 255, 255]); // Bottom is blue
    }

    #[test]
    fn test_surface_info() {
        let mut manager = RenderSurfaceManager::new();
        manager.add_surface("grid", 100, 100);
        manager.add_surface("mj_structure", 100, 100);
        manager.set_layout(SurfaceLayout::Horizontal(vec![
            "mj_structure".to_string(),
            "grid".to_string(),
        ]));

        let info = manager.info();
        assert_eq!(info.surfaces.len(), 2);
        assert_eq!(info.total_size, [200, 100]);
    }

    #[test]
    fn test_layer_operations() {
        let mut manager = RenderSurfaceManager::new();
        manager.add_surface("test", 4, 4);

        manager.add_layer(
            "test",
            Box::new(FillLayer {
                name: "layer1".to_string(),
                color: [255, 0, 0, 255],
            }),
        );

        let surface = manager.get_surface("test").unwrap();
        assert!(surface.has_layer("layer1"));
        assert_eq!(surface.list_layers(), vec!["layer1"]);

        // Replace layer
        manager.replace_layer(
            "test",
            Box::new(FillLayer {
                name: "layer1".to_string(),
                color: [0, 255, 0, 255],
            }),
        );

        // Should still only have one layer
        let surface = manager.get_surface("test").unwrap();
        assert_eq!(surface.list_layers(), vec!["layer1"]);
    }
}
