//! Generator system with step events for visualization.
//!
//! Provides the `Generator` trait for composable generators and `StepInfo`
//! events that describe each generation step.
//!
//! # Architecture
//!
//! - `Generator` trait: Core interface for all generators
//! - `GeneratorStructure`: Serializable tree structure for MCP introspection
//! - `StepInfo`: Data emitted after each generator step
//! - `GeneratorListener`: Trait for systems that observe generation
//!
//! # Built-in Generators
//!
//! - `SequentialGenerator`: Runs children one after another
//! - `ParallelGenerator`: Runs all children each step
//! - `ScatterGenerator`: Randomly places material on target cells
//! - `FillGenerator`: Fills cells matching a condition
//! - `MjGenerator`: Wraps Markov Jr. model with rich step info
//!
//! # Example
//!
//! ```ignore
//! use crate::map_editor::generator::{Generator, SequentialGenerator, FillGenerator};
//!
//! let gen = SequentialGenerator::new(vec![
//!     ("fill".to_string(), Box::new(FillGenerator::new(1, FillCondition::All))),
//!     ("scatter".to_string(), Box::new(ScatterGenerator::new(2, 1, 0.1))),
//! ]);
//!
//! // Get structure for MCP
//! let structure = gen.structure();
//! ```

pub mod fill;
pub mod markov;
pub mod parallel;
pub mod scatter;
pub mod sequential;
pub mod traits;

pub use fill::{FillCondition, FillGenerator};
pub use markov::MjGenerator;
pub use parallel::ParallelGenerator;
pub use scatter::ScatterGenerator;
pub use sequential::SequentialGenerator;
pub use traits::{Generator, GeneratorContext, GeneratorStructure};

use bevy::prelude::*;
use std::collections::HashMap;

/// Resource holding the active generator.
///
/// This is a non-send resource because `Generator` implementations may not be
/// thread-safe (e.g., `MjGenerator` contains `dyn Node` which isn't Send).
///
/// The active generator is set via MCP `set_generator` or on startup.
pub struct ActiveGenerator {
    /// The current generator, if any.
    generator: Option<Box<dyn Generator + 'static>>,
}

impl Default for ActiveGenerator {
    fn default() -> Self {
        Self { generator: None }
    }
}

impl ActiveGenerator {
    /// Create with no generator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with an initial generator.
    pub fn with_generator(generator: Box<dyn Generator + 'static>) -> Self {
        Self {
            generator: Some(generator),
        }
    }

    /// Set the active generator.
    pub fn set(&mut self, generator: Box<dyn Generator + 'static>) {
        self.generator = Some(generator);
    }

    /// Clear the active generator.
    pub fn clear(&mut self) {
        self.generator = None;
    }

    /// Get a reference to the generator.
    pub fn get(&self) -> Option<&(dyn Generator + 'static)> {
        self.generator.as_deref()
    }

    /// Get a mutable reference to the generator.
    pub fn get_mut(&mut self) -> Option<&mut (dyn Generator + 'static)> {
        self.generator.as_deref_mut()
    }

    /// Check if there's an active generator.
    pub fn is_some(&self) -> bool {
        self.generator.is_some()
    }

    /// Get the structure of the active generator.
    pub fn structure(&self) -> Option<GeneratorStructure> {
        self.generator.as_ref().map(|g| g.structure())
    }
}

/// Information about a single generator step.
///
/// Emitted after each cell is filled by the generator.
#[derive(Debug, Clone, Default)]
pub struct StepInfo {
    /// Scene tree path of the generator that emitted this step (e.g., "root.step_1").
    pub path: String,
    /// The step number (0-indexed).
    pub step_number: usize,
    /// X coordinate of the cell that was filled.
    pub x: usize,
    /// Y coordinate of the cell that was filled.
    pub y: usize,
    /// Material ID that was placed.
    pub material_id: u32,
    /// Whether generation is now complete.
    pub completed: bool,
    /// Name of the rule that was applied (for Markov generators).
    pub rule_name: Option<String>,
    /// Number of cells affected by this step (for batch operations).
    pub affected_cells: Option<usize>,
}

impl StepInfo {
    /// Create a new step info with default path "root".
    pub fn new(step_number: usize, x: usize, y: usize, material_id: u32, completed: bool) -> Self {
        Self {
            path: "root".to_string(),
            step_number,
            x,
            y,
            material_id,
            completed,
            rule_name: None,
            affected_cells: None,
        }
    }

    /// Create step info with a specific path.
    pub fn with_path(
        path: impl Into<String>,
        step_number: usize,
        x: usize,
        y: usize,
        material_id: u32,
        completed: bool,
    ) -> Self {
        Self {
            path: path.into(),
            step_number,
            x,
            y,
            material_id,
            completed,
            rule_name: None,
            affected_cells: None,
        }
    }

    /// Create step info with extended Markov information.
    pub fn with_markov_info(
        path: impl Into<String>,
        step_number: usize,
        x: usize,
        y: usize,
        material_id: u32,
        completed: bool,
        rule_name: Option<String>,
        affected_cells: Option<usize>,
    ) -> Self {
        Self {
            path: path.into(),
            step_number,
            x,
            y,
            material_id,
            completed,
            rule_name,
            affected_cells,
        }
    }
}

/// Trait for systems that observe generator steps.
///
/// Implement this trait to receive notifications after each generator step.
/// This enables visualization, logging, analytics, and debugging tools.
pub trait GeneratorListener: Send + Sync {
    /// Called after each generator step.
    fn on_step(&mut self, info: &StepInfo);

    /// Called when generation is reset (e.g., hot reload or palette change).
    fn on_reset(&mut self) {
        // Default: do nothing
    }
}

/// Resource that holds the current step info for the latest step.
///
/// This is updated by the generator system and can be read by other systems
/// to observe generation progress without implementing GeneratorListener.
#[derive(Resource, Default)]
pub struct CurrentStepInfo {
    /// The most recent step info, or None if no steps have occurred.
    pub info: Option<StepInfo>,
}

impl CurrentStepInfo {
    /// Update with new step info.
    pub fn update(&mut self, info: StepInfo) {
        self.info = Some(info);
    }

    /// Clear the step info (on reset).
    pub fn clear(&mut self) {
        self.info = None;
    }
}

/// Registry of step info keyed by scene tree path.
///
/// Enables visualizers to access step info from specific nodes in a composed
/// generator tree. For example, a visualizer might only care about step info
/// from "root.markov" and ignore "root.scatter".
///
/// # Example
///
/// ```ignore
/// // Get step info for the markov node
/// if let Some(info) = registry.get("root.step_1") {
///     println!("Markov at step {}", info.step_number);
/// }
///
/// // Get all step info under "root"
/// for (path, info) in registry.get_subtree("root") {
///     println!("{}: step {}", path, info.step_number);
/// }
/// ```
#[derive(Resource, Default)]
pub struct StepInfoRegistry {
    /// Map of path -> most recent StepInfo for that node.
    steps: HashMap<String, StepInfo>,
}

impl StepInfoRegistry {
    /// Emit step info for a path.
    pub fn emit(&mut self, path: impl Into<String>, info: StepInfo) {
        self.steps.insert(path.into(), info);
    }

    /// Get step info for a specific path.
    pub fn get(&self, path: &str) -> Option<&StepInfo> {
        self.steps.get(path)
    }

    /// Get all step info for paths that start with the given prefix.
    pub fn get_subtree(&self, prefix: &str) -> Vec<(&str, &StepInfo)> {
        self.steps
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }

    /// Get all step info as a map (for serialization).
    pub fn all(&self) -> &HashMap<String, StepInfo> {
        &self.steps
    }

    /// Clear all step info (on reset).
    pub fn clear(&mut self) {
        self.steps.clear();
    }

    /// Check if a path exists.
    pub fn contains(&self, path: &str) -> bool {
        self.steps.contains_key(path)
    }

    /// Get the number of tracked paths.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

/// Resource that holds registered generator listeners.
///
/// The generator calls all listeners when it emits step events.
/// Listeners can be added/removed dynamically.
#[derive(Resource, Default)]
pub struct GeneratorListeners {
    listeners: Vec<Box<dyn GeneratorListener>>,
}

impl GeneratorListeners {
    /// Add a listener.
    pub fn add(&mut self, listener: Box<dyn GeneratorListener>) {
        self.listeners.push(listener);
    }

    /// Notify all listeners of a step.
    pub fn notify_step(&mut self, info: &StepInfo) {
        for listener in &mut self.listeners {
            listener.on_step(info);
        }
    }

    /// Notify all listeners of a reset.
    pub fn notify_reset(&mut self) {
        for listener in &mut self.listeners {
            listener.on_reset();
        }
    }

    /// Get the number of registered listeners.
    pub fn len(&self) -> usize {
        self.listeners.len()
    }

    /// Check if there are no listeners.
    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_info_creation() {
        let info = StepInfo::new(10, 5, 3, 42, false);
        assert_eq!(info.path, "root");
        assert_eq!(info.step_number, 10);
        assert_eq!(info.x, 5);
        assert_eq!(info.y, 3);
        assert_eq!(info.material_id, 42);
        assert!(!info.completed);
    }

    #[test]
    fn test_step_info_with_path() {
        let info = StepInfo::with_path("root.markov", 5, 10, 20, 3, false);
        assert_eq!(info.path, "root.markov");
        assert_eq!(info.step_number, 5);
    }

    #[test]
    fn test_current_step_info() {
        let mut current = CurrentStepInfo::default();
        assert!(current.info.is_none());

        current.update(StepInfo::new(0, 0, 0, 1, false));
        assert!(current.info.is_some());
        assert_eq!(current.info.as_ref().unwrap().step_number, 0);

        current.clear();
        assert!(current.info.is_none());
    }

    #[test]
    fn test_step_info_registry() {
        let mut registry = StepInfoRegistry::default();
        assert!(registry.is_empty());

        // Emit step info for different paths
        registry.emit("root", StepInfo::new(0, 0, 0, 1, false));
        registry.emit(
            "root.step_1",
            StepInfo::with_path("root.step_1", 5, 1, 1, 2, false),
        );
        registry.emit(
            "root.step_2",
            StepInfo::with_path("root.step_2", 0, 0, 0, 0, false),
        );

        assert_eq!(registry.len(), 3);
        assert!(registry.contains("root"));
        assert!(registry.contains("root.step_1"));

        // Get specific path
        let info = registry.get("root.step_1").unwrap();
        assert_eq!(info.step_number, 5);

        // Get subtree
        let subtree = registry.get_subtree("root.step");
        assert_eq!(subtree.len(), 2);

        // Clear
        registry.clear();
        assert!(registry.is_empty());
    }

    struct TestListener {
        steps: Vec<StepInfo>,
    }

    impl GeneratorListener for TestListener {
        fn on_step(&mut self, info: &StepInfo) {
            self.steps.push(info.clone());
        }

        fn on_reset(&mut self) {
            self.steps.clear();
        }
    }

    #[test]
    fn test_generator_listener() {
        let mut listener = TestListener { steps: vec![] };

        listener.on_step(&StepInfo::new(0, 0, 0, 1, false));
        listener.on_step(&StepInfo::new(1, 1, 0, 2, false));

        assert_eq!(listener.steps.len(), 2);

        listener.on_reset();
        assert!(listener.steps.is_empty());
    }
}
