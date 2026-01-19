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

---

## M4.75 Audit: Generator on Launch + MCP Set Endpoints

### 1. MCP Error Response Variant Unused

**Milestone:** M4.75 (Generator on Launch + MCP Set)

**Current State:**
```rust
enum McpResponse {
    // ... other variants
    Error(String),  // Never constructed - errors returned via error_response()
}
```

**Why:** The `Error` variant was added during initial MCP design but is never used. Error responses are handled by the HTTP layer via `error_response()` helper.

**Proposed Change:** Remove the unused variant, or use it consistently for error handling.

**Criticality:** **Low** - Generates compiler warning but doesn't affect functionality.

**When to Do:** Can be addressed when adding more MCP error handling in future milestones.

---

### 2. No Cleanup Items Identified

M4.75 was a small, focused milestone:
- Changed `PlaybackState` default to `playing: true`
- Added `PlaybackState::restart()` method
- Added two MCP endpoints that write files

No new abstractions were introduced that could be applied to existing code. The file-writing pattern is straightforward and doesn't warrant abstraction yet (only two similar endpoints).

---

## Cleanup Decision Log

| Item | Decision | Rationale |
|------|----------|-----------|
| MaterialPalette → InMemoryStore | ✅ Done during M4.5 | Consistency with Asset trait pattern; enables unified search |
| Search inconsistency | ✅ Done during M4.5 | Naturally resolved by item #1 |
| MCP Error variant | Defer | Low criticality, may be useful in future |

---

## Summary Statistics

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 0 | - |
| Medium | 1 | ✅ Completed (M4.5) |
| Low | 2 | ✅ 1 Completed (M4.5), 1 Deferred (M4.75) |

**M4.75 Cleanup complete.** No blocking items. Proceed to M5.

---

## M5 Audit: Lua Renderer with Layer System

### 1. Duplicate PNG Rendering Code

**Milestone:** M5 (Render Layer System)

**Current State:**
```rust
// mcp_server.rs has two rendering paths:
fn generate_png_from_buffer(...) -> Vec<u8>  // Legacy, duplicates BaseRenderLayer logic
fn encode_png(pixels: &PixelBuffer) -> Vec<u8>  // New, uses render stack
```

**Why:** `generate_png_from_buffer()` was the original rendering code. Now that we have `RenderLayerStack` with `BaseRenderLayer`, this is duplication.

**Proposed Change:** Remove `generate_png_from_buffer()` and always use the render stack.

**Criticality:** **Low** - Code works, just has two paths doing same thing.

**When to Do:** Can be done when we're confident the render stack is solid.

---

### 2. ~~LuaRenderLayer Hot Reload Not Integrated~~ ✅ COMPLETED

**Milestone:** M5 (Render Layer System)

**Resolution:** Created `LuaRendererPlugin` in `lua_renderer.rs` that:
- Watches `assets/map_editor/renderers/` directory for changes
- Reloads `LuaRenderLayer` when any `.lua` file changes
- Follows same pattern as `LuaGeneratorPlugin`

Verified with `scripts/verify_m5_renderer_hotreload.py` - PNG output hash changes when Lua renderer is modified.

---

### 3. ~~Render Stack Uses Rust BaseRenderLayer, Not Lua~~ ✅ COMPLETED

**Milestone:** M5 (Render Layer System)

**Resolution:** Updated `app.rs` to use `LuaRendererPlugin` instead of manually creating `BaseRenderLayer`. The plugin:
- Creates `LuaRenderLayer` loading `renderers/grid_2d.lua`
- Sets `needs_reload: true` on startup to trigger initial load
- Adds file watcher for hot reload

`assets/map_editor/renderers/grid_2d.lua` now contains a working Lua renderer that maps voxels to material colors.

---

## Cleanup Decision Log

| Item | Decision | Rationale |
|------|----------|-----------|
| MaterialPalette → InMemoryStore | ✅ Done during M4.5 | Consistency with Asset trait pattern |
| Search inconsistency | ✅ Done during M4.5 | Naturally resolved by item #1 |
| MCP Error variant | Defer | Low criticality |
| Duplicate PNG code | Defer | Low criticality, works fine |
| Lua hot reload not integrated | ✅ Done during M5 cleanup | Created `LuaRendererPlugin` with file watcher |
| BaseRenderLayer vs LuaRenderLayer | ✅ Done during M5 cleanup | App now uses `LuaRendererPlugin` |

---

## Summary Statistics

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 0 | - |
| Medium | 3 | ✅ All Completed (1 M4.5, 2 M5) |
| Low | 3 | 1 Completed (M4.5), 2 Deferred |

**M5 Cleanup complete.** All identified items resolved. M5 functionality verified with `verify_m5_renderer_hotreload.py` script. Proceed to M6.

---

## M6 Audit: Generator Visualizer

### 1. ~~Visualizer Plugin Duplicates Pattern from Renderer Plugin~~ ✅ COMPLETED

**Milestone:** M6 (Generator Visualizer)

**Resolution:** Created generic `hot_reload.rs` module with:
- `HotReloadConfig<T>` - generic config holding watch path and lua path
- `HotReloadFlag<T>` - generic reload flag
- `setup_hot_reload<T>` - generic file watcher setup
- `check_hot_reload<T>` - generic file change detection

Both `lua_renderer.rs` and `lua_visualizer.rs` now use this generic infrastructure, reducing each from ~140 lines to ~70 lines.

---

### 2. ~~GeneratorListener Trait Not Used~~ ✅ COMPLETED

**Milestone:** M6 (Generator Visualizer)

**Resolution:** Now properly used! The architecture was fixed:
- `GeneratorListeners` resource holds list of listeners
- Generator calls `listeners.notify_step(info)` when it fills a cell
- `SharedVisualizer` wraps `LuaVisualizer` in `Arc<Mutex<>>` so same instance can be both `GeneratorListener` AND `RenderLayer`
- Visualizer stores step info internally via `on_step()` callback, reads during render
- Removed `step_info` from `RenderContext` (visualizer no longer needs it passed through)

---

### 3. ~~VisualizerState Resource Unused~~ ✅ COMPLETED

**Milestone:** M6 (Generator Visualizer)

**Resolution:** `VisualizerState` now holds `SharedVisualizer` and is properly used:
- `setup_visualizer` creates `SharedVisualizer` and stores in `VisualizerState`
- Same `SharedVisualizer` is added to both `RenderLayerStack` and `GeneratorListeners`
- `reload_visualizer` uses `VisualizerState` to access and reload the visualizer in-place
- Because it's `Arc<Mutex<>>`, the render stack and listener registry both see the reloaded visualizer

---

## Cleanup Decision Log

| Item | Decision | Rationale |
|------|----------|-----------|
| MaterialPalette → InMemoryStore | ✅ Done during M4.5 | Consistency with Asset trait pattern |
| Search inconsistency | ✅ Done during M4.5 | Naturally resolved by item #1 |
| MCP Error variant | Defer | Low criticality |
| Duplicate PNG code | Defer | Low criticality, works fine |
| Lua hot reload not integrated | ✅ Done during M5 cleanup | Created `LuaRendererPlugin` with file watcher |
| BaseRenderLayer vs LuaRenderLayer | ✅ Done during M5 cleanup | App now uses `LuaRendererPlugin` |
| Hot-reload plugin duplication | ✅→⚠️ Superseded | Created `hot_reload.rs`, now deprecated by `LuaLayerPlugin` |
| GeneratorListener unused | ✅ Done during M6 cleanup | Generator now calls listeners, visualizer implements trait |
| VisualizerState unused | ✅→⚠️ Superseded | Now deprecated - `LuaLayerRegistry` manages visualizers |
| Plugins hardcoded to 1 instance | ✅ Done after M7 | Created `LuaLayerRegistry` with multi-instance support |
| M7 tag search | N/A | Clean implementation, no cleanup needed |
| Deprecated plugins/hot_reload | ⚠️ Defer to Phase 3 | ~380 lines dead code: `lua_renderer.rs`, `lua_visualizer.rs`, `hot_reload.rs` |

---

## Summary Statistics (Pre-M6 Audit)

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 1 | ✅ Completed (multi-instance plugins) |
| Medium | 3 | ✅ All Completed (1 M4.5, 2 M5) |
| Low | 6 | ✅ 4 Completed (1 M4.5, 3 M6), 2 Deferred |

**M6 Cleanup complete.** All M6 items resolved. M6 functionality verified with `verify_m6_visualizer.py` script.

**Note:** Some M6 cleanup work was superseded by Post-M7 refactor. See Post-M7 Audit below.

---

## M6 Post-Cleanup Audit: Multiplicity Constraints

### 4. ~~Plugins Hardcoded to Single Instance~~ ✅ COMPLETED

**Milestone:** M6 (Post-Cleanup Audit) → Resolved after M7

**Resolution:** Created `LuaLayerPlugin` with `LuaLayerRegistry`:

| Component | Count | Implementation |
|-----------|-------|----------------|
| Lua Generators | 1 | (unchanged - generator is separate concern) |
| Generator Listeners | **Many** | `GeneratorListeners` is `Vec<Box<dyn GeneratorListener>>` ✅ |
| Render Layers | **Many** | `RenderLayerStack` is `Vec<Box<dyn RenderLayer>>` ✅ |
| Lua Renderers | **Many** | `LuaLayerRegistry` + `LuaLayerPlugin` ✅ |
| Lua Visualizers | **Many** | `LuaLayerRegistry` + `LuaLayerPlugin` ✅ |

**New Architecture:**
- `LuaLayerDef` - Definition of a layer (name, type, path, tags), implements `Asset`
- `LuaLayerRegistry` - Uses `InMemoryStore<LuaLayerDef>` for definitions, tracks live instances
- `LuaLayerPlugin` - Single plugin manages all layers with hot-reload
- MCP endpoints: `GET /mcp/layer_registry`, `POST /mcp/register_layer`, `DELETE /mcp/layer/{name}`

**Migration:** `LuaRendererPlugin` and `LuaVisualizerPlugin` deprecated, use `LuaLayerPlugin` instead.

---

## Updated Summary Statistics

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 1 | ✅ Completed (multi-instance plugins) |
| Medium | 3 | ✅ All Completed (1 M4.5, 2 M5) |
| Low | 6 | ✅ 4 Completed (1 M4.5, 3 M6), 2 Deferred |

---

## M7 Audit: Text Search Across Assets

### No Cleanup Items Identified

M7 was a clean extension of the existing Asset system:
- Added `tags()` method to `Asset` trait with default empty implementation
- Added `tags: Vec<String>` field to `Material`
- Extended `InMemoryStore.search()` to match tags (exact match, case-insensitive)
- Updated `lua_materials.rs` to parse optional `tags` field
- Updated MCP search response to include tags
- Updated UI search panel to show tags in tooltips

No new duplication or architectural issues introduced.

---

---

## Post-M7 Audit: Deprecated Code from Multi-Instance Refactor

### 5. Deprecated Plugins and Generic Hot-Reload Module

**Milestone:** Post-M7 (Multi-Instance Refactor)

**Current State:**

The following files/exports are now deprecated but still exist:

| File | Status | Notes |
|------|--------|-------|
| `lua_renderer.rs` | **Deprecated** | Replaced by `LuaLayerPlugin` |
| `lua_visualizer.rs` | **Deprecated** | Replaced by `LuaLayerPlugin` |
| `hot_reload.rs` | **Deprecated** | Only used by deprecated plugins |

**Deprecated Exports in `mod.rs`:**
```rust
pub use lua_renderer::{LuaRendererPlugin, RendererReloadFlag};
pub use lua_visualizer::{LuaVisualizerPlugin, VisualizerReloadFlag, VisualizerState};
```

**Why This Happened:**
- M6 created `hot_reload.rs` to reduce duplication between `lua_renderer.rs` and `lua_visualizer.rs`
- Post-M7 created `LuaLayerPlugin` with its own hot-reload logic (single recursive watcher)
- The generic `hot_reload.rs` is now unused - `LuaLayerPlugin` handles everything

**Proposed Change:**
- Delete `lua_renderer.rs` (82 lines)
- Delete `lua_visualizer.rs` (115 lines)
- Delete `hot_reload.rs` (181 lines)
- Remove deprecated exports from `mod.rs`
- Total: ~380 lines of dead code removed

**Criticality:** **Medium** - Dead code that works but adds confusion. Not blocking.

**When to Do:** Beginning of Phase 3, or as quick cleanup now.

---

## Updated Summary Statistics

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 1 | ✅ Completed (multi-instance plugins) |
| Medium | 4 | ✅ 3 Completed (1 M4.5, 2 M5), 1 New (deprecated code) |
| Low | 6 | ✅ 4 Completed (1 M4.5, 3 M6), 2 Deferred |

---

## Phase 2 Summary

All five milestones completed:
- **M4.5:** Asset Trait + Search ✅
- **M4.75:** Generator Runs on Launch + MCP Set Endpoints ✅
- **M5:** Lua Renderer with Layer System ✅
- **M6:** Generator Visualizer ✅
- **M7:** Text Search Across Assets ✅

**Key Foundations Built:**
1. `Asset` + `AssetStore<T>` - unified asset management with search (name + tags)
2. `RenderLayer` trait + `RenderLayerStack` - compositable rendering
3. `StepInfo` + `CurrentStepInfo` - observable generation progress
4. `LuaLayerRegistry` - multi-instance layer management with MCP API
5. Tag-based categorization and search

**Deferred Cleanup Items:**
- 1 medium-criticality: Deprecated plugins + hot_reload.rs (~380 lines dead code)
- 2 low-criticality: MCP Error variant, Duplicate PNG code

**Post-M7 Refactor:**
- Created `LuaLayerPlugin` with `LuaLayerRegistry` for multi-instance support
- Deprecated `LuaRendererPlugin` and `LuaVisualizerPlugin`
- Added MCP endpoints for dynamic layer registration

**M6 Cleanup Refactor (now superseded):**
- Created generic `hot_reload.rs` module (now deprecated - superseded by `LuaLayerPlugin`)
- Fixed GeneratorListener pattern (generator calls listeners, not render layers reading context)
- Created `SharedVisualizer` wrapper for proper Arc<Mutex<>> sharing
- Removed `step_info` from `RenderContext` (visualizer stores internally)
