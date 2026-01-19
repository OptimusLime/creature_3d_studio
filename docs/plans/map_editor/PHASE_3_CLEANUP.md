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

**M8 Cleanup complete.** No blocking items. Proceed to M9.
