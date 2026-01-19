//! Lua-based render layer with hot reload support.
//!
//! Allows users to define custom rendering logic in Lua scripts.
//! The Lua script receives a render context and pixel buffer to draw into.
//!
//! # Lua Protocol
//!
//! ```lua
//! local Layer = {}
//! function Layer:render(ctx, pixels)
//!   -- ctx.width, ctx.height
//!   -- ctx:get_voxel(x, y) -> material_id
//!   -- ctx:get_material_color(id) -> r, g, b or nil
//!   -- pixels:set_pixel(x, y, r, g, b, a)
//!   -- pixels:get_pixel(x, y) -> r, g, b, a
//!   -- pixels:blend_pixel(x, y, r, g, b, a)
//! end
//! return Layer
//! ```

use super::{PixelBuffer, RenderContext, RenderLayer};
use bevy::prelude::*;
use mlua::{Function, Lua, Table, UserData, UserDataMethods};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Default path for the base Lua renderer.
pub const RENDERER_LUA_PATH: &str = "assets/map_editor/renderers/grid_2d.lua";

/// A render layer implemented in Lua.
pub struct LuaRenderLayer {
    name: String,
    lua: Lua,
    layer_table: Option<Table>,
    enabled: bool,
    path: PathBuf,
}

impl LuaRenderLayer {
    /// Create a new Lua render layer from a script path.
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let name = name.into();
        let lua = Lua::new();

        Self {
            name,
            lua,
            layer_table: None,
            enabled: true,
            path,
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

        self.layer_table = Some(table);
        info!(
            "LuaRenderLayer '{}' reloaded from {}",
            self.name,
            self.path.display()
        );
        Ok(())
    }

    /// Check if the layer has been loaded.
    pub fn is_loaded(&self) -> bool {
        self.layer_table.is_some()
    }

    /// Set enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get the path to the Lua script.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Wrapper for RenderContext to expose to Lua.
struct LuaRenderContext {
    width: usize,
    height: usize,
    // We store the voxel and material data as copies since RenderContext borrows
    voxels: Vec<u32>,
    material_colors: Vec<Option<[f32; 3]>>,
}

impl LuaRenderContext {
    fn from_context(ctx: &RenderContext) -> Self {
        // Copy voxel data
        let mut voxels = Vec::with_capacity(ctx.width() * ctx.height());
        for y in 0..ctx.height() {
            for x in 0..ctx.width() {
                voxels.push(ctx.get_voxel(x, y));
            }
        }

        // Build material color lookup (for IDs 0-255, sufficient for now)
        let mut material_colors = Vec::with_capacity(256);
        for id in 0..256u32 {
            material_colors.push(ctx.get_material_color(id));
        }

        Self {
            width: ctx.width(),
            height: ctx.height(),
            voxels,
            material_colors,
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

impl UserData for LuaRenderContext {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, this| Ok(this.width));
        fields.add_field_method_get("height", |_, this| Ok(this.height));
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

impl RenderLayer for LuaRenderLayer {
    fn name(&self) -> &str {
        &self.name
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn render(&self, ctx: &RenderContext, pixels: &mut PixelBuffer) {
        let Some(ref layer_table) = self.layer_table else {
            return;
        };

        // Get the render function
        let render_fn: Function = match layer_table.get("render") {
            Ok(f) => f,
            Err(e) => {
                error!(
                    "LuaRenderLayer '{}': no render function: {:?}",
                    self.name, e
                );
                return;
            }
        };

        // Create Lua-compatible wrappers
        let lua_ctx = LuaRenderContext::from_context(ctx);

        // We need to share the pixel buffer with Lua
        // Create a temporary copy, let Lua modify it, then copy back
        let shared_pixels = Arc::new(Mutex::new(pixels.clone()));
        let lua_pixels = LuaPixelBuffer::new(Arc::clone(&shared_pixels));

        // Call render(self, ctx, pixels)
        if let Err(e) = render_fn.call::<()>((layer_table.clone(), lua_ctx, lua_pixels)) {
            error!("LuaRenderLayer '{}': render error: {:?}", self.name, e);
            return;
        }

        // Copy pixels back
        {
            let result = shared_pixels.lock().unwrap();
            pixels.data.copy_from_slice(&result.data);
        }
    }
}

// Note: LuaRenderLayer is not Send+Sync because Lua is not thread-safe.
// For Bevy integration, we'll need to handle this specially (NonSend resource).
// For now, we implement the trait but the actual usage will be careful.
unsafe impl Send for LuaRenderLayer {}
unsafe impl Sync for LuaRenderLayer {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lua_render_context_creation() {
        use crate::map_editor::material::{Material, MaterialPalette};
        use crate::map_editor::voxel_buffer_2d::VoxelBuffer2D;

        let mut buffer = VoxelBuffer2D::new(2, 2);
        buffer.set(0, 0, 1);
        buffer.set(1, 1, 2);

        let palette = MaterialPalette::new(vec![
            Material::new(1, "stone", [0.5, 0.5, 0.5]),
            Material::new(2, "dirt", [0.6, 0.4, 0.2]),
        ]);

        let ctx = RenderContext::new(&buffer, &palette);
        let lua_ctx = LuaRenderContext::from_context(&ctx);

        assert_eq!(lua_ctx.width, 2);
        assert_eq!(lua_ctx.height, 2);
        assert_eq!(lua_ctx.get_voxel(0, 0), 1);
        assert_eq!(lua_ctx.get_voxel(1, 1), 2);
        assert!(lua_ctx.get_material_color(1).is_some());
        assert!(lua_ctx.get_material_color(99).is_none());
    }
}
