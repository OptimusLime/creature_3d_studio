//! Generator system with step events for visualization.
//!
//! Provides the `GeneratorListener` trait for observing generation progress
//! and `StepInfo` events that describe each generation step.
//!
//! # Architecture
//!
//! - `StepInfo`: Data emitted after each generator step
//! - `GeneratorListener`: Trait for systems that observe generation
//! - Any system can implement `GeneratorListener` to receive step events
//!
//! # Example
//!
//! ```ignore
//! struct MyListener;
//!
//! impl GeneratorListener for MyListener {
//!     fn on_step(&mut self, info: &StepInfo) {
//!         println!("Step {} at ({}, {})", info.step_number, info.x, info.y);
//!     }
//! }
//! ```

use bevy::prelude::*;

/// Information about a single generator step.
///
/// Emitted after each cell is filled by the generator.
#[derive(Debug, Clone, Default)]
pub struct StepInfo {
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
    /// Create a new step info.
    pub fn new(step_number: usize, x: usize, y: usize, material_id: u32, completed: bool) -> Self {
        Self {
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
        step_number: usize,
        x: usize,
        y: usize,
        material_id: u32,
        completed: bool,
        rule_name: Option<String>,
        affected_cells: Option<usize>,
    ) -> Self {
        Self {
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
        assert_eq!(info.step_number, 10);
        assert_eq!(info.x, 5);
        assert_eq!(info.y, 3);
        assert_eq!(info.material_id, 42);
        assert!(!info.completed);
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
