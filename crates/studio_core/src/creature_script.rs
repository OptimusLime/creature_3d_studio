//! Lua scripting for creature voxel placement.
//!
//! This module provides a simple API for loading Lua scripts that define
//! creature voxel patterns. Scripts call `place_voxel(x, y, z, r, g, b, emission)`
//! to fill voxels in a chunk.
//!
//! # Example Lua Script
//!
//! ```lua
//! -- Place a red voxel at the center
//! place_voxel(8, 8, 8, 255, 0, 0, 0)
//!
//! -- Place a glowing cyan voxel
//! place_voxel(9, 8, 8, 0, 255, 255, 255)
//! ```

use crate::voxel::{Voxel, VoxelChunk};
use mlua::{Lua, Result as LuaResult};
use std::cell::RefCell;
use std::rc::Rc;

/// Load a creature script and execute it to produce a VoxelChunk.
///
/// The script has access to:
/// - `place_voxel(x, y, z, r, g, b, emission)` - Place a voxel at (x,y,z) with color RGB and emission
/// - `clear_voxel(x, y, z)` - Remove a voxel at (x,y,z)
///
/// # Arguments
/// * `script_path` - Path to the Lua script file
///
/// # Returns
/// * `Ok(VoxelChunk)` - The chunk populated by the script
/// * `Err` - If script loading or execution fails
pub fn load_creature_script(script_path: &str) -> LuaResult<VoxelChunk> {
    let src = std::fs::read_to_string(script_path)
        .map_err(|e| mlua::Error::RuntimeError(format!("Failed to read {}: {}", script_path, e)))?;

    execute_creature_script(&src)
}

/// Execute a creature script from a string and return the resulting VoxelChunk.
///
/// Useful for testing or inline script execution.
pub fn execute_creature_script(script_src: &str) -> LuaResult<VoxelChunk> {
    let chunk = Rc::new(RefCell::new(VoxelChunk::new()));

    // Scope the Lua VM so it gets dropped before we try to unwrap the Rc
    {
        let lua = Lua::new();

        // Register place_voxel function
        {
            let chunk_ref = Rc::clone(&chunk);
            let place_voxel = lua.create_function(
                move |_, (x, y, z, r, g, b, emission): (usize, usize, usize, u8, u8, u8, u8)| {
                    let voxel = Voxel::new(r, g, b, emission);
                    chunk_ref.borrow_mut().set(x, y, z, voxel);
                    Ok(())
                },
            )?;
            lua.globals().set("place_voxel", place_voxel)?;
        }

        // Register clear_voxel function
        {
            let chunk_ref = Rc::clone(&chunk);
            let clear_voxel = lua.create_function(move |_, (x, y, z): (usize, usize, usize)| {
                chunk_ref.borrow_mut().clear(x, y, z);
                Ok(())
            })?;
            lua.globals().set("clear_voxel", clear_voxel)?;
        }

        // Register print function for debugging
        let print_fn = lua.create_function(|_, msg: String| {
            println!("[creature] {}", msg);
            Ok(())
        })?;
        lua.globals().set("print", print_fn)?;

        // Execute the script
        lua.load(script_src).exec()?;

        // Lua VM drops here, releasing references to chunk
    }

    // Extract the chunk from the Rc<RefCell<>>
    let result = Rc::try_unwrap(chunk)
        .map_err(|_| mlua::Error::RuntimeError("Failed to unwrap chunk".into()))?
        .into_inner();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_place_single_voxel() {
        let script = r#"
            place_voxel(5, 5, 5, 255, 0, 0, 0)
        "#;

        let chunk = execute_creature_script(script).unwrap();
        assert_eq!(chunk.count(), 1);

        let voxel = chunk.get(5, 5, 5).unwrap();
        assert_eq!(voxel.color, [255, 0, 0]);
        assert_eq!(voxel.emission, 0);
    }

    #[test]
    fn test_place_multiple_voxels() {
        let script = r#"
            place_voxel(0, 0, 0, 255, 0, 0, 0)
            place_voxel(1, 0, 0, 0, 255, 0, 0)
            place_voxel(2, 0, 0, 0, 0, 255, 0)
        "#;

        let chunk = execute_creature_script(script).unwrap();
        assert_eq!(chunk.count(), 3);
    }

    #[test]
    fn test_emissive_voxel() {
        let script = r#"
            place_voxel(8, 8, 8, 255, 0, 255, 200)
        "#;

        let chunk = execute_creature_script(script).unwrap();
        let voxel = chunk.get(8, 8, 8).unwrap();
        assert_eq!(voxel.emission, 200);
    }

    #[test]
    fn test_clear_voxel() {
        let script = r#"
            place_voxel(5, 5, 5, 255, 0, 0, 0)
            clear_voxel(5, 5, 5)
        "#;

        let chunk = execute_creature_script(script).unwrap();
        assert_eq!(chunk.count(), 0);
    }

    #[test]
    fn test_cross_pattern() {
        let script = r#"
            -- Cross pattern centered at (8, 8, 8)
            place_voxel(8, 8, 8, 255, 0, 0, 0)    -- center: red
            place_voxel(9, 8, 8, 0, 255, 0, 0)    -- +X: green
            place_voxel(7, 8, 8, 0, 0, 255, 0)    -- -X: blue
            place_voxel(8, 8, 9, 255, 255, 0, 0)  -- +Z: yellow
            place_voxel(8, 8, 7, 0, 255, 255, 0)  -- -Z: cyan
        "#;

        let chunk = execute_creature_script(script).unwrap();
        assert_eq!(chunk.count(), 5);

        // Verify colors
        assert_eq!(chunk.get(8, 8, 8).unwrap().color, [255, 0, 0]); // red
        assert_eq!(chunk.get(9, 8, 8).unwrap().color, [0, 255, 0]); // green
        assert_eq!(chunk.get(7, 8, 8).unwrap().color, [0, 0, 255]); // blue
        assert_eq!(chunk.get(8, 8, 9).unwrap().color, [255, 255, 0]); // yellow
        assert_eq!(chunk.get(8, 8, 7).unwrap().color, [0, 255, 255]); // cyan
    }
}
