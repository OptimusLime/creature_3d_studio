//! Material definitions for the map editor.
//!
//! Materials define visual properties (color) and are referenced by ID in the voxel buffer.
//!
//! # Architecture
//!
//! - `MaterialPalette.available` - All materials loaded from Lua (read-only source)
//! - `MaterialPalette.active` - Materials added to the active palette for generation
//!
//! The generator receives the active palette and can use materials by index.

use bevy::prelude::*;

/// A voxel material with an ID, name, and color.
#[derive(Clone, Debug)]
pub struct Material {
    /// Unique identifier for this material.
    pub id: u32,
    /// Human-readable name.
    pub name: String,
    /// RGB color, each component in 0.0-1.0 range.
    pub color: [f32; 3],
}

impl Material {
    /// Create a new material with the given properties.
    pub fn new(id: u32, name: impl Into<String>, color: [f32; 3]) -> Self {
        Self {
            id,
            name: name.into(),
            color,
        }
    }
}

/// Collection of available and active materials for terrain generation.
#[derive(Resource)]
pub struct MaterialPalette {
    /// All available materials (loaded from Lua).
    pub available: Vec<Material>,
    /// Active palette - material IDs in order, passed to generator.
    /// Generator uses palette[0], palette[1], etc.
    pub active: Vec<u32>,
    /// Flag indicating the active palette changed (needs regeneration).
    pub changed: bool,
}

impl MaterialPalette {
    /// Create a new palette with the given available materials.
    /// Initializes active palette with first 2 materials if available.
    pub fn new(available: Vec<Material>) -> Self {
        let active: Vec<u32> = available.iter().take(2).map(|m| m.id).collect();
        Self {
            available,
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
        self.available.iter().find(|m| m.id == id)
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
    pub fn set_available(&mut self, available: Vec<Material>) {
        // Filter active to only include IDs that exist in new available
        let valid_ids: std::collections::HashSet<u32> = available.iter().map(|m| m.id).collect();
        self.active.retain(|id| valid_ids.contains(id));

        // If active is empty, initialize with first 2
        if self.active.is_empty() {
            self.active = available.iter().take(2).map(|m| m.id).collect();
        }

        self.available = available;
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
        assert_eq!(palette.available[0].name, "stone");
        assert_eq!(palette.available[1].name, "dirt");
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
}
