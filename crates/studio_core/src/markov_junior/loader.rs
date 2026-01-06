//! XML model loader for MarkovJunior.
//!
//! Loads MarkovJunior XML model files and constructs the corresponding
//! node tree and grid.
//!
//! C# Reference: Interpreter.Load(), Node.Factory(), RuleNode.Load()

use super::convchain_node::ConvChainNode;
use super::convolution_node::{ConvolutionNode, ConvolutionRule};
use super::field::Field;
use super::helper::{load_resource, split_rule_image, ResourceError};
use super::map_node::{MapNode, ScaleFactor};
use super::node::Node;
use super::observation::Observation;
use super::path_node::PathNode;
use super::rule_node::RuleNodeData;
use super::symmetry::{cube_symmetries, square_symmetries, SquareSubgroup};
use super::wfc::{OverlapNode, TileNode};
use super::{AllNode, MarkovNode, MjGrid, MjRule, OneNode, ParallelNode, SequenceNode};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Error type for model loading.
#[derive(Debug, Clone)]
pub enum LoadError {
    /// File not found or cannot be read
    FileNotFound(String),
    /// XML parsing error
    XmlError(String),
    /// Missing required attribute
    MissingAttribute { element: String, attribute: String },
    /// Invalid attribute value
    InvalidAttribute {
        element: String,
        attribute: String,
        value: String,
        reason: String,
    },
    /// Unknown node type
    UnknownNodeType(String),
    /// Unknown symmetry type
    UnknownSymmetry(String),
    /// Rule parsing error
    RuleError(String),
    /// Unknown character in pattern
    UnknownCharacter { character: char, context: String },
    /// Grid construction error
    GridError(String),
    /// Resource loading error (PNG, etc.)
    ResourceError(String),
}

impl From<ResourceError> for LoadError {
    fn from(e: ResourceError) -> Self {
        LoadError::ResourceError(e.to_string())
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::FileNotFound(path) => write!(f, "file not found: {}", path),
            LoadError::XmlError(msg) => write!(f, "XML error: {}", msg),
            LoadError::MissingAttribute { element, attribute } => {
                write!(f, "missing attribute '{}' in <{}>", attribute, element)
            }
            LoadError::InvalidAttribute {
                element,
                attribute,
                value,
                reason,
            } => {
                write!(
                    f,
                    "invalid value '{}' for attribute '{}' in <{}>: {}",
                    value, attribute, element, reason
                )
            }
            LoadError::UnknownNodeType(name) => write!(f, "unknown node type: {}", name),
            LoadError::UnknownSymmetry(name) => write!(f, "unknown symmetry: {}", name),
            LoadError::RuleError(msg) => write!(f, "rule error: {}", msg),
            LoadError::UnknownCharacter { character, context } => {
                write!(f, "unknown character '{}' in {}", character, context)
            }
            LoadError::GridError(msg) => write!(f, "grid error: {}", msg),
            LoadError::ResourceError(msg) => write!(f, "resource error: {}", msg),
        }
    }
}

impl std::error::Error for LoadError {}

/// Known node type names.
const NODE_NAMES: &[&str] = &[
    "one",
    "all",
    "prl",
    "markov",
    "sequence",
    "path",
    "map",
    "convolution",
    "convchain",
    "wfc",
];

/// Result of loading a model from XML.
pub struct LoadedModel {
    /// The root node of the model
    pub root: Box<dyn Node>,
    /// The grid with values configured
    pub grid: MjGrid,
    /// Whether origin flag is set
    pub origin: bool,
}

impl std::fmt::Debug for LoadedModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoadedModel")
            .field("grid_size", &(self.grid.mx, self.grid.my, self.grid.mz))
            .field("origin", &self.origin)
            .finish()
    }
}

/// Context for loading, containing resource paths and grid folder.
#[derive(Clone)]
struct LoadContext {
    /// Base path for resources (e.g., MarkovJunior/resources)
    resources_path: Option<PathBuf>,
    /// Optional folder within resources/rules for this model
    folder: Option<String>,
}

impl LoadContext {
    fn new() -> Self {
        Self {
            resources_path: None,
            folder: None,
        }
    }

    fn with_resources_path(resources_path: PathBuf) -> Self {
        Self {
            resources_path: Some(resources_path),
            folder: None,
        }
    }

    /// Get the path to a rule file (PNG or VOX).
    fn rule_path(&self, name: &str, is_2d: bool) -> Option<PathBuf> {
        let resources = self.resources_path.as_ref()?;
        let mut path = resources.join("rules");
        if let Some(ref folder) = self.folder {
            path = path.join(folder);
        }
        path = path.join(name);
        let extension = if is_2d { "png" } else { "vox" };
        Some(path.with_extension(extension))
    }

    /// Get the path to a sample image for WFC overlap model.
    fn sample_path(&self, name: &str) -> Option<PathBuf> {
        let resources = self.resources_path.as_ref()?;
        Some(resources.join("samples").join(name).with_extension("png"))
    }

    /// Get the path to a tileset XML file.
    ///
    /// C# Reference: TileModel.cs line 24
    /// `string filepath = $"resources/tilesets/{name}.xml";`
    fn tileset_xml_path(&self, name: &str) -> Option<PathBuf> {
        let resources = self.resources_path.as_ref()?;
        Some(resources.join("tilesets").join(format!("{}.xml", name)))
    }
}

/// Load a model from an XML file.
pub fn load_model(path: &Path) -> Result<LoadedModel, LoadError> {
    let content = std::fs::read_to_string(path)
        .map_err(|_| LoadError::FileNotFound(path.display().to_string()))?;

    // Determine resources path from file location
    // Typically: models/Foo.xml -> ../resources
    let resources_path = path
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("resources"));

    let ctx = match resources_path {
        Some(p) if p.exists() => LoadContext::with_resources_path(p),
        _ => LoadContext::new(),
    };

    load_model_str_with_context(&content, 0, 0, 0, ctx)
}

/// Load a model from an XML string.
///
/// If mx, my, mz are 0, uses defaults (typically from models.xml or 16x16x1).
pub fn load_model_str(
    xml: &str,
    mx: usize,
    my: usize,
    mz: usize,
) -> Result<LoadedModel, LoadError> {
    load_model_str_with_context(xml, mx, my, mz, LoadContext::new())
}

/// Load a model from an XML string with explicit resources path.
pub fn load_model_str_with_resources(
    xml: &str,
    mx: usize,
    my: usize,
    mz: usize,
    resources_path: PathBuf,
) -> Result<LoadedModel, LoadError> {
    load_model_str_with_context(
        xml,
        mx,
        my,
        mz,
        LoadContext::with_resources_path(resources_path),
    )
}

/// Internal: Load a model with full context.
fn load_model_str_with_context(
    xml: &str,
    mx: usize,
    my: usize,
    mz: usize,
    ctx: LoadContext,
) -> Result<LoadedModel, LoadError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // Find the root element
    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                return load_root_element(e, xml, mx, my, mz, &ctx);
            }
            Ok(Event::Eof) => {
                return Err(LoadError::XmlError("empty document".to_string()));
            }
            Err(e) => {
                return Err(LoadError::XmlError(format!("{}", e)));
            }
            _ => {} // Skip comments, declarations, etc.
        }
    }
}

/// Load the root element of the model.
fn load_root_element(
    elem: &BytesStart,
    xml: &str,
    mx: usize,
    my: usize,
    mz: usize,
    ctx: &LoadContext,
) -> Result<LoadedModel, LoadError> {
    let elem_name = std::str::from_utf8(elem.name().as_ref())
        .map_err(|e| LoadError::XmlError(format!("invalid UTF-8 in element name: {}", e)))?
        .to_string();

    // Parse attributes
    let attrs = parse_attributes(elem)?;

    // Get values string (required)
    let values_str = attrs
        .get("values")
        .ok_or_else(|| LoadError::MissingAttribute {
            element: elem_name.clone(),
            attribute: "values".to_string(),
        })?;

    // Get origin flag
    let origin = attrs
        .get("origin")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // Determine grid size
    let mx = if mx > 0 { mx } else { 16 };
    let my = if my > 0 { my } else { 16 };
    let mz = if mz > 0 { mz } else { 1 };

    // Create grid
    let mut grid = MjGrid::try_with_values(mx, my, mz, values_str)
        .map_err(|e| LoadError::GridError(format!("{}", e)))?;

    // Parse <union> elements to add combined wave types
    parse_union_elements(xml, &mut grid)?;

    // Update context with folder if specified
    let mut ctx = ctx.clone();
    if let Some(folder) = attrs.get("folder") {
        ctx.folder = Some(folder.clone());
    }

    // Get symmetry for root
    let is_2d = mz == 1;
    let default_symmetry = get_default_symmetry(is_2d);
    let symmetry = if let Some(sym_str) = attrs.get("symmetry") {
        get_symmetry(is_2d, sym_str)?
    } else {
        default_symmetry.clone()
    };

    // Check if this is a simple rule node (one/all/prl with inline rule)
    let node: Box<dyn Node> = if NODE_NAMES.contains(&elem_name.as_str()) {
        // Load as a node
        load_node_from_xml(xml, &elem_name, &attrs, &grid, &symmetry, &ctx)?
    } else {
        return Err(LoadError::UnknownNodeType(elem_name));
    };

    // Wrap in MarkovNode if not already a Branch
    let root = match elem_name.as_str() {
        "markov" | "sequence" => node,
        _ => Box::new(MarkovNode::new(vec![node])),
    };

    Ok(LoadedModel { root, grid, origin })
}

/// Parse a node from XML string given its name and attributes.
fn load_node_from_xml(
    xml: &str,
    name: &str,
    attrs: &HashMap<String, String>,
    grid: &MjGrid,
    parent_symmetry: &[bool],
    ctx: &LoadContext,
) -> Result<Box<dyn Node>, LoadError> {
    // Get node-specific symmetry or inherit from parent
    let is_2d = grid.mz == 1;
    let symmetry = if let Some(sym_str) = attrs.get("symmetry") {
        get_symmetry(is_2d, sym_str)?
    } else {
        parent_symmetry.to_vec()
    };

    match name {
        "one" | "all" | "prl" => {
            // Rule node - parse rules
            let rules = load_rules_from_attrs_and_children(xml, attrs, grid, &symmetry, ctx)?;
            let grid_size = grid.state.len();
            let steps = attrs.get("steps").and_then(|s| s.parse().ok()).unwrap_or(0);
            let temperature = attrs
                .get("temperature")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            // Parse search attributes
            let search = attrs
                .get("search")
                .map(|s| s.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let limit = attrs
                .get("limit")
                .and_then(|s| s.parse().ok())
                .unwrap_or(-1i32);
            let depth_coefficient = attrs
                .get("depthCoefficient")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            // Parse field elements
            let fields = load_fields_from_xml(xml, grid)?;

            // Parse observe elements
            let observations = load_observations_from_xml(xml, grid)?;

            // Create node with appropriate data
            let mut data = if fields.is_some() {
                let mut data =
                    RuleNodeData::with_fields(rules, grid_size, fields.unwrap(), temperature);
                data.steps = steps;
                data
            } else {
                let mut data = RuleNodeData::new(rules, grid_size);
                data.steps = steps;
                data.temperature = temperature;
                data
            };

            // Add observations if present
            if let Some(obs) = observations {
                data.set_observations(obs, grid_size);
            }

            // Configure search
            if search {
                data.set_search(search, limit, depth_coefficient);
            }

            match name {
                "one" => Ok(Box::new(OneNode { data })),
                "all" => Ok(Box::new(AllNode { data })),
                "prl" => Ok(Box::new(ParallelNode::with_data(data))),
                _ => unreachable!(),
            }
        }
        "path" => {
            // PathNode - parse path attributes
            load_path_node(attrs, grid)
        }
        "markov" | "sequence" => {
            // Branch node - parse children
            let children = load_children_from_xml(xml, grid, &symmetry, ctx)?;
            match name {
                "markov" => Ok(Box::new(MarkovNode::new(children))),
                "sequence" => Ok(Box::new(SequenceNode::new(children))),
                _ => unreachable!(),
            }
        }
        "map" => {
            // MapNode - grid transformation
            load_map_node(xml, attrs, grid, &symmetry, ctx)
        }
        "wfc" => {
            // WFC node - either OverlapNode (sample) or TileNode (tileset)
            load_wfc_node(xml, attrs, grid, &symmetry, ctx)
        }
        "convolution" => {
            // Convolution node - cellular automata rules
            load_convolution_node(xml, attrs, grid)
        }
        "convchain" => {
            // ConvChain node - MCMC texture synthesis
            load_convchain_node(attrs, grid, parent_symmetry, ctx)
        }
        _ => Err(LoadError::UnknownNodeType(name.to_string())),
    }
}

/// Load a PathNode from XML attributes.
///
/// C# Reference: Path.cs Load() lines 14-26
fn load_path_node(
    attrs: &HashMap<String, String>,
    grid: &MjGrid,
) -> Result<Box<dyn Node>, LoadError> {
    // Required: from (start), to (finish), on (substrate)
    let from_str = attrs
        .get("from")
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "path".to_string(),
            attribute: "from".to_string(),
        })?;
    let to_str = attrs.get("to").ok_or_else(|| LoadError::MissingAttribute {
        element: "path".to_string(),
        attribute: "to".to_string(),
    })?;
    let on_str = attrs.get("on").ok_or_else(|| LoadError::MissingAttribute {
        element: "path".to_string(),
        attribute: "on".to_string(),
    })?;

    let start = grid.wave(from_str);
    let finish = grid.wave(to_str);
    let substrate = grid.wave(on_str);

    // Color to write - default to first char of 'from'
    let color_char = attrs
        .get("color")
        .and_then(|s| s.chars().next())
        .unwrap_or_else(|| from_str.chars().next().unwrap_or('B'));

    let value =
        grid.values
            .get(&color_char)
            .copied()
            .ok_or_else(|| LoadError::UnknownCharacter {
                character: color_char,
                context: "path color".to_string(),
            })?;

    let mut node = PathNode::new(start, finish, substrate, value);

    // Optional attributes
    node.inertia = attrs
        .get("inertia")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    node.longest = attrs
        .get("longest")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    node.edges = attrs
        .get("edges")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    node.vertices = attrs
        .get("vertices")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    Ok(Box::new(node))
}

/// Load a MapNode from XML.
///
/// MapNode transforms the grid by applying mapping rules at a scaled resolution.
///
/// C# Reference: Map.cs Load() lines 12-57
fn load_map_node(
    xml: &str,
    attrs: &HashMap<String, String>,
    grid: &MjGrid,
    parent_symmetry: &[bool],
    ctx: &LoadContext,
) -> Result<Box<dyn Node>, LoadError> {
    // Required: scale attribute like "2 2 1" or "1/2 1/2 1"
    let scale_str = attrs
        .get("scale")
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "map".to_string(),
            attribute: "scale".to_string(),
        })?;

    // Parse scale factors
    let scale_parts: Vec<&str> = scale_str.split_whitespace().collect();
    if scale_parts.len() != 3 {
        return Err(LoadError::InvalidAttribute {
            element: "map".to_string(),
            attribute: "scale".to_string(),
            value: scale_str.clone(),
            reason: "expected 3 components separated by space".to_string(),
        });
    }

    let scale_x =
        ScaleFactor::parse(scale_parts[0]).ok_or_else(|| LoadError::InvalidAttribute {
            element: "map".to_string(),
            attribute: "scale".to_string(),
            value: scale_parts[0].to_string(),
            reason: "invalid scale factor".to_string(),
        })?;
    let scale_y =
        ScaleFactor::parse(scale_parts[1]).ok_or_else(|| LoadError::InvalidAttribute {
            element: "map".to_string(),
            attribute: "scale".to_string(),
            value: scale_parts[1].to_string(),
            reason: "invalid scale factor".to_string(),
        })?;
    let scale_z =
        ScaleFactor::parse(scale_parts[2]).ok_or_else(|| LoadError::InvalidAttribute {
            element: "map".to_string(),
            attribute: "scale".to_string(),
            value: scale_parts[2].to_string(),
            reason: "invalid scale factor".to_string(),
        })?;

    // Calculate new grid dimensions
    let new_mx = scale_x.apply(grid.mx);
    let new_my = scale_y.apply(grid.my);
    let new_mz = scale_z.apply(grid.mz);

    // Get values for new grid (optional, defaults to same as parent)
    let new_values = attrs.get("values").map(|s| s.as_str()).unwrap_or_else(|| {
        // Use parent grid's character string
        ""
    });

    // Create new grid
    let mut newgrid = if new_values.is_empty() {
        // Clone structure from parent grid
        let mut g = MjGrid::new(new_mx, new_my, new_mz);
        g.c = grid.c;
        g.characters = grid.characters.clone();
        g.values = grid.values.clone();
        g.waves = grid.waves.clone();
        g
    } else {
        MjGrid::try_with_values(new_mx, new_my, new_mz, new_values)
            .map_err(|e| LoadError::GridError(format!("{}", e)))?
    };

    // Parse union elements for the new grid
    parse_union_elements(xml, &mut newgrid)?;

    // Get symmetry for rules
    let is_2d = grid.mz == 1;
    let symmetry = if let Some(sym_str) = attrs.get("symmetry") {
        get_symmetry(is_2d, sym_str)?
    } else {
        parent_symmetry.to_vec()
    };

    // Load map rules - these map from input grid patterns to output grid patterns
    let rules = load_map_rules_from_xml(xml, grid, &newgrid, &symmetry, ctx)?;

    // Load child nodes (operate on the new grid)
    let children = load_children_from_xml(xml, &newgrid, &symmetry, ctx)?;

    let node = MapNode::new(newgrid, rules, scale_x, scale_y, scale_z).with_children(children);
    Ok(Box::new(node))
}

/// Load a WFC node from XML.
///
/// WFC nodes come in two variants:
/// - OverlapNode: Uses `sample` attribute to load a sample image and extract NxN patterns
/// - TileNode: Uses `tileset` attribute to load a tileset with neighbor constraints
///
/// C# Reference: Node.Factory() lines 35-36
fn load_wfc_node(
    xml: &str,
    attrs: &HashMap<String, String>,
    grid: &MjGrid,
    parent_symmetry: &[bool],
    ctx: &LoadContext,
) -> Result<Box<dyn Node>, LoadError> {
    let is_2d = grid.mz == 1;

    // Get symmetry
    let symmetry = if let Some(sym_str) = attrs.get("symmetry") {
        get_symmetry(is_2d, sym_str)?
    } else {
        parent_symmetry.to_vec()
    };

    // Check if this is overlap model or tile model
    if let Some(sample_name) = attrs.get("sample") {
        // OverlapNode - pattern-based WFC from sample image
        load_overlap_node(xml, attrs, sample_name, grid, &symmetry, ctx)
    } else if let Some(tileset_name) = attrs.get("tileset") {
        // TileNode - tile-based WFC from tileset
        load_tile_node(xml, attrs, tileset_name, grid, &symmetry, ctx)
    } else {
        Err(LoadError::MissingAttribute {
            element: "wfc".to_string(),
            attribute: "sample or tileset".to_string(),
        })
    }
}

/// Load an OverlapNode from XML.
///
/// C# Reference: OverlapNode.Load() (OverlapModel.cs lines 12-133)
fn load_overlap_node(
    xml: &str,
    attrs: &HashMap<String, String>,
    sample_name: &str,
    grid: &MjGrid,
    symmetry: &[bool],
    ctx: &LoadContext,
) -> Result<Box<dyn Node>, LoadError> {
    // Overlap model only works for 2D
    if grid.mz != 1 {
        return Err(LoadError::InvalidAttribute {
            element: "wfc".to_string(),
            attribute: "sample".to_string(),
            value: sample_name.to_string(),
            reason: "overlapping model currently works only for 2d".to_string(),
        });
    }

    // Get attributes
    let n = attrs.get("n").and_then(|s| s.parse().ok()).unwrap_or(3);
    let periodic_input = attrs
        .get("periodicInput")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(true);
    let periodic = attrs
        .get("periodic")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(true);
    let shannon = attrs
        .get("shannon")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let tries = attrs
        .get("tries")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    // Get sample path
    let sample_path = ctx.sample_path(sample_name).ok_or_else(|| {
        LoadError::ResourceError(format!(
            "no resources path configured to load sample '{}'",
            sample_name
        ))
    })?;

    // Create the output grid for WFC
    // Use values attribute if present, otherwise inherit from parent grid
    let newgrid = if let Some(values_str) = attrs.get("values") {
        MjGrid::try_with_values(grid.mx, grid.my, grid.mz, values_str)
            .map_err(|e| LoadError::GridError(format!("{}", e)))?
    } else {
        // Clone structure from parent grid
        let mut g = MjGrid::new(grid.mx, grid.my, grid.mz);
        g.c = grid.c;
        g.characters = grid.characters.clone();
        g.values = grid.values.clone();
        g.waves = grid.waves.clone();
        g
    };

    // Parse <rule> elements for input->output mappings
    let rules = load_wfc_rules_from_xml(xml, grid, &newgrid)?;

    // Load child nodes that execute after WFC completes
    // C# Reference: WFCNode extends Branch, which parses children
    let children = load_children_from_xml(xml, &newgrid, symmetry, ctx)?;

    // Create OverlapNode
    let node = OverlapNode::from_sample(
        &sample_path,
        n,
        periodic_input,
        periodic,
        shannon,
        tries,
        symmetry,
        newgrid.clone(),
        grid,
        &rules,
    )
    .map_err(|e| LoadError::ResourceError(e))?
    .with_children(children);

    Ok(Box::new(node))
}

/// Load a TileNode from XML.
///
/// C# Reference: TileNode.Load() (TileModel.cs lines 9-188)
fn load_tile_node(
    xml: &str,
    attrs: &HashMap<String, String>,
    tileset_name: &str,
    grid: &MjGrid,
    parent_symmetry: &[bool],
    ctx: &LoadContext,
) -> Result<Box<dyn Node>, LoadError> {
    // Get tileset XML path
    // C# Reference: TileModel.cs line 24: `string filepath = $"resources/tilesets/{name}.xml";`
    let tileset_xml_path = ctx.tileset_xml_path(tileset_name).ok_or_else(|| {
        LoadError::ResourceError(format!(
            "no resources path configured to load tileset '{}'",
            tileset_name
        ))
    })?;

    // Get attributes
    let periodic = attrs
        .get("periodic")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(true);
    let shannon = attrs
        .get("shannon")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let tries = attrs
        .get("tries")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    // Get overlap (can be negative in C# but we use 0 as default)
    let overlap = attrs
        .get("overlap")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0)
        .max(0) as usize;
    let overlapz = attrs
        .get("overlapz")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0)
        .max(0) as usize;

    // Get tiles subdirectory name (defaults to tileset name)
    let tiles_name = attrs
        .get("tiles")
        .map(|s| s.as_str())
        .unwrap_or(tileset_name);

    // Get full_symmetry flag
    let full_symmetry = attrs
        .get("fullSymmetry")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // Create the output grid for WFC
    // Use values attribute if present, otherwise inherit from parent grid
    let newgrid = if let Some(values_str) = attrs.get("values") {
        MjGrid::try_with_values(grid.mx, grid.my, grid.mz, values_str)
            .map_err(|e| LoadError::GridError(format!("{}", e)))?
    } else {
        // Clone structure from parent grid
        let mut g = MjGrid::new(grid.mx, grid.my, grid.mz);
        g.c = grid.c;
        g.characters = grid.characters.clone();
        g.values = grid.values.clone();
        g.waves = grid.waves.clone();
        g
    };

    // Parse rules for tile model
    // Rules map input grid values to allowed tile names
    let rules = load_tile_rules_from_xml(xml, grid)?;

    // Load child nodes that execute after WFC completes
    // C# Reference: WFCNode extends Branch, which parses children
    let children = load_children_from_xml(xml, &newgrid, parent_symmetry, ctx)?;

    // Create TileNode
    let node = TileNode::from_tileset(
        &tileset_xml_path,
        tiles_name,
        periodic,
        shannon,
        tries,
        overlap,
        overlapz,
        newgrid.clone(),
        grid,
        &rules,
        full_symmetry,
    )
    .map_err(|e| LoadError::ResourceError(e))?
    .with_children(children);

    Ok(Box::new(node))
}

/// Load a ConvolutionNode from XML.
///
/// Convolution nodes apply cellular automata rules based on neighbor counts.
///
/// C# Reference: Convolution.cs Load() lines 27-45
fn load_convolution_node(
    xml: &str,
    attrs: &HashMap<String, String>,
    grid: &MjGrid,
) -> Result<Box<dyn Node>, LoadError> {
    // Get neighborhood kernel name
    let neighborhood = attrs
        .get("neighborhood")
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "convolution".to_string(),
            attribute: "neighborhood".to_string(),
        })?;

    // Get periodic flag
    let periodic = attrs
        .get("periodic")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // Get steps limit
    let steps = attrs
        .get("steps")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0usize);

    // Parse convolution rules
    let rules = load_convolution_rules_from_xml(xml, grid)?;

    if rules.is_empty() {
        return Err(LoadError::RuleError(
            "no rules found in convolution node".to_string(),
        ));
    }

    // Create the node with appropriate kernel
    let is_2d = grid.mz == 1;
    let grid_size = grid.state.len();
    let num_colors = grid.c as usize;

    let mut node = if is_2d {
        ConvolutionNode::with_2d_kernel(rules, neighborhood, periodic, grid_size, num_colors)
            .ok_or_else(|| LoadError::InvalidAttribute {
                element: "convolution".to_string(),
                attribute: "neighborhood".to_string(),
                value: neighborhood.clone(),
                reason: "unknown 2D kernel name (expected VonNeumann or Moore)".to_string(),
            })?
    } else {
        ConvolutionNode::with_3d_kernel(rules, neighborhood, periodic, grid_size, num_colors)
            .ok_or_else(|| LoadError::InvalidAttribute {
                element: "convolution".to_string(),
                attribute: "neighborhood".to_string(),
                value: neighborhood.clone(),
                reason: "unknown 3D kernel name (expected VonNeumann or NoCorners)".to_string(),
            })?
    };

    if steps > 0 {
        node = node.with_steps(steps);
    }

    Ok(Box::new(node))
}

/// Load a ConvChainNode from XML.
///
/// ConvChain performs MCMC texture synthesis using patterns learned from a sample image.
///
/// C# Reference: ConvChain.cs Load() lines 20-58
fn load_convchain_node(
    attrs: &HashMap<String, String>,
    grid: &MjGrid,
    symmetry: &[bool],
    ctx: &LoadContext,
) -> Result<Box<dyn Node>, LoadError> {
    // ConvChain only works for 2D
    if grid.mz != 1 {
        return Err(LoadError::InvalidAttribute {
            element: "convchain".to_string(),
            attribute: "".to_string(),
            value: "".to_string(),
            reason: "convchain currently works only for 2d".to_string(),
        });
    }

    // Required: sample (name of sample image)
    let sample_name = attrs
        .get("sample")
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "convchain".to_string(),
            attribute: "sample".to_string(),
        })?;

    // Required: on (substrate color to synthesize on)
    let on_char = attrs
        .get("on")
        .and_then(|s| s.chars().next())
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "convchain".to_string(),
            attribute: "on".to_string(),
        })?;

    let substrate_color =
        grid.values
            .get(&on_char)
            .copied()
            .ok_or_else(|| LoadError::UnknownCharacter {
                character: on_char,
                context: "convchain on".to_string(),
            })?;

    // Required: black and white colors
    let black_char = attrs
        .get("black")
        .and_then(|s| s.chars().next())
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "convchain".to_string(),
            attribute: "black".to_string(),
        })?;

    let c0 = grid
        .values
        .get(&black_char)
        .copied()
        .ok_or_else(|| LoadError::UnknownCharacter {
            character: black_char,
            context: "convchain black".to_string(),
        })?;

    let white_char = attrs
        .get("white")
        .and_then(|s| s.chars().next())
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "convchain".to_string(),
            attribute: "white".to_string(),
        })?;

    let c1 = grid
        .values
        .get(&white_char)
        .copied()
        .ok_or_else(|| LoadError::UnknownCharacter {
            character: white_char,
            context: "convchain white".to_string(),
        })?;

    // Optional attributes
    let n = attrs.get("n").and_then(|s| s.parse().ok()).unwrap_or(3);
    let temperature = attrs
        .get("temperature")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let steps = attrs
        .get("steps")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(-1);

    // Get sample path
    let sample_path = ctx.sample_path(sample_name).ok_or_else(|| {
        LoadError::ResourceError(format!(
            "no resources path configured to load sample '{}'",
            sample_name
        ))
    })?;

    // Create ConvChainNode from sample
    let grid_size = grid.state.len();
    let mut node = ConvChainNode::from_sample(
        &sample_path,
        n,
        temperature,
        c0,
        c1,
        substrate_color,
        grid_size,
        symmetry,
    )
    .map_err(|e| LoadError::ResourceError(e))?;

    // Set steps if specified (steps > 0)
    if steps > 0 {
        node = node.with_steps(steps as usize);
    }

    Ok(Box::new(node))
}

/// Load convolution rules from XML.
///
/// C# Reference: Convolution.cs ConvolutionRule.Load() lines 146-188
fn load_convolution_rules_from_xml(
    xml: &str,
    grid: &MjGrid,
) -> Result<Vec<ConvolutionRule>, LoadError> {
    let mut rules = Vec::new();

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0;

    // Check if the root element itself has in/out attributes (inline rule)
    let mut has_root_rule = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = get_element_name(e)?;
                if depth == 0 && name == "convolution" {
                    // Check for inline rule on the convolution element
                    let attrs = parse_attributes(e)?;
                    if attrs.contains_key("in") && attrs.contains_key("out") {
                        if let Some(rule) = parse_convolution_rule_element(e, grid)? {
                            rules.push(rule);
                            has_root_rule = true;
                        }
                    }
                } else if depth == 1 && name == "rule" {
                    if let Some(rule) = parse_convolution_rule_element(e, grid)? {
                        rules.push(rule);
                    }
                }
                depth += 1;
            }
            Ok(Event::Empty(ref e)) => {
                let name = get_element_name(e)?;
                if depth == 0 && name == "convolution" {
                    // Check for inline rule on self-closing convolution element
                    let attrs = parse_attributes(e)?;
                    if attrs.contains_key("in") && attrs.contains_key("out") {
                        if let Some(rule) = parse_convolution_rule_element(e, grid)? {
                            rules.push(rule);
                            has_root_rule = true;
                        }
                    }
                } else if depth == 1 && name == "rule" {
                    if let Some(rule) = parse_convolution_rule_element(e, grid)? {
                        rules.push(rule);
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LoadError::XmlError(format!("{}", e))),
            _ => {}
        }
    }

    // If we only found child rules, that's fine
    // If we only found root rule, that's fine
    // If we found neither, return empty (caller handles error)
    let _ = has_root_rule; // Suppress warning

    Ok(rules)
}

/// Parse a convolution rule element.
///
/// C# Reference: Convolution.cs ConvolutionRule.Load() lines 146-188
fn parse_convolution_rule_element(
    e: &BytesStart,
    grid: &MjGrid,
) -> Result<Option<ConvolutionRule>, LoadError> {
    let attrs = parse_attributes(e)?;

    // Required: in (input value)
    let in_str = match attrs.get("in") {
        Some(s) => s,
        None => return Ok(None),
    };

    // Required: out (output value)
    let out_str = match attrs.get("out") {
        Some(s) => s,
        None => return Ok(None),
    };

    // Get input value
    let in_char = in_str
        .chars()
        .next()
        .ok_or_else(|| LoadError::InvalidAttribute {
            element: "rule".to_string(),
            attribute: "in".to_string(),
            value: in_str.clone(),
            reason: "empty input".to_string(),
        })?;

    let input = grid
        .values
        .get(&in_char)
        .copied()
        .ok_or_else(|| LoadError::UnknownCharacter {
            character: in_char,
            context: "convolution rule input".to_string(),
        })?;

    // Get output value
    let out_char = out_str
        .chars()
        .next()
        .ok_or_else(|| LoadError::InvalidAttribute {
            element: "rule".to_string(),
            attribute: "out".to_string(),
            value: out_str.clone(),
            reason: "empty output".to_string(),
        })?;

    let output =
        grid.values
            .get(&out_char)
            .copied()
            .ok_or_else(|| LoadError::UnknownCharacter {
                character: out_char,
                context: "convolution rule output".to_string(),
            })?;

    // Get probability
    let p = attrs.get("p").and_then(|s| s.parse().ok()).unwrap_or(1.0);

    // Check for sum constraint
    let values_str = attrs.get("values");
    let sum_str = attrs.get("sum");

    let rule = match (values_str, sum_str) {
        (Some(vals), Some(sums)) => {
            // Parse values - convert each char to its index
            let values: Vec<u8> = vals
                .chars()
                .filter_map(|c| grid.values.get(&c).copied())
                .collect();

            // Parse sum intervals
            let sums = ConvolutionRule::parse_sum_intervals(sums);

            ConvolutionRule::with_sums(input, output, values, sums).with_probability(p)
        }
        (None, None) => ConvolutionRule::new(input, output).with_probability(p),
        (Some(_), None) => {
            return Err(LoadError::MissingAttribute {
                element: "rule".to_string(),
                attribute: "sum".to_string(),
            });
        }
        (None, Some(_)) => {
            return Err(LoadError::MissingAttribute {
                element: "rule".to_string(),
                attribute: "values".to_string(),
            });
        }
    };

    Ok(Some(rule))
}

/// Load tile rules from XML.
///
/// Tile rules map input grid values to allowed tile names.
/// Format: `<rule in="B" out="Path|Wall"/>` means input value B allows tiles Path or Wall.
fn load_tile_rules_from_xml(
    xml: &str,
    input_grid: &MjGrid,
) -> Result<Vec<(u8, Vec<String>)>, LoadError> {
    let mut rules = Vec::new();

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = get_element_name(e)?;
                if (depth == 0 || depth == 1) && name == "rule" {
                    if let Some(rule) = parse_tile_rule_element(e, input_grid)? {
                        rules.push(rule);
                    }
                }
                depth += 1;
            }
            Ok(Event::Empty(ref e)) => {
                let name = get_element_name(e)?;
                if (depth == 0 || depth == 1) && name == "rule" {
                    if let Some(rule) = parse_tile_rule_element(e, input_grid)? {
                        rules.push(rule);
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LoadError::XmlError(format!("{}", e))),
            _ => {}
        }
    }

    Ok(rules)
}

/// Parse a tile rule element.
///
/// Returns (input_value, allowed_tile_names) or None if not a tile-style rule.
fn parse_tile_rule_element(
    e: &BytesStart,
    input_grid: &MjGrid,
) -> Result<Option<(u8, Vec<String>)>, LoadError> {
    let attrs = parse_attributes(e)?;

    // Tile rules have "in" (single char) and "out" (pipe-separated tile names)
    let in_str = match attrs.get("in") {
        Some(s) => s,
        None => return Ok(None),
    };
    let out_str = match attrs.get("out") {
        Some(s) => s,
        None => return Ok(None),
    };

    // Get input value
    let in_char = in_str
        .chars()
        .next()
        .ok_or_else(|| LoadError::InvalidAttribute {
            element: "rule".to_string(),
            attribute: "in".to_string(),
            value: in_str.clone(),
            reason: "empty input".to_string(),
        })?;

    let input_value =
        input_grid
            .values
            .get(&in_char)
            .copied()
            .ok_or_else(|| LoadError::UnknownCharacter {
                character: in_char,
                context: "tile rule input".to_string(),
            })?;

    // Parse pipe-separated tile names
    let tile_names: Vec<String> = out_str.split('|').map(|s| s.to_string()).collect();

    Ok(Some((input_value, tile_names)))
}

/// Load WFC rules from XML.
///
/// WFC rules map input grid values to allowed output pattern values.
/// Format: `<rule in="B" out="N|g"/>` means input value B allows output colors N or g.
///
/// C# Reference: OverlapNode.Load() lines 122-129
fn load_wfc_rules_from_xml(
    xml: &str,
    input_grid: &MjGrid,
    output_grid: &MjGrid,
) -> Result<Vec<(u8, Vec<u8>)>, LoadError> {
    let mut rules = Vec::new();

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = get_element_name(e)?;
                if (depth == 0 || depth == 1) && name == "rule" {
                    if let Some(rule) = parse_wfc_rule_element(e, input_grid, output_grid)? {
                        rules.push(rule);
                    }
                }
                depth += 1;
            }
            Ok(Event::Empty(ref e)) => {
                let name = get_element_name(e)?;
                if (depth == 0 || depth == 1) && name == "rule" {
                    if let Some(rule) = parse_wfc_rule_element(e, input_grid, output_grid)? {
                        rules.push(rule);
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LoadError::XmlError(format!("{}", e))),
            _ => {}
        }
    }

    Ok(rules)
}

/// Parse a WFC rule element.
///
/// Returns (input_value, allowed_output_values) or None if not a WFC-style rule.
fn parse_wfc_rule_element(
    e: &BytesStart,
    input_grid: &MjGrid,
    output_grid: &MjGrid,
) -> Result<Option<(u8, Vec<u8>)>, LoadError> {
    let attrs = parse_attributes(e)?;

    // WFC rules have "in" (single char) and "out" (pipe-separated chars)
    let in_str = match attrs.get("in") {
        Some(s) => s,
        None => return Ok(None), // Not a WFC rule
    };
    let out_str = match attrs.get("out") {
        Some(s) => s,
        None => return Ok(None), // Not a WFC rule
    };

    // Check if "out" contains pipe separator (WFC style) or is a pattern (rewrite style)
    if !out_str.contains('|') && out_str.len() > 1 {
        // This is a rewrite rule, not a WFC rule
        return Ok(None);
    }

    // Get input value
    let in_char = in_str
        .chars()
        .next()
        .ok_or_else(|| LoadError::InvalidAttribute {
            element: "rule".to_string(),
            attribute: "in".to_string(),
            value: in_str.clone(),
            reason: "empty input".to_string(),
        })?;

    let input_value =
        input_grid
            .values
            .get(&in_char)
            .copied()
            .ok_or_else(|| LoadError::UnknownCharacter {
                character: in_char,
                context: "WFC rule input".to_string(),
            })?;

    // Parse pipe-separated output values
    let mut output_values = Vec::new();
    for part in out_str.split('|') {
        let out_char = part
            .chars()
            .next()
            .ok_or_else(|| LoadError::InvalidAttribute {
                element: "rule".to_string(),
                attribute: "out".to_string(),
                value: out_str.clone(),
                reason: "empty output part".to_string(),
            })?;

        let output_value = output_grid.values.get(&out_char).copied().ok_or_else(|| {
            LoadError::UnknownCharacter {
                character: out_char,
                context: "WFC rule output".to_string(),
            }
        })?;

        output_values.push(output_value);
    }

    Ok(Some((input_value, output_values)))
}

/// Load rules for a MapNode.
///
/// Map rules can have different input and output grids, so we handle them specially.
fn load_map_rules_from_xml(
    xml: &str,
    input_grid: &MjGrid,
    output_grid: &MjGrid,
    symmetry: &[bool],
    ctx: &LoadContext,
) -> Result<Vec<MjRule>, LoadError> {
    let mut all_rules = Vec::new();

    // Parse child <rule> elements
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = get_element_name(e)?;
                if (depth == 0 || depth == 1) && name == "rule" {
                    parse_map_rule_element(
                        e,
                        input_grid,
                        output_grid,
                        symmetry,
                        &mut all_rules,
                        ctx,
                    )?;
                }
                depth += 1;
            }
            Ok(Event::Empty(ref e)) => {
                let name = get_element_name(e)?;
                if (depth == 0 || depth == 1) && name == "rule" {
                    parse_map_rule_element(
                        e,
                        input_grid,
                        output_grid,
                        symmetry,
                        &mut all_rules,
                        ctx,
                    )?;
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LoadError::XmlError(format!("{}", e))),
            _ => {}
        }
    }

    if all_rules.is_empty() {
        return Err(LoadError::RuleError(
            "no rules found in map node".to_string(),
        ));
    }

    Ok(all_rules)
}

/// Parse a rule element for MapNode (different input/output grids).
fn parse_map_rule_element(
    e: &BytesStart,
    input_grid: &MjGrid,
    output_grid: &MjGrid,
    parent_symmetry: &[bool],
    rules: &mut Vec<MjRule>,
    _ctx: &LoadContext,
) -> Result<(), LoadError> {
    let rule_attrs = parse_attributes(e)?;
    let is_2d = input_grid.mz == 1;

    // For map rules, we need in/out attributes
    let in_str = rule_attrs
        .get("in")
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "rule".to_string(),
            attribute: "in".to_string(),
        })?;
    let out_str = rule_attrs
        .get("out")
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "rule".to_string(),
            attribute: "out".to_string(),
        })?;
    let p = rule_attrs
        .get("p")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);

    // Rule-specific symmetry
    let rule_symmetry = if let Some(sym_str) = rule_attrs.get("symmetry") {
        get_symmetry(is_2d, sym_str)?
    } else {
        parent_symmetry.to_vec()
    };

    // Parse input pattern using input_grid's waves
    let base_rule = parse_map_rule(in_str, out_str, input_grid, output_grid)?;

    let rule_with_p = MjRule { p, ..base_rule };
    let variants = apply_symmetry(rule_with_p, &rule_symmetry, is_2d);
    rules.extend(variants);

    Ok(())
}

/// Parse a map rule with different input and output grids.
fn parse_map_rule(
    input_str: &str,
    output_str: &str,
    input_grid: &MjGrid,
    output_grid: &MjGrid,
) -> Result<MjRule, LoadError> {
    // Parse input pattern
    let (in_chars, imx, imy, imz) = parse_pattern_string(input_str)?;

    // Parse output pattern
    let (out_chars, omx, omy, omz) = parse_pattern_string(output_str)?;

    // Convert input chars to wave bitmasks using input_grid
    let mut input = Vec::with_capacity(in_chars.len());
    for &ch in &in_chars {
        let wave =
            input_grid
                .waves
                .get(&ch)
                .copied()
                .ok_or_else(|| LoadError::UnknownCharacter {
                    character: ch,
                    context: "map rule input pattern".to_string(),
                })?;
        input.push(wave);
    }

    // Convert output chars to byte values using output_grid
    let mut output = Vec::with_capacity(out_chars.len());
    for &ch in &out_chars {
        if ch == '*' {
            output.push(0xff);
        } else {
            let value = output_grid.values.get(&ch).copied().ok_or_else(|| {
                LoadError::UnknownCharacter {
                    character: ch,
                    context: "map rule output pattern".to_string(),
                }
            })?;
            output.push(value);
        }
    }

    Ok(MjRule::from_patterns(
        input,
        imx,
        imy,
        imz,
        output,
        omx,
        omy,
        omz,
        output_grid.c,
        1.0,
    ))
}

/// Parse a pattern string into characters and dimensions.
/// This is similar to MjRule::parse_pattern but returns chars instead of waves.
fn parse_pattern_string(s: &str) -> Result<(Vec<char>, usize, usize, usize), LoadError> {
    if s.is_empty() {
        return Err(LoadError::RuleError("empty pattern".to_string()));
    }

    // Split by space for Z layers, then by / for Y rows
    let layers: Vec<&str> = s.split(' ').collect();
    let mz = layers.len();

    // Determine dimensions from first layer
    let first_rows: Vec<&str> = layers[0].split('/').collect();
    let my = first_rows.len();
    let mx = if !first_rows.is_empty() {
        first_rows[0].chars().count()
    } else {
        return Err(LoadError::RuleError("empty pattern".to_string()));
    };

    // Pre-allocate result array
    let mut result = vec![' '; mx * my * mz];

    // Process layers with Z reversal to match C#
    for z in 0..mz {
        let layer = layers[mz - 1 - z];
        let rows: Vec<&str> = layer.split('/').collect();

        if rows.len() != my {
            return Err(LoadError::RuleError("non-rectangular pattern".to_string()));
        }

        for (y, row) in rows.iter().enumerate() {
            if row.chars().count() != mx {
                return Err(LoadError::RuleError("non-rectangular pattern".to_string()));
            }

            for (x, ch) in row.chars().enumerate() {
                let idx = x + y * mx + z * mx * my;
                result[idx] = ch;
            }
        }
    }

    Ok((result, mx, my, mz))
}

/// Load field elements from XML.
///
/// Returns None if no fields found, Some(Vec) otherwise.
///
/// C# Reference: RuleNode.Load() lines 65-80
fn load_fields_from_xml(xml: &str, grid: &MjGrid) -> Result<Option<Vec<Option<Field>>>, LoadError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut fields: Vec<Option<Field>> = vec![None; grid.c as usize];
    let mut found_any = false;

    let mut depth = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = get_element_name(e)?;
                if depth <= 1 && name == "field" {
                    parse_field_element(e, grid, &mut fields)?;
                    found_any = true;
                }
                depth += 1;
            }
            Ok(Event::Empty(ref e)) => {
                let name = get_element_name(e)?;
                if depth <= 1 && name == "field" {
                    parse_field_element(e, grid, &mut fields)?;
                    found_any = true;
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LoadError::XmlError(format!("{}", e))),
            _ => {}
        }
    }

    Ok(if found_any { Some(fields) } else { None })
}

/// Parse a single <field> element.
///
/// C# Reference: Field constructor lines 12-23
fn parse_field_element(
    e: &BytesStart,
    grid: &MjGrid,
    fields: &mut [Option<Field>],
) -> Result<(), LoadError> {
    let attrs = parse_attributes(e)?;

    // Required: for (which color this field is for)
    let for_char = attrs
        .get("for")
        .and_then(|s| s.chars().next())
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "field".to_string(),
            attribute: "for".to_string(),
        })?;

    let color_idx =
        grid.values
            .get(&for_char)
            .copied()
            .ok_or_else(|| LoadError::UnknownCharacter {
                character: for_char,
                context: "field for".to_string(),
            })?;

    // Required: on (substrate to traverse)
    let on_str = attrs.get("on").ok_or_else(|| LoadError::MissingAttribute {
        element: "field".to_string(),
        attribute: "on".to_string(),
    })?;
    let substrate = grid.wave(on_str);

    // Zero cells: either "from" (inversed) or "to"
    let (zero, inversed) = if let Some(from_str) = attrs.get("from") {
        (grid.wave(from_str), true)
    } else if let Some(to_str) = attrs.get("to") {
        (grid.wave(to_str), false)
    } else {
        return Err(LoadError::MissingAttribute {
            element: "field".to_string(),
            attribute: "to (or from)".to_string(),
        });
    };

    let recompute = attrs
        .get("recompute")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let essential = attrs
        .get("essential")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut field = Field::new(substrate, zero);
    field.recompute = recompute;
    field.inversed = inversed;
    field.essential = essential;

    fields[color_idx as usize] = Some(field);

    Ok(())
}

/// Load observe elements from XML.
///
/// Returns None if no observations found, Some(Vec) otherwise.
/// The vector is indexed by color value: observations[value] = Some(Observation) or None.
///
/// C# Reference: RuleNode.Load() lines 85-95
fn load_observations_from_xml(
    xml: &str,
    grid: &MjGrid,
) -> Result<Option<Vec<Option<Observation>>>, LoadError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut observations: Vec<Option<Observation>> = vec![None; grid.c as usize];
    let mut found_any = false;

    let mut depth = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = get_element_name(e)?;
                if depth <= 1 && name == "observe" {
                    parse_observe_element(e, grid, &mut observations)?;
                    found_any = true;
                }
                depth += 1;
            }
            Ok(Event::Empty(ref e)) => {
                let name = get_element_name(e)?;
                if depth <= 1 && name == "observe" {
                    parse_observe_element(e, grid, &mut observations)?;
                    found_any = true;
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LoadError::XmlError(format!("{}", e))),
            _ => {}
        }
    }

    Ok(if found_any { Some(observations) } else { None })
}

/// Parse a single <observe> element.
///
/// C# Reference: RuleNode.cs line 88-89
/// `observations[value] = new Observation(x.Get("from", grid.characters[value]), x.Get<string>("to"), grid);`
///
/// The `from` attribute is optional and defaults to the same character as `value`.
fn parse_observe_element(
    e: &BytesStart,
    grid: &MjGrid,
    observations: &mut [Option<Observation>],
) -> Result<(), LoadError> {
    let attrs = parse_attributes(e)?;

    // Required: value (which color value triggers this observation)
    let value_char = attrs
        .get("value")
        .and_then(|s| s.chars().next())
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "observe".to_string(),
            attribute: "value".to_string(),
        })?;

    let value_idx =
        grid.values
            .get(&value_char)
            .copied()
            .ok_or_else(|| LoadError::UnknownCharacter {
                character: value_char,
                context: "observe value".to_string(),
            })?;

    // Optional: from (what value gets placed initially)
    // Defaults to value_char if not specified
    // C# Reference: x.Get("from", grid.characters[value])
    let from_char = attrs
        .get("from")
        .and_then(|s| s.chars().next())
        .unwrap_or(value_char);

    // Required: to (what values are allowed in the goal)
    let to_str = attrs.get("to").ok_or_else(|| LoadError::MissingAttribute {
        element: "observe".to_string(),
        attribute: "to".to_string(),
    })?;

    let obs =
        Observation::new(from_char, to_str, grid).ok_or_else(|| LoadError::UnknownCharacter {
            character: from_char,
            context: "observe from".to_string(),
        })?;

    observations[value_idx as usize] = Some(obs);

    Ok(())
}

/// Parse <union> elements and add them to the grid's waves.
///
/// Union elements define combined wave types that match multiple values.
/// For example: `<union symbol="?" values="BR"/>` creates a wave that matches B or R.
///
/// C# Reference: Grid.cs Load() lines 59-74
fn parse_union_elements(xml: &str, grid: &mut MjGrid) -> Result<(), LoadError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = get_element_name(e)?;
                if name == "union" {
                    let attrs = parse_attributes(e)?;

                    // Get the symbol character
                    let symbol = attrs
                        .get("symbol")
                        .and_then(|s| s.chars().next())
                        .ok_or_else(|| LoadError::MissingAttribute {
                            element: "union".to_string(),
                            attribute: "symbol".to_string(),
                        })?;

                    // Check for duplicate
                    if grid.waves.contains_key(&symbol) {
                        return Err(LoadError::InvalidAttribute {
                            element: "union".to_string(),
                            attribute: "symbol".to_string(),
                            value: symbol.to_string(),
                            reason: "symbol already defined".to_string(),
                        });
                    }

                    // Get the values to combine
                    let values_str =
                        attrs
                            .get("values")
                            .ok_or_else(|| LoadError::MissingAttribute {
                                element: "union".to_string(),
                                attribute: "values".to_string(),
                            })?;

                    // Create combined wave
                    let wave = grid.wave(values_str);
                    grid.waves.insert(symbol, wave);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LoadError::XmlError(format!("{}", e))),
            _ => {}
        }
    }

    Ok(())
}

/// Load child nodes from XML.
fn load_children_from_xml(
    xml: &str,
    grid: &MjGrid,
    parent_symmetry: &[bool],
    ctx: &LoadContext,
) -> Result<Vec<Box<dyn Node>>, LoadError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut children = Vec::new();
    let mut depth = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                if depth == 2 {
                    // Child of root
                    let name = get_element_name(e)?;

                    if NODE_NAMES.contains(&name.as_str()) {
                        let attrs = parse_attributes(e)?;
                        let child_xml = read_element_content(&mut reader)?;
                        let full_xml = format!(
                            "<{}{}>{}</{}>",
                            name,
                            attrs_to_string(&attrs),
                            child_xml,
                            name
                        );
                        let node = load_node_from_xml(
                            &full_xml,
                            &name,
                            &attrs,
                            grid,
                            parent_symmetry,
                            ctx,
                        )?;
                        children.push(node);
                        // read_element_content consumed the End event, so decrement depth
                        depth -= 1;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                if depth == 1 {
                    // Child of root (self-closing)
                    let name = get_element_name(e)?;

                    if NODE_NAMES.contains(&name.as_str()) {
                        let attrs = parse_attributes(e)?;
                        let node = load_node_from_xml(
                            &format!("<{}{}/>", name, attrs_to_string(&attrs)),
                            &name,
                            &attrs,
                            grid,
                            parent_symmetry,
                            ctx,
                        )?;
                        children.push(node);
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LoadError::XmlError(format!("{}", e))),
            _ => {}
        }
    }

    Ok(children)
}

/// Read element content as a string (for nested parsing).
fn read_element_content(reader: &mut Reader<&[u8]>) -> Result<String, LoadError> {
    let mut content = String::new();
    let mut depth = 1;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                let name = get_element_name(e)?;
                content.push_str(&format!("<{}", name));
                for attr in e.attributes().flatten() {
                    let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                    let value = std::str::from_utf8(&attr.value).unwrap_or("");
                    content.push_str(&format!(" {}=\"{}\"", key, value));
                }
                content.push('>');
            }
            Ok(Event::Empty(ref e)) => {
                let name = get_element_name(e)?;
                content.push_str(&format!("<{}", name));
                for attr in e.attributes().flatten() {
                    let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                    let value = std::str::from_utf8(&attr.value).unwrap_or("");
                    content.push_str(&format!(" {}=\"{}\"", key, value));
                }
                content.push_str("/>");
            }
            Ok(Event::End(ref e)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                let name = get_end_element_name(e)?;
                content.push_str(&format!("</{}>", name));
            }
            Ok(Event::Text(ref e)) => {
                content.push_str(
                    &e.unescape()
                        .map_err(|err| LoadError::XmlError(format!("invalid text: {}", err)))?,
                );
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LoadError::XmlError(format!("{}", e))),
            _ => {}
        }
    }

    Ok(content)
}

/// Get element name as String from BytesStart.
fn get_element_name(e: &BytesStart) -> Result<String, LoadError> {
    std::str::from_utf8(e.name().as_ref())
        .map_err(|err| LoadError::XmlError(format!("invalid UTF-8: {}", err)))
        .map(|s| s.to_string())
}

/// Get element name as String from BytesEnd.
fn get_end_element_name(e: &quick_xml::events::BytesEnd) -> Result<String, LoadError> {
    std::str::from_utf8(e.name().as_ref())
        .map_err(|err| LoadError::XmlError(format!("invalid UTF-8: {}", err)))
        .map(|s| s.to_string())
}

/// Convert attributes to string for reconstruction.
fn attrs_to_string(attrs: &HashMap<String, String>) -> String {
    let mut s = String::new();
    for (k, v) in attrs {
        s.push_str(&format!(" {}=\"{}\"", k, v));
    }
    s
}

/// Load rules from attributes (inline) and child <rule> elements.
fn load_rules_from_attrs_and_children(
    xml: &str,
    attrs: &HashMap<String, String>,
    grid: &MjGrid,
    symmetry: &[bool],
    ctx: &LoadContext,
) -> Result<Vec<MjRule>, LoadError> {
    let mut all_rules = Vec::new();
    let is_2d = grid.mz == 1;

    // Check for file attribute (PNG/VOX rule)
    if let Some(file_name) = attrs.get("file") {
        let legend = attrs
            .get("legend")
            .ok_or_else(|| LoadError::MissingAttribute {
                element: "rule node".to_string(),
                attribute: "legend".to_string(),
            })?;
        let p = attrs.get("p").and_then(|s| s.parse().ok()).unwrap_or(1.0);

        let base_rule = load_rule_from_file(file_name, legend, grid, ctx, is_2d)?;
        let rule_with_p = MjRule { p, ..base_rule };
        let variants = apply_symmetry(rule_with_p, symmetry, is_2d);
        all_rules.extend(variants);
    }
    // Check for inline rule (in="..." out="..." on the element itself)
    else if let (Some(in_str), Some(out_str)) = (attrs.get("in"), attrs.get("out")) {
        let p = attrs.get("p").and_then(|s| s.parse().ok()).unwrap_or(1.0);

        let base_rule = MjRule::parse(in_str, out_str, grid)
            .map_err(|e| LoadError::RuleError(format!("{}", e)))?;

        // Apply symmetry
        let rule_with_p = MjRule { p, ..base_rule };
        let variants = apply_symmetry(rule_with_p, symmetry, is_2d);
        all_rules.extend(variants);
    }

    // Parse child <rule> elements
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = get_element_name(e)?;
                if (depth == 0 || depth == 1) && name == "rule" {
                    parse_rule_element(e, grid, symmetry, &mut all_rules, ctx)?;
                }
                depth += 1;
            }
            Ok(Event::Empty(ref e)) => {
                let name = get_element_name(e)?;
                if (depth == 0 || depth == 1) && name == "rule" {
                    parse_rule_element(e, grid, symmetry, &mut all_rules, ctx)?;
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LoadError::XmlError(format!("{}", e))),
            _ => {}
        }
    }

    if all_rules.is_empty() {
        return Err(LoadError::RuleError(
            "no rules found in rule node".to_string(),
        ));
    }

    Ok(all_rules)
}

/// Load a rule from a PNG/VOX file.
///
/// C# Reference: Rule.cs Load() lines 223-248 (file attribute branch)
fn load_rule_from_file(
    file_name: &str,
    legend: &str,
    grid: &MjGrid,
    ctx: &LoadContext,
    is_2d: bool,
) -> Result<MjRule, LoadError> {
    // Get the file path
    let path = ctx.rule_path(file_name, is_2d).ok_or_else(|| {
        LoadError::ResourceError(format!(
            "no resources path configured to load file '{}'",
            file_name
        ))
    })?;

    // Load the resource and convert to characters
    let (chars, full_mx, my, mz) = load_resource(&path, legend, is_2d)?;

    // Split into input and output halves
    let (in_chars, out_chars, mx, my, mz) = split_rule_image(&chars, full_mx, my, mz)?;

    // Convert input chars to wave bitmasks
    let mut input = Vec::with_capacity(in_chars.len());
    for &ch in &in_chars {
        let wave = grid
            .waves
            .get(&ch)
            .copied()
            .ok_or_else(|| LoadError::UnknownCharacter {
                character: ch,
                context: format!("input pattern from file '{}'", file_name),
            })?;
        input.push(wave);
    }

    // Convert output chars to byte values
    let mut output = Vec::with_capacity(out_chars.len());
    for &ch in &out_chars {
        if ch == '*' {
            output.push(0xff);
        } else {
            let value =
                grid.values
                    .get(&ch)
                    .copied()
                    .ok_or_else(|| LoadError::UnknownCharacter {
                        character: ch,
                        context: format!("output pattern from file '{}'", file_name),
                    })?;
            output.push(value);
        }
    }

    Ok(MjRule::from_patterns(
        input, mx, my, mz, output, mx, my, mz, grid.c, 1.0,
    ))
}

/// Parse a <rule> element and add its variants to the rules list.
fn parse_rule_element(
    e: &BytesStart,
    grid: &MjGrid,
    parent_symmetry: &[bool],
    rules: &mut Vec<MjRule>,
    ctx: &LoadContext,
) -> Result<(), LoadError> {
    let rule_attrs = parse_attributes(e)?;
    let is_2d = grid.mz == 1;

    // Check for file attribute first
    if let Some(file_name) = rule_attrs.get("file") {
        let legend = rule_attrs
            .get("legend")
            .ok_or_else(|| LoadError::MissingAttribute {
                element: "rule".to_string(),
                attribute: "legend".to_string(),
            })?;
        let p = rule_attrs
            .get("p")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);

        // Rule-specific symmetry
        let rule_symmetry = if let Some(sym_str) = rule_attrs.get("symmetry") {
            get_symmetry(is_2d, sym_str)?
        } else {
            parent_symmetry.to_vec()
        };

        let base_rule = load_rule_from_file(file_name, legend, grid, ctx, is_2d)?;
        let rule_with_p = MjRule { p, ..base_rule };
        let variants = apply_symmetry(rule_with_p, &rule_symmetry, is_2d);
        rules.extend(variants);
        return Ok(());
    }

    // Otherwise expect in/out attributes
    let in_str = rule_attrs
        .get("in")
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "rule".to_string(),
            attribute: "in".to_string(),
        })?;
    let out_str = rule_attrs
        .get("out")
        .ok_or_else(|| LoadError::MissingAttribute {
            element: "rule".to_string(),
            attribute: "out".to_string(),
        })?;
    let p = rule_attrs
        .get("p")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);

    // Rule-specific symmetry
    let rule_symmetry = if let Some(sym_str) = rule_attrs.get("symmetry") {
        get_symmetry(grid.mz == 1, sym_str)?
    } else {
        parent_symmetry.to_vec()
    };

    let base_rule =
        MjRule::parse(in_str, out_str, grid).map_err(|e| LoadError::RuleError(format!("{}", e)))?;

    let rule_with_p = MjRule { p, ..base_rule };
    let variants = apply_symmetry(rule_with_p, &rule_symmetry, grid.mz == 1);
    rules.extend(variants);

    Ok(())
}

/// Apply symmetry transformations to a rule.
///
/// For 2D, uses square symmetries (8 transformations).
/// For 3D, uses cube symmetries (48 transformations).
///
/// C# Reference: SymmetryHelper.cs, Rule.cs uses symmetry in Load()
fn apply_symmetry(rule: MjRule, symmetry: &[bool], is_2d: bool) -> Vec<MjRule> {
    if is_2d {
        let subgroup = bool_slice_to_subgroup(symmetry);
        square_symmetries(&rule, Some(subgroup))
    } else {
        // 3D symmetry uses cube symmetries (48 transformations)
        // Convert bool slice to [bool; 48] array for cube_symmetries
        if symmetry.len() >= 48 {
            let mut subgroup = [false; 48];
            for (i, &b) in symmetry.iter().take(48).enumerate() {
                subgroup[i] = b;
            }
            cube_symmetries(&rule, Some(&subgroup))
        } else if symmetry.iter().all(|&b| b) || symmetry.is_empty() {
            // All symmetries enabled or empty (default to all)
            cube_symmetries(&rule, None)
        } else {
            // Partial symmetry specified but not full 48 - use as-is with padding
            let mut subgroup = [false; 48];
            for (i, &b) in symmetry.iter().enumerate() {
                if i < 48 {
                    subgroup[i] = b;
                }
            }
            cube_symmetries(&rule, Some(&subgroup))
        }
    }
}

/// Convert a bool slice to SquareSubgroup.
fn bool_slice_to_subgroup(symmetry: &[bool]) -> SquareSubgroup {
    if symmetry.len() < 8 {
        return SquareSubgroup::All;
    }

    // Check known patterns
    if symmetry == [true, false, false, false, false, false, false, false] {
        SquareSubgroup::None
    } else if symmetry == [true, true, false, false, false, false, false, false] {
        SquareSubgroup::ReflectX
    } else if symmetry == [true, false, false, false, false, true, false, false] {
        SquareSubgroup::ReflectY
    } else if symmetry == [true, true, false, false, true, true, false, false] {
        SquareSubgroup::ReflectXY
    } else if symmetry == [true, false, true, false, true, false, true, false] {
        SquareSubgroup::Rotate
    } else if symmetry.iter().all(|&b| b) {
        SquareSubgroup::All
    } else {
        SquareSubgroup::All // Default to all if pattern not recognized
    }
}

/// Get default symmetry (all transformations).
fn get_default_symmetry(is_2d: bool) -> Vec<bool> {
    if is_2d {
        vec![true; 8]
    } else {
        vec![true; 48]
    }
}

/// Get symmetry from string name.
///
/// C# Reference: SymmetryHelper.cs lines 9-16 (squareSubgroups) and 37-46 (cubeSubgroups)
fn get_symmetry(is_2d: bool, name: &str) -> Result<Vec<bool>, LoadError> {
    if is_2d {
        // C# Reference: SymmetryHelper.cs lines 9-16 (squareSubgroups)
        match name {
            "()" => Ok(vec![true, false, false, false, false, false, false, false]),
            "(x)" => Ok(vec![true, true, false, false, false, false, false, false]),
            "(y)" => Ok(vec![true, false, false, false, false, true, false, false]),
            "(x)(y)" => Ok(vec![true, true, false, false, true, true, false, false]),
            "(xy+)" => Ok(vec![true, false, true, false, true, false, true, false]),
            "(xy)" => Ok(vec![true; 8]),
            _ => Err(LoadError::UnknownSymmetry(name.to_string())),
        }
    } else {
        // C# Reference: SymmetryHelper.cs lines 37-46 (cubeSubgroups)
        match name {
            // ["()"] = AH.Array1D(48, l => l == 0)
            "()" => {
                let mut v = vec![false; 48];
                v[0] = true;
                Ok(v)
            }
            // ["(x)"] = AH.Array1D(48, l => l == 0 || l == 1)
            "(x)" => {
                let mut v = vec![false; 48];
                v[0] = true;
                v[1] = true;
                Ok(v)
            }
            // ["(z)"] = AH.Array1D(48, l => l == 0 || l == 17)
            "(z)" => {
                let mut v = vec![false; 48];
                v[0] = true;
                v[17] = true;
                Ok(v)
            }
            // ["(xy)"] = AH.Array1D(48, l => l < 8)
            "(xy)" => {
                let mut v = vec![false; 48];
                for i in 0..8 {
                    v[i] = true;
                }
                Ok(v)
            }
            // ["(xyz+)"] = AH.Array1D(48, l => l % 2 == 0)
            "(xyz+)" => {
                let mut v = vec![false; 48];
                for i in 0..48 {
                    if i % 2 == 0 {
                        v[i] = true;
                    }
                }
                Ok(v)
            }
            // ["(xyz)"] = AH.Array1D(48, true)
            "(xyz)" => Ok(vec![true; 48]),
            _ => Err(LoadError::UnknownSymmetry(name.to_string())),
        }
    }
}

/// Parse attributes from an XML element into a HashMap.
fn parse_attributes(elem: &BytesStart) -> Result<HashMap<String, String>, LoadError> {
    let mut attrs = HashMap::new();
    for attr_result in elem.attributes() {
        let attr =
            attr_result.map_err(|e| LoadError::XmlError(format!("attribute error: {}", e)))?;
        let key = std::str::from_utf8(attr.key.as_ref())
            .map_err(|e| LoadError::XmlError(format!("invalid UTF-8 in attribute key: {}", e)))?
            .to_string();
        let value = std::str::from_utf8(&attr.value)
            .map_err(|e| LoadError::XmlError(format!("invalid UTF-8 in attribute value: {}", e)))?
            .to_string();
        attrs.insert(key, value);
    }
    Ok(attrs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn models_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MarkovJunior/models")
    }

    #[test]
    fn test_load_basic_xml() {
        let path = models_path().join("Basic.xml");
        let result = load_model(&path);
        assert!(result.is_ok(), "Failed to load Basic.xml: {:?}", result);

        let model = result.unwrap();
        assert_eq!(model.grid.c, 2, "Should have 2 values (B, W)");
        assert!(model.grid.values.contains_key(&'B'));
        assert!(model.grid.values.contains_key(&'W'));
        assert!(!model.origin, "Basic.xml should not have origin=true");
    }

    #[test]
    fn test_load_growth_xml() {
        let path = models_path().join("Growth.xml");
        let result = load_model(&path);
        assert!(result.is_ok(), "Failed to load Growth.xml: {:?}", result);

        let model = result.unwrap();
        assert_eq!(model.grid.c, 2, "Should have 2 values (B, W)");
        assert!(model.origin, "Growth.xml should have origin=true");
    }

    #[test]
    fn test_load_backtracker_xml() {
        let path = models_path().join("Backtracker.xml");
        let result = load_model(&path);
        assert!(
            result.is_ok(),
            "Failed to load Backtracker.xml: {:?}",
            result
        );

        let model = result.unwrap();
        assert_eq!(model.grid.c, 4, "Should have 4 values (B, R, W, U)");
        assert!(model.origin, "Backtracker.xml should have origin=true");
    }

    #[test]
    fn test_load_maze_growth_xml() {
        let path = models_path().join("MazeGrowth.xml");
        let result = load_model(&path);
        assert!(
            result.is_ok(),
            "Failed to load MazeGrowth.xml: {:?}",
            result
        );

        let model = result.unwrap();
        assert_eq!(model.grid.c, 3, "Should have 3 values (B, W, A)");
        assert!(model.origin, "MazeGrowth.xml should have origin=true");
    }

    #[test]
    fn test_load_missing_file_returns_error() {
        let path = PathBuf::from("nonexistent.xml");
        let result = load_model(&path);
        assert!(matches!(result, Err(LoadError::FileNotFound(_))));
    }

    #[test]
    fn test_load_model_str_basic() {
        let xml = r#"<one values="BW" in="B" out="W"/>"#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(result.is_ok(), "Failed to load inline XML: {:?}", result);

        let model = result.unwrap();
        assert_eq!(model.grid.c, 2);
        assert_eq!(model.grid.mx, 10);
        assert_eq!(model.grid.my, 10);
    }

    #[test]
    fn test_symmetry_parsing() {
        // Test different symmetry values
        let sym_none = get_symmetry(true, "()").unwrap();
        assert_eq!(sym_none[0], true);
        assert!(sym_none[1..].iter().all(|&b| !b));

        let sym_all = get_symmetry(true, "(xy)").unwrap();
        assert!(sym_all.iter().all(|&b| b));

        let sym_x = get_symmetry(true, "(x)").unwrap();
        assert_eq!(sym_x[0], true);
        assert_eq!(sym_x[1], true);
        assert!(sym_x[2..].iter().all(|&b| !b));
    }

    /// Test that 3D symmetry `(xy)` is supported.
    ///
    /// C# Reference: SymmetryHelper.cs line 42
    /// `["(xy)"] = AH.Array1D(48, l => l < 8)`
    ///
    /// `(xy)` symmetry in 3D means "square symmetries in XY plane" -
    /// the first 8 of 48 cube symmetries (rotations around Z axis + X reflection).
    ///
    /// This is required for models like Apartemazements.xml which use:
    /// `<sequence values="BWN" symmetry="(xy)">`
    #[test]
    fn test_3d_symmetry_xy_is_supported() {
        // 3D symmetry (xy) should return first 8 indices as true
        let result = get_symmetry(false, "(xy)");
        assert!(
            result.is_ok(),
            "3D symmetry (xy) should be supported but got: {:?}",
            result
        );

        let sym = result.unwrap();
        assert_eq!(sym.len(), 48, "3D symmetry should have 48 elements");

        // First 8 should be true (C#: l < 8)
        for i in 0..8 {
            assert!(sym[i], "3D (xy) symmetry index {} should be true", i);
        }

        // Rest should be false
        for i in 8..48 {
            assert!(!sym[i], "3D (xy) symmetry index {} should be false", i);
        }
    }

    /// Test that 3D symmetry `(x)` is supported.
    ///
    /// C# Reference: SymmetryHelper.cs line 40
    /// `["(x)"] = AH.Array1D(48, l => l == 0 || l == 1)`
    #[test]
    fn test_3d_symmetry_x_is_supported() {
        let result = get_symmetry(false, "(x)");
        assert!(
            result.is_ok(),
            "3D symmetry (x) should be supported but got: {:?}",
            result
        );

        let sym = result.unwrap();
        assert_eq!(sym.len(), 48);

        // Only indices 0 and 1 should be true
        assert!(sym[0], "Index 0 should be true");
        assert!(sym[1], "Index 1 should be true");
        for i in 2..48 {
            assert!(!sym[i], "Index {} should be false", i);
        }
    }

    /// Test that 3D symmetry `(z)` is supported.
    ///
    /// C# Reference: SymmetryHelper.cs line 41
    /// `["(z)"] = AH.Array1D(48, l => l == 0 || l == 17)`
    #[test]
    fn test_3d_symmetry_z_is_supported() {
        let result = get_symmetry(false, "(z)");
        assert!(
            result.is_ok(),
            "3D symmetry (z) should be supported but got: {:?}",
            result
        );

        let sym = result.unwrap();
        assert_eq!(sym.len(), 48);

        // Only indices 0 and 17 should be true
        assert!(sym[0], "Index 0 should be true");
        assert!(sym[17], "Index 17 should be true");
        for i in 1..48 {
            if i != 17 {
                assert!(!sym[i], "Index {} should be false", i);
            }
        }
    }

    /// Test that 3D symmetry `(xyz+)` is supported.
    ///
    /// C# Reference: SymmetryHelper.cs line 43
    /// `["(xyz+)"] = AH.Array1D(48, l => l % 2 == 0)`
    ///
    /// This is all 24 rotations (even indices only, no reflections).
    #[test]
    fn test_3d_symmetry_xyz_plus_is_supported() {
        let result = get_symmetry(false, "(xyz+)");
        assert!(
            result.is_ok(),
            "3D symmetry (xyz+) should be supported but got: {:?}",
            result
        );

        let sym = result.unwrap();
        assert_eq!(sym.len(), 48);

        // Even indices should be true, odd should be false
        for i in 0..48 {
            if i % 2 == 0 {
                assert!(sym[i], "Even index {} should be true", i);
            } else {
                assert!(!sym[i], "Odd index {} should be false", i);
            }
        }
    }

    /// Test that Apartemazements.xml can be loaded in 3D mode.
    ///
    /// This model requires:
    /// 1. 3D `(xy)` symmetry support (C# SymmetryHelper.cs line 42)
    /// 2. Correct tileset path resolution (C# TileModel.cs line 24)
    #[test]
    fn test_load_apartemazements_xml_3d() {
        let path = models_path().join("Apartemazements.xml");
        let content = std::fs::read_to_string(&path).expect("Failed to read Apartemazements.xml");

        // Determine resources path
        let resources_path = path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("resources"))
            .expect("Could not find resources path");

        // Load with 3D dimensions (8x8x8 as used in reference)
        let result = load_model_str_with_resources(&content, 8, 8, 8, resources_path);

        // Should load successfully
        assert!(
            result.is_ok(),
            "Failed to load Apartemazements.xml: {:?}",
            result
        );

        let model = result.unwrap();
        assert_eq!(model.grid.c, 3, "Should have 3 values (B, W, N)");
        assert_eq!(model.grid.mz, 8, "Should be 3D with mz=8");
    }

    #[test]
    fn test_parse_attributes() {
        let xml = r#"<test foo="bar" baz="123"/>"#;
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        if let Ok(Event::Empty(ref e)) = reader.read_event() {
            let attrs = parse_attributes(e).unwrap();
            assert_eq!(attrs.get("foo"), Some(&"bar".to_string()));
            assert_eq!(attrs.get("baz"), Some(&"123".to_string()));
        } else {
            panic!("Expected Empty event");
        }
    }

    #[test]
    fn test_load_biased_growth_xml() {
        // BiasedGrowth.xml uses <field> elements
        let path = models_path().join("BiasedGrowth.xml");
        let result = load_model(&path);
        assert!(
            result.is_ok(),
            "Failed to load BiasedGrowth.xml: {:?}",
            result
        );

        let model = result.unwrap();
        assert_eq!(model.grid.c, 4, "Should have 4 values (N, W, S, B)");
        assert!(model.origin, "BiasedGrowth.xml should have origin=true");
    }

    #[test]
    fn test_load_model_with_field() {
        // Test XML with field element
        let xml = r#"
        <one values="BW" temperature="10.0">
            <rule in="B" out="W"/>
            <field for="W" to="B" on="W"/>
        </one>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(
            result.is_ok(),
            "Failed to load XML with field: {:?}",
            result
        );
    }

    #[test]
    fn test_load_model_with_path() {
        // Test XML with path node inside sequence
        let xml = r#"
        <sequence values="BSGP">
            <path from="S" to="G" on="B" color="P" inertia="True"/>
        </sequence>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(result.is_ok(), "Failed to load XML with path: {:?}", result);
    }

    #[test]
    fn test_load_basic_dijkstra_dungeon() {
        // BasicDijkstraDungeon.xml uses file attribute for PNG rules
        let path = models_path().join("BasicDijkstraDungeon.xml");
        let result = load_model(&path);
        assert!(
            result.is_ok(),
            "Failed to load BasicDijkstraDungeon.xml: {:?}",
            result
        );

        let model = result.unwrap();
        assert_eq!(model.grid.c, 3, "Should have 3 values (B, G, W)");
    }

    #[test]
    fn test_load_file_rule_from_png() {
        // Test loading a rule from PNG file via inline XML
        use super::load_model_str_with_resources;

        let resources_path = models_path().parent().unwrap().join("resources");

        let xml = r#"<one values="BW" file="BasicDijkstraRoom" legend="BW"/>"#;
        let result = load_model_str_with_resources(xml, 16, 16, 1, resources_path);
        assert!(
            result.is_ok(),
            "Failed to load model with file attribute: {:?}",
            result
        );
    }

    #[test]
    fn test_union_creates_combined_wave() {
        // Test that <union> element creates a combined wave
        let xml = r#"
        <sequence values="BRGW">
            <union symbol="?" values="BR"/>
            <one in="?" out="W"/>
        </sequence>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(
            result.is_ok(),
            "Failed to load XML with union: {:?}",
            result
        );

        let model = result.unwrap();
        // Union '?' should match B (0b0001) | R (0b0010) = 0b0011 = 3
        assert_eq!(model.grid.waves.get(&'?'), Some(&3));
    }

    #[test]
    fn test_load_dungeon_growth_xml() {
        // DungeonGrowth.xml uses <union> and file attributes
        let path = models_path().join("DungeonGrowth.xml");
        let result = load_model(&path);
        assert!(
            result.is_ok(),
            "Failed to load DungeonGrowth.xml: {:?}",
            result
        );

        let model = result.unwrap();
        assert_eq!(model.grid.c, 6, "Should have 6 values (W, R, B, U, P, Y)");
        // Should have union '?' = B | R
        assert!(
            model.grid.waves.contains_key(&'?'),
            "Should have union type '?'"
        );
        // B = 0b000100 = 4, R = 0b000010 = 2 -> BR = 0b000110 = 6
        assert_eq!(model.grid.waves.get(&'?'), Some(&6));
    }

    #[test]
    fn test_union_duplicate_symbol_error() {
        // Test that duplicate union symbol returns error
        let xml = r#"
        <sequence values="BW">
            <union symbol="B" values="BW"/>
            <one in="B" out="W"/>
        </sequence>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(result.is_err(), "Should fail with duplicate symbol error");
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("already defined"),
            "Error should mention already defined: {}",
            err
        );
    }

    #[test]
    fn test_load_map_node() {
        // Test loading a model with <map> node
        let xml = r#"
        <sequence values="BW">
            <all in="B" out="W"/>
            <map scale="2 2 1" values="RG">
                <rule in="W" out="RG/GR"/>
            </map>
        </sequence>
        "#;
        let result = load_model_str(xml, 4, 4, 1);
        assert!(
            result.is_ok(),
            "Failed to load model with map: {:?}",
            result
        );
    }

    #[test]
    fn test_load_maze_map_xml() {
        // MazeMap.xml uses <map> node
        let path = models_path().join("MazeMap.xml");
        let result = load_model(&path);
        assert!(result.is_ok(), "Failed to load MazeMap.xml: {:?}", result);

        let model = result.unwrap();
        assert_eq!(model.grid.c, 5, "Should have 5 values (B, O, E, I, N)");
    }

    #[test]
    fn test_map_node_scale_parsing() {
        // Test various scale formats
        use super::ScaleFactor;

        let sf1 = ScaleFactor::parse("2").unwrap();
        assert_eq!(sf1.apply(10), 20);

        let sf2 = ScaleFactor::parse("1/2").unwrap();
        assert_eq!(sf2.apply(10), 5);

        let sf3 = ScaleFactor::parse("3/2").unwrap();
        assert_eq!(sf3.apply(10), 15);
    }

    #[test]
    fn test_load_model_with_observe() {
        // Test XML with observe element
        let xml = r#"
        <one values="BWR">
            <rule in="B" out="W"/>
            <observe value="R" from="B" to="W"/>
        </one>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(
            result.is_ok(),
            "Failed to load XML with observe: {:?}",
            result
        );
    }

    #[test]
    fn test_load_model_with_search() {
        // Test XML with search attributes
        let xml = r#"
        <one values="BW" search="True" limit="1000" depthCoefficient="0.5">
            <rule in="B" out="W"/>
            <observe value="W" from="B" to="W"/>
        </one>
        "#;
        let result = load_model_str(xml, 5, 5, 1);
        assert!(
            result.is_ok(),
            "Failed to load XML with search: {:?}",
            result
        );
    }

    // WFC Tests

    #[test]
    fn test_load_wfc_overlap_wave_flowers() {
        // WaveFlowers.xml uses WFC overlap model with sample image
        let path = models_path().join("WaveFlowers.xml");
        let result = load_model(&path);
        assert!(
            result.is_ok(),
            "Failed to load WaveFlowers.xml: {:?}",
            result
        );

        let model = result.unwrap();
        // Outer sequence has values "BW"
        assert_eq!(model.grid.c, 2, "Should have 2 values (B, W)");
    }

    #[test]
    fn test_load_wfc_overlap_inline() {
        // Test inline WFC overlap model
        use super::load_model_str_with_resources;

        let resources_path = models_path().parent().unwrap().join("resources");

        let xml = r#"
        <wfc values="zYgN" sample="Flowers" n="3" periodic="True" shannon="True"/>
        "#;
        let result = load_model_str_with_resources(xml, 16, 16, 1, resources_path);
        assert!(
            result.is_ok(),
            "Failed to load inline WFC overlap: {:?}",
            result
        );
    }

    #[test]
    fn test_load_wfc_with_rules() {
        // Test WFC overlap model with rule constraints
        use super::load_model_str_with_resources;

        let resources_path = models_path().parent().unwrap().join("resources");

        let xml = r#"
        <sequence values="BW">
            <wfc sample="Flowers" values="zYgN" n="3" periodic="True">
                <rule in="B" out="N|g"/>
                <rule in="W" out="z|Y"/>
            </wfc>
        </sequence>
        "#;
        let result = load_model_str_with_resources(xml, 16, 16, 1, resources_path);
        assert!(
            result.is_ok(),
            "Failed to load WFC with rules: {:?}",
            result
        );
    }

    #[test]
    fn test_wfc_overlap_missing_sample_error() {
        // Test that missing sample attribute gives error
        let xml = r#"<wfc values="BW" n="3"/>"#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("sample or tileset"),
            "Error should mention missing sample or tileset: {}",
            err
        );
    }

    #[test]
    fn test_wfc_3d_overlap_not_supported() {
        // Test that 3D overlap model gives error (not yet supported)
        use super::load_model_str_with_resources;

        let resources_path = models_path().parent().unwrap().join("resources");

        let xml = r#"
        <wfc values="BW" sample="Flowers" n="3"/>
        "#;
        let result = load_model_str_with_resources(xml, 10, 10, 2, resources_path);
        assert!(result.is_err(), "3D overlap should not be supported yet");
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("2d"),
            "Error should mention 2d restriction: {}",
            err
        );
    }

    // Convolution Tests

    #[test]
    fn test_load_convolution_cave_xml() {
        // Cave.xml uses convolution rules
        let path = models_path().join("Cave.xml");
        let result = load_model(&path);
        assert!(result.is_ok(), "Failed to load Cave.xml: {:?}", result);

        let model = result.unwrap();
        assert_eq!(model.grid.c, 2, "Should have 2 values (D, A)");
        assert!(model.grid.values.contains_key(&'D'));
        assert!(model.grid.values.contains_key(&'A'));
    }

    #[test]
    fn test_load_convolution_inline() {
        // Test inline convolution node
        let xml = r#"
        <convolution values="DA" neighborhood="Moore">
            <rule in="A" out="D" sum="5..8" values="D"/>
            <rule in="D" out="A" sum="6..8" values="A"/>
        </convolution>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(
            result.is_ok(),
            "Failed to load inline convolution: {:?}",
            result
        );
    }

    #[test]
    fn test_load_convolution_von_neumann() {
        // Test convolution with VonNeumann kernel
        let xml = r#"
        <convolution values="DA" neighborhood="VonNeumann" periodic="True">
            <rule in="A" out="D" sum="3..4" values="A"/>
        </convolution>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(
            result.is_ok(),
            "Failed to load convolution with VonNeumann: {:?}",
            result
        );
    }

    #[test]
    fn test_load_convolution_simple_rule() {
        // Test convolution with simple rule (no sum constraint)
        let xml = r#"
        <convolution values="DA" neighborhood="Moore" steps="10">
            <rule in="A" out="D"/>
        </convolution>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(
            result.is_ok(),
            "Failed to load convolution with simple rule: {:?}",
            result
        );
    }

    #[test]
    fn test_load_convolution_missing_neighborhood() {
        // Test that missing neighborhood gives error
        let xml = r#"
        <convolution values="DA">
            <rule in="A" out="D"/>
        </convolution>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(result.is_err(), "Should fail without neighborhood");
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("neighborhood"),
            "Error should mention neighborhood: {}",
            err
        );
    }

    #[test]
    fn test_load_convolution_invalid_kernel() {
        // Test that invalid kernel name gives error
        let xml = r#"
        <convolution values="DA" neighborhood="InvalidKernel">
            <rule in="A" out="D"/>
        </convolution>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(result.is_err(), "Should fail with invalid kernel");
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("unknown"),
            "Error should mention unknown kernel: {}",
            err
        );
    }

    // ConvChain Tests

    #[test]
    fn test_load_convchain_chain_maze_xml() {
        // ChainMaze.xml uses convchain for texture synthesis
        let path = models_path().join("ChainMaze.xml");
        let result = load_model(&path);
        assert!(result.is_ok(), "Failed to load ChainMaze.xml: {:?}", result);

        let model = result.unwrap();
        assert_eq!(model.grid.c, 3, "Should have 3 values (B, D, A)");
        assert!(model.grid.values.contains_key(&'B'));
        assert!(model.grid.values.contains_key(&'D'));
        assert!(model.grid.values.contains_key(&'A'));
    }

    #[test]
    fn test_load_convchain_inline() {
        // Test inline convchain node
        use super::load_model_str_with_resources;

        let resources_path = models_path().parent().unwrap().join("resources");

        let xml = r#"
        <convchain values="BDA" sample="Maze" on="B" black="D" white="A" n="2" steps="5"/>
        "#;
        let result = load_model_str_with_resources(xml, 16, 16, 1, resources_path);
        assert!(
            result.is_ok(),
            "Failed to load inline convchain: {:?}",
            result
        );
    }

    #[test]
    fn test_load_convchain_in_sequence() {
        // Test convchain inside sequence (like ChainMaze.xml)
        use super::load_model_str_with_resources;

        let resources_path = models_path().parent().unwrap().join("resources");

        let xml = r#"
        <sequence values="BDA">
            <convchain sample="Maze" on="B" black="D" white="A" n="2" steps="10"/>
            <all in="AD/DA" out="AD/AA"/>
        </sequence>
        "#;
        let result = load_model_str_with_resources(xml, 16, 16, 1, resources_path);
        assert!(
            result.is_ok(),
            "Failed to load convchain in sequence: {:?}",
            result
        );
    }

    #[test]
    fn test_load_convchain_missing_sample() {
        // Test that missing sample gives error
        let xml = r#"
        <convchain values="BDA" on="B" black="D" white="A" n="2"/>
        "#;
        let result = load_model_str(xml, 10, 10, 1);
        assert!(result.is_err(), "Should fail without sample");
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("sample"),
            "Error should mention sample: {}",
            err
        );
    }

    #[test]
    fn test_load_convchain_missing_on() {
        // Test that missing on attribute gives error
        use super::load_model_str_with_resources;

        let resources_path = models_path().parent().unwrap().join("resources");

        let xml = r#"
        <convchain values="BDA" sample="Maze" black="D" white="A" n="2"/>
        "#;
        let result = load_model_str_with_resources(xml, 10, 10, 1, resources_path);
        assert!(result.is_err(), "Should fail without on attribute");
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("on"),
            "Error should mention on: {}",
            err
        );
    }

    #[test]
    fn test_load_convchain_3d_not_supported() {
        // Test that 3D convchain gives error (not yet supported)
        use super::load_model_str_with_resources;

        let resources_path = models_path().parent().unwrap().join("resources");

        let xml = r#"
        <convchain values="BDA" sample="Maze" on="B" black="D" white="A" n="2"/>
        "#;
        let result = load_model_str_with_resources(xml, 10, 10, 2, resources_path);
        assert!(result.is_err(), "3D convchain should not be supported");
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("2d"),
            "Error should mention 2d restriction: {}",
            err
        );
    }
}
