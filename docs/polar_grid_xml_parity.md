# Polar Grid XML Parity Plan

## Executive Summary

The Polar Grid currently has a **custom programmatic API** for defining rules (`add_rule_str`), but **NO XML support**. This is a critical gap because:

1. XML enables **declarative model definition** - easier to author and share
2. XML supports **sequential composition** (`<sequence>`) - essential for complex models
3. XML is the **standard MJ format** - compatibility with existing models
4. Without XML, we can't express models like geological layers properly

**Current State**: Polar Grid is ~10% feature-complete relative to Cartesian XML.

---

## Feature Comparison Matrix

### Legend
- [x] Implemented
- [ ] Not implemented
- [~] Partially implemented

### 1. Core Node Types

| Feature | Cartesian | Polar | Notes |
|---------|-----------|-------|-------|
| `<one>` node | [x] | [ ] | Apply ONE random match per step |
| `<all>` node | [x] | [ ] | Apply ALL non-overlapping matches |
| `<prl>` node | [x] | [ ] | Apply ALL matches simultaneously (parallel) |
| `<sequence>` node | [x] | [ ] | **CRITICAL**: Sequential composition |
| `<markov>` node | [x] | [ ] | Markov chain looping |
| `<path>` node | [x] | [ ] | Pathfinding |
| `<map>` node | [x] | [ ] | Grid scaling/transformation |
| `<convolution>` node | [x] | [ ] | Cellular automata |
| `<convchain>` node | [x] | [ ] | MCMC texture synthesis |
| `<wfc>` node | [x] | [ ] | Wave Function Collapse |

### 2. XML Loading Infrastructure

| Feature | Cartesian | Polar | Notes |
|---------|-----------|-------|-------|
| XML parser | [x] | [ ] | quick_xml based |
| `load_model()` from file | [x] | [ ] | Load from .xml path |
| `load_model_str()` from string | [x] | [ ] | Load from inline XML |
| Attribute parsing | [x] | [ ] | HashMap<String, String> |
| Child element parsing | [x] | [ ] | Recursive node loading |
| Error handling | [x] | [ ] | LoadError enum |

### 3. Rule Definition

| Feature | Cartesian | Polar | Notes |
|---------|-----------|-------|-------|
| Inline rules (`in`/`out` attributes) | [x] | [~] | Polar has string DSL, not XML |
| Pattern syntax (rows/cols/layers) | [x] | [ ] | 2D: `/` separator, 3D: space |
| File-based rules (PNG/VOX) | [x] | [ ] | `file` attribute |
| Legend color mapping | [x] | [ ] | `legend` attribute |
| Wildcard `*` | [x] | [x] | Don't care / no change |
| Rule probability (`p`) | [x] | [ ] | Probabilistic application |
| Per-rule symmetry | [x] | [~] | Polar has global symmetry only |

### 4. Branch/Composition

| Feature | Cartesian | Polar | Notes |
|---------|-----------|-------|-------|
| Sequential execution | [x] | [ ] | **CRITICAL GAP** |
| Markov loop | [x] | [ ] | Loop until no progress |
| Nested branches | [x] | [ ] | Arbitrary nesting depth |
| Active child tracking | [x] | [ ] | `ip.current` simulation |

### 5. Symmetry System

| Feature | Cartesian | Polar | Notes |
|---------|-----------|-------|-------|
| Identity | [x] | [x] | No transform |
| Reflection | [x] | [x] | r_flip, theta_flip |
| Rotation | [x] | [~] | Polar has theta shift, not 90deg |
| Named symmetry groups | [x] | [ ] | `"(xy)"`, `"(x)(y)"`, etc. |
| Per-node symmetry override | [x] | [ ] | `symmetry` attribute |

### 6. Advanced Features

| Feature | Cartesian | Polar | Notes |
|---------|-----------|-------|-------|
| Heuristic fields (`<field>`) | [x] | [ ] | Distance-guided selection |
| Observations (`<observe>`) | [x] | [ ] | Goal-directed generation |
| Backtracking search | [x] | [ ] | `search="True"` |
| Step limits | [x] | [ ] | `steps` attribute |
| Temperature | [x] | [ ] | Probabilistic vs greedy |
| Union types (`<union>`) | [x] | [ ] | Combined wave masks |
| Origin flag | [x] | [ ] | Seed center cell |

### 7. Execution Model

| Feature | Cartesian | Polar | Notes |
|---------|-----------|-------|-------|
| Node trait | [x] | [ ] | `go()`, `reset()`, `is_branch()` |
| ExecutionContext | [x] | [ ] | Grid, RNG, changes, counter |
| Interpreter | [x] | [ ] | Main execution loop |
| Animated mode | [x] | [ ] | `gif` flag for frame-by-frame |
| Change tracking | [x] | [ ] | Incremental match updates |
| Deterministic RNG | [x] | [x] | Seeded pseudo-random |

### 8. Integration

| Feature | Cartesian | Polar | Notes |
|---------|-----------|-------|-------|
| Recording system | [x] | [x] | SimulationRecorder |
| Video export | [x] | [x] | VideoExporter |
| PNG rendering | [x] | [x] | Grid visualization |
| 3D voxel export | [x] | [ ] | Polar needs 3D extension |

---

## Current Polar Grid Capabilities

### What EXISTS:
```rust
// 1. Grid data structure
PolarMjGrid { r_min, r_depth, theta_divisions, rings }

// 2. Neighbor lookup
PolarNeighbors { theta_minus, theta_plus, r_minus, r_plus }

// 3. Pattern matching (single cell + 4 neighbors)
PolarPattern { center, theta_minus, theta_plus, r_minus, r_plus }

// 4. Rules (pattern -> output)
PolarRule { input: PolarPattern, output: u8 }

// 5. Symmetry (4 variants)
PolarSymmetry::Identity, RFlip, ThetaFlip, BothFlip

// 6. Model (holds grid + rules)
PolarModel { grid, rules, seed, step }

// 7. String-based rule DSL
model.add_rule_str("B;*,*,M,* -> M", true)
```

### What's MISSING for XML Parity:

1. **XML Loader** - No `load_polar_model()` or `load_polar_model_str()`
2. **Node Trait** - No `PolarNode` trait with `go()`/`reset()`
3. **Branch Nodes** - No `PolarSequenceNode` or `PolarMarkovNode`
4. **Rule Nodes** - No `PolarOneNode`, `PolarAllNode`, `PolarParallelNode`
5. **Interpreter** - No `PolarInterpreter` to run models
6. **ExecutionContext** - No shared execution state
7. **Multi-cell patterns** - Only 1x1+neighbors, not NxN regions

---

## Why the Geological Model Failed

The geological layers model I created was "dog shit" because:

1. **No Sequential Composition**: I couldn't say "first do X, then do Y"
2. **No Phases**: MJ geological models use sequences like:
   ```xml
   <sequence>
     <one>fill magma core</one>
     <one>cool magma to stone</one>
     <one>weather stone to dirt</one>
     <one>grow grass on dirt</one>
   </sequence>
   ```
3. **Flat Rule List**: All rules competed simultaneously instead of executing in order
4. **No Step Limits**: Couldn't control when phases transition

With XML support, the geological model would look like:
```xml
<sequence values="BMSDG" origin="True">
  <!-- Phase 1: Fill core with magma -->
  <one in="B" out="M" steps="500"/>
  
  <!-- Phase 2: Cool outer magma to stone -->
  <all in="M" out="S" steps="1"/>
  <one in="B;*,*,S,*" out="S" steps="200"/>
  
  <!-- Phase 3: Weather stone to dirt -->
  <all in="S" out="D" steps="1"/>
  <one in="B;*,*,D,*" out="D" steps="150"/>
  
  <!-- Phase 4: Grow grass on surface -->
  <all in="D" out="G" steps="1"/>
  <one in="B;*,*,G,*" out="G"/>
</sequence>
```

---

## Implementation Roadmap

### Phase 1: Core Infrastructure (Foundation)
**Priority: CRITICAL**
**Estimated: 2-3 days**

| Task | Description | Dependencies |
|------|-------------|--------------|
| 1.1 | Define `PolarNode` trait | None |
| 1.2 | Create `PolarExecutionContext` | 1.1 |
| 1.3 | Create `PolarInterpreter` | 1.1, 1.2 |
| 1.4 | Implement basic XML parser structure | None |

### Phase 2: Branch Nodes (Sequential Composition)
**Priority: CRITICAL**
**Estimated: 1-2 days**

| Task | Description | Dependencies |
|------|-------------|--------------|
| 2.1 | Implement `PolarSequenceNode` | 1.1 |
| 2.2 | Implement `PolarMarkovNode` | 1.1 |
| 2.3 | Test nested branches | 2.1, 2.2 |

### Phase 3: Rule Nodes (Core Functionality)
**Priority: HIGH**
**Estimated: 2-3 days**

| Task | Description | Dependencies |
|------|-------------|--------------|
| 3.1 | Create `PolarRuleNodeData` (shared state) | 1.2 |
| 3.2 | Implement `PolarOneNode` | 3.1 |
| 3.3 | Implement `PolarAllNode` | 3.1 |
| 3.4 | Implement `PolarParallelNode` | 3.1 |
| 3.5 | Add step limits (`steps` attribute) | 3.1-3.4 |

### Phase 4: XML Loading
**Priority: HIGH**
**Estimated: 2 days**

| Task | Description | Dependencies |
|------|-------------|--------------|
| 4.1 | Create `PolarLoadedModel` struct | 1.3, 2.1-2.2, 3.2-3.4 |
| 4.2 | Implement `load_polar_model_str()` | 4.1 |
| 4.3 | Implement `load_polar_model()` from file | 4.2 |
| 4.4 | Parse root attributes (values, origin) | 4.2 |
| 4.5 | Parse rule nodes with attributes | 4.2 |

### Phase 5: Pattern System Enhancement
**Priority: MEDIUM**
**Estimated: 2-3 days**

| Task | Description | Dependencies |
|------|-------------|--------------|
| 5.1 | Design multi-cell polar patterns | None |
| 5.2 | Implement inline pattern parsing | 5.1 |
| 5.3 | Add wildcard support in patterns | 5.2 |
| 5.4 | Add rule probability (`p` attribute) | 3.1 |

### Phase 6: Advanced Features
**Priority: MEDIUM**
**Estimated: 3-4 days**

| Task | Description | Dependencies |
|------|-------------|--------------|
| 6.1 | Implement `<union>` element | 4.2 |
| 6.2 | Add temperature parameter | 3.1 |
| 6.3 | Implement heuristic fields | 3.1 |
| 6.4 | Add observations/search | 6.3 |

### Phase 7: Additional Nodes
**Priority: LOW**
**Estimated: 4-5 days**

| Task | Description | Dependencies |
|------|-------------|--------------|
| 7.1 | `PolarPathNode` (polar pathfinding) | 1.1 |
| 7.2 | `PolarConvolutionNode` | 1.1 |
| 7.3 | `PolarMapNode` (if applicable) | 1.1 |

---

## Work Item Breakdown

### Sprint 1: Foundation (Week 1)
**Goal**: Basic XML loading with sequence/markov + one node

```
[ ] 1. Create polar_loader.rs with PolarLoadedModel
[ ] 2. Define PolarNode trait (go, reset, is_branch)
[ ] 3. Create PolarExecutionContext
[ ] 4. Implement PolarSequenceNode
[ ] 5. Implement PolarMarkovNode
[ ] 6. Implement PolarOneNode (basic, no heuristics)
[ ] 7. Create PolarInterpreter
[ ] 8. Implement load_polar_model_str() - basic parsing
[ ] 9. Test: Simple sequential model in XML
[ ] 10. Test: Markov loop model in XML
```

### Sprint 2: Core Completion (Week 2)
**Goal**: Full rule node suite + step limits

```
[ ] 11. Implement PolarAllNode
[ ] 12. Implement PolarParallelNode  
[ ] 13. Add `steps` attribute to all rule nodes
[ ] 14. Add `origin` attribute support
[ ] 15. Add `symmetry` attribute support
[ ] 16. Implement <union> element
[ ] 17. Test: Geological layers model in XML
[ ] 18. Test: Complex nested sequence/markov
```

### Sprint 3: Polish & Advanced (Week 3)
**Goal**: Feature parity for common use cases

```
[ ] 19. Add rule probability (`p` attribute)
[ ] 20. Add temperature parameter
[ ] 21. Implement heuristic fields
[ ] 22. Add file-based rule loading (if applicable)
[ ] 23. Integration with recording system
[ ] 24. Documentation and examples
[ ] 25. Comprehensive test suite
```

---

## Success Criteria

### Minimum Viable XML Support
The following model MUST work:

```xml
<sequence values="BMSDG" origin="True" polar="True">
  <one in="B;*,*,*,*" out="M" steps="100"/>
  <one in="B;*,*,M,*" out="M" steps="400"/>
  <one in="M;*,*,*,B" out="S"/>
  <one in="B;*,*,S,*" out="S" steps="300"/>
  <one in="S;*,*,*,B" out="D"/>
  <one in="B;*,*,D,*" out="D" steps="200"/>
  <one in="D;*,*,*,B" out="G"/>
  <one in="B;*,*,G,*" out="G"/>
</sequence>
```

### Full Parity Checklist
- [ ] Can load any valid polar XML model
- [ ] Sequence and Markov nodes work correctly
- [ ] One, All, Prl nodes work correctly
- [ ] Step limits work correctly
- [ ] Origin flag works correctly
- [ ] Symmetry attribute works correctly
- [ ] Recording and video export work
- [ ] At least 3 example models validated

---

## Open Questions

1. **Pattern Format**: Should polar patterns use the same `/` syntax or a polar-specific format?
   - Option A: `"B;*,*,M,*"` (current DSL)
   - Option B: `"B/M"` with implicit neighbor meaning
   - Option C: Custom polar-aware format

2. **Multi-cell Patterns**: Do we need NxN patterns in polar coordinates?
   - Cartesian: `"BWB/WBW/BWB"` (3x3)
   - Polar: How to represent radial extent?

3. **Shared Loader vs. Separate**: Should we extend the existing loader or create a new one?
   - Extend: Add `polar="True"` attribute to existing loader
   - Separate: Create `polar_loader.rs` with clean implementation

4. **3D Polar**: Do we eventually need spherical coordinates?
   - Current: 2D polar only
   - Future: May need `PolarMjGrid3D` with φ (phi) dimension

---

## Appendix: Reference XML Models

### A. Simple Growth (Cartesian)
```xml
<one values="BW" origin="True" in="BW" out="WW"/>
```

### B. Maze Backtracker (Cartesian)
```xml
<markov values="BRGW" origin="True">
  <one in="RBB" out="GGR"/>
  <one in="RGG" out="WWR"/>
</markov>
```

### C. Dungeon Growth (Cartesian)
```xml
<sequence values="BRACDG" origin="True">
  <union symbol="?" values="BR"/>
  <one in="**?**/*BBB*/*BBB?/*BBB*/**R**" out="AARAA/ADDDA/ADDDR/ADDDA/AACAA"/>
  <one in="ACA/BBB" out="ARA/BBB"/>
  <all>
    <rule in="C" out="D"/>
    <rule in="R" out="D"/>
  </all>
  <all in="BD" out="*A"/>
</sequence>
```

### D. Target: Geological Layers (Polar)
```xml
<sequence values="BMSDG" origin="True" polar="True">
  <one in="B" out="M" steps="100"/>
  <one in="B;*,*,M,*" out="M" steps="400"/>
  <markov>
    <one in="M;*,*,*,B" out="S"/>
    <one in="B;*,*,S,*" out="S"/>
  </markov>
  <markov>
    <one in="S;*,*,*,B" out="D"/>
    <one in="B;*,*,D,*" out="D"/>
  </markov>
  <markov>
    <one in="D;*,*,*,B" out="G"/>
    <one in="B;*,*,G,*" out="G"/>
  </markov>
</sequence>
```
