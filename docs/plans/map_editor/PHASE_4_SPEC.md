# Phase 4 Specification: Unified Asset Store (Database-Backed)

*Following HOW_WE_WORK.md and WRITING_MILESTONES.md.*

**Key principle:** All Lua assets share a common storage, search, and discovery infrastructure.

---

## Why This Phase Exists

Phases 1-3.5 built the map editor's core capabilities:
- Lua materials, generators, renderers, visualizers (all hot-reloadable)
- Markov Jr. integration with full introspection
- MCP server for AI access
- Multi-surface rendering and video export

But the asset system is fragmented:
- Files are the source of truth (no database)
- Each asset type has its own loading code
- No semantic search ("find me something for a cave")
- No unified browsing (materials here, generators there)
- AI writes files but can't search existing assets intelligently

This matters because:
1. **Discoverability:** With 100+ assets, file browsing becomes impossible
2. **AI Collaboration:** AI needs semantic search to find relevant existing work
3. **Persistence:** Game sessions need checkpoints, not just loose files
4. **Sharing:** Assets should be importable/exportable as packages

---

## Phase Outcome

**When Phase 4 is complete, I can:**
- Store all my Lua assets (materials, generators, renderers, visualizers) in a SQLite database
- Search assets by description ("glowing crystals") and find relevant matches
- Browse all asset types in one unified panel with folder navigation
- Drop files into a watched directory and have them auto-import to the database
- AI can search, create, and compose assets through MCP

**Phase Foundation:**
1. `AssetStore<T>` trait - Generic storage with namespace-based keys
2. `DatabaseStore` backend - SQLite with embedded vector search
3. `AssetBrowser` UI - Unified discovery across all asset types
4. `FileWatcher` auto-import - File → database pipeline

---

## Current Architecture (For Reference)

### Asset Loading (Phase 1-3.5)

```
                                   ┌─────────────────┐
                                   │  File Watcher   │
                                   │ (hot reload)    │
                                   └────────┬────────┘
                                            │ detects change
                                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Lua Context                              │
│                                                                  │
│  assets/materials.lua ────► MaterialPalette                     │
│  assets/generator.lua ────► LuaGenerator                        │
│  assets/renderers/*.lua ──► RenderLayerStack                    │
│  assets/visualizers/*.lua ► (visualizer layers)                 │
└─────────────────────────────────────────────────────────────────┘
```

**Problems:**
- Files are scattered across directories
- No unified API for "get asset by name"
- No search capability
- No metadata (descriptions, tags, embeddings)
- Each asset type has custom loading code

### Target Architecture (Phase 4)

```
┌─────────────────────────────────────────────────────────────────┐
│                       AssetStore<T>                             │
│                                                                 │
│  get(namespace, path) ──────────────────────► T                 │
│  set(namespace, path, T) ◄────────────────── T                  │
│  search(query) ─────────────────────────────► Vec<AssetRef>     │
│  search_semantic(embedding) ────────────────► Vec<AssetRef>     │
│  list(namespace, glob) ─────────────────────► Vec<Path>         │
│  watch(directory) ──────────────────────────► auto-import       │
└───────────────────────┬─────────────────────────────────────────┘
                        │
                        │ implements
                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                      DatabaseStore                              │
│                                                                 │
│  SQLite DB:                                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ assets                                                   │   │
│  │ ├── id: INTEGER PRIMARY KEY                              │   │
│  │ ├── namespace: TEXT        (e.g., "paul")               │   │
│  │ ├── path: TEXT             (e.g., "materials/crystal")  │   │
│  │ ├── asset_type: TEXT       (e.g., "material")           │   │
│  │ ├── content: BLOB          (Lua source or serialized)   │   │
│  │ ├── metadata: JSON         (name, description, tags)    │   │
│  │ ├── embedding: BLOB        (f32 vector for semantic)    │   │
│  │ └── updated_at: TIMESTAMP                                │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                        │
                        │ used by
                        ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ MaterialStore│  │GeneratorStore│  │RendererStore │  ...
│ (thin wrap)  │  │ (thin wrap)  │  │ (thin wrap)  │
└──────────────┘  └──────────────┘  └──────────────┘
```

---

## Milestone Details

### M11: Database-Backed Asset Store

**Functionality:** I can store and retrieve Lua assets from a SQLite database, and existing file-based loading still works during migration.

**Foundation:** `AssetStore<T>` trait and `DatabaseStore` implementation that all asset types use. The generic trait means adding new asset types requires no storage code changes.

#### Why First

Everything else in Phase 4 depends on database storage:
- Semantic search needs a place to store embeddings
- Browser needs unified queries across asset types
- File watcher needs a destination to import into
- AI search via MCP needs a searchable store

#### API Design

```rust
/// Key for locating an asset in the store.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct AssetKey {
    /// User or organization (e.g., "paul", "anomaly")
    pub namespace: String,
    /// Path within namespace (e.g., "materials/crystal")
    pub path: String,
}

impl AssetKey {
    /// Parse from string like "paul/materials/crystal"
    pub fn parse(s: &str) -> Option<Self>;
    
    /// Format as "namespace/path"
    pub fn to_string(&self) -> String;
}

/// Metadata attached to every asset.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetMetadata {
    /// Display name
    pub name: String,
    /// Human-readable description (used for semantic search)
    pub description: Option<String>,
    /// Searchable tags
    pub tags: Vec<String>,
    /// Asset type: "material", "generator", "renderer", "visualizer"
    pub asset_type: String,
    /// When last modified
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Reference to an asset without loading its content.
#[derive(Clone, Debug)]
pub struct AssetRef {
    pub key: AssetKey,
    pub metadata: AssetMetadata,
}

/// Generic asset storage trait.
pub trait AssetStore: Send + Sync {
    /// Get asset content by key.
    fn get(&self, key: &AssetKey) -> Result<Option<Vec<u8>>, AssetError>;
    
    /// Get asset metadata by key.
    fn get_metadata(&self, key: &AssetKey) -> Result<Option<AssetMetadata>, AssetError>;
    
    /// Store asset content and metadata.
    fn set(&self, key: &AssetKey, content: &[u8], metadata: AssetMetadata) -> Result<(), AssetError>;
    
    /// Delete asset.
    fn delete(&self, key: &AssetKey) -> Result<bool, AssetError>;
    
    /// List assets matching glob pattern within namespace.
    /// Pattern: "materials/*" matches "materials/stone", "materials/dirt"
    fn list(&self, namespace: &str, pattern: &str) -> Result<Vec<AssetRef>, AssetError>;
    
    /// Text search across name, description, tags.
    fn search(&self, query: &str, asset_type: Option<&str>) -> Result<Vec<AssetRef>, AssetError>;
    
    /// Semantic search using embedding vector (Phase 4.5).
    fn search_semantic(&self, embedding: &[f32], limit: usize) -> Result<Vec<AssetRef>, AssetError>;
    
    /// Store embedding for an asset (Phase 4.5).
    fn set_embedding(&self, key: &AssetKey, embedding: &[f32]) -> Result<(), AssetError>;
}

/// SQLite-backed implementation.
pub struct DatabaseStore {
    conn: rusqlite::Connection,  // Or pool for concurrent access
}

impl DatabaseStore {
    pub fn open(path: &Path) -> Result<Self, AssetError>;
    pub fn open_in_memory() -> Result<Self, AssetError>;  // For tests
}

impl AssetStore for DatabaseStore { ... }
```

**Type-specific wrappers:**

```rust
/// Convenience wrapper for materials.
pub struct MaterialStore<'a> {
    store: &'a dyn AssetStore,
}

impl MaterialStore<'_> {
    /// Load and parse a Lua material definition.
    pub fn get_material(&self, key: &AssetKey) -> Result<Option<MaterialDef>, AssetError> {
        if let Some(content) = self.store.get(key)? {
            let lua_source = String::from_utf8(content)?;
            // Parse Lua and extract MaterialDef
            Ok(Some(parse_material_lua(&lua_source)?))
        } else {
            Ok(None)
        }
    }
    
    /// List all materials in a namespace.
    pub fn list_materials(&self, namespace: &str) -> Result<Vec<AssetRef>, AssetError> {
        self.store.list(namespace, "materials/*")
    }
}

// Similar wrappers for GeneratorStore, RendererStore, VisualizerStore
```

#### Database Schema

```sql
CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    namespace TEXT NOT NULL,
    path TEXT NOT NULL,
    asset_type TEXT NOT NULL,
    content BLOB NOT NULL,
    metadata JSON NOT NULL,
    embedding BLOB,  -- NULL until semantic search enabled
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(namespace, path)
);

CREATE INDEX idx_assets_namespace ON assets(namespace);
CREATE INDEX idx_assets_type ON assets(asset_type);
CREATE INDEX idx_assets_updated ON assets(updated_at);

-- Full-text search index
CREATE VIRTUAL TABLE assets_fts USING fts5(
    name, description, tags,
    content='assets',
    content_rowid='id'
);

-- Trigger to keep FTS in sync
CREATE TRIGGER assets_ai AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, name, description, tags)
    VALUES (
        new.id,
        json_extract(new.metadata, '$.name'),
        json_extract(new.metadata, '$.description'),
        json_extract(new.metadata, '$.tags')
    );
END;
```

#### Migration Strategy

Phase 4 doesn't break existing file-based loading. Instead:

1. **File loading continues to work** - Existing code unchanged
2. **Database is additional storage** - New assets can go to DB
3. **Import tool migrates files** - Explicit migration when ready
4. **Eventually files become optional** - DB becomes source of truth

```rust
/// Asset resolution order:
/// 1. Check database
/// 2. Fall back to file system
/// 3. Return None if neither has it
pub fn resolve_asset(key: &AssetKey, store: &DatabaseStore, assets_dir: &Path) -> Option<Vec<u8>> {
    // Try database first
    if let Ok(Some(content)) = store.get(key) {
        return Some(content);
    }
    
    // Fall back to file
    let file_path = assets_dir.join(&key.namespace).join(&key.path);
    if file_path.exists() {
        return std::fs::read(&file_path).ok();
    }
    
    None
}
```

#### Implementation Tasks

| # | Task | File | Verification |
|---|------|------|--------------|
| 1 | Create `AssetKey`, `AssetMetadata`, `AssetRef` structs | `map_editor/asset_store/mod.rs` | Compiles |
| 2 | Create `AssetStore` trait | `map_editor/asset_store/mod.rs` | Compiles |
| 3 | Create `DatabaseStore` with SQLite connection | `map_editor/asset_store/database.rs` | Opens DB |
| 4 | Implement schema creation and migrations | `map_editor/asset_store/database.rs` | Tables exist |
| 5 | Implement `get`, `set`, `delete` | `map_editor/asset_store/database.rs` | Unit tests pass |
| 6 | Implement `list` with glob matching | `map_editor/asset_store/database.rs` | Unit tests pass |
| 7 | Implement `search` using FTS5 | `map_editor/asset_store/database.rs` | Unit tests pass |
| 8 | Create `MaterialStore` wrapper | `map_editor/asset_store/material_store.rs` | Compiles |
| 9 | Create `GeneratorStore` wrapper | `map_editor/asset_store/generator_store.rs` | Compiles |
| 10 | Add MCP endpoint `GET /mcp/assets` | `map_editor/mcp_server.rs` | Returns JSON |
| 11 | Add MCP endpoint `POST /mcp/assets` | `map_editor/mcp_server.rs` | Creates asset |
| 12 | Add MCP endpoint `GET /mcp/search` | `map_editor/mcp_server.rs` | Returns matches |
| 13 | Integration test: create + search + list | `tests/asset_store_integration.rs` | Pass |

#### Verification

```bash
# 1. Build succeeds
cargo build -p studio_core

# 2. Unit tests pass
cargo test -p studio_core asset_store

# 3. MCP endpoints work
cargo run --example p_map_editor_2d &
sleep 6

# Create an asset
curl -X POST http://127.0.0.1:8088/mcp/assets \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "test",
    "path": "materials/crystal",
    "asset_type": "material",
    "content": "return { name = \"Crystal\", color = {0.5, 0.8, 1.0} }",
    "metadata": {
      "name": "Crystal",
      "description": "Glowing blue crystal material",
      "tags": ["crystal", "glowing", "blue"]
    }
  }'
# Returns: {"ok": true, "key": "test/materials/crystal"}

# Search for it
curl "http://127.0.0.1:8088/mcp/search?q=crystal"
# Returns: {"results": [{"key": "test/materials/crystal", "name": "Crystal", ...}]}

# List all materials
curl "http://127.0.0.1:8088/mcp/assets?namespace=test&pattern=materials/*"
# Returns: {"assets": [{"key": "test/materials/crystal", ...}]}

pkill -f p_map_editor_2d
```

---

### M11.5: MCP Universal Asset CRUD

**Functionality:** I can create, read, update, and delete ANY asset type via simple curl calls. Direct path access lets me GET any asset's raw content. AI can write materials, generators, renderers, visualizers—all through the same endpoint pattern.

**Foundation:** RESTful asset endpoints with consistent URL patterns. The same CRUD pattern works for all asset types, making AI integration trivial.

#### Why This Milestone

Before the browser (M13) and file watcher (M14), we need AI to be able to:
1. Create assets of any type directly via HTTP
2. Read existing assets (raw Lua content)
3. Update assets in place
4. Delete assets

This is the "AI writes code" primitive that everything else builds on.

#### URL Pattern

```
# Create/Update asset (PUT is idempotent)
PUT /mcp/asset/{namespace}/{path}
Content-Type: application/json
{
  "asset_type": "material",
  "content": "return { name = 'Crystal', color = {0.5, 0.8, 1.0} }",
  "metadata": {
    "name": "Crystal",
    "description": "Glowing blue crystal",
    "tags": ["crystal", "blue", "glow"]
  }
}

# Get asset content (raw Lua)
GET /mcp/asset/{namespace}/{path}
# Returns: return { name = 'Crystal', color = {0.5, 0.8, 1.0} }

# Get asset with metadata
GET /mcp/asset/{namespace}/{path}?include_metadata=true
# Returns: {"content": "return {...}", "metadata": {...}}

# Delete asset
DELETE /mcp/asset/{namespace}/{path}
# Returns: {"deleted": true}

# List assets (already in M11)
GET /mcp/assets?namespace={ns}&pattern={glob}&type={type}
```

#### Key Design Points

1. **Path-based addressing:** `/mcp/asset/paul/materials/crystal` maps directly to `AssetKey { namespace: "paul", path: "materials/crystal" }`

2. **Raw content by default:** GET returns just the Lua source, not wrapped in JSON. This lets AI read and write code naturally.

3. **PUT is idempotent:** Same call creates or updates. No need for separate POST/PATCH.

4. **Type in body, not URL:** Asset type is metadata, not routing. One endpoint handles all types.

5. **Metadata optional on read:** By default GET is lean (just content). Add `?include_metadata=true` for full details.

#### API Design

```rust
/// PUT /mcp/asset/{namespace}/{path}
#[derive(Deserialize)]
pub struct PutAssetRequest {
    pub asset_type: String,
    pub content: String,  // Raw Lua source
    #[serde(default)]
    pub metadata: Option<AssetMetadataInput>,
}

#[derive(Deserialize)]
pub struct AssetMetadataInput {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Handler for PUT /mcp/asset/{namespace}/{path}
async fn put_asset(
    Path((namespace, path)): Path<(String, String)>,
    Json(req): Json<PutAssetRequest>,
    store: Data<DatabaseStore>,
) -> impl Responder {
    let key = AssetKey { namespace, path };
    
    // Build metadata, using filename as default name
    let metadata = AssetMetadata {
        name: req.metadata.as_ref()
            .and_then(|m| m.name.clone())
            .unwrap_or_else(|| key.path.split('/').last().unwrap_or("unnamed").to_string()),
        description: req.metadata.as_ref().and_then(|m| m.description.clone()),
        tags: req.metadata.as_ref().map(|m| m.tags.clone()).unwrap_or_default(),
        asset_type: req.asset_type,
        updated_at: chrono::Utc::now(),
    };
    
    store.set(&key, req.content.as_bytes(), metadata)?;
    
    Json(json!({ "ok": true, "key": key.to_string() }))
}

/// Handler for GET /mcp/asset/{namespace}/{path}
async fn get_asset(
    Path((namespace, path)): Path<(String, String)>,
    Query(params): Query<GetAssetParams>,
    store: Data<DatabaseStore>,
) -> impl Responder {
    let key = AssetKey { namespace, path };
    
    let content = store.get(&key)?
        .ok_or(AssetError::NotFound)?;
    
    if params.include_metadata.unwrap_or(false) {
        let metadata = store.get_metadata(&key)?.unwrap();
        Json(json!({
            "content": String::from_utf8_lossy(&content),
            "metadata": metadata
        }))
    } else {
        // Return raw content
        HttpResponse::Ok()
            .content_type("text/x-lua")
            .body(content)
    }
}

/// Handler for DELETE /mcp/asset/{namespace}/{path}
async fn delete_asset(
    Path((namespace, path)): Path<(String, String)>,
    store: Data<DatabaseStore>,
) -> impl Responder {
    let key = AssetKey { namespace, path };
    let deleted = store.delete(&key)?;
    Json(json!({ "deleted": deleted }))
}
```

#### Implementation Tasks

| # | Task | File | Verification |
|---|------|------|--------------|
| 1 | Add `PUT /mcp/asset/{namespace}/{path}` route | `map_editor/mcp_server.rs` | curl PUT works |
| 2 | Add `GET /mcp/asset/{namespace}/{path}` route | `map_editor/mcp_server.rs` | curl GET returns Lua |
| 3 | Add `DELETE /mcp/asset/{namespace}/{path}` route | `map_editor/mcp_server.rs` | curl DELETE works |
| 4 | Handle `?include_metadata=true` query param | `map_editor/mcp_server.rs` | Returns JSON with metadata |
| 5 | Return proper content-type for raw Lua | `map_editor/mcp_server.rs` | `text/x-lua` header |
| 6 | Unit tests for all CRUD operations | `map_editor/mcp_server.rs` | Pass |

#### Verification

```bash
# 1. Build succeeds
cargo build -p studio_core

# 2. Run app
cargo run --example p_map_editor_2d &
sleep 6

# 3. CREATE: Put a new material
curl -X PUT "http://127.0.0.1:8088/mcp/asset/test/materials/emerald" \
  -H "Content-Type: application/json" \
  -d '{
    "asset_type": "material",
    "content": "return { name = \"Emerald\", color = {0.2, 0.8, 0.3} }",
    "metadata": {
      "name": "Emerald",
      "description": "Green precious gemstone",
      "tags": ["gem", "green"]
    }
  }'
# Returns: {"ok": true, "key": "test/materials/emerald"}

# 4. READ: Get raw content
curl "http://127.0.0.1:8088/mcp/asset/test/materials/emerald"
# Returns: return { name = "Emerald", color = {0.2, 0.8, 0.3} }

# 5. READ with metadata
curl "http://127.0.0.1:8088/mcp/asset/test/materials/emerald?include_metadata=true"
# Returns: {"content": "return {...}", "metadata": {"name": "Emerald", ...}}

# 6. UPDATE: Modify the asset
curl -X PUT "http://127.0.0.1:8088/mcp/asset/test/materials/emerald" \
  -H "Content-Type: application/json" \
  -d '{
    "asset_type": "material",
    "content": "return { name = \"Emerald\", color = {0.1, 0.9, 0.2}, roughness = 0.3 }"
  }'
# Returns: {"ok": true, "key": "test/materials/emerald"}

# 7. Verify update
curl "http://127.0.0.1:8088/mcp/asset/test/materials/emerald"
# Returns updated content with roughness

# 8. DELETE
curl -X DELETE "http://127.0.0.1:8088/mcp/asset/test/materials/emerald"
# Returns: {"deleted": true}

# 9. Verify deleted
curl "http://127.0.0.1:8088/mcp/asset/test/materials/emerald"
# Returns: 404

# 10. CREATE: Put a generator (same endpoint!)
curl -X PUT "http://127.0.0.1:8088/mcp/asset/test/generators/scatter_crystals" \
  -H "Content-Type: application/json" \
  -d '{
    "asset_type": "generator",
    "content": "local Generator = require(\"lib.generator\")\nlocal ScatterCrystals = Generator:extend(\"ScatterCrystals\")\nfunction ScatterCrystals:step(ctx) end\nreturn ScatterCrystals"
  }'
# Returns: {"ok": true, "key": "test/generators/scatter_crystals"}

pkill -f p_map_editor_2d
```

---

### M12: Semantic Search Across All Assets

**Functionality:** I can search for any asset by description, not just name, and find relevant matches even with different wording.

**Foundation:** Embedding generation and vector similarity search. The embedding infrastructure works for any asset type and can be extended to other content (e.g., code, documentation).

#### Why This Milestone

Text search finds "crystal" when you type "crystal". But:
- "something shiny for a cave" should find crystal material
- "maze generation" should find MazeGrowth generator
- "dark atmosphere" should find relevant renderers

Semantic search bridges the gap between user intent and asset names.

#### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Embedding Pipeline                            │
│                                                                  │
│  Asset ──► Extract Text ──► Embedding Model ──► Store Vector    │
│            (name+desc+tags)   (local or API)                     │
└─────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Semantic Search                               │
│                                                                  │
│  Query ──► Embedding Model ──► Vector ──► Cosine Similarity     │
│                                           with stored vectors    │
│                                               │                  │
│                                               ▼                  │
│                                         Ranked Results           │
└─────────────────────────────────────────────────────────────────┘
```

#### API Design

```rust
/// Trait for generating text embeddings.
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding for text. Returns vector of floats.
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;
    
    /// Embedding dimension (e.g., 384 for MiniLM, 1536 for OpenAI)
    fn dimension(&self) -> usize;
}

/// Local embedding using Candle ML (pure Rust, no external dependencies).
pub struct CandleEmbedding {
    model: candle_transformers::models::bert::BertModel,
    tokenizer: tokenizers::Tokenizer,
    device: candle_core::Device,
}

impl CandleEmbedding {
    /// Load MiniLM or similar small model from HuggingFace cache.
    pub fn load(model_id: &str) -> Result<Self, EmbedError> {
        // model_id: "sentence-transformers/all-MiniLM-L6-v2"
        let device = candle_core::Device::Cpu;  // Or cuda if available
        let tokenizer = tokenizers::Tokenizer::from_pretrained(model_id, None)?;
        let model = Self::load_bert_model(model_id, &device)?;
        Ok(Self { model, tokenizer, device })
    }
}

impl EmbeddingProvider for CandleEmbedding {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let encoding = self.tokenizer.encode(text, true)?;
        let input_ids = candle_core::Tensor::new(encoding.get_ids(), &self.device)?;
        let token_type_ids = candle_core::Tensor::zeros_like(&input_ids)?;
        let attention_mask = candle_core::Tensor::new(encoding.get_attention_mask(), &self.device)?;
        
        let embeddings = self.model.forward(&input_ids, &token_type_ids, Some(&attention_mask))?;
        // Mean pooling over sequence length
        let pooled = embeddings.mean(1)?;
        Ok(pooled.to_vec1()?)
    }
    
    fn dimension(&self) -> usize { 384 }  // MiniLM
}

/// OpenAI API embedding (higher quality, requires API key).
pub struct OpenAIEmbedding {
    client: reqwest::Client,
    api_key: String,
    model: String,  // "text-embedding-3-small"
}

impl EmbeddingProvider for OpenAIEmbedding { ... }

/// Extended AssetStore with semantic search.
impl DatabaseStore {
    /// Generate and store embedding for an asset.
    pub fn embed_asset(&self, key: &AssetKey, provider: &dyn EmbeddingProvider) -> Result<(), AssetError> {
        let metadata = self.get_metadata(key)?.ok_or(AssetError::NotFound)?;
        
        // Combine text fields for embedding
        let text = format!(
            "{} {} {}",
            metadata.name,
            metadata.description.unwrap_or_default(),
            metadata.tags.join(" ")
        );
        
        let embedding = provider.embed(&text)?;
        self.set_embedding(key, &embedding)
    }
    
    /// Search using vector similarity.
    /// Returns assets sorted by similarity (highest first).
    fn search_semantic(&self, embedding: &[f32], limit: usize) -> Result<Vec<AssetRef>, AssetError> {
        // Cosine similarity: dot(a, b) / (|a| * |b|)
        // For normalized vectors: just dot(a, b)
        // SQLite doesn't have native vector ops, so we load all embeddings and compute in Rust
        // (For production scale, use pgvector or similar)
        ...
    }
}
```

#### Embedding Model Choice

**Recommended: all-MiniLM-L6-v2** (via Candle ML)
- 384-dimension embeddings
- ~22MB model size (auto-downloaded from HuggingFace)
- Runs locally, no API calls
- Pure Rust via `candle-core` and `candle-transformers`
- Good quality for short text (names, descriptions)

**Alternative: OpenAI text-embedding-3-small**
- 1536-dimension embeddings
- API call per embedding
- Higher quality, especially for longer text
- Requires API key

**Implementation:** Start with Candle local model. Add OpenAI as optional premium path.

#### Implementation Tasks

| # | Task | File | Verification |
|---|------|------|--------------|
| 1 | Add `candle-core`, `candle-transformers`, `tokenizers`, `hf-hub` dependencies | `Cargo.toml` | Compiles |
| 2 | Create `EmbeddingProvider` trait | `map_editor/asset_store/embedding.rs` | Compiles |
| 3 | Implement `CandleEmbedding` with MiniLM | `map_editor/asset_store/embedding.rs` | Generates vectors |
| 4 | Add model auto-download via `hf-hub` | `map_editor/asset_store/embedding.rs` | Model cached |
| 5 | Implement `embed_asset` on `DatabaseStore` | `map_editor/asset_store/database.rs` | Stores embedding |
| 6 | Implement `search_semantic` with cosine similarity | `map_editor/asset_store/database.rs` | Returns ranked results |
| 7 | Add background embedding job for new assets | `map_editor/asset_store/database.rs` | Auto-embeds on insert |
| 8 | Add MCP endpoint `GET /mcp/search_semantic` | `map_editor/mcp_server.rs` | Returns ranked results |
| 9 | Unit tests for embedding generation | `map_editor/asset_store/embedding.rs` | Pass |
| 10 | Integration test: semantic search finds related | `tests/semantic_search_integration.rs` | Pass |

#### Verification

```bash
# 1. First run auto-downloads model from HuggingFace
# Model cached at: ~/.cache/huggingface/hub/models--sentence-transformers--all-MiniLM-L6-v2

# 2. Build succeeds
cargo build -p studio_core

# 3. Unit tests pass
cargo test -p studio_core embedding

# 4. Semantic search works
cargo run --example p_map_editor_2d &
sleep 8

# Create some assets
curl -X POST http://127.0.0.1:8088/mcp/assets -d '{
  "namespace": "test", "path": "materials/crystal",
  "content": "return {}",
  "metadata": {"name": "Crystal", "description": "Glowing blue gemstone", "tags": ["gem", "glow"], "asset_type": "material"}
}'

curl -X POST http://127.0.0.1:8088/mcp/assets -d '{
  "namespace": "test", "path": "materials/lava",
  "content": "return {}",
  "metadata": {"name": "Lava", "description": "Hot molten rock", "tags": ["hot", "liquid"], "asset_type": "material"}
}'

# Semantic search - should find crystal even though "shiny cave" doesn't match exactly
curl "http://127.0.0.1:8088/mcp/search_semantic?q=something%20shiny%20for%20a%20cave"
# Returns: [{"key": "test/materials/crystal", "score": 0.72, ...}]

# Search for hot things - should find lava
curl "http://127.0.0.1:8088/mcp/search_semantic?q=volcanic%20material"
# Returns: [{"key": "test/materials/lava", "score": 0.81, ...}]

pkill -f p_map_editor_2d
```

---

### M13: Asset Browser Panel

**Functionality:** I can browse all my assets in one panel with folder navigation, filtering by type, and quick preview.

**Foundation:** `AssetBrowser` UI component that works with any asset type. The component pattern (tree view + detail panel) is reusable for other hierarchical data.

#### Why This Milestone

With database storage and semantic search, we need a UI to:
- See all assets at a glance
- Navigate namespaces/folders
- Filter by type (materials, generators, etc.)
- Preview before loading
- Quick-load into editor

#### UI Design

```
┌─────────────────────────────────────────────────────────────────┐
│ Asset Browser                                           [x] [_] │
├─────────────────────────────────────────────────────────────────┤
│ [Search: _______________] [Type: All ▼]                         │
├────────────────────────────┬────────────────────────────────────┤
│ ▼ paul/                    │  Crystal                           │
│   ▼ materials/             │  ────────────────────────────────  │
│     ● crystal     ◄───────────  Type: material                  │
│     ○ stone                │  Description: Glowing blue         │
│     ○ dirt                 │  gemstone for cave environments    │
│   ▼ generators/            │                                    │
│     ○ maze_growth          │  Tags: gem, glow, blue, cave       │
│     ○ dungeon              │                                    │
│ ▼ shared/                  │  Preview:                          │
│   ▼ materials/             │  ┌──────────┐                      │
│     ○ water                │  │ ■■■■■■■■ │  (color swatch)      │
│                            │  └──────────┘                      │
│                            │                                    │
│                            │  [Load] [Edit] [Delete]            │
└────────────────────────────┴────────────────────────────────────┘
```

#### Component Design

```rust
/// Asset browser UI state.
pub struct AssetBrowser {
    /// Currently selected namespace
    selected_namespace: Option<String>,
    /// Currently selected asset
    selected_asset: Option<AssetKey>,
    /// Search query
    search_query: imgui::ImString,
    /// Type filter index (0 = All, 1 = Material, etc.)
    type_filter_idx: usize,
    /// Expanded folders in tree (path -> expanded)
    expanded: HashSet<String>,
    /// Cached tree structure
    tree: AssetTree,
    /// Action to perform after frame (load, edit, delete)
    pending_action: Option<BrowserAction>,
}

/// Tree structure for display.
pub struct AssetTree {
    roots: Vec<AssetTreeNode>,
}

pub enum AssetTreeNode {
    Folder {
        name: String,
        path: String,
        children: Vec<AssetTreeNode>,
    },
    Asset {
        key: AssetKey,
        metadata: AssetMetadata,
    },
}

pub enum BrowserAction {
    Load(AssetKey),
    Edit(AssetKey),
    Delete(AssetKey),
}

impl AssetBrowser {
    /// Render the browser panel using imgui.
    pub fn ui(&mut self, ui: &imgui::Ui, store: &dyn AssetStore) -> Option<BrowserAction> {
        self.pending_action = None;
        
        // Search bar
        ui.input_text("##search", &mut self.search_query).build();
        ui.same_line();
        
        let type_options = ["All", "Materials", "Generators", "Renderers", "Visualizers"];
        ui.combo("Type", &mut self.type_filter_idx, &type_options, |s| std::borrow::Cow::Borrowed(*s));
        
        // Two-column layout using imgui columns
        ui.columns(2, "browser_cols", true);
        
        // Left: Tree view
        self.render_tree(ui, store);
        
        ui.next_column();
        
        // Right: Detail panel
        self.render_detail(ui, store);
        
        ui.columns(1, "end_cols", false);
        
        self.pending_action.take()
    }
    
    fn render_tree(&mut self, ui: &imgui::Ui, store: &dyn AssetStore) {
        // Use imgui TreeNode for recursive rendering
        for node in &self.tree.roots {
            self.render_tree_node(ui, node);
        }
    }
    
    fn render_tree_node(&mut self, ui: &imgui::Ui, node: &AssetTreeNode) {
        match node {
            AssetTreeNode::Folder { name, path, children } => {
                let flags = if self.expanded.contains(path) {
                    imgui::TreeNodeFlags::DEFAULT_OPEN
                } else {
                    imgui::TreeNodeFlags::empty()
                };
                
                if ui.tree_node_config(name).flags(flags).build(|| {
                    self.expanded.insert(path.clone());
                    for child in children {
                        self.render_tree_node(ui, child);
                    }
                }).is_none() {
                    self.expanded.remove(path);
                }
            }
            AssetTreeNode::Asset { key, metadata } => {
                let selected = self.selected_asset.as_ref() == Some(key);
                if ui.selectable_config(&metadata.name)
                    .selected(selected)
                    .build()
                {
                    self.selected_asset = Some(key.clone());
                }
            }
        }
    }
    
    fn render_detail(&mut self, ui: &imgui::Ui, store: &dyn AssetStore) {
        if let Some(key) = &self.selected_asset {
            if let Ok(Some(metadata)) = store.get_metadata(key) {
                ui.text_colored([1.0, 1.0, 0.0, 1.0], &metadata.name);
                ui.separator();
                ui.text(format!("Type: {}", metadata.asset_type));
                if let Some(desc) = &metadata.description {
                    ui.text_wrapped(desc);
                }
                ui.spacing();
                
                // Tags as small buttons
                for tag in &metadata.tags {
                    ui.small_button(tag);
                    ui.same_line();
                }
                ui.new_line();
                ui.spacing();
                
                // Preview (type-specific)
                self.render_preview(ui, key, &metadata);
                
                ui.separator();
                
                // Actions
                if ui.button("Load") {
                    self.pending_action = Some(BrowserAction::Load(key.clone()));
                }
                ui.same_line();
                if ui.button("Edit") {
                    self.pending_action = Some(BrowserAction::Edit(key.clone()));
                }
                ui.same_line();
                if ui.button("Delete") {
                    self.pending_action = Some(BrowserAction::Delete(key.clone()));
                }
            }
        } else {
            ui.text_disabled("Select an asset");
        }
    }
    
    fn render_preview(&self, ui: &imgui::Ui, key: &AssetKey, metadata: &AssetMetadata) {
        match metadata.asset_type.as_str() {
            "material" => {
                // Color swatch - would need to load and parse Lua to get color
                // For now, placeholder
                ui.text("Preview: [color swatch]");
            }
            "generator" => {
                // Thumbnail if cached
                ui.text("Preview: [thumbnail]");
            }
            _ => {
                ui.text_disabled("No preview available");
            }
        }
    }
}
```

#### Preview by Type

| Type | Preview |
|------|---------|
| Material | Color swatch (primary color from Lua) |
| Generator | Thumbnail of last output (cached PNG) |
| Renderer | Description only (no visual preview) |
| Visualizer | Description only |

#### Implementation Tasks

| # | Task | File | Verification |
|---|------|------|--------------|
| 1 | Create `AssetTree` struct with folder grouping | `map_editor/ui/asset_browser.rs` | Compiles |
| 2 | Create `AssetBrowser` state struct | `map_editor/ui/asset_browser.rs` | Compiles |
| 3 | Implement tree view with imgui TreeNode | `map_editor/ui/asset_browser.rs` | Renders tree |
| 4 | Implement detail panel with imgui | `map_editor/ui/asset_browser.rs` | Shows metadata |
| 5 | Add search filtering | `map_editor/ui/asset_browser.rs` | Filters tree |
| 6 | Add type filtering via imgui combo | `map_editor/ui/asset_browser.rs` | Filters by type |
| 7 | Add material preview (color swatch) | `map_editor/ui/asset_browser.rs` | Shows color |
| 8 | Add "Load" action via `BrowserAction` enum | `map_editor/ui/asset_browser.rs` | Asset loads |
| 9 | Integrate browser into main app imgui render | `map_editor/app.rs` | Visible in app |
| 10 | Keyboard navigation (imgui handles arrows) | `map_editor/ui/asset_browser.rs` | Navigates |

#### Verification

```bash
# 1. Build succeeds
cargo build --example p_map_editor_2d

# 2. Run app and verify browser
cargo run --example p_map_editor_2d

# Manual verification:
# - Asset Browser panel visible
# - Can expand/collapse folders
# - Can select assets and see details
# - Can filter by type
# - Can search by name
# - "Load" button loads selected asset
# - Keyboard navigation works

# 3. MCP can list assets that appear in browser
curl "http://127.0.0.1:8088/mcp/assets?namespace=paul"
# Should match what's visible in browser
```

---

### M14: File Watcher Auto-Import

**Functionality:** When I (or AI) write a file to a watched directory, it auto-imports to the database with extracted metadata.

**Foundation:** `FileWatcher` component with pluggable import handlers. The pattern supports any file type (Lua, JSON, images) and can be extended for future formats.

#### Why This Milestone

AI tools write files. Human editors write files. Both need assets to appear in the database without manual import steps.

The watched directory becomes the "inbox" for new assets:
1. AI writes `assets/incoming/paul/materials/crystal.lua`
2. Watcher detects new file
3. Import handler parses Lua, extracts metadata
4. Asset appears in database (and browser)
5. User can immediately use it

#### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      FileWatcher                                 │
│                                                                  │
│  Watch: assets/incoming/                                         │
│                                                                  │
│  On file change:                                                 │
│  1. Detect file type from extension                              │
│  2. Route to appropriate ImportHandler                           │
│  3. Handler extracts content + metadata                          │
│  4. Store in DatabaseStore                                       │
│  5. Generate embedding (async)                                   │
└─────────────────────────────────────────────────────────────────┘
                        │
                        │ uses
                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                    ImportHandler                                 │
│                                                                  │
│  LuaMaterialHandler:                                             │
│    - Parse material Lua                                          │
│    - Extract: name, color, description from Lua fields           │
│    - Asset type: "material"                                      │
│                                                                  │
│  LuaGeneratorHandler:                                            │
│    - Parse generator Lua                                         │
│    - Extract: name, description from Generator fields            │
│    - Asset type: "generator"                                     │
│                                                                  │
│  GenericLuaHandler:                                              │
│    - For unknown Lua, extract module name as asset name          │
└─────────────────────────────────────────────────────────────────┘
```

#### Directory Convention

```
assets/
├── incoming/           # Watched for auto-import
│   ├── paul/          # Namespace = "paul"
│   │   ├── materials/ # path prefix
│   │   │   └── crystal.lua → key = "paul/materials/crystal"
│   │   └── generators/
│   │       └── maze.lua → key = "paul/generators/maze"
│   └── shared/        # Namespace = "shared"
│       └── ...
└── ...
```

#### API Design

```rust
/// Handler for importing a specific file type.
pub trait ImportHandler: Send + Sync {
    /// File extensions this handler supports (e.g., ["lua"])
    fn extensions(&self) -> &[&str];
    
    /// Import file content into asset.
    /// Returns (content_to_store, extracted_metadata).
    fn import(&self, path: &Path, content: &[u8]) -> Result<(Vec<u8>, AssetMetadata), ImportError>;
}

/// Lua material file handler.
pub struct LuaMaterialHandler;

impl ImportHandler for LuaMaterialHandler {
    fn extensions(&self) -> &[&str] { &["lua"] }
    
    fn import(&self, path: &Path, content: &[u8]) -> Result<(Vec<u8>, AssetMetadata), ImportError> {
        let source = String::from_utf8_lossy(content);
        
        // Parse Lua to extract metadata
        let lua = Lua::new();
        let table: mlua::Table = lua.load(&source).eval()?;
        
        let name = table.get::<String>("name")
            .unwrap_or_else(|_| path.file_stem().unwrap().to_string_lossy().into());
        let description = table.get::<String>("description").ok();
        let tags = table.get::<Vec<String>>("tags").unwrap_or_default();
        
        Ok((content.to_vec(), AssetMetadata {
            name,
            description,
            tags,
            asset_type: "material".to_string(),
            updated_at: chrono::Utc::now(),
        }))
    }
}

/// File system watcher with auto-import.
pub struct AssetFileWatcher {
    watcher: notify::RecommendedWatcher,
    handlers: HashMap<String, Box<dyn ImportHandler>>,
    store: Arc<DatabaseStore>,
    watch_dir: PathBuf,
}

impl AssetFileWatcher {
    pub fn new(watch_dir: &Path, store: Arc<DatabaseStore>) -> Result<Self, WatchError>;
    
    /// Register a handler for file extension.
    pub fn register_handler(&mut self, handler: Box<dyn ImportHandler>);
    
    /// Start watching (spawns background thread).
    pub fn start(&mut self) -> Result<(), WatchError>;
    
    /// Process a file change event.
    fn on_file_change(&self, path: &Path, event: notify::EventKind) {
        match event {
            notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                self.import_file(path);
            }
            notify::EventKind::Remove(_) => {
                self.remove_asset(path);
            }
            _ => {}
        }
    }
    
    fn import_file(&self, path: &Path) {
        // Determine namespace and asset path from file path
        let relative = path.strip_prefix(&self.watch_dir).ok()?;
        let namespace = relative.components().next()?.as_os_str().to_string_lossy();
        let asset_path = relative.components().skip(1).collect::<PathBuf>();
        
        // Get handler for extension
        let ext = path.extension()?.to_string_lossy();
        let handler = self.handlers.get(&ext)?;
        
        // Read and import
        let content = std::fs::read(path).ok()?;
        let (stored_content, metadata) = handler.import(path, &content).ok()?;
        
        let key = AssetKey {
            namespace: namespace.into(),
            path: asset_path.to_string_lossy().into(),
        };
        
        self.store.set(&key, &stored_content, metadata).ok()?;
        
        // Queue embedding generation (async)
        self.queue_embedding(&key);
    }
}
```

#### Implementation Tasks

| # | Task | File | Verification |
|---|------|------|--------------|
| 1 | Add `notify` crate dependency | `Cargo.toml` | Compiles |
| 2 | Create `ImportHandler` trait | `map_editor/asset_store/import.rs` | Compiles |
| 3 | Implement `LuaMaterialHandler` | `map_editor/asset_store/import.rs` | Extracts metadata |
| 4 | Implement `LuaGeneratorHandler` | `map_editor/asset_store/import.rs` | Extracts metadata |
| 5 | Create `AssetFileWatcher` | `map_editor/asset_store/watcher.rs` | Compiles |
| 6 | Implement file event handling | `map_editor/asset_store/watcher.rs` | Imports on create |
| 7 | Handle file deletion | `map_editor/asset_store/watcher.rs` | Removes from DB |
| 8 | Add async embedding queue | `map_editor/asset_store/watcher.rs` | Embeddings generated |
| 9 | Integrate watcher into app | `map_editor/app.rs` | Starts on launch |
| 10 | Integration test: file drop → appears in DB | `tests/file_watcher_integration.rs` | Pass |

#### Verification

```bash
# 1. Build succeeds
cargo build --example p_map_editor_2d

# 2. Run app
cargo run --example p_map_editor_2d &
sleep 6

# 3. Create watched directory structure
mkdir -p assets/incoming/test/materials

# 4. Write a file (simulating AI or human)
cat > assets/incoming/test/materials/ruby.lua << 'EOF'
return {
    name = "Ruby",
    description = "Deep red precious gemstone",
    color = {0.9, 0.1, 0.1},
    tags = {"gem", "red", "precious"}
}
EOF

# 5. Wait for import (should be < 2 seconds)
sleep 2

# 6. Verify asset appears in database via MCP
curl "http://127.0.0.1:8088/mcp/assets?namespace=test"
# Should include: {"key": "test/materials/ruby", "name": "Ruby", ...}

# 7. Search for it
curl "http://127.0.0.1:8088/mcp/search?q=precious"
# Should return ruby

# 8. Verify in browser (manual)
# Asset Browser should show test/materials/ruby

# 9. Delete file and verify removal
rm assets/incoming/test/materials/ruby.lua
sleep 2
curl "http://127.0.0.1:8088/mcp/assets?namespace=test"
# Should NOT include ruby

pkill -f p_map_editor_2d
```

---

## Files Changed Summary

### New Files

| File | Purpose |
|------|---------|
| `map_editor/asset_store/mod.rs` | `AssetStore` trait, `AssetKey`, `AssetMetadata` |
| `map_editor/asset_store/database.rs` | `DatabaseStore` SQLite implementation |
| `map_editor/asset_store/embedding.rs` | `EmbeddingProvider` trait, `LocalEmbedding` |
| `map_editor/asset_store/import.rs` | `ImportHandler` trait, Lua handlers |
| `map_editor/asset_store/watcher.rs` | `AssetFileWatcher` |
| `map_editor/asset_store/material_store.rs` | `MaterialStore` wrapper |
| `map_editor/asset_store/generator_store.rs` | `GeneratorStore` wrapper |
| `map_editor/ui/asset_browser.rs` | `AssetBrowser` UI component |
| `scripts/download_embedding_model.sh` | Model download script |
| `tests/asset_store_integration.rs` | Integration tests |
| `tests/semantic_search_integration.rs` | Semantic search tests |
| `tests/file_watcher_integration.rs` | File watcher tests |

### Modified Files

| File | Change |
|------|--------|
| `map_editor/mod.rs` | Export asset_store module |
| `map_editor/mcp_server.rs` | Add asset CRUD endpoints, search endpoints |
| `map_editor/app.rs` | Initialize AssetStore, FileWatcher, integrate AssetBrowser (imgui) |
| `Cargo.toml` | Add rusqlite, notify, candle-core, candle-transformers, tokenizers, hf-hub |

---

## Dependencies

```
Phase 3.5 Complete
    │
    ▼
M11: Database-Backed Asset Store
    │
    ▼
M11.5: MCP Universal Asset CRUD  ◄── AI can write any asset type here
    │
    ├───────────────────┐
    ▼                   ▼
M12: Semantic Search   M14: File Watcher
    │                   │
    └───────┬───────────┘
            ▼
    M13: Asset Browser Panel (imgui)
            │
            ▼
    Phase 5 (3D Upgrade)
```

**M11 → M11.5:** CRUD endpoints need database store
**M11.5 → M12, M14:** Search and watcher build on CRUD primitives
**M12, M14 → M13:** Browser shows searchable, auto-imported assets

---

## Estimated Time

| Milestone | Time |
|-----------|------|
| M11 (Database Store) | 8-10 hours |
| M11.5 (MCP Universal CRUD) | 2-3 hours |
| M12 (Semantic Search) | 6-8 hours |
| M13 (Asset Browser - imgui) | 6-8 hours |
| M14 (File Watcher) | 4-6 hours |
| **Total** | **26-35 hours** |

---

## Cleanup Notes (To Review at Phase End)

Anticipated cleanup items:

- [ ] Should existing file-based loaders be deprecated?
- [ ] Is `AssetKey` format (`namespace/path`) consistent with Lua require paths?
- [ ] Should embeddings be computed eagerly (on insert) or lazily (on first search)?
- [ ] How to handle asset versioning? (Future work, but consider schema)
- [ ] Should the browser support drag-and-drop import?

---

## Phase 4 Verification Script

```bash
#!/bin/bash
set -e

echo "=== Phase 4 Verification ==="

cargo run --example p_map_editor_2d &
APP_PID=$!
sleep 8

# M11: Database Store
echo "M11: Database Store..."
curl -s -X POST http://127.0.0.1:8088/mcp/assets -H "Content-Type: application/json" \
  -d '{"namespace":"test","path":"materials/test","asset_type":"material","content":"return {}","metadata":{"name":"Test","tags":[],"asset_type":"material"}}' \
  | grep -q "ok" && echo "PASS: create asset via POST"

curl -s "http://127.0.0.1:8088/mcp/search?q=test" | grep -q "test/materials/test" && echo "PASS: text search"

# M11.5: MCP Universal CRUD
echo "M11.5: MCP Universal CRUD..."
# PUT create
curl -s -X PUT "http://127.0.0.1:8088/mcp/asset/test/materials/ruby" \
  -H "Content-Type: application/json" \
  -d '{"asset_type":"material","content":"return {name=\"Ruby\"}"}' \
  | grep -q "ok" && echo "PASS: PUT create"

# GET raw content
curl -s "http://127.0.0.1:8088/mcp/asset/test/materials/ruby" | grep -q "Ruby" && echo "PASS: GET raw content"

# GET with metadata
curl -s "http://127.0.0.1:8088/mcp/asset/test/materials/ruby?include_metadata=true" | grep -q "metadata" && echo "PASS: GET with metadata"

# DELETE
curl -s -X DELETE "http://127.0.0.1:8088/mcp/asset/test/materials/ruby" | grep -q "deleted" && echo "PASS: DELETE"

# PUT generator (same endpoint pattern)
curl -s -X PUT "http://127.0.0.1:8088/mcp/asset/test/generators/test_gen" \
  -H "Content-Type: application/json" \
  -d '{"asset_type":"generator","content":"return {}"}' \
  | grep -q "ok" && echo "PASS: PUT generator (same endpoint)"

# M12: Semantic Search
echo "M12: Semantic Search..."
curl -s "http://127.0.0.1:8088/mcp/search_semantic?q=gemstone" | grep -q "score" && echo "PASS: semantic search"

# M13: Asset Browser (manual verification required)
echo "M13: Asset Browser (imgui) - Manual verification required"
echo "  - Verify Asset Browser panel visible in imgui"
echo "  - Verify TreeNode navigation works"
echo "  - Verify Load button triggers BrowserAction"

# M14: File Watcher
echo "M14: File Watcher..."
mkdir -p assets/incoming/verify/materials
echo 'return {name="Verify",tags={}}' > assets/incoming/verify/materials/verify.lua
sleep 3
curl -s "http://127.0.0.1:8088/mcp/assets?namespace=verify" | grep -q "verify/materials/verify" && echo "PASS: file watcher import"
rm assets/incoming/verify/materials/verify.lua
sleep 2
curl -s "http://127.0.0.1:8088/mcp/assets?namespace=verify" | grep -q "verify/materials/verify" || echo "PASS: file watcher delete"

kill $APP_PID 2>/dev/null
rmdir -p assets/incoming/verify/materials 2>/dev/null || true
echo "=== Phase 4 Complete ==="
```

---

## Summary

Phase 4 transforms the map editor from a file-based tool into a database-backed asset management system:

1. **M11** establishes the `AssetStore<T>` trait and SQLite backend
2. **M11.5** exposes universal CRUD via MCP (`PUT/GET/DELETE /mcp/asset/{ns}/{path}`)
3. **M12** adds semantic search via Candle ML embeddings (pure Rust, no ONNX)
4. **M13** provides a unified imgui browser for all assets
5. **M14** enables auto-import from watched directories

**Key design choices:**
- **Candle ML** for embeddings (pure Rust, auto-downloads from HuggingFace)
- **imgui** for UI (consistent with rest of project)
- **RESTful asset paths** (`/mcp/asset/namespace/path`) for AI-friendly CRUD

The foundation enables Phase 5 (3D) to add new asset types (3D materials, meshes) without changing the storage infrastructure. It also enables future features like:
- Asset versioning and history
- Multi-user collaboration (shared namespaces)
- Asset packages (export/import bundles)
- Cloud sync (swap SQLite for remote DB)
