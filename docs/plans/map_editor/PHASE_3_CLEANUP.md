# Phase 3 Cleanup Audit

This document tracks refactoring candidates identified during Phase 3 milestone work. Each entry describes existing code that could benefit from new abstractions introduced in this phase.

**Purpose:** Capture tech debt opportunities as we go, without blocking milestone progress. Review at phase end to decide what to address before Phase 4.

---

## How to Read This Document

Each cleanup item includes:
- **Milestone:** When we noticed this opportunity
- **Current State:** What the code looks like now
- **Proposed Change:** What it could look like using new abstractions
- **Why Refactor:** Engineering rationale for the change
- **Criticality:** How urgent is this?
  - **High:** Blocks future work or causes active problems
  - **Medium:** Creates inconsistency or minor duplication
  - **Low:** Nice-to-have, purely aesthetic improvement
- **When to Do:** Suggested timing for the refactor

---

## M8 Audit: Markov Jr. Generator

### 1. Design Change: Lua-Based Instead of Rust Adapter

**Milestone:** M8 (Markov Jr. Generator)

**Original Plan:** Create `MarkovGenerator` Rust struct that wraps `Interpreter` and implements `Generator` trait.

**Actual Implementation:** Expose `mj` module to Lua context, let users write Lua generators that call `mj.load_model()`.

**Why Changed:** 
- Simpler: No new Rust code needed
- More flexible: Users can combine mj calls with other Lua logic
- Follows existing pattern: All generators are Lua-based already

**Impact on Spec:** The spec's `generator/markov.rs` file is NOT needed. The functionality is achieved through:
1. `register_markov_junior_api(&lua)` in `lua_generator.rs`
2. Example Lua script `assets/map_editor/generator_markov.lua`

**Criticality:** **N/A** - This is a design change, not cleanup item.

---

### 2. Generator Type Detection for MCP Endpoint

**Milestone:** M8 (Markov Jr. Generator)

**Current State:**
```rust
// mcp_server.rs
GeneratorStateJson {
    generator_type: "lua".to_string(), // Always "lua"
    // ...
}
```

**Issue:** The `generator_type` field is always "lua" because we can't detect what the Lua script is doing internally (whether it uses mj or not).

**Proposed Change:** Either:
a) Accept that type is always "lua" (simpler)
b) Add a way for Lua generators to self-report their type (more complex)

**Why Refactor:** The spec shows `"type": "markov"` but implementation returns `"type": "lua"`.

**Criticality:** **Low** - Doesn't affect functionality, just metadata accuracy.

**When to Do:** Defer to Phase 3 end review. May not be worth the complexity.

---

### 3. StepInfo Extended Fields Not Used by Markov Lua Generator

**Milestone:** M8 (Markov Jr. Generator)

**Current State:**
- `StepInfo` has `rule_name: Option<String>` and `affected_cells: Option<usize>`
- The current Markov Lua generator doesn't populate these fields
- They would require the Lua generator to extract info from `model:step()` results

**Proposed Change:** Consider adding Lua bindings that let generators emit richer step info:
```lua
ctx:set_step_info({
    rule_name = "WB -> WW",
    affected_cells = 5
})
```

**Why Refactor:** Enable visualizers to show rule-level information for Markov generators.

**Criticality:** **Low** - Extended fields are optional; base functionality works.

**When to Do:** Defer to M9/M10 or Phase 4 if visualizer needs this information.

---

### 4. Deferred from Phase 2: MCP Error Variant Unused

**Milestone:** M4.75 (Phase 2)

**Status:** Still deferred. No urgency.

---

### 5. Deferred from Phase 2: Duplicate PNG Rendering Code

**Milestone:** M5 (Phase 2)

**Status:** Still deferred. Legacy fallback path rarely used.

---

## Cleanup Decision Log

| Item | Decision | Rationale |
|------|----------|-----------|
| Lua-based instead of Rust adapter | Accepted | Simpler, more flexible, follows existing patterns |
| Generator type detection | Defer | Low criticality, complexity not worth it |
| StepInfo extended fields | Defer | Optional fields, base functionality works |
| MCP Error variant (P2) | Defer | Low criticality |
| Duplicate PNG code (P2) | Defer | Legacy fallback, works fine |

---

## Summary Statistics

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 0 | - |
| Medium | 0 | - |
| Low | 3 | All Deferred (1 M8, 2 Phase 2) |

**M8 Cleanup complete.** No blocking items. Proceed to M8.5.

---

## M8.5 Audit: Generator Scene Tree & Step Info Registry

### 1. Structure Field Not Returned by MCP Endpoint

**Milestone:** M8.5 (Generator Scene Tree)

**Spec Expected:**
```json
{
  "structure": {"type":"Sequential","path":"root","children":{...}},
  "steps": {"root.step_1": {...}}
}
```

**Actual Implementation:**
```json
{
  "type": "lua",
  "step": 1024,
  "steps": {"root.step_1": {...}}
}
```

**Why Deferred:** Getting the `structure` requires calling `generator:get_structure()` from Rust, which needs access to the Lua generator table. This adds complexity for marginal benefit - the `steps` HashMap already gives path information.

**Criticality:** **Low** - Path-keyed step info works; structure is nice-to-have for introspection.

**When to Do:** Defer to M9 polish if needed, or Phase 4 if MCP introspection becomes important.

---

### 2. MjModel Step() Doesn't Emit Step Info

**Milestone:** M8.5 (Generator Scene Tree)

**Current State:**
- `MjLuaModel` has `_set_path` and `_set_context` methods
- `MjLuaModel::step()` calls internal model step but doesn't emit step info
- Only Lua-based generators (Scatter, Fill) emit step info

**Proposed Change:** Have `MjLuaModel::step()` emit step info with:
- Path from `self.path`
- Step count from model counter
- Changed cells from `model:last_changes()`

**Why Refactor:** Would enable visualizers to see Markov step info with path.

**Criticality:** **Low** - Lua generators work; Markov emit is nice-to-have.

**When to Do:** Defer to M9 if visualizer needs Markov step info with paths.

---

### 3. Generator Base Class is Lua-Only

**Milestone:** M8.5 (Generator Scene Tree)

**Current State:**
- `Generator` base class exists in `assets/map_editor/lib/generator.lua`
- No Rust backing; scene tree is Lua-native

**Alternative:** Create Rust `GeneratorTree` that tracks paths and emits step info.

**Why Keep Lua-Only:**
- Simpler: No FFI complexity
- Follows existing patterns: All generators are Lua-based
- Can migrate later if needed

**Criticality:** **Low** - Current approach works; Rust backing would be over-engineering.

**When to Do:** Don't do unless we hit performance issues or need Rust type safety.

---

### 4. Path Separator is `.`

**Milestone:** M8.5 (Generator Scene Tree)

**Current State:** Paths use `.` separator: `"root.step_1.scatter"`

**Alternative:** Use `/` like file paths: `"root/step_1/scatter"`

**Why Keep `.`:**
- Consistent with Lua field access (`generator.children.step_1`)
- Already implemented and tested
- No clear benefit to changing

**Criticality:** **N/A** - Not a cleanup item, just a design decision.

---

## Cleanup Decision Log (Updated)

| Item | Decision | Rationale |
|------|----------|-----------|
| Lua-based instead of Rust adapter (M8) | Accepted | Simpler, more flexible, follows existing patterns |
| Generator type detection (M8) | Defer | Low criticality, complexity not worth it |
| StepInfo extended fields (M8) | Defer | Optional fields, base functionality works |
| MCP Error variant (P2) | Defer | Low criticality |
| Duplicate PNG code (P2) | Defer | Legacy fallback, works fine |
| Structure field in MCP (M8.5) | Defer | Steps HashMap sufficient for now |
| MjModel step info emit (M8.5) | Defer | Lua generators work; nice-to-have |
| Generator base class Lua-only (M8.5) | Keep | Works fine, Rust backing would over-engineer |

---

## Summary Statistics

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 0 | - |
| Medium | 0 | - |
| Low | 5 | All Deferred (2 M8, 2 M8.5, 1 Phase 2) |

**M8.5 Cleanup complete.** No blocking items. Ready for M8.75.

---

## M8.75 Audit: Generator Foundation in Rust

### 1. MjModel Step Info Emission - RESOLVED

**Milestone:** M8.75 (Generator Foundation in Rust)

**Previous Status:** MjModel did not emit step info.

**Current Status:** **RESOLVED** - `MjLuaModel::step()` now emits step info.

**Implementation:**
1. Added `last_step_changes()` and `last_step_change_count()` to `Interpreter` and `Model`
2. Modified `MjLuaModel::step()` to emit step info via context after each step:
   - Path from stored `_path`
   - Step number from `model.counter()`
   - Affected cells count from `model.last_step_change_count()`
   - Position (x, y) from first changed cell
   - Material from grid at changed position

**MCP Response Now Includes:**
```json
{
  "steps": {
    "root.step_1": {
      "step": 1,
      "x": 8,
      "y": 6,
      "material_id": 1,
      "completed": false,
      "affected_cells": 2
    }
  }
}
```

**Note:** `rule_name` is not included because the interpreter architecture doesn't track which specific rule fired. The `affected_cells` count is the key metric for visualizers.

---

### 2. Structure Field Now Returns Data

**Milestone:** M8.75 (Generator Foundation in Rust)

**Previous Status (M8.5):** Structure field was not returned, deferred.

**Current Status:** **RESOLVED** - M8.75 added `ActiveGenerator` resource and wired Lua generators to Rust implementations. The MCP endpoint now returns:
```json
{
  "structure": {
    "type": "Sequential",
    "path": "root",
    "children": {
      "step_1": {"type": "MjModel", "path": "root.step_1", "model_name": "step_1"},
      "step_2": {"type": "Scatter", "path": "root.step_2", "config": {...}}
    }
  }
}
```

**Implementation:**
- `lua_table_to_rust_generator()` in `lua_generator.rs` converts Lua tables to `Box<dyn Generator>`
- `MjGeneratorPlaceholder` provides structure for MjModel nodes
- `reload_generator()` stores converted generator in `ActiveGenerator`
- MCP endpoint calls `active_generator.structure()` directly

---

### 3. Hot Reload Works for Sequential Generators

**Milestone:** M8.75 (Generator Foundation in Rust)

**Verified:** Editing `generator.lua` and saving triggers hot reload:
- Config changes (e.g., scatter density) immediately reflected
- Structure is re-converted from Lua on reload
- MCP endpoint shows updated structure

---

### 4. Lua Generators Could Be Replaced by Rust

**Milestone:** M8.75 (Generator Foundation in Rust)

**Current State:**
- Rust has `Generator` trait with `SequentialGenerator`, `ParallelGenerator`, `ScatterGenerator`, `FillGenerator`, `MjGenerator`
- Lua still does actual execution via `lib/generators.lua`
- Rust implementations are used for structure introspection only

**Spec Vision:**
```lua
-- generators.lua now wraps Rust implementations
generators.sequential = function(children)
    return _G._rust_create_sequential(children)
end
```

**Why Keep Lua Execution:**
- Works well, users can add custom Lua logic
- No performance issues
- Changing would require significant refactoring

**Criticality:** **Low** - Current approach works. Full Rust execution is a future optimization if needed.

**When to Do:** Not recommended unless performance becomes an issue.

---

## Cleanup Decision Log (Updated)

| Item | Decision | Rationale |
|------|----------|-----------|
| Lua-based instead of Rust adapter (M8) | Accepted | Simpler, more flexible, follows existing patterns |
| Generator type detection (M8) | Defer | Low criticality, complexity not worth it |
| StepInfo extended fields (M8) | **Resolved** | MjModel now emits affected_cells; rule_name not feasible with current architecture |
| MCP Error variant (P2) | Defer | Low criticality |
| Duplicate PNG code (P2) | Defer | Legacy fallback, works fine |
| Structure field in MCP (M8.5) | **Resolved in M8.75** | Now returns full structure tree |
| MjModel step info emit (M8.5/M8.75) | **Resolved** | MjLuaModel::step() now emits step info |
| Generator base class Lua-only (M8.5) | Keep | Works fine, Rust backing for structure only |
| Full Rust generator execution (M8.75) | Defer | Low priority, Lua execution works well |

---

## Summary Statistics

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 0 | - |
| Medium | 0 | - |
| Low | 3 | Deferred (generator type detection, MCP error variant, duplicate PNG) |
| Resolved | 3 | Structure field, MjModel step info, StepInfo extended fields |

**M8.75 Complete.** All critical items resolved. Ready for M9.

**Note:** Markov Jr. internal visibility limitations are addressed in Phase 3.5, not deferred indefinitely. See [PHASE_3_5_SPEC.md](./PHASE_3_5_SPEC.md) for the plan.

---

## M8.75 Verification Checklist (Updated)

Per the spec, here's what was completed:

| Item | Status | Notes |
|------|--------|-------|
| `Generator` trait exists in `generator/traits.rs` | **Done** | With `structure()`, `init()`, `step()`, etc. |
| `GeneratorStructure` struct is serializable | **Done** | Uses serde with skip_serializing_if |
| `SequentialGenerator` implements `Generator` | **Done** | In `generator/sequential.rs` |
| `ParallelGenerator` implements `Generator` | **Done** | In `generator/parallel.rs` |
| `ScatterGenerator` implements `Generator` | **Done** | In `generator/scatter.rs` |
| `FillGenerator` implements `Generator` | **Done** | In `generator/fill.rs` |
| `MjGenerator` implements `Generator` | **Done** | `MjGeneratorPlaceholder` for structure; `MjLuaModel` for execution+step info |
| `MjGenerator` emits step info with rule names | **Partial** | Emits affected_cells; rule_name not available from interpreter |
| `MjGenerator` reports affected cell count | **Done** | Added `last_step_change_count()` to Interpreter/Model |
| `GET /mcp/generator_state` returns `structure` field | **Done** | Full tree returned |
| All step info includes emitting generator's path | **Done** | MjModel, Scatter, Fill all emit with path |
| Lua `generators.sequential()` wraps Rust | **Partial** | Lua executes, Rust for structure |
| Lua `mj.load_model()` returns Rust-backed `MjGenerator` | **Partial** | Returns MjLuaModel which emits step info |
| 84 tests pass | **Done** | `cargo test -p studio_core map_editor::` |

### Note on rule_name and Internal Structure

The `rule_name` field is not populated and internal Markov Jr. structure is opaque because:
1. The interpreter architecture doesn't track which specific node/rule fired during a step
2. Rules are executed through a tree of nodes (MarkovNode, SequenceNode, OneNode, etc.)
3. The "rule" concept is spread across the node hierarchy, not exposed as a single string
4. Path tracking doesn't exist in the current `ExecutionContext`

**This is addressed in Phase 3.5 (Markov Jr. Introspection).** See [PHASE_3_5_SPEC.md](./PHASE_3_5_SPEC.md).

Phase 3.5 adds:
- M10.4: Multi-surface rendering foundation + video export
- M10.5: `Node::structure()` for introspecting the Markov node tree
- M10.6: Path tracking in `ExecutionContext` for per-node step info
- M10.7: Budget-controlled stepping for fine-grained control
- M10.8: Dedicated visualizer that shows the node tree and active rules

---

## M10.4 Audit: Multi-Surface Rendering Foundation

### 1. Multi-Surface Rendering Infrastructure - IMPLEMENTED

**Milestone:** M10.4 (Multi-Surface Rendering Foundation)

**Implementation:**
1. Created `RenderSurface` struct with buffer and layer stack (`render/surface.rs`)
2. Created `RenderSurfaceManager` resource to manage multiple surfaces
3. Created `SurfaceLayout` enum for compositing (Single, Horizontal, Vertical, Grid)
4. Created `FrameCapture` struct for video export (`render/frame_capture.rs`)
5. Updated MCP server with new endpoints:
   - `GET /mcp/surfaces` - List all surfaces and layout
   - `GET /mcp/get_output?surface=grid` - Render specific surface
   - `POST /mcp/start_recording` - Start frame capture
   - `POST /mcp/stop_recording` - Stop frame capture
   - `POST /mcp/export_video` - Export to video file

**Files Created:**
- `crates/studio_core/src/map_editor/render/surface.rs`
- `crates/studio_core/src/map_editor/render/frame_capture.rs`

**Files Modified:**
- `crates/studio_core/src/map_editor/render/mod.rs` - Added exports
- `crates/studio_core/src/map_editor/mod.rs` - Added exports
- `crates/studio_core/src/map_editor/mcp_server.rs` - Added new endpoints
- `crates/studio_core/src/map_editor/app.rs` - Added resources

**Criticality:** **N/A** - New feature, not cleanup item.

---

### 2. Surface Manager vs Render Stack - Design Decision

**Milestone:** M10.4 (Multi-Surface Rendering Foundation)

**Current State:**
- Both `RenderLayerStack` and `RenderSurfaceManager` exist
- MCP `get_output` prefers `RenderSurfaceManager` if available, falls back to `RenderLayerStack`
- App initializes both: `RenderLayerStack` for backward compatibility, `RenderSurfaceManager` for new features

**Design Decision:**
- Keep both during transition period
- `RenderLayerStack` will be deprecated in favor of `RenderSurfaceManager` once all layers migrate to surfaces
- Each surface in `RenderSurfaceManager` has its own layer stack

**Criticality:** **Low** - Works fine, can unify later.

**When to Do:** During Phase 4 or when adding second surface (mj_structure).

---

### 3. FrameCapture Not Integrated with Generation Loop

**Milestone:** M10.4 (Multi-Surface Rendering Foundation)

**Current State:**
- `FrameCapture` is a resource that can be controlled via MCP
- Frames must be captured manually via API calls
- No automatic capture on each generation step

**Proposed Enhancement:**
- Add system that automatically captures frame after each generation step when recording
- Integrate with playback state for synchronized recording

**Criticality:** **Low** - API works, automatic capture is nice-to-have.

**When to Do:** When implementing video export workflow in M10.8.

---

## Cleanup Decision Log (Updated)

| Item | Decision | Rationale |
|------|----------|-----------|
| Lua-based instead of Rust adapter (M8) | Accepted | Simpler, more flexible, follows existing patterns |
| Generator type detection (M8) | Defer | Low criticality, complexity not worth it |
| StepInfo extended fields (M8) | **Resolved** | MjModel now emits affected_cells; rule_name not feasible with current architecture |
| MCP Error variant (P2) | Defer | Low criticality |
| Duplicate PNG code (P2) | Defer | Legacy fallback, works fine |
| Structure field in MCP (M8.5) | **Resolved in M8.75** | Now returns full structure tree |
| MjModel step info emit (M8.5/M8.75) | **Resolved** | MjLuaModel::step() now emits step info |
| Generator base class Lua-only (M8.5) | Keep | Works fine, Rust backing for structure only |
| Full Rust generator execution (M8.75) | Defer | Low priority, Lua execution works well |
| Surface Manager vs Render Stack (M10.4) | Defer | Keep both during transition, unify later |
| FrameCapture auto-capture (M10.4) | Defer | API works, automatic is nice-to-have |

---

## Summary Statistics (Phase 3.5)

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 2 | Proposed unit tests (M10.4), **MjGenerator/MjGeneratorPlaceholder duplication (M10.5) - DO NOW** |
| Medium | 1 | Duplicate structure representations (MjNodeStructure vs GeneratorStructure) |
| Low | 7 | generator type detection, MCP error variant, duplicate PNG, surface/stack unify, auto-capture, JSON round-trip, characters duplication |

---

## M10.5 Audit: Markov Jr. Structure Introspection

### 1. Duplicate Structure Representations

**Milestone:** M10.5

**Current State:**
- `MjNodeStructure` in `markov_junior/node.rs` - represents MJ internal node tree
- `GeneratorStructure` in `map_editor/generator/traits.rs` - represents Lua generator tree

These are two different struct types that serve similar purposes. `GeneratorStructure` now has an `mj_structure: Option<MjNodeStructure>` field to embed MJ structure inside generator structure.

**Issue:** We now have nested structure types. If we want to unify visualizers later, we may need a single unified tree representation.

**Criticality:** **Medium** - Works but creates conceptual overhead when reasoning about "structure". Not blocking, but confusing.

**When to Do:** Consider unifying in Phase 4 if we build a combined visualizer.

---

### 2. MjGenerator vs MjGeneratorPlaceholder Duplication (CRITICAL)

**Milestone:** M10.5

**Current State - THREE separate MJ-related types:**

| Type | Location | Purpose | Actually Used? |
|------|----------|---------|----------------|
| `MjGenerator` | `generator/markov.rs` | Full Rust generator with `init/step/reset` | **NO** - orphaned code |
| `MjGeneratorPlaceholder` | `lua_generator.rs` | Fake generator for introspection only | Yes - for MCP structure |
| `MjLuaModel` | `markov_junior/lua_api.rs` | Lua userdata wrapping `Model` | Yes - execution path |

**The Problem:**

1. `MjGenerator` is a complete, tested implementation that is **never instantiated**
2. `MjGeneratorPlaceholder` exists because Lua owns the model, so Rust can't execute it
3. `MjLuaModel` is the actual execution path, but it's not a `Generator`

This creates:
- **Confusion:** Which is the "real" MJ generator? Answer: none of them do both jobs
- **Bloat:** 200+ lines of dead code in `markov.rs` 
- **Inconsistency:** `MjGenerator::structure()` was missing `mj_structure` (just fixed), but it doesn't matter because it's never used
- **Maintenance burden:** Changes must consider all three types

**Root Cause:**

The architecture evolved backwards:
1. M8: Made MJ callable from Lua (`MjLuaModel` userdata owns `Model`)
2. M8.75: Needed Rust `Generator` trait for MCP → created `MjGenerator`
3. M10.5: Couldn't extract `Model` from Lua → created `MjGeneratorPlaceholder`

**Correct Architecture:**

```
Current (inverted):
  Lua owns Model → Rust has placeholder for introspection

Better:
  Rust owns Model → Lua has thin wrapper for scripting
```

**Criticality:** **HIGH** - Three types doing partial jobs creates confusion, bloat, and maintenance burden.

**Resolution:** See cleanup spec below.

---

### 3. JSON Round-Trip for mj_structure

**Milestone:** M10.5

**Current State:**
```rust
// lua_generator.rs
if let Ok(json_str) = ud.call_method::<String>("mj_structure", ()) {
    if let Ok(mj_struct) = serde_json::from_str::<MjNodeStructure>(&json_str) {
        // ...
    }
}
```

**Issue:** We serialize `MjNodeStructure` to JSON in Lua, then deserialize it back in Rust. This is inefficient - we're crossing the Lua/Rust boundary twice with serialization overhead.

**Alternative:** Could pass the structure directly as a Lua table and convert to Rust struct without JSON.

**Criticality:** **Low** - Only happens once during generator reload, not per-step. Negligible performance impact.

---

### 4. characters Field Added to RuleNodeData

**Milestone:** M10.5

**Current State:**
- Added `characters: Vec<char>` to `RuleNodeData`
- Set by loader: `data.characters = grid.characters.clone();`
- Used by `MjRule::to_display_string()` for human-readable rules

**Issue:** This duplicates data that already exists in the grid. Every rule node now carries a copy of the character array.

**Criticality:** **Low** - Small memory overhead. Could pass characters as parameter instead of storing, but would require changing all `structure()` signatures.

---

## M10.4 Post-Mortem: Critical Bug Fix & Proposed Tests

### Bug Summary

Two critical bugs in MjLuaModel Lua integration prevented MazeGrowth from rendering:

1. **Missing `init()` method**: When `Sequential:init(ctx)` called `child:init(ctx)` on MjLuaModel children, nothing happened because MjLuaModel lacked an `init` method. The MJ model was never reset before stepping.

2. **Inverted `step()` return value**: MjLuaModel.step() returned `true` when progress was made, but the generator protocol expects `true` when **done**. This caused Sequential to skip to the next child after just 1 step.

### Why These Bugs Went Undetected

- No unit tests for MjLuaModel Lua integration
- No integration tests for Sequential + MjLuaModel composition
- Only tested via full example app, which has too many layers to debug

### Proposed Tests

These tests would have caught the bugs without running the full example:

#### 1. MjLuaModel.init() Unit Test

```rust
#[test]
fn test_mj_lua_model_init_resets_model() {
    let lua = Lua::new();
    register_markov_junior_api(&lua).unwrap();
    
    // Load a simple model
    let result: mlua::AnyUserData = lua.load(r#"
        local model = mj.load_model_xml("MarkovJunior/models/MazeGrowth.xml", {size=8})
        return model
    "#).eval().unwrap();
    
    // Create mock context with seed
    let ctx = lua.create_table().unwrap();
    ctx.set("seed", 42u64).unwrap();
    ctx.set("width", 8).unwrap();
    ctx.set("height", 8).unwrap();
    
    // Call init
    result.call_method::<()>("init", ctx.clone()).unwrap();
    
    // Model should be running after init
    let is_done: bool = result.call_method("is_done", ()).unwrap();
    assert!(!is_done, "Model should be running after init()");
}
```

#### 2. MjLuaModel.step() Return Value Test

```rust
#[test]
fn test_mj_lua_model_step_returns_done_not_progress() {
    let lua = Lua::new();
    register_markov_junior_api(&lua).unwrap();
    
    // Load and init model
    let model: mlua::AnyUserData = lua.load(r#"
        local model = mj.load_model_xml("MarkovJunior/models/MazeGrowth.xml", {size=4})
        return model
    "#).eval().unwrap();
    
    let ctx = lua.create_table().unwrap();
    ctx.set("seed", 42u64).unwrap();
    model.call_method::<()>("init", ctx).unwrap();
    
    // First step should return false (not done yet)
    let step1_result: bool = model.call_method("step", ()).unwrap();
    assert!(!step1_result, "step() should return false while model is running");
    
    // Run to completion
    loop {
        let done: bool = model.call_method("step", ()).unwrap();
        if done { break; }
    }
    
    // After completion, is_done should be true
    let is_done: bool = model.call_method("is_done", ()).unwrap();
    assert!(is_done, "is_done() should be true after model completes");
}
```

#### 3. Sequential + MjLuaModel Integration Test

```rust
#[test]
fn test_sequential_with_mj_model_runs_to_completion() {
    let lua = Lua::new();
    register_markov_junior_api(&lua).unwrap();
    setup_generator_lib(&lua).unwrap();
    
    // Create sequential with MJ model
    let generator: mlua::Table = lua.load(r#"
        local generators = require("lib.generators")
        return generators.sequential({
            mj.load_model_xml("MarkovJunior/models/MazeGrowth.xml", {size=4})
        })
    "#).eval().unwrap();
    
    // Create mock context
    let ctx = create_mock_context(&lua, 4, 4, 42);
    
    // Init
    generator.call_method::<()>("init", ctx.clone()).unwrap();
    
    // Step through - should take more than 1 step for 4x4 maze
    let mut step_count = 0;
    loop {
        let done: bool = generator.call_method("step", ctx.clone()).unwrap();
        step_count += 1;
        if done { break; }
        assert!(step_count < 1000, "Should complete within 1000 steps");
    }
    
    // Should have taken multiple steps, not just 1
    assert!(step_count > 1, "MJ model should take multiple steps, got {}", step_count);
}
```

#### 4. Voxel Buffer Population Test

```rust
#[test]
fn test_mj_model_populates_buffer_after_completion() {
    let lua = Lua::new();
    register_markov_junior_api(&lua).unwrap();
    setup_generator_lib(&lua).unwrap();
    
    // Create context with real buffer
    let buffer = SharedBuffer::new(4, 4);
    let ctx = create_context_with_buffer(&lua, buffer.clone(), 42);
    
    // Create and run generator
    let generator: mlua::Table = lua.load(r#"
        local generators = require("lib.generators")
        return generators.sequential({
            mj.load_model_xml("MarkovJunior/models/MazeGrowth.xml", {size=4})
        })
    "#).eval().unwrap();
    
    generator.call_method::<()>("init", ctx.clone()).unwrap();
    loop {
        let done: bool = generator.call_method("step", ctx.clone()).unwrap();
        if done { break; }
    }
    
    // Buffer should have non-zero values (maze pattern)
    let non_empty = buffer.count_non_zero();
    assert!(non_empty > 0, "Buffer should have filled cells after maze generation");
    // MazeGrowth should fill most cells (walls + paths)
    assert!(non_empty >= 12, "4x4 maze should have at least 12 non-empty cells, got {}", non_empty);
}
```

### Recommended Test Infrastructure

1. **Add `#[cfg(test)]` module to `lua_api.rs`** with unit tests for each MjLuaModel method
2. **Add integration test file** `tests/lua_mj_integration.rs` for Sequential/Parallel + MjLuaModel
3. **Add `SharedBuffer::count_non_zero()` helper** for test assertions
4. **Create `create_mock_context()` test helper** that builds a valid Lua context

### Priority

**High** - These tests would have saved hours of debugging and prevented shipping a broken feature.

---

## Cleanup Spec: MJ Generator Unification

**Goal:** Eliminate `MjGeneratorPlaceholder` and make `MjGenerator` the single source of truth.

### Current Architecture

```
Lua calls mj.load_model_xml()
    ↓
Returns MjLuaModel userdata (owns Model)
    ↓
Lua calls model:step() → executes via Model
    ↓
For MCP structure: lua_generator.rs creates MjGeneratorPlaceholder
    ↓
Placeholder calls Lua mj_structure() → JSON → MjNodeStructure
    ↓
MjGenerator in markov.rs sits unused
```

### Target Architecture

```
Lua calls mj.load_model_xml()
    ↓
Returns MjLuaModel userdata (holds Rc<RefCell<MjGenerator>>)
    ↓
MjGenerator owns Model, implements Generator trait
    ↓
Lua calls model:step() → MjLuaModel delegates to MjGenerator
    ↓
For MCP structure: lua_generator.rs gets MjGenerator directly from userdata
    ↓
MjGeneratorPlaceholder deleted - no longer needed
```

### Key Changes

1. **MjLuaModel holds `Rc<RefCell<MjGenerator>>` instead of `Rc<RefCell<Model>>`**
   - MjGenerator already owns Model
   - MjLuaModel becomes a thin Lua wrapper

2. **MjLuaModel methods delegate to MjGenerator**
   - `init(ctx)` → extract seed, call `generator.init(&mut rust_ctx)`
   - `step()` → call `generator.step(&mut rust_ctx)`, copy buffer to Lua ctx
   - `is_done()` → `generator.is_done()`
   - Structure comes from `generator.structure()` directly

3. **lua_generator.rs extracts MjGenerator from userdata**
   - Add method to MjLuaModel: `get_generator() -> Rc<RefCell<MjGenerator>>`
   - `value_to_generator()` clones the Rc and returns it as `Box<dyn Generator>`

4. **Delete MjGeneratorPlaceholder entirely**

### Implementation Tasks

| # | Task | File | Verification |
|---|------|------|--------------|
| 1 | Add `MjGenerator::new_with_path()` or setter | `generator/markov.rs` | Compiles |
| 2 | Change `MjLuaModel.inner` type to `Rc<RefCell<MjGenerator>>` | `lua_api.rs` | Compiles |
| 3 | Update `mj.load_model_xml()` to create `MjGenerator` | `lua_api.rs` | Compiles |
| 4 | Update `MjLuaModel::step()` to use MjGenerator | `lua_api.rs` | Tests pass |
| 5 | Add `MjLuaModel::get_generator()` method | `lua_api.rs` | Compiles |
| 6 | Update `value_to_generator()` to use real generator | `lua_generator.rs` | Structure returned |
| 7 | Delete `MjGeneratorPlaceholder` | `lua_generator.rs` | Compiles |
| 8 | Verify MCP returns `mj_structure` | Manual | `curl` shows structure |
| 9 | Run all tests | - | `cargo test` passes |

### Verification

```bash
# 1. Build succeeds
cargo build --example p_map_editor_2d

# 2. Tests pass
cargo test -p studio_core

# 3. MCP returns mj_structure
cargo run --example p_map_editor_2d &
sleep 6
curl -s http://127.0.0.1:8088/mcp/generator_state | jq '.structure.children.step_1.mj_structure.node_type'
# Expected: "Markov"

# 4. Generation still works (visual verification)
# - App shows generated maze
# - Stepping works

pkill -f p_map_editor_2d
```

### Risk Assessment

**Risk:** Breaking Lua execution path
**Mitigation:** Keep `MjLuaModel` API identical, only change internals

**Risk:** Breaking MCP introspection
**Mitigation:** Verify with curl after each step

### Estimated Time

2-3 hours

### Decision

**SUPERSEDED** - See new cleanup spec below for VoxelGrid trait approach.

---

## M10.5 Cleanup Spec v2: VoxelGrid Trait (Zero-Copy Architecture)

**Date:** 2026-01-19  
**Status:** PROPOSED - Awaiting approval  
**Supersedes:** MjGenerator/MjGeneratorPlaceholder unification spec above

### Problem Statement

The current architecture has THREE buffer copies:

```
MjGrid (MJ internal) → SharedBuffer (Lua intermediate) → VoxelBuffer2D (Bevy resource)
```

The previous "fix" (MjLuaModel wrapping MjGenerator) would reduce to TWO copies but misses the deeper issue: **why copy at all?**

The grid→buffer copy does a **semantic translation**: MJ grid values (0,1,2...) map to material IDs via character lookup. This translation must happen, but it doesn't need to be a copy.

### Core Insight

**Translation can happen on read, not on copy.**

If we define a trait for "something you can read voxels from," then:
- `VoxelBuffer2D` implements it directly
- `MjGrid` implements it with translation-on-read
- Renderers read from the trait, not concrete types
- Zero copies. Translation happens lazily.

### The VoxelGrid Trait

```rust
/// Trait for anything that provides voxel/material data.
/// 
/// Both VoxelBuffer2D and MJ grids implement this.
/// Renderers and MCP endpoints read from this trait.
pub trait VoxelGrid2D {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    /// Get material ID at position. Returns 0 if out of bounds.
    fn get(&self, x: usize, y: usize) -> u32;
}

/// For 3D (Phase 5):
pub trait VoxelGrid3D {
    fn size(&self) -> (usize, usize, usize);
    fn get(&self, x: usize, y: usize, z: usize) -> u32;
}
```

### Implementation for VoxelBuffer2D

```rust
impl VoxelGrid2D for VoxelBuffer2D {
    fn width(&self) -> usize { self.width }
    fn height(&self) -> usize { self.height }
    fn get(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.data[y * self.width + x]
        } else {
            0
        }
    }
}
```

### Implementation for MjGrid (with translation)

```rust
/// View into MjGrid that translates values to material IDs.
pub struct MjGridView<'a> {
    grid: &'a MjGrid,
    /// Maps MJ grid values (0,1,2...) to material IDs
    value_to_material: Vec<u32>,
}

impl<'a> MjGridView<'a> {
    pub fn new(grid: &'a MjGrid, char_to_material: &HashMap<char, u32>) -> Self {
        // Pre-compute value→material mapping from grid.characters
        let value_to_material: Vec<u32> = grid.characters
            .iter()
            .enumerate()
            .map(|(i, &ch)| {
                char_to_material.get(&ch).copied().unwrap_or(i as u32 + 1)
            })
            .collect();
        
        Self { grid, value_to_material }
    }
}

impl VoxelGrid2D for MjGridView<'_> {
    fn width(&self) -> usize { self.grid.mx }
    fn height(&self) -> usize { self.grid.my }
    
    fn get(&self, x: usize, y: usize) -> u32 {
        let val = self.grid.get(x, y, 0).unwrap_or(0) as usize;
        self.value_to_material.get(val).copied().unwrap_or(0)
    }
}
```

### Updated Architecture

```
BEFORE (3 copies):
┌────────┐    ┌──────────────┐    ┌──────────────┐
│MjGrid  │───►│SharedBuffer  │───►│VoxelBuffer2D │
│        │copy│(Arc<Mutex>)  │copy│(Bevy)        │
└────────┘    └──────────────┘    └──────────────┘

AFTER (0 copies, translation on read):
┌────────────────────────────────────────────────┐
│              trait VoxelGrid2D                 │
│  fn width(), fn height(), fn get(x,y) -> u32  │
└────────────────────┬───────────────────────────┘
                     │
        ┌────────────┴────────────┐
        │                         │
        ▼                         ▼
┌──────────────────┐     ┌──────────────────┐
│  VoxelBuffer2D   │     │   MjGridView     │
│                  │     │                  │
│  impl VoxelGrid  │     │  impl VoxelGrid  │
│  (direct)        │     │  (translates on  │
│                  │     │   read)          │
└──────────────────┘     └────────┬─────────┘
                                  │ holds ref
                                  ▼
                         ┌──────────────────┐
                         │     MjGrid       │
                         │  + char mapping  │
                         └──────────────────┘
```

### What Uses VoxelGrid2D

| Consumer | Current | After |
|----------|---------|-------|
| `RenderContext` | `buffer: &VoxelBuffer2D` | `grid: &dyn VoxelGrid2D` |
| `RenderLayer::render()` | reads from buffer | reads from trait |
| MCP `get_output` | reads from buffer | reads from trait |
| MCP `generator_state` | reads step info | unchanged |

### Generator Changes

**Option C from discussion: All generators produce `Box<dyn VoxelGrid2D>`**

```rust
pub trait Generator {
    // ... existing methods ...
    
    /// Get the current grid state for rendering.
    /// Returns None if generator hasn't produced output yet.
    fn grid(&self) -> Option<&dyn VoxelGrid2D>;
}
```

For generators:

| Generator | `grid()` returns |
|-----------|------------------|
| `MjGenerator` | `Some(&MjGridView)` |
| `ScatterGenerator` | `None` (writes to shared buffer) |
| `FillGenerator` | `None` (writes to shared buffer) |
| `SequentialGenerator` | Delegates to active child |

**Key insight:** MJ generators own their grid. Lua generators (Scatter, Fill) write to a shared buffer. Both work through the same trait.

### SharedBuffer Still Needed (For Lua Generators)

Pure Lua generators call `ctx:set_voxel()`. They need a buffer to write to. But:
- `SharedBuffer` implements `VoxelGrid2D`
- When active generator is Lua-based, render from `SharedBuffer`
- When active generator is MJ-based, render from `MjGridView`
- Renderer doesn't care which

### ActiveGenerator Changes

```rust
pub struct ActiveGenerator {
    /// Structure for MCP introspection
    structure: Option<GeneratorStructure>,
    /// Current grid for rendering (trait object)
    grid: Option<Box<dyn VoxelGrid2D>>,
}
```

Or simpler: just track which generator is active, call `generator.grid()` when rendering.

### Bevy Integration

The tricky part: `RenderContext` is created per-frame with a reference to the grid. Who owns the grid?

**Current:** `VoxelBuffer2D` is a Bevy `Resource`.

**After:** Need to handle two cases:
1. Lua generators → `SharedBuffer` (or keep `VoxelBuffer2D`) as resource
2. MJ generators → `MjGridView` created on-demand from `MjGenerator`

**Solution:** `RenderContext` takes `&dyn VoxelGrid2D`:

```rust
pub struct RenderContext<'a> {
    pub grid: &'a dyn VoxelGrid2D,
    pub palette: &'a MaterialPalette,
}

// In render system:
fn render_system(
    buffer: Res<VoxelBuffer2D>,
    active_gen: Option<NonSend<ActiveGenerator>>,
    // ...
) {
    // Get grid from active generator if it has one, else use buffer
    let grid: &dyn VoxelGrid2D = active_gen
        .and_then(|g| g.grid())
        .unwrap_or(&*buffer);
    
    let ctx = RenderContext::new(grid, &palette);
    // ... render
}
```

### Implementation Tasks

| # | Task | File | Verification |
|---|------|------|--------------|
| 1 | Create `VoxelGrid2D` trait | `map_editor/voxel_buffer_2d.rs` or new file | Compiles |
| 2 | Impl `VoxelGrid2D` for `VoxelBuffer2D` | `voxel_buffer_2d.rs` | Tests pass |
| 3 | Create `MjGridView` struct | `markov_junior/grid_view.rs` | Compiles |
| 4 | Impl `VoxelGrid2D` for `MjGridView` | `grid_view.rs` | Tests pass |
| 5 | Update `RenderContext` to use trait | `render/mod.rs` | Compiles |
| 6 | Update all `RenderLayer` impls | Various | Compiles |
| 7 | Add `grid()` method to `Generator` trait | `generator/traits.rs` | Compiles |
| 8 | Impl `grid()` for `MjGenerator` | `generator/markov.rs` | Returns view |
| 9 | Update render system to get grid from generator | `lua_generator.rs` or `app.rs` | Renders |
| 10 | Update MCP `get_output` to use trait | `mcp_server.rs` | PNG works |
| 11 | Delete `SharedBuffer` copy-to-buffer code | `lua_generator.rs` | Compiles |
| 12 | Run full test suite | - | All pass |

### Verification

```bash
# 1. Build succeeds
cargo build --example p_map_editor_2d

# 2. Tests pass  
cargo test -p studio_core

# 3. MJ generator renders correctly
cargo run --example p_map_editor_2d &
sleep 6
curl -s http://127.0.0.1:8088/mcp/get_output -o /tmp/output.png
# Verify PNG shows maze pattern

# 4. Lua generator still works
# Edit generator.lua to use Scatter, verify it renders

# 5. MCP structure still works
curl -s http://127.0.0.1:8088/mcp/generator_state | jq '.structure'

pkill -f p_map_editor_2d
```

### What Gets Deleted

- `SharedBuffer::copy_to_buffer()` - no longer needed
- `MjLuaModel` grid copying in `step()` - replaced by view
- `MjStructureHolder` - can use real generator
- Possibly `MjGeneratorPlaceholder` - depends on final design

### What Gets Kept

- `SharedBuffer` - still needed for Lua generators that call `ctx:set_voxel()`
- `VoxelBuffer2D` - backward compatibility, implements trait
- `MjGenerator` - now actually used, provides `grid()` via view

### Future-Proofing for 3D (Phase 5)

The pattern extends naturally:

```rust
pub trait VoxelGrid3D {
    fn size(&self) -> (usize, usize, usize);
    fn get(&self, x: usize, y: usize, z: usize) -> u32;
}

impl VoxelGrid3D for VoxelBuffer3D { ... }
impl VoxelGrid3D for MjGridView3D { ... }
```

Same zero-copy architecture. Same trait-based rendering. The API doesn't change.

### Risk Assessment

**Risk:** Breaking existing Lua generators  
**Mitigation:** `SharedBuffer` still exists, Lua still writes to it, but now it implements `VoxelGrid2D`

**Risk:** Performance regression from virtual dispatch  
**Mitigation:** One virtual call per pixel read. Profile if concerned, but unlikely to matter.

**Risk:** Lifetime complexity with `MjGridView`  
**Mitigation:** View is created on-demand in render system, lives for duration of render call only

### Estimated Time

4-6 hours

### Decision

**PROPOSED** - This is the correct abstraction that:
1. Eliminates buffer copies (0 instead of 2-3)
2. Creates reusable `VoxelGrid2D` trait for Phase 5
3. Makes MjGenerator the single source of truth
4. Keeps Lua generators working unchanged

Awaiting approval before implementation.

