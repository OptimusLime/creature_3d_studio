//! Lua-based generator visualizer with hot reload support.
//!
//! The visualizer implements `GeneratorListener` to receive step events
//! and `RenderLayer` to render an overlay. Step info is stored internally,
//! NOT passed through RenderContext.
//!
//! # Lua Protocol
//!
//! ```lua
//! local Visualizer = {}
//!
//! function Visualizer:render(ctx, pixels)
//!   -- ctx.step_x, ctx.step_y - current step position (nil if none)
//!   -- ctx.step_material_id - material placed
//!   -- ctx.step_completed - whether generation is done
//!   -- ctx:has_step_info() - returns true if step info is available
//!   
//!   if ctx:has_step_info() then
//!     pixels:blend_pixel(ctx.step_x, ctx.step_y, 255, 255, 0, 200)
//!   end
//! end
//!
//! return Visualizer
//! ```

use super::{PixelBuffer, RenderContext, RenderLayer};
use crate::map_editor::generator::{GeneratorListener, StepInfo};
use bevy::prelude::*;
use mlua::{Function, Lua, Table, UserData, UserDataMethods};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Default path for the visualizer Lua script.
pub const VISUALIZER_LUA_PATH: &str = "assets/map_editor/visualizers/step_highlight.lua";

/// A visualizer implemented in Lua that receives step events and renders overlays.
///
/// Implements both `GeneratorListener` (to receive step events) and `RenderLayer`
/// (to render the overlay). Step info is stored internally.
pub struct LuaVisualizer {
    name: String,
    lua: Lua,
    visualizer_table: Option<Table>,
    enabled: bool,
    path: PathBuf,
    /// Current step info, updated by on_step()
    current_step: Option<StepInfo>,
}

impl LuaVisualizer {
    /// Create a new Lua visualizer from a script path.
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let name = name.into();
        let lua = Lua::new();

        Self {
            name,
            lua,
            visualizer_table: None,
            enabled: true,
            path,
            current_step: None,
        }
    }

    /// Load or reload the Lua script.
    pub fn reload(&mut self) -> Result<(), String> {
        let src = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("Failed to read {}: {}", self.path.display(), e))?;

        let table: Table = self
            .lua
            .load(&src)
            .eval()
            .map_err(|e| format!("Failed to load Lua script: {:?}", e))?;

        self.visualizer_table = Some(table);
        info!(
            "LuaVisualizer '{}' reloaded from {}",
            self.name,
            self.path.display()
        );
        Ok(())
    }

    /// Check if the visualizer has been loaded.
    pub fn is_loaded(&self) -> bool {
        self.visualizer_table.is_some()
    }

    /// Set enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get the path to the Lua script.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the current step info (for rendering).
    pub fn current_step(&self) -> Option<&StepInfo> {
        self.current_step.as_ref()
    }
}

impl GeneratorListener for LuaVisualizer {
    fn on_step(&mut self, info: &StepInfo) {
        self.current_step = Some(info.clone());
    }

    fn on_reset(&mut self) {
        self.current_step = None;
    }
}

/// Wrapper for context to expose to Lua during rendering.
/// Includes step info from the visualizer's internal state.
struct LuaVisualizerContext {
    width: usize,
    height: usize,
    voxels: Vec<u32>,
    material_colors: Vec<Option<[f32; 3]>>,
    // Step info from visualizer's internal state
    step_x: Option<usize>,
    step_y: Option<usize>,
    step_material_id: Option<u32>,
    step_completed: bool,
}

impl LuaVisualizerContext {
    fn new(ctx: &RenderContext, step_info: Option<&StepInfo>) -> Self {
        let mut voxels = Vec::with_capacity(ctx.width() * ctx.height());
        for y in 0..ctx.height() {
            for x in 0..ctx.width() {
                voxels.push(ctx.get_voxel(x, y));
            }
        }

        let mut material_colors = Vec::with_capacity(256);
        for id in 0..256u32 {
            material_colors.push(ctx.get_material_color(id));
        }

        // Extract step info from visualizer's internal state (NOT RenderContext)
        let (step_x, step_y, step_material_id, step_completed) = if let Some(info) = step_info {
            (
                Some(info.x),
                Some(info.y),
                Some(info.material_id),
                info.completed,
            )
        } else {
            (None, None, None, false)
        };

        Self {
            width: ctx.width(),
            height: ctx.height(),
            voxels,
            material_colors,
            step_x,
            step_y,
            step_material_id,
            step_completed,
        }
    }

    fn get_voxel(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.voxels[y * self.width + x]
        } else {
            0
        }
    }

    fn get_material_color(&self, id: u32) -> Option<[f32; 3]> {
        self.material_colors.get(id as usize).copied().flatten()
    }
}

impl UserData for LuaVisualizerContext {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, this| Ok(this.width));
        fields.add_field_method_get("height", |_, this| Ok(this.height));
        // Step info fields
        fields.add_field_method_get("step_x", |_, this| Ok(this.step_x));
        fields.add_field_method_get("step_y", |_, this| Ok(this.step_y));
        fields.add_field_method_get("step_material_id", |_, this| Ok(this.step_material_id));
        fields.add_field_method_get("step_completed", |_, this| Ok(this.step_completed));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_voxel", |_, this, (x, y): (usize, usize)| {
            Ok(this.get_voxel(x, y))
        });

        methods.add_method("get_material_color", |_, this, id: u32| {
            match this.get_material_color(id) {
                Some(c) => Ok((Some(c[0]), Some(c[1]), Some(c[2]))),
                None => Ok((None, None, None)),
            }
        });

        methods.add_method("has_step_info", |_, this, ()| Ok(this.step_x.is_some()));
    }
}

/// Wrapper for PixelBuffer to expose to Lua.
struct LuaPixelBuffer {
    buffer: Arc<Mutex<PixelBuffer>>,
}

impl LuaPixelBuffer {
    fn new(buffer: Arc<Mutex<PixelBuffer>>) -> Self {
        Self { buffer }
    }
}

impl UserData for LuaPixelBuffer {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "set_pixel",
            |_, this, (x, y, r, g, b, a): (usize, usize, u8, u8, u8, u8)| {
                if let Ok(mut buf) = this.buffer.lock() {
                    buf.set_pixel(x, y, [r, g, b, a]);
                }
                Ok(())
            },
        );

        methods.add_method("get_pixel", |_, this, (x, y): (usize, usize)| {
            if let Ok(buf) = this.buffer.lock() {
                let p = buf.get_pixel(x, y);
                Ok((p[0], p[1], p[2], p[3]))
            } else {
                Ok((0, 0, 0, 0))
            }
        });

        methods.add_method(
            "blend_pixel",
            |_, this, (x, y, r, g, b, a): (usize, usize, u8, u8, u8, u8)| {
                if let Ok(mut buf) = this.buffer.lock() {
                    buf.blend_pixel(x, y, [r, g, b, a]);
                }
                Ok(())
            },
        );

        methods.add_method("fill", |_, this, (r, g, b, a): (u8, u8, u8, u8)| {
            if let Ok(mut buf) = this.buffer.lock() {
                buf.fill([r, g, b, a]);
            }
            Ok(())
        });
    }
}

impl RenderLayer for LuaVisualizer {
    fn name(&self) -> &str {
        &self.name
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn render(&self, ctx: &RenderContext, pixels: &mut PixelBuffer) {
        let Some(ref table) = self.visualizer_table else {
            return;
        };

        // Get the render function
        let render_fn: Function = match table.get("render") {
            Ok(f) => f,
            Err(e) => {
                error!("LuaVisualizer '{}': no render function: {:?}", self.name, e);
                return;
            }
        };

        // Create Lua context with step info from visualizer's internal state
        let lua_ctx = LuaVisualizerContext::new(ctx, self.current_step.as_ref());
        let shared_pixels = Arc::new(Mutex::new(pixels.clone()));
        let lua_pixels = LuaPixelBuffer::new(Arc::clone(&shared_pixels));

        // Call render(self, ctx, pixels)
        if let Err(e) = render_fn.call::<()>((table.clone(), lua_ctx, lua_pixels)) {
            error!("LuaVisualizer '{}': render error: {:?}", self.name, e);
            return;
        }

        // Copy pixels back
        {
            let result = shared_pixels.lock().unwrap();
            pixels.data.copy_from_slice(&result.data);
        }
    }
}

// Note: LuaVisualizer is not Send+Sync because Lua is not thread-safe.
// For Bevy integration, we need to handle this carefully.
unsafe impl Send for LuaVisualizer {}
unsafe impl Sync for LuaVisualizer {}

/// Shared visualizer that can be used as both a listener and a render layer.
///
/// Wraps `LuaVisualizer` in `Arc<Mutex>` so it can be:
/// - Registered as a `GeneratorListener`
/// - Added to `RenderSurfaceManager`'s surface as a `RenderLayer`
#[derive(Clone)]
pub struct SharedVisualizer {
    inner: Arc<Mutex<LuaVisualizer>>,
}

impl SharedVisualizer {
    /// Create a new shared visualizer.
    pub fn new(visualizer: LuaVisualizer) -> Self {
        Self {
            inner: Arc::new(Mutex::new(visualizer)),
        }
    }

    /// Get access to the inner visualizer for reloading.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, LuaVisualizer> {
        self.inner.lock().unwrap()
    }
}

impl GeneratorListener for SharedVisualizer {
    fn on_step(&mut self, info: &StepInfo) {
        if let Ok(mut vis) = self.inner.lock() {
            vis.on_step(info);
        }
    }

    fn on_reset(&mut self) {
        if let Ok(mut vis) = self.inner.lock() {
            vis.on_reset();
        }
    }
}

impl RenderLayer for SharedVisualizer {
    fn name(&self) -> &str {
        "visualizer"
    }

    fn enabled(&self) -> bool {
        if let Ok(vis) = self.inner.lock() {
            vis.enabled()
        } else {
            false
        }
    }

    fn render(&self, ctx: &RenderContext, pixels: &mut PixelBuffer) {
        if let Ok(vis) = self.inner.lock() {
            vis.render(ctx, pixels);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lua_visualizer_creation() {
        let vis = LuaVisualizer::new("test", "test.lua");
        assert_eq!(vis.name(), "test");
        assert!(!vis.is_loaded());
        assert!(vis.enabled());
        assert!(vis.current_step().is_none());
    }

    #[test]
    fn test_visualizer_listener() {
        let mut vis = LuaVisualizer::new("test", "test.lua");

        // Initially no step info
        assert!(vis.current_step().is_none());

        // Receive a step
        vis.on_step(&StepInfo::new(0, 5, 3, 42, false));
        let step = vis.current_step().unwrap();
        assert_eq!(step.x, 5);
        assert_eq!(step.y, 3);
        assert_eq!(step.material_id, 42);
        assert!(!step.completed);

        // Reset clears step info
        vis.on_reset();
        assert!(vis.current_step().is_none());
    }
}
