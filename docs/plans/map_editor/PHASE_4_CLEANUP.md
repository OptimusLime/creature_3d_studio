# Phase 4 Cleanup Audit

This document tracks refactoring candidates identified during Phase 4 milestone work. Each entry describes existing code that could benefit from improvement.

**Purpose:** Capture tech debt opportunities as we go, without blocking milestone progress. Review at phase end to decide what to address before Phase 5.

---

## How to Read This Document

Each cleanup item includes:
- **Milestone:** When we noticed this opportunity
- **Current State:** What the code looks like now
- **Proposed Change:** What it could look like
- **Why Refactor:** Engineering rationale for the change
- **Criticality:** How urgent is this?
  - **High:** Blocks future work or causes active problems
  - **Medium:** Creates inconsistency or minor duplication
  - **Low:** Nice-to-have, purely aesthetic improvement
- **When to Do:** Suggested timing for the refactor

---

## Carried Forward from Phase 3/3.5

| Item | Criticality | Status |
|------|-------------|--------|
| Generator type detection (always "lua") | Low | Deferred |
| MCP error variant unused | Low | Deferred |
| Duplicate PNG rendering code (legacy fallback) | Low | Deferred |
| True zero-copy MjGridView rendering | Low | Deferred - optimization only |

---

## M11 Audit: Database-Backed Asset Store

### 1. No Integration Tests for MCP Asset Endpoints

**Milestone:** M11 (Database-Backed Asset Store)

**Current State:**
- MCP asset endpoints (`POST /mcp/assets`, `GET /mcp/assets`, `GET /mcp/assets/search`) were tested manually via curl
- No automated integration tests exist
- Verification required running full example app and waiting for compilation/startup

**Proposed Change:**
Create integration tests that:
1. Spin up a test server with `DatabaseStore::open_in_memory()`
2. Send HTTP requests programmatically
3. Verify responses without running the full Bevy app

```rust
#[test]
fn test_mcp_create_asset() {
    let store = DatabaseStore::open_in_memory().unwrap();
    let (request_tx, request_rx) = channel();
    let (response_tx, response_rx) = channel();
    
    // Send CreateAsset request
    request_tx.send(McpRequest::CreateAsset(AssetCreateRequest {
        namespace: "test".into(),
        path: "materials/crystal".into(),
        asset_type: "material".into(),
        content: "return {}".into(),
        metadata: Default::default(),
    })).unwrap();
    
    // Process in mock handler
    handle_asset_request(&store, &request_rx, &response_tx);
    
    // Verify response
    match response_rx.recv().unwrap() {
        McpResponse::AssetCreated { ok, key } => {
            assert!(ok);
            assert_eq!(key, "test/materials/crystal");
        }
        _ => panic!("Wrong response type"),
    }
}
```

**Why Refactor:**
- Manual curl testing is slow and error-prone
- Can't run in CI without display/window
- Changes to MCP handlers could silently break asset endpoints

**Criticality:** **High** - Same class of bug as M10.4 MjLuaModel issues. Lack of tests allows regressions.

**When to Do:** Before M12 (semantic search will add more complexity to test).

---

### 2. DatabaseStore Path Hardcoded in app.rs

**Milestone:** M11 (Database-Backed Asset Store)

**Current State:**
```rust
// app.rs
let asset_db_path = std::path::Path::new("assets.db");
match DatabaseStore::open(asset_db_path) {
    Ok(store) => {
        info!("Opened asset database at {:?}", asset_db_path);
        app.insert_resource(store);
    }
    // ...
}
```

**Issue:** Database path is hardcoded. Can't:
- Use different paths for different projects
- Specify path via config/CLI
- Use in-memory for testing

**Proposed Change:**
Add to `MapEditor2DConfig`:
```rust
pub struct MapEditor2DConfig {
    // ...
    /// Path to asset database (None = use default "assets.db")
    pub asset_db_path: Option<PathBuf>,
}

impl MapEditor2DApp {
    pub fn with_asset_db(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.asset_db_path = Some(path.into());
        self
    }
}
```

**Criticality:** **Low** - Works fine for single-project use. Becomes Medium if we need multi-project support.

**When to Do:** When needed, or during M13 (Asset Browser) if config becomes important.

---

### 3. Asset Endpoints Not Documented in Module Header

**Milestone:** M11 (Database-Backed Asset Store)

**Current State:**
```rust
//! MCP (Model Context Protocol) HTTP server for external AI interaction.
//!
//! Provides a simple REST API on port 8088 for:
//! - Listing materials
//! - Creating new materials  
//! // ... (old list, doesn't include asset endpoints)
```

**Proposed Change:**
Update module doc to include:
```rust
//! - `GET /mcp/assets?namespace=X&pattern=Y&type=Z` - List assets
//! - `POST /mcp/assets` - Create/update asset
//! - `GET /mcp/assets/search?q=X&type=Y` - Search assets
```

**Criticality:** **Low** - Documentation only.

**When to Do:** Before M12 or during next MCP endpoint addition.

---

### 4. Duplicate Search Implementations

**Milestone:** M11 (Database-Backed Asset Store)

**Current State:**
Two search endpoints exist:
1. `GET /mcp/search?q=X&type=Y` - Original, searches `MaterialPalette` in-memory
2. `GET /mcp/assets/search?q=X&type=Y` - New, searches `DatabaseStore`

**Issue:**
- Two ways to search, different data sources
- Original search doesn't include database assets
- Confusing API surface

**Proposed Change:**
Option A: Deprecate `/mcp/search`, redirect to `/mcp/assets/search`
Option B: Make `/mcp/search` delegate to DatabaseStore if available
Option C: Keep both but document the difference clearly

**Criticality:** **Medium** - API inconsistency. Not blocking but confusing for AI users.

**When to Do:** During M12 (semantic search) - unify search implementations.

---

### 5. AssetStore Trait vs DatabaseStore Concrete Type

**Milestone:** M11 (Database-Backed Asset Store)

**Current State:**
- `AssetStore` trait exists in `asset/mod.rs`
- `DatabaseStore` is a concrete type that doesn't implement `AssetStore` trait
- MCP server takes `Option<Res<DatabaseStore>>` directly

**Issue:**
- If we add another backend (FileStore, RemoteStore), we can't swap them
- Violates the spec's vision of swappable backends

**Proposed Change:**
Have `DatabaseStore` implement `AssetStore`:
```rust
impl AssetStore<Vec<u8>> for DatabaseStore {
    fn get(&self, key: &AssetKey) -> Result<Option<Asset<Vec<u8>>>, AssetError> {
        // ...
    }
    // ...
}
```

Then MCP server takes `Option<Res<Box<dyn AssetStore>>>` or use Bevy's resource indirection.

**Criticality:** **Medium** - Architectural debt. Not blocking M12-M14, but makes future backends harder.

**When to Do:** Before adding second backend, or during Phase 4 polish.

---

### 6. No Migration Path from File-Based Assets

**Milestone:** M11 (Database-Backed Asset Store)

**Current State:**
- Database starts empty
- Existing Lua files in `assets/map_editor/` are not imported
- User must manually create assets via MCP

**Issue:**
- MILESTONES.md spec says "migration from file-backed"
- No auto-import of existing assets

**Proposed Change:**
This is actually M14 (File Watcher Auto-Import). Not a cleanup item, but noting the gap.

**Criticality:** **N/A** - Future milestone, not cleanup.

---

### 7. assets.db in Project Root

**Milestone:** M11 (Database-Backed Asset Store)

**Current State:**
- `assets.db` created in project root
- Not gitignored
- Will be committed if user runs example

**Proposed Change:**
Option A: Add `assets.db` to `.gitignore`
Option B: Put in `~/.creature_studio/assets.db` or XDG data dir
Option C: Put in project-specific `.creature_studio/assets.db`

**Criticality:** **Low** - Nuisance, not breaking.

**When to Do:** Now (simple gitignore) or during M13 (proper data dir).

---

## Cleanup Decision Log

| Item | Decision | Rationale |
|------|----------|-----------|
| No integration tests | **DO NOW** | High criticality, prevents regressions |
| Hardcoded DB path | Defer | Low priority, works for single project |
| Missing endpoint docs | Defer | Low priority, documentation |
| Duplicate search endpoints | Defer to M12 | Unify during semantic search work |
| AssetStore trait unused | Defer | Medium priority, do before second backend |
| No file migration | N/A | This is M14, not cleanup |
| assets.db in root | **DO NOW** | Simple gitignore fix |

---

## Summary Statistics

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 1 | Integration tests - DO NOW |
| Medium | 2 | Duplicate search, AssetStore trait - Deferred |
| Low | 3 | DB path, docs, gitignore - Deferred (gitignore quick fix) |

---

## M11 Verification Checklist

| Item | Status | Notes |
|------|--------|-------|
| DatabaseStore with SQLite | **Done** | rusqlite + FTS5 |
| Namespace/path key format | **Done** | "user/path/name" |
| `get()`, `set()`, `delete()` operations | **Done** | Full CRUD |
| `list()` with glob patterns | **Done** | SQL LIKE patterns |
| `search()` with FTS5 | **Done** | Full-text search |
| `search_simple()` fallback | **Done** | Substring matching |
| Embedding storage (`set_embedding`, `get_embedding`) | **Done** | Ready for M12 |
| `search_semantic()` cosine similarity | **Done** | Ready for M12 |
| MCP `POST /mcp/assets` | **Done** | Create/upsert |
| MCP `GET /mcp/assets?namespace=X` | **Done** | List with filters |
| MCP `GET /mcp/assets/search?q=X` | **Done** | Search |
| 10 unit tests | **Done** | All pass |
| Integration tests | **Missing** | Manual curl only |
| Auto-initialization in app | **Done** | Opens assets.db on startup |

---

## Action Items Before M12

1. [ ] Add `assets.db` to `.gitignore`
2. [ ] Create MCP asset endpoint integration tests (can use `DatabaseStore::open_in_memory()`)
3. [ ] Update mcp_server.rs module docs with new endpoints
