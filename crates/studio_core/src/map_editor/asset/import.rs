//! Import handlers for auto-importing files to the asset database.
//!
//! # Architecture
//!
//! ```text
//! File dropped in assets/incoming/
//!     │
//!     ▼
//! ImportHandler::can_handle(path) ──► true/false
//!     │
//!     ▼ (if true)
//! ImportHandler::import(path, content)
//!     │
//!     ▼
//! (Vec<u8>, AssetMetadata) ──► stored in BlobStore
//! ```
//!
//! # Handlers
//!
//! - `LuaAssetHandler`: Handles all `.lua` files, extracts metadata from Lua tables
//!
//! # Adding New Handlers
//!
//! Implement the `ImportHandler` trait and register with `ImportHandlerRegistry`.

use super::{AssetError, AssetMetadata};
use mlua::{Lua, Value};
use std::path::Path;

/// Error during import.
#[derive(Debug)]
pub enum ImportError {
    /// File type not supported.
    UnsupportedFileType(String),
    /// Failed to read file.
    IoError(std::io::Error),
    /// Failed to parse content.
    ParseError(String),
    /// Asset store error.
    StoreError(AssetError),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::UnsupportedFileType(ext) => {
                write!(f, "Unsupported file type: {}", ext)
            }
            ImportError::IoError(e) => write!(f, "IO error: {}", e),
            ImportError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ImportError::StoreError(e) => write!(f, "Store error: {:?}", e),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::IoError(e)
    }
}

impl From<AssetError> for ImportError {
    fn from(e: AssetError) -> Self {
        ImportError::StoreError(e)
    }
}

/// Result type for import operations.
pub type ImportResult<T> = Result<T, ImportError>;

/// Handler for importing a specific file type into the asset store.
///
/// Each handler knows how to:
/// 1. Check if it can handle a file (by extension or content)
/// 2. Extract metadata from the file content
/// 3. Return content + metadata for storage
pub trait ImportHandler: Send + Sync {
    /// File extensions this handler supports (e.g., ["lua"]).
    fn extensions(&self) -> &[&str];

    /// Check if this handler can process the given file.
    /// Default implementation checks extension.
    fn can_handle(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            self.extensions().iter().any(|e| *e == ext_str)
        } else {
            false
        }
    }

    /// Import file content and extract metadata.
    ///
    /// Returns (content_to_store, metadata).
    /// The content is typically the raw file bytes.
    fn import(&self, path: &Path, content: &[u8]) -> ImportResult<(Vec<u8>, AssetMetadata)>;
}

/// Unified Lua asset handler that handles all `.lua` files.
///
/// Detects asset type from Lua content:
/// - Materials: `{ name = "X", color = {...} }` or array of materials
/// - Generators: Contains `Generator:extend` or `function init/step`
/// - Renderers: Contains `Renderer:extend` or renderer patterns
/// - Visualizers: Contains `Visualizer:extend` or visualizer patterns
///
/// Falls back to "lua" type if can't detect.
pub struct LuaAssetHandler;

impl LuaAssetHandler {
    /// Create a new Lua asset handler.
    pub fn new() -> Self {
        Self
    }

    /// Detect asset type from Lua source content.
    fn detect_asset_type(source: &str) -> &'static str {
        // Check for class extension patterns first (most specific)
        if source.contains("Generator:extend") || source.contains("generator:extend") {
            return "generator";
        }
        if source.contains("Renderer:extend") || source.contains("renderer:extend") {
            return "renderer";
        }
        if source.contains("Visualizer:extend") || source.contains("visualizer:extend") {
            return "visualizer";
        }

        // Check for function patterns
        let has_init = source.contains("function") && source.contains(":init");
        let has_step = source.contains("function") && source.contains(":step");
        let has_render = source.contains("function") && source.contains(":render");

        if has_init && has_step && has_render {
            // Has all three - likely a visualizer
            return "visualizer";
        }
        if has_init && has_step {
            // Has init and step - likely a generator
            return "generator";
        }
        if has_render {
            // Has render - likely a renderer
            return "renderer";
        }

        // Check for material table patterns
        if source.contains("color") && (source.contains("name") || source.contains("id")) {
            return "material";
        }

        // Default to generic lua
        "lua"
    }

    /// Extract metadata from Lua table.
    fn extract_metadata(
        source: &str,
        path: &Path,
        asset_type: &str,
    ) -> ImportResult<AssetMetadata> {
        let lua = Lua::new();

        // Try to evaluate and extract metadata
        let metadata = match lua.load(source).eval::<Value>() {
            Ok(Value::Table(table)) => {
                // Extract name (use filename as fallback)
                let name: String = table
                    .get::<String>("name")
                    .ok()
                    .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| "unnamed".to_string());

                // Extract description
                let description: Option<String> = table.get("description").ok();

                // Extract tags
                let tags: Vec<String> = table
                    .get::<mlua::Table>("tags")
                    .ok()
                    .map(|t| {
                        t.sequence_values::<String>()
                            .filter_map(|r| r.ok())
                            .collect()
                    })
                    .unwrap_or_default();

                AssetMetadata::new(&name, asset_type)
                    .with_description_opt(description)
                    .with_tags(tags)
            }
            _ => {
                // Couldn't parse as table, use filename
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "unnamed".to_string());

                AssetMetadata::new(&name, asset_type)
            }
        };

        Ok(metadata)
    }
}

impl Default for LuaAssetHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportHandler for LuaAssetHandler {
    fn extensions(&self) -> &[&str] {
        &["lua"]
    }

    fn import(&self, path: &Path, content: &[u8]) -> ImportResult<(Vec<u8>, AssetMetadata)> {
        let source = String::from_utf8_lossy(content);

        // Detect asset type from content
        let asset_type = Self::detect_asset_type(&source);

        // Extract metadata
        let metadata = Self::extract_metadata(&source, path, asset_type)?;

        Ok((content.to_vec(), metadata))
    }
}

/// Registry of import handlers.
///
/// Maintains a list of handlers and routes files to the appropriate one.
pub struct ImportHandlerRegistry {
    handlers: Vec<Box<dyn ImportHandler>>,
}

impl ImportHandlerRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Create registry with default handlers.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(LuaAssetHandler::new()));
        registry
    }

    /// Register a handler.
    pub fn register(&mut self, handler: Box<dyn ImportHandler>) {
        self.handlers.push(handler);
    }

    /// Find a handler for the given path.
    pub fn find_handler(&self, path: &Path) -> Option<&dyn ImportHandler> {
        self.handlers
            .iter()
            .find(|h| h.can_handle(path))
            .map(|h| h.as_ref())
    }

    /// Import a file using the appropriate handler.
    pub fn import(&self, path: &Path, content: &[u8]) -> ImportResult<(Vec<u8>, AssetMetadata)> {
        let handler = self.find_handler(path).ok_or_else(|| {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".to_string());
            ImportError::UnsupportedFileType(ext)
        })?;

        handler.import(path, content)
    }
}

impl Default for ImportHandlerRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lua_handler_detects_material() {
        let source = r#"return { name = "Crystal", color = {0.5, 0.8, 1.0} }"#;
        assert_eq!(LuaAssetHandler::detect_asset_type(source), "material");
    }

    #[test]
    fn test_lua_handler_detects_generator() {
        let source = r#"
            local Generator = require("lib.generator")
            local MyGen = Generator:extend("MyGen")
            function MyGen:init(ctx) end
            function MyGen:step(ctx) end
            return MyGen
        "#;
        assert_eq!(LuaAssetHandler::detect_asset_type(source), "generator");
    }

    #[test]
    fn test_lua_handler_detects_renderer() {
        let source = r#"
            local Renderer = require("lib.renderer")
            local MyRenderer = Renderer:extend("MyRenderer")
            function MyRenderer:render(ctx) end
            return MyRenderer
        "#;
        assert_eq!(LuaAssetHandler::detect_asset_type(source), "renderer");
    }

    #[test]
    fn test_lua_handler_detects_visualizer() {
        let source = r#"
            local Visualizer = require("lib.visualizer")
            local MyVis = Visualizer:extend("MyVis")
            function MyVis:init(ctx) end
            function MyVis:step(ctx) end
            function MyVis:render(ctx) end
            return MyVis
        "#;
        assert_eq!(LuaAssetHandler::detect_asset_type(source), "visualizer");
    }

    #[test]
    fn test_lua_handler_extracts_metadata() {
        let handler = LuaAssetHandler::new();
        let content = br#"return { 
            name = "Ruby", 
            description = "Red gemstone", 
            color = {0.9, 0.1, 0.1},
            tags = {"gem", "red"} 
        }"#;

        let (stored, metadata) = handler.import(Path::new("test/ruby.lua"), content).unwrap();

        assert_eq!(stored, content.to_vec());
        assert_eq!(metadata.name, "Ruby");
        assert_eq!(metadata.description, Some("Red gemstone".to_string()));
        assert_eq!(metadata.asset_type, "material");
        assert_eq!(metadata.tags, vec!["gem", "red"]);
    }

    #[test]
    fn test_lua_handler_uses_filename_as_fallback() {
        let handler = LuaAssetHandler::new();
        let content = b"return { color = {1, 1, 1} }";

        let (_, metadata) = handler
            .import(Path::new("materials/white_stone.lua"), content)
            .unwrap();

        assert_eq!(metadata.name, "white_stone");
    }

    #[test]
    fn test_registry_finds_handler() {
        let registry = ImportHandlerRegistry::with_defaults();

        assert!(registry.find_handler(Path::new("test.lua")).is_some());
        assert!(registry.find_handler(Path::new("test.txt")).is_none());
        assert!(registry.find_handler(Path::new("test.LUA")).is_some()); // Case insensitive
    }

    #[test]
    fn test_registry_import() {
        let registry = ImportHandlerRegistry::with_defaults();
        let content = br#"return { name = "Test", color = {1, 0, 0} }"#;

        let result = registry.import(Path::new("test.lua"), content);
        assert!(result.is_ok());

        let (_, metadata) = result.unwrap();
        assert_eq!(metadata.name, "Test");
    }
}
