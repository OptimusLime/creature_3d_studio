//! Asset Browser UI Component
//!
//! A unified browser panel for all asset types (materials, generators, renderers, visualizers).
//! Provides tree navigation, search, type filtering, and quick actions.
//!
//! # UI Layout
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │ Asset Browser                                           [x] [_] │
//! ├─────────────────────────────────────────────────────────────────┤
//! │ [Search: _______________] [Type: All ▼]                         │
//! ├────────────────────────────┬────────────────────────────────────┤
//! │ ▼ paul/                    │  Crystal                           │
//! │   ▼ materials/             │  ────────────────────────────────  │
//! │     ● crystal              │  Type: material                    │
//! │     ○ stone                │  Description: Glowing blue         │
//! │   ▼ generators/            │  gemstone for cave environments    │
//! │     ○ maze_growth          │                                    │
//! │ ▼ shared/                  │  Tags: gem, glow, blue, cave       │
//! │   ▼ materials/             │                                    │
//! │     ○ water                │  [Load] [Edit] [Delete]            │
//! └────────────────────────────┴────────────────────────────────────┘
//! ```

use crate::map_editor::asset::{AssetKey, AssetMetadata, AssetRef, BlobStore};
use imgui::Ui;
use std::collections::{HashMap, HashSet};

/// Actions that can be triggered from the browser.
#[derive(Clone, Debug, PartialEq)]
pub enum BrowserAction {
    /// Load asset into editor.
    Load(AssetKey),
    /// Open asset for editing.
    Edit(AssetKey),
    /// Delete asset.
    Delete(AssetKey),
}

/// A node in the asset tree (either folder or asset).
#[derive(Clone, Debug)]
pub enum AssetTreeNode {
    /// A folder containing children.
    Folder {
        /// Display name of the folder.
        name: String,
        /// Full path to this folder (for tracking expanded state).
        path: String,
        /// Child nodes (folders and assets).
        children: Vec<AssetTreeNode>,
    },
    /// A leaf asset.
    Asset {
        /// Asset key for lookup.
        key: AssetKey,
        /// Asset metadata.
        metadata: AssetMetadata,
    },
}

impl AssetTreeNode {
    /// Get the display name of this node.
    pub fn name(&self) -> &str {
        match self {
            AssetTreeNode::Folder { name, .. } => name,
            AssetTreeNode::Asset { metadata, .. } => &metadata.name,
        }
    }

    /// Get the path of this node (for folders) or key string (for assets).
    pub fn path(&self) -> String {
        match self {
            AssetTreeNode::Folder { path, .. } => path.clone(),
            AssetTreeNode::Asset { key, .. } => key.to_key_string(),
        }
    }

    /// Check if this is a folder.
    pub fn is_folder(&self) -> bool {
        matches!(self, AssetTreeNode::Folder { .. })
    }
}

/// Hierarchical tree structure built from flat asset list.
#[derive(Clone, Debug, Default)]
pub struct AssetTree {
    /// Root nodes (typically namespaces).
    pub roots: Vec<AssetTreeNode>,
}

impl AssetTree {
    /// Build a tree from a list of assets.
    ///
    /// Assets are grouped by namespace, then by path segments.
    /// Example: `paul/materials/crystal` becomes:
    /// ```text
    /// paul/
    ///   materials/
    ///     crystal
    /// ```
    pub fn from_assets(assets: Vec<AssetRef>) -> Self {
        let mut tree = Self { roots: Vec::new() };

        // Group by namespace first
        let mut by_namespace: HashMap<String, Vec<AssetRef>> = HashMap::new();
        for asset in assets {
            by_namespace
                .entry(asset.key.namespace.clone())
                .or_default()
                .push(asset);
        }

        // Sort namespaces for consistent ordering
        let mut namespaces: Vec<_> = by_namespace.keys().cloned().collect();
        namespaces.sort();

        // Build tree for each namespace
        for namespace in namespaces {
            let ns_assets = by_namespace.remove(&namespace).unwrap();
            let ns_node = Self::build_namespace_tree(&namespace, ns_assets);
            tree.roots.push(ns_node);
        }

        tree
    }

    /// Build a tree for a single namespace.
    fn build_namespace_tree(namespace: &str, assets: Vec<AssetRef>) -> AssetTreeNode {
        // Intermediate structure for building the tree
        struct FolderBuilder {
            children_folders: HashMap<String, FolderBuilder>,
            assets: Vec<AssetRef>,
        }

        impl FolderBuilder {
            fn new() -> Self {
                Self {
                    children_folders: HashMap::new(),
                    assets: Vec::new(),
                }
            }

            fn insert(&mut self, path_parts: Vec<String>, asset: AssetRef) {
                if path_parts.is_empty() || path_parts.len() == 1 {
                    // This is the asset name, store it here
                    self.assets.push(asset);
                } else {
                    // Navigate into subfolder
                    let folder_name = path_parts[0].clone();
                    let remaining: Vec<String> = path_parts.into_iter().skip(1).collect();
                    let child = self
                        .children_folders
                        .entry(folder_name)
                        .or_insert_with(FolderBuilder::new);
                    child.insert(remaining, asset);
                }
            }

            fn to_nodes(mut self, parent_path: &str) -> Vec<AssetTreeNode> {
                let mut nodes = Vec::new();

                // Add folders first (sorted)
                let mut folder_names: Vec<_> = self.children_folders.keys().cloned().collect();
                folder_names.sort();

                for folder_name in folder_names {
                    // Remove the builder from the map to take ownership
                    let builder = self.children_folders.remove(&folder_name).unwrap();

                    let folder_path = if parent_path.is_empty() {
                        folder_name.clone()
                    } else {
                        format!("{}/{}", parent_path, folder_name)
                    };

                    let children = builder.to_nodes(&folder_path);

                    nodes.push(AssetTreeNode::Folder {
                        name: folder_name,
                        path: folder_path,
                        children,
                    });
                }

                // Add assets (sorted by name)
                self.assets
                    .sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));

                for asset in self.assets {
                    nodes.push(AssetTreeNode::Asset {
                        key: asset.key,
                        metadata: asset.metadata,
                    });
                }

                nodes
            }
        }

        // Build the folder structure
        let mut root_builder = FolderBuilder::new();

        for asset in assets {
            let path_parts: Vec<String> =
                asset.key.path.split('/').map(|s| s.to_string()).collect();
            root_builder.insert(path_parts, asset);
        }

        // Convert to nodes
        let children = root_builder.to_nodes(&format!("{}/", namespace));

        AssetTreeNode::Folder {
            name: format!("{}/", namespace),
            path: namespace.to_string(),
            children,
        }
    }

    /// Check if the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// Count total assets in the tree.
    pub fn asset_count(&self) -> usize {
        fn count_node(node: &AssetTreeNode) -> usize {
            match node {
                AssetTreeNode::Folder { children, .. } => children.iter().map(count_node).sum(),
                AssetTreeNode::Asset { .. } => 1,
            }
        }
        self.roots.iter().map(count_node).sum()
    }

    /// Filter tree to only include assets matching the query and type.
    pub fn filter(&self, query: &str, type_filter: Option<&str>) -> Self {
        let query_lower = query.to_lowercase();

        fn filter_node(
            node: &AssetTreeNode,
            query: &str,
            type_filter: Option<&str>,
        ) -> Option<AssetTreeNode> {
            match node {
                AssetTreeNode::Folder {
                    name,
                    path,
                    children,
                } => {
                    let filtered_children: Vec<_> = children
                        .iter()
                        .filter_map(|c| filter_node(c, query, type_filter))
                        .collect();

                    if filtered_children.is_empty() {
                        None
                    } else {
                        Some(AssetTreeNode::Folder {
                            name: name.clone(),
                            path: path.clone(),
                            children: filtered_children,
                        })
                    }
                }
                AssetTreeNode::Asset { key, metadata } => {
                    // Type filter
                    if let Some(t) = type_filter {
                        if metadata.asset_type != t {
                            return None;
                        }
                    }

                    // Query filter (search name, description, tags)
                    if !query.is_empty() {
                        let matches_name = metadata.name.to_lowercase().contains(query);
                        let matches_desc = metadata
                            .description
                            .as_ref()
                            .map(|d| d.to_lowercase().contains(query))
                            .unwrap_or(false);
                        let matches_tags = metadata
                            .tags
                            .iter()
                            .any(|t| t.to_lowercase().contains(query));
                        let matches_path = key.path.to_lowercase().contains(query);

                        if !matches_name && !matches_desc && !matches_tags && !matches_path {
                            return None;
                        }
                    }

                    Some(AssetTreeNode::Asset {
                        key: key.clone(),
                        metadata: metadata.clone(),
                    })
                }
            }
        }

        Self {
            roots: self
                .roots
                .iter()
                .filter_map(|r| filter_node(r, &query_lower, type_filter))
                .collect(),
        }
    }
}

/// Available type filter options.
pub const TYPE_FILTER_OPTIONS: &[(&str, Option<&str>)] = &[
    ("All", None),
    ("Materials", Some("material")),
    ("Generators", Some("generator")),
    ("Renderers", Some("renderer")),
    ("Visualizers", Some("visualizer")),
];

/// Asset browser UI state.
pub struct AssetBrowser {
    /// Currently selected asset.
    pub selected_asset: Option<AssetKey>,
    /// Search query string.
    pub search_query: String,
    /// Type filter index (into TYPE_FILTER_OPTIONS).
    pub type_filter_idx: usize,
    /// Set of expanded folder paths.
    pub expanded: HashSet<String>,
    /// Cached tree structure.
    tree: AssetTree,
    /// Filtered tree (after applying search/type filter).
    filtered_tree: AssetTree,
    /// Whether the tree needs refresh from store.
    needs_refresh: bool,
}

impl Default for AssetBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetBrowser {
    /// Create a new empty browser.
    pub fn new() -> Self {
        Self {
            selected_asset: None,
            search_query: String::new(),
            type_filter_idx: 0,
            expanded: HashSet::new(),
            tree: AssetTree::default(),
            filtered_tree: AssetTree::default(),
            needs_refresh: true,
        }
    }

    /// Mark the browser as needing a refresh from the store.
    pub fn mark_dirty(&mut self) {
        self.needs_refresh = true;
    }

    /// Refresh the tree from the store if needed.
    pub fn refresh_if_needed(&mut self, store: &dyn BlobStore) {
        if !self.needs_refresh {
            return;
        }

        // Get all namespaces and list all assets
        let mut all_assets = Vec::new();

        // List from all known namespaces via listing with wildcard
        // Since we can't get namespaces directly from trait, we'll use a workaround:
        // Try to list from common namespaces, or the caller should provide namespaces
        if let Ok(namespaces) = store.list_namespaces() {
            for ns in namespaces {
                if let Ok(assets) = store.list(&ns, "%", None) {
                    all_assets.extend(assets);
                }
            }
        }

        self.tree = AssetTree::from_assets(all_assets);
        self.apply_filter();
        self.needs_refresh = false;
    }

    /// Force refresh from the store.
    pub fn refresh(&mut self, store: &dyn BlobStore) {
        self.needs_refresh = true;
        self.refresh_if_needed(store);
    }

    /// Apply current search/type filter to the tree.
    fn apply_filter(&mut self) {
        let type_filter = TYPE_FILTER_OPTIONS
            .get(self.type_filter_idx)
            .and_then(|(_, t)| *t);

        self.filtered_tree = self.tree.filter(&self.search_query, type_filter);
    }

    /// Update search query and refilter.
    pub fn set_search(&mut self, query: String) {
        if self.search_query != query {
            self.search_query = query;
            self.apply_filter();
        }
    }

    /// Update type filter and refilter.
    pub fn set_type_filter(&mut self, idx: usize) {
        if self.type_filter_idx != idx && idx < TYPE_FILTER_OPTIONS.len() {
            self.type_filter_idx = idx;
            self.apply_filter();
        }
    }

    /// Get the filtered tree for display.
    pub fn filtered_tree(&self) -> &AssetTree {
        &self.filtered_tree
    }

    /// Get metadata for the selected asset (if any).
    pub fn selected_metadata(&self) -> Option<&AssetMetadata> {
        let key = self.selected_asset.as_ref()?;
        self.find_metadata_in_tree(key, &self.filtered_tree)
    }

    /// Find metadata for a key in the tree.
    fn find_metadata_in_tree<'a>(
        &self,
        key: &AssetKey,
        tree: &'a AssetTree,
    ) -> Option<&'a AssetMetadata> {
        fn find_in_node<'a>(key: &AssetKey, node: &'a AssetTreeNode) -> Option<&'a AssetMetadata> {
            match node {
                AssetTreeNode::Folder { children, .. } => {
                    for child in children {
                        if let Some(meta) = find_in_node(key, child) {
                            return Some(meta);
                        }
                    }
                    None
                }
                AssetTreeNode::Asset {
                    key: node_key,
                    metadata,
                } => {
                    if node_key == key {
                        Some(metadata)
                    } else {
                        None
                    }
                }
            }
        }

        for root in &tree.roots {
            if let Some(meta) = find_in_node(key, root) {
                return Some(meta);
            }
        }
        None
    }

    /// Toggle expanded state for a folder path.
    pub fn toggle_expanded(&mut self, path: &str) {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.to_string());
        }
    }

    /// Check if a folder is expanded.
    pub fn is_expanded(&self, path: &str) -> bool {
        self.expanded.contains(path)
    }

    /// Select an asset.
    pub fn select(&mut self, key: AssetKey) {
        self.selected_asset = Some(key);
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selected_asset = None;
    }

    /// Get type filter display labels.
    pub fn type_filter_labels() -> Vec<&'static str> {
        TYPE_FILTER_OPTIONS
            .iter()
            .map(|(label, _)| *label)
            .collect()
    }

    // =========================================================================
    // ImGui Rendering
    // =========================================================================

    /// Render the browser panel using imgui.
    /// Returns any action triggered (Load, Edit, Delete) for the caller to handle.
    pub fn render(&mut self, ui: &Ui, store: &dyn BlobStore) -> Option<BrowserAction> {
        // Refresh tree if needed
        self.refresh_if_needed(store);

        let mut action = None;

        // Search bar
        let mut search_changed = false;
        let mut new_search = self.search_query.clone();
        if ui
            .input_text("Search##browser", &mut new_search)
            .hint("Filter assets...")
            .build()
        {
            search_changed = true;
        }
        if search_changed && new_search != self.search_query {
            self.set_search(new_search);
        }

        ui.same_line();

        // Type filter combo
        let labels = Self::type_filter_labels();
        let current_label = labels.get(self.type_filter_idx).copied().unwrap_or("All");
        ui.set_next_item_width(100.0);
        if let Some(_combo) = ui.begin_combo("##type_filter", current_label) {
            for (idx, label) in labels.iter().enumerate() {
                let selected = idx == self.type_filter_idx;
                if ui.selectable_config(*label).selected(selected).build() {
                    self.set_type_filter(idx);
                }
            }
        }

        ui.separator();

        // Two-panel layout: Tree on left, Details on right
        // Using columns for simplicity (imgui child windows would be more complex)
        let avail = ui.content_region_avail();
        let tree_width = avail[0] * 0.45;

        // Left panel: Tree view
        ui.child_window("tree_panel")
            .size([tree_width, avail[1] - 40.0])
            .border(true)
            .build(|| {
                self.render_tree(ui);
            });

        ui.same_line();

        // Right panel: Detail view
        ui.child_window("detail_panel")
            .size([avail[0] - tree_width - 10.0, avail[1] - 40.0])
            .border(true)
            .build(|| {
                action = self.render_detail(ui);
            });

        action
    }

    /// Render the tree view.
    fn render_tree(&mut self, ui: &Ui) {
        let tree = self.filtered_tree.clone(); // Clone to avoid borrow issues
        for root in &tree.roots {
            self.render_tree_node(ui, root);
        }

        if tree.is_empty() {
            ui.text_disabled("No assets found");
        }
    }

    /// Render a single tree node (recursive).
    fn render_tree_node(&mut self, ui: &Ui, node: &AssetTreeNode) {
        match node {
            AssetTreeNode::Folder {
                name,
                path,
                children,
            } => {
                let is_expanded = self.is_expanded(path);
                let flags = if is_expanded {
                    imgui::TreeNodeFlags::DEFAULT_OPEN
                } else {
                    imgui::TreeNodeFlags::empty()
                };

                let tree_node = ui.tree_node_config(name).flags(flags).build(|| {
                    for child in children {
                        self.render_tree_node(ui, child);
                    }
                });

                // Track expanded state based on whether tree node is open
                if tree_node.is_some() {
                    if !is_expanded {
                        self.expanded.insert(path.clone());
                    }
                } else if is_expanded {
                    self.expanded.remove(path);
                }
            }
            AssetTreeNode::Asset { key, metadata } => {
                let is_selected = self.selected_asset.as_ref() == Some(key);

                // Type icon prefix
                let icon = match metadata.asset_type.as_str() {
                    "material" => "[M]",
                    "generator" => "[G]",
                    "renderer" => "[R]",
                    "visualizer" => "[V]",
                    _ => "[?]",
                };

                let label = format!("{} {}", icon, metadata.name);

                if ui.selectable_config(&label).selected(is_selected).build() {
                    self.selected_asset = Some(key.clone());
                }
            }
        }
    }

    /// Render the detail panel for selected asset.
    /// Returns any triggered action.
    fn render_detail(&mut self, ui: &Ui) -> Option<BrowserAction> {
        let Some(key) = self.selected_asset.clone() else {
            ui.text_disabled("Select an asset to view details");
            return None;
        };

        let Some(metadata) = self.selected_metadata().cloned() else {
            ui.text_disabled("Asset not found");
            return None;
        };

        // Asset name (large, colored)
        ui.text_colored([1.0, 0.9, 0.3, 1.0], &metadata.name);
        ui.separator();

        // Key path
        ui.text_disabled(format!("Key: {}", key));
        ui.spacing();

        // Type
        ui.text(format!("Type: {}", metadata.asset_type));
        ui.spacing();

        // Description
        if let Some(ref desc) = metadata.description {
            ui.text_wrapped(desc);
            ui.spacing();
        }

        // Tags
        if !metadata.tags.is_empty() {
            ui.text("Tags:");
            ui.same_line();
            for tag in &metadata.tags {
                ui.same_line();
                ui.text_colored([0.6, 0.8, 1.0, 1.0], format!("[{}]", tag));
            }
            ui.spacing();
        }

        // Material preview (color swatch) - parse from Lua if possible
        if metadata.asset_type == "material" {
            ui.separator();
            ui.text("Preview:");
            // Placeholder color swatch - would need Lua parsing for real color
            let _color_token = ui.push_style_color(imgui::StyleColor::Button, [0.5, 0.5, 0.5, 1.0]);
            ui.button_with_size("  Color  ", [60.0, 30.0]);
            ui.text_disabled("(parse Lua for actual color)");
        }

        ui.separator();

        // Action buttons
        let mut action = None;

        if ui.button("Load") {
            action = Some(BrowserAction::Load(key.clone()));
        }
        ui.same_line();
        if ui.button("Edit") {
            action = Some(BrowserAction::Edit(key.clone()));
        }
        ui.same_line();

        // Delete button (red)
        let _color_token = ui.push_style_color(imgui::StyleColor::Button, [0.6, 0.2, 0.2, 1.0]);
        if ui.button("Delete") {
            action = Some(BrowserAction::Delete(key.clone()));
        }

        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_asset(namespace: &str, path: &str, name: &str, asset_type: &str) -> AssetRef {
        AssetRef {
            key: AssetKey::new(namespace, path),
            metadata: AssetMetadata {
                name: name.to_string(),
                description: None,
                tags: Vec::new(),
                asset_type: asset_type.to_string(),
                updated_at: Utc::now(),
            },
        }
    }

    fn make_asset_with_desc(
        namespace: &str,
        path: &str,
        name: &str,
        asset_type: &str,
        desc: &str,
        tags: Vec<&str>,
    ) -> AssetRef {
        AssetRef {
            key: AssetKey::new(namespace, path),
            metadata: AssetMetadata {
                name: name.to_string(),
                description: Some(desc.to_string()),
                tags: tags.into_iter().map(|s| s.to_string()).collect(),
                asset_type: asset_type.to_string(),
                updated_at: Utc::now(),
            },
        }
    }

    #[test]
    fn test_tree_from_empty() {
        let tree = AssetTree::from_assets(vec![]);
        assert!(tree.is_empty());
        assert_eq!(tree.asset_count(), 0);
    }

    #[test]
    fn test_tree_single_asset() {
        let assets = vec![make_asset(
            "paul",
            "materials/crystal",
            "Crystal",
            "material",
        )];
        let tree = AssetTree::from_assets(assets);

        assert!(!tree.is_empty());
        assert_eq!(tree.asset_count(), 1);
        assert_eq!(tree.roots.len(), 1);

        // Check structure: paul/ -> materials/ -> crystal
        let paul = &tree.roots[0];
        assert!(paul.is_folder());
        assert_eq!(paul.name(), "paul/");

        if let AssetTreeNode::Folder { children, .. } = paul {
            assert_eq!(children.len(), 1);
            let materials = &children[0];
            assert!(materials.is_folder());
            assert_eq!(materials.name(), "materials");

            if let AssetTreeNode::Folder { children, .. } = materials {
                assert_eq!(children.len(), 1);
                let crystal = &children[0];
                assert!(!crystal.is_folder());
                assert_eq!(crystal.name(), "Crystal");
            } else {
                panic!("Expected folder");
            }
        } else {
            panic!("Expected folder");
        }
    }

    #[test]
    fn test_tree_multiple_namespaces() {
        let assets = vec![
            make_asset("paul", "materials/stone", "Stone", "material"),
            make_asset("shared", "materials/water", "Water", "material"),
        ];
        let tree = AssetTree::from_assets(assets);

        assert_eq!(tree.asset_count(), 2);
        assert_eq!(tree.roots.len(), 2);

        // Roots should be sorted alphabetically
        assert_eq!(tree.roots[0].name(), "paul/");
        assert_eq!(tree.roots[1].name(), "shared/");
    }

    #[test]
    fn test_tree_multiple_assets_same_folder() {
        let assets = vec![
            make_asset("ns", "materials/stone", "Stone", "material"),
            make_asset("ns", "materials/dirt", "Dirt", "material"),
            make_asset("ns", "materials/crystal", "Crystal", "material"),
        ];
        let tree = AssetTree::from_assets(assets);

        assert_eq!(tree.asset_count(), 3);

        // Navigate to materials folder
        if let AssetTreeNode::Folder { children, .. } = &tree.roots[0] {
            if let AssetTreeNode::Folder { children, .. } = &children[0] {
                // Assets should be sorted by name
                assert_eq!(children.len(), 3);
                assert_eq!(children[0].name(), "Crystal");
                assert_eq!(children[1].name(), "Dirt");
                assert_eq!(children[2].name(), "Stone");
            } else {
                panic!("Expected folder");
            }
        } else {
            panic!("Expected folder");
        }
    }

    #[test]
    fn test_tree_filter_by_type() {
        let assets = vec![
            make_asset("ns", "materials/stone", "Stone", "material"),
            make_asset("ns", "generators/maze", "Maze", "generator"),
        ];
        let tree = AssetTree::from_assets(assets);

        // Filter to materials only
        let filtered = tree.filter("", Some("material"));
        assert_eq!(filtered.asset_count(), 1);

        // Filter to generators only
        let filtered = tree.filter("", Some("generator"));
        assert_eq!(filtered.asset_count(), 1);

        // No filter
        let filtered = tree.filter("", None);
        assert_eq!(filtered.asset_count(), 2);
    }

    #[test]
    fn test_tree_filter_by_query() {
        let assets = vec![
            make_asset_with_desc(
                "ns",
                "materials/crystal",
                "Crystal",
                "material",
                "A glowing blue gemstone",
                vec!["gem", "glow"],
            ),
            make_asset_with_desc(
                "ns",
                "materials/stone",
                "Stone",
                "material",
                "Basic rock",
                vec!["natural"],
            ),
        ];
        let tree = AssetTree::from_assets(assets);

        // Search by name
        let filtered = tree.filter("crystal", None);
        assert_eq!(filtered.asset_count(), 1);

        // Search by description
        let filtered = tree.filter("glowing", None);
        assert_eq!(filtered.asset_count(), 1);

        // Search by tag
        let filtered = tree.filter("gem", None);
        assert_eq!(filtered.asset_count(), 1);

        // Search that matches nothing
        let filtered = tree.filter("nonexistent", None);
        assert_eq!(filtered.asset_count(), 0);

        // Case insensitive
        let filtered = tree.filter("CRYSTAL", None);
        assert_eq!(filtered.asset_count(), 1);
    }

    #[test]
    fn test_tree_filter_combined() {
        let assets = vec![
            make_asset("ns", "materials/crystal", "Crystal", "material"),
            make_asset("ns", "generators/crystal_gen", "Crystal Gen", "generator"),
        ];
        let tree = AssetTree::from_assets(assets);

        // Filter by query "crystal" and type "material"
        let filtered = tree.filter("crystal", Some("material"));
        assert_eq!(filtered.asset_count(), 1);

        // The asset should be the material, not the generator
        if let AssetTreeNode::Folder { children, .. } = &filtered.roots[0] {
            // Should only have materials folder
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].name(), "materials");
        } else {
            panic!("Expected folder");
        }
    }

    #[test]
    fn test_browser_state() {
        let mut browser = AssetBrowser::new();

        assert!(browser.selected_asset.is_none());
        assert!(browser.search_query.is_empty());
        assert_eq!(browser.type_filter_idx, 0);
        assert!(browser.expanded.is_empty());

        // Test selection
        let key = AssetKey::new("ns", "path");
        browser.select(key.clone());
        assert_eq!(browser.selected_asset, Some(key));

        browser.clear_selection();
        assert!(browser.selected_asset.is_none());

        // Test expanded
        browser.toggle_expanded("ns/materials");
        assert!(browser.is_expanded("ns/materials"));

        browser.toggle_expanded("ns/materials");
        assert!(!browser.is_expanded("ns/materials"));
    }

    #[test]
    fn test_browser_set_search() {
        let mut browser = AssetBrowser::new();

        // Manually set the tree
        let assets = vec![
            make_asset("ns", "materials/stone", "Stone", "material"),
            make_asset("ns", "materials/crystal", "Crystal", "material"),
        ];
        browser.tree = AssetTree::from_assets(assets);
        browser.apply_filter();

        assert_eq!(browser.filtered_tree().asset_count(), 2);

        // Apply search filter
        browser.set_search("stone".to_string());
        assert_eq!(browser.filtered_tree().asset_count(), 1);

        // Clear search
        browser.set_search(String::new());
        assert_eq!(browser.filtered_tree().asset_count(), 2);
    }

    #[test]
    fn test_browser_set_type_filter() {
        let mut browser = AssetBrowser::new();

        let assets = vec![
            make_asset("ns", "materials/stone", "Stone", "material"),
            make_asset("ns", "generators/maze", "Maze", "generator"),
        ];
        browser.tree = AssetTree::from_assets(assets);
        browser.apply_filter();

        assert_eq!(browser.filtered_tree().asset_count(), 2);

        // Filter to materials only (index 1)
        browser.set_type_filter(1);
        assert_eq!(browser.filtered_tree().asset_count(), 1);

        // Back to all (index 0)
        browser.set_type_filter(0);
        assert_eq!(browser.filtered_tree().asset_count(), 2);
    }

    #[test]
    fn test_type_filter_labels() {
        let labels = AssetBrowser::type_filter_labels();
        assert_eq!(
            labels,
            vec!["All", "Materials", "Generators", "Renderers", "Visualizers"]
        );
    }

    #[test]
    fn test_browser_action_enum() {
        let key = AssetKey::new("ns", "path");

        let load = BrowserAction::Load(key.clone());
        let edit = BrowserAction::Edit(key.clone());
        let delete = BrowserAction::Delete(key.clone());

        assert_eq!(load, BrowserAction::Load(key.clone()));
        assert_ne!(load, edit);
        assert_ne!(edit, delete);
    }

    #[test]
    fn test_browser_refresh_from_store() {
        use crate::map_editor::asset::InMemoryBlobStore;

        let store = InMemoryBlobStore::new();

        // Add some assets
        store
            .set(
                &AssetKey::new("paul", "materials/crystal"),
                b"lua",
                AssetMetadata {
                    name: "Crystal".to_string(),
                    description: Some("A glowing gem".to_string()),
                    tags: vec!["gem".to_string()],
                    asset_type: "material".to_string(),
                    updated_at: Utc::now(),
                },
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("paul", "generators/maze"),
                b"lua",
                AssetMetadata {
                    name: "Maze".to_string(),
                    description: None,
                    tags: vec![],
                    asset_type: "generator".to_string(),
                    updated_at: Utc::now(),
                },
            )
            .unwrap();
        store
            .set(
                &AssetKey::new("shared", "materials/water"),
                b"lua",
                AssetMetadata {
                    name: "Water".to_string(),
                    description: None,
                    tags: vec![],
                    asset_type: "material".to_string(),
                    updated_at: Utc::now(),
                },
            )
            .unwrap();

        // Create browser and refresh
        let mut browser = AssetBrowser::new();
        browser.refresh(&store);

        // Verify tree structure
        assert_eq!(browser.filtered_tree().asset_count(), 3);
        assert_eq!(browser.filtered_tree().roots.len(), 2); // paul, shared

        // Test selection
        let key = AssetKey::new("paul", "materials/crystal");
        browser.select(key.clone());
        assert_eq!(browser.selected_asset, Some(key.clone()));

        // Test selected metadata lookup
        let meta = browser.selected_metadata().unwrap();
        assert_eq!(meta.name, "Crystal");
        assert_eq!(meta.asset_type, "material");

        // Test search filter
        browser.set_search("crystal".to_string());
        assert_eq!(browser.filtered_tree().asset_count(), 1);

        // Test type filter
        browser.set_search(String::new());
        browser.set_type_filter(2); // Generators
        assert_eq!(browser.filtered_tree().asset_count(), 1);

        // Back to all
        browser.set_type_filter(0);
        assert_eq!(browser.filtered_tree().asset_count(), 3);
    }

    #[test]
    fn test_browser_mark_dirty() {
        let mut browser = AssetBrowser::new();

        // Initially needs refresh
        assert!(browser.needs_refresh);

        // After setting tree manually, mark as not needing refresh
        browser.tree = AssetTree::from_assets(vec![]);
        browser.needs_refresh = false;

        // Mark dirty
        browser.mark_dirty();
        assert!(browser.needs_refresh);
    }
}
