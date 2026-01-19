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
//!
//! # MarkovJunior Integration
//!
//! Materials can optionally bind to MJ palette characters via `mj_char`:
//! - `mj_char = Some('B')` means this material represents MJ's 'B' (Black)
//! - When an MJ model uses character 'B', it resolves to this material's ID
//! - If no material binds to a character, auto-create from MJ palette.xml colors

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
    /// Optional MarkovJunior palette character binding.
    /// When set, this material is used when MJ models output this character.
    /// E.g., `mj_char = Some('B')` binds to MJ's Black character.
    pub mj_char: Option<char>,
}

impl Material {
    /// Create a new material with the given properties (no tags, no MJ binding).
    pub fn new(id: u32, name: impl Into<String>, color: [f32; 3]) -> Self {
        Self {
            id,
            name: name.into(),
            color,
            tags: Vec::new(),
            mj_char: None,
        }
    }

    /// Create a new material with tags (no MJ binding).
    pub fn with_tags(id: u32, name: impl Into<String>, color: [f32; 3], tags: Vec<String>) -> Self {
        Self {
            id,
            name: name.into(),
            color,
            tags,
            mj_char: None,
        }
    }

    /// Create a new material with MJ character binding.
    pub fn with_mj_char(
        id: u32,
        name: impl Into<String>,
        color: [f32; 3],
        tags: Vec<String>,
        mj_char: char,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            color,
            tags,
            mj_char: Some(mj_char),
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
    /// Cache of MJ character to material ID mappings.
    /// Populated when materials with mj_char are loaded.
    mj_char_map: std::collections::HashMap<char, u32>,
}

impl MaterialPalette {
    /// Create a new palette with the given available materials.
    /// Initializes active palette with first 2 materials if available.
    /// Builds MJ character map from materials with mj_char bindings.
    pub fn new(materials: Vec<Material>) -> Self {
        let active: Vec<u32> = materials.iter().take(2).map(|m| m.id).collect();
        let mj_char_map = Self::build_mj_char_map(&materials);
        Self {
            available: InMemoryStore::with_assets(materials),
            active,
            changed: true,
            mj_char_map,
        }
    }

    /// Build the MJ character to material ID mapping.
    fn build_mj_char_map(materials: &[Material]) -> std::collections::HashMap<char, u32> {
        let mut map = std::collections::HashMap::new();
        for mat in materials {
            if let Some(ch) = mat.mj_char {
                map.insert(ch, mat.id);
            }
        }
        map
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
    /// Rebuilds MJ character mappings.
    pub fn set_available(&mut self, materials: Vec<Material>) {
        // Filter active to only include IDs that exist in new materials
        let valid_ids: std::collections::HashSet<u32> = materials.iter().map(|m| m.id).collect();
        self.active.retain(|id| valid_ids.contains(id));

        // If active is empty, initialize with first 2
        if self.active.is_empty() {
            self.active = materials.iter().take(2).map(|m| m.id).collect();
        }

        // Rebuild MJ char map
        self.mj_char_map = Self::build_mj_char_map(&materials);

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
        // Update MJ char map if this material has a binding
        if let Some(ch) = material.mj_char {
            self.mj_char_map.insert(ch, material.id);
        }
        self.available.set(material);
        self.changed = true;
    }

    // =========================================================================
    // MarkovJunior Integration
    // =========================================================================

    /// Get the material ID for an MJ palette character.
    /// Returns None if no material is bound to this character.
    pub fn get_material_for_mj_char(&self, ch: char) -> Option<u32> {
        self.mj_char_map.get(&ch).copied()
    }

    /// Get the material for an MJ palette character.
    pub fn get_material_by_mj_char(&self, ch: char) -> Option<&Material> {
        self.mj_char_map.get(&ch).and_then(|&id| self.get_by_id(id))
    }

    /// Resolve MJ grid characters to material IDs.
    /// For each character in the grid's character set, returns the material ID.
    /// Characters without bindings get auto-generated materials using MJ palette colors.
    ///
    /// Returns a Vec where index i corresponds to MJ grid value i.
    /// Index 0 is always material ID 0 (empty/transparent in MJ convention).
    pub fn resolve_mj_characters(&mut self, characters: &[char]) -> Vec<u32> {
        let mut result = Vec::with_capacity(characters.len());

        for (i, &ch) in characters.iter().enumerate() {
            if i == 0 {
                // MJ convention: value 0 is always empty/transparent
                // Map to material ID 0 (which means "no material" in our system)
                result.push(0);
            } else if let Some(mat_id) = self.get_material_for_mj_char(ch) {
                // Material already bound to this character
                result.push(mat_id);
            } else {
                // No binding - auto-create material from MJ palette
                let mat_id = self.auto_create_mj_material(ch);
                result.push(mat_id);
            }
        }

        result
    }

    /// Auto-create a material for an MJ character using palette.xml colors.
    /// Returns the new material's ID.
    fn auto_create_mj_material(&mut self, ch: char) -> u32 {
        // Get color from MJ palette (or magenta fallback)
        let color = Self::mj_palette_color(ch);

        // Find next available ID
        let max_id = self
            .available
            .list()
            .iter()
            .map(|m| m.id)
            .max()
            .unwrap_or(0);
        let new_id = max_id + 1;

        // Create and add the material
        let mat = Material {
            id: new_id,
            name: format!("mj_{}", ch),
            color,
            tags: vec!["mj".to_string(), "auto".to_string()],
            mj_char: Some(ch),
        };

        self.mj_char_map.insert(ch, new_id);
        self.available.set(mat);

        new_id
    }

    /// Get the MJ palette.xml color for a character as RGB floats.
    /// Returns magenta for unknown characters.
    pub fn mj_palette_color(ch: char) -> [f32; 3] {
        // Colors from MarkovJunior palette.xml (converted to 0-1 range)
        match ch {
            // Primary colors (uppercase)
            'B' => [0.0, 0.0, 0.0],       // Black
            'I' => [0.114, 0.169, 0.325], // Indigo
            'P' => [0.494, 0.145, 0.325], // Purple
            'E' => [0.0, 0.529, 0.318],   // Emerald
            'N' => [0.671, 0.322, 0.212], // browN
            'D' => [0.373, 0.341, 0.310], // Dead/Dark
            'A' => [0.761, 0.765, 0.780], // Alive/grAy
            'W' => [1.0, 0.945, 0.910],   // White
            'R' => [1.0, 0.0, 0.302],     // Red
            'O' => [1.0, 0.639, 0.0],     // Orange
            'Y' => [1.0, 0.925, 0.153],   // Yellow
            'G' => [0.0, 0.894, 0.212],   // Green
            'U' => [0.161, 0.678, 1.0],   // blUe
            'S' => [0.514, 0.463, 0.612], // Slate
            'K' => [1.0, 0.467, 0.659],   // pinK
            'F' => [1.0, 0.800, 0.667],   // Fawn

            // Secondary colors (lowercase) - darker variants
            'b' => [0.161, 0.094, 0.078], // black
            'i' => [0.067, 0.114, 0.208], // indigo
            'p' => [0.259, 0.129, 0.212], // purple
            'e' => [0.071, 0.325, 0.349], // emerald
            'n' => [0.455, 0.184, 0.161], // brown
            'd' => [0.286, 0.200, 0.231], // dead/dark
            'a' => [0.635, 0.533, 0.475], // alive/gray
            'w' => [0.953, 0.937, 0.490], // white
            'r' => [0.745, 0.071, 0.314], // red
            'o' => [1.0, 0.424, 0.141],   // orange
            'y' => [0.659, 0.906, 0.180], // yellow
            'g' => [0.0, 0.710, 0.263],   // green
            'u' => [0.024, 0.353, 0.710], // blue
            's' => [0.459, 0.275, 0.396], // slate
            'k' => [1.0, 0.431, 0.349],   // pink
            'f' => [1.0, 0.616, 0.506],   // fawn

            // Additional colors
            'C' => [0.0, 1.0, 1.0],       // Cyan
            'c' => [0.373, 0.804, 0.894], // cyan
            'Z' => [1.0, 1.0, 1.0],       // Z (pure white)
            'M' => [1.0, 0.0, 1.0],       // Magenta

            // Unknown - magenta fallback
            _ => [1.0, 0.0, 1.0],
        }
    }

    /// Check if all required MJ characters have material bindings.
    pub fn has_mj_bindings_for(&self, characters: &[char]) -> bool {
        characters
            .iter()
            .skip(1)
            .all(|&ch| self.mj_char_map.contains_key(&ch))
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
