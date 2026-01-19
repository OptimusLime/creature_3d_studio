//! Fill generator that fills cells matching a condition.

use super::traits::{Generator, GeneratorContext, GeneratorStructure};
use super::StepInfo;

/// Condition for filling cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FillCondition {
    /// Fill all cells.
    All,
    /// Fill only empty cells (material 0).
    Empty,
    /// Fill border cells.
    Border,
    /// Fill cells with a specific material.
    Material(u32),
}

/// Fills cells that match a condition.
pub struct FillGenerator {
    /// Material ID to place.
    material: u32,
    /// Condition for which cells to fill.
    condition: FillCondition,
    /// Scene tree path.
    path: String,
    /// Current position (x, y) during generation.
    x: usize,
    y: usize,
    /// Whether generation is complete.
    done: bool,
    /// Last step info.
    last_step_info: Option<StepInfo>,
    /// Step counter.
    step_count: usize,
}

impl FillGenerator {
    /// Create a new fill generator.
    pub fn new(material: u32, condition: FillCondition) -> Self {
        Self {
            material,
            condition,
            path: "root".to_string(),
            x: 0,
            y: 0,
            done: false,
            last_step_info: None,
            step_count: 0,
        }
    }

    /// Check if a cell matches the fill condition.
    fn matches(&self, ctx: &GeneratorContext, x: usize, y: usize) -> bool {
        match self.condition {
            FillCondition::All => true,
            FillCondition::Empty => ctx.get(x, y) == 0,
            FillCondition::Border => x == 0 || y == 0 || x == ctx.width - 1 || y == ctx.height - 1,
            FillCondition::Material(m) => ctx.get(x, y) == m,
        }
    }
}

impl Generator for FillGenerator {
    fn type_name(&self) -> &str {
        "Fill"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn structure(&self) -> GeneratorStructure {
        let condition_str = match self.condition {
            FillCondition::All => "all".to_string(),
            FillCondition::Empty => "empty".to_string(),
            FillCondition::Border => "border".to_string(),
            FillCondition::Material(m) => format!("material:{}", m),
        };

        GeneratorStructure::leaf(self.type_name(), &self.path).with_config(serde_json::json!({
            "material": self.material,
            "condition": condition_str,
        }))
    }

    fn init(&mut self, _ctx: &mut GeneratorContext) {
        self.x = 0;
        self.y = 0;
        self.done = false;
        self.last_step_info = None;
        self.step_count = 0;
    }

    fn step(&mut self, ctx: &mut GeneratorContext) -> bool {
        self.last_step_info = None;

        while self.y < ctx.height {
            while self.x < ctx.width {
                if self.matches(ctx, self.x, self.y) {
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

                self.x += 1;

                // Return after each cell for step-by-step visualization
                return false;
            }

            self.x = 0;
            self.y += 1;
        }

        self.done = true;
        true
    }

    fn reset(&mut self, _seed: u64) {
        self.x = 0;
        self.y = 0;
        self.done = false;
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
    fn test_fill_all() {
        let mut gen = FillGenerator::new(1, FillCondition::All);
        let mut ctx = GeneratorContext::new(4, 4, vec![1], 42);

        gen.init(&mut ctx);
        while !gen.step(&mut ctx) {}

        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(ctx.get(x, y), 1);
            }
        }
    }

    #[test]
    fn test_fill_empty() {
        let mut gen = FillGenerator::new(2, FillCondition::Empty);
        let mut ctx = GeneratorContext::new(4, 4, vec![1, 2], 42);

        // Pre-fill some cells
        ctx.set(0, 0, 1);
        ctx.set(1, 1, 1);

        gen.init(&mut ctx);
        while !gen.step(&mut ctx) {}

        // Pre-filled cells should be unchanged
        assert_eq!(ctx.get(0, 0), 1);
        assert_eq!(ctx.get(1, 1), 1);
        // Empty cells should be filled
        assert_eq!(ctx.get(2, 2), 2);
        assert_eq!(ctx.get(3, 3), 2);
    }

    #[test]
    fn test_fill_border() {
        let mut gen = FillGenerator::new(1, FillCondition::Border);
        let mut ctx = GeneratorContext::new(4, 4, vec![1], 42);

        gen.init(&mut ctx);
        while !gen.step(&mut ctx) {}

        // Corners
        assert_eq!(ctx.get(0, 0), 1);
        assert_eq!(ctx.get(3, 0), 1);
        assert_eq!(ctx.get(0, 3), 1);
        assert_eq!(ctx.get(3, 3), 1);

        // Interior should be empty
        assert_eq!(ctx.get(1, 1), 0);
        assert_eq!(ctx.get(2, 2), 0);
    }

    #[test]
    fn test_fill_structure() {
        let gen = FillGenerator::new(5, FillCondition::Border);
        let structure = gen.structure();

        assert_eq!(structure.type_name, "Fill");
        let config = structure.config.unwrap();
        assert_eq!(config["material"], 5);
        assert_eq!(config["condition"], "border");
    }

    #[test]
    fn test_fill_emits_step_info() {
        let mut gen = FillGenerator::new(1, FillCondition::All);
        let mut ctx = GeneratorContext::new(2, 2, vec![1], 42);

        gen.init(&mut ctx);
        gen.step(&mut ctx);

        let info = gen.last_step_info().expect("should have step info");
        assert_eq!(info.material_id, 1);
        assert_eq!(info.x, 0);
        assert_eq!(info.y, 0);
    }
}
