//! Generic asset system for the map editor.
//!
//! Provides a unified interface for all storable things: materials, generators,
//! renderers, visualizers, etc.
//!
//! # Architecture
//!
//! Two storage patterns:
//!
//! ## In-Memory Typed Storage (Phase 1-3)
//! - `Asset` trait: Implemented by typed things (Material, etc.)
//! - `AssetStore<T>` trait: Generic storage with get/list/set/search
//! - `InMemoryStore<T>`: Simple in-memory implementation
//!
//! ## Database Blob Storage (Phase 4+)
//! - `BlobStore` trait: Storage for raw bytes with metadata
//! - `DatabaseStore`: SQLite-backed implementation with FTS5 search
//! - `AssetKey`: Namespace/path key (e.g., "paul/materials/crystal")
//! - `AssetMetadata`: Name, description, tags, asset_type
//!
//! # Example (In-Memory)
//!
//! ```ignore
//! use studio_core::map_editor::asset::{Asset, AssetStore, InMemoryStore};
//!
//! // Material already implements Asset
//! let mut store: InMemoryStore<Material> = InMemoryStore::new();
//! store.set(material);
//! let results = store.search("stone");
//! ```
//!
//! # Example (Database)
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

mod database;
mod store;

pub use database::{AssetError, AssetKey, AssetMetadata, AssetRef, DatabaseStore};
pub use store::InMemoryStore;

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
/// Implementations: `DatabaseStore`
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
