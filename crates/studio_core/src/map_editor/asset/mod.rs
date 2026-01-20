//! Generic asset system for the map editor.
//!
//! Provides a unified interface for all storable things: materials, generators,
//! renderers, visualizers, etc.
//!
//! # Architecture
//!
//! - `Asset` trait: Implemented by anything that can be stored and searched (in-memory)
//! - `AssetStore<T>` trait: Generic storage with get/list/set/search operations (in-memory)
//! - `InMemoryStore<T>`: Simple in-memory implementation of AssetStore
//! - `DatabaseStore`: SQLite-backed storage with FTS5 search (Phase 4)
//! - `AssetKey`: Namespace/path key for database-backed assets
//! - `AssetMetadata`: Metadata for database-backed assets (name, description, tags)
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
//! use studio_core::map_editor::asset::{DatabaseStore, AssetKey, AssetMetadata};
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
