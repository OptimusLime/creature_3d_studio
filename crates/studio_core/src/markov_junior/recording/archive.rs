//! Simulation archive for storing and loading recorded simulations.
//!
//! Binary format (.mjsim):
//! ```text
//! Header (fixed 288 bytes):
//!   magic: [u8; 4] = "MJSM"
//!   version: u16 = 1
//!   grid_type_bytes: [u8; 16]
//!   palette_len: u8
//!   palette: [u8; 256] (null-padded)
//!   frame_count: u32
//!   bytes_per_frame: u32
//!   reserved: [u8; 5]
//!
//! Frames (variable):
//!   frame[0]: [u8; bytes_per_frame]
//!   frame[1]: [u8; bytes_per_frame]
//!   ...
//! ```

use super::grid_type::GridType;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

/// Magic bytes for .mjsim files.
const MAGIC: [u8; 4] = *b"MJSM";

/// Current file format version.
const VERSION: u16 = 1;

/// Header size in bytes.
const HEADER_SIZE: usize = 288;

/// A recorded simulation that can be saved/loaded from disk.
#[derive(Debug, Clone)]
pub struct SimulationArchive {
    /// Grid type and dimensions.
    pub grid_type: GridType,
    /// Palette string (e.g., "BWRGMYC").
    pub palette: String,
    /// Recorded frames (each is bytes_per_frame bytes).
    pub frames: Vec<Vec<u8>>,
}

/// Errors that can occur when loading/saving archives.
#[derive(Debug)]
pub enum ArchiveError {
    /// I/O error.
    Io(io::Error),
    /// Invalid magic bytes (not a .mjsim file).
    InvalidMagic,
    /// Unsupported file version.
    UnsupportedVersion(u16),
    /// Invalid grid type in header.
    InvalidGridType,
    /// Frame data doesn't match expected size.
    FrameSizeMismatch { expected: usize, got: usize },
    /// Palette too long (max 256 chars).
    PaletteTooLong,
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::InvalidMagic => write!(f, "Invalid file format (bad magic bytes)"),
            Self::UnsupportedVersion(v) => write!(f, "Unsupported file version: {}", v),
            Self::InvalidGridType => write!(f, "Invalid grid type in header"),
            Self::FrameSizeMismatch { expected, got } => {
                write!(f, "Frame size mismatch: expected {}, got {}", expected, got)
            }
            Self::PaletteTooLong => write!(f, "Palette too long (max 256 characters)"),
        }
    }
}

impl std::error::Error for ArchiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ArchiveError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl SimulationArchive {
    /// Create a new archive from recorded data.
    pub fn new(grid_type: GridType, palette: String, frames: Vec<Vec<u8>>) -> Self {
        Self {
            grid_type,
            palette,
            frames,
        }
    }

    /// Number of recorded frames.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Bytes per frame.
    pub fn bytes_per_frame(&self) -> usize {
        self.grid_type.bytes_per_frame()
    }

    /// Total size of all frame data.
    pub fn total_frame_bytes(&self) -> usize {
        self.frames.len() * self.bytes_per_frame()
    }

    /// Get a specific frame by index.
    pub fn frame(&self, index: usize) -> Option<&[u8]> {
        self.frames.get(index).map(|v| v.as_slice())
    }

    /// Save the archive to a file.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ArchiveError> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.write_to(&mut writer)
    }

    /// Load an archive from a file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ArchiveError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Self::read_from(&mut reader)
    }

    /// Write the archive to any writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), ArchiveError> {
        // Validate palette length
        if self.palette.len() > 256 {
            return Err(ArchiveError::PaletteTooLong);
        }

        // Build header
        let mut header = [0u8; HEADER_SIZE];
        let mut offset = 0;

        // Magic
        header[offset..offset + 4].copy_from_slice(&MAGIC);
        offset += 4;

        // Version
        header[offset..offset + 2].copy_from_slice(&VERSION.to_le_bytes());
        offset += 2;

        // Grid type (16 bytes)
        header[offset..offset + 16].copy_from_slice(&self.grid_type.to_bytes());
        offset += 16;

        // Palette length
        header[offset] = self.palette.len() as u8;
        offset += 1;

        // Palette (256 bytes, null-padded)
        let palette_bytes = self.palette.as_bytes();
        header[offset..offset + palette_bytes.len()].copy_from_slice(palette_bytes);
        offset += 256;

        // Frame count
        let frame_count = self.frames.len() as u32;
        header[offset..offset + 4].copy_from_slice(&frame_count.to_le_bytes());
        offset += 4;

        // Bytes per frame
        let bytes_per_frame = self.bytes_per_frame() as u32;
        header[offset..offset + 4].copy_from_slice(&bytes_per_frame.to_le_bytes());
        // offset += 4;

        // Reserved bytes already zeroed

        // Write header
        writer.write_all(&header)?;

        // Write frames
        for frame in &self.frames {
            writer.write_all(frame)?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Read an archive from any reader.
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, ArchiveError> {
        // Read header
        let mut header = [0u8; HEADER_SIZE];
        reader.read_exact(&mut header)?;

        let mut offset = 0;

        // Verify magic
        if &header[offset..offset + 4] != &MAGIC {
            return Err(ArchiveError::InvalidMagic);
        }
        offset += 4;

        // Check version
        let version = u16::from_le_bytes([header[offset], header[offset + 1]]);
        if version != VERSION {
            return Err(ArchiveError::UnsupportedVersion(version));
        }
        offset += 2;

        // Read grid type
        let mut grid_type_bytes = [0u8; 16];
        grid_type_bytes.copy_from_slice(&header[offset..offset + 16]);
        let grid_type =
            GridType::from_bytes(&grid_type_bytes).ok_or(ArchiveError::InvalidGridType)?;
        offset += 16;

        // Read palette
        let palette_len = header[offset] as usize;
        offset += 1;

        let palette = String::from_utf8_lossy(&header[offset..offset + palette_len]).to_string();
        offset += 256;

        // Read frame count
        let frame_count = u32::from_le_bytes([
            header[offset],
            header[offset + 1],
            header[offset + 2],
            header[offset + 3],
        ]) as usize;
        offset += 4;

        // Read bytes per frame
        let bytes_per_frame = u32::from_le_bytes([
            header[offset],
            header[offset + 1],
            header[offset + 2],
            header[offset + 3],
        ]) as usize;

        // Verify bytes_per_frame matches grid type
        let expected_bpf = grid_type.bytes_per_frame();
        if bytes_per_frame != expected_bpf {
            return Err(ArchiveError::FrameSizeMismatch {
                expected: expected_bpf,
                got: bytes_per_frame,
            });
        }

        // Read frames
        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let mut frame = vec![0u8; bytes_per_frame];
            reader.read_exact(&mut frame)?;
            frames.push(frame);
        }

        Ok(Self {
            grid_type,
            palette,
            frames,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_archive_roundtrip() {
        let grid_type = GridType::Cartesian2D {
            width: 10,
            height: 10,
        };
        let palette = "BWR".to_string();

        // Create some test frames
        let frame1 = vec![0u8; 100];
        let mut frame2 = vec![0u8; 100];
        frame2[50] = 1;
        frame2[51] = 2;

        let archive = SimulationArchive::new(grid_type, palette.clone(), vec![frame1, frame2]);

        // Write to buffer
        let mut buffer = Vec::new();
        archive.write_to(&mut buffer).unwrap();

        // Read back
        let mut cursor = Cursor::new(buffer);
        let loaded = SimulationArchive::read_from(&mut cursor).unwrap();

        assert_eq!(loaded.grid_type, grid_type);
        assert_eq!(loaded.palette, palette);
        assert_eq!(loaded.frame_count(), 2);
        assert_eq!(loaded.frame(0).unwrap()[50], 0);
        assert_eq!(loaded.frame(1).unwrap()[50], 1);
        assert_eq!(loaded.frame(1).unwrap()[51], 2);
    }

    #[test]
    fn test_archive_polar_roundtrip() {
        let grid_type = GridType::Polar2D {
            r_min: 64,
            r_depth: 32,
            theta_divisions: 402,
        };
        let palette = "XRGMYC".to_string();
        let frame_size = grid_type.bytes_per_frame();

        let frame = vec![0u8; frame_size];
        let archive = SimulationArchive::new(grid_type, palette.clone(), vec![frame]);

        let mut buffer = Vec::new();
        archive.write_to(&mut buffer).unwrap();

        let mut cursor = Cursor::new(buffer);
        let loaded = SimulationArchive::read_from(&mut cursor).unwrap();

        assert_eq!(loaded.grid_type, grid_type);
        assert_eq!(loaded.palette, palette);
    }

    #[test]
    fn test_invalid_magic() {
        let bad_data = vec![0u8; HEADER_SIZE];
        let mut cursor = Cursor::new(bad_data);
        let result = SimulationArchive::read_from(&mut cursor);
        assert!(matches!(result, Err(ArchiveError::InvalidMagic)));
    }
}
