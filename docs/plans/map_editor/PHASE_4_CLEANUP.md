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

### 1. No Integration Tests for MCP Asset Endpoints - RESOLVED

**Milestone:** M11 (Database-Backed Asset Store)

**Status:** **RESOLVED** - Added 4 BlobStore integration tests that verify the same operations as MCP endpoints.

**Tests Added:**
- `test_blobstore_create_and_list` - Tests create + list operations
- `test_blobstore_search` - Tests FTS search with type filtering
- `test_blobstore_get_operations` - Tests get, get_metadata, get_full, exists, count, delete
- `test_blobstore_upsert` - Tests update-on-conflict behavior

These test via the `BlobStore` trait which is what MCP handlers use. No need to spin up HTTP server.

---

### 2. DatabaseStore Path Hardcoded in app.rs - RESOLVED

**Milestone:** M11 (Database-Backed Asset Store)

**Status:** **RESOLVED** - Added CLI argument `--asset-db <path>` and builder method `.with_asset_db()`.

**Changes:**
- `MapEditor2DConfig.asset_db_path: Option<String>` field added
- `--asset-db` CLI argument parsing in `with_cli_args()`
- `.with_asset_db(path)` builder method
- Default remains "assets.db" when not specified

---

### 3. Asset Endpoints Not Documented in Module Header - RESOLVED

**Milestone:** M11 (Database-Backed Asset Store)

**Status:** **RESOLVED** - Updated `mcp_server.rs` module docs with complete endpoint listing organized by category.

---

### 4. Duplicate Search Implementations - RESOLVED

**Milestone:** M11 (Database-Backed Asset Store)

**Status:** **RESOLVED** - Removed `/mcp/search` endpoint entirely.

**Changes:**
- Deleted `McpRequest::Search` variant
- Deleted `SearchRequest` struct
- Deleted `SearchResultJson` struct  
- Deleted `McpResponse::SearchResults` variant
- Deleted HTTP handler for `/mcp/search`
- Deleted Bevy handler for `McpRequest::Search`

Now only `/mcp/assets/search` exists, which searches the DatabaseStore.

---

### 5. AssetStore Trait vs DatabaseStore Concrete Type - RESOLVED

**Milestone:** M11 (Database-Backed Asset Store)

**Status:** **RESOLVED** - Created `BlobStore` trait that `DatabaseStore` implements.

**Rationale:** The existing `AssetStore<T>` trait is for typed in-memory storage (returns `&T`). `DatabaseStore` stores blobs with metadata - fundamentally different. Created new `BlobStore` trait:

```rust
pub trait BlobStore: Send + Sync {
    fn get(&self, key: &AssetKey) -> Result<Option<Vec<u8>>, AssetError>;
    fn get_metadata(&self, key: &AssetKey) -> Result<Option<AssetMetadata>, AssetError>;
    fn get_full(&self, key: &AssetKey) -> Result<Option<(Vec<u8>, AssetMetadata)>, AssetError>;
    fn set(&self, key: &AssetKey, content: &[u8], metadata: AssetMetadata) -> Result<(), AssetError>;
    fn delete(&self, key: &AssetKey) -> Result<bool, AssetError>;
    fn list(&self, namespace: &str, pattern: &str, asset_type: Option<&str>) -> Result<Vec<AssetRef>, AssetError>;
    fn search(&self, query: &str, asset_type: Option<&str>) -> Result<Vec<AssetRef>, AssetError>;
    fn exists(&self, key: &AssetKey) -> Result<bool, AssetError>;
    fn count(&self, namespace: &str, asset_type: Option<&str>) -> Result<usize, AssetError>;
}
```

Future backends (FileStore, RemoteStore) can implement this trait.

---

### 6. No Migration Path from File-Based Assets

**Milestone:** M11 (Database-Backed Asset Store)

**Status:** N/A - This is M14 (File Watcher Auto-Import), not a cleanup item.

---

### 7. assets.db in Project Root - RESOLVED

**Milestone:** M11 (Database-Backed Asset Store)

**Status:** **RESOLVED** - Added `assets.db` to `.gitignore` (done in previous commit).

---

## Cleanup Decision Log

| Item | Decision | Rationale |
|------|----------|-----------|
| No integration tests | **DONE** | Added 4 BlobStore trait tests |
| Hardcoded DB path | **DONE** | Added --asset-db CLI + .with_asset_db() |
| Missing endpoint docs | **DONE** | Reorganized module docs |
| Duplicate search endpoints | **DONE** | Removed /mcp/search entirely |
| AssetStore trait unused | **DONE** | Created BlobStore trait, DatabaseStore implements it |
| No file migration | N/A | This is M14, not cleanup |
| assets.db in root | **DONE** | Added to .gitignore |

---

## Summary Statistics

| Criticality | Count | Status |
|-------------|-------|--------|
| High | 0 | All resolved |
| Medium | 0 | All resolved |
| Low | 0 | All resolved (for M11 items) |

---

## M11 Verification Checklist (Updated)

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
| Unit tests | **Done** | 14 tests (10 original + 4 BlobStore) |
| BlobStore trait | **Done** | DatabaseStore implements it |
| CLI --asset-db option | **Done** | Configurable path |
| Auto-initialization in app | **Done** | Opens assets.db on startup |

---

## M11 Complete

All cleanup items resolved. 14 tests pass. Ready for M12 (Semantic Search).
