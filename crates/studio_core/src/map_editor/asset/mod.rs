//! Generic asset system for the map editor.
//!
//! # Architecture
//!
//! Two distinct storage patterns serve different needs:
//!
//! ## 1. Typed Runtime Storage (`InMemoryStore<T>`)
//!
//! For in-memory collections of typed Rust objects with search:
//! - `Searchable` trait: Implemented by typed things (Material, LuaLayerDef)
//! - `InMemoryStore<T>`: Simple Vec-based storage with search
//!
//! Used by `MaterialPalette` and `LuaLayerRegistry` for runtime registries.
//!
//! ## 2. Blob Storage (`BlobStore`)
//!
//! For persisted serialized content (Lua source code):
//! - `BlobStore` trait: Storage for raw bytes with metadata
//! - `DatabaseStore`: SQLite-backed implementation with FTS5 search
//! - `InMemoryBlobStore`: In-memory implementation (no persistence, for testing)
//! - `AssetStoreResource`: Bevy resource wrapping any `BlobStore` implementation
//!
//! Backends are swappable via configuration:
//! - `--asset-db <path>` - Use SQLite database at path (default: assets.db)
//! - `--no-persist` - Use in-memory storage (assets lost on restart)
//!
//! # Example (Typed Storage)
//!
//! ```ignore
//! use studio_core::map_editor::asset::{InMemoryStore, Searchable};
//!
//! let mut store: InMemoryStore<Material> = InMemoryStore::new();
//! store.set(material);
//! let results = store.search("stone");
//! ```
//!
//! # Example (Blob Storage)
//!
//! ```ignore
//! use studio_core::map_editor::asset::{BlobStore, DatabaseStore, AssetKey, AssetMetadata};
//!
//! let store = DatabaseStore::open(Path::new("assets.db"))?;
//! let key = AssetKey::new("paul", "materials/crystal");
//! let metadata = AssetMetadata::new("Crystal", "material")
//!     .with_description("A glowing blue gemstone");
//! store.set(&key, b"return { name = 'Crystal' }", metadata)?;
//! ```
//!
//! # Example (Backend Swapping)
//!
//! ```ignore
//! use studio_core::map_editor::asset::{AssetStoreResource, DatabaseStore, InMemoryBlobStore};
//!
//! // Use database backend
//! let resource = AssetStoreResource::new(DatabaseStore::open(path)?);
//!
//! // Or use in-memory backend
//! let resource = AssetStoreResource::new(InMemoryBlobStore::new());
//!
//! // Both work through the same BlobStore trait
//! resource.set(&key, content, metadata)?;
//! ```

mod database;
pub mod embedding;
mod store;

pub use database::{AssetError, AssetKey, AssetMetadata, AssetRef, DatabaseStore};
pub use embedding::{
    CandleEmbedding, EmbedError, EmbedRequest, EmbeddingProvider, EmbeddingService, SharedEmbedding,
};
pub use store::{InMemoryBlobStore, InMemoryStore};
// Note: AssetStoreResource is defined in this file and automatically exported

/// Trait for anything that can be stored in an `InMemoryStore`.
///
/// Provides name and tags for search functionality.
pub trait Searchable: Send + Sync + 'static {
    /// Human-readable name of this item.
    fn name(&self) -> &str;

    /// Tags for categorization and search (e.g., ["natural", "terrain"]).
    /// Default implementation returns empty slice.
    fn tags(&self) -> &[String] {
        &[]
    }
}

/// Storage interface for blob assets with metadata.
///
/// Unlike `InMemoryStore<T>` which stores typed Rust objects, `BlobStore` stores
/// raw bytes with associated metadata. This is used for database-backed storage
/// where assets are serialized (e.g., Lua source code).
///
/// Implementations: `DatabaseStore`, `InMemoryBlobStore`
pub trait BlobStore: Send + Sync {
    /// Get asset content by key.
    fn get(&self, key: &AssetKey) -> Result<Option<Vec<u8>>, AssetError>;

    /// Get asset metadata by key.
    fn get_metadata(&self, key: &AssetKey) -> Result<Option<AssetMetadata>, AssetError>;

    /// Get asset content and metadata together.
    fn get_full(&self, key: &AssetKey) -> Result<Option<(Vec<u8>, AssetMetadata)>, AssetError>;

    /// Store asset content and metadata. Creates or updates.
    fn set(
        &self,
        key: &AssetKey,
        content: &[u8],
        metadata: AssetMetadata,
    ) -> Result<(), AssetError>;

    /// Delete asset. Returns true if it existed.
    fn delete(&self, key: &AssetKey) -> Result<bool, AssetError>;

    /// List assets matching pattern within namespace.
    fn list(
        &self,
        namespace: &str,
        pattern: &str,
        asset_type: Option<&str>,
    ) -> Result<Vec<AssetRef>, AssetError>;

    /// Full-text search across all assets.
    fn search(&self, query: &str, asset_type: Option<&str>) -> Result<Vec<AssetRef>, AssetError>;

    /// Check if asset exists.
    fn exists(&self, key: &AssetKey) -> Result<bool, AssetError>;

    /// Count assets in namespace (optionally filtered by type).
    fn count(&self, namespace: &str, asset_type: Option<&str>) -> Result<usize, AssetError>;

    /// Semantic search using vector similarity.
    /// Returns assets sorted by similarity (highest first) with scores.
    /// Default implementation returns empty (not supported).
    fn search_semantic(
        &self,
        _query_embedding: &[f32],
        _limit: usize,
    ) -> Result<Vec<(AssetRef, f32)>, AssetError> {
        Ok(Vec::new())
    }

    /// List all namespaces that contain assets.
    /// Used by AssetBrowser to discover available namespaces.
    fn list_namespaces(&self) -> Result<Vec<String>, AssetError>;
}

use bevy::prelude::Resource;
use std::sync::Arc;

/// Bevy resource wrapping a `BlobStore` implementation.
///
/// Allows switching between `DatabaseStore` and `InMemoryBlobStore` at runtime
/// via configuration.
///
/// # Example
///
/// ```ignore
/// // Use database backend
/// let db = DatabaseStore::open(Path::new("assets.db"))?;
/// app.insert_resource(AssetStoreResource::new(db));
///
/// // Or use in-memory backend (no persistence)
/// let mem = InMemoryBlobStore::new();
/// app.insert_resource(AssetStoreResource::new(mem));
/// ```
#[derive(Resource, Clone)]
pub struct AssetStoreResource {
    inner: Arc<dyn BlobStore>,
}

impl AssetStoreResource {
    /// Create from any `BlobStore` implementation.
    pub fn new<T: BlobStore + 'static>(store: T) -> Self {
        Self {
            inner: Arc::new(store),
        }
    }

    /// Create from an `Arc` to a `BlobStore` implementation.
    /// Use this when you need to share the store with other services.
    pub fn from_arc<T: BlobStore + 'static>(store: Arc<T>) -> Self {
        Self { inner: store }
    }

    /// Get reference to the underlying store.
    pub fn store(&self) -> &dyn BlobStore {
        self.inner.as_ref()
    }
}

impl std::ops::Deref for AssetStoreResource {
    type Target = dyn BlobStore;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}
