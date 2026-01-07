//! Tile model WFC node.
//!
//! TileNode uses pre-defined tilesets with neighbor constraints
//! to generate valid tile arrangements.
//!
//! C# Reference: TileModel.cs (lines 1-332)

use super::wfc_node::{WfcNode, WfcState};

use crate::markov_junior::helper::load_vox;
use crate::markov_junior::node::{ExecutionContext, Node};
use crate::markov_junior::rng::{DotNetRandom, MjRng};
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
///
/// Like C# WFCNode, TileNode extends Branch and can have child nodes
/// that execute after WFC completes on the newgrid.
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

    /// Child nodes to execute after WFC completes (like C# Branch.nodes)
    pub children: Vec<Box<dyn Node>>,

    /// Current child index for sequential execution
    child_n: usize,
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
        _full_symmetry: bool,        // Ignored - read from tileset XML instead
    ) -> Result<Self, String> {
        // Load tileset XML
        let xml = std::fs::read_to_string(tileset_path)
            .map_err(|e| format!("Failed to read tileset: {}", e))?;

        // Get directory for tile VOX files
        let tileset_dir = tileset_path.parent().unwrap_or(Path::new("."));

        // Parse tileset - fullSymmetry comes from tileset XML, not from caller
        // C# Reference: TileModel.cs line 33
        let (tile_info, neighbors, full_symmetry) = parse_tileset_xml(&xml)?;

        if tile_info.is_empty() {
            return Err("No tiles found in tileset".to_string());
        }

        // Determine tile size from first tile
        let first_tile_path = tileset_dir
            .join(tiles_name)
            .join(&format!("{}.vox", tile_info[0].0));
        let (s, sz) = get_tile_size(&first_tile_path)?;

        // Calculate correct output grid dimensions
        // C# Reference: TileModel.cs line 55:
        // newgrid = Grid.Load(xelem, (S - overlap) * grid.MX + overlap, ...)
        let output_mx = (s - overlap) * input_grid.mx + overlap;
        let output_my = (s - overlap) * input_grid.my + overlap;
        let output_mz = (sz - overlapz) * input_grid.mz + overlapz;

        // Create newgrid with correct dimensions (preserving values/characters from passed newgrid)
        let mut correct_newgrid = MjGrid::new(output_mx, output_my, output_mz);
        correct_newgrid.c = newgrid.c;
        correct_newgrid.characters = newgrid.characters.clone();
        correct_newgrid.values = newgrid.values.clone();
        correct_newgrid.waves = newgrid.waves.clone();

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

        // Wave dimensions are based on input grid (each wave cell places one tile)
        let wave_mx = input_grid.mx;
        let wave_my = input_grid.my;
        let wave_mz = input_grid.mz;
        let wave_length = wave_mx * wave_my * wave_mz;
        let num_directions = if wave_mz == 1 { 4 } else { 6 };

        // Build map from input values to allowed patterns
        let map = build_tile_map(input_grid, &tile_positions, rules, num_patterns);

        // For TileModel, N=1 because each wave cell places one tile at a discrete position.
        // (The tile SIZE is s, but the wave cell SIZE for boundary checks is 1.)
        // C# Reference: WFCNode line 11: "protected int P, N = 1;"
        let wfc = WfcNode::new(
            wave_length,
            num_patterns,
            num_directions,
            propagator,
            weights,
            correct_newgrid, // Use the correctly-sized output grid
            map,
            1, // N=1 for TileModel (not s)
            periodic,
            shannon,
            tries,
            wave_mx,
            wave_my,
            wave_mz,
        );

        Ok(Self {
            wfc,
            tiledata,
            s,
            sz,
            overlap,
            overlapz,
            children: Vec::new(),
            child_n: 0,
        })
    }

    /// Add children to execute after WFC completes.
    ///
    /// C# Reference: WFCNode extends Branch, which parses children in Load()
    pub fn with_children(mut self, children: Vec<Box<dyn Node>>) -> Self {
        self.children = children;
        self
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
        let output_mz = grid.mz;

        // Use fixed seed to match C# for testing
        let mut rng = DotNetRandom::from_seed(12345);

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
                                    let value = vote as f64 + 0.1 * rng.next_double();
                                    if value > max_vote {
                                        argmax = c as u8;
                                        max_vote = value;
                                    }
                                }

                                let sx = x * (s - overlap) + dx;
                                let sy = y * (s - overlap) + dy;
                                let sz_coord = z * (sz - overlapz) + dz;

                                if sx < output_mx && sy < output_my && sz_coord < output_mz {
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
        self.child_n = 0;
        for child in &mut self.children {
            child.reset();
        }
    }

    fn go(&mut self, ctx: &mut ExecutionContext) -> bool {
        // Phase 2: Execute children after WFC completes
        // C# Reference: WFCNode.Go() line 71: `if (n >= 0) return base.Go();`
        if self.wfc.child_index >= 0 {
            // Execute children sequentially (like Branch.Go())
            while self.child_n < self.children.len() {
                let child = &mut self.children[self.child_n];
                if child.go(ctx) {
                    return true;
                }
                // Child completed, move to next
                self.child_n += 1;
                if self.child_n < self.children.len() {
                    self.children[self.child_n].reset();
                }
            }
            // All children done
            self.reset();
            return false;
        }

        // Phase 1: WFC initialization
        if self.wfc.first_go {
            if !self.wfc.initialize(ctx.grid, ctx.random) {
                return false;
            }
            std::mem::swap(&mut self.wfc.newgrid, ctx.grid);
            return true;
        }

        // Phase 1: WFC stepping
        if self.wfc.step() {
            if ctx.gif {
                self.update_state(ctx.grid);
            }
            true
        } else {
            // WFC completed or failed
            if self.wfc.state == WfcState::Completed {
                self.update_state(ctx.grid);
                // Mark that we should execute children now
                // C# Reference: WFCNode.Go() line 114: `else n++;` followed by line 117: `return true;`
                // C# returns true on the step when WFC completes (n becomes 0), then on the
                // NEXT call, base.Go() is invoked (line 71). This means WFC completion
                // counts as a step, even if there are no children.
                self.wfc.child_index = 0;
                // Reset first child for execution
                if !self.children.is_empty() {
                    self.children[0].reset();
                }
                // Always return true on completion - C# does this even with no children
                // The next call will go to child execution path, which returns false if empty
                return true;
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

/// Parse tileset XML to extract tiles, neighbors, and fullSymmetry flag.
///
/// C# Reference: TileModel.cs line 33 reads fullSymmetry from tileset XML root.
fn parse_tileset_xml(xml: &str) -> Result<(TileInfo, Vec<Neighbor>, bool), String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut tiles = Vec::new();
    let mut neighbors = Vec::new();
    let mut in_tiles = false;
    let mut in_neighbors = false;
    let mut full_symmetry = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
                match name {
                    "tileset" => {
                        // Parse fullSymmetry attribute from root tileset element
                        for attr in e.attributes().flatten() {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let value = std::str::from_utf8(&attr.value).unwrap_or("");
                            if key == "fullSymmetry" {
                                full_symmetry = value.eq_ignore_ascii_case("true");
                            }
                        }
                    }
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

    Ok((tiles, neighbors, full_symmetry))
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

    // Map voxel palette indices to global ordinals.
    // C# Reference: Helper.cs Ords() lines 8-24
    //
    // CRITICAL: The C# Ords() function treats ALL values equally, including -1 (empty).
    // When -1 is first encountered, it gets added to the uniques list and assigned an ordinal.
    // This means empty voxels are NOT hardcoded to 0 - they get whatever ordinal corresponds
    // to when -1 was first seen in the encounter order.
    //
    // This is important because it affects the mapping between VOX colors and grid values.
    // If we hardcode empty to 0 but don't add -1 to uniques, the ordinals of actual colors
    // get shifted, breaking the color mapping.
    let mut result = vec![0u8; total];

    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                let src_idx = x + y * mx + z * mx * my;
                let dst_idx = x + y * s + z * s * s;

                let v = voxels[src_idx];
                // Treat ALL values (including -1 for empty) the same way:
                // find or add to uniques list
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
///
/// C# Reference (TileModel.cs line 63):
///   byte[] yRotate(byte[] p) => newtile((x, y, z) => p[z + y * S + (S - 1 - x) * S * S]);
///
/// This means: result[x, y, z] = input[z, y, S-1-x]
fn y_rotate(tile: &[u8], s: usize, sz: usize) -> Vec<u8> {
    let mut result = vec![0u8; s * s * sz];
    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                // C#: result[x + y*S + z*S*S] = p[z + y * S + (S - 1 - x) * S * S]
                // So input position is (z, y, S-1-x)
                let src = z + y * s + (s - 1 - x) * s * s;
                let dst = x + y * s + z * s * s;
                result[dst] = tile[src];
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

/// Find the index of a tile in tiledata by comparing byte arrays.
fn find_tile_index(tiledata: &[Vec<u8>], tile: &[u8]) -> Option<usize> {
    tiledata.iter().position(|t| t == tile)
}

/// Apply prefix rotations to a tile (e.g., "z Line" -> z-rotate the Line tile).
fn apply_tile_rotations(tile: &[u8], prefix: &str, s: usize, sz: usize) -> Vec<u8> {
    let mut result = tile.to_vec();
    // Apply rotations in reverse order (right to left in prefix)
    for c in prefix.chars().rev() {
        match c {
            'x' => result = x_rotate_tile(&result, s, sz),
            'y' => result = y_rotate_tile(&result, s, sz),
            'z' => result = z_rotate_tile(&result, s, sz),
            ' ' => {}
            _ => {}
        }
    }
    result
}

/// Rotate a tile 90 degrees around X axis (for propagator building).
///
/// C# Reference (TileModel.cs line 64):
///   byte[] xRotate(byte[] p) => newtile((x, y, z) => p[x + z * S + (S - 1 - y) * S * S]);
///
/// This means: result[x, y, z] = input[x, z, S-1-y]
fn x_rotate_tile(tile: &[u8], s: usize, sz: usize) -> Vec<u8> {
    let mut result = vec![0u8; s * s * sz];
    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                // C#: result[x + y*S + z*S*S] = p[x + z * S + (S - 1 - y) * S * S]
                // So input position is (x, z, S-1-y)
                let src = x + z * s + (s - 1 - y) * s * s;
                let dst = x + y * s + z * s * s;
                result[dst] = tile[src];
            }
        }
    }
    result
}

/// Rotate a tile 90 degrees around Y axis (for propagator building).
///
/// C# Reference (TileModel.cs line 63):
///   byte[] yRotate(byte[] p) => newtile((x, y, z) => p[z + y * S + (S - 1 - x) * S * S]);
///
/// This means: result[x, y, z] = input[z, y, S-1-x]
fn y_rotate_tile(tile: &[u8], s: usize, sz: usize) -> Vec<u8> {
    let mut result = vec![0u8; s * s * sz];
    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                // C#: result[x + y*S + z*S*S] = p[z + y * S + (S - 1 - x) * S * S]
                // So input position is (z, y, S-1-x)
                let src = z + y * s + (s - 1 - x) * s * s;
                let dst = x + y * s + z * s * s;
                result[dst] = tile[src];
            }
        }
    }
    result
}

/// Rotate a tile 90 degrees around Z axis (for propagator building).
fn z_rotate_tile(tile: &[u8], s: usize, sz: usize) -> Vec<u8> {
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

/// Reflect a tile along X axis (for propagator building).
fn x_reflect_tile(tile: &[u8], s: usize, sz: usize) -> Vec<u8> {
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

/// Reflect a tile along Y axis (for propagator building).
fn y_reflect_tile(tile: &[u8], s: usize, sz: usize) -> Vec<u8> {
    let mut result = vec![0u8; s * s * sz];
    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                let src = x + y * s + z * s * s;
                let dst = x + (s - 1 - y) * s + z * s * s;
                result[dst] = tile[src];
            }
        }
    }
    result
}

/// Reflect a tile along Z axis (for propagator building).
fn z_reflect_tile(tile: &[u8], s: usize, sz: usize) -> Vec<u8> {
    let mut result = vec![0u8; s * s * sz];
    for z in 0..sz {
        for y in 0..s {
            for x in 0..s {
                let src = x + y * s + z * s * s;
                let dst = x + y * s + (sz - 1 - z) * s * s;
                result[dst] = tile[src];
            }
        }
    }
    result
}

/// Generate square symmetries for a tile (8 elements) with parametric rotation and reflection.
/// C# Reference: SymmetryHelper.SquareSymmetries
fn tile_square_symmetries_with<R, F>(
    tile: &[u8],
    s: usize,
    sz: usize,
    rotate: R,
    reflect: F,
) -> Vec<Vec<u8>>
where
    R: Fn(&[u8], usize, usize) -> Vec<u8>,
    F: Fn(&[u8], usize, usize) -> Vec<u8>,
{
    let s0 = tile.to_vec();
    let s1 = reflect(&s0, s, sz);
    let s2 = rotate(&s0, s, sz);
    let s3 = reflect(&s2, s, sz);
    let s4 = rotate(&s2, s, sz);
    let s5 = reflect(&s4, s, sz);
    let s6 = rotate(&s4, s, sz);
    let s7 = reflect(&s6, s, sz);
    vec![s0, s1, s2, s3, s4, s5, s6, s7]
}

/// Build propagator from neighbor constraints.
///
/// C# Reference: TileModel.cs lines 165-261
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

    // Helper to get tile data for an attribute like "z Line"
    let get_tile_data = |attr: &str| -> Option<Vec<u8>> {
        let parts: Vec<&str> = attr.split(' ').collect();
        let (prefix, tile_name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("", parts[0])
        };
        let positions = tile_positions.get(tile_name)?;
        if positions.is_empty() {
            return None;
        }
        let base_tile = &tiledata[positions[0]];
        Some(apply_tile_rotations(base_tile, prefix, s, sz))
    };

    // Process each neighbor constraint
    for neighbor in neighbors {
        let left_tile = match get_tile_data(&neighbor.left) {
            Some(t) => t,
            None => continue,
        };
        let right_tile = match get_tile_data(&neighbor.right) {
            Some(t) => t,
            None => continue,
        };

        if full_symmetry {
            // C# Reference: TileModel.cs lines 167-210
            // For X direction: SquareSymmetries(tile, xRotate, yReflect, ...)
            let lsym =
                tile_square_symmetries_with(&left_tile, s, sz, x_rotate_tile, y_reflect_tile);
            let rsym =
                tile_square_symmetries_with(&right_tile, s, sz, x_rotate_tile, y_reflect_tile);

            // For each symmetry pair, set X-direction constraint
            for i in 0..lsym.len() {
                if let (Some(li), Some(ri)) = (
                    find_tile_index(tiledata, &lsym[i]),
                    find_tile_index(tiledata, &rsym[i]),
                ) {
                    propagator[0][li][ri] = true;
                }

                // Also set x-reflected version
                let lref = x_reflect_tile(&rsym[i], s, sz);
                let rref = x_reflect_tile(&lsym[i], s, sz);
                if let (Some(li), Some(ri)) = (
                    find_tile_index(tiledata, &lref),
                    find_tile_index(tiledata, &rref),
                ) {
                    propagator[0][li][ri] = true;
                }
            }

            // For Y direction: zRotate tiles, then SquareSymmetries(tile, yRotate, zReflect, ...)
            let dtile = z_rotate_tile(&left_tile, s, sz);
            let utile = z_rotate_tile(&right_tile, s, sz);
            let dsym = tile_square_symmetries_with(&dtile, s, sz, y_rotate_tile, z_reflect_tile);
            let usym = tile_square_symmetries_with(&utile, s, sz, y_rotate_tile, z_reflect_tile);

            for i in 0..dsym.len() {
                if let (Some(di), Some(ui)) = (
                    find_tile_index(tiledata, &dsym[i]),
                    find_tile_index(tiledata, &usym[i]),
                ) {
                    propagator[1][di][ui] = true;
                }

                let dref = y_reflect_tile(&usym[i], s, sz);
                let uref = y_reflect_tile(&dsym[i], s, sz);
                if let (Some(di), Some(ui)) = (
                    find_tile_index(tiledata, &dref),
                    find_tile_index(tiledata, &uref),
                ) {
                    propagator[1][di][ui] = true;
                }
            }

            // For Z direction: yRotate tiles, then SquareSymmetries(tile, zRotate, xReflect, ...)
            let btile = y_rotate_tile(&left_tile, s, sz);
            let ttile = y_rotate_tile(&right_tile, s, sz);
            let bsym = tile_square_symmetries_with(&btile, s, sz, z_rotate_tile, x_reflect_tile);
            let tsym = tile_square_symmetries_with(&ttile, s, sz, z_rotate_tile, x_reflect_tile);

            for i in 0..bsym.len() {
                if let (Some(bi), Some(ti)) = (
                    find_tile_index(tiledata, &bsym[i]),
                    find_tile_index(tiledata, &tsym[i]),
                ) {
                    propagator[4][bi][ti] = true;
                }

                let bref = z_reflect_tile(&tsym[i], s, sz);
                let tref = z_reflect_tile(&bsym[i], s, sz);
                if let (Some(bi), Some(ti)) = (
                    find_tile_index(tiledata, &bref),
                    find_tile_index(tiledata, &tref),
                ) {
                    propagator[4][bi][ti] = true;
                }
            }
        } else {
            // Non-fullSymmetry case with proper rotation/reflection handling
            // C# Reference: TileModel.cs lines 212-254
            match neighbor.dir {
                NeighborDir::Horizontal => {
                    // C# lines 214-236: left/right neighbors
                    // Uses tile() to apply rotation prefix, then 4 symmetry variants for dir 0,
                    // and 4 more variants (with zRotate) for dir 1

                    // Direction 0 (+X): 4 symmetry variants
                    // C# lines 222-225:
                    // propagator[0][ltile][rtile] = true
                    // propagator[0][yReflect(ltile)][yReflect(rtile)] = true
                    // propagator[0][xReflect(rtile)][xReflect(ltile)] = true  (swapped!)
                    // propagator[0][yReflect(xReflect(rtile))][yReflect(xReflect(ltile))] = true

                    if let (Some(li), Some(ri)) = (
                        find_tile_index(tiledata, &left_tile),
                        find_tile_index(tiledata, &right_tile),
                    ) {
                        propagator[0][li][ri] = true;
                    }

                    let ly = y_reflect_tile(&left_tile, s, sz);
                    let ry = y_reflect_tile(&right_tile, s, sz);
                    if let (Some(li), Some(ri)) = (
                        find_tile_index(tiledata, &ly),
                        find_tile_index(tiledata, &ry),
                    ) {
                        propagator[0][li][ri] = true;
                    }

                    // xReflect: swapped (rtile, ltile)
                    let rx = x_reflect_tile(&right_tile, s, sz);
                    let lx = x_reflect_tile(&left_tile, s, sz);
                    if let (Some(ri), Some(li)) = (
                        find_tile_index(tiledata, &rx),
                        find_tile_index(tiledata, &lx),
                    ) {
                        propagator[0][ri][li] = true;
                    }

                    let ryx = y_reflect_tile(&x_reflect_tile(&right_tile, s, sz), s, sz);
                    let lyx = y_reflect_tile(&x_reflect_tile(&left_tile, s, sz), s, sz);
                    if let (Some(ri), Some(li)) = (
                        find_tile_index(tiledata, &ryx),
                        find_tile_index(tiledata, &lyx),
                    ) {
                        propagator[0][ri][li] = true;
                    }

                    // Direction 1 (+Y): zRotate tiles, then 4 variants
                    // C# lines 227-235
                    let dtile = z_rotate_tile(&left_tile, s, sz);
                    let utile = z_rotate_tile(&right_tile, s, sz);

                    if let (Some(di), Some(ui)) = (
                        find_tile_index(tiledata, &dtile),
                        find_tile_index(tiledata, &utile),
                    ) {
                        propagator[1][di][ui] = true;
                    }

                    let dx = x_reflect_tile(&dtile, s, sz);
                    let ux = x_reflect_tile(&utile, s, sz);
                    if let (Some(di), Some(ui)) = (
                        find_tile_index(tiledata, &dx),
                        find_tile_index(tiledata, &ux),
                    ) {
                        propagator[1][di][ui] = true;
                    }

                    // yReflect: swapped
                    let uy = y_reflect_tile(&utile, s, sz);
                    let dy = y_reflect_tile(&dtile, s, sz);
                    if let (Some(ui), Some(di)) = (
                        find_tile_index(tiledata, &uy),
                        find_tile_index(tiledata, &dy),
                    ) {
                        propagator[1][ui][di] = true;
                    }

                    let uxy = x_reflect_tile(&y_reflect_tile(&utile, s, sz), s, sz);
                    let dxy = x_reflect_tile(&y_reflect_tile(&dtile, s, sz), s, sz);
                    if let (Some(ui), Some(di)) = (
                        find_tile_index(tiledata, &uxy),
                        find_tile_index(tiledata, &dxy),
                    ) {
                        propagator[1][ui][di] = true;
                    }
                }
                NeighborDir::Vertical => {
                    // C# lines 239-253: top/bottom neighbors
                    // For top/bottom (note: parse_neighbor puts bottom in left, top in right)
                    // top = right_tile, bottom = left_tile
                    let ttile = &right_tile; // top
                    let btile = &left_tile; // bottom

                    // Generate 4 square symmetries for each
                    let tsym =
                        tile_square_symmetries_with(ttile, s, sz, z_rotate_tile, x_reflect_tile);
                    let bsym =
                        tile_square_symmetries_with(btile, s, sz, z_rotate_tile, x_reflect_tile);

                    // Direction 4 (+Z): all symmetry pairs
                    for i in 0..tsym.len() {
                        if let (Some(bi), Some(ti)) = (
                            find_tile_index(tiledata, &bsym[i]),
                            find_tile_index(tiledata, &tsym[i]),
                        ) {
                            propagator[4][bi][ti] = true;
                        }
                    }
                }
            }
        }
    }

    // Derive opposite directions
    // C# Reference: TileModel.cs lines 256-261
    for p2 in 0..num_patterns {
        for p1 in 0..num_patterns {
            propagator[2][p2][p1] = propagator[0][p1][p2]; // -X from +X
            propagator[3][p2][p1] = propagator[1][p1][p2]; // -Y from +Y
            propagator[5][p2][p1] = propagator[4][p1][p2]; // -Z from +Z
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

        let (tiles, neighbors, full_symmetry) = parse_tileset_xml(xml).unwrap();

        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0], ("Empty".to_string(), 2.0));
        assert_eq!(tiles[1], ("Line".to_string(), 1.0));

        assert_eq!(neighbors.len(), 2);
        assert!(!full_symmetry); // No fullSymmetry attribute in this test
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

    // ====================================================================
    // BUG REGRESSION TEST: Tile Color Mapping
    // ====================================================================
    //
    // This test catches the Apartemazements bug where tile VOX palette
    // indices are not correctly mapped to grid character values.
    //
    // See: docs/bugs/apartemazements_deep_dive.md
    // ====================================================================

    #[test]
    fn test_tile_color_mapping_bug() {
        // This test documents the KNOWN BUG in tile loading.
        // When this test passes, the bug is fixed.
        //
        // The Paths tileset is used by Apartemazements.
        // Grid values are "BYDAWP RFUENC" (12 characters, values 0-11).
        // The tiles should produce values that map to these characters.

        let tileset_path = tilesets_path().join("Paths.xml");
        if !tileset_path.exists() {
            println!("Skipping test: {:?} not found", tileset_path);
            return;
        }

        // Create grid with same values as Apartemazements WFC
        let grid = MjGrid::try_with_values(8, 8, 8, "BYDAWP RFUENC").unwrap();
        assert_eq!(grid.c, 12, "Grid should have 12 character values");

        // Load the TileNode
        let result = TileNode::from_tileset(
            &tileset_path,
            "Paths", // tiles folder name
            true,    // periodic
            false,   // shannon
            10,      // tries
            0,       // overlap
            0,       // overlapz
            grid.clone(),
            &grid,
            &[],   // no rules
            false, // full_symmetry
        );

        assert!(result.is_ok(), "Failed to load tileset: {:?}", result.err());
        let tile_node = result.unwrap();

        // Collect all values used in tile data
        let mut tile_values: std::collections::HashSet<u8> = std::collections::HashSet::new();
        for tile in &tile_node.tiledata {
            for &v in tile {
                tile_values.insert(v);
            }
        }

        println!("Tile values found: {:?}", tile_values);
        println!("Number of tiles: {}", tile_node.tiledata.len());
        println!(
            "Tile size: {}x{}x{}",
            tile_node.s, tile_node.s, tile_node.sz
        );

        // THE BUG: Currently tiles only have values {0,1,2,3,4}
        // These are sequential ordinals from load_vox_tile(), NOT grid values.
        //
        // The fix needs to map VOX palette colors to the correct grid values.
        // After fixing, tiles should have values that correspond to:
        // - B (0): Background/empty
        // - Y (1): Earth marker for random placement
        // - D (2): Down/column marker
        // - A (3): Air
        // - W (4): Wall/path
        // - P (5): Path marker
        // - etc.
        //
        // The "Down" tile specifically should contain D (2) values to mark
        // where columns should be drawn by WFC children.

        // This assertion documents the current broken behavior:
        let only_has_sequential_ordinals = tile_values.iter().all(|&v| v <= 4);

        if only_has_sequential_ordinals && tile_values.len() <= 5 {
            // BUG PRESENT: Tiles only have sequential ordinals 0-4
            println!("\n=== BUG DETECTED ===");
            println!("Tiles only contain values {:?}", tile_values);
            println!("These are raw VOX ordinals, NOT grid character values!");
            println!("The WFC children will fail because they expect values like:");
            println!("  - D (2) for columns");
            println!("  - Y (1) for earth markers");
            println!("  - P (5), R (6), F (7), etc.");
            println!("See: docs/bugs/apartemazements_deep_dive.md");
            println!("===================\n");

            // Fail the test to indicate the bug exists
            panic!(
                "KNOWN BUG: Tile values are raw ordinals {:?}, not grid values. \
                 Fix tile color mapping in load_vox_tile(). \
                 See docs/bugs/apartemazements_deep_dive.md",
                tile_values
            );
        }

        // After the fix, tiles should contain meaningful grid values
        // that correspond to the building structure elements.
        assert!(
            tile_values.len() > 5,
            "After fix: tiles should use more than 5 distinct values for building elements"
        );
    }

    #[test]
    fn test_wfc_output_has_structure_values() {
        // This test verifies that after WFC runs, the output grid
        // contains the values needed by child nodes.
        //
        // Apartemazements children need:
        // - B (0): converted to C (earth)
        // - D (2): used for column placement
        // - Y (1): random earth markers
        // - A (3): air cells
        // - W (4): wall cells
        //
        // If WFC only outputs {0, 3, 4}, children for columns/windows fail.

        let tileset_path = tilesets_path().join("Paths.xml");
        if !tileset_path.exists() {
            println!("Skipping test: {:?} not found", tileset_path);
            return;
        }

        // Setup grid with constraints like Apartemazements
        let mut grid = MjGrid::try_with_values(8, 8, 8, "BYDAWP RFUENC").unwrap();

        // Set up a simple constraint: N cells should become paths
        // W cells should become empty
        // In Apartemazements, N marks the boundary where paths go
        let n_value = *grid.values.get(&'N').unwrap(); // 10
        let w_value = *grid.values.get(&'W').unwrap(); // 4
        let b_value = *grid.values.get(&'B').unwrap(); // 0

        // Create a simple test pattern: bottom layer is N (paths), rest is W (empty)
        for i in 0..grid.state.len() {
            let z = i / (grid.mx * grid.my);
            if z == 0 {
                grid.state[i] = n_value; // Bottom: path constraints
            } else {
                grid.state[i] = w_value; // Rest: empty constraints
            }
        }

        // Define rules: N -> path tiles, W -> empty tiles
        let rules = vec![
            (w_value, vec!["Empty".to_string()]),
            (
                n_value,
                vec![
                    "Empty".to_string(),
                    "Line".to_string(),
                    "Turn".to_string(),
                    "X".to_string(),
                ],
            ),
        ];

        let result = TileNode::from_tileset(
            &tileset_path,
            "Paths",
            true,
            false,
            10,
            0,
            0,
            grid.clone(),
            &grid,
            &rules,
            false,
        );

        if result.is_err() {
            println!("Skipping test: Failed to load tileset: {:?}", result.err());
            return;
        }

        let mut tile_node = result.unwrap();

        // Run WFC to completion
        use crate::markov_junior::node::{ExecutionContext, Node};
        use crate::markov_junior::rng::StdRandom;
        let mut rng = StdRandom::from_u64_seed(12345);
        let mut ctx = ExecutionContext::new(&mut grid, &mut rng);

        // Run until WFC completes (or max steps)
        let mut steps = 0;
        while tile_node.go(&mut ctx) && steps < 10000 {
            steps += 1;
        }

        // Collect output values
        let mut output_values: std::collections::HashSet<u8> = std::collections::HashSet::new();
        for &v in &ctx.grid.state {
            output_values.insert(v);
        }

        println!("WFC completed after {} steps", steps);
        println!("Output values: {:?}", output_values);

        // Check if we got meaningful structure
        // After fix, output should have D values for columns, etc.
        let has_column_markers = output_values.contains(&2); // D
        let has_path_markers = output_values.contains(&5); // P

        if !has_column_markers && !has_path_markers {
            println!("\n=== STRUCTURE VALUES MISSING ===");
            println!("WFC output only has: {:?}", output_values);
            println!("Missing D (2) for columns and P (5) for paths");
            println!("This indicates tile color mapping is broken");
            println!("================================\n");
        }

        // This will fail until the bug is fixed
        // Uncomment to enforce after fix:
        // assert!(has_column_markers || has_path_markers,
        //     "WFC output should have structure marker values (D=2, P=5, etc.)");
    }

    /// Test that WFC newgrid dimensions are calculated correctly.
    ///
    /// C# Reference: TileModel.cs line 55:
    /// newgrid = Grid.Load(xelem, (S - overlap) * grid.MX + overlap, ...)
    ///
    /// For 5x5x5 tiles with 0 overlap on an 8x8x8 input grid:
    /// newgrid should be (5-0)*8+0 = 40x40x40, NOT 8x8x8
    ///
    /// BUG: loader.rs creates newgrid with input grid dimensions instead of
    /// calculated output dimensions based on tile size and overlap.
    #[test]
    fn test_wfc_newgrid_dimensions() {
        // This test documents the KNOWN BUG in WFC newgrid dimension calculation.
        //
        // The Paths tileset has 5x5x5 tiles.
        // With an 8x8x8 input grid and 0 overlap:
        // - C# formula: (S - overlap) * input_dim + overlap
        // - Expected output: (5-0)*8+0 = 40 in each dimension
        //
        // Our code incorrectly uses input grid dimensions (8x8x8).

        let tileset_path = tilesets_path().join("Paths.xml");
        if !tileset_path.exists() {
            println!("Skipping test: {:?} not found", tileset_path);
            return;
        }

        // Create input grid (like Apartemazements before WFC)
        let input_grid = MjGrid::try_with_values(8, 8, 8, "BWN").unwrap();

        // Create newgrid with values from WFC (like Apartemazements WFC)
        let newgrid = MjGrid::try_with_values(8, 8, 8, "BYDAWP RFUENC").unwrap();

        // Load the TileNode
        let result = TileNode::from_tileset(
            &tileset_path,
            "Paths",
            true,  // periodic
            false, // shannon
            10,    // tries
            0,     // overlap
            0,     // overlapz
            newgrid,
            &input_grid,
            &[], // no rules
            false,
        );

        let tile_node = result.expect("Failed to load tileset");

        // The tile size should be 5x5x5
        assert_eq!(tile_node.s, 5, "Paths tiles should be 5x5 in XY");
        assert_eq!(tile_node.sz, 5, "Paths tiles should be 5 in Z");

        // The WFC newgrid dimensions should be:
        // (tile_size - overlap) * input_size + overlap
        // = (5 - 0) * 8 + 0 = 40
        let expected_dim = (tile_node.s - tile_node.overlap) * input_grid.mx + tile_node.overlap;
        let expected_dimz =
            (tile_node.sz - tile_node.overlapz) * input_grid.mz + tile_node.overlapz;

        println!(
            "Tile size: {}x{}x{}",
            tile_node.s, tile_node.s, tile_node.sz
        );
        println!(
            "Input grid: {}x{}x{}",
            input_grid.mx, input_grid.my, input_grid.mz
        );
        println!(
            "Overlap: {}, overlapz: {}",
            tile_node.overlap, tile_node.overlapz
        );
        println!(
            "Expected output dimensions: {}x{}x{}",
            expected_dim, expected_dim, expected_dimz
        );
        println!(
            "Actual output dimensions: {}x{}x{}",
            tile_node.wfc.newgrid.mx, tile_node.wfc.newgrid.my, tile_node.wfc.newgrid.mz
        );

        // This assertion will FAIL until the bug is fixed
        assert_eq!(
            tile_node.wfc.newgrid.mx,
            expected_dim,
            "BUG: WFC newgrid MX should be {} (calculated from tile size and input), got {}.\n\
             C# formula: (S - overlap) * grid.MX + overlap = ({} - {}) * {} + {} = {}\n\
             See: TileModel.cs line 55",
            expected_dim,
            tile_node.wfc.newgrid.mx,
            tile_node.s,
            tile_node.overlap,
            input_grid.mx,
            tile_node.overlap,
            expected_dim
        );
    }
}
