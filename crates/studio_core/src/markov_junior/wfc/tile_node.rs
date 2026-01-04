//! Tile model WFC node.
//!
//! TileNode uses pre-defined tilesets with neighbor constraints
//! to generate valid tile arrangements.
//!
//! C# Reference: TileModel.cs (lines 1-332)

use super::wfc_node::{WfcNode, WfcState};

use crate::markov_junior::helper::{load_vox, load_vox_ords};
use crate::markov_junior::node::{ExecutionContext, Node};
use crate::markov_junior::MjGrid;
use quick_xml::events::Event;
use quick_xml::Reader;
use rand::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// Tile model WFC node.
///
/// Uses pre-defined tilesets with explicit neighbor constraints
/// to generate valid tile arrangements.
pub struct TileNode {
    /// Base WFC node with shared algorithms
    pub wfc: WfcNode,

    /// Tile data: tiledata[tile_index] = flat array of color indices (S * S * SZ)
    pub tiledata: Vec<Vec<u8>>,

    /// Tile size (assumed square: S x S x SZ)
    pub s: usize,

    /// Tile depth (Z dimension)
    pub sz: usize,

    /// Overlap between tiles (0 for no overlap)
    pub overlap: usize,

    /// Z-axis overlap
    pub overlapz: usize,
}

impl TileNode {
    /// Create a TileNode from a tileset XML file.
    ///
    /// # Arguments
    /// * `tileset_path` - Path to the tileset XML file
    /// * `tiles_name` - Name of tiles folder (may differ from tileset name)
    /// * `periodic` - Whether output wraps
    /// * `shannon` - Whether to use Shannon entropy
    /// * `tries` - Number of seed attempts
    /// * `overlap` - Tile overlap (0 for no overlap)
    /// * `overlapz` - Z-axis overlap
    /// * `newgrid` - Output grid
    /// * `input_grid` - Input grid for initial constraints
    /// * `rules` - Map from input values to allowed tile names
    ///
    /// C# Reference: TileNode.Load() (lines 15-287)
    #[allow(clippy::too_many_arguments)]
    pub fn from_tileset(
        tileset_path: &Path,
        tiles_name: &str,
        periodic: bool,
        shannon: bool,
        tries: usize,
        overlap: usize,
        overlapz: usize,
        newgrid: MjGrid,
        input_grid: &MjGrid,
        rules: &[(u8, Vec<String>)], // (input_value, allowed_tile_names)
        full_symmetry: bool,
    ) -> Result<Self, String> {
        // Load tileset XML
        let xml = std::fs::read_to_string(tileset_path)
            .map_err(|e| format!("Failed to read tileset: {}", e))?;

        // Get directory for tile VOX files
        let tileset_dir = tileset_path.parent().unwrap_or(Path::new("."));

        // Parse tileset
        let (tile_info, neighbors) = parse_tileset_xml(&xml)?;

        if tile_info.is_empty() {
            return Err("No tiles found in tileset".to_string());
        }

        // Determine tile size from first tile
        let first_tile_path = tileset_dir
            .join(tiles_name)
            .join(&format!("{}.vox", tile_info[0].0));
        let (s, sz) = get_tile_size(&first_tile_path)?;

        // Load all tiles with symmetry variants
        let mut tiledata: Vec<Vec<u8>> = Vec::new();
        let mut weights: Vec<f64> = Vec::new();
        let mut tile_positions: HashMap<String, Vec<usize>> = HashMap::new();
        let mut uniques: Vec<i32> = Vec::new();

        for (tile_name, weight) in &tile_info {
            let tile_path = tileset_dir
                .join(tiles_name)
                .join(&format!("{}.vox", tile_name));
            let (flat_tile, _) = load_vox_tile(&tile_path, &mut uniques)?;

            // Generate symmetry variants
            let variants = if full_symmetry {
                cube_symmetries(&flat_tile, s, sz)
            } else {
                square_symmetries_3d(&flat_tile, s, sz)
            };

            let start_idx = tiledata.len();
            let mut positions = Vec::new();

            for variant in variants {
                positions.push(tiledata.len());
                tiledata.push(variant);
                weights.push(*weight);
            }

            tile_positions.insert(tile_name.clone(), positions);
        }

        let num_patterns = tiledata.len();
        if num_patterns == 0 {
            return Err("No tile variants generated".to_string());
        }

        // Build propagator from neighbor constraints
        let propagator =
            build_tile_propagator(&tiledata, &neighbors, &tile_positions, s, sz, full_symmetry)?;

        // Calculate output grid dimensions
        let mx = input_grid.mx;
        let my = input_grid.my;
        let mz = input_grid.mz;
        let wave_length = mx * my * mz;
        let num_directions = if mz == 1 { 4 } else { 6 };

        // Build map from input values to allowed patterns
        let map = build_tile_map(input_grid, &tile_positions, rules, num_patterns);

        let wfc = WfcNode::new(
            wave_length,
            num_patterns,
            num_directions,
            propagator,
            weights,
            newgrid,
            map,
            s,
            periodic,
            shannon,
            tries,
            mx,
            my,
            mz,
        );

        Ok(Self {
            wfc,
            tiledata,
            s,
            sz,
            overlap,
            overlapz,
        })
    }

    /// Update the output grid state from the wave.
    ///
    /// C# Reference: TileNode.UpdateState() (lines 290-330)
    pub fn update_state(&self, grid: &mut MjGrid) {
        let input_mx = self.wfc.mx;
        let input_my = self.wfc.my;
        let input_mz = self.wfc.mz;
        let s = self.s;
        let sz = self.sz;
        let overlap = self.overlap;
        let overlapz = self.overlapz;
        let num_colors = grid.c as usize;

        let output_mx = grid.mx;
        let output_my = grid.my;

        let mut rng = rand::thread_rng();

        for z in 0..input_mz {
            for y in 0..input_my {
                for x in 0..input_mx {
                    let wave_idx = x + y * input_mx + z * input_mx * input_my;

                    // Vote for each sub-cell of the tile
                    let mut votes: Vec<Vec<i32>> = vec![vec![0; num_colors]; s * s * sz];

                    for t in 0..self.wfc.wave.p {
                        if self.wfc.wave.get_data(wave_idx, t) {
                            let tile = &self.tiledata[t];
                            for dz in 0..sz {
                                for dy in 0..s {
                                    for dx in 0..s {
                                        let di = dx + dy * s + dz * s * s;
                                        votes[di][tile[di] as usize] += 1;
                                    }
                                }
                            }
                        }
                    }

                    // Assign most-voted color to each output cell
                    for dz in 0..sz {
                        for dy in 0..s {
                            for dx in 0..s {
                                let v = &votes[dx + dy * s + dz * s * s];
                                let mut max_vote = -1.0;
                                let mut argmax: u8 = 0xff;

                                for (c, &vote) in v.iter().enumerate() {
                                    let value = vote as f64 + 0.1 * rng.gen::<f64>();
                                    if value > max_vote {
                                        argmax = c as u8;
                                        max_vote = value;
                                    }
                                }

                                let sx = x * (s - overlap) + dx;
                                let sy = y * (s - overlap) + dy;
                                let sz_coord = z * (sz - overlapz) + dz;

                                if sx < output_mx && sy < output_my {
                                    grid.state
                                        [sx + sy * output_mx + sz_coord * output_mx * output_my] =
                                        argmax;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Node for TileNode {
    fn reset(&mut self) {
        self.wfc.reset();
    }

    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        if self.wfc.child_index >= 0 {
            self.wfc.reset();
            return false;
        }

        if self.wfc.first_go {
            if !self.wfc.initialize(ctx.grid, ctx.random) {
                return false;
            }
            std::mem::swap(&mut self.wfc.newgrid, ctx.grid);
            return true;
        }

        if self.wfc.step() {
            if ctx.gif {
                self.update_state(ctx.grid);
            }
            true
        } else {
            // Completed or failed
            // ctx.grid is already the newgrid (swapped on first_go)
            // Don't swap back - let parent sequence continue with newgrid
            if self.wfc.state == WfcState::Completed {
                self.update_state(ctx.grid);
            }
            false
        }
    }
}

// ============================================================================
// Helper functions for tileset loading
// ============================================================================

/// Tile info from XML: (name, weight)
type TileInfo = Vec<(String, f64)>;

/// Neighbor constraint: (direction, left_tile, right_tile)
/// Direction: "left"/"right" for horizontal, "top"/"bottom" for vertical
#[derive(Debug, Clone)]
struct Neighbor {
    /// Direction type
    dir: NeighborDir,
    /// Left or bottom tile (with optional rotation prefix like "z ")
    left: String,
    /// Right or top tile
    right: String,
}

#[derive(Debug, Clone, Copy)]
enum NeighborDir {
    Horizontal, // left/right
    Vertical,   // top/bottom
}

/// Parse tileset XML to extract tiles and neighbors.
fn parse_tileset_xml(xml: &str) -> Result<(TileInfo, Vec<Neighbor>), String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut tiles = Vec::new();
    let mut neighbors = Vec::new();
    let mut in_tiles = false;
    let mut in_neighbors = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
                match name {
                    "tiles" => in_tiles = true,
                    "neighbors" => in_neighbors = true,
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
                if in_tiles && name == "tile" {
                    if let Some((tile_name, weight)) = parse_tile_element(e) {
                        tiles.push((tile_name, weight));
                    }
                } else if in_neighbors && name == "neighbor" {
                    if let Some(neighbor) = parse_neighbor_element(e) {
                        neighbors.push(neighbor);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
                match name {
                    "tiles" => in_tiles = false,
                    "neighbors" => in_neighbors = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
    }

    Ok((tiles, neighbors))
}

fn parse_tile_element(e: &quick_xml::events::BytesStart) -> Option<(String, f64)> {
    let mut name = None;
    let mut weight = 1.0;

    for attr in e.attributes().flatten() {
        let key = std::str::from_utf8(attr.key.as_ref()).ok()?;
        let value = std::str::from_utf8(&attr.value).ok()?;
        match key {
            "name" => name = Some(value.to_string()),
            "weight" => weight = value.parse().unwrap_or(1.0),
            _ => {}
        }
    }

    name.map(|n| (n, weight))
}

fn parse_neighbor_element(e: &quick_xml::events::BytesStart) -> Option<Neighbor> {
    let mut left = None;
    let mut right = None;
    let mut top = None;
    let mut bottom = None;

    for attr in e.attributes().flatten() {
        let key = std::str::from_utf8(attr.key.as_ref()).ok()?;
        let value = std::str::from_utf8(&attr.value).ok()?;
        match key {
            "left" => left = Some(value.to_string()),
            "right" => right = Some(value.to_string()),
            "top" => top = Some(value.to_string()),
            "bottom" => bottom = Some(value.to_string()),
            _ => {}
        }
    }

    if let (Some(l), Some(r)) = (left, right) {
        Some(Neighbor {
            dir: NeighborDir::Horizontal,
            left: l,
            right: r,
        })
    } else if let (Some(t), Some(b)) = (top, bottom) {
        Some(Neighbor {
            dir: NeighborDir::Vertical,
            left: b, // bottom is "left" in vertical terms
            right: t,
        })
    } else {
        None
    }
}

/// Get tile size from VOX file.
///
/// Returns (s, sz) where s is the XY dimension and sz is the Z dimension.
/// Assumes tiles are square in XY plane.
fn get_tile_size(path: &Path) -> Result<(usize, usize), String> {
    let (_, mx, my, mz) = load_vox(path).map_err(|e| e.to_string())?;

    // VOX files should have square XY dimensions for tiles
    if mx != my {
        return Err(format!(
            "Tile {} has non-square XY dimensions: {}x{}",
            path.display(),
            mx,
            my
        ));
    }

    Ok((mx, mz))
}

/// Load a VOX tile and convert to ordinal indices.
///
/// Returns (flat_data, num_colors) where flat_data[x + y*s + z*s*s] is the ordinal
/// at that position. Empty voxels are mapped to 0.
///
/// The `uniques` vector is used to track global unique palette indices across all tiles.
fn load_vox_tile(path: &Path, uniques: &mut Vec<i32>) -> Result<(Vec<u8>, usize), String> {
    let (voxels, mx, my, mz) = load_vox(path).map_err(|e| e.to_string())?;

    if mx != my {
        return Err(format!(
            "Tile {} has non-square XY dimensions: {}x{}",
            path.display(),
            mx,
            my
        ));
    }

    let s = mx;
    let sz = mz;
    let total = s * s * sz;

    // Map voxel palette indices to global ordinals
    let mut result = vec![0u8; total];

    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                let src_idx = x + y * mx + z * mx * my;
                let dst_idx = x + y * s + z * s * s;

                let v = voxels[src_idx];
                if v < 0 {
                    // Empty voxel - map to first ordinal (typically empty/background)
                    result[dst_idx] = 0;
                } else {
                    // Find or add to uniques
                    let ord = if let Some(pos) = uniques.iter().position(|&u| u == v) {
                        pos
                    } else {
                        let pos = uniques.len();
                        uniques.push(v);
                        pos
                    };
                    result[dst_idx] = ord as u8;
                }
            }
        }
    }

    Ok((result, uniques.len()))
}

/// Generate square symmetries for a 3D tile (Z-rotate and X-reflect).
fn square_symmetries_3d(tile: &[u8], s: usize, sz: usize) -> Vec<Vec<u8>> {
    let mut results = Vec::new();
    let mut current = tile.to_vec();

    // 4 rotations
    for _ in 0..4 {
        if !results.iter().any(|t| t == &current) {
            results.push(current.clone());
        }
        current = z_rotate(current, s, sz);
    }

    // Reflect and 4 more rotations
    current = x_reflect(&tile.to_vec(), s, sz);
    for _ in 0..4 {
        if !results.iter().any(|t| t == &current) {
            results.push(current.clone());
        }
        current = z_rotate(current, s, sz);
    }

    results
}

/// Rotate a tile 90 degrees around Y axis.
/// Transformation: (x, y, z) -> (sz-1-z, y, x)
fn y_rotate(tile: &[u8], s: usize, sz: usize) -> Vec<u8> {
    // After Y rotation, dimensions change: (s, s, sz) -> (sz, s, s)
    // Only works correctly when s == sz (cubic tiles)
    // For non-cubic tiles, we'd need to track dimension changes
    let new_sx = sz;
    let new_sy = s;
    let new_sz = s;

    let mut result = vec![0u8; new_sx * new_sy * new_sz];
    for z in 0..new_sz {
        for y in 0..new_sy {
            for x in 0..new_sx {
                // new[x, y, z] = old[s-1-z, y, x] (when s == sz)
                // More generally: new[x, y, z] = old[sz-1-z, y, x]
                let src_x = sz - 1 - x; // Actually this maps x -> sz-1-x in old coords
                let src_y = y;
                let src_z = x; // No wait, need to think about this more carefully

                // C# Reference (Rule.cs lines 78-80):
                // for (int z = 0; z < IMX; z++) for (int y = 0; y < IMY; y++) for (int x = 0; x < IMZ; x++)
                //     newinput[x + y * IMZ + z * IMZ * IMY] = input[IMX - 1 - z + y * IMX + x * IMX * IMY];
                // So: new[x,y,z] = old[IMX-1-z, y, x]
                // Where new dims are (IMZ, IMY, IMX) and old dims are (IMX, IMY, IMZ)

                // For our case: old is (s, s, sz), new is (sz, s, s)
                // new[x,y,z] = old[s-1-z, y, x]
                if s == sz {
                    // Cubic case - straightforward
                    let src = (s - 1 - z) + src_y * s + x * s * s;
                    let dst = x + y * new_sx + z * new_sx * new_sy;
                    result[dst] = tile[src];
                } else {
                    // Non-cubic case
                    let src = (s - 1 - z) + src_y * s + x * s * s;
                    let dst = x + y * new_sx + z * new_sx * new_sy;
                    if src < tile.len() {
                        result[dst] = tile[src];
                    }
                }
            }
        }
    }
    result
}

/// Generate full cube symmetries for a 3D tile (48 elements).
///
/// Uses the same group structure as SymmetryHelper.CubeSymmetries:
/// - a: 90° rotation around Z axis
/// - b: 90° rotation around Y axis
/// - r: reflection (X-axis mirror)
fn cube_symmetries(tile: &[u8], s: usize, sz: usize) -> Vec<Vec<u8>> {
    // For tiles where s != sz, cube symmetries don't make sense
    // Fall back to square symmetries
    if s != sz {
        return square_symmetries_3d(tile, s, sz);
    }

    let mut results: Vec<Vec<u8>> = Vec::with_capacity(48);

    // Generate all 48 variants using the group structure
    // s[0] = identity
    let s0 = tile.to_vec();
    // s[1] = r (reflection)
    let s1 = x_reflect(&s0, s, sz);
    // s[2] = a (Z rotation)
    let s2 = z_rotate(s0.clone(), s, sz);
    // s[3] = ra
    let s3 = x_reflect(&s2, s, sz);
    // s[4] = a²
    let s4 = z_rotate(s2.clone(), s, sz);
    // s[5] = ra²
    let s5 = x_reflect(&s4, s, sz);
    // s[6] = a³
    let s6 = z_rotate(s4.clone(), s, sz);
    // s[7] = ra³
    let s7 = x_reflect(&s6, s, sz);
    // s[8] = b (Y rotation)
    let s8 = y_rotate(&s0, s, sz);
    // s[9] = rb
    let s9 = x_reflect(&s8, s, sz);
    // s[10] = ba
    let s10 = y_rotate(&s2, s, sz);
    // s[11] = rba
    let s11 = x_reflect(&s10, s, sz);
    // s[12] = ba²
    let s12 = y_rotate(&s4, s, sz);
    // s[13] = rba²
    let s13 = x_reflect(&s12, s, sz);
    // s[14] = ba³
    let s14 = y_rotate(&s6, s, sz);
    // s[15] = rba³
    let s15 = x_reflect(&s14, s, sz);
    // s[16] = b²
    let s16 = y_rotate(&s8, s, sz);
    // s[17] = rb²
    let s17 = x_reflect(&s16, s, sz);
    // s[18] = b²a
    let s18 = y_rotate(&s10, s, sz);
    // s[19] = rb²a
    let s19 = x_reflect(&s18, s, sz);
    // s[20] = b²a²
    let s20 = y_rotate(&s12, s, sz);
    // s[21] = rb²a²
    let s21 = x_reflect(&s20, s, sz);
    // s[22] = b²a³
    let s22 = y_rotate(&s14, s, sz);
    // s[23] = rb²a³
    let s23 = x_reflect(&s22, s, sz);
    // s[24] = b³
    let s24 = y_rotate(&s16, s, sz);
    // s[25] = rb³
    let s25 = x_reflect(&s24, s, sz);
    // s[26] = b³a
    let s26 = y_rotate(&s18, s, sz);
    // s[27] = rb³a
    let s27 = x_reflect(&s26, s, sz);
    // s[28] = b³a²
    let s28 = y_rotate(&s20, s, sz);
    // s[29] = rb³a²
    let s29 = x_reflect(&s28, s, sz);
    // s[30] = b³a³
    let s30 = y_rotate(&s22, s, sz);
    // s[31] = rb³a³
    let s31 = x_reflect(&s30, s, sz);
    // s[32] = ab
    let s32 = z_rotate(s8.clone(), s, sz);
    // s[33] = rab
    let s33 = x_reflect(&s32, s, sz);
    // s[34] = aba
    let s34 = z_rotate(s10.clone(), s, sz);
    // s[35] = raba
    let s35 = x_reflect(&s34, s, sz);
    // s[36] = aba²
    let s36 = z_rotate(s12.clone(), s, sz);
    // s[37] = raba²
    let s37 = x_reflect(&s36, s, sz);
    // s[38] = aba³
    let s38 = z_rotate(s14.clone(), s, sz);
    // s[39] = raba³
    let s39 = x_reflect(&s38, s, sz);
    // s[40] = ab³
    let s40 = z_rotate(s24.clone(), s, sz);
    // s[41] = rab³
    let s41 = x_reflect(&s40, s, sz);
    // s[42] = ab³a
    let s42 = z_rotate(s26.clone(), s, sz);
    // s[43] = rab³a
    let s43 = x_reflect(&s42, s, sz);
    // s[44] = ab³a²
    let s44 = z_rotate(s28.clone(), s, sz);
    // s[45] = rab³a²
    let s45 = x_reflect(&s44, s, sz);
    // s[46] = ab³a³
    let s46 = z_rotate(s30.clone(), s, sz);
    // s[47] = rab³a³
    let s47 = x_reflect(&s46, s, sz);

    let all = [
        s0, s1, s2, s3, s4, s5, s6, s7, s8, s9, s10, s11, s12, s13, s14, s15, s16, s17, s18, s19,
        s20, s21, s22, s23, s24, s25, s26, s27, s28, s29, s30, s31, s32, s33, s34, s35, s36, s37,
        s38, s39, s40, s41, s42, s43, s44, s45, s46, s47,
    ];

    // Add unique variants
    for variant in all {
        if !results.iter().any(|r| r == &variant) {
            results.push(variant);
        }
    }

    results
}

/// Rotate a tile 90 degrees around Z axis.
fn z_rotate(tile: Vec<u8>, s: usize, sz: usize) -> Vec<u8> {
    let mut result = vec![0u8; s * s * sz];
    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                let src = x + y * s + z * s * s;
                let dst = (s - 1 - y) + x * s + z * s * s;
                result[dst] = tile[src];
            }
        }
    }
    result
}

/// Reflect a tile along X axis.
fn x_reflect(tile: &[u8], s: usize, sz: usize) -> Vec<u8> {
    let mut result = vec![0u8; s * s * sz];
    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                let src = x + y * s + z * s * s;
                let dst = (s - 1 - x) + y * s + z * s * s;
                result[dst] = tile[src];
            }
        }
    }
    result
}

/// Build propagator from neighbor constraints.
fn build_tile_propagator(
    tiledata: &[Vec<u8>],
    neighbors: &[Neighbor],
    tile_positions: &HashMap<String, Vec<usize>>,
    s: usize,
    sz: usize,
    full_symmetry: bool,
) -> Result<Vec<Vec<Vec<usize>>>, String> {
    let num_patterns = tiledata.len();

    // 6 directions: +X, +Y, -X, -Y, +Z, -Z
    let mut propagator: Vec<Vec<Vec<bool>>> =
        vec![vec![vec![false; num_patterns]; num_patterns]; 6];

    // Process each neighbor constraint
    for neighbor in neighbors {
        let left_name = get_tile_name(&neighbor.left);
        let right_name = get_tile_name(&neighbor.right);

        let left_positions = tile_positions.get(left_name);
        let right_positions = tile_positions.get(right_name);

        if left_positions.is_none() || right_positions.is_none() {
            continue; // Unknown tile, skip
        }

        let left_pos = left_positions.unwrap();
        let right_pos = right_positions.unwrap();

        // Apply constraint based on direction
        let (dir_idx, opp_idx) = match neighbor.dir {
            NeighborDir::Horizontal => (0, 2), // +X and -X
            NeighborDir::Vertical => (1, 3),   // +Y and -Y
        };

        // Set constraints for all variants
        for &l in left_pos {
            for &r in right_pos {
                propagator[dir_idx][l][r] = true;
                propagator[opp_idx][r][l] = true;
            }
        }
    }

    // Convert to sparse format
    let mut sparse_propagator: Vec<Vec<Vec<usize>>> = vec![Vec::new(); 6];
    for d in 0..6 {
        sparse_propagator[d] = vec![Vec::new(); num_patterns];
        for p1 in 0..num_patterns {
            for p2 in 0..num_patterns {
                if propagator[d][p1][p2] {
                    sparse_propagator[d][p1].push(p2);
                }
            }
        }
    }

    Ok(sparse_propagator)
}

/// Extract tile name from attribute (strip rotation prefix like "z " or "zz ").
fn get_tile_name(attr: &str) -> &str {
    if let Some(pos) = attr.rfind(' ') {
        &attr[pos + 1..]
    } else {
        attr
    }
}

/// Build map from input values to allowed tile patterns.
fn build_tile_map(
    input_grid: &MjGrid,
    tile_positions: &HashMap<String, Vec<usize>>,
    rules: &[(u8, Vec<String>)],
    num_patterns: usize,
) -> Vec<Vec<bool>> {
    let num_input_values = input_grid.c as usize;

    // Default: value 0 allows all patterns
    let mut map = vec![vec![true; num_patterns]; num_input_values];

    // Apply rules
    for (input_value, allowed_tiles) in rules {
        if (*input_value as usize) < map.len() {
            // Start with no patterns allowed
            map[*input_value as usize] = vec![false; num_patterns];

            // Allow patterns for each tile name
            for tile_name in allowed_tiles {
                if let Some(positions) = tile_positions.get(tile_name) {
                    for &pos in positions {
                        map[*input_value as usize][pos] = true;
                    }
                }
            }
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tileset_xml() {
        let xml = r#"
        <tileset>
            <tiles>
                <tile name="Empty" weight="2.0"/>
                <tile name="Line"/>
            </tiles>
            <neighbors>
                <neighbor left="Empty" right="Line"/>
                <neighbor top="Empty" bottom="Line"/>
            </neighbors>
        </tileset>
        "#;

        let (tiles, neighbors) = parse_tileset_xml(xml).unwrap();

        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0], ("Empty".to_string(), 2.0));
        assert_eq!(tiles[1], ("Line".to_string(), 1.0));

        assert_eq!(neighbors.len(), 2);
    }

    #[test]
    fn test_get_tile_name() {
        assert_eq!(get_tile_name("Empty"), "Empty");
        assert_eq!(get_tile_name("z Line"), "Line");
        assert_eq!(get_tile_name("zz Turn"), "Turn");
        assert_eq!(get_tile_name("zzz Down"), "Down");
    }

    #[test]
    fn test_z_rotate() {
        // 2x2x1 tile:
        // 0 1
        // 2 3
        let tile = vec![0, 1, 2, 3];
        let rotated = z_rotate(tile, 2, 1);
        // After 90 degree clockwise rotation:
        // 2 0
        // 3 1
        assert_eq!(rotated, vec![2, 0, 3, 1]);
    }

    #[test]
    fn test_x_reflect() {
        // 2x2x1 tile:
        // 0 1
        // 2 3
        let tile = vec![0, 1, 2, 3];
        let reflected = x_reflect(&tile, 2, 1);
        // After X reflection:
        // 1 0
        // 3 2
        assert_eq!(reflected, vec![1, 0, 3, 2]);
    }

    #[test]
    fn test_square_symmetries_3d() {
        let tile = vec![0, 1, 2, 3]; // 2x2x1
        let variants = square_symmetries_3d(&tile, 2, 1);

        // Should generate up to 8 variants
        assert!(!variants.is_empty());
        assert!(variants.len() <= 8);

        // All should be unique
        for (i, v1) in variants.iter().enumerate() {
            for (j, v2) in variants.iter().enumerate() {
                if i != j {
                    assert_ne!(v1, v2, "Variants {} and {} should be different", i, j);
                }
            }
        }
    }

    #[test]
    fn test_build_tile_map() {
        let grid = MjGrid::with_values(2, 2, 1, "AB");

        let mut tile_positions = HashMap::new();
        tile_positions.insert("Empty".to_string(), vec![0, 1]);
        tile_positions.insert("Line".to_string(), vec![2, 3]);

        let rules = vec![
            (1, vec!["Empty".to_string()]), // Value B allows only Empty
        ];

        let map = build_tile_map(&grid, &tile_positions, &rules, 4);

        // Value 0 (A) allows all patterns
        assert!(map[0].iter().all(|&b| b));

        // Value 1 (B) allows only Empty patterns (0, 1)
        assert!(map[1][0]);
        assert!(map[1][1]);
        assert!(!map[1][2]);
        assert!(!map[1][3]);
    }

    // ====================================================================
    // VOX Loading Tests
    // ====================================================================

    fn tilesets_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MarkovJunior/resources/tilesets")
    }

    #[test]
    fn test_get_tile_size_real_vox() {
        let path = tilesets_path().join("EscherSurface/Empty.vox");
        if !path.exists() {
            println!("Skipping test: {:?} not found", path);
            return;
        }

        let result = get_tile_size(&path);
        assert!(result.is_ok(), "Failed to get tile size: {:?}", result);

        let (s, sz) = result.unwrap();
        assert!(s > 0, "Tile size should be positive");
        assert!(sz > 0, "Tile depth should be positive");
        println!("Empty.vox tile size: {}x{}x{}", s, s, sz);
    }

    #[test]
    fn test_load_vox_tile_real() {
        let path = tilesets_path().join("EscherSurface/Line.vox");
        if !path.exists() {
            println!("Skipping test: {:?} not found", path);
            return;
        }

        let mut uniques = Vec::new();
        let result = load_vox_tile(&path, &mut uniques);
        assert!(result.is_ok(), "Failed to load tile: {:?}", result);

        let (data, num_colors) = result.unwrap();
        assert!(!data.is_empty(), "Tile data should not be empty");
        assert!(num_colors > 0, "Should have at least one color");

        println!("Line.vox: {} voxels, {} colors", data.len(), num_colors);
    }

    #[test]
    fn test_cube_symmetries_cubic_tile() {
        // Create a 2x2x2 cubic tile with distinct values
        let tile = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let variants = cube_symmetries(&tile, 2, 2);

        // Should get multiple unique variants for an asymmetric tile
        assert!(
            variants.len() > 1,
            "Asymmetric tile should have multiple variants"
        );

        // Should not exceed 48
        assert!(variants.len() <= 48, "Should not exceed 48 variants");

        // All should be unique
        for (i, v1) in variants.iter().enumerate() {
            for (j, v2) in variants.iter().enumerate() {
                if i != j {
                    assert_ne!(v1, v2, "Variants {} and {} should be different", i, j);
                }
            }
        }

        println!("2x2x2 asymmetric tile: {} unique variants", variants.len());
    }

    #[test]
    fn test_cube_symmetries_symmetric_tile() {
        // Create a fully symmetric 2x2x2 tile (all same value)
        let tile = vec![1, 1, 1, 1, 1, 1, 1, 1];
        let variants = cube_symmetries(&tile, 2, 2);

        // Fully symmetric tile should have only 1 variant
        assert_eq!(
            variants.len(),
            1,
            "Fully symmetric tile should have 1 variant"
        );
    }

    #[test]
    fn test_y_rotate() {
        // Create a 2x2x2 cubic tile
        // Layout: z=0: [0,1,2,3], z=1: [4,5,6,7]
        let tile = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let rotated = y_rotate(&tile, 2, 2);

        // After Y rotation, the tile should be different
        assert_ne!(tile, rotated, "Y rotation should change the tile");

        // Four Y rotations should return to original
        let r1 = y_rotate(&tile, 2, 2);
        let r2 = y_rotate(&r1, 2, 2);
        let r3 = y_rotate(&r2, 2, 2);
        let r4 = y_rotate(&r3, 2, 2);
        assert_eq!(tile, r4, "Four Y rotations should return to original");
    }
}
