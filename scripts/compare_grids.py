#!/usr/bin/env python3
"""
Compare C# and Rust MarkovJunior grid outputs cell-by-cell.

Usage:
    python scripts/compare_grids.py verification/csharp/Basic_seed42.json verification/rust/Basic_seed42.json
    python scripts/compare_grids.py MarkovJunior/verification/Basic_seed42.json verification/rust/Basic_seed42.json
"""

import json
import sys
from pathlib import Path


def load_json(path):
    """Load a JSON grid state file."""
    with open(path) as f:
        return json.load(f)


def compare(csharp_path, rust_path, verbose=True):
    """Compare two grid state JSON files.
    
    Returns:
        dict with comparison results
    """
    csharp = load_json(csharp_path)
    rust = load_json(rust_path)
    
    result = {
        "model": csharp.get("model", "unknown"),
        "seed": csharp.get("seed", -1),
        "csharp_dimensions": csharp["dimensions"],
        "rust_dimensions": rust["dimensions"],
        "dimensions_match": csharp["dimensions"] == rust["dimensions"],
        "total_cells": len(csharp["state"]),
        "matching_cells": 0,
        "differences": [],
    }
    
    # Check dimensions
    if not result["dimensions_match"]:
        if verbose:
            print(f"DIMENSION MISMATCH: C#={csharp['dimensions']} Rust={rust['dimensions']}")
        return result
    
    # Compare cell-by-cell
    mx, my, mz = csharp["dimensions"]
    for i, (c, r) in enumerate(zip(csharp["state"], rust["state"])):
        if c == r:
            result["matching_cells"] += 1
        else:
            x = i % mx
            y = (i // mx) % my
            z = i // (mx * my)
            result["differences"].append({
                "index": i,
                "x": x,
                "y": y,
                "z": z,
                "csharp": c,
                "rust": r,
            })
    
    # Calculate accuracy
    total = result["total_cells"]
    matching = result["matching_cells"]
    result["accuracy"] = 100.0 * matching / total if total > 0 else 100.0
    result["is_perfect"] = len(result["differences"]) == 0
    
    if verbose:
        print_report(result, csharp, rust)
    
    return result


def print_report(result, csharp, rust):
    """Print a human-readable comparison report."""
    print(f"Model: {result['model']}")
    print(f"Seed: {result['seed']}")
    print(f"Dimensions: {result['csharp_dimensions']}")
    print(f"Total cells: {result['total_cells']}")
    print(f"Matching: {result['matching_cells']} ({result['accuracy']:.2f}%)")
    print(f"Different: {len(result['differences'])}")
    
    if result["is_perfect"]:
        print("\n*** PERFECT MATCH ***")
    elif result["differences"]:
        print(f"\nFirst 20 differences:")
        csharp_chars = csharp.get("characters", [])
        rust_chars = rust.get("characters", [])
        
        for diff in result["differences"][:20]:
            c = diff["csharp"]
            r = diff["rust"]
            c_char = csharp_chars[c] if c < len(csharp_chars) else "?"
            r_char = rust_chars[r] if r < len(rust_chars) else "?"
            print(f"  ({diff['x']},{diff['y']},{diff['z']}): C#={c}({c_char}) Rust={r}({r_char})")
        
        # Analyze patterns
        analyze_diff_patterns(result)


def analyze_diff_patterns(result):
    """Analyze patterns in differences to help debugging."""
    if not result["differences"]:
        return
    
    diffs = result["differences"]
    
    # Check for coordinate patterns
    x_values = set(d["x"] for d in diffs)
    y_values = set(d["y"] for d in diffs)
    z_values = set(d["z"] for d in diffs)
    
    print(f"\nDiff pattern analysis:")
    print(f"  Unique X values: {len(x_values)} (range {min(x_values)}-{max(x_values)})")
    print(f"  Unique Y values: {len(y_values)} (range {min(y_values)}-{max(y_values)})")
    print(f"  Unique Z values: {len(z_values)} (range {min(z_values)}-{max(z_values)})")
    
    # Check for value patterns
    csharp_vals = [d["csharp"] for d in diffs]
    rust_vals = [d["rust"] for d in diffs]
    print(f"  C# values in diffs: {set(csharp_vals)}")
    print(f"  Rust values in diffs: {set(rust_vals)}")
    
    # Check for systematic offset
    if len(diffs) > 10:
        first_diff_idx = diffs[0]["index"]
        print(f"  First diff at index: {first_diff_idx}")


def batch_compare(csharp_dir, rust_dir):
    """Compare all matching JSON files in two directories."""
    csharp_dir = Path(csharp_dir)
    rust_dir = Path(rust_dir)
    
    results = []
    
    for csharp_file in sorted(csharp_dir.glob("*.json")):
        rust_file = rust_dir / csharp_file.name
        if rust_file.exists():
            print(f"\n{'='*60}")
            print(f"Comparing: {csharp_file.name}")
            print('='*60)
            result = compare(csharp_file, rust_file)
            results.append(result)
    
    # Summary
    print(f"\n{'='*60}")
    print("SUMMARY")
    print('='*60)
    
    perfect = [r for r in results if r["is_perfect"]]
    high = [r for r in results if not r["is_perfect"] and r["accuracy"] >= 99.0]
    partial = [r for r in results if r["accuracy"] < 99.0]
    
    print(f"\nPERFECT (100%): {len(perfect)}")
    for r in perfect:
        print(f"  {r['model']} seed={r['seed']}")
    
    print(f"\nHIGH (>=99%): {len(high)}")
    for r in high:
        print(f"  {r['model']} seed={r['seed']}: {r['accuracy']:.2f}%")
    
    print(f"\nPARTIAL (<99%): {len(partial)}")
    for r in partial:
        print(f"  {r['model']} seed={r['seed']}: {r['accuracy']:.2f}%")
    
    return results


def main():
    if len(sys.argv) < 3:
        print("Usage: python compare_grids.py <csharp.json> <rust.json>")
        print("   or: python compare_grids.py --batch <csharp_dir> <rust_dir>")
        sys.exit(1)
    
    if sys.argv[1] == "--batch":
        batch_compare(sys.argv[2], sys.argv[3])
    else:
        compare(sys.argv[1], sys.argv[2])


if __name__ == "__main__":
    main()
