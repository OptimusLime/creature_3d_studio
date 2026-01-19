//! Material definitions for the map editor.
//!
//! Materials define visual properties (color) and are referenced by ID in the voxel buffer.
//!
//! # Architecture
//!
//! - `MaterialPalette.available` - `InMemoryStore<Material>` storing all materials from Lua
//! - `MaterialPalette.active` - Material IDs selected for the active generation palette
//!
//! The generator receives the active palette and can use materials by index.
//!
//! `Material` implements the `Asset` trait, enabling unified search and storage via `AssetStore`.

use bevy::prelude::*;

use super::asset::{Asset, AssetStore, InMemoryStore};

/// A voxel material with an ID, name, color, and tags.
#[derive(Clone, Debug)]
pub struct Material {
    /// Unique identifier for this material.
    pub id: u32,
    /// Human-readable name.
    pub name: String,
    /// RGB color, each component in 0.0-1.0 range.
    pub color: [f32; 3],
    /// Tags for categorization and search (e.g., ["natural", "terrain"]).
    pub tags: Vec<String>,
}

impl Material {
    /// Create a new material with the given properties (no tags).
    pub fn new(id: u32, name: impl Into<String>, color: [f32; 3]) -> Self {
        Self {
            id,
            name: name.into(),
            color,
            tags: Vec::new(),
        }
    }

    /// Create a new material with tags.
    pub fn with_tags(id: u32, name: impl Into<String>, color: [f32; 3], tags: Vec<String>) -> Self {
        Self {
            id,
            name: name.into(),
            color,
            tags,
        }
    }
}

impl Asset for Material {
    fn name(&self) -> &str {
        &self.name
    }

    fn asset_type() -> &'static str {
        "material"
    }

    fn tags(&self) -> &[String] {
        &self.tags
    }
}

/// Collection of available and active materials for terrain generation.
#[derive(Resource)]
pub struct MaterialPalette {
    /// All available materials (loaded from Lua), stored via `AssetStore`.
    pub available: InMemoryStore<Material>,
    /// Active palette - material IDs in order, passed to generator.
    /// Generator uses palette[0], palette[1], etc.
    pub active: Vec<u32>,
    /// Flag indicating the active palette changed (needs regeneration).
    pub changed: bool,
}

impl MaterialPalette {
    /// Create a new palette with the given available materials.
    /// Initializes active palette with first 2 materials if available.
    pub fn new(materials: Vec<Material>) -> Self {
        let active: Vec<u32> = materials.iter().take(2).map(|m| m.id).collect();
        Self {
            available: InMemoryStore::with_assets(materials),
            active,
            changed: true,
        }
    }

    /// Create the default palette with stone and dirt.
    pub fn default_palette() -> Self {
        Self::new(vec![
            Material::new(1, "stone", [0.5, 0.5, 0.5]),
            Material::new(2, "dirt", [0.6, 0.4, 0.2]),
        ])
    }

    /// Get a material by its ID from available materials.
    pub fn get_by_id(&self, id: u32) -> Option<&Material> {
        self.available.find(|m| m.id == id)
    }

    /// Get a mutable material by its ID from available materials.
    pub fn get_by_id_mut(&mut self, id: u32) -> Option<&mut Material> {
        self.available.find_mut(|m| m.id == id)
    }

    /// Check if a material ID is in the active palette.
    pub fn is_active(&self, id: u32) -> bool {
        self.active.contains(&id)
    }

    /// Add a material to the active palette (if not already present).
    pub fn add_to_active(&mut self, id: u32) {
        if !self.active.contains(&id) && self.get_by_id(id).is_some() {
            self.active.push(id);
            self.changed = true;
        }
    }

    /// Remove a material from the active palette.
    pub fn remove_from_active(&mut self, id: u32) {
        if let Some(pos) = self.active.iter().position(|&x| x == id) {
            self.active.remove(pos);
            self.changed = true;
        }
    }

    /// Get the active palette as material references.
    pub fn active_materials(&self) -> Vec<&Material> {
        self.active
            .iter()
            .filter_map(|&id| self.get_by_id(id))
            .collect()
    }

    /// Clear the changed flag.
    pub fn clear_changed(&mut self) {
        self.changed = false;
    }

    /// Update available materials (from Lua reload).
    /// Preserves active palette entries that still exist.
    pub fn set_available(&mut self, materials: Vec<Material>) {
        // Filter active to only include IDs that exist in new materials
        let valid_ids: std::collections::HashSet<u32> = materials.iter().map(|m| m.id).collect();
        self.active.retain(|id| valid_ids.contains(id));

        // If active is empty, initialize with first 2
        if self.active.is_empty() {
            self.active = materials.iter().take(2).map(|m| m.id).collect();
        }

        self.available.set_all(materials);
        self.changed = true;
    }

    /// Search materials by name using the `AssetStore::search` method.
    pub fn search(&self, query: &str) -> Vec<&Material> {
        self.available.search(query)
    }

    /// Check if a material with the given ID exists.
    pub fn has_material(&self, id: u32) -> bool {
        self.available.any(|m| m.id == id)
    }

    /// Add a new material to the store.
    pub fn add_material(&mut self, material: Material) {
        self.available.set(material);
        self.changed = true;
    }
}

impl Default for MaterialPalette {
    fn default() -> Self {
        Self::default_palette()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_palette() {
        let palette = MaterialPalette::default_palette();
        assert_eq!(palette.available.len(), 2);
        // Use list() to access materials by index
        let materials = palette.available.list();
        assert_eq!(materials[0].name, "stone");
        assert_eq!(materials[1].name, "dirt");
        // Active should have first 2
        assert_eq!(palette.active, vec![1, 2]);
    }

    #[test]
    fn test_get_by_id() {
        let palette = MaterialPalette::default_palette();
        let stone = palette.get_by_id(1).unwrap();
        assert_eq!(stone.name, "stone");
        assert!(palette.get_by_id(99).is_none());
    }

    #[test]
    fn test_active_palette() {
        let mut palette = MaterialPalette::new(vec![
            Material::new(1, "stone", [0.5, 0.5, 0.5]),
            Material::new(2, "dirt", [0.6, 0.4, 0.2]),
            Material::new(3, "coal", [0.2, 0.2, 0.2]),
        ]);

        // Initial active is first 2
        assert_eq!(palette.active, vec![1, 2]);
        assert!(palette.is_active(1));
        assert!(palette.is_active(2));
        assert!(!palette.is_active(3));

        // Add coal
        palette.add_to_active(3);
        assert_eq!(palette.active, vec![1, 2, 3]);
        assert!(palette.is_active(3));

        // Remove dirt
        palette.remove_from_active(2);
        assert_eq!(palette.active, vec![1, 3]);
        assert!(!palette.is_active(2));
    }

    #[test]
    fn test_set_available_preserves_active() {
        let mut palette = MaterialPalette::new(vec![
            Material::new(1, "stone", [0.5, 0.5, 0.5]),
            Material::new(2, "dirt", [0.6, 0.4, 0.2]),
        ]);

        // Add new material set that includes stone but not dirt
        palette.set_available(vec![
            Material::new(1, "stone", [0.5, 0.5, 0.5]),
            Material::new(3, "coal", [0.2, 0.2, 0.2]),
        ]);

        // Active should only have stone now (dirt was removed)
        assert_eq!(palette.active, vec![1]);
    }

    #[test]
    fn test_search() {
        let palette = MaterialPalette::new(vec![
            Material::new(1, "stone", [0.5, 0.5, 0.5]),
            Material::new(2, "dirt", [0.6, 0.4, 0.2]),
            Material::new(3, "cobblestone", [0.4, 0.4, 0.4]),
        ]);

        // Search for "stone" should find both stone and cobblestone
        let results = palette.search("stone");
        assert_eq!(results.len(), 2);

        // Search for "dirt" should find only dirt
        let results = palette.search("dirt");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "dirt");

        // Search for "xyz" should find nothing
        let results = palette.search("xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_has_material() {
        let palette = MaterialPalette::default_palette();
        assert!(palette.has_material(1));
        assert!(palette.has_material(2));
        assert!(!palette.has_material(99));
    }
}
