//! Database-backed asset store using SQLite.
//!
//! Provides persistent storage for all Lua assets (materials, generators, renderers, visualizers)
//! with full-text search support.

use bevy::prelude::Resource;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Error type for asset store operations.
#[derive(Debug)]
pub enum AssetError {
    /// Asset not found.
    NotFound,
    /// Database error.
    Database(rusqlite::Error),
    /// Serialization error.
    Serialization(serde_json::Error),
    /// Invalid key format.
    InvalidKey(String),
    /// UTF-8 conversion error.
    Utf8(std::string::FromUtf8Error),
    /// I/O error.
    Io(std::io::Error),
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetError::NotFound => write!(f, "Asset not found"),
            AssetError::Database(e) => write!(f, "Database error: {}", e),
            AssetError::Serialization(e) => write!(f, "Serialization error: {}", e),
            AssetError::InvalidKey(k) => write!(f, "Invalid key format: {}", k),
            AssetError::Utf8(e) => write!(f, "UTF-8 error: {}", e),
            AssetError::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for AssetError {}

impl From<rusqlite::Error> for AssetError {
    fn from(e: rusqlite::Error) -> Self {
        AssetError::Database(e)
    }
}

impl From<serde_json::Error> for AssetError {
    fn from(e: serde_json::Error) -> Self {
        AssetError::Serialization(e)
    }
}

impl From<std::string::FromUtf8Error> for AssetError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        AssetError::Utf8(e)
    }
}

impl From<std::io::Error> for AssetError {
    fn from(e: std::io::Error) -> Self {
        AssetError::Io(e)
    }
}

/// Key for locating an asset in the store.
///
/// Format: `namespace/path` where namespace is typically a username or "shared",
/// and path is the asset location (e.g., "materials/crystal").
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetKey {
    /// User or organization (e.g., "paul", "anomaly", "shared")
    pub namespace: String,
    /// Path within namespace (e.g., "materials/crystal")
    pub path: String,
}

impl AssetKey {
    /// Create a new asset key.
    pub fn new(namespace: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            path: path.into(),
        }
    }

    /// Parse from string like "paul/materials/crystal".
    /// First component is namespace, rest is path.
    /// Leading slash is trimmed, but namespace must not be empty.
    pub fn parse(s: &str) -> Option<Self> {
        // Don't trim leading slash - if it starts with /, the first component would be empty
        let mut parts = s.splitn(2, '/');
        let namespace = parts.next()?.to_string();
        let path = parts.next()?.to_string();

        // If input started with /, namespace would be empty
        if namespace.is_empty() || path.is_empty() {
            return None;
        }

        Some(Self { namespace, path })
    }

    /// Format as "namespace/path".
    pub fn to_key_string(&self) -> String {
        format!("{}/{}", self.namespace, self.path)
    }
}

impl std::fmt::Display for AssetKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.namespace, self.path)
    }
}

/// Metadata attached to every asset.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetMetadata {
    /// Display name.
    pub name: String,
    /// Human-readable description (used for semantic search).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Searchable tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Asset type: "material", "generator", "renderer", "visualizer".
    pub asset_type: String,
    /// When last modified.
    pub updated_at: DateTime<Utc>,
}

impl AssetMetadata {
    /// Create metadata with minimal fields.
    pub fn new(name: impl Into<String>, asset_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            tags: Vec::new(),
            asset_type: asset_type.into(),
            updated_at: Utc::now(),
        }
    }

    /// Set description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

/// Reference to an asset without loading its content.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetRef {
    pub key: AssetKey,
    pub metadata: AssetMetadata,
}

/// SQLite-backed asset storage.
///
/// Thread-safe via internal Mutex. Stores assets as blobs with JSON metadata.
/// Supports full-text search via FTS5.
#[derive(Resource)]
pub struct DatabaseStore {
    conn: Arc<Mutex<Connection>>,
}

impl DatabaseStore {
    /// Open or create a database at the given path.
    pub fn open(path: &Path) -> Result<Self, AssetError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// Create an in-memory database (for testing).
    pub fn open_in_memory() -> Result<Self, AssetError> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// Initialize database schema.
    fn init_schema(&self) -> Result<(), AssetError> {
        let conn = self.conn.lock().unwrap();

        // Main assets table
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS assets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace TEXT NOT NULL,
                path TEXT NOT NULL,
                asset_type TEXT NOT NULL,
                content BLOB NOT NULL,
                metadata TEXT NOT NULL,
                embedding BLOB,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(namespace, path)
            )
            "#,
            [],
        )?;

        // Indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_assets_namespace ON assets(namespace)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(asset_type)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_assets_updated ON assets(updated_at)",
            [],
        )?;

        // Full-text search table
        conn.execute(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
                name, description, tags, asset_type,
                content='assets',
                content_rowid='id'
            )
            "#,
            [],
        )?;

        // Triggers to keep FTS in sync
        // Insert trigger
        conn.execute(
            r#"
            CREATE TRIGGER IF NOT EXISTS assets_ai AFTER INSERT ON assets BEGIN
                INSERT INTO assets_fts(rowid, name, description, tags, asset_type)
                VALUES (
                    new.id,
                    json_extract(new.metadata, '$.name'),
                    COALESCE(json_extract(new.metadata, '$.description'), ''),
                    COALESCE((
                        SELECT group_concat(value, ' ')
                        FROM json_each(new.metadata, '$.tags')
                    ), ''),
                    new.asset_type
                );
            END
            "#,
            [],
        )?;

        // Delete trigger
        conn.execute(
            r#"
            CREATE TRIGGER IF NOT EXISTS assets_ad AFTER DELETE ON assets BEGIN
                INSERT INTO assets_fts(assets_fts, rowid, name, description, tags, asset_type)
                VALUES (
                    'delete',
                    old.id,
                    json_extract(old.metadata, '$.name'),
                    COALESCE(json_extract(old.metadata, '$.description'), ''),
                    COALESCE((
                        SELECT group_concat(value, ' ')
                        FROM json_each(old.metadata, '$.tags')
                    ), ''),
                    old.asset_type
                );
            END
            "#,
            [],
        )?;

        // Update trigger
        conn.execute(
            r#"
            CREATE TRIGGER IF NOT EXISTS assets_au AFTER UPDATE ON assets BEGIN
                INSERT INTO assets_fts(assets_fts, rowid, name, description, tags, asset_type)
                VALUES (
                    'delete',
                    old.id,
                    json_extract(old.metadata, '$.name'),
                    COALESCE(json_extract(old.metadata, '$.description'), ''),
                    COALESCE((
                        SELECT group_concat(value, ' ')
                        FROM json_each(old.metadata, '$.tags')
                    ), ''),
                    old.asset_type
                );
                INSERT INTO assets_fts(rowid, name, description, tags, asset_type)
                VALUES (
                    new.id,
                    json_extract(new.metadata, '$.name'),
                    COALESCE(json_extract(new.metadata, '$.description'), ''),
                    COALESCE((
                        SELECT group_concat(value, ' ')
                        FROM json_each(new.metadata, '$.tags')
                    ), ''),
                    new.asset_type
                );
            END
            "#,
            [],
        )?;

        Ok(())
    }

    /// Get asset content by key.
    pub fn get(&self, key: &AssetKey) -> Result<Option<Vec<u8>>, AssetError> {
        let conn = self.conn.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT content FROM assets WHERE namespace = ?1 AND path = ?2",
                params![key.namespace, key.path],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?;
        Ok(result)
    }

    /// Get asset metadata by key.
    pub fn get_metadata(&self, key: &AssetKey) -> Result<Option<AssetMetadata>, AssetError> {
        let conn = self.conn.lock().unwrap();
        let result: Option<String> = conn
            .query_row(
                "SELECT metadata FROM assets WHERE namespace = ?1 AND path = ?2",
                params![key.namespace, key.path],
                |row| row.get(0),
            )
            .optional()?;

        match result {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    /// Get asset content and metadata together.
    pub fn get_full(&self, key: &AssetKey) -> Result<Option<(Vec<u8>, AssetMetadata)>, AssetError> {
        let conn = self.conn.lock().unwrap();
        let result: Option<(Vec<u8>, String)> = conn
            .query_row(
                "SELECT content, metadata FROM assets WHERE namespace = ?1 AND path = ?2",
                params![key.namespace, key.path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        match result {
            Some((content, json)) => {
                let metadata: AssetMetadata = serde_json::from_str(&json)?;
                Ok(Some((content, metadata)))
            }
            None => Ok(None),
        }
    }

    /// Store asset content and metadata.
    /// Creates or updates the asset.
    pub fn set(
        &self,
        key: &AssetKey,
        content: &[u8],
        metadata: AssetMetadata,
    ) -> Result<(), AssetError> {
        let conn = self.conn.lock().unwrap();
        let metadata_json = serde_json::to_string(&metadata)?;

        conn.execute(
            r#"
            INSERT INTO assets (namespace, path, asset_type, content, metadata, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
            ON CONFLICT(namespace, path) DO UPDATE SET
                asset_type = excluded.asset_type,
                content = excluded.content,
                metadata = excluded.metadata,
                updated_at = datetime('now')
            "#,
            params![
                key.namespace,
                key.path,
                metadata.asset_type,
                content,
                metadata_json
            ],
        )?;

        Ok(())
    }

    /// Delete asset.
    /// Returns true if asset existed and was deleted.
    pub fn delete(&self, key: &AssetKey) -> Result<bool, AssetError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM assets WHERE namespace = ?1 AND path = ?2",
            params![key.namespace, key.path],
        )?;
        Ok(rows > 0)
    }

    /// List assets matching glob pattern within namespace.
    /// Pattern examples: "materials/*", "generators/**", "*"
    pub fn list(
        &self,
        namespace: &str,
        pattern: &str,
        asset_type: Option<&str>,
    ) -> Result<Vec<AssetRef>, AssetError> {
        let conn = self.conn.lock().unwrap();

        // Convert glob pattern to SQL LIKE pattern
        let sql_pattern = glob_to_like(pattern);

        let mut results = Vec::new();

        if let Some(at) = asset_type {
            let mut stmt = conn.prepare(
                "SELECT namespace, path, metadata FROM assets WHERE namespace = ?1 AND path LIKE ?2 AND asset_type = ?3 ORDER BY updated_at DESC"
            )?;
            let rows = stmt.query_map(params![namespace, sql_pattern, at], |row| {
                let ns: String = row.get(0)?;
                let path: String = row.get(1)?;
                let metadata_json: String = row.get(2)?;
                Ok((ns, path, metadata_json))
            })?;
            for row in rows {
                let (ns, path, metadata_json) = row?;
                let metadata: AssetMetadata = serde_json::from_str(&metadata_json)?;
                results.push(AssetRef {
                    key: AssetKey::new(ns, path),
                    metadata,
                });
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT namespace, path, metadata FROM assets WHERE namespace = ?1 AND path LIKE ?2 ORDER BY updated_at DESC"
            )?;
            let rows = stmt.query_map(params![namespace, sql_pattern], |row| {
                let ns: String = row.get(0)?;
                let path: String = row.get(1)?;
                let metadata_json: String = row.get(2)?;
                Ok((ns, path, metadata_json))
            })?;
            for row in rows {
                let (ns, path, metadata_json) = row?;
                let metadata: AssetMetadata = serde_json::from_str(&metadata_json)?;
                results.push(AssetRef {
                    key: AssetKey::new(ns, path),
                    metadata,
                });
            }
        }

        Ok(results)
    }

    /// List all assets in a namespace.
    pub fn list_all(&self, namespace: &str) -> Result<Vec<AssetRef>, AssetError> {
        self.list(namespace, "%", None)
    }

    /// List all namespaces.
    pub fn list_namespaces(&self) -> Result<Vec<String>, AssetError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT DISTINCT namespace FROM assets ORDER BY namespace")?;
        let rows = stmt.query_map([], |row| row.get(0))?;

        let mut namespaces = Vec::new();
        for row in rows {
            namespaces.push(row?);
        }
        Ok(namespaces)
    }

    /// Text search across name, description, tags using FTS5.
    pub fn search(
        &self,
        query: &str,
        asset_type: Option<&str>,
    ) -> Result<Vec<AssetRef>, AssetError> {
        let conn = self.conn.lock().unwrap();

        // Escape special FTS5 characters and prepare query
        let fts_query = escape_fts_query(query);

        let mut results = Vec::new();

        if let Some(at) = asset_type {
            let mut stmt = conn.prepare(
                r#"
                SELECT a.namespace, a.path, a.metadata
                FROM assets a
                JOIN assets_fts f ON a.id = f.rowid
                WHERE assets_fts MATCH ?1 AND a.asset_type = ?2
                ORDER BY rank
                "#,
            )?;
            let rows = stmt.query_map(params![fts_query, at], |row| {
                let ns: String = row.get(0)?;
                let path: String = row.get(1)?;
                let metadata_json: String = row.get(2)?;
                Ok((ns, path, metadata_json))
            })?;
            for row in rows {
                let (ns, path, metadata_json) = row?;
                let metadata: AssetMetadata = serde_json::from_str(&metadata_json)?;
                results.push(AssetRef {
                    key: AssetKey::new(ns, path),
                    metadata,
                });
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT a.namespace, a.path, a.metadata
                FROM assets a
                JOIN assets_fts f ON a.id = f.rowid
                WHERE assets_fts MATCH ?1
                ORDER BY rank
                "#,
            )?;
            let rows = stmt.query_map(params![fts_query], |row| {
                let ns: String = row.get(0)?;
                let path: String = row.get(1)?;
                let metadata_json: String = row.get(2)?;
                Ok((ns, path, metadata_json))
            })?;
            for row in rows {
                let (ns, path, metadata_json) = row?;
                let metadata: AssetMetadata = serde_json::from_str(&metadata_json)?;
                results.push(AssetRef {
                    key: AssetKey::new(ns, path),
                    metadata,
                });
            }
        }

        Ok(results)
    }

    /// Simple substring search (fallback when FTS fails or for simple queries).
    pub fn search_simple(
        &self,
        query: &str,
        asset_type: Option<&str>,
    ) -> Result<Vec<AssetRef>, AssetError> {
        let conn = self.conn.lock().unwrap();
        let like_query = format!("%{}%", query.to_lowercase());

        let mut results = Vec::new();

        if let Some(at) = asset_type {
            let mut stmt = conn.prepare(
                r#"
                SELECT namespace, path, metadata FROM assets
                WHERE asset_type = ?2 AND (
                    LOWER(json_extract(metadata, '$.name')) LIKE ?1
                    OR LOWER(COALESCE(json_extract(metadata, '$.description'), '')) LIKE ?1
                )
                ORDER BY updated_at DESC
                "#,
            )?;
            let rows = stmt.query_map(params![like_query, at], |row| {
                let ns: String = row.get(0)?;
                let path: String = row.get(1)?;
                let metadata_json: String = row.get(2)?;
                Ok((ns, path, metadata_json))
            })?;
            for row in rows {
                let (ns, path, metadata_json) = row?;
                let metadata: AssetMetadata = serde_json::from_str(&metadata_json)?;
                results.push(AssetRef {
                    key: AssetKey::new(ns, path),
                    metadata,
                });
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT namespace, path, metadata FROM assets
                WHERE LOWER(json_extract(metadata, '$.name')) LIKE ?1
                   OR LOWER(COALESCE(json_extract(metadata, '$.description'), '')) LIKE ?1
                ORDER BY updated_at DESC
                "#,
            )?;
            let rows = stmt.query_map(params![like_query], |row| {
                let ns: String = row.get(0)?;
                let path: String = row.get(1)?;
                let metadata_json: String = row.get(2)?;
                Ok((ns, path, metadata_json))
            })?;
            for row in rows {
                let (ns, path, metadata_json) = row?;
                let metadata: AssetMetadata = serde_json::from_str(&metadata_json)?;
                results.push(AssetRef {
                    key: AssetKey::new(ns, path),
                    metadata,
                });
            }
        }

        Ok(results)
    }

    /// Store embedding vector for an asset.
    pub fn set_embedding(&self, key: &AssetKey, embedding: &[f32]) -> Result<(), AssetError> {
        let conn = self.conn.lock().unwrap();

        // Convert f32 slice to bytes
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        conn.execute(
            "UPDATE assets SET embedding = ?3 WHERE namespace = ?1 AND path = ?2",
            params![key.namespace, key.path, bytes],
        )?;

        Ok(())
    }

    /// Get embedding vector for an asset.
    pub fn get_embedding(&self, key: &AssetKey) -> Result<Option<Vec<f32>>, AssetError> {
        let conn = self.conn.lock().unwrap();
        let result: Option<Vec<u8>> = conn
            .query_row(
                "SELECT embedding FROM assets WHERE namespace = ?1 AND path = ?2",
                params![key.namespace, key.path],
                |row| row.get(0),
            )
            .optional()?;

        match result {
            Some(bytes) if !bytes.is_empty() => {
                // Convert bytes back to f32
                let floats: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();
                Ok(Some(floats))
            }
            _ => Ok(None),
        }
    }

    /// Semantic search using cosine similarity with stored embeddings.
    /// Returns assets sorted by similarity (highest first).
    pub fn search_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(AssetRef, f32)>, AssetError> {
        let conn = self.conn.lock().unwrap();

        // Get all assets with embeddings
        let mut stmt = conn.prepare(
            "SELECT namespace, path, metadata, embedding FROM assets WHERE embedding IS NOT NULL",
        )?;

        let rows = stmt.query_map([], |row| {
            let ns: String = row.get(0)?;
            let path: String = row.get(1)?;
            let metadata_json: String = row.get(2)?;
            let embedding_bytes: Vec<u8> = row.get(3)?;
            Ok((ns, path, metadata_json, embedding_bytes))
        })?;

        // Calculate cosine similarity for each
        let mut scored: Vec<(AssetRef, f32)> = Vec::new();

        for row in rows {
            let (ns, path, metadata_json, embedding_bytes) = row?;

            // Convert bytes to f32
            let embedding: Vec<f32> = embedding_bytes
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();

            // Calculate cosine similarity
            let score = cosine_similarity(query_embedding, &embedding);

            let metadata: AssetMetadata = serde_json::from_str(&metadata_json)?;
            scored.push((
                AssetRef {
                    key: AssetKey::new(ns, path),
                    metadata,
                },
                score,
            ));
        }

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top N
        scored.truncate(limit);

        Ok(scored)
    }

    /// Check if an asset exists.
    pub fn exists(&self, key: &AssetKey) -> Result<bool, AssetError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM assets WHERE namespace = ?1 AND path = ?2",
            params![key.namespace, key.path],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Count assets in namespace (optionally filtered by type).
    pub fn count_all(
        &self,
        namespace: Option<&str>,
        asset_type: Option<&str>,
    ) -> Result<usize, AssetError> {
        let conn = self.conn.lock().unwrap();

        let count: i64 = match (namespace, asset_type) {
            (Some(ns), Some(at)) => conn.query_row(
                "SELECT COUNT(*) FROM assets WHERE namespace = ?1 AND asset_type = ?2",
                params![ns, at],
                |row| row.get(0),
            )?,
            (Some(ns), None) => conn.query_row(
                "SELECT COUNT(*) FROM assets WHERE namespace = ?1",
                params![ns],
                |row| row.get(0),
            )?,
            (None, Some(at)) => conn.query_row(
                "SELECT COUNT(*) FROM assets WHERE asset_type = ?1",
                params![at],
                |row| row.get(0),
            )?,
            (None, None) => conn.query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))?,
        };

        Ok(count as usize)
    }
}

use super::BlobStore;

impl BlobStore for DatabaseStore {
    fn get(&self, key: &AssetKey) -> Result<Option<Vec<u8>>, AssetError> {
        DatabaseStore::get(self, key)
    }

    fn get_metadata(&self, key: &AssetKey) -> Result<Option<AssetMetadata>, AssetError> {
        DatabaseStore::get_metadata(self, key)
    }

    fn get_full(&self, key: &AssetKey) -> Result<Option<(Vec<u8>, AssetMetadata)>, AssetError> {
        DatabaseStore::get_full(self, key)
    }

    fn set(
        &self,
        key: &AssetKey,
        content: &[u8],
        metadata: AssetMetadata,
    ) -> Result<(), AssetError> {
        DatabaseStore::set(self, key, content, metadata)
    }

    fn delete(&self, key: &AssetKey) -> Result<bool, AssetError> {
        DatabaseStore::delete(self, key)
    }

    fn list(
        &self,
        namespace: &str,
        pattern: &str,
        asset_type: Option<&str>,
    ) -> Result<Vec<AssetRef>, AssetError> {
        DatabaseStore::list(self, namespace, pattern, asset_type)
    }

    fn search(&self, query: &str, asset_type: Option<&str>) -> Result<Vec<AssetRef>, AssetError> {
        // Try FTS first, fall back to simple search
        DatabaseStore::search(self, query, asset_type)
            .or_else(|_| DatabaseStore::search_simple(self, query, asset_type))
    }

    fn exists(&self, key: &AssetKey) -> Result<bool, AssetError> {
        DatabaseStore::exists(self, key)
    }

    fn count(&self, namespace: &str, asset_type: Option<&str>) -> Result<usize, AssetError> {
        DatabaseStore::count_all(self, Some(namespace), asset_type)
    }
}

/// Convert glob pattern to SQL LIKE pattern.
fn glob_to_like(pattern: &str) -> String {
    pattern.replace('*', "%").replace('?', "_")
}

/// Escape special FTS5 characters for safe querying.
fn escape_fts_query(query: &str) -> String {
    // For simple queries, just wrap each word in quotes
    query
        .split_whitespace()
        .map(|word| {
            // Remove special chars that could break FTS
            let clean: String = word
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if clean.is_empty() {
                String::new()
            } else {
                format!("\"{}\"", clean)
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Calculate cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_key_parse() {
        let key = AssetKey::parse("paul/materials/crystal").unwrap();
        assert_eq!(key.namespace, "paul");
        assert_eq!(key.path, "materials/crystal");
        assert_eq!(key.to_key_string(), "paul/materials/crystal");

        // Invalid - no path
        assert!(AssetKey::parse("paul").is_none());

        // Invalid - empty
        assert!(AssetKey::parse("").is_none());

        // Invalid - empty namespace (leading slash)
        assert!(AssetKey::parse("/materials/crystal").is_none());

        // Valid - nested path
        let key = AssetKey::parse("shared/generators/dungeon/maze").unwrap();
        assert_eq!(key.namespace, "shared");
        assert_eq!(key.path, "generators/dungeon/maze");
    }

    #[test]
    fn test_database_store_basic() {
        let store = DatabaseStore::open_in_memory().unwrap();

        let key = AssetKey::new("test", "materials/stone");
        let content = b"return { name = 'Stone' }";
        let metadata = AssetMetadata::new("Stone", "material")
            .with_description("A basic stone material")
            .with_tags(vec!["terrain".to_string(), "natural".to_string()]);

        // Set
        store.set(&key, content, metadata.clone()).unwrap();

        // Get content
        let retrieved = store.get(&key).unwrap().unwrap();
        assert_eq!(retrieved, content);

        // Get metadata
        let meta = store.get_metadata(&key).unwrap().unwrap();
        assert_eq!(meta.name, "Stone");
        assert_eq!(meta.asset_type, "material");
        assert_eq!(meta.description, Some("A basic stone material".to_string()));

        // Exists
        assert!(store.exists(&key).unwrap());

        // Delete
        assert!(store.delete(&key).unwrap());
        assert!(!store.exists(&key).unwrap());
        assert!(store.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_database_store_upsert() {
        let store = DatabaseStore::open_in_memory().unwrap();

        let key = AssetKey::new("test", "materials/stone");

        // Initial set
        store
            .set(&key, b"v1", AssetMetadata::new("Stone v1", "material"))
            .unwrap();

        // Update
        store
            .set(&key, b"v2", AssetMetadata::new("Stone v2", "material"))
            .unwrap();

        // Should have updated content
        let content = store.get(&key).unwrap().unwrap();
        assert_eq!(content, b"v2");

        let meta = store.get_metadata(&key).unwrap().unwrap();
        assert_eq!(meta.name, "Stone v2");
    }

    #[test]
    fn test_database_store_list() {
        let store = DatabaseStore::open_in_memory().unwrap();

        // Add some assets
        store
            .set(
                &AssetKey::new("paul", "materials/stone"),
                b"stone",
                AssetMetadata::new("Stone", "material"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("paul", "materials/dirt"),
                b"dirt",
                AssetMetadata::new("Dirt", "material"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("paul", "generators/maze"),
                b"maze",
                AssetMetadata::new("Maze", "generator"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("shared", "materials/water"),
                b"water",
                AssetMetadata::new("Water", "material"),
            )
            .unwrap();

        // List all in namespace
        let results = store.list_all("paul").unwrap();
        assert_eq!(results.len(), 3);

        // List with pattern
        let results = store.list("paul", "materials/%", None).unwrap();
        assert_eq!(results.len(), 2);

        // List with type filter
        let results = store.list("paul", "%", Some("generator")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata.name, "Maze");

        // List namespaces
        let namespaces = store.list_namespaces().unwrap();
        assert_eq!(namespaces.len(), 2);
        assert!(namespaces.contains(&"paul".to_string()));
        assert!(namespaces.contains(&"shared".to_string()));
    }

    #[test]
    fn test_database_store_search() {
        let store = DatabaseStore::open_in_memory().unwrap();

        store
            .set(
                &AssetKey::new("test", "materials/crystal"),
                b"crystal",
                AssetMetadata::new("Crystal", "material")
                    .with_description("A glowing blue gemstone"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("test", "materials/lava"),
                b"lava",
                AssetMetadata::new("Lava", "material").with_description("Hot molten rock"),
            )
            .unwrap();

        // Search by name
        let results = store.search_simple("crystal", None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata.name, "Crystal");

        // Search by description
        let results = store.search_simple("glowing", None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata.name, "Crystal");

        // Search with type filter
        let results = store.search_simple("crystal", Some("generator")).unwrap();
        assert_eq!(results.len(), 0);

        // Case insensitive
        let results = store.search_simple("CRYSTAL", None).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_database_store_fts_search() {
        let store = DatabaseStore::open_in_memory().unwrap();

        store
            .set(
                &AssetKey::new("test", "materials/crystal"),
                b"crystal",
                AssetMetadata::new("Crystal", "material")
                    .with_description("A glowing blue gemstone")
                    .with_tags(vec!["gem".to_string(), "blue".to_string()]),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("test", "materials/lava"),
                b"lava",
                AssetMetadata::new("Lava", "material").with_description("Hot molten rock"),
            )
            .unwrap();

        // FTS search
        let results = store.search("crystal", None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata.name, "Crystal");

        // FTS search by description word
        let results = store.search("glowing", None).unwrap();
        assert_eq!(results.len(), 1);

        // FTS search by tag
        let results = store.search("gem", None).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_database_store_count() {
        let store = DatabaseStore::open_in_memory().unwrap();

        store
            .set(
                &AssetKey::new("paul", "materials/stone"),
                b"stone",
                AssetMetadata::new("Stone", "material"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("paul", "generators/maze"),
                b"maze",
                AssetMetadata::new("Maze", "generator"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("shared", "materials/water"),
                b"water",
                AssetMetadata::new("Water", "material"),
            )
            .unwrap();

        assert_eq!(store.count_all(None, None).unwrap(), 3);
        assert_eq!(store.count_all(Some("paul"), None).unwrap(), 2);
        assert_eq!(store.count_all(None, Some("material")).unwrap(), 2);
        assert_eq!(store.count_all(Some("paul"), Some("material")).unwrap(), 1);
    }

    #[test]
    fn test_cosine_similarity() {
        // Identical vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        // Orthogonal vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < 0.001);

        // Opposite vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_embeddings() {
        let store = DatabaseStore::open_in_memory().unwrap();

        let key = AssetKey::new("test", "materials/crystal");
        store
            .set(&key, b"crystal", AssetMetadata::new("Crystal", "material"))
            .unwrap();

        // Set embedding
        let embedding = vec![0.1, 0.2, 0.3, 0.4];
        store.set_embedding(&key, &embedding).unwrap();

        // Get embedding
        let retrieved = store.get_embedding(&key).unwrap().unwrap();
        assert_eq!(retrieved.len(), 4);
        assert!((retrieved[0] - 0.1).abs() < 0.001);
        assert!((retrieved[3] - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_glob_to_like() {
        assert_eq!(glob_to_like("materials/*"), "materials/%");
        assert_eq!(glob_to_like("*"), "%");
        assert_eq!(glob_to_like("file?.txt"), "file_.txt");
        assert_eq!(glob_to_like("a/*/b"), "a/%/b");
    }

    // =========================================================================
    // MCP Integration Tests
    // =========================================================================
    // These test the BlobStore trait operations as used by MCP endpoints,
    // without needing to run the full HTTP server or Bevy app.

    use super::super::BlobStore;

    #[test]
    fn test_blobstore_create_and_list() {
        let store = DatabaseStore::open_in_memory().unwrap();

        // Create assets via BlobStore trait
        let key1 = AssetKey::new("test", "materials/crystal");
        let meta1 = AssetMetadata::new("Crystal", "material");
        BlobStore::set(&store, &key1, b"return {}", meta1).unwrap();

        let key2 = AssetKey::new("test", "generators/cave");
        let meta2 = AssetMetadata::new("Cave Gen", "generator");
        BlobStore::set(&store, &key2, b"-- cave", meta2).unwrap();

        // List all in namespace
        let results = BlobStore::list(&store, "test", "%", None).unwrap();
        assert_eq!(results.len(), 2);

        // List filtered by type
        let materials = BlobStore::list(&store, "test", "%", Some("material")).unwrap();
        assert_eq!(materials.len(), 1);
        assert_eq!(materials[0].key.path, "materials/crystal");
    }

    #[test]
    fn test_blobstore_search() {
        let store = DatabaseStore::open_in_memory().unwrap();

        // Create searchable assets
        let key1 = AssetKey::new("user", "materials/crystal");
        let meta1 = AssetMetadata::new("Crystal Material", "material")
            .with_description("A glowing blue gemstone");
        BlobStore::set(&store, &key1, b"return {}", meta1).unwrap();

        let key2 = AssetKey::new("user", "generators/maze");
        let meta2 = AssetMetadata::new("Maze Generator", "generator");
        BlobStore::set(&store, &key2, b"-- maze", meta2).unwrap();

        // Search by name
        let results = BlobStore::search(&store, "crystal", None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata.name, "Crystal Material");

        // Search with type filter
        let gen_results = BlobStore::search(&store, "maze", Some("generator")).unwrap();
        assert_eq!(gen_results.len(), 1);

        // Search that matches nothing
        let empty = BlobStore::search(&store, "nonexistent", None).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_blobstore_get_operations() {
        let store = DatabaseStore::open_in_memory().unwrap();

        let key = AssetKey::new("ns", "path/to/asset");
        let content = b"content here";
        let meta = AssetMetadata::new("Test Asset", "test")
            .with_description("A test")
            .with_tags(vec!["tag1".into(), "tag2".into()]);

        // Set
        BlobStore::set(&store, &key, content, meta.clone()).unwrap();

        // Get content
        let retrieved = BlobStore::get(&store, &key).unwrap().unwrap();
        assert_eq!(retrieved, content);

        // Get metadata
        let meta_out = BlobStore::get_metadata(&store, &key).unwrap().unwrap();
        assert_eq!(meta_out.name, "Test Asset");
        assert_eq!(meta_out.tags.len(), 2);

        // Get full
        let (content_out, meta_full) = BlobStore::get_full(&store, &key).unwrap().unwrap();
        assert_eq!(content_out, content);
        assert_eq!(meta_full.description, Some("A test".to_string()));

        // Exists
        assert!(BlobStore::exists(&store, &key).unwrap());
        assert!(!BlobStore::exists(&store, &AssetKey::new("ns", "other")).unwrap());

        // Count
        assert_eq!(BlobStore::count(&store, "ns", None).unwrap(), 1);
        assert_eq!(BlobStore::count(&store, "ns", Some("test")).unwrap(), 1);
        assert_eq!(BlobStore::count(&store, "ns", Some("other")).unwrap(), 0);

        // Delete
        assert!(BlobStore::delete(&store, &key).unwrap());
        assert!(!BlobStore::exists(&store, &key).unwrap());
        assert!(!BlobStore::delete(&store, &key).unwrap()); // Already deleted
    }

    #[test]
    fn test_blobstore_upsert() {
        let store = DatabaseStore::open_in_memory().unwrap();

        let key = AssetKey::new("test", "item");

        // Create
        let meta1 = AssetMetadata::new("Version 1", "type");
        BlobStore::set(&store, &key, b"v1", meta1).unwrap();
        assert_eq!(BlobStore::get(&store, &key).unwrap().unwrap(), b"v1");

        // Update (upsert)
        let meta2 = AssetMetadata::new("Version 2", "type");
        BlobStore::set(&store, &key, b"v2", meta2).unwrap();
        assert_eq!(BlobStore::get(&store, &key).unwrap().unwrap(), b"v2");

        let meta = BlobStore::get_metadata(&store, &key).unwrap().unwrap();
        assert_eq!(meta.name, "Version 2");

        // Still only 1 asset
        assert_eq!(BlobStore::count(&store, "test", None).unwrap(), 1);
    }
}
