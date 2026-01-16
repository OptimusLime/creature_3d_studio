//! Grid type definitions for simulation recording.
//!
//! Supports multiple grid types: Cartesian 2D/3D and Polar 2D/3D.

use std::io::{Read, Write};

/// Type identifier for binary serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GridTypeId {
    Cartesian2D = 0,
    Cartesian3D = 1,
    Polar2D = 2,
    Polar3D = 3,
}

impl GridTypeId {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Cartesian2D),
            1 => Some(Self::Cartesian3D),
            2 => Some(Self::Polar2D),
            3 => Some(Self::Polar3D),
            _ => None,
        }
    }
}

/// Grid type with dimensions.
///
/// This enum describes the geometry and dimensions of any MarkovJunior grid,
/// enabling generic recording and playback of simulations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridType {
    /// 2D Cartesian grid (standard rectangular)
    Cartesian2D { width: u32, height: u32 },
    /// 3D Cartesian grid (standard voxel)
    Cartesian3D { width: u32, height: u32, depth: u32 },
    /// 2D Polar grid (ring/disc)
    Polar2D {
        r_min: u32,
        r_depth: u16,
        theta_divisions: u16,
    },
    /// 3D Polar/Spherical grid (future)
    Polar3D {
        r_min: u32,
        r_depth: u16,
        theta_divisions: u16,
        phi_divisions: u16,
    },
}

impl GridType {
    /// Get the type identifier for binary serialization.
    pub fn type_id(&self) -> GridTypeId {
        match self {
            Self::Cartesian2D { .. } => GridTypeId::Cartesian2D,
            Self::Cartesian3D { .. } => GridTypeId::Cartesian3D,
            Self::Polar2D { .. } => GridTypeId::Polar2D,
            Self::Polar3D { .. } => GridTypeId::Polar3D,
        }
    }

    /// Calculate the number of cells in the grid.
    pub fn total_cells(&self) -> usize {
        match self {
            Self::Cartesian2D { width, height } => (*width as usize) * (*height as usize),
            Self::Cartesian3D {
                width,
                height,
                depth,
            } => (*width as usize) * (*height as usize) * (*depth as usize),
            Self::Polar2D {
                r_depth,
                theta_divisions,
                ..
            } => (*r_depth as usize) * (*theta_divisions as usize),
            Self::Polar3D {
                r_depth,
                theta_divisions,
                phi_divisions,
                ..
            } => (*r_depth as usize) * (*theta_divisions as usize) * (*phi_divisions as usize),
        }
    }

    /// Number of bytes needed to store one frame of this grid type.
    /// Currently 1 byte per cell (u8 values).
    pub fn bytes_per_frame(&self) -> usize {
        self.total_cells()
    }

    /// Whether this is a 2D grid type (renderable to image).
    pub fn is_2d(&self) -> bool {
        matches!(self, Self::Cartesian2D { .. } | Self::Polar2D { .. })
    }

    /// Whether this is a 3D grid type (renderable to voxels/mesh).
    pub fn is_3d(&self) -> bool {
        matches!(self, Self::Cartesian3D { .. } | Self::Polar3D { .. })
    }

    /// Serialize grid type to bytes (16 bytes fixed).
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0] = self.type_id() as u8;

        match self {
            Self::Cartesian2D { width, height } => {
                bytes[1..5].copy_from_slice(&width.to_le_bytes());
                bytes[5..9].copy_from_slice(&height.to_le_bytes());
            }
            Self::Cartesian3D {
                width,
                height,
                depth,
            } => {
                bytes[1..5].copy_from_slice(&width.to_le_bytes());
                bytes[5..9].copy_from_slice(&height.to_le_bytes());
                bytes[9..13].copy_from_slice(&depth.to_le_bytes());
            }
            Self::Polar2D {
                r_min,
                r_depth,
                theta_divisions,
            } => {
                bytes[1..5].copy_from_slice(&r_min.to_le_bytes());
                bytes[5..7].copy_from_slice(&r_depth.to_le_bytes());
                bytes[7..9].copy_from_slice(&theta_divisions.to_le_bytes());
            }
            Self::Polar3D {
                r_min,
                r_depth,
                theta_divisions,
                phi_divisions,
            } => {
                bytes[1..5].copy_from_slice(&r_min.to_le_bytes());
                bytes[5..7].copy_from_slice(&r_depth.to_le_bytes());
                bytes[7..9].copy_from_slice(&theta_divisions.to_le_bytes());
                bytes[9..11].copy_from_slice(&phi_divisions.to_le_bytes());
            }
        }

        bytes
    }

    /// Deserialize grid type from bytes.
    pub fn from_bytes(bytes: &[u8; 16]) -> Option<Self> {
        let type_id = GridTypeId::from_u8(bytes[0])?;

        match type_id {
            GridTypeId::Cartesian2D => {
                let width = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                let height = u32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
                Some(Self::Cartesian2D { width, height })
            }
            GridTypeId::Cartesian3D => {
                let width = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                let height = u32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
                let depth = u32::from_le_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]);
                Some(Self::Cartesian3D {
                    width,
                    height,
                    depth,
                })
            }
            GridTypeId::Polar2D => {
                let r_min = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                let r_depth = u16::from_le_bytes([bytes[5], bytes[6]]);
                let theta_divisions = u16::from_le_bytes([bytes[7], bytes[8]]);
                Some(Self::Polar2D {
                    r_min,
                    r_depth,
                    theta_divisions,
                })
            }
            GridTypeId::Polar3D => {
                let r_min = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                let r_depth = u16::from_le_bytes([bytes[5], bytes[6]]);
                let theta_divisions = u16::from_le_bytes([bytes[7], bytes[8]]);
                let phi_divisions = u16::from_le_bytes([bytes[9], bytes[10]]);
                Some(Self::Polar3D {
                    r_min,
                    r_depth,
                    theta_divisions,
                    phi_divisions,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cartesian_2d_roundtrip() {
        let grid_type = GridType::Cartesian2D {
            width: 100,
            height: 200,
        };
        let bytes = grid_type.to_bytes();
        let recovered = GridType::from_bytes(&bytes).unwrap();
        assert_eq!(grid_type, recovered);
        assert_eq!(grid_type.total_cells(), 20000);
    }

    #[test]
    fn test_cartesian_3d_roundtrip() {
        let grid_type = GridType::Cartesian3D {
            width: 10,
            height: 20,
            depth: 30,
        };
        let bytes = grid_type.to_bytes();
        let recovered = GridType::from_bytes(&bytes).unwrap();
        assert_eq!(grid_type, recovered);
        assert_eq!(grid_type.total_cells(), 6000);
    }

    #[test]
    fn test_polar_2d_roundtrip() {
        let grid_type = GridType::Polar2D {
            r_min: 64,
            r_depth: 32,
            theta_divisions: 402,
        };
        let bytes = grid_type.to_bytes();
        let recovered = GridType::from_bytes(&bytes).unwrap();
        assert_eq!(grid_type, recovered);
        assert_eq!(grid_type.total_cells(), 32 * 402);
    }

    #[test]
    fn test_is_2d_3d() {
        assert!(GridType::Cartesian2D {
            width: 1,
            height: 1
        }
        .is_2d());
        assert!(GridType::Polar2D {
            r_min: 1,
            r_depth: 1,
            theta_divisions: 1
        }
        .is_2d());
        assert!(GridType::Cartesian3D {
            width: 1,
            height: 1,
            depth: 1
        }
        .is_3d());
        assert!(GridType::Polar3D {
            r_min: 1,
            r_depth: 1,
            theta_divisions: 1,
            phi_divisions: 1
        }
        .is_3d());
    }
}
