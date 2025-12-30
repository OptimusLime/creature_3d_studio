//! World save/load functionality for VoxelWorld.
//!
//! Supports multiple formats:
//! - `.voxworld` - Binary format (fast, compact)
//! - `.voxworld.json` - JSON format (human readable, debuggable)
//!
//! # Example
//!
//! ```ignore
//! use studio_core::{VoxelWorld, Voxel, save_world, load_world};
//!
//! let mut world = VoxelWorld::new();
//! world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
//!
//! // Save to binary format
//! save_world(&world, "worlds/test.voxworld").unwrap();
//!
//! // Load it back
//! let loaded = load_world("worlds/test.voxworld").unwrap();
//! ```

use crate::voxel::VoxelWorld;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

/// Magic bytes for binary voxworld files
const MAGIC: &[u8; 8] = b"VOXWORLD";

/// Current file format version
const VERSION: u32 = 1;

/// Errors that can occur during world I/O operations.
#[derive(Debug)]
pub enum WorldIoError {
    /// File system error
    Io(std::io::Error),
    /// Binary serialization error
    Bincode(bincode::Error),
    /// JSON serialization error
    Json(String),
    /// Invalid file format
    InvalidFormat(String),
    /// Unsupported version
    UnsupportedVersion(u32),
}

impl std::fmt::Display for WorldIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldIoError::Io(e) => write!(f, "IO error: {}", e),
            WorldIoError::Bincode(e) => write!(f, "Bincode error: {}", e),
            WorldIoError::Json(e) => write!(f, "JSON error: {}", e),
            WorldIoError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
            WorldIoError::UnsupportedVersion(v) => write!(f, "Unsupported version: {}", v),
        }
    }
}

impl std::error::Error for WorldIoError {}

impl From<std::io::Error> for WorldIoError {
    fn from(e: std::io::Error) -> Self {
        WorldIoError::Io(e)
    }
}

impl From<bincode::Error> for WorldIoError {
    fn from(e: bincode::Error) -> Self {
        WorldIoError::Bincode(e)
    }
}

/// Result type for world I/O operations.
pub type WorldIoResult<T> = Result<T, WorldIoError>;

/// Save a VoxelWorld to a file.
///
/// Format is determined by file extension:
/// - `.voxworld` - Binary format (default)
/// - `.json` or `.voxworld.json` - JSON format
///
/// # Example
///
/// ```ignore
/// save_world(&world, "worlds/test.voxworld")?;
/// save_world(&world, "worlds/test.json")?;
/// ```
pub fn save_world<P: AsRef<Path>>(world: &VoxelWorld, path: P) -> WorldIoResult<()> {
    let path = path.as_ref();
    let path_str = path.to_string_lossy().to_lowercase();
    
    if path_str.ends_with(".json") {
        save_world_json(world, path)
    } else {
        save_world_binary(world, path)
    }
}

/// Load a VoxelWorld from a file.
///
/// Format is determined by file extension:
/// - `.voxworld` - Binary format (default)
/// - `.json` or `.voxworld.json` - JSON format
///
/// # Example
///
/// ```ignore
/// let world = load_world("worlds/test.voxworld")?;
/// let world = load_world("worlds/test.json")?;
/// ```
pub fn load_world<P: AsRef<Path>>(path: P) -> WorldIoResult<VoxelWorld> {
    let path = path.as_ref();
    let path_str = path.to_string_lossy().to_lowercase();
    
    if path_str.ends_with(".json") {
        load_world_json(path)
    } else {
        load_world_binary(path)
    }
}

/// Save world in binary format (fast, compact).
pub fn save_world_binary<P: AsRef<Path>>(world: &VoxelWorld, path: P) -> WorldIoResult<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    
    // Write header
    writer.write_all(MAGIC)?;
    writer.write_all(&VERSION.to_le_bytes())?;
    
    // Write world data
    let data = bincode::serialize(world)?;
    let size = data.len() as u64;
    writer.write_all(&size.to_le_bytes())?;
    writer.write_all(&data)?;
    
    writer.flush()?;
    Ok(())
}

/// Load world from binary format.
pub fn load_world_binary<P: AsRef<Path>>(path: P) -> WorldIoResult<VoxelWorld> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    
    // Read and verify magic bytes
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(WorldIoError::InvalidFormat(
            "Invalid magic bytes - not a voxworld file".to_string()
        ));
    }
    
    // Read and check version
    let mut version_bytes = [0u8; 4];
    reader.read_exact(&mut version_bytes)?;
    let version = u32::from_le_bytes(version_bytes);
    if version > VERSION {
        return Err(WorldIoError::UnsupportedVersion(version));
    }
    
    // Read world data
    let mut size_bytes = [0u8; 8];
    reader.read_exact(&mut size_bytes)?;
    let size = u64::from_le_bytes(size_bytes) as usize;
    
    let mut data = vec![0u8; size];
    reader.read_exact(&mut data)?;
    
    let world: VoxelWorld = bincode::deserialize(&data)?;
    Ok(world)
}

/// Save world in JSON format (human readable).
pub fn save_world_json<P: AsRef<Path>>(world: &VoxelWorld, path: P) -> WorldIoResult<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    
    serde_json::to_writer_pretty(writer, world)
        .map_err(|e| WorldIoError::Json(e.to_string()))?;
    
    Ok(())
}

/// Load world from JSON format.
pub fn load_world_json<P: AsRef<Path>>(path: P) -> WorldIoResult<VoxelWorld> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    
    let world: VoxelWorld = serde_json::from_reader(reader)
        .map_err(|e| WorldIoError::Json(e.to_string()))?;
    
    Ok(world)
}

/// Get information about a world file without fully loading it.
#[derive(Debug, Clone)]
pub struct WorldFileInfo {
    /// File format (binary or json)
    pub format: WorldFormat,
    /// File version (for binary files)
    pub version: Option<u32>,
    /// File size in bytes
    pub file_size: u64,
}

/// World file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldFormat {
    Binary,
    Json,
}

/// Get information about a world file.
pub fn world_file_info<P: AsRef<Path>>(path: P) -> WorldIoResult<WorldFileInfo> {
    let path = path.as_ref();
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();
    
    let path_str = path.to_string_lossy().to_lowercase();
    
    if path_str.ends_with(".json") {
        Ok(WorldFileInfo {
            format: WorldFormat::Json,
            version: None,
            file_size,
        })
    } else {
        // Read header from binary file
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        
        if &magic != MAGIC {
            return Err(WorldIoError::InvalidFormat(
                "Invalid magic bytes".to_string()
            ));
        }
        
        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);
        
        Ok(WorldFileInfo {
            format: WorldFormat::Binary,
            version: Some(version),
            file_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::Voxel;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_world() -> VoxelWorld {
        let mut world = VoxelWorld::new();
        world.set_voxel(0, 0, 0, Voxel::solid(255, 0, 0));
        world.set_voxel(10, 20, 30, Voxel::solid(0, 255, 0));
        world.set_voxel(-5, 0, 5, Voxel::emissive(0, 0, 255));
        world
    }

    #[test]
    fn test_save_load_binary() {
        let world = create_test_world();
        let temp_file = NamedTempFile::with_suffix(".voxworld").unwrap();
        let path = temp_file.path();

        // Save
        save_world_binary(&world, path).unwrap();

        // Load
        let loaded = load_world_binary(path).unwrap();

        // Verify
        assert_eq!(loaded.chunk_count(), world.chunk_count());
        assert_eq!(loaded.total_voxel_count(), world.total_voxel_count());
        assert_eq!(loaded.get_voxel(0, 0, 0), world.get_voxel(0, 0, 0));
        assert_eq!(loaded.get_voxel(10, 20, 30), world.get_voxel(10, 20, 30));
        assert_eq!(loaded.get_voxel(-5, 0, 5), world.get_voxel(-5, 0, 5));
    }

    #[test]
    fn test_save_load_json() {
        let world = create_test_world();
        let temp_file = NamedTempFile::with_suffix(".json").unwrap();
        let path = temp_file.path();

        // Save
        save_world_json(&world, path).unwrap();

        // Load
        let loaded = load_world_json(path).unwrap();

        // Verify
        assert_eq!(loaded.chunk_count(), world.chunk_count());
        assert_eq!(loaded.total_voxel_count(), world.total_voxel_count());
    }

    #[test]
    fn test_auto_format_detection() {
        let world = create_test_world();
        
        // Binary
        let temp_binary = NamedTempFile::with_suffix(".voxworld").unwrap();
        save_world(&world, temp_binary.path()).unwrap();
        let loaded = load_world(temp_binary.path()).unwrap();
        assert_eq!(loaded.total_voxel_count(), world.total_voxel_count());
        
        // JSON
        let temp_json = NamedTempFile::with_suffix(".json").unwrap();
        save_world(&world, temp_json.path()).unwrap();
        let loaded = load_world(temp_json.path()).unwrap();
        assert_eq!(loaded.total_voxel_count(), world.total_voxel_count());
    }

    #[test]
    fn test_invalid_magic() {
        let temp_file = NamedTempFile::with_suffix(".voxworld").unwrap();
        let mut file = File::create(temp_file.path()).unwrap();
        file.write_all(b"INVALID!").unwrap();
        
        let result = load_world_binary(temp_file.path());
        assert!(matches!(result, Err(WorldIoError::InvalidFormat(_))));
    }

    #[test]
    fn test_world_file_info() {
        let world = create_test_world();
        let temp_file = NamedTempFile::with_suffix(".voxworld").unwrap();
        save_world_binary(&world, temp_file.path()).unwrap();
        
        let info = world_file_info(temp_file.path()).unwrap();
        assert_eq!(info.format, WorldFormat::Binary);
        assert_eq!(info.version, Some(VERSION));
        assert!(info.file_size > 0);
    }
}
