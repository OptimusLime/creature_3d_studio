#!/usr/bin/env python3
"""
MarkovJunior Model Verification Status Tracker

This script tracks which models have been verified against C# reference output.
It maintains a JSON file with verification status and can:
1. List all models and their verification status
2. Generate C# reference outputs for unverified models
3. Run Rust verification tests
4. Compare outputs and update status

Usage:
    python scripts/verification_status.py status          # Show verification status
    python scripts/verification_status.py list-unverified # List models needing verification
    python scripts/verification_status.py verify MODEL    # Verify a specific model
    python scripts/verification_status.py verify-all      # Verify all models (slow)
"""

import argparse
import json
import os
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Dict, List, Optional, Tuple

# Paths relative to repo root
REPO_ROOT = Path(__file__).parent.parent
MODELS_XML = REPO_ROOT / "MarkovJunior" / "models.xml"
VERIFICATION_DIR = REPO_ROOT / "MarkovJunior" / "verification"
RUST_VERIFICATION_DIR = REPO_ROOT / "verification" / "rust"
STATUS_FILE = REPO_ROOT / "verification" / "status.json"

# Models we know don't work yet (skip in automated testing)
KNOWN_UNSUPPORTED = {
    # WFC tile models (not fully implemented)
    "WaveBrickWall", "WaveDungeon", "WaveFlowers",
    # ConvChain 3D (explicitly not supported)
    # Add others as discovered
}

# Test models we created for debugging (skip in final verification)
TEST_MODELS = {
    "RiverTest1", "RiverTest2", "RiverTest3", "RiverTest4", "RiverTest5",
    "RiverTest6", "RiverTest7", "RiverTest8", "RiverTest9", "RiverTest10",
    "FlowersTest1", "FlowersTest2", "FlowersTest3", "FlowersTest4",
    "FlowersTest5", "FlowersTest6",
}


def parse_models_xml() -> List[Dict]:
    """Parse models.xml and return list of unique model configurations."""
    if not MODELS_XML.exists():
        print(f"ERROR: {MODELS_XML} not found")
        sys.exit(1)
    
    tree = ET.parse(MODELS_XML)
    root = tree.getroot()
    
    models = {}
    for elem in root.findall(".//model"):
        name = elem.get("name")
        if not name:
            continue
        
        # Only keep first occurrence of each model name
        if name in models:
            continue
        
        # Parse dimensions
        size = int(elem.get("size", 16))
        length = int(elem.get("length", size))
        width = int(elem.get("width", size))
        d = int(elem.get("d", 2))
        height = int(elem.get("height", 1 if d == 2 else size))
        steps = int(elem.get("steps", 50000))
        
        models[name] = {
            "name": name,
            "mx": length,
            "my": width,
            "mz": height,
            "is_3d": d == 3 or height > 1,
            "steps": steps,
        }
    
    return list(models.values())


def load_status() -> Dict:
    """Load verification status from JSON file."""
    if STATUS_FILE.exists():
        with open(STATUS_FILE) as f:
            return json.load(f)
    return {"verified": {}, "failed": {}, "skipped": {}}


def save_status(status: Dict):
    """Save verification status to JSON file."""
    STATUS_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(STATUS_FILE, "w") as f:
        json.dump(status, f, indent=2, sort_keys=True)


def generate_csharp_reference(model_name: str, seed: int = 42) -> bool:
    """Generate C# reference output for a model."""
    VERIFICATION_DIR.mkdir(parents=True, exist_ok=True)
    
    result = subprocess.run(
        ["dotnet", "run", "--", "--model", model_name, "--seed", str(seed), "--dump-json"],
        cwd=REPO_ROOT / "MarkovJunior",
        capture_output=True,
        text=True,
        timeout=120,
    )
    
    if "JSON dumped" in result.stdout:
        return True
    
    print(f"  C# generation failed: {result.stderr}")
    return False


def run_rust_verification(model_name: str, seed: int = 42) -> Optional[Path]:
    """Run Rust model and save output JSON."""
    RUST_VERIFICATION_DIR.mkdir(parents=True, exist_ok=True)
    
    # We'll need to create a test or use existing infrastructure
    # For now, use cargo test with a specific test
    result = subprocess.run(
        ["cargo", "test", "-p", "studio_core", 
         f"verification::tests::test_single_model_{model_name.lower()}", 
         "--", "--nocapture"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        timeout=120,
    )
    
    output_file = RUST_VERIFICATION_DIR / f"{model_name}_seed{seed}.json"
    if output_file.exists():
        return output_file
    
    return None


def compare_outputs(model_name: str, seed: int = 42) -> Tuple[bool, float, str]:
    """Compare C# and Rust outputs. Returns (is_match, accuracy, details)."""
    csharp_file = VERIFICATION_DIR / f"{model_name}_seed{seed}.json"
    rust_file = RUST_VERIFICATION_DIR / f"{model_name}_seed{seed}.json"
    
    if not csharp_file.exists():
        return False, 0.0, f"C# reference not found: {csharp_file}"
    
    if not rust_file.exists():
        return False, 0.0, f"Rust output not found: {rust_file}"
    
    with open(csharp_file) as f:
        csharp = json.load(f)
    with open(rust_file) as f:
        rust = json.load(f)
    
    # Compare dimensions
    if csharp["dimensions"] != rust["dimensions"]:
        return False, 0.0, f"Dimension mismatch: C#={csharp['dimensions']} Rust={rust['dimensions']}"
    
    # Compare state
    csharp_state = csharp["state"]
    rust_state = rust["state"]
    
    if len(csharp_state) != len(rust_state):
        return False, 0.0, f"State length mismatch: C#={len(csharp_state)} Rust={len(rust_state)}"
    
    matches = sum(1 for c, r in zip(csharp_state, rust_state) if c == r)
    accuracy = 100.0 * matches / len(csharp_state)
    
    if matches == len(csharp_state):
        return True, 100.0, "PERFECT MATCH"
    else:
        diff_count = len(csharp_state) - matches
        return False, accuracy, f"{diff_count} cells differ ({accuracy:.2f}% match)"


def cmd_status(args):
    """Show verification status."""
    models = parse_models_xml()
    status = load_status()
    
    # Filter out test models
    real_models = [m for m in models if m["name"] not in TEST_MODELS]
    
    verified = set(status.get("verified", {}).keys())
    failed = set(status.get("failed", {}).keys())
    skipped = set(status.get("skipped", {}).keys()) | KNOWN_UNSUPPORTED
    
    total = len(real_models)
    verified_count = len([m for m in real_models if m["name"] in verified])
    failed_count = len([m for m in real_models if m["name"] in failed])
    skipped_count = len([m for m in real_models if m["name"] in skipped])
    pending_count = total - verified_count - failed_count - skipped_count
    
    print(f"\n{'='*60}")
    print(f"MarkovJunior Model Verification Status")
    print(f"{'='*60}")
    print(f"Total models:    {total}")
    print(f"  Verified:      {verified_count} ({100*verified_count/total:.1f}%)")
    print(f"  Failed:        {failed_count}")
    print(f"  Skipped:       {skipped_count}")
    print(f"  Pending:       {pending_count}")
    print(f"{'='*60}\n")
    
    if args.verbose:
        print("Verified models:")
        for name in sorted(verified):
            info = status["verified"].get(name, {})
            print(f"  {name}: {info.get('accuracy', '?')}% (seed {info.get('seed', '?')})")
        
        if failed:
            print("\nFailed models:")
            for name in sorted(failed):
                info = status["failed"].get(name, {})
                print(f"  {name}: {info.get('accuracy', '?')}% - {info.get('reason', '?')}")


def cmd_list_unverified(args):
    """List models that need verification."""
    models = parse_models_xml()
    status = load_status()
    
    verified = set(status.get("verified", {}).keys())
    skipped = set(status.get("skipped", {}).keys()) | KNOWN_UNSUPPORTED | TEST_MODELS
    
    unverified = [m for m in models if m["name"] not in verified and m["name"] not in skipped]
    
    print(f"Unverified models ({len(unverified)}):\n")
    
    # Group by 2D vs 3D
    models_2d = [m for m in unverified if not m["is_3d"]]
    models_3d = [m for m in unverified if m["is_3d"]]
    
    if models_2d:
        print("2D Models:")
        for m in sorted(models_2d, key=lambda x: x["name"]):
            print(f"  {m['name']} ({m['mx']}x{m['my']})")
    
    if models_3d:
        print("\n3D Models:")
        for m in sorted(models_3d, key=lambda x: x["name"]):
            print(f"  {m['name']} ({m['mx']}x{m['my']}x{m['mz']})")


def cmd_verify(args):
    """Verify a specific model."""
    model_name = args.model
    seed = args.seed
    
    print(f"Verifying {model_name} (seed {seed})...")
    
    # Check if C# reference exists, generate if not
    csharp_file = VERIFICATION_DIR / f"{model_name}_seed{seed}.json"
    if not csharp_file.exists():
        print(f"  Generating C# reference...")
        if not generate_csharp_reference(model_name, seed):
            print(f"  FAILED: Could not generate C# reference")
            return
    
    # Check if Rust output exists
    rust_file = RUST_VERIFICATION_DIR / f"{model_name}_seed{seed}.json"
    if not rust_file.exists():
        print(f"  Rust output not found. Run verification test first.")
        print(f"  Expected: {rust_file}")
        return
    
    # Compare
    is_match, accuracy, details = compare_outputs(model_name, seed)
    
    # Update status
    status = load_status()
    if is_match:
        status["verified"][model_name] = {
            "seed": seed,
            "accuracy": accuracy,
            "verified_at": subprocess.check_output(["date", "-u", "+%Y-%m-%dT%H:%M:%SZ"]).decode().strip()
        }
        if model_name in status.get("failed", {}):
            del status["failed"][model_name]
        print(f"  PASSED: {details}")
    else:
        status["failed"][model_name] = {
            "seed": seed,
            "accuracy": accuracy,
            "reason": details
        }
        print(f"  FAILED: {details}")
    
    save_status(status)


def main():
    parser = argparse.ArgumentParser(description="MarkovJunior Model Verification")
    subparsers = parser.add_subparsers(dest="command", help="Command to run")
    
    # status command
    status_parser = subparsers.add_parser("status", help="Show verification status")
    status_parser.add_argument("-v", "--verbose", action="store_true", help="Show details")
    
    # list-unverified command
    list_parser = subparsers.add_parser("list-unverified", help="List unverified models")
    
    # verify command
    verify_parser = subparsers.add_parser("verify", help="Verify a model")
    verify_parser.add_argument("model", help="Model name")
    verify_parser.add_argument("--seed", type=int, default=42, help="Random seed")
    
    args = parser.parse_args()
    
    if args.command == "status":
        cmd_status(args)
    elif args.command == "list-unverified":
        cmd_list_unverified(args)
    elif args.command == "verify":
        cmd_verify(args)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
