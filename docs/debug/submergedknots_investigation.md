# SubmergedKnots Investigation

**Status:** IN PROGRESS
**Model:** SubmergedKnots.xml (3D)
**Match:** 68.52% (34817 differ out of 110592)
**Dimensions:** 48x48x48

## Model Structure

```xml
<sequence values="BW">
  <all in="B" out="W"/>
  <all in="***/***/*** ***/*W*/*** ***/***/***" out="***/***/*** ***/*B*/*** ***/***/***"/>
  <wfc values="BWU" tileset="Knots3D" tiles="Knots3D/4">
    <rule in="W" out="Empty"/>
    <sequence>
      <all in="B" out="U"/>
      <all in="U *" out="B *" symmetry="()"/>
      <one in="UB" out="UU" steps="42000"/>
    </sequence>
  </wfc>
</sequence>
```

## Diff Analysis

```
Dimensions: [48, 48, 48]
Total cells: 110592
Matching: 75775 (68.52%)
Different: 34817

Diff pattern analysis:
  C# values in diffs: {0}   -> all B
  Rust values in diffs: {2} -> all U
  First diff at index: 2346
```

**Key Observation:** 
- C# outputs `B` (value 0) where Rust outputs `U` (value 2)
- The WFC child sequence converts `B -> U` and then should convert some `U -> B` back
- Rust is leaving cells as `U` that C# has converted back to `B`

## Related Models Analysis

| Model | WFC Children | Status |
|-------|--------------|--------|
| Knots3D | None (just rule) | PASS |
| ColoredKnots | None | PASS |
| Knots2D | None | PASS |
| SelectLongKnots | `<markov><sequence>...</sequence></markov>` | PASS |
| SubmergedKnots | `<sequence>` directly | **FAIL (68.52%)** |

## Hypothesis

### Primary Hypothesis: Sequence as direct WFC child differs from Markov-wrapped

SelectLongKnots wraps its sequence in `<markov>`, while SubmergedKnots has `<sequence>` directly as WFC child.

**Potential issues:**
1. WFC child parsing treats `<sequence>` differently than `<markov>`
2. The `<sequence>` execution inside WFC differs from standalone
3. The `symmetry="()"` attribute on the second `<all>` node may not be parsed correctly

### Secondary Hypothesis: symmetry="()" attribute

The rule `<all in="U *" out="B *" symmetry="()"/>` has `symmetry="()"` which means identity-only (no rotations/reflections). This may not be handled correctly.

### Tertiary Hypothesis: steps="42000" on one node

The `<one in="UB" out="UU" steps="42000"/>` has a very large steps count. This may:
- Consume RNG differently
- Hit a loop limit
- Have different termination conditions

## Debug Plan

### Phase 1: Create simplified test models

#### Test 1: WFC with sequence child (no steps limit)
```xml
<sequence values="BW">
  <all in="B" out="W"/>
  <wfc values="BWU" tileset="Knots3D" tiles="Knots3D/4">
    <rule in="W" out="Empty"/>
    <sequence>
      <all in="B" out="U"/>
    </sequence>
  </wfc>
</sequence>
```

#### Test 2: WFC with sequence child (with B->U and U->B)
```xml
<sequence values="BW">
  <all in="B" out="W"/>
  <wfc values="BWU" tileset="Knots3D" tiles="Knots3D/4">
    <rule in="W" out="Empty"/>
    <sequence>
      <all in="B" out="U"/>
      <all in="U" out="B"/>
    </sequence>
  </wfc>
</sequence>
```

#### Test 3: WFC with markov-wrapped sequence (like SelectLongKnots)
```xml
<sequence values="BW">
  <all in="B" out="W"/>
  <wfc values="BWU" tileset="Knots3D" tiles="Knots3D/4">
    <rule in="W" out="Empty"/>
    <markov>
      <sequence>
        <all in="B" out="U"/>
        <all in="U" out="B"/>
      </sequence>
    </markov>
  </wfc>
</sequence>
```

### Phase 2: Add debug logging

Add identical logging to C# and Rust:
1. Log when WFC completes and children start executing
2. Log grid state before/after each child node
3. Log step counts for `<one>` nodes
4. Log symmetry handling for rules

### Phase 3: Binary search the divergence

1. Run Test 1 - if passes, sequence-as-child works for simple case
2. Run Test 2 - if fails, the U->B conversion differs
3. Run Test 3 - if passes, markov-wrapping changes behavior
4. Add the `symmetry="()"` attribute - if fails, symmetry parsing issue
5. Add the `steps="42000"` - if fails, steps handling issue

### Phase 4: Fix identified issue

Based on which test fails first, implement the fix.

## Key Files

### Rust
- `crates/studio_core/src/markov_junior/wfc/wfc_node.rs` - WFC base with child handling
- `crates/studio_core/src/markov_junior/wfc/tile_node.rs` - TileNode child execution
- `crates/studio_core/src/markov_junior/sequence_node.rs` - Sequence node
- `crates/studio_core/src/markov_junior/all_node.rs` - All node
- `crates/studio_core/src/markov_junior/one_node.rs` - One node
- `crates/studio_core/src/markov_junior/loader.rs` - Node parsing

### C#
- `MarkovJunior/source/WFCNode.cs` - WFC with children (extends Branch)
- `MarkovJunior/source/TileModel.cs` - Tile WFC
- `MarkovJunior/source/SequenceNode.cs` - Sequence node
- `MarkovJunior/source/RuleNode.cs` - All/One nodes

## Commands

```bash
# Run C# 
cd MarkovJunior && dotnet run -- --model SubmergedKnots --seed 42

# Run Rust
MJ_MODELS=SubmergedKnots MJ_SEED=42 cargo test -p studio_core batch_generate_outputs -- --ignored --nocapture

# Compare
python3 scripts/compare_grids.py MarkovJunior/verification/SubmergedKnots_seed42.json verification/rust/SubmergedKnots_seed42.json

# Verify
python3 scripts/batch_verify.py SubmergedKnots --regenerate

# Test related models
python3 scripts/batch_verify.py Knots3D ColoredKnots SelectLongKnots --regenerate
```

## Progress Log

### Initial Analysis
- SubmergedKnots at 68.52% match
- All diff cells: C#=B, Rust=U
- Related models (Knots3D, SelectLongKnots) pass
- Key difference: SubmergedKnots has `<sequence>` directly as WFC child

### Phase 1 Results: Test Model Isolation

Created 5 test models to isolate the issue:

| Model | Structure | Result |
|-------|-----------|--------|
| TestSubmergedL1 | WFC + sequence + `<all in="B" out="U"/>` | **PASS** |
| TestSubmergedL2 | WFC + sequence + B->U + U->B | **PASS** |
| TestSubmergedL3 | WFC + markov + sequence (wrapped) | TIMEOUT |
| TestSubmergedL4 | WFC + sequence + B->U + `U*->B*` with `symmetry="()"` | **PASS** |
| TestSubmergedL5 | WFC + sequence + B->U + `U*->B*` + `<one steps="100">` | **FAIL 99.86%** |

**Key Finding:** The `<one>` node with `steps` attribute causes divergence!

### TestSubmergedL5 Diff Analysis

```
Dimensions: [48, 48, 48]
Matching: 110432 (99.86%)
Different: 160

Diff pattern:
- C# values in diffs: {0, 2} (B and U)
- Rust values in diffs: {0, 2} (B and U)
- BIDIRECTIONAL: Some cells C#=B,Rust=U and others C#=U,Rust=B
```

This is different from SubmergedKnots which had ALL diffs as C#=B, Rust=U.

The bidirectional diffs suggest the `<one>` node is:
1. Making different random choices (RNG divergence)
2. Or executing a different number of steps
3. Or selecting different match locations

### Hypothesis Refined

The `<one in="UB" out="UU" steps="N">` node has a bug when:
- It's inside a WFC child sequence
- It has a limited `steps` count

Possible causes:
1. **Step counting differs** - Rust counts steps differently
2. **RNG consumption differs** - Rust uses RNG in different order
3. **Match finding differs** - Rust finds matches in different order

## Next Steps

1. Add debug logging to `<one>` node execution:
   - Log step count at start/end
   - Log RNG state before each step
   - Log which match is selected each step
2. Compare C# vs Rust logs
3. Find exact divergence point
4. Fix and verify
