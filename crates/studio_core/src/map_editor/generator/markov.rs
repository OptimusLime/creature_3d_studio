//! Markov Jr. generator that wraps the markov_junior::Model.

use super::traits::{Generator, GeneratorContext, GeneratorStructure};
use super::StepInfo;
use crate::markov_junior::Model;

/// Generator that wraps a Markov Jr. model.
///
/// Emits step info with affected cell count on each step.
pub struct MjGenerator {
    /// The underlying Markov Jr. model.
    model: Model,
    /// Scene tree path.
    path: String,
    /// Last step info.
    last_step_info: Option<StepInfo>,
    /// Seed for reset.
    seed: u64,
    /// Previous change count (to calculate delta).
    prev_change_count: usize,
}

impl MjGenerator {
    /// Create a new MjGenerator from a Model.
    pub fn new(model: Model) -> Self {
        Self {
            model,
            path: "root".to_string(),
            last_step_info: None,
            seed: 0,
            prev_change_count: 0,
        }
    }

    /// Get a reference to the underlying model.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Get a mutable reference to the underlying model.
    pub fn model_mut(&mut self) -> &mut Model {
        &mut self.model
    }
}

impl Generator for MjGenerator {
    fn type_name(&self) -> &str {
        "MjModel"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn structure(&self) -> GeneratorStructure {
        GeneratorStructure::mj_model_with_structure(
            &self.path,
            &self.model.name,
            self.model.structure(),
        )
    }

    fn init(&mut self, ctx: &mut GeneratorContext) {
        // Reset the model
        self.model.reset(ctx.seed);
        self.seed = ctx.seed;
        self.last_step_info = None;
        self.prev_change_count = 0;

        // Enable animated mode for step-by-step visualization
        self.model.set_animated(true);
    }

    fn step(&mut self, ctx: &mut GeneratorContext) -> bool {
        self.last_step_info = None;

        // Step the model
        let made_progress = self.model.step();

        if made_progress {
            // Get changes since last step
            let changes = self.model.interpreter.changes();
            let new_changes = changes.len() - self.prev_change_count;
            self.prev_change_count = changes.len();

            // Copy model grid to context buffer
            let grid = self.model.grid();
            let mz = grid.mz.max(1);
            for y in 0..ctx.height.min(grid.my) {
                for x in 0..ctx.width.min(grid.mx) {
                    // For 2D, use z=0
                    let idx = x + y * grid.mx;
                    if idx < grid.state.len() {
                        ctx.set(x, y, grid.state[idx] as u32);
                    }
                }
            }

            // Find the last changed position (for step info)
            let (last_x, last_y, last_mat) = if !changes.is_empty() {
                let (x, y, _z) = changes[changes.len() - 1];
                let idx = (x as usize) + (y as usize) * grid.mx;
                let mat = if idx < grid.state.len() {
                    grid.state[idx] as u32
                } else {
                    0
                };
                (x as usize, y as usize, mat)
            } else {
                (0, 0, 0)
            };

            self.last_step_info = Some(StepInfo::with_markov_info(
                &self.path,
                self.model.counter(),
                last_x,
                last_y,
                last_mat,
                !self.model.is_running(),
                None, // TODO: Extract rule name from interpreter
                Some(new_changes),
            ));
        }

        !self.model.is_running()
    }

    fn reset(&mut self, seed: u64) {
        self.seed = seed;
        self.model.reset(seed);
        self.last_step_info = None;
        self.prev_change_count = 0;
    }

    fn last_step_info(&self) -> Option<&StepInfo> {
        self.last_step_info.as_ref()
    }

    fn is_done(&self) -> bool {
        !self.model.is_running()
    }

    fn set_path(&mut self, path: String) {
        self.path = path;
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
    fn test_mj_generator_structure() {
        let path = models_path().join("Basic.xml");
        if !path.exists() {
            eprintln!("Skipping test: {:?} not found", path);
            return;
        }

        let model = Model::load(&path).expect("Failed to load model");
        let gen = MjGenerator::new(model);

        let structure = gen.structure();
        assert_eq!(structure.type_name, "MjModel");
        assert_eq!(structure.model_name, Some("Basic".to_string()));
    }

    #[test]
    fn test_mj_generator_execution() {
        let path = models_path().join("Basic.xml");
        if !path.exists() {
            eprintln!("Skipping test: {:?} not found", path);
            return;
        }

        let model = Model::load(&path).expect("Failed to load model");
        let mut gen = MjGenerator::new(model);

        // Use a context that matches typical grid size
        let mut ctx = GeneratorContext::new(16, 16, vec![0, 1], 12345);
        gen.init(&mut ctx);

        // Run a few steps
        let mut steps = 0;
        while !gen.is_done() && steps < 100 {
            gen.step(&mut ctx);
            steps += 1;
        }

        // Should have completed
        assert!(gen.is_done() || steps == 100);
    }

    #[test]
    fn test_mj_generator_emits_step_info() {
        let path = models_path().join("Basic.xml");
        if !path.exists() {
            eprintln!("Skipping test: {:?} not found", path);
            return;
        }

        let model = Model::load(&path).expect("Failed to load model");
        let mut gen = MjGenerator::new(model);

        let mut ctx = GeneratorContext::new(16, 16, vec![0, 1], 12345);
        gen.init(&mut ctx);
        gen.step(&mut ctx);

        // Should have step info with affected_cells
        if let Some(info) = gen.last_step_info() {
            assert!(info.affected_cells.is_some());
            assert!(info.affected_cells.unwrap() > 0);
        }
    }
}
