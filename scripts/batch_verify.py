#!/usr/bin/env python3
"""
Batch verification of MarkovJunior models.

This script:
1. Generates C# reference outputs (if missing)
2. Generates Rust outputs (if missing)
3. Compares them and reports results

Usage:
    python scripts/batch_verify.py Basic River Growth    # Verify specific models
    python scripts/batch_verify.py --all                 # Verify all models
    python scripts/batch_verify.py --list-2d             # List 2D models
"""

import argparse
import json
import os
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

REPO_ROOT = Path(__file__).parent.parent
MODELS_XML = REPO_ROOT / "MarkovJunior" / "models.xml"
CSHARP_DIR = REPO_ROOT / "MarkovJunior" / "verification"
RUST_DIR = REPO_ROOT / "verification" / "rust"
STATUS_FILE = REPO_ROOT / "verification" / "status.json"

# Skip these in --all mode
SKIP_MODELS = {
    # Test models
    "RiverTest1", "RiverTest2", "RiverTest3", "RiverTest4", "RiverTest5",
    "RiverTest6", "RiverTest7", "RiverTest8", "RiverTest9", "RiverTest10",
    "FlowersTest1", "FlowersTest2", "FlowersTest3", "FlowersTest4",
    "FlowersTest5", "FlowersTest6",
    # Known unsupported
    "WaveBrickWall", "WaveDungeon", "WaveFlowers",
}


def get_all_models():
    """Get list of all model names from models.xml."""
    tree = ET.parse(MODELS_XML)
    root = tree.getroot()
    seen = set()
    models = []
    for elem in root.findall(".//model"):
        name = elem.get("name")
        if name and name not in seen:
            seen.add(name)
            d = int(elem.get("d", 2))
            size = int(elem.get("size", 16))
            height = int(elem.get("height", 1 if d == 2 else size))
            is_3d = d == 3 or height > 1
            models.append({"name": name, "is_3d": is_3d})
    return models


def generate_csharp(model_name: str, seed: int = 42) -> bool:
    """Generate C# reference output."""
    output_file = CSHARP_DIR / f"{model_name}_seed{seed}.json"
    if output_file.exists():
        return True
    
    CSHARP_DIR.mkdir(parents=True, exist_ok=True)
    
    try:
        result = subprocess.run(
            ["dotnet", "run", "--", "--model", model_name, "--seed", str(seed), "--dump-json"],
            cwd=REPO_ROOT / "MarkovJunior",
            capture_output=True,
            text=True,
            timeout=300,
        )
        return "JSON dumped" in result.stdout
    except subprocess.TimeoutExpired:
        return False
    except Exception as e:
        print(f"  C# error: {e}")
        return False


def generate_rust(model_name: str, seed: int = 42) -> bool:
    """Generate Rust output by running capture_model_state."""
    output_file = RUST_DIR / f"{model_name}_seed{seed}.json"
    if output_file.exists():
        return True
    
    RUST_DIR.mkdir(parents=True, exist_ok=True)
    
    # Use environment variable to pass model name
    env = os.environ.copy()
    env["MJ_MODELS"] = model_name
    env["MJ_SEED"] = str(seed)
    
    try:
        result = subprocess.run(
            ["cargo", "test", "-p", "studio_core",
             "verification::tests::batch_generate_outputs",
             "--", "--ignored", "--nocapture"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=300,
            env=env,
        )
        return output_file.exists()
    except subprocess.TimeoutExpired:
        return False
    except Exception as e:
        print(f"  Rust error: {e}")
        return False


def compare(model_name: str, seed: int = 42) -> tuple:
    """Compare C# and Rust outputs. Returns (is_match, accuracy, message)."""
    csharp_file = CSHARP_DIR / f"{model_name}_seed{seed}.json"
    rust_file = RUST_DIR / f"{model_name}_seed{seed}.json"
    
    if not csharp_file.exists():
        return False, 0.0, "No C# reference"
    if not rust_file.exists():
        return False, 0.0, "No Rust output"
    
    try:
        with open(csharp_file) as f:
            csharp = json.load(f)
        with open(rust_file) as f:
            rust = json.load(f)
    except Exception as e:
        return False, 0.0, f"JSON error: {e}"
    
    if csharp["dimensions"] != rust["dimensions"]:
        return False, 0.0, f"Dim mismatch: {csharp['dimensions']} vs {rust['dimensions']}"
    
    cs_state = csharp["state"]
    rs_state = rust["state"]
    
    if len(cs_state) != len(rs_state):
        return False, 0.0, f"Length mismatch: {len(cs_state)} vs {len(rs_state)}"
    
    matches = sum(1 for c, r in zip(cs_state, rs_state) if c == r)
    accuracy = 100.0 * matches / len(cs_state) if cs_state else 100.0
    
    if matches == len(cs_state):
        return True, 100.0, "MATCH"
    else:
        return False, accuracy, f"{accuracy:.2f}% ({len(cs_state) - matches} differ)"


def verify_model(model_name: str, seed: int = 42, regenerate: bool = False) -> dict:
    """Verify a single model. Returns result dict."""
    result = {"name": model_name, "seed": seed}
    
    # Generate if needed
    if regenerate:
        csharp_file = CSHARP_DIR / f"{model_name}_seed{seed}.json"
        rust_file = RUST_DIR / f"{model_name}_seed{seed}.json"
        if csharp_file.exists():
            csharp_file.unlink()
        if rust_file.exists():
            rust_file.unlink()
    
    if not generate_csharp(model_name, seed):
        result["status"] = "csharp_failed"
        result["message"] = "C# generation failed"
        return result
    
    if not generate_rust(model_name, seed):
        result["status"] = "rust_failed"
        result["message"] = "Rust generation failed"
        return result
    
    is_match, accuracy, message = compare(model_name, seed)
    result["accuracy"] = accuracy
    result["message"] = message
    result["status"] = "verified" if is_match else "failed"
    
    return result


def update_status(results: list):
    """Update status.json with results."""
    status = {"verified": {}, "failed": {}, "skipped": {}}
    if STATUS_FILE.exists():
        with open(STATUS_FILE) as f:
            status = json.load(f)
    
    for r in results:
        name = r["name"]
        if r["status"] == "verified":
            status["verified"][name] = {
                "seed": r["seed"],
                "accuracy": r["accuracy"],
            }
            if name in status.get("failed", {}):
                del status["failed"][name]
        elif r["status"] == "failed":
            status["failed"][name] = {
                "seed": r["seed"],
                "accuracy": r.get("accuracy", 0),
                "reason": r["message"],
            }
    
    STATUS_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(STATUS_FILE, "w") as f:
        json.dump(status, f, indent=2, sort_keys=True)


def main():
    parser = argparse.ArgumentParser(description="Batch verify MarkovJunior models")
    parser.add_argument("models", nargs="*", help="Model names to verify")
    parser.add_argument("--all", action="store_true", help="Verify all models")
    parser.add_argument("--all-2d", action="store_true", help="Verify all 2D models")
    parser.add_argument("--list-2d", action="store_true", help="List 2D models")
    parser.add_argument("--list-3d", action="store_true", help="List 3D models")
    parser.add_argument("--seed", type=int, default=42, help="Random seed")
    parser.add_argument("--regenerate", action="store_true", help="Regenerate outputs")
    parser.add_argument("-j", "--jobs", type=int, default=1, help="Parallel jobs")
    
    args = parser.parse_args()
    
    all_models = get_all_models()
    models_2d = [m for m in all_models if not m["is_3d"] and m["name"] not in SKIP_MODELS]
    models_3d = [m for m in all_models if m["is_3d"] and m["name"] not in SKIP_MODELS]
    
    if args.list_2d:
        print("2D Models:")
        for m in sorted(models_2d, key=lambda x: x["name"]):
            print(f"  {m['name']}")
        return
    
    if args.list_3d:
        print("3D Models:")
        for m in sorted(models_3d, key=lambda x: x["name"]):
            print(f"  {m['name']}")
        return
    
    # Determine which models to verify
    if args.all:
        to_verify = [m["name"] for m in all_models if m["name"] not in SKIP_MODELS]
    elif args.all_2d:
        to_verify = [m["name"] for m in models_2d]
    elif args.models:
        to_verify = args.models
    else:
        parser.print_help()
        return
    
    print(f"Verifying {len(to_verify)} models (seed={args.seed})...\n")
    
    results = []
    verified = 0
    failed = 0
    
    for model in to_verify:
        result = verify_model(model, args.seed, args.regenerate)
        results.append(result)
        
        status_icon = "OK" if result["status"] == "verified" else "FAIL"
        if result["status"] == "verified":
            verified += 1
            print(f"  [{status_icon}] {model}")
        else:
            failed += 1
            print(f"  [{status_icon}] {model}: {result['message']}")
    
    # Update status file
    update_status(results)
    
    print(f"\nResults: {verified} verified, {failed} failed out of {len(to_verify)}")
    print(f"Status saved to: {STATUS_FILE}")


if __name__ == "__main__":
    main()
