//! Generator trait and related types for the composable generator system.
//!
//! All generators implement the `Generator` trait, which provides:
//! - Structure introspection via `structure()`
//! - Step-by-step generation via `init()`, `step()`, `reset()`
//! - Step info emission via `last_step_info()`
//!
//! # Example
//!
//! ```ignore
//! use crate::map_editor::generator::{Generator, GeneratorContext, GeneratorStructure};
//!
//! struct MyGenerator {
//!     path: String,
//!     step_count: usize,
//! }
//!
//! impl Generator for MyGenerator {
//!     fn type_name(&self) -> &str { "MyGenerator" }
//!     
//!     fn structure(&self) -> GeneratorStructure {
//!         GeneratorStructure::leaf(self.type_name(), &self.path)
//!     }
//!     
//!     fn step(&mut self, ctx: &mut GeneratorContext) -> bool {
//!         // Generate one cell...
//!         self.step_count += 1;
//!         self.step_count >= ctx.width * ctx.height
//!     }
//! }
//! ```

use super::StepInfo;
use serde::Serialize;
use std::collections::HashMap;

/// Context passed to generators during init/step/reset.
///
/// Provides access to the voxel buffer and palette.
pub struct GeneratorContext {
    /// Width of the buffer.
    pub width: usize,
    /// Height of the buffer.
    pub height: usize,
    /// Flat buffer data (row-major). Generators write directly to this.
    pub data: Vec<u32>,
    /// Active material palette (material IDs available for use).
    pub palette: Vec<u32>,
    /// Random seed for deterministic generation.
    pub seed: u64,
}

impl GeneratorContext {
    /// Create a new context with the given dimensions.
    pub fn new(width: usize, height: usize, palette: Vec<u32>, seed: u64) -> Self {
        Self {
            width,
            height,
            data: vec![0; width * height],
            palette,
            seed,
        }
    }

    /// Set a voxel at (x, y) to the given material ID.
    pub fn set(&mut self, x: usize, y: usize, material_id: u32) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = material_id;
        }
    }

    /// Get the material ID at (x, y).
    pub fn get(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.data[y * self.width + x]
        } else {
            0
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.data.fill(0);
    }
}

/// Recursive structure of a generator tree.
///
/// Serializable for MCP endpoint responses.
#[derive(Clone, Debug, Serialize)]
pub struct GeneratorStructure {
    /// Type name (e.g., "Sequential", "MjModel", "Scatter").
    #[serde(rename = "type")]
    pub type_name: String,

    /// Scene tree path (e.g., "root", "root.step_1").
    pub path: String,

    /// For MjModel: the XML model name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,

    /// Child generators (name -> structure).
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub children: HashMap<String, GeneratorStructure>,

    /// Generator-specific configuration (serialized as JSON).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,

    /// For MjModel: the internal Markov Jr. node structure (M10.5).
    /// Contains the tree of Markov/Sequence/One/All/etc nodes and their rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mj_structure: Option<crate::markov_junior::MjNodeStructure>,
}

impl GeneratorStructure {
    /// Create a leaf node (no children).
    pub fn leaf(type_name: &str, path: &str) -> Self {
        Self {
            type_name: type_name.to_string(),
            path: path.to_string(),
            model_name: None,
            children: HashMap::new(),
            config: None,
            mj_structure: None,
        }
    }

    /// Create a leaf node with model name (for MjModel).
    pub fn mj_model(path: &str, model_name: &str) -> Self {
        Self {
            type_name: "MjModel".to_string(),
            path: path.to_string(),
            model_name: Some(model_name.to_string()),
            children: HashMap::new(),
            config: None,
            mj_structure: None,
        }
    }

    /// Create a MjModel node with internal structure (M10.5).
    pub fn mj_model_with_structure(
        path: &str,
        model_name: &str,
        mj_structure: crate::markov_junior::MjNodeStructure,
    ) -> Self {
        Self {
            type_name: "MjModel".to_string(),
            path: path.to_string(),
            model_name: Some(model_name.to_string()),
            children: HashMap::new(),
            config: None,
            mj_structure: Some(mj_structure),
        }
    }

    /// Create a node with children (for Sequential, Parallel).
    pub fn with_children(
        type_name: &str,
        path: &str,
        children: HashMap<String, GeneratorStructure>,
    ) -> Self {
        Self {
            type_name: type_name.to_string(),
            path: path.to_string(),
            model_name: None,
            children,
            config: None,
            mj_structure: None,
        }
    }

    /// Add configuration data.
    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = Some(config);
        self
    }

    /// Add Markov Jr. internal structure (M10.5).
    pub fn with_mj_structure(
        mut self,
        mj_structure: crate::markov_junior::MjNodeStructure,
    ) -> Self {
        self.mj_structure = Some(mj_structure);
        self
    }
}

/// Core trait for all generators.
///
/// Generators produce voxel data step-by-step and can be composed into trees.
///
/// Note: Generators are not required to be Send + Sync because they run on the
/// main Bevy thread. If cross-thread use is needed, wrap in Arc<Mutex>.
pub trait Generator {
    /// Returns the generator's type name (e.g., "Sequential", "MjModel", "Scatter").
    fn type_name(&self) -> &str;

    /// Returns the scene tree path of this generator.
    fn path(&self) -> &str;

    /// Returns the recursive structure of this generator and its children.
    fn structure(&self) -> GeneratorStructure;

    /// Initialize the generator with the given context.
    ///
    /// Called once before generation starts.
    fn init(&mut self, ctx: &mut GeneratorContext);

    /// Execute one step of generation.
    ///
    /// Returns `true` if generation is complete, `false` if more steps remain.
    fn step(&mut self, ctx: &mut GeneratorContext) -> bool;

    /// Reset the generator to its initial state.
    ///
    /// Called when restarting generation (e.g., on hot reload).
    fn reset(&mut self, seed: u64);

    /// Get the step info emitted during the last call to `step()`.
    ///
    /// Returns `None` if no step info was emitted.
    fn last_step_info(&self) -> Option<&StepInfo>;

    /// Check if the generator has completed.
    fn is_done(&self) -> bool;

    /// Set the scene tree path (called by parent when adding as child).
    fn set_path(&mut self, path: String);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generator_context() {
        let mut ctx = GeneratorContext::new(4, 4, vec![1, 2, 3], 42);
        assert_eq!(ctx.width, 4);
        assert_eq!(ctx.height, 4);
        assert_eq!(ctx.get(0, 0), 0);

        ctx.set(1, 2, 5);
        assert_eq!(ctx.get(1, 2), 5);

        ctx.clear();
        assert_eq!(ctx.get(1, 2), 0);
    }

    #[test]
    fn test_generator_structure_leaf() {
        let s = GeneratorStructure::leaf("Scatter", "root.step_1");
        assert_eq!(s.type_name, "Scatter");
        assert_eq!(s.path, "root.step_1");
        assert!(s.children.is_empty());
        assert!(s.model_name.is_none());
    }

    #[test]
    fn test_generator_structure_mj_model() {
        let s = GeneratorStructure::mj_model("root.step_1", "MazeGrowth.xml");
        assert_eq!(s.type_name, "MjModel");
        assert_eq!(s.model_name, Some("MazeGrowth.xml".to_string()));
    }

    #[test]
    fn test_generator_structure_with_children() {
        let mut children = HashMap::new();
        children.insert(
            "step_1".to_string(),
            GeneratorStructure::leaf("Scatter", "root.step_1"),
        );
        children.insert(
            "step_2".to_string(),
            GeneratorStructure::leaf("Fill", "root.step_2"),
        );

        let s = GeneratorStructure::with_children("Sequential", "root", children);
        assert_eq!(s.type_name, "Sequential");
        assert_eq!(s.children.len(), 2);
        assert!(s.children.contains_key("step_1"));
    }

    #[test]
    fn test_generator_structure_serialization() {
        let s = GeneratorStructure::mj_model("root.markov", "BasicDungeon.xml")
            .with_config(serde_json::json!({"seed": 42}));

        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("MjModel"));
        assert!(json.contains("BasicDungeon.xml"));
        assert!(json.contains("seed"));
    }
}
