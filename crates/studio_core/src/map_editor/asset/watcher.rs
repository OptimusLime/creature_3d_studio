//! File watcher for auto-importing assets to the database.
//!
//! # Architecture
//!
//! ```text
//! assets/incoming/
//! ├── paul/              ◄── namespace = "paul"
//! │   ├── materials/
//! │   │   └── crystal.lua  ◄── key = "paul/materials/crystal"
//! │   └── generators/
//! │       └── maze.lua     ◄── key = "paul/generators/maze"
//! └── shared/            ◄── namespace = "shared"
//!     └── ...
//! ```
//!
//! The watcher:
//! 1. Monitors the watch directory recursively
//! 2. On file create/modify: imports to database with extracted metadata
//! 3. On file delete: removes from database
//! 4. Queues embedding generation for new assets (via EmbeddingService)
//!
//! # Usage
//!
//! ```ignore
//! let watcher = AssetFileWatcher::new(
//!     Path::new("assets/incoming"),
//!     store.clone(),
//!     Some(embedding_service.clone()),
//! )?;
//! watcher.start()?;
//! ```

use super::import::{ImportError, ImportHandlerRegistry};
use super::{AssetError, AssetKey, BlobStore, EmbedRequest, EmbeddingService};
use bevy::log::{error, info, warn};
use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

/// Error from the file watcher.
#[derive(Debug)]
pub enum WatchError {
    /// Failed to create watcher.
    WatcherCreation(String),
    /// Failed to watch directory.
    WatchDirectory(String),
    /// Import failed.
    Import(ImportError),
    /// Store error.
    Store(AssetError),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchError::WatcherCreation(msg) => write!(f, "Failed to create watcher: {}", msg),
            WatchError::WatchDirectory(msg) => write!(f, "Failed to watch directory: {}", msg),
            WatchError::Import(e) => write!(f, "Import error: {}", e),
            WatchError::Store(e) => write!(f, "Store error: {:?}", e),
        }
    }
}

impl std::error::Error for WatchError {}

impl From<ImportError> for WatchError {
    fn from(e: ImportError) -> Self {
        WatchError::Import(e)
    }
}

impl From<AssetError> for WatchError {
    fn from(e: AssetError) -> Self {
        WatchError::Store(e)
    }
}

/// File watcher that auto-imports assets to the database.
pub struct AssetFileWatcher {
    /// The watcher (kept alive to maintain watches).
    _watcher: RecommendedWatcher,
    /// Receiver for file events.
    receiver: Receiver<Result<Event, notify::Error>>,
    /// Directory being watched.
    watch_dir: PathBuf,
    /// Import handler registry.
    handlers: ImportHandlerRegistry,
    /// Asset store to import into.
    store: Arc<dyn BlobStore>,
    /// Optional embedding service for generating embeddings.
    embedding_service: Option<Arc<EmbeddingService>>,
}

impl AssetFileWatcher {
    /// Create a new file watcher.
    ///
    /// # Arguments
    ///
    /// * `watch_dir` - Directory to watch (e.g., "assets/incoming")
    /// * `store` - Asset store to import into
    /// * `embedding_service` - Optional embedding service for semantic search
    ///
    /// # Errors
    ///
    /// Returns error if watcher creation or directory watch fails.
    pub fn new(
        watch_dir: &Path,
        store: Arc<dyn BlobStore>,
        embedding_service: Option<Arc<EmbeddingService>>,
    ) -> Result<Self, WatchError> {
        let (tx, rx) = channel();

        let mut watcher = recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .map_err(|e| WatchError::WatcherCreation(e.to_string()))?;

        // Create watch directory if it doesn't exist
        if !watch_dir.exists() {
            std::fs::create_dir_all(watch_dir)
                .map_err(|e| WatchError::WatchDirectory(format!("Failed to create dir: {}", e)))?;
        }

        // Canonicalize watch_dir so path prefix stripping works correctly
        // (notify returns absolute paths on most platforms)
        let watch_dir_canonical = watch_dir.canonicalize().map_err(|e| {
            WatchError::WatchDirectory(format!("Failed to canonicalize dir: {}", e))
        })?;

        watcher
            .watch(&watch_dir_canonical, RecursiveMode::Recursive)
            .map_err(|e| WatchError::WatchDirectory(e.to_string()))?;

        info!(
            "Asset file watcher started on {} (recursive)",
            watch_dir_canonical.display()
        );

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
            watch_dir: watch_dir_canonical,
            handlers: ImportHandlerRegistry::with_defaults(),
            store,
            embedding_service,
        })
    }

    /// Process pending file events.
    ///
    /// Call this periodically (e.g., each frame) to handle file changes.
    /// Returns the number of events processed.
    pub fn process_events(&self) -> usize {
        let mut count = 0;

        loop {
            match self.receiver.try_recv() {
                Ok(Ok(event)) => {
                    self.handle_event(&event);
                    count += 1;
                }
                Ok(Err(e)) => {
                    error!("File watcher error: {:?}", e);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    error!("File watcher channel disconnected");
                    break;
                }
            }
        }

        count
    }

    /// Handle a single file event.
    fn handle_event(&self, event: &Event) {
        match &event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in &event.paths {
                    if path.is_file() {
                        self.import_file(path);
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in &event.paths {
                    self.remove_file(path);
                }
            }
            _ => {}
        }
    }

    /// Import a file to the database.
    fn import_file(&self, path: &Path) {
        // Check if we have a handler for this file type
        if self.handlers.find_handler(path).is_none() {
            // Silently skip unsupported files
            return;
        }

        // Parse path to get namespace and asset path
        let Some((namespace, asset_path)) = self.parse_asset_path(path) else {
            warn!("Could not parse asset path from: {}", path.display());
            return;
        };

        // Read file content
        let content = match std::fs::read(path) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to read file {}: {}", path.display(), e);
                return;
            }
        };

        // Import using handler
        let (stored_content, metadata) = match self.handlers.import(path, &content) {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to import {}: {}", path.display(), e);
                return;
            }
        };

        // Build key
        let key = AssetKey::new(&namespace, &asset_path);

        // Store in database
        if let Err(e) = self.store.set(&key, &stored_content, metadata.clone()) {
            error!("Failed to store asset {}: {:?}", key, e);
            return;
        }

        info!(
            "Auto-imported: {} (type: {}, namespace: {})",
            key, metadata.asset_type, namespace
        );

        // Queue embedding generation if service available
        if let Some(ref service) = self.embedding_service {
            let text = format!(
                "{} {} {}",
                metadata.name,
                metadata.description.as_deref().unwrap_or(""),
                metadata.tags.join(" ")
            );
            service.queue(EmbedRequest {
                key: key.clone(),
                text,
            });
        }
    }

    /// Remove a file from the database.
    fn remove_file(&self, path: &Path) {
        // Parse path to get namespace and asset path
        let Some((namespace, asset_path)) = self.parse_asset_path(path) else {
            return;
        };

        let key = AssetKey::new(&namespace, &asset_path);

        match self.store.delete(&key) {
            Ok(true) => {
                info!("Auto-removed: {} (file deleted)", key);
            }
            Ok(false) => {
                // Asset didn't exist, that's fine
            }
            Err(e) => {
                error!("Failed to remove asset {}: {:?}", key, e);
            }
        }
    }

    /// Parse a file path into (namespace, asset_path).
    ///
    /// Example:
    /// - watch_dir: "assets/incoming"
    /// - path: "assets/incoming/paul/materials/crystal.lua"
    /// - returns: ("paul", "materials/crystal")
    fn parse_asset_path(&self, path: &Path) -> Option<(String, String)> {
        // Try to canonicalize input path to match watch_dir (handles symlinks like /tmp -> /private/tmp)
        // Fall back to original path if canonicalization fails (file doesn't exist yet)
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Get path relative to watch directory
        let relative = canonical_path.strip_prefix(&self.watch_dir).ok()?;

        // First component is namespace
        let mut components = relative.components();
        let namespace = components.next()?.as_os_str().to_string_lossy().to_string();

        // Rest is asset path (without extension)
        let rest: PathBuf = components.collect();
        let asset_path = rest.with_extension("").to_string_lossy().to_string();

        if asset_path.is_empty() {
            return None;
        }

        Some((namespace, asset_path))
    }

    /// Get the watch directory.
    pub fn watch_dir(&self) -> &Path {
        &self.watch_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_editor::asset::InMemoryBlobStore;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_parse_asset_path() {
        let temp = tempdir().unwrap();
        let watch_dir = temp.path().join("incoming");
        fs::create_dir_all(&watch_dir).unwrap();

        let store = Arc::new(InMemoryBlobStore::new());
        let watcher = AssetFileWatcher::new(&watch_dir, store, None).unwrap();

        // Use watcher.watch_dir() which is canonicalized (handles /tmp -> /private/tmp on macOS)
        let canonical_watch_dir = watcher.watch_dir();

        // Test basic path
        let path = canonical_watch_dir.join("paul/materials/crystal.lua");
        let result = watcher.parse_asset_path(&path);
        assert_eq!(
            result,
            Some(("paul".to_string(), "materials/crystal".to_string()))
        );

        // Test nested path
        let path = canonical_watch_dir.join("shared/generators/dungeon/maze.lua");
        let result = watcher.parse_asset_path(&path);
        assert_eq!(
            result,
            Some(("shared".to_string(), "generators/dungeon/maze".to_string()))
        );

        // Test root level file (no asset path)
        let path = canonical_watch_dir.join("readme.txt");
        let result = watcher.parse_asset_path(&path);
        assert_eq!(result, None);
    }

    #[test]
    fn test_import_creates_asset() {
        let temp = tempdir().unwrap();
        let watch_dir = temp.path().join("incoming");
        fs::create_dir_all(watch_dir.join("test/materials")).unwrap();

        let store = Arc::new(InMemoryBlobStore::new());
        let watcher = AssetFileWatcher::new(&watch_dir, store.clone(), None).unwrap();

        // Write a test file
        let lua_path = watch_dir.join("test/materials/ruby.lua");
        fs::write(
            &lua_path,
            r#"return { name = "Ruby", color = {0.9, 0.1, 0.1}, tags = {"gem"} }"#,
        )
        .unwrap();

        // Manually trigger import (normally done via event)
        watcher.import_file(&lua_path);

        // Check asset was created
        let key = AssetKey::new("test", "materials/ruby");
        assert!(store.exists(&key).unwrap());

        let metadata = store.get_metadata(&key).unwrap().unwrap();
        assert_eq!(metadata.name, "Ruby");
        assert_eq!(metadata.asset_type, "material");
        assert_eq!(metadata.tags, vec!["gem"]);
    }

    #[test]
    fn test_remove_deletes_asset() {
        let temp = tempdir().unwrap();
        let watch_dir = temp.path().join("incoming");
        fs::create_dir_all(watch_dir.join("test/materials")).unwrap();

        let store = Arc::new(InMemoryBlobStore::new());
        let watcher = AssetFileWatcher::new(&watch_dir, store.clone(), None).unwrap();

        // Create an asset first
        let lua_path = watch_dir.join("test/materials/ruby.lua");
        fs::write(&lua_path, r#"return { name = "Ruby", color = {1,0,0} }"#).unwrap();
        watcher.import_file(&lua_path);

        let key = AssetKey::new("test", "materials/ruby");
        assert!(store.exists(&key).unwrap());

        // Now remove it
        watcher.remove_file(&lua_path);
        assert!(!store.exists(&key).unwrap());
    }

    #[test]
    fn test_unsupported_file_type_ignored() {
        let temp = tempdir().unwrap();
        let watch_dir = temp.path().join("incoming");
        fs::create_dir_all(watch_dir.join("test")).unwrap();

        let store = Arc::new(InMemoryBlobStore::new());
        let watcher = AssetFileWatcher::new(&watch_dir, store.clone(), None).unwrap();

        // Write a non-Lua file
        let txt_path = watch_dir.join("test/readme.txt");
        fs::write(&txt_path, "This is a readme").unwrap();

        // Try to import - should be silently ignored
        watcher.import_file(&txt_path);

        // No asset should be created
        assert_eq!(store.count("test", None).unwrap(), 0);
    }
}
