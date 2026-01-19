//! Scatter generator that randomly places a material on target cells.

use super::traits::{Generator, GeneratorContext, GeneratorStructure};
use super::StepInfo;

/// Randomly scatters a material on cells matching a target material.
pub struct ScatterGenerator {
    /// Material ID to place.
    material: u32,
    /// Target material ID (only scatter on cells with this material).
    target: u32,
    /// Probability of placing material on each matching cell (0.0 - 1.0).
    density: f64,
    /// Scene tree path.
    path: String,
    /// Current position (x, y) during generation.
    x: usize,
    y: usize,
    /// Whether generation is complete.
    done: bool,
    /// Random state (simple LCG).
    rng_state: u64,
    /// Seed for reset.
    seed: u64,
    /// Last step info.
    last_step_info: Option<StepInfo>,
    /// Step counter.
    step_count: usize,
}

impl ScatterGenerator {
    /// Create a new scatter generator.
    pub fn new(material: u32, target: u32, density: f64) -> Self {
        Self {
            material,
            target,
            density: density.clamp(0.0, 1.0),
            path: "root".to_string(),
            x: 0,
            y: 0,
            done: false,
            rng_state: 0,
            seed: 0,
            last_step_info: None,
            step_count: 0,
        }
    }

    /// Simple LCG random number generator.
    fn random(&mut self) -> f64 {
        // LCG parameters from Numerical Recipes
        self.rng_state = self
            .rng_state
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        (self.rng_state as f64) / (u64::MAX as f64)
    }
}

impl Generator for ScatterGenerator {
    fn type_name(&self) -> &str {
        "Scatter"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn structure(&self) -> GeneratorStructure {
        GeneratorStructure::leaf(self.type_name(), &self.path).with_config(serde_json::json!({
            "material": self.material,
            "target": self.target,
            "density": self.density,
        }))
    }

    fn init(&mut self, _ctx: &mut GeneratorContext) {
        self.x = 0;
        self.y = 0;
        self.done = false;
        self.rng_state = self.seed;
        self.last_step_info = None;
        self.step_count = 0;
    }

    fn step(&mut self, ctx: &mut GeneratorContext) -> bool {
        self.last_step_info = None;

        // Scan through grid looking for target material
        while self.y < ctx.height {
            while self.x < ctx.width {
                let current = ctx.get(self.x, self.y);

                if current == self.target {
                    // Roll for scatter
                    if self.random() < self.density {
                        ctx.set(self.x, self.y, self.material);

                        self.last_step_info = Some(StepInfo::with_path(
                            &self.path,
                            self.step_count,
                            self.x,
                            self.y,
                            self.material,
                            false,
                        ));
                        self.step_count += 1;
                    }
                }

                self.x += 1;

                // Return after processing each cell to allow step-by-step visualization
                return false;
            }

            self.x = 0;
            self.y += 1;
        }

        self.done = true;
        true
    }

    fn reset(&mut self, seed: u64) {
        self.seed = seed;
        self.x = 0;
        self.y = 0;
        self.done = false;
        self.rng_state = seed;
        self.last_step_info = None;
        self.step_count = 0;
    }

    fn last_step_info(&self) -> Option<&StepInfo> {
        self.last_step_info.as_ref()
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn set_path(&mut self, path: String) {
        self.path = path;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scatter_structure() {
        let gen = ScatterGenerator::new(3, 1, 0.1);
        let structure = gen.structure();

        assert_eq!(structure.type_name, "Scatter");
        assert!(structure.config.is_some());

        let config = structure.config.unwrap();
        assert_eq!(config["material"], 3);
        assert_eq!(config["target"], 1);
        assert_eq!(config["density"], 0.1);
    }

    #[test]
    fn test_scatter_execution() {
        let mut gen = ScatterGenerator::new(2, 1, 1.0); // 100% density
        let mut ctx = GeneratorContext::new(4, 4, vec![1, 2], 42);

        // Fill with target material
        for y in 0..4 {
            for x in 0..4 {
                ctx.set(x, y, 1);
            }
        }

        gen.reset(12345);
        gen.init(&mut ctx);

        // Run to completion
        while !gen.step(&mut ctx) {}

        // All cells should now be material 2 (100% density)
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(ctx.get(x, y), 2);
            }
        }
    }

    #[test]
    fn test_scatter_respects_target() {
        let mut gen = ScatterGenerator::new(3, 1, 1.0);
        let mut ctx = GeneratorContext::new(4, 4, vec![1, 2, 3], 42);

        // Only some cells have target material
        ctx.set(0, 0, 1);
        ctx.set(1, 1, 2); // Not target
        ctx.set(2, 2, 1);

        gen.reset(12345);
        gen.init(&mut ctx);

        while !gen.step(&mut ctx) {}

        assert_eq!(ctx.get(0, 0), 3); // Scattered
        assert_eq!(ctx.get(1, 1), 2); // Unchanged (not target)
        assert_eq!(ctx.get(2, 2), 3); // Scattered
    }

    #[test]
    fn test_scatter_emits_step_info() {
        let mut gen = ScatterGenerator::new(2, 1, 1.0);
        let mut ctx = GeneratorContext::new(2, 2, vec![1, 2], 42);

        ctx.set(0, 0, 1);
        gen.reset(12345);
        gen.init(&mut ctx);
        gen.step(&mut ctx);

        let info = gen.last_step_info().expect("should have step info");
        assert_eq!(info.material_id, 2);
        assert_eq!(info.x, 0);
        assert_eq!(info.y, 0);
    }
}
