//! In-memory storage implementations.
//!
//! Two storage types:
//! - `InMemoryStore<T>` - Typed storage for runtime Rust objects with search
//! - `InMemoryBlobStore` - Blob storage for serialized content (implements `BlobStore`)

use super::{AssetError, AssetKey, AssetMetadata, AssetRef, BlobStore, Searchable};
use bevy::prelude::Resource;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Simple in-memory store for searchable items.
///
/// Items are stored in a Vec. Search performs case-insensitive
/// substring matching on names and tags.
#[derive(Debug)]
pub struct InMemoryStore<T: Searchable> {
    assets: Vec<T>,
}

impl<T: Searchable> InMemoryStore<T> {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self { assets: Vec::new() }
    }

    /// Create a store with initial items.
    pub fn with_assets(assets: Vec<T>) -> Self {
        Self { assets }
    }

    /// Replace all items in the store.
    pub fn set_all(&mut self, assets: Vec<T>) {
        self.assets = assets;
    }

    /// Clear all items from the store.
    pub fn clear(&mut self) {
        self.assets.clear();
    }

    /// Get an item by index.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.assets.get(index)
    }

    /// Get a mutable item by index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.assets.get_mut(index)
    }

    /// Get all items as a slice.
    pub fn list(&self) -> &[T] {
        &self.assets
    }

    /// Add an item, returning its index.
    pub fn set(&mut self, asset: T) -> usize {
        let index = self.assets.len();
        self.assets.push(asset);
        index
    }

    /// Search items by name or tag (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&T> {
        let query_lower = query.to_lowercase();
        self.assets
            .iter()
            .filter(|asset| {
                // Match by name (substring)
                if asset.name().to_lowercase().contains(&query_lower) {
                    return true;
                }
                // Match by tag (exact match, case-insensitive)
                asset
                    .tags()
                    .iter()
                    .any(|tag| tag.to_lowercase() == query_lower)
            })
            .collect()
    }

    /// Find the first item matching a predicate.
    pub fn find<F>(&self, predicate: F) -> Option<&T>
    where
        F: Fn(&T) -> bool,
    {
        self.assets.iter().find(|a| predicate(a))
    }

    /// Find the first item matching a predicate (mutable).
    pub fn find_mut<F>(&mut self, predicate: F) -> Option<&mut T>
    where
        F: Fn(&T) -> bool,
    {
        self.assets.iter_mut().find(|a| predicate(a))
    }

    /// Check if any item matches a predicate.
    pub fn any<F>(&self, predicate: F) -> bool
    where
        F: Fn(&T) -> bool,
    {
        self.assets.iter().any(|a| predicate(a))
    }

    /// Get the number of items.
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Iterate over all items.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.assets.iter()
    }
}

impl<T: Searchable> Default for InMemoryStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// InMemoryBlobStore - BlobStore implementation for testing/no-persist mode
// =============================================================================

/// Stored asset entry for in-memory blob store.
#[derive(Clone)]
struct BlobEntry {
    content: Vec<u8>,
    metadata: AssetMetadata,
}

/// In-memory implementation of `BlobStore`.
///
/// Useful for:
/// - Testing without database dependencies
/// - Running without persistence (assets lost on restart)
/// - Config-based switching between in-memory and database backends
///
/// Thread-safe via internal `RwLock`.
#[derive(Resource, Default)]
pub struct InMemoryBlobStore {
    assets: Arc<RwLock<HashMap<String, BlobEntry>>>,
}

impl InMemoryBlobStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self {
            assets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get number of stored assets.
    pub fn len(&self) -> usize {
        self.assets.read().unwrap().len()
    }

    /// Check if store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all assets.
    pub fn clear(&self) {
        self.assets.write().unwrap().clear();
    }

    /// Convert AssetKey to internal string key.
    fn key_to_string(key: &AssetKey) -> String {
        format!("{}/{}", key.namespace, key.path)
    }
}

impl BlobStore for InMemoryBlobStore {
    fn get(&self, key: &AssetKey) -> Result<Option<Vec<u8>>, AssetError> {
        let assets = self.assets.read().unwrap();
        Ok(assets
            .get(&Self::key_to_string(key))
            .map(|e| e.content.clone()))
    }

    fn get_metadata(&self, key: &AssetKey) -> Result<Option<AssetMetadata>, AssetError> {
        let assets = self.assets.read().unwrap();
        Ok(assets
            .get(&Self::key_to_string(key))
            .map(|e| e.metadata.clone()))
    }

    fn get_full(&self, key: &AssetKey) -> Result<Option<(Vec<u8>, AssetMetadata)>, AssetError> {
        let assets = self.assets.read().unwrap();
        Ok(assets
            .get(&Self::key_to_string(key))
            .map(|e| (e.content.clone(), e.metadata.clone())))
    }

    fn set(
        &self,
        key: &AssetKey,
        content: &[u8],
        metadata: AssetMetadata,
    ) -> Result<(), AssetError> {
        let mut assets = self.assets.write().unwrap();
        let mut meta = metadata;
        meta.updated_at = Utc::now();
        assets.insert(
            Self::key_to_string(key),
            BlobEntry {
                content: content.to_vec(),
                metadata: meta,
            },
        );
        Ok(())
    }

    fn delete(&self, key: &AssetKey) -> Result<bool, AssetError> {
        let mut assets = self.assets.write().unwrap();
        Ok(assets.remove(&Self::key_to_string(key)).is_some())
    }

    fn list(
        &self,
        namespace: &str,
        pattern: &str,
        asset_type: Option<&str>,
    ) -> Result<Vec<AssetRef>, AssetError> {
        let assets = self.assets.read().unwrap();
        let prefix = format!("{}/", namespace);

        // Convert glob pattern to simple matching
        let pattern_lower = pattern.to_lowercase();
        let is_match_all = pattern == "%" || pattern == "*";

        let results: Vec<AssetRef> = assets
            .iter()
            .filter(|(k, entry)| {
                // Must be in namespace
                if !k.starts_with(&prefix) {
                    return false;
                }

                // Type filter
                if let Some(t) = asset_type {
                    if entry.metadata.asset_type != t {
                        return false;
                    }
                }

                // Pattern matching (simplified)
                if is_match_all {
                    return true;
                }

                let path = &k[prefix.len()..];
                path.to_lowercase()
                    .contains(&pattern_lower.replace('%', ""))
            })
            .map(|(k, entry)| {
                let key = AssetKey::parse(k).unwrap_or_else(|| AssetKey::new(namespace, "unknown"));
                AssetRef {
                    key,
                    metadata: entry.metadata.clone(),
                }
            })
            .collect();

        Ok(results)
    }

    fn search(&self, query: &str, asset_type: Option<&str>) -> Result<Vec<AssetRef>, AssetError> {
        let assets = self.assets.read().unwrap();
        let query_lower = query.to_lowercase();

        let results: Vec<AssetRef> = assets
            .iter()
            .filter(|(_, entry)| {
                // Type filter
                if let Some(t) = asset_type {
                    if entry.metadata.asset_type != t {
                        return false;
                    }
                }

                // Search in name
                if entry.metadata.name.to_lowercase().contains(&query_lower) {
                    return true;
                }

                // Search in description
                if let Some(ref desc) = entry.metadata.description {
                    if desc.to_lowercase().contains(&query_lower) {
                        return true;
                    }
                }

                // Search in tags
                entry
                    .tags()
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .map(|(k, entry)| {
                let key = AssetKey::parse(k).unwrap_or_else(|| AssetKey::new("unknown", "unknown"));
                AssetRef {
                    key,
                    metadata: entry.metadata.clone(),
                }
            })
            .collect();

        Ok(results)
    }

    fn exists(&self, key: &AssetKey) -> Result<bool, AssetError> {
        let assets = self.assets.read().unwrap();
        Ok(assets.contains_key(&Self::key_to_string(key)))
    }

    fn count(&self, namespace: &str, asset_type: Option<&str>) -> Result<usize, AssetError> {
        let assets = self.assets.read().unwrap();
        let prefix = format!("{}/", namespace);

        let count = assets
            .iter()
            .filter(|(k, entry)| {
                if !k.starts_with(&prefix) {
                    return false;
                }
                if let Some(t) = asset_type {
                    if entry.metadata.asset_type != t {
                        return false;
                    }
                }
                true
            })
            .count();

        Ok(count)
    }

    fn list_namespaces(&self) -> Result<Vec<String>, AssetError> {
        let assets = self.assets.read().unwrap();
        let mut namespaces: std::collections::HashSet<String> = std::collections::HashSet::new();

        for key in assets.keys() {
            // Key format is "namespace/path", extract namespace
            if let Some(slash_pos) = key.find('/') {
                namespaces.insert(key[..slash_pos].to_string());
            }
        }

        let mut result: Vec<String> = namespaces.into_iter().collect();
        result.sort();
        Ok(result)
    }
}

// Helper trait for accessing tags from metadata
trait MetadataTags {
    fn tags(&self) -> &[String];
}

impl MetadataTags for BlobEntry {
    fn tags(&self) -> &[String] {
        &self.metadata.tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test asset
    #[derive(Debug, Clone)]
    struct TestAsset {
        name: String,
        tags: Vec<String>,
    }

    impl TestAsset {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                tags: Vec::new(),
            }
        }

        fn with_tags(name: &str, tags: Vec<&str>) -> Self {
            Self {
                name: name.to_string(),
                tags: tags.into_iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    impl Searchable for TestAsset {
        fn name(&self) -> &str {
            &self.name
        }

        fn tags(&self) -> &[String] {
            &self.tags
        }
    }

    #[test]
    fn test_empty_store() {
        let store: InMemoryStore<TestAsset> = InMemoryStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert!(store.get(0).is_none());
    }

    #[test]
    fn test_add_and_get() {
        let mut store: InMemoryStore<TestAsset> = InMemoryStore::new();

        let id = store.set(TestAsset::new("stone"));
        assert_eq!(id, 0);
        assert_eq!(store.len(), 1);

        let asset = store.get(0).unwrap();
        assert_eq!(asset.name(), "stone");
    }

    #[test]
    fn test_list() {
        let mut store: InMemoryStore<TestAsset> = InMemoryStore::new();
        store.set(TestAsset::new("stone"));
        store.set(TestAsset::new("dirt"));

        let list = store.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name(), "stone");
        assert_eq!(list[1].name(), "dirt");
    }

    #[test]
    fn test_search_case_insensitive() {
        let mut store: InMemoryStore<TestAsset> = InMemoryStore::new();
        store.set(TestAsset::new("Stone Block"));
        store.set(TestAsset::new("Dirt"));
        store.set(TestAsset::new("Cobblestone"));

        // Search should be case-insensitive
        let results = store.search("stone");
        assert_eq!(results.len(), 2);

        // Should find partial matches
        let results = store.search("STONE");
        assert_eq!(results.len(), 2);

        // No matches
        let results = store.search("xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_by_tag() {
        let mut store: InMemoryStore<TestAsset> = InMemoryStore::new();
        store.set(TestAsset::with_tags("stone", vec!["natural", "terrain"]));
        store.set(TestAsset::with_tags("dirt", vec!["natural", "terrain"]));
        store.set(TestAsset::with_tags("metal_plate", vec!["industrial"]));
        store.set(TestAsset::new("glass")); // no tags

        // Search by tag "natural" should find stone and dirt
        let results = store.search("natural");
        assert_eq!(results.len(), 2);

        // Search by tag "industrial" should find metal_plate
        let results = store.search("industrial");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name(), "metal_plate");

        // Tag search is case-insensitive
        let results = store.search("NATURAL");
        assert_eq!(results.len(), 2);

        // Name search still works
        let results = store.search("glass");
        assert_eq!(results.len(), 1);

        // Partial name match still works
        let results = store.search("stone");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name(), "stone");
    }

    #[test]
    fn test_with_assets() {
        let store = InMemoryStore::with_assets(vec![TestAsset::new("a"), TestAsset::new("b")]);
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_set_all() {
        let mut store: InMemoryStore<TestAsset> = InMemoryStore::new();
        store.set(TestAsset::new("old"));

        store.set_all(vec![TestAsset::new("new1"), TestAsset::new("new2")]);

        assert_eq!(store.len(), 2);
        assert_eq!(store.list()[0].name(), "new1");
    }

    // =========================================================================
    // InMemoryBlobStore tests
    // =========================================================================

    #[test]
    fn test_inmemory_blob_store_basic() {
        let store = InMemoryBlobStore::new();
        assert!(store.is_empty());

        let key = AssetKey::new("test", "materials/stone");
        let content = b"return { name = 'Stone' }";
        let metadata = AssetMetadata::new("Stone", "material");

        // Set
        store.set(&key, content, metadata).unwrap();
        assert_eq!(store.len(), 1);

        // Get
        let retrieved = store.get(&key).unwrap().unwrap();
        assert_eq!(retrieved, content);

        // Get metadata
        let meta = store.get_metadata(&key).unwrap().unwrap();
        assert_eq!(meta.name, "Stone");

        // Exists
        assert!(store.exists(&key).unwrap());

        // Delete
        assert!(store.delete(&key).unwrap());
        assert!(!store.exists(&key).unwrap());
    }

    #[test]
    fn test_inmemory_blob_store_list() {
        let store = InMemoryBlobStore::new();

        // Add assets
        store
            .set(
                &AssetKey::new("ns", "materials/a"),
                b"a",
                AssetMetadata::new("A", "material"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("ns", "materials/b"),
                b"b",
                AssetMetadata::new("B", "material"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("ns", "generators/c"),
                b"c",
                AssetMetadata::new("C", "generator"),
            )
            .unwrap();

        // List all in namespace
        let all = store.list("ns", "%", None).unwrap();
        assert_eq!(all.len(), 3);

        // List by type
        let materials = store.list("ns", "%", Some("material")).unwrap();
        assert_eq!(materials.len(), 2);

        // Count
        assert_eq!(store.count("ns", None).unwrap(), 3);
        assert_eq!(store.count("ns", Some("generator")).unwrap(), 1);
    }

    #[test]
    fn test_inmemory_blob_store_search() {
        let store = InMemoryBlobStore::new();

        store
            .set(
                &AssetKey::new("user", "crystal"),
                b"lua",
                AssetMetadata::new("Crystal Material", "material")
                    .with_description("A glowing gem"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("user", "maze"),
                b"lua",
                AssetMetadata::new("Maze Gen", "generator"),
            )
            .unwrap();

        // Search by name
        let results = store.search("crystal", None).unwrap();
        assert_eq!(results.len(), 1);

        // Search by description
        let results = store.search("glowing", None).unwrap();
        assert_eq!(results.len(), 1);

        // Search with type filter
        let results = store.search("maze", Some("generator")).unwrap();
        assert_eq!(results.len(), 1);

        // No match
        let results = store.search("nonexistent", None).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_inmemory_blob_store_upsert() {
        let store = InMemoryBlobStore::new();
        let key = AssetKey::new("test", "item");

        // Create
        store
            .set(&key, b"v1", AssetMetadata::new("V1", "type"))
            .unwrap();
        assert_eq!(store.get(&key).unwrap().unwrap(), b"v1");

        // Upsert
        store
            .set(&key, b"v2", AssetMetadata::new("V2", "type"))
            .unwrap();
        assert_eq!(store.get(&key).unwrap().unwrap(), b"v2");

        let meta = store.get_metadata(&key).unwrap().unwrap();
        assert_eq!(meta.name, "V2");

        // Still only 1 asset
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_inmemory_blob_store_list_namespaces() {
        let store = InMemoryBlobStore::new();

        // Empty store has no namespaces
        assert!(store.list_namespaces().unwrap().is_empty());

        // Add assets in different namespaces
        store
            .set(
                &AssetKey::new("paul", "materials/stone"),
                b"lua",
                AssetMetadata::new("Stone", "material"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("shared", "materials/water"),
                b"lua",
                AssetMetadata::new("Water", "material"),
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("paul", "generators/maze"),
                b"lua",
                AssetMetadata::new("Maze", "generator"),
            )
            .unwrap();

        // Should have 2 namespaces, sorted
        let namespaces = store.list_namespaces().unwrap();
        assert_eq!(namespaces, vec!["paul", "shared"]);
    }
}
