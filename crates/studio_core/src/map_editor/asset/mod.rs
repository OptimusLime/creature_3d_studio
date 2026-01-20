//! Generic asset system for the map editor.
//!
//! # Architecture
//!
//! Two distinct storage patterns serve different needs:
//!
//! ## 1. Typed Runtime Storage (`AssetStore<T>`)
//!
//! For in-memory collections of typed Rust objects with search:
//! - `Asset` trait: Implemented by typed things (Material, LuaLayerDef)
//! - `AssetStore<T>` trait: Generic storage with get/list/set/search
//! - `InMemoryStore<T>`: Simple in-memory implementation
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
//! use studio_core::map_editor::asset::{Asset, AssetStore, InMemoryStore};
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
mod store;

pub use database::{AssetError, AssetKey, AssetMetadata, AssetRef, DatabaseStore};
pub use store::{InMemoryBlobStore, InMemoryStore};
// Note: AssetStoreResource is defined in this file and automatically exported

/// Trait for anything that can be stored in an AssetStore.
///
/// All storable things implement this trait, enabling unified search and management.
pub trait Asset: Send + Sync + 'static {
    /// Human-readable name of this asset.
    fn name(&self) -> &str;

    /// Type identifier for this asset category (e.g., "material", "generator").
    fn asset_type() -> &'static str
    where
        Self: Sized;

    /// Tags for categorization and search (e.g., ["natural", "terrain"]).
    /// Default implementation returns empty slice for assets without tags.
    fn tags(&self) -> &[String] {
        &[]
    }
}

/// Generic storage interface for assets.
///
/// Provides CRUD operations plus search. Implementations can be in-memory,
/// file-backed, or network-based.
pub trait AssetStore<T: Asset>: Send + Sync {
    /// Get an asset by its store index.
    fn get(&self, index: usize) -> Option<&T>;

    /// Get a mutable reference to an asset by its store index.
    fn get_mut(&mut self, index: usize) -> Option<&mut T>;

    /// List all assets in the store.
    fn list(&self) -> &[T];

    /// Add or update an asset, returning its store index.
    fn set(&mut self, asset: T) -> usize;

    /// Search assets by name (case-insensitive substring match).
    fn search(&self, query: &str) -> Vec<&T>;

    /// Find the first asset matching a predicate.
    fn find<F>(&self, predicate: F) -> Option<&T>
    where
        F: Fn(&T) -> bool,
    {
        self.list().iter().find(|a| predicate(a))
    }

    /// Find the first asset matching a predicate (mutable).
    fn find_mut<F>(&mut self, predicate: F) -> Option<&mut T>
    where
        F: Fn(&T) -> bool;

    /// Check if any asset matches a predicate.
    fn any<F>(&self, predicate: F) -> bool
    where
        F: Fn(&T) -> bool,
    {
        self.list().iter().any(|a| predicate(a))
    }

    /// Get the number of assets in the store.
    fn len(&self) -> usize {
        self.list().len()
    }

    /// Check if the store is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over all assets.
    fn iter(&self) -> std::slice::Iter<'_, T> {
        self.list().iter()
    }
}

/// Storage interface for blob assets with metadata.
///
/// Unlike `AssetStore<T>` which stores typed Rust objects, `BlobStore` stores
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
