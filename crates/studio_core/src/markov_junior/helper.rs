//! Helper utilities for MarkovJunior.
//!
//! Provides PNG/image loading and VOX file loading for file-based rules.
//!
//! C# Reference: Helper.cs, Graphics.cs (LoadBitmap), VoxHelper.cs (LoadVox)

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Error type for resource loading.
#[derive(Debug, Clone)]
pub enum ResourceError {
    /// File not found
    FileNotFound(String),
    /// Image loading/decoding error
    ImageError(String),
    /// Legend doesn't cover all colors in image
    LegendTooShort {
        colors_found: usize,
        legend_len: usize,
    },
    /// Image has odd width (can't split into input/output)
    OddWidth(u32),
    /// VOX file format error
    VoxError(String),
    /// IO error
    IoError(String),
}

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceError::FileNotFound(path) => write!(f, "file not found: {}", path),
            ResourceError::ImageError(msg) => write!(f, "image error: {}", msg),
            ResourceError::LegendTooShort {
                colors_found,
                legend_len,
            } => write!(
                f,
                "legend has {} chars but image has {} unique colors",
                legend_len, colors_found
            ),
            ResourceError::OddWidth(w) => write!(f, "image has odd width {} (must be even)", w),
            ResourceError::VoxError(msg) => write!(f, "VOX error: {}", msg),
            ResourceError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl std::error::Error for ResourceError {}

/// Load a PNG/bitmap file and return raw pixel data as RGBA integers.
///
/// Returns (pixels, width, height, depth) where depth is always 1 for 2D images.
/// Each pixel is stored as a packed RGBA integer.
///
/// C# Reference: Graphics.cs LoadBitmap() lines 11-22
pub fn load_bitmap(path: &Path) -> Result<(Vec<i32>, usize, usize, usize), ResourceError> {
    let img = image::open(path).map_err(|e| {
        if e.to_string().contains("No such file") || e.to_string().contains("not found") {
            ResourceError::FileNotFound(path.display().to_string())
        } else {
            ResourceError::ImageError(e.to_string())
        }
    })?;

    let rgba = img.to_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;

    // Convert to packed i32 RGBA values (same format as C#)
    // C# uses Bgra32, we'll use the same packing: 0xAARRGGBB
    let pixels: Vec<i32> = rgba
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            ((a as i32) << 24) | ((r as i32) << 16) | ((g as i32) << 8) | (b as i32)
        })
        .collect();

    Ok((pixels, width, height, 1))
}

/// Convert pixel data to ordinals (indices into unique colors list).
///
/// Returns (ordinals, unique_count) where ordinals[i] is the index of pixels[i]
/// in the unique colors list.
///
/// C# Reference: Helper.cs Ords() lines 8-24
pub fn ords(data: &[i32]) -> (Vec<u8>, usize) {
    let mut result = vec![0u8; data.len()];
    let mut uniques: Vec<i32> = Vec::new();

    for (i, &d) in data.iter().enumerate() {
        let ord = uniques.iter().position(|&u| u == d);
        let ord = match ord {
            Some(idx) => idx,
            None => {
                let idx = uniques.len();
                uniques.push(d);
                idx
            }
        };
        result[i] = ord as u8;
    }

    (result, uniques.len())
}

/// Load a resource (PNG for 2D) and convert to character array using legend.
///
/// The legend string maps ordinal indices to characters.
/// For example, legend="BW" means:
/// - First unique color -> 'B'
/// - Second unique color -> 'W'
///
/// C# Reference: Rule.cs LoadResource() lines 116-136
pub fn load_resource(
    path: &Path,
    legend: &str,
    _is_2d: bool,
) -> Result<(Vec<char>, usize, usize, usize), ResourceError> {
    // Load the image
    let (data, mx, my, mz) = load_bitmap(path)?;

    // Convert to ordinals
    let (ords, amount) = ords(&data);

    // Check legend covers all colors
    if amount > legend.len() {
        return Err(ResourceError::LegendTooShort {
            colors_found: amount,
            legend_len: legend.len(),
        });
    }

    // Map ordinals to characters via legend
    let legend_chars: Vec<char> = legend.chars().collect();
    let chars: Vec<char> = ords.iter().map(|&o| legend_chars[o as usize]).collect();

    Ok((chars, mx, my, mz))
}

// ============================================================================
// VOX File Loading (MagicaVoxel format)
// ============================================================================

/// Load a MagicaVoxel .vox file and return voxel data.
///
/// Returns (voxels, width, height, depth) where voxels[x + y*MX + z*MX*MY] is
/// the palette index at that position, or -1 if empty.
///
/// C# Reference: VoxHelper.cs LoadVox() lines 10-70
pub fn load_vox(path: &Path) -> Result<(Vec<i32>, usize, usize, usize), ResourceError> {
    let file = File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ResourceError::FileNotFound(path.display().to_string())
        } else {
            ResourceError::IoError(e.to_string())
        }
    })?;

    let mut reader = BufReader::new(file);

    // Read magic number "VOX "
    let mut magic = [0u8; 4];
    reader
        .read_exact(&mut magic)
        .map_err(|e| ResourceError::VoxError(format!("failed to read magic: {}", e)))?;

    if &magic != b"VOX " {
        return Err(ResourceError::VoxError(format!(
            "invalid magic: {:?}",
            magic
        )));
    }

    // Read version
    let mut version_bytes = [0u8; 4];
    reader
        .read_exact(&mut version_bytes)
        .map_err(|e| ResourceError::VoxError(format!("failed to read version: {}", e)))?;
    let _version = i32::from_le_bytes(version_bytes);

    let mut result: Option<Vec<i32>> = None;
    let mut mx: i32 = -1;
    let mut my: i32 = -1;
    let mut mz: i32 = -1;

    // Read chunks until EOF
    loop {
        // Try to read chunk ID (4 bytes)
        let mut chunk_id = [0u8; 4];
        match reader.read_exact(&mut chunk_id) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => {
                return Err(ResourceError::VoxError(format!(
                    "failed to read chunk ID: {}",
                    e
                )))
            }
        }

        let chunk_name = String::from_utf8_lossy(&chunk_id);

        // Read chunk content size and children size
        let mut size_bytes = [0u8; 4];
        reader
            .read_exact(&mut size_bytes)
            .map_err(|e| ResourceError::VoxError(format!("failed to read chunk size: {}", e)))?;
        let chunk_size = i32::from_le_bytes(size_bytes) as usize;

        let mut children_bytes = [0u8; 4];
        reader
            .read_exact(&mut children_bytes)
            .map_err(|e| ResourceError::VoxError(format!("failed to read children size: {}", e)))?;
        let _children_size = i32::from_le_bytes(children_bytes);

        match chunk_name.as_ref() {
            "SIZE" => {
                // Read dimensions
                let mut x_bytes = [0u8; 4];
                let mut y_bytes = [0u8; 4];
                let mut z_bytes = [0u8; 4];
                reader.read_exact(&mut x_bytes).map_err(|e| {
                    ResourceError::VoxError(format!("failed to read SIZE x: {}", e))
                })?;
                reader.read_exact(&mut y_bytes).map_err(|e| {
                    ResourceError::VoxError(format!("failed to read SIZE y: {}", e))
                })?;
                reader.read_exact(&mut z_bytes).map_err(|e| {
                    ResourceError::VoxError(format!("failed to read SIZE z: {}", e))
                })?;

                mx = i32::from_le_bytes(x_bytes);
                my = i32::from_le_bytes(y_bytes);
                mz = i32::from_le_bytes(z_bytes);

                // Skip remaining bytes in chunk if any
                let read_bytes = 12;
                if chunk_size > read_bytes {
                    let mut skip = vec![0u8; chunk_size - read_bytes];
                    reader.read_exact(&mut skip).ok();
                }
            }
            "XYZI" => {
                if mx <= 0 || my <= 0 || mz <= 0 {
                    return Err(ResourceError::VoxError(
                        "XYZI chunk before SIZE chunk".to_string(),
                    ));
                }

                // Initialize result array
                let total = (mx * my * mz) as usize;
                let mut voxels = vec![-1i32; total];

                // Read number of voxels
                let mut num_bytes = [0u8; 4];
                reader.read_exact(&mut num_bytes).map_err(|e| {
                    ResourceError::VoxError(format!("failed to read voxel count: {}", e))
                })?;
                let num_voxels = i32::from_le_bytes(num_bytes);

                // Read each voxel
                for _ in 0..num_voxels {
                    let mut voxel_data = [0u8; 4];
                    reader.read_exact(&mut voxel_data).map_err(|e| {
                        ResourceError::VoxError(format!("failed to read voxel: {}", e))
                    })?;

                    let x = voxel_data[0] as i32;
                    let y = voxel_data[1] as i32;
                    let z = voxel_data[2] as i32;
                    let color = voxel_data[3] as i32;

                    if x < mx && y < my && z < mz {
                        let idx = (x + y * mx + z * mx * my) as usize;
                        voxels[idx] = color;
                    }
                }

                result = Some(voxels);
            }
            "MAIN" => {
                // MAIN chunk has no content, just children
                // Skip content (should be 0)
                if chunk_size > 0 {
                    let mut skip = vec![0u8; chunk_size];
                    reader.read_exact(&mut skip).ok();
                }
            }
            _ => {
                // Skip unknown chunks
                if chunk_size > 0 {
                    let mut skip = vec![0u8; chunk_size];
                    reader.read_exact(&mut skip).ok();
                }
            }
        }
    }

    match result {
        Some(voxels) => Ok((voxels, mx as usize, my as usize, mz as usize)),
        None => Err(ResourceError::VoxError("no XYZI chunk found".to_string())),
    }
}

/// Load a VOX file and convert to ordinals (0-based indices).
///
/// Returns (ordinals, mx, my, mz, num_colors) where empty voxels are preserved as 0xff.
///
/// C# Reference: Similar to how PNG loading uses ords()
pub fn load_vox_ords(path: &Path) -> Result<(Vec<u8>, usize, usize, usize, usize), ResourceError> {
    let (voxels, mx, my, mz) = load_vox(path)?;

    // Find unique palette indices (excluding -1 for empty)
    let mut uniques: Vec<i32> = Vec::new();
    for &v in &voxels {
        if v >= 0 && !uniques.contains(&v) {
            uniques.push(v);
        }
    }
    uniques.sort();

    // Convert to ordinals
    let ords: Vec<u8> = voxels
        .iter()
        .map(|&v| {
            if v < 0 {
                0xff // Empty voxel marker
            } else {
                uniques.iter().position(|&u| u == v).unwrap_or(0) as u8
            }
        })
        .collect();

    Ok((ords, mx, my, mz, uniques.len()))
}

/// Load a 3D VOX resource and convert to characters using legend.
///
/// C# Reference: Rule.cs LoadResource() with d2=false path
pub fn load_vox_resource(
    path: &Path,
    legend: &str,
) -> Result<(Vec<char>, usize, usize, usize), ResourceError> {
    let (ords, mx, my, mz, num_colors) = load_vox_ords(path)?;

    // Check legend covers all colors
    if num_colors > legend.len() {
        return Err(ResourceError::LegendTooShort {
            colors_found: num_colors,
            legend_len: legend.len(),
        });
    }

    // Map ordinals to characters via legend
    // Empty voxels (0xff) map to a special character (first in legend, typically 'B' for black/empty)
    let legend_chars: Vec<char> = legend.chars().collect();
    let chars: Vec<char> = ords
        .iter()
        .map(|&o| {
            if o == 0xff {
                legend_chars[0] // Empty maps to first legend char
            } else {
                legend_chars[o as usize]
            }
        })
        .collect();

    Ok((chars, mx, my, mz))
}

/// Split a rule image into input and output halves.
///
/// The image is split vertically down the middle:
/// - Left half = input pattern
/// - Right half = output pattern
///
/// Returns (input_chars, output_chars, mx, my, mz) where mx is half the original width.
pub fn split_rule_image(
    chars: &[char],
    full_mx: usize,
    my: usize,
    mz: usize,
) -> Result<(Vec<char>, Vec<char>, usize, usize, usize), ResourceError> {
    if full_mx % 2 != 0 {
        return Err(ResourceError::OddWidth(full_mx as u32));
    }

    let mx = full_mx / 2;
    let mut input = vec![' '; mx * my * mz];
    let mut output = vec![' '; mx * my * mz];

    // C# Reference: Rule.cs lines 246-247
    // inRect = AH.FlatArray3D(FX / 2, FY, FZ, (x, y, z) => rect[x + y * FX + z * FX * FY]);
    // outRect = AH.FlatArray3D(FX / 2, FY, FZ, (x, y, z) => rect[x + FX / 2 + y * FX + z * FX * FY]);
    for z in 0..mz {
        for y in 0..my {
            for x in 0..mx {
                let src_idx_in = x + y * full_mx + z * full_mx * my;
                let src_idx_out = (x + mx) + y * full_mx + z * full_mx * my;
                let dst_idx = x + y * mx + z * mx * my;

                input[dst_idx] = chars[src_idx_in];
                output[dst_idx] = chars[src_idx_out];
            }
        }
    }

    Ok((input, output, mx, my, mz))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn resources_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MarkovJunior/resources")
    }

    #[test]
    fn test_ords_simple() {
        let data = vec![100, 200, 100, 300, 200];
        let (ords, count) = ords(&data);

        assert_eq!(count, 3); // 3 unique values
        assert_eq!(ords[0], 0); // 100 -> first unique
        assert_eq!(ords[1], 1); // 200 -> second unique
        assert_eq!(ords[2], 0); // 100 -> first unique again
        assert_eq!(ords[3], 2); // 300 -> third unique
        assert_eq!(ords[4], 1); // 200 -> second unique again
    }

    #[test]
    fn test_ords_all_same() {
        let data = vec![42, 42, 42, 42];
        let (ords, count) = ords(&data);

        assert_eq!(count, 1);
        assert!(ords.iter().all(|&o| o == 0));
    }

    #[test]
    fn test_load_bitmap_basic_dijkstra_room() {
        let path = resources_path().join("rules/BasicDijkstraRoom.png");
        if !path.exists() {
            return; // Skip if file doesn't exist
        }

        let result = load_bitmap(&path);
        assert!(result.is_ok(), "Failed to load: {:?}", result);

        let (pixels, mx, my, mz) = result.unwrap();
        assert_eq!(mx, 10); // 10 x 6 image
        assert_eq!(my, 6);
        assert_eq!(mz, 1);
        assert_eq!(pixels.len(), 60);
    }

    #[test]
    fn test_load_resource_with_legend() {
        let path = resources_path().join("rules/BasicDijkstraRoom.png");
        if !path.exists() {
            return;
        }

        let result = load_resource(&path, "BW", true);
        assert!(result.is_ok(), "Failed to load resource: {:?}", result);

        let (chars, mx, my, mz) = result.unwrap();
        assert_eq!(mx, 10);
        assert_eq!(my, 6);
        assert_eq!(mz, 1);
        assert_eq!(chars.len(), 60);

        // All chars should be either 'B' or 'W'
        assert!(chars.iter().all(|&c| c == 'B' || c == 'W'));
    }

    #[test]
    fn test_split_rule_image() {
        // Create a simple 4x2 pattern: ABCD / EFGH
        let chars = vec!['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'];
        let (input, output, mx, my, mz) = split_rule_image(&chars, 4, 2, 1).unwrap();

        assert_eq!(mx, 2);
        assert_eq!(my, 2);
        assert_eq!(mz, 1);

        // Input should be left half: AB / EF
        assert_eq!(input, vec!['A', 'B', 'E', 'F']);
        // Output should be right half: CD / GH
        assert_eq!(output, vec!['C', 'D', 'G', 'H']);
    }

    #[test]
    fn test_split_rule_image_odd_width_error() {
        let chars = vec!['A', 'B', 'C'];
        let result = split_rule_image(&chars, 3, 1, 1);
        assert!(matches!(result, Err(ResourceError::OddWidth(3))));
    }

    #[test]
    fn test_load_and_split_basic_dijkstra_room() {
        let path = resources_path().join("rules/BasicDijkstraRoom.png");
        if !path.exists() {
            return;
        }

        let (chars, full_mx, my, mz) = load_resource(&path, "BW", true).unwrap();
        let (input, output, mx, _, _) = split_rule_image(&chars, full_mx, my, mz).unwrap();

        // BasicDijkstraRoom.png is 10x6, so each half is 5x6
        assert_eq!(mx, 5);
        assert_eq!(input.len(), 30);
        assert_eq!(output.len(), 30);
    }

    // ====================================================================
    // VOX Loading Tests
    // ====================================================================

    fn tilesets_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MarkovJunior/resources/tilesets")
    }

    #[test]
    fn test_load_vox_escher_empty() {
        let path = tilesets_path().join("EscherSurface/Empty.vox");
        if !path.exists() {
            println!("Skipping test: {:?} not found", path);
            return;
        }

        let result = load_vox(&path);
        assert!(result.is_ok(), "Failed to load VOX: {:?}", result);

        let (voxels, mx, my, mz) = result.unwrap();

        // EscherSurface tiles are typically small (3x3x3 or similar)
        assert!(mx > 0, "MX should be positive");
        assert!(my > 0, "MY should be positive");
        assert!(mz > 0, "MZ should be positive");
        assert_eq!(voxels.len(), mx * my * mz);
    }

    #[test]
    fn test_load_vox_escher_line() {
        let path = tilesets_path().join("EscherSurface/Line.vox");
        if !path.exists() {
            println!("Skipping test: {:?} not found", path);
            return;
        }

        let result = load_vox(&path);
        assert!(result.is_ok(), "Failed to load VOX: {:?}", result);

        let (voxels, mx, my, mz) = result.unwrap();

        // Should have some non-empty voxels
        let non_empty = voxels.iter().filter(|&&v| v >= 0).count();
        assert!(non_empty > 0, "Line tile should have some voxels");

        println!(
            "Loaded Line.vox: {}x{}x{}, {} voxels",
            mx, my, mz, non_empty
        );
    }

    #[test]
    fn test_load_vox_ords() {
        let path = tilesets_path().join("EscherSurface/Line.vox");
        if !path.exists() {
            println!("Skipping test: {:?} not found", path);
            return;
        }

        let result = load_vox_ords(&path);
        assert!(result.is_ok(), "Failed to load VOX: {:?}", result);

        let (ords, mx, my, mz, num_colors) = result.unwrap();
        assert_eq!(ords.len(), mx * my * mz);

        // All non-0xff values should be valid ordinals
        for &o in &ords {
            if o != 0xff {
                assert!(
                    (o as usize) < num_colors,
                    "Ordinal {} out of range for {} colors",
                    o,
                    num_colors
                );
            }
        }
    }

    #[test]
    fn test_load_vox_nonexistent() {
        let path = PathBuf::from("/nonexistent/file.vox");
        let result = load_vox(&path);
        assert!(matches!(result, Err(ResourceError::FileNotFound(_))));
    }

    #[test]
    fn test_load_vox_invalid_format() {
        // Create a temp file with invalid content
        use std::io::Write;
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("invalid_test.vox");

        {
            let mut file = File::create(&temp_path).unwrap();
            file.write_all(b"NOT A VOX FILE").unwrap();
        }

        let result = load_vox(&temp_path);
        assert!(
            matches!(result, Err(ResourceError::VoxError(_))),
            "Expected VoxError, got {:?}",
            result
        );

        // Clean up
        std::fs::remove_file(&temp_path).ok();
    }
}
