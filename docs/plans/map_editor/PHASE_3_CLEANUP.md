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

**M8.5 Cleanup complete.** No blocking items. Ready for M9.
