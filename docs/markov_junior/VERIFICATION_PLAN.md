# MarkovJunior Verification Plan - Phase 5: Cell-by-Cell Accuracy

## Problem Statement

The Rust port now passes 300 tests and produces visible structure for 3D models. However, **visual inspection is insufficient** - output looks "close but not quite right." We need **100% cell-by-cell accuracy** verification against the C# reference implementation.

## Approach: Ground Truth Comparison

1. **Run C# Reference**: Execute original MarkovJunior with deterministic seeds
2. **Capture Reference Output**: Save VOX files or grid state for each model
3. **Run Rust Implementation**: Execute our port with same seeds
4. **Cell-by-Cell Diff**: Compare every cell value, report exact differences
5. **Debug Systematically**: Use diffs to identify root causes

## Key Insight

Visual comparison fails because:
- Random patterns look "plausible" even when wrong
- Small systematic errors are invisible to the eye
- 95% accuracy looks identical to 100% accuracy visually

Cell-by-cell comparison reveals:
- Exact coordinates where values differ
- Patterns in failures (e.g., "all Z>5 cells wrong")
- Whether errors are random or systematic

## 3D Models Requiring Verification

From `MarkovJunior/models.xml`, 3D models (d="3" or d>1):

| Model | Grid Size | Notes |
|-------|-----------|-------|
| Apartemazements | 8x8x8 | WFC tile + children |
| CarmaTower | 12x12x18 | Complex structure |
| ModernHouse | 9x9x4 | Building |
| SeaVilla | 10x10x4 | Building |
| ClosedSurface | 12x12x12 | Surface generation |
| ColoredKnots | 12x12x12 | Knot patterns |
| Counting | 8x8x8 | 3D counting |
| ParallelGrowth | 29x29x29 | 3D growth (fixed) |
| StairsPath | 33x33x33 | 3D stairs |
| Surface | 10x10x10 | Surface |

---

## Phase 5.0: C# Reference Environment Setup

**Outcome:** Can run C# MarkovJunior and capture deterministic output  
**Verification:** `cd MarkovJunior && dotnet run -- BasicGrowth3D 0` produces same output twice

### Tasks

1. **Verify .NET environment**
   ```bash
   dotnet --version  # Need .NET 6.0+
   ```

2. **Build C# MarkovJunior**
   ```bash
   cd MarkovJunior
   dotnet build
   ```

3. **Modify C# to accept seed argument** (if needed)
   - Check if `Interpreter.cs` supports command-line seed
   - If not, add simple argument parsing

4. **Test determinism**
   ```bash
   cd MarkovJunior
   dotnet run -- Basic 42 > output1.txt
   dotnet run -- Basic 42 > output2.txt
   diff output1.txt output2.txt  # Should be empty
   ```

5. **Document exact command format**
   - Create `scripts/run_csharp.sh` wrapper

### Verification
- Run same model+seed twice
- Output files are byte-identical

---

## Phase 5.1: C# Output Capture Script

**Outcome:** Script that captures C# output as comparable data  
**Verification:** `python scripts/capture_csharp.py Basic --seed 0` creates `verification/csharp/Basic_seed0.json`

### Tasks

1. **Determine C# output format**
   - Check what files C# produces (VOX? PNG? Console?)
   - Find where grid state is accessible

2. **Create capture script** `scripts/capture_csharp.py`
   ```python
   # Runs C# MarkovJunior
   # Captures final grid state
   # Saves as JSON: {"dimensions": [x,y,z], "values": [...], "characters": [...]}
   ```

3. **Test on simple model**
   ```bash
   python scripts/capture_csharp.py Basic --seed 0
   cat verification/csharp/Basic_seed0.json
   ```

### Output Format
```json
{
  "model": "Basic",
  "seed": 0,
  "dimensions": [60, 60, 1],
  "characters": ["B", "W"],
  "grid": [0, 0, 1, 1, 0, ...]  // Flat array of cell values
}
```

---

## Phase 5.2: Rust Output Capture 

**Outcome:** Rust outputs same JSON format as C#  
**Verification:** `cargo test capture_model_output -- Basic 0` creates matching JSON

### Tasks

1. **Create Rust test harness** in `tile_node.rs` or new file
   ```rust
   #[test]
   fn capture_model_output() {
       // Parse args or env for model name and seed
       // Load model, run with seed
       // Output JSON to verification/rust/{model}_seed{seed}.json
   }
   ```

2. **Match C# JSON format exactly**
   - Same field names
   - Same value ordering (x + y*mx + z*mx*my)

3. **Test on same models as Phase 5.1**

---

## Phase 5.3: Comparison Script

**Outcome:** Script that diffs C# vs Rust cell-by-cell  
**Verification:** `python scripts/compare.py Basic --seed 0` outputs accuracy report

### Tasks

1. **Create `scripts/compare.py`**
   ```python
   def compare(csharp_json, rust_json):
       # Load both
       # Check dimensions match
       # Compare cell-by-cell
       # Report: total, matching, different, accuracy %
       # List first 20 differences with coordinates
   ```

2. **Output format**
   ```
   Model: Basic
   Seed: 0
   C# Dimensions: 60x60x1
   Rust Dimensions: 60x60x1
   
   Total cells: 3600
   Matching: 3595 (99.86%)
   Different: 5
   
   First differences:
     (12, 34, 0): C#=0 Rust=1
     (15, 22, 0): C#=1 Rust=0
     ...
   ```

3. **Test on captured outputs**

---

## Phase 5.4: Capture All 3D Model References

**Outcome:** C# reference output for every 3D model  
**Verification:** `ls verification/csharp/*_3d_*.json | wc -l` equals 3D model count

### Tasks

1. **List all 3D models from models.xml**
   ```python
   # Parse models.xml for d="3" or d>1
   ```

2. **Run capture script on each**
   ```bash
   for model in $3D_MODELS; do
       python scripts/capture_csharp.py $model --seed 0
   done
   ```

3. **Log failures** (timeout, error, missing resources)

4. **Create manifest**
   ```
   verification/csharp/manifest.txt:
   Basic_seed0.json - OK
   Apartemazements_seed0.json - OK
   CarmaTower_seed0.json - FAILED: missing VOX
   ```

---

## Phase 5.5: Capture All Rust 3D Outputs

**Outcome:** Rust output for every 3D model we captured C# for  
**Verification:** Rust manifest matches C# manifest

### Tasks

1. **Run Rust capture on each model from manifest**

2. **Log failures separately**

3. **Create Rust manifest**

---

## Phase 5.6: Full Comparison Report

**Outcome:** Accuracy report for every 3D model  
**Verification:** `cat verification/reports/3d_accuracy.txt` shows per-model accuracy

### Tasks

1. **Run comparison on all models**

2. **Generate summary report**
   ```
   3D Model Accuracy Report
   ========================
   
   PERFECT (100%):
     ClosedSurface: 1728/1728 (100.00%)
     
   HIGH (>99%):
     ParallelGrowth: 24350/24389 (99.84%)
     
   PARTIAL (<99%):
     Apartemazements: 58000/64000 (90.63%)
     
   FAILED:
     CarmaTower: Could not load (missing VOX)
   ```

3. **Identify patterns**
   - Are failures random or systematic?
   - Do all WFC models fail? All with children?
   - Are failures at specific Z levels?

---

## Phase 5.7: Hypothesis-Driven Debugging

**Outcome:** Root cause identified for each failure category  
**Verification:** Each category has test + fix

### Process (per failure category)

1. **Select simplest failing model** in category

2. **Analyze diff pattern**
   - Where are differences? (coordinates)
   - What are the values? (expected vs actual)
   - Is there a pattern? (all Z>5, all near edges, etc.)

3. **Form hypothesis**
   - "3D rotation is wrong"
   - "Tile overlap calculation is off by 1"
   - "Child execution order differs"

4. **Create minimal test**
   ```rust
   #[test]
   fn test_hypothesis_tile_overlap() {
       // Minimal reproduction
       // Assert specific behavior
   }
   ```

5. **Fix and verify**
   - Fix passes the test
   - Re-run full comparison
   - Category accuracy improves

6. **Iterate** until 100%

---

## Directory Structure

```
scripts/
  run_csharp.sh              # Wrapper to run C# with args
  capture_csharp.py          # Capture C# output to JSON
  capture_rust.py            # Capture Rust output to JSON  
  compare.py                 # Cell-by-cell comparison
  run_all_comparisons.py     # Batch comparison script

verification/
  csharp/                    # C# reference outputs
    Basic_seed0.json
    Apartemazements_seed0.json
    manifest.txt
  rust/                      # Rust outputs
    Basic_seed0.json
    Apartemazements_seed0.json
    manifest.txt
  reports/                   # Comparison reports
    3d_accuracy.txt
    Apartemazements_diff.txt
```

---

## Success Criteria

### Phase 5.0 Complete When:
- [ ] `dotnet build` succeeds in MarkovJunior/
- [ ] Can run any model with specific seed
- [ ] Same seed produces identical output

### Phase 5.1 Complete When:
- [ ] `capture_csharp.py` produces JSON for Basic model
- [ ] JSON contains dimensions, characters, grid array

### Phase 5.2 Complete When:
- [ ] Rust produces matching JSON format
- [ ] Can capture any model we can load

### Phase 5.3 Complete When:
- [ ] `compare.py` reports accuracy percentage
- [ ] Lists coordinate-level differences

### Phase 5.4 Complete When:
- [ ] All loadable 3D models have C# reference
- [ ] Manifest documents status of each

### Phase 5.5 Complete When:
- [ ] All C#-captured models have Rust output
- [ ] Manifests match

### Phase 5.6 Complete When:
- [ ] Accuracy report exists for all models
- [ ] Failure patterns documented

### Phase 5.7 Complete When:
- [ ] Each failure category has hypothesis
- [ ] Tests exist for each hypothesis
- [ ] Fixes improve accuracy to 100% OR document known limitation

---

## Commands Reference

```bash
# Phase 5.0: Setup
cd MarkovJunior && dotnet build
dotnet run -- Basic 0

# Phase 5.1: Capture C#
python scripts/capture_csharp.py Basic --seed 0

# Phase 5.2: Capture Rust
cargo test -p studio_core capture_model -- Basic 0

# Phase 5.3: Compare
python scripts/compare.py Basic --seed 0

# Phase 5.6: Full report
python scripts/run_all_comparisons.py --3d-only
```
