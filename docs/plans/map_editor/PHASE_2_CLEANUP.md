# Phase 2 Cleanup Audit

This document tracks refactoring candidates identified during Phase 2 milestone work. Each entry describes existing code that could benefit from new abstractions introduced in this phase.

**Purpose:** Capture tech debt opportunities as we go, without blocking milestone progress. Review at phase end to decide what to address before Phase 3.

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

## M4.5 Audit: Asset Trait + Search

### 1. ~~MaterialPalette.available → InMemoryStore<Material>~~ ✅ COMPLETED

**Milestone:** M4.5 (Asset Trait + Search)

**Resolution:** Completed during M4.5 cleanup. `MaterialPalette.available` now uses `InMemoryStore<Material>`. Added convenience methods to `MaterialPalette`:
- `search(query)` - delegates to `available.search()`
- `has_material(id)` - delegates to `available.any()`
- `add_material(mat)` - delegates to `available.set()`
- `get_by_id_mut(id)` - delegates to `available.find_mut()`

---

### 2. ~~Search Implementation Inconsistency~~ ✅ COMPLETED

**Milestone:** M4.5 (Asset Trait + Search)

**Resolution:** Resolved as part of cleanup item #1. Both `app.rs` (UI search) and `mcp_server.rs` (MCP search endpoint) now use `palette.search(query)` instead of manual filtering.

---

### 3. Future Asset Candidates

**Milestone:** M4.5 (Asset Trait + Search)

The following types don't currently implement `Asset` but might benefit from it in future milestones:

| Type | Current Location | Likely Milestone | Notes |
|------|------------------|------------------|-------|
| Generator definitions | `lua_generator.rs` | M6 | Currently just loads one file; no store needed yet |
| Renderer definitions | (not yet created) | M5 | Will need when we have multiple renderers |
| Visualizer definitions | (not yet created) | M6 | Will need when we have multiple visualizers |

**Why Track:**
These aren't cleanup items yet—the code doesn't exist. But knowing that these will likely implement `Asset` helps us design M5/M6 correctly from the start.

**Criticality:** **N/A** (forward-looking, not current debt)

**When to Do:** Design into M5/M6 from the beginning, not as refactor.

---

## Cleanup Decision Log

| Item | Decision | Rationale |
|------|----------|-----------|
| MaterialPalette → InMemoryStore | ✅ Done during M4.5 | Consistency with Asset trait pattern; enables unified search |
| Search inconsistency | ✅ Done during M4.5 | Naturally resolved by item #1 |

---

## Summary Statistics

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 0 | - |
| Medium | 1 | ✅ Completed |
| Low | 1 | ✅ Completed |

**M4.5 Cleanup complete.** All identified items resolved. Proceed to M4.75.
