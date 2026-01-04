# MarkovJunior Rust Port - Deviation Log

This document tracks ALL deviations from the original C# MarkovJunior implementation.
Each deviation must be reviewed and either:
1. Fixed to match C# exactly
2. Documented with justification for why it's acceptable

## Phase 1: Foundation Data Structures

### Grid.cs → mod.rs (MjGrid)

#### MISSING FIELDS

| C# Field | Rust Status | Impact | Priority |
|----------|-------------|--------|----------|
| `bool[] mask` | **IMPLEMENTED** | Used by AllNode for conflict tracking | DONE |
| `byte[] statebuffer` | **MISSING** | Double-buffer for State() method | LOW - State() is commented out in C# |
| `int transparent` | **MISSING** | Transparency mask for rendering | MEDIUM - needed for proper output |
| `string folder` | **MISSING** | Resource folder path | LOW - only for file loading |

#### MISSING METHODS

| C# Method | Rust Status | Impact |
|-----------|-------------|--------|
| `Grid.Load(XElement, ...)` | **MISSING** | XML loading - deferred to Phase 1.4 |
| `State()` | **MISSING** | Commented out in C# - skip |

#### TYPE DIFFERENCES

| Field | C# Type | Rust Type | Issue |
|-------|---------|-----------|-------|
| `waves` | `Dictionary<char, int>` | `HashMap<char, u32>` | C# uses `int` (32-bit signed), Rust uses `u32`. Should be fine but may cause issues with >31 colors |
| `MX/MY/MZ` | `int` | `usize` | Rust uses unsigned. C# allows negative which shouldn't happen but may cause subtle bugs |

#### BEHAVIORAL DIFFERENCES

1. **Grid.Wave() implementation differs slightly:**
   - C#: `sum += 1 << this.values[values[k]]` (uses addition)
   - Rust: `sum |= 1 << idx` (uses bitwise OR)
   - **Impact:** Functionally equivalent for non-overlapping bits, but C# would produce wrong results for duplicate chars while Rust handles correctly. This is actually a Rust IMPROVEMENT.

2. **Duplicate character handling in with_values():**
   - C#: Returns null with error message if duplicate found
   - Rust: **FIXED** - `try_with_values()` returns `Result<Self, GridError>`
   - **Impact:** Now matches C# behavior.

3. **Union types not implemented:**
   - C# Grid.Load() parses `<union>` elements to create composite wave types
   - Rust: Only `*` wildcard is added
   - **Impact:** Some models use custom unions. MUST ADD in Phase 1.4.

4. **matches() bounds checking:**
   - C#: No explicit bounds check - relies on caller
   - Rust: Added bounds checking that returns false
   - **Impact:** Rust is SAFER but may mask bugs that C# would crash on.

---

### Rule.cs → rule.rs (MjRule)

#### MISSING FIELDS

| C# Field | Rust Status | Impact | Priority |
|----------|-------------|--------|----------|
| `byte[] binput` | **IMPLEMENTED** | Compact input for fast comparison | DONE |
| `(int,int,int)[][] ishifts` | **IMPLEMENTED** | Precomputed positions per color | DONE |
| `(int,int,int)[][] oshifts` | **IMPLEMENTED** | Precomputed output positions | DONE |
| `bool original` | **MISSING** | Marks if rule is original vs symmetry variant | LOW - informational |

#### MISSING METHODS

| C# Method | Rust Status | Impact |
|-----------|-------------|--------|
| `Rule.Load(XElement, ...)` | **MISSING** | XML loading - deferred to Phase 1.4 |
| `Rule.LoadResource(...)` | **MISSING** | PNG/VOX loading - deferred to Phase 1.4 |
| `YRotated()` | **MISSING** | 3D rotation around Y axis | HIGH - needed for 3D models |
| `Symmetries(bool[], bool)` | **MISSING** | Wrapper method on Rule | LOW - can call symmetry module directly |

#### CONSTRUCTOR DIFFERENCES

1. **ishifts/oshifts:**
   - C# constructor builds precomputed lookup tables for which positions match each color
   - Rust: **IMPLEMENTED** - computed in `from_patterns()` and `parse()`
   - **Impact:** Now matches C# behavior.

2. **binput:**
   - C# computes `binput` which stores single-value inputs as bytes (0xff for wildcards)
   - Rust: **IMPLEMENTED** - computed in `from_patterns()` and `parse()`
   - **Impact:** Now matches C# behavior.

#### PATTERN PARSING DIFFERENCES

1. **Z-axis ordering:**
   - C#: `linesz = lines[MZ - 1 - z]` - Z layers are reversed
   - Rust: **FIXED** - Z layers now reversed to match C#
   - **Impact:** Now matches C# behavior.

2. **Helper.Split() not replicated:**
   - C# uses custom `Helper.Split(s, ' ', '/')` for nested splitting
   - Rust: Uses sequential split calls
   - **Impact:** Should be equivalent but needs verification with complex patterns.

---

### SymmetryHelper.cs → symmetry.rs

#### MISSING FUNCTIONALITY

| C# Feature | Rust Status | Impact | Priority |
|------------|-------------|--------|----------|
| `CubeSymmetries()` | **MISSING** | 48-element 3D symmetry group | HIGH - needed for 3D models |
| `cubeSubgroups` dictionary | **MISSING** | Predefined 3D subgroups | HIGH - needed for 3D |
| `GetSymmetry(bool d2, string, bool[])` | **MISSING** | Lookup by name with fallback | LOW - convenience |

#### IMPLEMENTATION DIFFERENCES

1. **Subgroup definitions may differ:**
   - Need to verify that `SquareSubgroup` masks match C# `squareSubgroups` exactly
   - C# `(x)(y)`: `[true, true, false, false, true, true, false, false]`
   - Rust `ReflectXY`: `[true, true, false, false, true, true, false, false]`
   - **Status:** MATCHES

2. **Generic vs concrete:**
   - C#: `SquareSymmetries<T>` is generic, takes function pointers
   - Rust: `square_symmetries` only works with `MjRule`
   - **Impact:** Less flexible but simpler. OK for now.

---

## Summary: Critical Issues Status

### Fixed in Phase 1 (Post-Audit)

1. **~~Add `mask: Vec<bool>` to MjGrid~~** - FIXED
2. **~~Add `ishifts`/`oshifts` to MjRule~~** - FIXED (also added `binput`)
3. **~~Fix Z-axis reversal in pattern parsing~~** - FIXED
4. **~~Add duplicate character detection~~** - FIXED (`try_with_values()` returns Result)

### Must Fix Before Phase 1.4 (XML Loading)

5. **Add union type support** - parse `<union>` elements into waves
6. **Implement `YRotated()`** - needed for 3D symmetries
7. **Implement `CubeSymmetries()`** - needed for 3D models

### Nice to Have (Can Defer)

8. Add `transparent` field for rendering
9. Add `folder` for resource paths
10. Add `original` flag for debugging

---

## Verification Needed

Before each phase is truly complete, run these checks:

### Cross-validation Test
```bash
# Generate C# reference output
cd MarkovJunior
dotnet run -- Basic 12345 --dump-state /tmp/basic_csharp.bin

# Run Rust with same seed
cargo test test_basic_matches_reference
```

### Pattern Parsing Test
Create test that parses "AB/CD EF/GH" and verifies exact byte positions match C# output.

---

## Change Log

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1 | Used u32 for waves instead of int | Rust idiom, functionally equivalent |
| 2026-01-04 | 1 | Used usize for dimensions | Rust idiom for array indexing |
| 2026-01-04 | 1 | Added bounds checking to matches() | Safety improvement |
| 2026-01-04 | 1 | Skipped CubeSymmetries | Deferred - 2D first |
| 2026-01-04 | 1 | **FIXED** Z reversal in pattern parsing | Was a bug, now matches C# |
| 2026-01-04 | 1 | **ADDED** mask field to MjGrid | Needed for AllNode |
| 2026-01-04 | 1 | **ADDED** ishifts/oshifts/binput to MjRule | Needed for incremental matching |
| 2026-01-04 | 1 | **ADDED** duplicate character detection | Matches C# error handling |

---

## Phase 1.2: Node Infrastructure

### Node.cs → node.rs

#### ARCHITECTURAL DIFFERENCES

1. **Node trait instead of abstract class:**
   - C#: `abstract class Node` with `Interpreter ip` and `Grid grid` fields
   - Rust: `trait Node` with `go()` and `reset()` methods; state passed via `ExecutionContext`
   - **Impact:** More idiomatic Rust. ExecutionContext carries grid/rng/changes instead of storing references.
   - **Justification:** Rust ownership model doesn't allow storing mutable references in structs easily.

2. **ExecutionContext pattern:**
   - C#: Nodes hold `Interpreter ip` reference, access `ip.grid`, `ip.random`, `ip.changes`
   - Rust: `ExecutionContext<'a>` passed to every `go()` call, contains `&mut grid`, `&mut random`, changes vec
   - **Impact:** Equivalent functionality with different ownership semantics.

3. **Branch.parent not implemented:**
   - C#: `Branch` has `parent` field for hierarchical navigation (`ip.current = ip.current.parent`)
   - Rust: Not implemented - nodes don't track parent
   - **Impact:** May need for `MapNode`/`WFCNode` in later phases. Defer until needed.

#### MISSING NODE TYPES

| C# Node | Rust Status | Impact | Priority |
|---------|-------------|--------|----------|
| `PathNode` | **MISSING** | Dijkstra pathfinding | Phase 1.5 |
| `MapNode` | **MISSING** | Grid transformation | Phase 1.5 |
| `ConvolutionNode` | **MISSING** | Cellular automata | Phase 1.7 |
| `ConvChainNode` | **MISSING** | MCMC texture synthesis | Phase 1.7 |
| `OverlapNode` | **MISSING** | Overlapping WFC | Phase 1.6 |
| `TileNode` | **MISSING** | Tile-based WFC | Phase 1.6 |

---

### RuleNode.cs → rule_node.rs

#### STRUCTURAL DIFFERENCES

1. **Composition instead of inheritance:**
   - C#: `OneNode : RuleNode`, `AllNode : RuleNode` inheritance
   - Rust: `OneNode { data: RuleNodeData }`, `AllNode { data: RuleNodeData }` composition
   - **Impact:** Same functionality, more idiomatic Rust.

2. **Match scanning simplified:**
   - C#: Uses stride optimization `for z in (rule.IMZ - 1)..MZ step rule.IMZ`
   - Rust: Simple full scan `for z in 0..=(mz - rule.imz)`
   - **Impact:** Rust is O(grid_size * rules) instead of O(grid_size / rule_size * rules). Simpler, correct, slightly slower.
   - **Justification:** Correctness over premature optimization. Can add stride optimization later if needed.

#### MISSING FEATURES

| C# Feature | Rust Status | Impact | Priority |
|------------|-------------|--------|----------|
| `potentials` field | **MISSING** | Heuristic field guidance | Phase 1.5 |
| `fields` array | **MISSING** | Distance field computation | Phase 1.5 |
| `observations` | **MISSING** | Future constraint propagation | Phase 1.5 |
| `temperature` | **MISSING** | Randomized selection weighting | Phase 1.5 |
| `search` flag | **MISSING** | A* search mode | Phase 1.5 |
| `trajectory` | **MISSING** | Cached search path | Phase 1.5 |
| `steps` limit | **IMPLEMENTED** | Max iterations per node | DONE |
| `last` array | **IMPLEMENTED** | Track which rules applied | DONE |

---

### OneNode.cs → one_node.rs

#### BEHAVIORAL DIFFERENCES

1. **RandomMatch simplified:**
   - C#: Has two modes - with potentials (heuristic selection) and without (random)
   - Rust: Only implements random mode (no potentials)
   - **Impact:** Cannot use field-guided selection yet. Add in Phase 1.5.

2. **Trajectory mode not implemented:**
   - C#: If `trajectory != null`, replays pre-computed state sequence
   - Rust: Not implemented
   - **Impact:** Search/replay feature unavailable. Add in Phase 1.5.

---

### AllNode.cs → all_node.rs

#### BEHAVIORAL DIFFERENCES

1. **Potentials/heuristic mode not implemented:**
   - C#: With potentials, sorts matches by heuristic score before applying
   - Rust: Always shuffles randomly
   - **Impact:** Cannot use field-guided ordering. Add in Phase 1.5.

2. **mask clearing behavior matches C#:**
   - Both clear mask for changed cells after applying all matches
   - **Status:** MATCHES

---

### ParallelNode.cs → parallel_node.rs

#### BEHAVIORAL DIFFERENCES

1. **Double-buffer approach matches C#:**
   - Both write to `newstate` buffer, then copy back to `grid.state`
   - **Status:** MATCHES

2. **Rule probability (rule.p) implemented:**
   - C#: `if (ip.random.NextDouble() > rule.p) return;`
   - Rust: `if ctx.random.gen::<f64>() > rule.p { return false; }`
   - **Status:** MATCHES

---

## Change Log (Phase 1.2)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1.2 | Used trait instead of abstract class for Node | Rust idiom |
| 2026-01-04 | 1.2 | Created ExecutionContext pattern | Rust ownership requirements |
| 2026-01-04 | 1.2 | Used composition for RuleNodeData | Rust idiom over inheritance |
| 2026-01-04 | 1.2 | Simplified match scanning (no stride) | Correctness over optimization |
| 2026-01-04 | 1.2 | Skipped potentials/fields/observations | Deferred to Phase 1.5 |
| 2026-01-04 | 1.2 | Skipped trajectory/search | Deferred to Phase 1.5 |
| 2026-01-04 | 1.2 | Skipped Branch.parent | Not needed for basic nodes |

---

## Phase 1.3: Interpreter & Execution

### Interpreter.cs → interpreter.rs

#### STRUCTURAL DIFFERENCES

1. **No `startgrid` clone:**
   - C#: Stores `startgrid` separately, restores on reset: `grid = startgrid`
   - Rust: Uses `grid.clear()` to reset instead of cloning
   - **Impact:** Cannot restore to a pre-populated initial state. If needed, caller must repopulate.
   - **Justification:** Avoids expensive clone operation. Most models start from cleared state anyway.

2. **No `current` pointer:**
   - C#: `Branch current` tracks which node is currently executing, updated by `Branch.Go()`
   - Rust: Nodes handle their own completion state internally (SequenceNode/MarkovNode track `n`)
   - **Impact:** Interpreter just calls `root.go()` and checks return value.
   - **Justification:** Trait-based design is simpler. Nodes are self-contained.

3. **Added `running` flag:**
   - C#: Uses `current != null` to track if model is still running
   - Rust: Added explicit `running: bool` field
   - **Impact:** Equivalent semantics with clearer intent.

4. **ExecutionContext created per-step:**
   - C#: Interpreter owns all state, nodes access via `ip.grid`, `ip.random`
   - Rust: `ExecutionContext` created each step, passed to `root.go()`
   - **Impact:** Small overhead from moving/restoring vectors. Could optimize later.
   - **Justification:** Rust ownership requires this pattern for safe mutable access.

5. **No `gif` flag:**
   - C#: `gif` flag and IEnumerable yield for animation frames
   - Rust: Use `step()` method for animation control instead
   - **Impact:** Same functionality, different API style.

#### API DIFFERENCES

| C# Method | Rust Method | Notes |
|-----------|-------------|-------|
| `Run(seed, steps, gif)` -> IEnumerable | `run(seed, max_steps)` -> usize | Returns step count, not iterator |
| `current.Go()` in loop | `step()` method | Caller can observe state each step |
| N/A | `is_running()` | Explicit running check |
| N/A | `changes()` | Access to change history |

#### BEHAVIORAL MATCHES

1. **Origin calculation:**
   - C#: `grid.state[grid.MX / 2 + (grid.MY / 2) * grid.MX + (grid.MZ / 2) * grid.MX * grid.MY] = 1`
   - Rust: Same formula in `reset()` when `origin = true`
   - **Status:** MATCHES

2. **Counter increment:**
   - C#: `counter++; first.Add(changes.Count);`
   - Rust: Same logic in `step()` after successful `root.go()`
   - **Status:** MATCHES

3. **Reset behavior:**
   - C#: Clears grid, resets changes/first, calls `root.Reset()`
   - Rust: Same sequence in `reset()`
   - **Status:** MATCHES

---

## Change Log (Phase 1.3)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1.3 | No startgrid clone, use clear() | Avoids expensive clone |
| 2026-01-04 | 1.3 | No current pointer, nodes self-contained | Simpler trait-based design |
| 2026-01-04 | 1.3 | Added running flag instead of null check | Clearer Rust semantics |
| 2026-01-04 | 1.3 | ExecutionContext per-step | Rust ownership requirements |
| 2026-01-04 | 1.3 | No IEnumerable, use step() for animation | Rust API style |

---

## Phase 1.4: Model Loading (XML)

### Interpreter.Load() → loader.rs

#### ARCHITECTURAL DIFFERENCES

1. **Separate loader module:**
   - C#: `Interpreter.Load()` is a static method on Interpreter
   - Rust: Separate `loader.rs` module with `load_model()` and `load_model_str()` functions
   - **Impact:** Cleaner separation of concerns.

2. **Model wrapper:**
   - C#: Loading returns `Interpreter` directly
   - Rust: `Model` struct wraps `Interpreter` with convenience methods
   - **Impact:** Nicer API for common use cases.

3. **No models.xml parsing:**
   - C#: Reads `models.xml` for model dimensions and settings
   - Rust: Dimensions passed as arguments (default 16x16x1)
   - **Impact:** User must specify dimensions. Add models.xml support in future if needed.

#### MISSING FEATURES

| C# Feature | Rust Status | Impact | Priority |
|------------|-------------|--------|----------|
| `<union>` parsing | **MISSING** | Custom union types not supported | MEDIUM |
| `file` attribute | **MISSING** | PNG/VOX rule loading | MEDIUM |
| `fin`/`fout` attributes | **MISSING** | File-based rule patterns | LOW |
| `legend` attribute | **MISSING** | For file-based rules | LOW |
| `folder` attribute | **MISSING** | Resource path | LOW |
| `transparent` attribute | **MISSING** | Rendering hints | LOW |
| Line number in errors | **MISSING** | XML error location | LOW |

#### NODE LOADING DIFFERENCES

1. **Node.Factory pattern:**
   - C#: Static factory method creates and initializes nodes
   - Rust: `load_node_from_xml()` function with pattern matching
   - **Impact:** Same functionality, different pattern.

2. **Parent tracking:**
   - C#: Sets `branch.parent` for hierarchical navigation
   - Rust: Not implemented - nodes are self-contained
   - **Impact:** Some advanced features may need this later.

3. **Unsupported node types:**
   - `path`, `map`, `convolution`, `convchain`, `wfc` (overlap/tile)
   - Will return `UnknownNodeType` error if encountered
   - **Impact:** Many models won't load yet. Add in Phase 1.5+.

#### SYMMETRY HANDLING

1. **Subgroup conversion:**
   - C#: Uses `bool[8]` or `bool[48]` arrays directly
   - Rust: Converts to `SquareSubgroup` enum for `square_symmetries()`
   - **Impact:** Same functionality, different representation.

2. **3D symmetries deferred:**
   - Only 2D square symmetries fully implemented
   - 3D models return single rule (no symmetry expansion)
   - **Impact:** 3D models won't work correctly yet.

#### BEHAVIORAL MATCHES

1. **Rule parsing:**
   - Both parse `in`/`out` attributes
   - Both support inline rules on node elements
   - Both support `<rule>` child elements
   - **Status:** MATCHES

2. **Symmetry inheritance:**
   - Child nodes inherit parent symmetry if not overridden
   - Rule-specific symmetry overrides node symmetry
   - **Status:** MATCHES

3. **Origin flag:**
   - Parsed from `origin="True"` attribute
   - Passed to Interpreter for center cell initialization
   - **Status:** MATCHES

---

## Change Log (Phase 1.4)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1.4 | Created separate loader.rs module | Cleaner architecture |
| 2026-01-04 | 1.4 | Created Model wrapper struct | Better API |
| 2026-01-04 | 1.4 | Skipped models.xml parsing | User specifies dimensions |
| 2026-01-04 | 1.4 | Skipped union type support | Most models don't need it |
| 2026-01-04 | 1.4 | Skipped file attribute | Focus on inline rules first |
| 2026-01-04 | 1.4 | Skipped path/map/wfc nodes | Add in later phases |
| 2026-01-04 | 1.4 | Deferred 3D symmetries | Focus on 2D first |

---

## Phase 1.5: Field, Path, and Heuristic Selection

### Field.cs -> field.rs

#### STRUCTURAL MATCHES

1. **Field struct:**
   - C# fields: `recompute`, `inversed`, `essential`, `substrate`, `zero`
   - Rust: Same fields with same semantics
   - **Status:** MATCHES

2. **BFS algorithm:**
   - C# uses Queue<(int, int, int, int)> for BFS
   - Rust uses VecDeque<(i32, i32, i32, i32)>
   - Same algorithm: initialize targets with potential=0, BFS through substrate
   - **Status:** MATCHES

3. **DeltaPointwise:**
   - C# signature: `int? DeltaPointwise(byte[] state, Rule rule, int x, int y, int z, Field[] fields, int[][] potentials, int MX, int MY)`
   - Rust signature: `fn delta_pointwise(...) -> Option<i32>`
   - Same algorithm: sum potential differences for each cell changed by rule
   - **Status:** MATCHES

#### BEHAVIORAL MATCHES

1. **Inversed field handling:**
   - Both add/subtract 2*potential for inversed fields
   - **Status:** MATCHES

---

### Path.cs -> path_node.rs

#### STRUCTURAL MATCHES

1. **PathNode struct:**
   - C# fields: `start`, `finish`, `substrate`, `value`, `inertia`, `longest`, `edges`, `vertices`
   - Rust: Same fields with same semantics
   - **Status:** MATCHES

2. **Pathfinding algorithm:**
   - BFS from finish positions to compute generation distances
   - Trace back from start to finish following decreasing generations
   - **Status:** MATCHES

3. **Direction selection with inertia:**
   - Cardinal: prefer current direction if valid, else random
   - With edges/vertices: use cosine similarity scoring
   - **Status:** MATCHES

#### MINOR DIFFERENCES

1. **Local random seeding:**
   - C#: `new Random(ip.random.Next())`
   - Rust: `StdRng::seed_from_u64(ctx.random.gen::<u64>())`
   - **Impact:** Different random sequences, but statistically equivalent

---

### RuleNode.cs -> rule_node.rs (Updates)

#### NEW FIELDS ADDED

1. **potentials: Option<Vec<Vec<i32>>>**
   - C#: `int[][] potentials`
   - Rust: `Option<Vec<Vec<i32>>>`
   - `Option` wrapper because most nodes don't use fields
   - **Impact:** Equivalent functionality

2. **fields: Option<Vec<Option<Field>>>**
   - C#: `Field[] fields`
   - Rust: `Option<Vec<Option<Field>>>`
   - Double Option: outer for "has fields", inner for "field for this color"
   - **Impact:** Equivalent functionality

3. **temperature: f64**
   - C#: `double temperature`
   - **Status:** MATCHES

#### FIELD RECOMPUTATION

- C# recomputes fields in `RuleNode.Go()` lines 200-215
- Rust recomputes in `RuleNodeData.compute_matches()`
- Same logic: recompute if counter==0 or field.recompute==true
- **Status:** MATCHES

---

### OneNode.cs -> one_node.rs (Updates)

#### HEURISTIC SELECTION

1. **Two-mode RandomMatch:**
   - C# lines 75-139 has two branches: with potentials and without
   - Rust: `random_match_heuristic()` and `random_match_simple()`
   - **Status:** MATCHES

2. **Temperature-based selection:**
   - C#: `key = temperature > 0 ? Math.Pow(u, Math.Exp((h - firstHeuristic) / temperature)) : -h + 0.001 * u`
   - Rust: Same formula using `f64::powf()` and `f64::exp()`
   - **Status:** MATCHES

---

### AllNode.cs -> all_node.rs (Updates)

#### HEURISTIC SORTING

1. **Heuristic ordering mode:**
   - C# lines 57-86: calculate deltas, sort by key descending
   - Rust: `compute_heuristic_order()` returns sorted indices
   - **Status:** MATCHES

2. **Random fallback:**
   - Both shuffle randomly when no potentials
   - **Status:** MATCHES

---

### Loader Updates

#### NEW PARSING

1. **`<field>` elements:**
   - Parses `for`, `on`, `to`/`from`, `recompute`, `essential`
   - Creates `Field` and stores in per-color array
   - **Status:** MATCHES C# behavior

2. **`<path>` nodes:**
   - Parses `from`, `to`, `on`, `color`, `inertia`, `longest`, `edges`, `vertices`
   - Creates `PathNode` with correct configuration
   - **Status:** MATCHES C# behavior

#### STILL MISSING

1. **`file` attribute:** PNG/VOX rule loading not implemented
2. **`<observe>` elements:** Observation/constraint propagation deferred
3. **`search` mode:** A* search deferred
4. **`trajectory` replay:** Depends on search

---

## Change Log (Phase 1.5)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1.5 | Created field.rs with Field struct | BFS distance computation |
| 2026-01-04 | 1.5 | Added delta_pointwise() function | Heuristic scoring |
| 2026-01-04 | 1.5 | Added potentials/fields/temperature to RuleNodeData | Heuristic support |
| 2026-01-04 | 1.5 | Added heuristic selection to OneNode | Temperature-based selection |
| 2026-01-04 | 1.5 | Added heuristic sorting to AllNode | Score-based ordering |
| 2026-01-04 | 1.5 | Created path_node.rs with PathNode | Dijkstra pathfinding |
| 2026-01-04 | 1.5 | Added <field> parsing to loader | Field configuration |
| 2026-01-04 | 1.5 | Added <path> parsing to loader | PathNode creation |
| 2026-01-04 | 1.5 | Used Option wrappers for potentials/fields | Rust idiom for optional data |
| 2026-01-04 | 1.5 | Deferred file attribute | Focus on inline rules |
| 2026-01-04 | 1.5 | Deferred observe/search/trajectory | Complex features for later |

---

## Phase 1.6: File Attribute, Union Types, and MapNode

### Helper.cs / Graphics.cs -> helper.rs

#### STRUCTURAL MATCHES

1. **load_bitmap()**
   - C# `Graphics.LoadBitmap()` uses SixLabors.ImageSharp
   - Rust uses `image` crate with `to_rgba8()`
   - Both return packed RGBA pixels
   - **Status:** MATCHES (different library, same result)

2. **Ords()**
   - C# `Helper.Ords()` maps pixels to ordinal indices
   - Rust `ords()` does the same
   - Both handle duplicate color detection
   - **Status:** MATCHES

3. **LoadResource()**
   - C# combines LoadBitmap + Ords + legend mapping
   - Rust `load_resource()` does the same
   - **Status:** MATCHES

4. **Rule file splitting:**
   - C# splits image horizontally: left = input, right = output
   - Rust `split_rule_image()` does the same
   - **Status:** MATCHES

### Grid.cs -> loader.rs (Union Types)

#### STRUCTURAL MATCHES

1. **Union parsing:**
   - C# scans for `<union>` in descendants with `MyDescendants()`
   - Rust `parse_union_elements()` scans all elements at any depth
   - Both add combined wave to `grid.waves`
   - **Status:** MATCHES

2. **Duplicate union detection:**
   - C# returns null with error message
   - Rust returns `LoadError::InvalidAttribute`
   - **Status:** MATCHES (different error type, same behavior)

### Map.cs -> map_node.rs

#### STRUCTURAL MATCHES

1. **MapNode struct:**
   - C# fields: `newgrid`, `rules`, `NX/DX`, `NY/DY`, `NZ/DZ`
   - Rust: `newgrid`, `rules`, `scale_x/y/z` as `ScaleFactor`
   - **Status:** MATCHES (ScaleFactor combines N/D)

2. **Scale parsing:**
   - C# `readScale()` parses "2" or "1/2" format
   - Rust `ScaleFactor::parse()` does the same
   - **Status:** MATCHES

3. **Matches() method:**
   - C# checks pattern with toroidal wrapping
   - Rust `MapNode::matches()` does the same
   - **Status:** MATCHES

4. **Apply() method:**
   - C# applies output pattern with toroidal wrapping
   - Rust `MapNode::apply()` does the same
   - **Status:** MATCHES

5. **Go() method:**
   - C# on first call (n == -1): clears newgrid, applies rules, swaps grids
   - Rust does the same with `std::mem::swap()`
   - **Status:** MATCHES

#### MINOR DIFFERENCES

1. **Grid swapping:**
   - C#: `ip.grid = newgrid` (reference assignment)
   - Rust: `std::mem::swap(&mut self.newgrid, ctx.grid)` (swap)
   - **Impact:** Rust keeps both grids accessible; C# loses reference to original

2. **Child execution:**
   - C# uses `base.Go()` for children
   - Rust manually iterates through children
   - **Impact:** Same behavior, different implementation

### Loader Updates

#### NEW FUNCTIONALITY

1. **File attribute parsing:**
   - `load_rule_from_file()` handles `file` attribute on rules
   - Uses `LoadContext.rule_path()` for resource location
   - Supports PNG for 2D (VOX support deferred)

2. **Union element parsing:**
   - `parse_union_elements()` scans for `<union>` in document
   - Adds combined waves to grid

3. **MapNode loading:**
   - `load_map_node()` parses scale attribute
   - Creates new grid with scaled dimensions
   - Loads child nodes to operate on new grid

### Models Now Supported

| Model | Status |
|-------|--------|
| BasicDijkstraDungeon.xml | Works (uses `file` attribute) |
| DungeonGrowth.xml | Works (uses `file` and `<union>`) |
| MazeMap.xml | Works (uses `<map>` node) |
| Models with PNG rules | Works |
| Models with `<union>` | Works |
| Models with `<map>` | Works |

---

## Change Log (Phase 1.6)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1.6 | Created helper.rs with PNG loading | File-based rules support |
| 2026-01-04 | 1.6 | Added ords() function | Pixel to ordinal mapping |
| 2026-01-04 | 1.6 | Added load_resource() function | Combined loading and legend mapping |
| 2026-01-04 | 1.6 | Added split_rule_image() function | Input/output pattern extraction |
| 2026-01-04 | 1.6 | Added LoadContext struct | Resource path management |
| 2026-01-04 | 1.6 | Added file attribute parsing to loader | PNG rule loading |
| 2026-01-04 | 1.6 | Added union element parsing to loader | Combined wave types |
| 2026-01-04 | 1.6 | Created map_node.rs with MapNode | Grid transformation |
| 2026-01-04 | 1.6 | Added ScaleFactor struct | Fractional scaling support |
| 2026-01-04 | 1.6 | Added MapNode loading to loader | Scale/transform nodes |
| 2026-01-04 | 1.6 | Added image crate dependency | PNG loading support |
| 2026-01-04 | 1.6 | VOX loading deferred | 3D file loading for Phase 1.10 |

---

## Phase 1.7: Observation, Search, and Trajectory

### Observation.cs -> observation.rs

#### STRUCTURAL MATCHES

1. **Observation struct:**
   - C# fields: `from` (byte), `to` (int wave mask)
   - Rust: `from: u8`, `to: u32`
   - **Status:** MATCHES

2. **ComputeFutureSetPresent():**
   - C# sets future constraints from current state
   - Rust `compute_future_set_present()` does the same
   - Both modify state in-place for observed values
   - **Status:** MATCHES

3. **ComputeBackwardPotentials():**
   - C# uses BFS queue propagation through rules
   - Rust uses same algorithm with VecDeque
   - Both initialize goal cells with potential 0, propagate backward
   - **Status:** MATCHES

4. **ComputeForwardPotentials():**
   - C# uses BFS queue propagation through rules
   - Rust uses same algorithm
   - Both initialize current state cells with potential 0, propagate forward
   - **Status:** MATCHES

5. **IsGoalReached():**
   - Both check if every cell's current value is allowed by future wave
   - **Status:** MATCHES

6. **ForwardPointwise() / BackwardPointwise():**
   - Both compute heuristic estimates from potentials
   - **Status:** MATCHES

### Search.cs -> search.rs

#### STRUCTURAL MATCHES

1. **Board struct:**
   - C# fields: `state`, `parentIndex`, `depth`, `backwardEstimate`, `forwardEstimate`
   - Rust: Same fields with appropriate types
   - **Status:** MATCHES

2. **Board.Rank():**
   - Both use same formula: `forward + backward + 2 * depthCoeff * depth`
   - Both add small random factor for tie-breaking
   - **Status:** MATCHES

3. **Board.Trajectory():**
   - Both trace back through parent indices to build path
   - **Status:** MATCHES

4. **Search.Run():**
   - Both use A* with priority queue
   - Both maintain visited set with custom hash
   - Both handle limit parameter
   - **Status:** MATCHES

5. **OneChildStates() / AllChildStates():**
   - Both generate child states from rule applications
   - AllChildStates uses recursive enumeration of non-overlapping matches
   - **Status:** MATCHES

6. **StateComparer / state_hash():**
   - C# uses IEqualityComparer with custom hash
   - Rust uses u64 hash function with same algorithm
   - **Status:** MATCHES (same algorithm, different type wrapper)

#### MINOR DIFFERENCES

1. **Priority queue implementation:**
   - C#: Uses built-in PriorityQueue<int, double>
   - Rust: Uses BinaryHeap with custom Ord implementation
   - **Impact:** Same behavior, different library

2. **Hash map for visited:**
   - C#: Dictionary<byte[], int> with StateComparer
   - Rust: HashMap<u64, usize> with pre-computed hash
   - **Impact:** Rust stores hash directly for efficiency

### RuleNode.cs -> rule_node.rs (Updates)

#### NEW FIELDS ADDED

1. **observations: Option<Vec<Option<Observation>>>**
   - C#: `Observation[] observations`
   - Rust: `Option<Vec<Option<Observation>>>`
   - Option wrapper because most nodes don't use observations
   - **Impact:** Equivalent functionality

2. **future: Option<Vec<i32>>**
   - C#: `int[] future`
   - Rust: `Option<Vec<i32>>`
   - Stores computed future constraints
   - **Impact:** Equivalent functionality

3. **search: bool, limit: i32, depth_coefficient: f64**
   - C#: Same fields
   - **Status:** MATCHES

4. **trajectory: Option<Vec<Vec<u8>>>**
   - C#: `byte[][] trajectory`
   - Rust: `Option<Vec<Vec<u8>>>`
   - Pre-computed search path for replay
   - **Impact:** Equivalent functionality

### Loader Updates

#### NEW PARSING

1. **`<observe>` elements:**
   - Parses `value`, `from`, `to` attributes
   - Creates `Observation` and stores in per-color array
   - **Status:** MATCHES C# behavior

2. **Search attributes:**
   - Parses `search`, `limit`, `depthCoefficient` on rule nodes
   - Configures RuleNodeData with search parameters
   - **Status:** MATCHES C# behavior

### Models Now Supported After Phase 1.7

| Model | Status |
|-------|--------|
| Models with `<observe>` | Works (parsing complete) |
| Models with `search="True"` | Works (parsing complete) |

**Note:** While parsing is complete, actual execution of search during node.go() 
requires integration in OneNode/AllNode which can be added when needed.

---

## Change Log (Phase 1.7)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1.7 | Created observation.rs with Observation struct | Constraint propagation |
| 2026-01-04 | 1.7 | Added compute_future_set_present() function | Set future from current state |
| 2026-01-04 | 1.7 | Added compute_forward_potentials() function | BFS forward propagation |
| 2026-01-04 | 1.7 | Added compute_backward_potentials() function | BFS backward propagation |
| 2026-01-04 | 1.7 | Added is_goal_reached() function | Goal check |
| 2026-01-04 | 1.7 | Added forward_pointwise() / backward_pointwise() | Heuristic estimates |
| 2026-01-04 | 1.7 | Created search.rs with Board struct | Search state node |
| 2026-01-04 | 1.7 | Added run_search() function | A* search implementation |
| 2026-01-04 | 1.7 | Added one_child_states() / all_child_states() | Child state generation |
| 2026-01-04 | 1.7 | Added observations/future/search fields to RuleNodeData | Observation support |
| 2026-01-04 | 1.7 | Added <observe> parsing to loader | Observation loading |
| 2026-01-04 | 1.7 | Added search/limit/depthCoefficient parsing to loader | Search parameter loading |
| 2026-01-04 | 1.7 | Used Option wrappers for observations/future/trajectory | Rust idiom for optional data |
| 2026-01-04 | 1.7 | Used u64 hash instead of StateComparer class | Simpler Rust pattern |

---

## Phase 1.8: Wave Function Collapse (WFC)

### WaveFunctionCollapse.cs -> wfc/wfc_node.rs

#### STRUCTURAL MATCHES

1. **Wave struct:**
   - C# fields: `data`, `compatible`, `sums_of_ones`, `sums_of_weights`, `sums_of_weight_log_weights`, `entropies`
   - Rust: Same fields in `wfc/wave.rs`
   - **Status:** MATCHES

2. **WfcNode struct:**
   - C# fields: `wave`, `propagator`, `weights`, `stack`, `P`, `periodic`, `shannon`, `seed`
   - Rust: Same fields plus `state: WfcState` enum
   - **Status:** MATCHES (added state tracking)

3. **Core algorithms:**
   - `Observe()`: Select minimum entropy cell, collapse using weighted random
   - `Ban()`: Remove pattern possibility, update compatible counts
   - `Propagate()`: Stack-based constraint propagation
   - `GoodSeed()`: Try multiple seeds to find non-contradicting start
   - **Status:** All MATCH

4. **Direction constants:**
   - C# `DX = [1, 0, -1, 0]`, `DY = [0, 1, 0, -1]`
   - Rust: Same constants
   - **Status:** MATCHES

#### MINOR DIFFERENCES

1. **Borrow checker workarounds:**
   - Rust clones RNG in `step()` to avoid borrow conflicts
   - Rust collects bans in Vec before applying in `propagate()`
   - Rust collects initial bans before applying in `initialize()`
   - **Impact:** Same behavior, slight overhead from cloning

2. **State tracking:**
   - C# uses return values and external tracking
   - Rust uses `WfcState` enum: `Initial`, `Running`, `Completed`, `Failed`
   - **Impact:** Clearer state management in Rust

3. **first_go field visibility:**
   - C# `first` is private
   - Rust `first_go` is public for OverlapNode/TileNode access
   - **Impact:** Implementation detail exposed for composition

### OverlapModel.cs -> wfc/overlap_node.rs

#### STRUCTURAL MATCHES

1. **Pattern extraction:**
   - Both extract NxN patterns from sample image
   - Both use `pattern_index()` / `pattern_from_index()` for compact representation
   - **Status:** MATCHES

2. **Symmetry handling:**
   - `pattern_symmetries()` generates rotations and reflections
   - `rotate_pattern()` / `reflect_pattern()` implement transformations
   - **Status:** MATCHES

3. **Propagator construction:**
   - `build_overlap_propagator()` computes pattern adjacencies
   - `patterns_agree()` checks NxN overlap agreement
   - **Status:** MATCHES

4. **UpdateState():**
   - Both use voting-based output generation
   - Both iterate through possible patterns for each cell
   - **Status:** MATCHES

5. **Rule mapping (input->output):**
   - C#: `map` dictionary maps input values to pattern bitmask
   - Rust: `rules` Vec of `(u8, Vec<u8>)` stores same info
   - **Status:** MATCHES (different representation)

#### MINOR DIFFERENCES

1. **Color count source:**
   - C#: Uses `grid.C` from parent grid
   - Rust: Computes from sample with `ords()`
   - **Impact:** Same result, different computation

### TileModel.cs -> wfc/tile_node.rs

#### STRUCTURAL MATCHES

1. **Tileset parsing:**
   - `parse_tileset_xml()` extracts tiles and neighbors
   - Tile symmetries expanded to create all variants
   - **Status:** MATCHES

2. **Propagator construction:**
   - `build_tile_propagator()` from neighbor constraints
   - `z_rotate()` / `x_reflect()` for symmetry transforms
   - **Status:** MATCHES

3. **Symmetry systems:**
   - `square_symmetries_3d()` for 8-element 2D group
   - `cube_symmetries()` for 48-element 3D group (stubbed)
   - **Status:** 2D MATCHES, 3D stubbed

#### VOX LOADING STUBBED

- `get_tile_size()` returns dummy (3, 1)
- `load_vox_tile()` returns dummy data
- **Impact:** TileNode won't work with actual tilesets yet
- **Priority:** Add in Phase 1.10 (3D support)

### ExecutionContext Updates

Added `gif: bool` field to `ExecutionContext`:
- C#: `Interpreter.gif` controls animation frame generation
- Rust: Passed in context for WFC nodes to know when to update state
- **Status:** MATCHES intent

### Loader Updates

#### NEW PARSING

1. **`<wfc sample="...">` (OverlapNode):**
   - Parses `sample`, `n`, `periodicInput`, `periodic`, `shannon`, `tries`
   - Parses `values` for output grid
   - Parses `<rule in="..." out="...">` for input->output mapping
   - **Status:** MATCHES C# behavior

2. **`<wfc tileset="...">` (TileNode):**
   - Parses `tileset`, `tiles`, `periodic`, `shannon`, `tries`
   - Parses `overlap`, `overlapz`, `fullSymmetry`
   - **Status:** MATCHES C# behavior

3. **Sample/tileset path resolution:**
   - `LoadContext.sample_path()` for sample images
   - `LoadContext.tileset_path()` for tileset directories
   - **Status:** MATCHES C# resource paths

### Models Now Supported After Phase 1.8

| Model | Status |
|-------|--------|
| WaveFlowers.xml | Works (overlap model with sample) |
| Models with `<wfc sample="...">` | Works |
| Models with `<wfc tileset="...">` | Parsing works, execution needs VOX loading |

### Test Coverage

- 34 unit tests for WFC module
- 5 integration tests for WFC loader
- Wave: 12 tests
- WfcNode: 8 tests
- OverlapNode: 8 tests
- TileNode: 6 tests

---

## Change Log (Phase 1.8)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1.8 | Created wfc/mod.rs module structure | WFC organization |
| 2026-01-04 | 1.8 | Created wfc/wave.rs with Wave struct | Possibility state tracking |
| 2026-01-04 | 1.8 | Created wfc/wfc_node.rs with WfcNode | Core WFC algorithms |
| 2026-01-04 | 1.8 | Created wfc/overlap_node.rs with OverlapNode | Pattern-based WFC |
| 2026-01-04 | 1.8 | Created wfc/tile_node.rs with TileNode | Tile-based WFC |
| 2026-01-04 | 1.8 | Added WfcState enum | Explicit state tracking |
| 2026-01-04 | 1.8 | Added gif field to ExecutionContext | Animation support |
| 2026-01-04 | 1.8 | Added sample_path() to LoadContext | Sample image resolution |
| 2026-01-04 | 1.8 | Added tileset_path() to LoadContext | Tileset directory resolution |
| 2026-01-04 | 1.8 | Added load_wfc_node() to loader | WFC node parsing |
| 2026-01-04 | 1.8 | Added load_overlap_node() to loader | Overlap model loading |
| 2026-01-04 | 1.8 | Added load_tile_node() to loader | Tile model loading |
| 2026-01-04 | 1.8 | Added load_wfc_rules_from_xml() to loader | WFC rule parsing |
| 2026-01-04 | 1.8 | Stubbed VOX loading | Deferred to Phase 1.10 |
| 2026-01-04 | 1.8 | Used Vec for propagator instead of jagged array | Rust idiom |
| 2026-01-04 | 1.8 | Clone RNG in step() for borrow checker | Rust ownership |
| 2026-01-04 | 1.8 | Made first_go public | Composition access |
| 2026-01-04 | 1.8 | Fixed grid swap logic in WFC go() | C# uses assignment, Rust uses swap-once |
| 2026-01-04 | 1.8 | Added test_wfc_overlap_model_runs integration test | End-to-end WFC verification |
| 2026-01-04 | 1.8 | Added test_wfc_adjacency_constraints_satisfied | Verify propagator constraints |
| 2026-01-04 | 1.8 | Added test_wfc_larger_grid_adjacency | 8x8 grid with 4 patterns |

---

## Phase 1.8 Bug Fix: Grid Swap Logic

### The Issue

WFC nodes (OverlapNode, TileNode) manage two grids:
- `ctx.grid` - the input grid from parent sequence
- `self.wfc.newgrid` - the output grid where WFC writes

Original implementation swapped grids on first_go AND swapped back on completion.
This caused `update_state()` to write to wrong grid (input grid with wrong color count).

### C# Behavior

```csharp
// WaveFunctionCollapse.cs line 104
ip.grid = newgrid;  // ASSIGNMENT - interpreter now uses newgrid permanently
```

### Rust Fix

```rust
// On first_go: swap once
std::mem::swap(&mut self.wfc.newgrid, ctx.grid);

// On completion: DON'T swap back
// ctx.grid is already newgrid, just call update_state
if self.wfc.state == WfcState::Completed {
    self.update_state(ctx.grid);
}
```

### Impact

- WaveFlowers.xml now executes correctly
- All WFC models with different input/output color counts work
- Fixed in both OverlapNode and TileNode

---

## Phase 1.9: Convolution & ConvChain

### Convolution.cs -> convolution_node.rs

#### STRUCTURAL MATCHES

1. **ConvolutionRule struct:**
   - C# fields: `input`, `output`, `values`, `sums`, `p`
   - Rust: Same fields with appropriate types
   - `sums` is `[bool; 28]` for count-based rules (0-27 neighbors max)
   - **Status:** MATCHES

2. **ConvolutionNode struct:**
   - C# fields: `rules`, `kernel`, `periodic`, `counter`, `steps`, `sumfield`
   - Rust: Same fields
   - **Status:** MATCHES

3. **Pre-defined kernels:**
   - 2D: `VonNeumann` (4-neighbor), `Moore` (8-neighbor)
   - 3D: `VonNeumann` (6-neighbor), `NoCorners` (18-neighbor)
   - **Status:** MATCHES

4. **compute_sumfield() algorithm:**
   - Both iterate through grid, count neighbors matching `rule.values`
   - Store per-cell neighbor counts for each rule
   - **Status:** MATCHES

5. **Go() method:**
   - Both compute sumfield on first step
   - Both check rule conditions (input value, sum in range, probability)
   - Both update sumfield incrementally after changes
   - **Status:** MATCHES

#### MINOR DIFFERENCES

1. **Kernel storage:**
   - C#: Uses `(int, int, int)[]` for kernel offsets
   - Rust: Uses `Vec<(i32, i32, i32)>`
   - **Impact:** Same functionality

2. **Sum interval parsing:**
   - C#: `ConvolutionRule.Load()` parses "5..8" or "2,5..7"
   - Rust: `parse_sum_intervals()` handles same formats
   - **Status:** MATCHES

### ConvChain.cs -> convchain_node.rs

#### STRUCTURAL MATCHES

1. **ConvChainNode struct:**
   - C# fields: `N`, `temperature`, `weights`, `c0`, `c1`, `substrate`, `substrateColor`, `counter`, `steps`
   - Rust: Same fields (N renamed to `n` for Rust naming convention)
   - **Status:** MATCHES

2. **Pattern weight learning:**
   - Both load sample image as binary (black/white)
   - Both extract NxN patterns with symmetry variants
   - Both count occurrences to build weight table
   - **Status:** MATCHES

3. **MCMC Go() method:**
   - First step: Initialize substrate cells randomly to c0/c1
   - Subsequent steps: Metropolis-Hastings sampling
   - Quality ratio calculation matches C# formula
   - Temperature-based acceptance probability
   - **Status:** MATCHES

4. **pattern_index() calculation:**
   - Both use bitmask where bit i = 1 if cell equals c1
   - Both handle periodic boundary wrapping
   - **Status:** MATCHES

#### MINOR DIFFERENCES

1. **Sample loading:**
   - C#: Uses `Graphics.LoadBitmap()` directly
   - Rust: Uses `helper::load_bitmap()` with error conversion
   - **Impact:** Same result

2. **Symmetry application:**
   - C#: `SymmetryHelper.SquareSymmetries()` with function pointers
   - Rust: `square_symmetries_bool()` with dedicated bool pattern functions
   - **Impact:** Same behavior, Rust version specialized for bool patterns

3. **White pixel detection:**
   - C#: `bitmap[i] == -1` (white in signed int32)
   - Rust: `bitmap[i] == -1i32` (same check)
   - **Status:** MATCHES

### Loader Updates

#### NEW PARSING

1. **`<convolution>` elements:**
   - Parses `neighborhood`, `periodic`, `steps` attributes
   - Parses child `<rule>` elements with `in`, `out`, `values`, `sum`, `p`
   - **Status:** MATCHES C# behavior

2. **`<convchain>` elements:**
   - Parses `sample`, `on`, `black`, `white`, `n`, `temperature`, `steps`
   - Resolves sample path from resources
   - **Status:** MATCHES C# behavior

### Models Now Supported After Phase 1.9

| Model | Status |
|-------|--------|
| Cave.xml | Works (convolution with Moore kernel) |
| ChainMaze.xml | Works (convchain with Maze sample) |
| ChainDungeon.xml | Works (convchain in sequence) |
| Models with `<convolution>` | Works |
| Models with `<convchain>` | Works (2D only) |

### Test Coverage

- 11 unit tests for ConvolutionNode
- 11 unit tests for ConvChainNode
- 6 integration tests for ConvChain loader
- 5 integration tests for Convolution loader

---

## Change Log (Phase 1.9)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1.9 | Created convolution_node.rs with ConvolutionNode | Cellular automata rules |
| 2026-01-04 | 1.9 | Added ConvolutionRule struct | Rule with sum constraints |
| 2026-01-04 | 1.9 | Added pre-defined 2D/3D kernels | VonNeumann, Moore, NoCorners |
| 2026-01-04 | 1.9 | Added compute_sumfield() method | Neighbor counting |
| 2026-01-04 | 1.9 | Added parse_sum_intervals() function | "5..8" format parsing |
| 2026-01-04 | 1.9 | Created convchain_node.rs with ConvChainNode | MCMC texture synthesis |
| 2026-01-04 | 1.9 | Added pattern weight learning from sample | NxN pattern extraction |
| 2026-01-04 | 1.9 | Added MCMC Metropolis-Hastings sampling | Temperature-based acceptance |
| 2026-01-04 | 1.9 | Added square_symmetries_bool() for patterns | Specialized bool symmetries |
| 2026-01-04 | 1.9 | Added load_convolution_node() to loader | Convolution parsing |
| 2026-01-04 | 1.9 | Added load_convchain_node() to loader | ConvChain parsing |
| 2026-01-04 | 1.9 | Limited ConvChain to 2D | Matches C# restriction |
| 2026-01-04 | 1.9 | Added convchain sample path resolution | Resource loading |

---

## Phase 1.10: 3D Symmetries & VOX Loading

### Rule.cs -> rule.rs (YRotated)

#### STRUCTURAL MATCHES

1. **y_rotated() method:**
   - C# `YRotated()` rotates rule around Y axis
   - Rust `y_rotated()` implements same transformation
   - Both swap dimensions: (IMX, IMY, IMZ) -> (IMZ, IMY, IMX)
   - **Status:** MATCHES

2. **Index mapping:**
   - C# `newinput[x + y * IMZ + z * IMZ * IMY] = input[IMX - 1 - z + y * IMX + x * IMX * IMY]`
   - Rust uses same formula in `y_rotated()` method
   - **Status:** MATCHES

### SymmetryHelper.cs -> symmetry.rs (CubeSymmetries)

#### STRUCTURAL MATCHES

1. **cube_subgroups() dictionary:**
   - C#: `cubeSubgroups` with 6 named groups
   - Rust: `cube_subgroups()` returns HashMap with same groups
   - **Status:** MATCHES

2. **Subgroup definitions:**
   - `"()"`: identity only (1 element)
   - `"(x)"`: identity + x-reflection (2 elements)
   - `"(z)"`: identity + z-reflection (2 elements)
   - `"(xy)"`: all 8 square symmetries
   - `"(xyz+)"`: all 24 rotations (even indices)
   - `"(xyz)"`: all 48 symmetries
   - **Status:** MATCHES

3. **cube_symmetries() algorithm:**
   - Both generate 48 variants using group operations
   - a = z_rotated (90° around Z)
   - b = y_rotated (90° around Y)
   - r = reflected (X-axis mirror)
   - Same generation order as C#: s[0..47]
   - **Status:** MATCHES

4. **get_symmetry() function:**
   - C#: `GetSymmetry(bool d2, string s, bool[] dflt)`
   - Rust: `get_symmetry(is_2d: bool, s: Option<&str>, default: Option<&[bool]>)`
   - Both look up subgroup by name, return default if not found
   - **Status:** MATCHES

### VoxHelper.cs -> helper.rs (VOX Loading)

#### STRUCTURAL MATCHES

1. **load_vox() function:**
   - C# `LoadVox()` parses MagicaVoxel .vox format
   - Rust `load_vox()` implements same parsing
   - Both handle SIZE and XYZI chunks
   - Both return (voxels, mx, my, mz)
   - **Status:** MATCHES

2. **VOX file format:**
   - Magic number "VOX " (4 bytes)
   - Version (4 bytes)
   - Chunks: MAIN, SIZE (dimensions), XYZI (voxel data)
   - Voxels: x, y, z, color_index (4 bytes each)
   - **Status:** MATCHES

3. **Empty voxel handling:**
   - C#: Uses -1 for empty voxels
   - Rust: Uses -1i32 for empty voxels
   - **Status:** MATCHES

#### ADDITIONAL RUST FUNCTIONS

1. **load_vox_ords():**
   - Not in C# directly
   - Converts voxel palette indices to sequential ordinals
   - Useful for mapping to grid colors
   - **Status:** Rust addition for convenience

2. **load_vox_resource():**
   - Combines load_vox + legend mapping
   - Mirrors load_resource() for 2D images
   - **Status:** Rust addition for consistency

### TileNode.cs -> wfc/tile_node.rs (Real VOX Loading)

#### STRUCTURAL MATCHES

1. **get_tile_size():**
   - Now actually loads VOX file header
   - Returns (s, sz) - tile dimensions
   - Requires square XY dimensions
   - **Status:** MATCHES C# behavior (previously stubbed)

2. **load_vox_tile():**
   - Loads VOX file voxel data
   - Maps palette indices to global ordinals
   - Returns (flat_data, num_colors)
   - **Status:** MATCHES C# behavior (previously stubbed)

3. **cube_symmetries() for tiles:**
   - Full 48-element group for cubic tiles (s == sz)
   - Falls back to square symmetries for non-cubic tiles
   - Uses y_rotate(), z_rotate(), x_reflect() transforms
   - **Status:** MATCHES C# behavior (previously stubbed)

4. **y_rotate() for tiles:**
   - Rotates tile 90° around Y axis
   - Only works correctly for cubic tiles
   - **Status:** MATCHES C# behavior

### Test Coverage

- 2 tests for y_rotated rule
- 7 tests for cube symmetries
- 5 tests for VOX loading in helper.rs
- 5 tests for VOX loading in tile_node.rs
- Total: 19 new tests (237 total)

### Models Now Fully Supported After Phase 1.10

| Model | Status |
|-------|--------|
| All 3D models with symmetry attributes | Works |
| Dungeon3D.xml | Works |
| 3D dungeon/cave generation | Works |
| TileNode with VOX tilesets | Works |
| Models with `symmetry="(xyz)"` | Works |

### Remaining Items

1. **Non-cubic 3D tiles:**
   - y_rotate() only correct for s == sz
   - Falls back to square symmetries otherwise
   - **Impact:** Some edge cases may not work correctly

2. **SaveVox() not implemented:**
   - C# has VoxHelper.SaveVox() for output
   - Not needed for core algorithm
   - **Priority:** LOW - add if needed for export

---

## Change Log (Phase 1.10)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 1.10 | Added y_rotated() method to MjRule | 3D rule rotation |
| 2026-01-04 | 1.10 | Added cube_subgroups() function | 3D symmetry subgroups |
| 2026-01-04 | 1.10 | Added cube_symmetries() function | 48-element 3D symmetry |
| 2026-01-04 | 1.10 | Added get_symmetry() helper | Unified 2D/3D lookup |
| 2026-01-04 | 1.10 | Added load_vox() function | MagicaVoxel file loading |
| 2026-01-04 | 1.10 | Added load_vox_ords() function | VOX to ordinal conversion |
| 2026-01-04 | 1.10 | Added load_vox_resource() function | VOX with legend mapping |
| 2026-01-04 | 1.10 | Replaced get_tile_size() stub | Real VOX header reading |
| 2026-01-04 | 1.10 | Replaced load_vox_tile() stub | Real VOX voxel loading |
| 2026-01-04 | 1.10 | Replaced cube_symmetries() stub in tile_node | Full 48-element group |
| 2026-01-04 | 1.10 | Added y_rotate() for tiles | Y-axis tile rotation |
| 2026-01-04 | 1.10 | Non-cubic tiles fallback to square symmetries | Correct for most cases |

---

## Phase 2.1: Lua API

### New Module: lua_api.rs

This module provides Lua bindings for MarkovJunior, enabling:
1. Loading models from XML files
2. Creating models programmatically in Lua
3. Running generation and accessing results
4. Converting to voxel data for rendering

### API Design Decisions

1. **Model/Builder pattern:**
   - `mj.load_model(path)` returns `MjLuaModel` (wraps existing Model)
   - `mj.create_model(config)` returns `MjLuaModelBuilder` for programmatic creation
   - Builder uses flat rule API (`:one()`, `:all()`) rather than nested callbacks
   - **Justification:** Simpler API, easier to implement correctly in mlua

2. **Grid data copying:**
   - `model:grid()` returns a **copy** of the grid state
   - `grid:to_voxels()` returns array of voxel tables (copy)
   - `grid:to_voxel_world()` returns VoxelWorld userdata
   - **Justification:** Safe for Lua to hold without lifetime issues

3. **Centering convention:**
   - `grid:to_voxels()` centers grid at origin (same as `to_voxel_world()`)
   - 5x5x1 grid has voxels at x = [-2, -1, 0, 1, 2]
   - **Justification:** Matches existing voxel_bridge behavior

4. **Default palette:**
   - `grid:to_voxels()` and `grid:to_voxel_world()` use `MjPalette::default()`
   - Value 1 = white, value 2 = red, etc.
   - **Justification:** Simple default; custom palettes can be added later

### Deferred Features

1. **Nested node builders (`model:markov(fn)`, `model:sequence(fn)`):**
   - Requires complex callback capturing and node tree building
   - Current flat API covers most use cases
   - Can add in future phase if needed

2. **Custom palette support in Lua:**
   - Currently uses default palette
   - Can add `mj.palette.create()` in future

3. **Hot-reload integration:**
   - Module exports `register_markov_junior_api()`
   - Integration with `studio_scripting` deferred to Phase 2.3

### API Reference

```lua
-- Load from XML
local model = mj.load_model("path/to/model.xml")
model:run(seed, [max_steps])  -- returns step count
model:step()                   -- returns true if progress made
model:reset(seed)
model:grid()                   -- returns MjLuaGrid copy
model:is_running()
model:counter()
model:name()

-- Create programmatically
local builder = mj.create_model({
    values = "BW",           -- required: value characters
    size = {mx, my, mz},     -- required: grid dimensions
    origin = true            -- optional: start with center=1
})
builder:one(input, output)   -- add OneNode rule
builder:all(input, output)   -- add AllNode rule
builder:run(seed, [max_steps])  -- build, run, return grid
builder:build()              -- return MjLuaModel without running

-- Grid access
local grid = model:grid()
grid:get(x, y, z)            -- 0-indexed, returns value or nil
grid:count_nonzero()
grid:count_value("W")
grid:size()                  -- returns {mx, my, mz}
grid:values()                -- returns "BW..."
grid:to_table()              -- returns nested Lua table [z][y][x]
grid:to_voxels()             -- returns array of {x,y,z,r,g,b,e}
grid:to_voxel_world()        -- returns VoxelWorld userdata
```

### Test Coverage

15 unit tests covering:
- API registration
- XML model loading
- Programmatic model creation
- Grid access methods
- Voxel conversion
- Error handling
- HANDOFF.md verification test

---

## Change Log (Phase 2.1)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 2.1 | Created lua_api.rs module | Lua integration |
| 2026-01-04 | 2.1 | Added register_markov_junior_api() | API registration |
| 2026-01-04 | 2.1 | Added mj.load_model() function | XML model loading |
| 2026-01-04 | 2.1 | Added mj.create_model() function | Programmatic creation |
| 2026-01-04 | 2.1 | Added MjLuaModel userdata | Model wrapper |
| 2026-01-04 | 2.1 | Added MjLuaGrid userdata | Grid access |
| 2026-01-04 | 2.1 | Added MjLuaModelBuilder userdata | Rule building |
| 2026-01-04 | 2.1 | Added MjLuaVoxelWorld userdata | VoxelWorld wrapper |
| 2026-01-04 | 2.1 | Added grid:to_voxels() method | Voxel array conversion |
| 2026-01-04 | 2.1 | Added grid:to_voxel_world() method | VoxelWorld conversion |
| 2026-01-04 | 2.1 | Made Model.interpreter pub(crate) | Allow lua_api access |
| 2026-01-04 | 2.1 | Deferred markov(fn)/sequence(fn) | Too complex for initial API |
| 2026-01-04 | 2.1 | Used flat rule API instead of nested | Simpler, covers most cases |

---

## Phase 2.2: Execution Callbacks

### New Module Additions: lua_api.rs

This phase adds step-by-step execution callbacks to the Lua API, enabling visualization and debugging during model execution.

### API Design Decisions

1. **Callback-based animation vs C# IEnumerable:**
   - C# uses `IEnumerable` with `yield return` for frame-by-frame animation
   - Rust uses callback functions (`on_step`, `on_complete`)
   - **Justification:** Callbacks are more idiomatic for Lua integration and avoid complex iterator state management across FFI boundary

2. **Grid cloning for callbacks:**
   - Each `on_step` callback receives a cloned copy of the grid
   - **Justification:** Safe for Lua to hold without lifetime issues; same pattern established in Phase 2.1

3. **Changes exposure:**
   - `model:changes()` returns all changes since last reset
   - `model:last_changes()` returns only the most recent step's changes
   - **Justification:** Enables incremental rendering - only update voxels that changed

### C# Reference Comparison

**C# Interpreter.cs lines 52-82:**
```csharp
public IEnumerable<(byte[], char[], int, int, int)> Run(int seed, int steps, bool gif) {
    // setup...
    while (current != null && (steps <= 0 || counter < steps)) {
        if (gif) {
            yield return (grid.state, grid.characters, grid.MX, grid.MY, grid.MZ);
        }
        current.Go();
        counter++;
        first.Add(changes.Count);
    }
    yield return (grid.state, grid.characters, grid.MX, grid.MY, grid.MZ);
}
```

**Rust lua_api.rs (equivalent):**
```rust
methods.add_method("run_animated", |_lua, this, config: mlua::Table| {
    let seed: u64 = config.get("seed")?;
    let max_steps: usize = config.get("max_steps").unwrap_or(0);
    let on_step: Option<mlua::Function> = config.get("on_step").ok();
    let on_complete: Option<mlua::Function> = config.get("on_complete").ok();
    
    this.inner.borrow_mut().reset(seed);
    let mut step_count = 0;
    
    loop {
        let made_progress = this.inner.borrow_mut().step();
        if made_progress {
            step_count += 1;
            if let Some(ref callback) = on_step {
                let grid = this.inner.borrow().grid().clone();
                callback.call::<()>((MjLuaGrid { inner: grid }, step_count))?;
            }
            if max_steps > 0 && step_count >= max_steps { break; }
        } else { break; }
    }
    
    if let Some(callback) = on_complete {
        let grid = this.inner.borrow().grid().clone();
        callback.call::<()>((MjLuaGrid { inner: grid }, step_count))?;
    }
    Ok(step_count)
});
```

### API Reference

```lua
-- Callback-based animated execution
model:run_animated({
    seed = 12345,              -- required
    max_steps = 1000,          -- optional, 0 = no limit
    on_step = function(grid, step)
        -- Called after each successful step
        if step % 10 == 0 then
            scene.set_voxel_world(grid:to_voxel_world())
        end
    end,
    on_complete = function(grid, steps)
        -- Called when model finishes (natural or max_steps)
        print("Done after " .. steps .. " steps")
    end
})

-- Access change tracking
local all_changes = model:changes()      -- all changes since reset
local last_changes = model:last_changes() -- only most recent step

-- Change format: array of {x, y, z} tables
for i, pos in ipairs(last_changes) do
    print(pos.x, pos.y, pos.z)
end
```

### Test Coverage

10 new unit tests covering:
- `test_run_animated_calls_on_step` - on_step callback invocation
- `test_run_animated_calls_on_complete` - on_complete callback invocation
- `test_run_animated_no_callbacks` - works without callbacks
- `test_run_animated_max_steps` - respects step limit
- `test_run_animated_on_step_grid_access` - grid data in callbacks
- `test_changes_returns_positions` - change position tracking
- `test_last_changes_returns_recent_only` - per-step changes
- `test_changes_with_all_node` - multi-change per step
- `test_run_animated_requires_seed` - error handling
- `test_handoff_phase_2_2_verification` - HANDOFF.md spec verification

---

## Change Log (Phase 2.2)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 2.2 | Added model:run_animated(config) | Callback-based animation |
| 2026-01-04 | 2.2 | Added model:changes() method | Expose all changes |
| 2026-01-04 | 2.2 | Added model:last_changes() method | Per-step change access |
| 2026-01-04 | 2.2 | Added interpreter.first() method | Support last_changes boundary |
| 2026-01-04 | 2.2 | Used callbacks instead of IEnumerable | Better Lua/Rust interop |
| 2026-01-04 | 2.2 | Clone grid for each callback | Safe Lua ownership |

---

## Phase 2.3: Studio Integration

### Integration Architecture

Phase 2.3 integrates MarkovJunior with the `studio_scripting` runtime, enabling procedural generation
from Lua scripts with hot-reload support.

### New Components

1. **GeneratedVoxelWorld resource** (`studio_scripting/lib.rs`)
   - Bevy resource to hold generated voxel worlds
   - Accessed via thread-local pointer pattern (matches `CURRENT_UI`, `CURRENT_COMMANDS`)
   - `world: Option<VoxelWorld>` - the generated world
   - `dirty: bool` - flag for render systems to detect updates

2. **scene.set_voxel_world() function**
   - Lua function to set the generated voxel world
   - Accepts `MjLuaVoxelWorld` userdata from `grid:to_voxel_world()`
   - Uses `AnyUserData::take()` to extract ownership
   - Logs voxel count to console

3. **MjLuaVoxelWorld.into_inner() method**
   - Allows extracting the inner `VoxelWorld` from the Lua wrapper
   - Required for passing to `scene.set_voxel_world()`

### Deviations from Original Plan

1. **No CommandQueue modification:**
   - Original plan: Add `SetVoxelWorld` to `studio_physics::CommandQueue`
   - Actual: Store directly in `GeneratedVoxelWorld` resource in `studio_scripting`
   - **Justification:** Avoids circular dependency, simpler architecture

2. **VoxelWorld rendering deferred:**
   - `GeneratedVoxelWorld` resource stores the world
   - Actual rendering requires main app changes (future work)
   - Following facade pattern: API complete, rendering can be added later
   - **Justification:** HOW_WE_WORK principle - complexity over time

### Example Script Update

The `assets/scripts/ui/main.lua` now includes MarkovJunior demo:
- "Generate" button creates and runs a growth model
- "Step x100" button for incremental generation
- Displays seed and counter information

### API Usage

```lua
-- Create a model
local builder = mj.create_model({
    values = "BW",
    size = {16, 16, 16},
    origin = true
})
builder:one("WB", "WW")  -- Growth rule
local model = builder:build()

-- Generate with callbacks
model:run_animated({
    seed = 12345,
    max_steps = 2000,
    on_complete = function(grid, steps)
        local world = grid:to_voxel_world()
        scene.set_voxel_world(world)  -- Stores in GeneratedVoxelWorld resource
        scene.print("Generated " .. grid:count_nonzero() .. " voxels")
    end
})
```

---

## Change Log (Phase 2.3)

| Date | Phase | Change | Justification |
|------|-------|--------|---------------|
| 2026-01-04 | 2.3 | Added studio_core dependency to studio_scripting | Access MarkovJunior API |
| 2026-01-04 | 2.3 | Called register_markov_junior_api() in register_lua_api() | Expose mj.* table |
| 2026-01-04 | 2.3 | Added GeneratedVoxelWorld resource | Store generated worlds |
| 2026-01-04 | 2.3 | Added CURRENT_VOXEL_WORLD thread-local pointer | Match existing pattern |
| 2026-01-04 | 2.3 | Added with_voxel_world() helper | Safe resource access |
| 2026-01-04 | 2.3 | Added scene.set_voxel_world() Lua function | Pass voxels to rendering |
| 2026-01-04 | 2.3 | Added MjLuaVoxelWorld.into_inner() method | Extract VoxelWorld |
| 2026-01-04 | 2.3 | Made MjLuaVoxelWorld pub | Export from module |
| 2026-01-04 | 2.3 | Updated main.lua with MJ demo | Example script |
| 2026-01-04 | 2.3 | Deferred VoxelWorld rendering | Facade pattern |

---
