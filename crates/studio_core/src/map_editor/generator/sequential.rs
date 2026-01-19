//! Sequential generator that runs child generators one after another.

use super::traits::{Generator, GeneratorContext, GeneratorStructure};
use super::StepInfo;
use std::collections::HashMap;

/// Runs child generators sequentially, one at a time.
///
/// Moves to the next child when the current one completes.
pub struct SequentialGenerator {
    /// Child generators with their names.
    children: Vec<(String, Box<dyn Generator>)>,
    /// Index of the currently active child.
    current_index: usize,
    /// Scene tree path.
    path: String,
    /// Whether generation is complete.
    done: bool,
    /// Last step info from active child.
    last_step_info: Option<StepInfo>,
}

impl SequentialGenerator {
    /// Create a new sequential generator with the given children.
    pub fn new(children: Vec<(String, Box<dyn Generator>)>) -> Self {
        let mut gen = Self {
            children,
            current_index: 0,
            path: "root".to_string(),
            done: false,
            last_step_info: None,
        };
        // Set child paths
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

impl Generator for SequentialGenerator {
    fn type_name(&self) -> &str {
        "Sequential"
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
        self.current_index = 0;
        self.done = false;
        self.last_step_info = None;

        // Initialize all children
        for (_, child) in &mut self.children {
            child.init(ctx);
        }
    }

    fn step(&mut self, ctx: &mut GeneratorContext) -> bool {
        self.last_step_info = None;

        // Check if we've completed all children
        if self.current_index >= self.children.len() {
            self.done = true;
            return true;
        }

        // Step the current child
        let (_, child) = &mut self.children[self.current_index];
        let child_done = child.step(ctx);

        // Capture child's step info
        if let Some(info) = child.last_step_info() {
            self.last_step_info = Some(info.clone());
        }

        // If current child is done, move to next
        if child_done {
            self.current_index += 1;

            // Check if all done
            if self.current_index >= self.children.len() {
                self.done = true;
                return true;
            }
        }

        false
    }

    fn reset(&mut self, seed: u64) {
        self.current_index = 0;
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
    use super::*;

    // Simple test generator that completes after N steps
    struct CountingGenerator {
        path: String,
        max_steps: usize,
        current_step: usize,
        done: bool,
        last_info: Option<StepInfo>,
    }

    impl CountingGenerator {
        fn new(max_steps: usize) -> Self {
            Self {
                path: "root".to_string(),
                max_steps,
                current_step: 0,
                done: false,
                last_info: None,
            }
        }
    }

    impl Generator for CountingGenerator {
        fn type_name(&self) -> &str {
            "Counting"
        }

        fn path(&self) -> &str {
            &self.path
        }

        fn structure(&self) -> GeneratorStructure {
            GeneratorStructure::leaf(self.type_name(), &self.path)
        }

        fn init(&mut self, _ctx: &mut GeneratorContext) {
            self.current_step = 0;
            self.done = false;
        }

        fn step(&mut self, ctx: &mut GeneratorContext) -> bool {
            if self.current_step < self.max_steps {
                let x = self.current_step % ctx.width;
                let y = self.current_step / ctx.width;
                ctx.set(x, y, 1);

                self.last_info = Some(StepInfo::with_path(
                    &self.path,
                    self.current_step,
                    x,
                    y,
                    1,
                    false,
                ));

                self.current_step += 1;
            }

            self.done = self.current_step >= self.max_steps;
            self.done
        }

        fn reset(&mut self, _seed: u64) {
            self.current_step = 0;
            self.done = false;
            self.last_info = None;
        }

        fn last_step_info(&self) -> Option<&StepInfo> {
            self.last_info.as_ref()
        }

        fn is_done(&self) -> bool {
            self.done
        }

        fn set_path(&mut self, path: String) {
            self.path = path;
        }
    }

    #[test]
    fn test_sequential_structure() {
        let seq = SequentialGenerator::new(vec![
            ("step_1".to_string(), Box::new(CountingGenerator::new(5))),
            ("step_2".to_string(), Box::new(CountingGenerator::new(3))),
        ]);

        let structure = seq.structure();
        assert_eq!(structure.type_name, "Sequential");
        assert_eq!(structure.path, "root");
        assert_eq!(structure.children.len(), 2);
        assert!(structure.children.contains_key("step_1"));
        assert!(structure.children.contains_key("step_2"));
    }

    #[test]
    fn test_sequential_child_paths() {
        let seq = SequentialGenerator::new(vec![
            ("step_1".to_string(), Box::new(CountingGenerator::new(5))),
            ("step_2".to_string(), Box::new(CountingGenerator::new(3))),
        ]);

        let structure = seq.structure();
        assert_eq!(structure.children["step_1"].path, "root.step_1");
        assert_eq!(structure.children["step_2"].path, "root.step_2");
    }

    #[test]
    fn test_sequential_execution() {
        let mut seq = SequentialGenerator::new(vec![
            ("step_1".to_string(), Box::new(CountingGenerator::new(2))),
            ("step_2".to_string(), Box::new(CountingGenerator::new(2))),
        ]);

        let mut ctx = GeneratorContext::new(4, 4, vec![1], 42);
        seq.init(&mut ctx);

        // First child: 2 steps
        assert!(!seq.step(&mut ctx)); // step 1
        assert!(!seq.step(&mut ctx)); // step 2 - child 1 done, move to child 2

        // Second child: 2 steps
        assert!(!seq.step(&mut ctx)); // step 1
        assert!(seq.step(&mut ctx)); // step 2 - child 2 done, all done

        assert!(seq.is_done());
    }

    #[test]
    fn test_sequential_step_info_passthrough() {
        let mut seq = SequentialGenerator::new(vec![(
            "step_1".to_string(),
            Box::new(CountingGenerator::new(2)),
        )]);

        let mut ctx = GeneratorContext::new(4, 4, vec![1], 42);
        seq.init(&mut ctx);
        seq.step(&mut ctx);

        let info = seq.last_step_info().expect("should have step info");
        assert_eq!(info.path, "root.step_1");
    }
}
