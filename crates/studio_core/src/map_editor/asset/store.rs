//! In-memory implementation of AssetStore.

use super::{Asset, AssetStore};

/// Simple in-memory asset store.
///
/// Assets are stored in a Vec and accessed by index. Search performs
/// case-insensitive substring matching on asset names.
#[derive(Debug)]
pub struct InMemoryStore<T: Asset> {
    assets: Vec<T>,
}

impl<T: Asset> InMemoryStore<T> {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self { assets: Vec::new() }
    }

    /// Create a store with initial assets.
    pub fn with_assets(assets: Vec<T>) -> Self {
        Self { assets }
    }

    /// Replace all assets in the store.
    pub fn set_all(&mut self, assets: Vec<T>) {
        self.assets = assets;
    }

    /// Clear all assets from the store.
    pub fn clear(&mut self) {
        self.assets.clear();
    }
}

impl<T: Asset> Default for InMemoryStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Asset> AssetStore<T> for InMemoryStore<T> {
    fn get(&self, index: usize) -> Option<&T> {
        self.assets.get(index)
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.assets.get_mut(index)
    }

    fn list(&self) -> &[T] {
        &self.assets
    }

    fn set(&mut self, asset: T) -> usize {
        let index = self.assets.len();
        self.assets.push(asset);
        index
    }

    fn search(&self, query: &str) -> Vec<&T> {
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

    fn find_mut<F>(&mut self, predicate: F) -> Option<&mut T>
    where
        F: Fn(&T) -> bool,
    {
        self.assets.iter_mut().find(|a| predicate(a))
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

    impl Asset for TestAsset {
        fn name(&self) -> &str {
            &self.name
        }

        fn asset_type() -> &'static str {
            "test"
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
}
