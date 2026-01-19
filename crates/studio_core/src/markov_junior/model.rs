//! High-level Model API for MarkovJunior.
//!
//! Provides a convenient wrapper around Interpreter for loading and running models.

use super::interpreter::Interpreter;
use super::loader::{load_model, load_model_str, LoadError, LoadedModel};
use super::MjGrid;
use std::path::Path;

/// A MarkovJunior model that can be loaded from XML and executed.
///
/// # Example
///
/// ```ignore
/// use studio_core::markov_junior::Model;
///
/// // Load from file
/// let mut model = Model::load("MarkovJunior/models/Basic.xml")?;
///
/// // Run with seed
/// model.run(42, 0);  // 0 = no step limit
///
/// // Access result
/// let grid = model.grid();
/// println!("Generated {} non-zero cells", grid.count_nonzero());
/// ```
pub struct Model {
    /// Name of the model (from filename)
    pub name: String,
    /// The interpreter running this model
    pub(crate) interpreter: Interpreter,
}

impl Model {
    /// Load a model from an XML file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, LoadError> {
        let path = path.as_ref();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let LoadedModel { root, grid, origin } = load_model(path)?;

        let interpreter = if origin {
            Interpreter::with_origin(root, grid)
        } else {
            Interpreter::new(root, grid)
        };

        Ok(Self { name, interpreter })
    }

    /// Load a model from an XML file with custom grid dimensions.
    ///
    /// This allows overriding the default 16x16x1 grid size for 3D generation.
    ///
    /// # Arguments
    /// * `path` - Path to the XML model file
    /// * `mx`, `my`, `mz` - Grid dimensions
    pub fn load_with_size<P: AsRef<Path>>(
        path: P,
        mx: usize,
        my: usize,
        mz: usize,
    ) -> Result<Self, LoadError> {
        let path = path.as_ref();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Read XML content
        let content = std::fs::read_to_string(path)
            .map_err(|_| LoadError::FileNotFound(path.display().to_string()))?;

        // Determine resources path from file location
        let resources_path = path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("resources"));

        // Load with custom dimensions
        let LoadedModel { root, grid, origin } = if let Some(ref res) = resources_path {
            if res.exists() {
                super::loader::load_model_str_with_resources(&content, mx, my, mz, res.clone())?
            } else {
                load_model_str(&content, mx, my, mz)?
            }
        } else {
            load_model_str(&content, mx, my, mz)?
        };

        let interpreter = if origin {
            Interpreter::with_origin(root, grid)
        } else {
            Interpreter::new(root, grid)
        };

        Ok(Self { name, interpreter })
    }

    /// Load a model from an XML string.
    ///
    /// # Arguments
    /// * `xml` - The XML content
    /// * `mx`, `my`, `mz` - Grid dimensions (use 0 for defaults)
    pub fn load_str(xml: &str, mx: usize, my: usize, mz: usize) -> Result<Self, LoadError> {
        let LoadedModel { root, grid, origin } = load_model_str(xml, mx, my, mz)?;

        let interpreter = if origin {
            Interpreter::with_origin(root, grid)
        } else {
            Interpreter::new(root, grid)
        };

        Ok(Self {
            name: "inline".to_string(),
            interpreter,
        })
    }

    /// Run the model with the given seed.
    ///
    /// # Arguments
    /// * `seed` - Random seed for deterministic generation
    /// * `max_steps` - Maximum steps (0 = no limit)
    ///
    /// Returns the number of steps executed.
    pub fn run(&mut self, seed: u64, max_steps: usize) -> usize {
        self.interpreter.run(seed, max_steps)
    }

    /// Execute a single step of the model.
    ///
    /// Returns `true` if the model made progress.
    pub fn step(&mut self) -> bool {
        self.interpreter.step()
    }

    /// Reset the model for a new run.
    pub fn reset(&mut self, seed: u64) {
        self.interpreter.reset(seed);
    }

    /// Get a reference to the current grid state.
    pub fn grid(&self) -> &MjGrid {
        self.interpreter.grid()
    }

    /// Get the current step counter.
    pub fn counter(&self) -> usize {
        self.interpreter.counter()
    }

    /// Check if the model is still running.
    pub fn is_running(&self) -> bool {
        self.interpreter.is_running()
    }

    /// Enable or disable animated mode.
    ///
    /// When animated is true, the grid state is updated incrementally during execution,
    /// allowing visualization of the generation process step-by-step. This is useful
    /// for real-time rendering of the generation process.
    ///
    /// Call this after loading the model but before calling `reset()` or `step()`.
    pub fn set_animated(&mut self, animated: bool) {
        self.interpreter.set_animated(animated);
    }

    /// Check if animated mode is enabled.
    pub fn is_animated(&self) -> bool {
        self.interpreter.is_animated()
    }

    /// Get the number of cells changed in the last step.
    ///
    /// Returns 0 if no steps have been executed or if the last step made no changes.
    pub fn last_step_change_count(&self) -> usize {
        self.interpreter.last_step_change_count()
    }

    /// Get the positions of cells changed in the last step.
    ///
    /// Returns an empty slice if no steps have been executed or if the last step
    /// made no changes.
    pub fn last_step_changes(&self) -> &[(i32, i32, i32)] {
        self.interpreter.last_step_changes()
    }
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
    fn test_basic_model_runs_correctly() {
        let path = models_path().join("Basic.xml");
        let mut model = Model::load(&path).expect("Failed to load Basic.xml");

        // Run to completion
        let steps = model.run(12345, 0);

        // Basic.xml converts all B's to W's on a 16x16 grid
        // So it should take 256 steps (one per cell)
        assert_eq!(steps, 256, "Basic.xml should complete in 256 steps");

        // All cells should be W (value 1)
        assert!(
            model.grid().state.iter().all(|&v| v == 1),
            "All cells should be W after Basic.xml completes"
        );
    }

    #[test]
    fn test_growth_model_runs_correctly() {
        let path = models_path().join("Growth.xml");
        let mut model = Model::load(&path).expect("Failed to load Growth.xml");

        // Run to completion
        let steps = model.run(42, 0);

        // Growth.xml starts from center and grows outward
        // Should complete when no more WB patterns exist
        assert!(steps > 0, "Growth.xml should take at least one step");

        // Should have many W cells (grown from origin)
        let w_count = model.grid().state.iter().filter(|&&v| v == 1).count();
        assert!(w_count > 100, "Growth should produce many W cells");
    }

    #[test]
    fn test_model_step_by_step() {
        let xml = r#"<one values="BW" in="B" out="W"/>"#;
        let mut model = Model::load_str(xml, 5, 5, 1).expect("Failed to load inline XML");

        model.reset(42);

        // Step through
        let mut steps = 0;
        while model.step() {
            steps += 1;
            assert!(steps <= 25, "Should not take more than 25 steps");
        }

        assert_eq!(steps, 25, "5x5 grid should take 25 steps");
        assert!(
            model.grid().state.iter().all(|&v| v == 1),
            "All cells should be W"
        );
    }

    #[test]
    fn test_model_with_max_steps() {
        let xml = r#"<one values="BW" in="B" out="W"/>"#;
        let mut model = Model::load_str(xml, 10, 10, 1).expect("Failed to load inline XML");

        // Run with limit
        let steps = model.run(42, 50);
        assert_eq!(steps, 50, "Should stop at 50 steps");

        // Should have exactly 50 W cells
        let w_count = model.grid().state.iter().filter(|&&v| v == 1).count();
        assert_eq!(w_count, 50, "Should have 50 W cells");
    }

    #[test]
    fn test_backtracker_model_runs() {
        let path = models_path().join("Backtracker.xml");
        let mut model = Model::load(&path).expect("Failed to load Backtracker.xml");

        // Run for limited steps (backtracker can take many)
        let steps = model.run(42, 1000);

        assert!(steps > 0, "Backtracker should make progress");

        // Should have some non-zero cells (maze structure)
        let nonzero = model.grid().count_nonzero();
        assert!(nonzero > 0, "Backtracker should produce non-zero cells");
    }

    #[test]
    fn test_wfc_overlap_model_runs() {
        // Test that WFC overlap model can be loaded and executed
        let path = models_path().join("WaveFlowers.xml");
        let mut model = Model::load(&path).expect("Failed to load WaveFlowers.xml");

        // WaveFlowers.xml is:
        // <sequence values="BW" symmetry="(x)">
        //   <all in="B/*/*" out="W/*/*"/>
        //   <wfc sample="Flowers" values="zYgN" n="3" periodic="True" shannon="True">
        //     <rule in="B" out="N|g"/>
        //     <rule in="W" out="z|g|Y"/>
        //   </wfc>
        // </sequence>

        // Run to completion (or max steps)
        let steps = model.run(42, 10000);

        assert!(steps > 0, "WaveFlowers should make progress");

        // Model should complete (not hit max steps for small grids)
        assert!(
            !model.is_running() || steps < 10000,
            "Model should complete or make significant progress"
        );

        // Grid should have non-zero values (the WFC fills it)
        let nonzero = model.grid().count_nonzero();
        assert!(nonzero > 0, "WFC should produce non-zero cells");
    }
}
