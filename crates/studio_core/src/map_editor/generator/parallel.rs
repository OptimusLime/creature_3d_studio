//! Parallel generator that runs all child generators each step.

use super::traits::{Generator, GeneratorContext, GeneratorStructure};
use super::StepInfo;
use std::collections::HashMap;

/// Runs all child generators in parallel, stepping each one every frame.
///
/// Completes when all children are done.
pub struct ParallelGenerator {
    /// Child generators with their names.
    children: Vec<(String, Box<dyn Generator>)>,
    /// Scene tree path.
    path: String,
    /// Whether generation is complete.
    done: bool,
    /// Combined step info from all children this step.
    last_step_info: Option<StepInfo>,
}

impl ParallelGenerator {
    /// Create a new parallel generator with the given children.
    pub fn new(children: Vec<(String, Box<dyn Generator>)>) -> Self {
        let mut gen = Self {
            children,
            path: "root".to_string(),
            done: false,
            last_step_info: None,
        };
        gen.update_child_paths();
        gen
    }

    /// Update child paths based on our path.
    fn update_child_paths(&mut self) {
        for (name, child) in &mut self.children {
            let child_path = format!("{}.{}", self.path, name);
            child.set_path(child_path);
        }
    }
}

impl Generator for ParallelGenerator {
    fn type_name(&self) -> &str {
        "Parallel"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn structure(&self) -> GeneratorStructure {
        let mut children = HashMap::new();
        for (name, child) in &self.children {
            children.insert(name.clone(), child.structure());
        }
        GeneratorStructure::with_children(self.type_name(), &self.path, children)
    }

    fn init(&mut self, ctx: &mut GeneratorContext) {
        self.done = false;
        self.last_step_info = None;

        for (_, child) in &mut self.children {
            child.init(ctx);
        }
    }

    fn step(&mut self, ctx: &mut GeneratorContext) -> bool {
        self.last_step_info = None;
        let mut all_done = true;

        for (_, child) in &mut self.children {
            // Skip children that are already done
            if child.is_done() {
                continue;
            }

            let child_done = child.step(ctx);

            // Capture the first step info we see
            if self.last_step_info.is_none() {
                if let Some(info) = child.last_step_info() {
                    self.last_step_info = Some(info.clone());
                }
            }

            if !child_done {
                all_done = false;
            }
        }

        self.done = all_done;
        all_done
    }

    fn reset(&mut self, seed: u64) {
        self.done = false;
        self.last_step_info = None;

        for (_, child) in &mut self.children {
            child.reset(seed);
        }
    }

    fn last_step_info(&self) -> Option<&StepInfo> {
        self.last_step_info.as_ref()
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn set_path(&mut self, path: String) {
        self.path = path;
        self.update_child_paths();
    }
}

#[cfg(test)]
mod tests {
    use super::super::fill::{FillCondition, FillGenerator};
    use super::*;

    #[test]
    fn test_parallel_structure() {
        let par = ParallelGenerator::new(vec![
            (
                "branch_1".to_string(),
                Box::new(FillGenerator::new(1, FillCondition::All)),
            ),
            (
                "branch_2".to_string(),
                Box::new(FillGenerator::new(2, FillCondition::Border)),
            ),
        ]);

        let structure = par.structure();
        assert_eq!(structure.type_name, "Parallel");
        assert_eq!(structure.children.len(), 2);
        assert!(structure.children.contains_key("branch_1"));
        assert!(structure.children.contains_key("branch_2"));
    }

    #[test]
    fn test_parallel_child_paths() {
        let par = ParallelGenerator::new(vec![(
            "branch_1".to_string(),
            Box::new(FillGenerator::new(1, FillCondition::All)),
        )]);

        let structure = par.structure();
        assert_eq!(structure.children["branch_1"].path, "root.branch_1");
    }

    #[test]
    fn test_parallel_execution() {
        // Both branches run each step
        let mut par = ParallelGenerator::new(vec![
            (
                "branch_1".to_string(),
                Box::new(FillGenerator::new(1, FillCondition::All)),
            ),
            (
                "branch_2".to_string(),
                Box::new(FillGenerator::new(2, FillCondition::Border)),
            ),
        ]);

        let mut ctx = GeneratorContext::new(4, 4, vec![1, 2], 42);
        par.init(&mut ctx);

        // Run until both complete (they should complete at the same time since
        // they both iterate through the same grid)
        let mut steps = 0;
        while !par.step(&mut ctx) {
            steps += 1;
            if steps > 100 {
                panic!("Parallel generator did not complete");
            }
        }

        assert!(par.is_done());
    }
}
