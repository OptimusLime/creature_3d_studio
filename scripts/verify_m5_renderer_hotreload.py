#!/usr/bin/env python3
"""
M5 Verification Script: Lua Renderer Hot Reload

This script verifies that:
1. The app uses LuaRenderLayer (not Rust BaseRenderLayer)
2. Editing grid_2d.lua causes the output to change
3. No app restart is required

Usage:
    # Start the app first in another terminal:
    cargo run --example p_map_editor_2d
    
    # Then run this script:
    python3 scripts/verify_m5_renderer_hotreload.py
"""

import hashlib
import json
import os
import shutil
import sys
import time
import urllib.request

MCP_BASE = "http://127.0.0.1:8088"
RENDERER_PATH = "assets/map_editor/renderers/grid_2d.lua"

def get_png_hash():
    """Get hash of current output PNG."""
    url = f"{MCP_BASE}/mcp/get_output"
    try:
        with urllib.request.urlopen(url, timeout=5) as response:
            data = response.read()
            return hashlib.md5(data).hexdigest()
    except Exception as e:
        print(f"ERROR: Failed to get output: {e}")
        return None

def check_health():
    """Check if MCP server is running."""
    url = f"{MCP_BASE}/health"
    try:
        with urllib.request.urlopen(url, timeout=5) as response:
            return response.status == 200
    except:
        return False

def main():
    print("=" * 60)
    print("M5 Verification: Lua Renderer Hot Reload")
    print("=" * 60)
    
    # Step 1: Check server is running
    print("\n[1] Checking MCP server...")
    if not check_health():
        print("ERROR: MCP server not running!")
        print("Start the app first: cargo run --example p_map_editor_2d")
        sys.exit(1)
    print("OK: MCP server is running")
    
    # Step 2: Get initial PNG hash
    print("\n[2] Getting initial output...")
    hash1 = get_png_hash()
    if not hash1:
        sys.exit(1)
    print(f"Initial hash: {hash1[:16]}...")
    
    # Step 3: Backup original renderer
    print("\n[3] Backing up original renderer...")
    backup_path = RENDERER_PATH + ".backup"
    shutil.copy(RENDERER_PATH, backup_path)
    
    # Step 4: Modify the renderer (invert colors)
    print("\n[4] Modifying renderer (adding red tint)...")
    modified_renderer = '''-- Modified renderer with red tint
local Layer = {}

function Layer:render(ctx, pixels)
  for y = 0, ctx.height - 1 do
    for x = 0, ctx.width - 1 do
      local mat_id = ctx:get_voxel(x, y)
      
      if mat_id == 0 then
        -- Empty cell: dark red instead of gray
        pixels:set_pixel(x, y, 50, 20, 20, 255)
      else
        local r, g, b = ctx:get_material_color(mat_id)
        if r then
          -- Add red tint
          local nr = math.min(255, math.floor(r * 255) + 50)
          pixels:set_pixel(x, y, nr, math.floor(g * 200), math.floor(b * 200), 255)
        else
          pixels:set_pixel(x, y, 255, 0, 255, 255)
        end
      end
    end
  end
end

return Layer
'''
    with open(RENDERER_PATH, 'w') as f:
        f.write(modified_renderer)
    print("Modified renderer written")
    
    # Step 5: Wait for hot reload
    print("\n[5] Waiting for hot reload (2 seconds)...")
    time.sleep(2)
    
    # Step 6: Get new PNG hash
    print("\n[6] Getting output after modification...")
    hash2 = get_png_hash()
    if not hash2:
        # Restore backup
        shutil.move(backup_path, RENDERER_PATH)
        sys.exit(1)
    print(f"Modified hash: {hash2[:16]}...")
    
    # Step 7: Restore original
    print("\n[7] Restoring original renderer...")
    shutil.move(backup_path, RENDERER_PATH)
    time.sleep(1)  # Wait for restore hot reload
    
    # Step 8: Compare hashes
    print("\n" + "=" * 60)
    if hash1 != hash2:
        print("SUCCESS: Output changed after modifying Lua renderer!")
        print(f"  Before: {hash1}")
        print(f"  After:  {hash2}")
        print("\nM5 VERIFIED: Lua renderer hot reload is working!")
        sys.exit(0)
    else:
        print("FAILURE: Output did NOT change!")
        print("The renderer hot reload may not be working correctly.")
        sys.exit(1)

if __name__ == "__main__":
    main()
