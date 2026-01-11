#!/usr/bin/env python3
"""
Verify sky dome rendering output.

This script analyzes the screenshot from sky_dome_test and checks:
1. Is the image not all black? (pass is running)
2. What are the dominant colors? (expected: magenta for solid color test)
3. Is there any variation? (gradient vs solid)

Usage:
    python3 scripts/verify_sky_dome.py [path_to_image]
    
Default path: test_output/sky_dome_test.png
"""

import sys
from pathlib import Path

try:
    from PIL import Image
    import numpy as np
except ImportError:
    print("ERROR: PIL and numpy required. Install with: pip install Pillow numpy")
    sys.exit(1)


def analyze_image(path: str) -> dict:
    """Analyze the image and return statistics."""
    img = Image.open(path)
    arr = np.array(img)
    
    # Basic info
    height, width = arr.shape[:2]
    channels = arr.shape[2] if len(arr.shape) > 2 else 1
    
    # Color statistics (assuming RGB or RGBA)
    if channels >= 3:
        r = arr[:, :, 0].astype(float)
        g = arr[:, :, 1].astype(float)
        b = arr[:, :, 2].astype(float)
    else:
        r = g = b = arr.astype(float)
    
    return {
        "width": width,
        "height": height,
        "channels": channels,
        "r_mean": r.mean(),
        "g_mean": g.mean(),
        "b_mean": b.mean(),
        "r_std": r.std(),
        "g_std": g.std(),
        "b_std": b.std(),
        "r_min": r.min(),
        "g_min": g.min(),
        "b_min": b.min(),
        "r_max": r.max(),
        "g_max": g.max(),
        "b_max": b.max(),
        # Sample center pixel
        "center_r": r[height//2, width//2],
        "center_g": g[height//2, width//2],
        "center_b": b[height//2, width//2],
        # Sample corners
        "top_left": (r[10, 10], g[10, 10], b[10, 10]),
        "top_right": (r[10, width-10], g[10, width-10], b[10, width-10]),
        "bottom_left": (r[height-10, 10], g[height-10, 10], b[height-10, 10]),
        "bottom_right": (r[height-10, width-10], g[height-10, width-10], b[height-10, width-10]),
    }


def check_solid_magenta(stats: dict) -> tuple[bool, str]:
    """Check if image is solid magenta (R=255, G=0, B=255)."""
    is_magenta = (
        stats["r_mean"] > 240 and
        stats["g_mean"] < 15 and
        stats["b_mean"] > 240 and
        stats["r_std"] < 5 and
        stats["g_std"] < 5 and
        stats["b_std"] < 5
    )
    if is_magenta:
        return True, "PASS: Image is solid magenta - sky dome pass is running!"
    return False, f"FAIL: Not solid magenta. Mean RGB: ({stats['r_mean']:.1f}, {stats['g_mean']:.1f}, {stats['b_mean']:.1f})"


def check_all_black(stats: dict) -> tuple[bool, str]:
    """Check if image is all black."""
    is_black = (
        stats["r_mean"] < 5 and
        stats["g_mean"] < 5 and
        stats["b_mean"] < 5
    )
    if is_black:
        return True, "FAIL: Image is ALL BLACK - sky dome pass is NOT running or being overwritten!"
    return False, "OK: Image is not all black"


def check_has_variation(stats: dict) -> tuple[bool, str]:
    """Check if image has color variation (not solid color)."""
    total_std = stats["r_std"] + stats["g_std"] + stats["b_std"]
    has_variation = total_std > 10
    if has_variation:
        return True, f"INFO: Image has variation (std={total_std:.1f}) - likely gradient or scene content"
    return False, f"INFO: Image is mostly solid (std={total_std:.1f})"


def main():
    # Get image path
    if len(sys.argv) > 1:
        path = sys.argv[1]
    else:
        path = "test_output/sky_dome_test.png"
    
    if not Path(path).exists():
        print(f"ERROR: Image not found: {path}")
        print("Run the test first: cargo run --example sky_dome_test")
        sys.exit(1)
    
    print(f"Analyzing: {path}")
    print("=" * 50)
    
    stats = analyze_image(path)
    
    # Print basic info
    print(f"Size: {stats['width']}x{stats['height']}, Channels: {stats['channels']}")
    print()
    
    # Print color stats
    print("Color Statistics:")
    print(f"  R: mean={stats['r_mean']:.1f}, std={stats['r_std']:.1f}, range=[{stats['r_min']:.0f}, {stats['r_max']:.0f}]")
    print(f"  G: mean={stats['g_mean']:.1f}, std={stats['g_std']:.1f}, range=[{stats['g_min']:.0f}, {stats['g_max']:.0f}]")
    print(f"  B: mean={stats['b_mean']:.1f}, std={stats['b_std']:.1f}, range=[{stats['b_min']:.0f}, {stats['b_max']:.0f}]")
    print()
    
    # Print sample points
    print("Sample Points:")
    print(f"  Center: RGB({stats['center_r']:.0f}, {stats['center_g']:.0f}, {stats['center_b']:.0f})")
    print(f"  Top-Left: RGB{tuple(int(x) for x in stats['top_left'])}")
    print(f"  Top-Right: RGB{tuple(int(x) for x in stats['top_right'])}")
    print(f"  Bottom-Left: RGB{tuple(int(x) for x in stats['bottom_left'])}")
    print(f"  Bottom-Right: RGB{tuple(int(x) for x in stats['bottom_right'])}")
    print()
    
    # Run checks
    print("Checks:")
    
    is_black, msg = check_all_black(stats)
    print(f"  {msg}")
    
    is_magenta, msg = check_solid_magenta(stats)
    print(f"  {msg}")
    
    has_var, msg = check_has_variation(stats)
    print(f"  {msg}")
    
    print()
    
    # Summary
    if is_black:
        print("RESULT: SKY DOME PASS IS NOT WORKING")
        print("The pass either:")
        print("  - Is not running at all")
        print("  - Is being overwritten by another pass")
        print("  - Has a shader compilation error")
        sys.exit(1)
    elif is_magenta:
        print("RESULT: SKY DOME PASS IS WORKING (solid color test)")
        print("Next step: Enable gradient/texture rendering")
        sys.exit(0)
    else:
        print(f"RESULT: Image has content but not expected solid magenta")
        print(f"Mean color: RGB({stats['r_mean']:.0f}, {stats['g_mean']:.0f}, {stats['b_mean']:.0f})")
        sys.exit(2)


if __name__ == "__main__":
    main()
