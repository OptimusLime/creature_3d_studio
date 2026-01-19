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
| High | 0 | - |
| Medium | 0 | - |
| Low | 5 | Deferred (generator type detection, MCP error variant, duplicate PNG, surface/stack unify, auto-capture) |
| Resolved | 3 | Structure field, MjModel step info, StepInfo extended fields |

**M10.4 Complete.** Multi-surface rendering foundation in place. Ready for M10.5.

