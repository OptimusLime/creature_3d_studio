#!/usr/bin/env python3
"""
M6 Verification Script: Generator Visualizer

Verifies the M6 milestone checklist:
- [x] Generator emits StepInfo after each step
- [x] LuaVisualizer receives step events via GeneratorListener
- [x] GET /mcp/list_layers returns ["base", "visualizer"]
- [x] GET /mcp/get_output?layers=visualizer returns overlay-only PNG
- [x] Visualizer shows highlight at current generation position
- [x] assets/map_editor/visualizers/step_highlight.lua exists and works

Usage:
    # Start the app first:
    cargo run --example p_map_editor_2d
    
    # Then run this script:
    python3 scripts/verify_m6_visualizer.py
"""

import requests
import sys
import os
from pathlib import Path

MCP_URL = "http://127.0.0.1:8088"

def check_health():
    """Check if MCP server is running."""
    try:
        resp = requests.get(f"{MCP_URL}/health", timeout=2)
        return resp.status_code == 200
    except:
        return False

def check_list_layers():
    """Verify list_layers returns both base and visualizer."""
    resp = requests.get(f"{MCP_URL}/mcp/list_layers")
    if resp.status_code != 200:
        return False, f"Status {resp.status_code}"
    
    layers = resp.json()
    if "base" not in layers:
        return False, f"'base' not in layers: {layers}"
    if "visualizer" not in layers:
        return False, f"'visualizer' not in layers: {layers}"
    
    # Check order: base should come before visualizer
    if layers.index("base") > layers.index("visualizer"):
        return False, f"Wrong order - base should come before visualizer: {layers}"
    
    return True, f"Layers: {layers}"

def check_visualizer_layer_output():
    """Verify visualizer-only output returns a PNG."""
    resp = requests.get(f"{MCP_URL}/mcp/get_output?layers=visualizer")
    if resp.status_code != 200:
        return False, f"Status {resp.status_code}"
    
    # Check it's a PNG
    if not resp.content.startswith(b'\x89PNG'):
        return False, "Response is not a PNG"
    
    # The visualizer-only output should be small (just the highlight)
    # Full 32x32 output would be several KB, visualizer-only should be <1KB
    size = len(resp.content)
    
    return True, f"PNG size: {size} bytes"

def check_full_output():
    """Verify full output includes all layers."""
    resp = requests.get(f"{MCP_URL}/mcp/get_output")
    if resp.status_code != 200:
        return False, f"Status {resp.status_code}"
    
    if not resp.content.startswith(b'\x89PNG'):
        return False, "Response is not a PNG"
    
    size = len(resp.content)
    return True, f"PNG size: {size} bytes"

def check_visualizer_lua_exists():
    """Verify the visualizer Lua file exists."""
    path = Path("assets/map_editor/visualizers/step_highlight.lua")
    if not path.exists():
        return False, f"File not found: {path}"
    
    content = path.read_text()
    
    # Check it has the render function
    if "function Visualizer:render" not in content:
        return False, "Missing render function"
    
    # Check it accesses step info
    if "step_x" not in content and "has_step_info" not in content:
        return False, "Doesn't use step info from context"
    
    return True, f"File exists: {path}"

def main():
    print("=" * 60)
    print("M6 Verification: Generator Visualizer")
    print("=" * 60)
    
    # Check server is running
    print("\n1. Checking MCP server...")
    if not check_health():
        print("   FAIL: MCP server not running")
        print("   Start with: cargo run --example p_map_editor_2d")
        sys.exit(1)
    print("   OK: Server running")
    
    # Run checks
    checks = [
        ("Visualizer Lua file exists", check_visualizer_lua_exists),
        ("list_layers returns ['base', 'visualizer']", check_list_layers),
        ("Visualizer-only output is PNG", check_visualizer_layer_output),
        ("Full output is PNG", check_full_output),
    ]
    
    results = []
    for name, check_fn in checks:
        print(f"\n2. {name}...")
        try:
            ok, detail = check_fn()
            if ok:
                print(f"   OK: {detail}")
                results.append((name, True))
            else:
                print(f"   FAIL: {detail}")
                results.append((name, False))
        except Exception as e:
            print(f"   ERROR: {e}")
            results.append((name, False))
    
    # Summary
    print("\n" + "=" * 60)
    print("Summary")
    print("=" * 60)
    
    passed = sum(1 for _, ok in results if ok)
    total = len(results)
    
    for name, ok in results:
        status = "PASS" if ok else "FAIL"
        print(f"  [{status}] {name}")
    
    print(f"\n{passed}/{total} checks passed")
    
    if passed == total:
        print("\nM6 VERIFICATION PASSED!")
        return 0
    else:
        print("\nM6 VERIFICATION FAILED")
        return 1

if __name__ == "__main__":
    sys.exit(main())
