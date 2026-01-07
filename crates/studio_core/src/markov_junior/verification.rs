//! Verification module for cell-by-cell comparison with C# MarkovJunior.
//!
//! This module provides tools for running MarkovJunior models with the exact
//! same RNG as C# (using DotNetRandom) and outputting grid state in JSON format
//! for comparison.
//!
//! # Usage
//!
//! ```ignore
//! use studio_core::markov_junior::verification::{capture_model_state, ModelState};
//!
//! let state = capture_model_state("Basic", 42, 50000)?;
//! let json = serde_json::to_string_pretty(&state)?;
//! std::fs::write("verification/rust/Basic_seed42.json", json)?;
//! ```

use super::loader::{load_model_str_with_resources, LoadError, LoadedModel};
use super::rng::DotNetRandom;
use super::Interpreter;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Captured model state for verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelState {
    /// Model name
    pub model: String,
    /// Random seed used
    pub seed: i32,
    /// Grid dimensions [MX, MY, MZ]
    pub dimensions: [usize; 3],
    /// Character legend (index -> character)
    pub characters: Vec<String>,
    /// Grid state as flat array of byte values
    pub state: Vec<u8>,
}

/// Error type for verification operations.
#[derive(Debug)]
pub enum VerificationError {
    /// Model loading failed
    LoadError(LoadError),
    /// Model file not found
    ModelNotFound(String),
    /// IO error
    IoError(std::io::Error),
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationError::LoadError(e) => write!(f, "load error: {}", e),
            VerificationError::ModelNotFound(name) => write!(f, "model not found: {}", name),
            VerificationError::IoError(e) => write!(f, "io error: {}", e),
        }
    }
}

impl std::error::Error for VerificationError {}

impl From<LoadError> for VerificationError {
    fn from(e: LoadError) -> Self {
        VerificationError::LoadError(e)
    }
}

impl From<std::io::Error> for VerificationError {
    fn from(e: std::io::Error) -> Self {
        VerificationError::IoError(e)
    }
}

/// Find the MarkovJunior directory.
fn find_mj_dir() -> Option<PathBuf> {
    let candidates = ["MarkovJunior", "../MarkovJunior", "../../MarkovJunior"];

    for candidate in &candidates {
        let path = Path::new(candidate);
        if path.exists() && path.join("models.xml").exists() {
            return Some(path.to_path_buf());
        }
    }

    None
}

/// Model configuration from models.xml
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub name: String,
    pub mx: usize,
    pub my: usize,
    pub mz: usize,
    pub steps: usize,
}

/// Parse models.xml to get model configuration (dimensions, steps).
pub fn parse_models_xml(model_name: &str) -> Result<ModelConfig, VerificationError> {
    let mj_dir = find_mj_dir().ok_or_else(|| {
        VerificationError::ModelNotFound("MarkovJunior directory not found".to_string())
    })?;

    let models_xml_path = mj_dir.join("models.xml");
    let content = std::fs::read_to_string(&models_xml_path)?;

    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"model" {
                    let mut name = None;
                    let mut size = None;
                    let mut length = None;
                    let mut width = None;
                    let mut height = None;
                    let mut d = 2usize;
                    let mut steps = 50000usize;

                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");

                        match key {
                            "name" => name = Some(val.to_string()),
                            "size" => size = val.parse().ok(),
                            "length" => length = val.parse().ok(),
                            "width" => width = val.parse().ok(),
                            "height" => height = val.parse().ok(),
                            "d" => d = val.parse().unwrap_or(2),
                            "steps" => {
                                // steps="-1" means unlimited, use 0 which our run loop treats as unlimited
                                let parsed: i64 = val.parse().unwrap_or(50000);
                                steps = if parsed < 0 { 0 } else { parsed as usize };
                            }
                            _ => {}
                        }
                    }

                    if name.as_deref() == Some(model_name) {
                        let linear_size = size.unwrap_or(16);
                        let mx = length.unwrap_or(linear_size);
                        let my = width.unwrap_or(linear_size);
                        let mz = height.unwrap_or(if d == 2 { 1 } else { linear_size });

                        return Ok(ModelConfig {
                            name: model_name.to_string(),
                            mx,
                            my,
                            mz,
                            steps,
                        });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(VerificationError::LoadError(LoadError::XmlError(format!(
                    "{}",
                    e
                ))))
            }
            _ => {}
        }
    }

    Err(VerificationError::ModelNotFound(format!(
        "model '{}' not found in models.xml",
        model_name
    )))
}

/// Load a model by name with dimensions from models.xml.
pub fn load_model_by_name(
    model_name: &str,
) -> Result<(LoadedModel, ModelConfig), VerificationError> {
    let mj_dir = find_mj_dir().ok_or_else(|| {
        VerificationError::ModelNotFound("MarkovJunior directory not found".to_string())
    })?;

    let config = parse_models_xml(model_name)?;

    let model_path = mj_dir.join("models").join(format!("{}.xml", model_name));
    let content = std::fs::read_to_string(&model_path)?;

    let resources_path = mj_dir.join("resources");

    let loaded =
        load_model_str_with_resources(&content, config.mx, config.my, config.mz, resources_path)?;

    Ok((loaded, config))
}

/// Capture the final grid state of a model run using DotNetRandom.
///
/// This runs the model with the exact same RNG as C# MarkovJunior,
/// allowing for cell-by-cell comparison of outputs.
///
/// # Arguments
/// * `model_name` - Name of the model (e.g., "Basic", "River")
/// * `seed` - Random seed (as i32 to match C# System.Random)
/// * `max_steps` - Maximum steps to run (0 = use model's default from models.xml)
///
/// # Returns
/// `ModelState` containing the final grid state.
pub fn capture_model_state(
    model_name: &str,
    seed: i32,
    max_steps: usize,
) -> Result<ModelState, VerificationError> {
    let (loaded, config) = load_model_by_name(model_name)?;

    let LoadedModel { root, grid, origin } = loaded;

    // Create interpreter
    let mut interpreter = if origin {
        Interpreter::with_origin(root, grid)
    } else {
        Interpreter::new(root, grid)
    };

    // Reset with DotNetRandom for C# compatibility
    let rng = Box::new(DotNetRandom::from_seed(seed));
    interpreter.reset_with_rng(rng);

    // Run to completion (use model's steps from models.xml if max_steps is 0)
    // A limit of 0 means unlimited
    let mut steps = 0;
    let limit = if max_steps == 0 {
        config.steps
    } else {
        max_steps
    };
    while interpreter.is_running() && (limit == 0 || steps < limit) {
        interpreter.step();
        steps += 1;
    }

    // Capture final state
    let grid = interpreter.grid();
    Ok(ModelState {
        model: model_name.to_string(),
        seed,
        dimensions: [grid.mx, grid.my, grid.mz],
        characters: grid.characters.iter().map(|c| c.to_string()).collect(),
        state: grid.state.clone(),
    })
}

/// Compare two model states and return differences.
///
/// Returns a list of (index, x, y, z, expected, actual) for each differing cell.
pub fn compare_states(expected: &ModelState, actual: &ModelState) -> ComparisonResult {
    let mut result = ComparisonResult {
        model: expected.model.clone(),
        seed: expected.seed,
        dimensions_match: expected.dimensions == actual.dimensions,
        total_cells: expected.state.len(),
        matching_cells: 0,
        differences: Vec::new(),
    };

    if !result.dimensions_match {
        return result;
    }

    let [mx, my, _mz] = expected.dimensions;

    for (i, (&e, &a)) in expected.state.iter().zip(actual.state.iter()).enumerate() {
        if e == a {
            result.matching_cells += 1;
        } else {
            let x = i % mx;
            let y = (i / mx) % my;
            let z = i / (mx * my);
            result.differences.push(CellDiff {
                index: i,
                x,
                y,
                z,
                expected: e,
                actual: a,
            });
        }
    }

    result
}

/// Result of comparing two model states.
#[derive(Debug, Clone, Serialize)]
pub struct ComparisonResult {
    pub model: String,
    pub seed: i32,
    pub dimensions_match: bool,
    pub total_cells: usize,
    pub matching_cells: usize,
    pub differences: Vec<CellDiff>,
}

impl ComparisonResult {
    /// Get accuracy as a percentage.
    pub fn accuracy(&self) -> f64 {
        if self.total_cells == 0 {
            return 100.0;
        }
        100.0 * self.matching_cells as f64 / self.total_cells as f64
    }

    /// Check if results match 100%.
    pub fn is_perfect(&self) -> bool {
        self.dimensions_match && self.differences.is_empty()
    }
}

/// A single cell difference.
#[derive(Debug, Clone, Serialize)]
pub struct CellDiff {
    pub index: usize,
    pub x: usize,
    pub y: usize,
    pub z: usize,
    pub expected: u8,
    pub actual: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_multiple_seeds() {
        // Verify Basic model matches 100% for multiple seeds
        for seed in [42, 123, 999] {
            let result = capture_model_state("Basic", seed, 0).unwrap();

            let json = serde_json::to_string_pretty(&result).unwrap();
            let _ = std::fs::create_dir_all("../../verification/rust");
            let _ = std::fs::write(
                format!("../../verification/rust/Basic_seed{}.json", seed),
                &json,
            );

            println!(
                "Basic seed {}: {} cells, first 10: {:?}",
                seed,
                result.state.len(),
                &result.state[..10]
            );
        }
    }

    #[test]
    fn test_river_bisect() {
        // Systematically test River components to find where divergence starts
        let _ = std::fs::create_dir_all("../../verification/rust");

        for i in 1..=10 {
            let model_name = format!("RiverTest{}", i);
            match capture_model_state(&model_name, 42, 0) {
                Ok(result) => {
                    let json = serde_json::to_string_pretty(&result).unwrap();
                    let _ = std::fs::write(
                        format!("../../verification/rust/{}_seed42.json", model_name),
                        &json,
                    );
                    println!("{}: {} cells", model_name, result.state.len());
                }
                Err(e) => {
                    println!("{}: ERROR - {}", model_name, e);
                }
            }
        }
    }

    #[test]
    fn test_flowers_bisect() {
        // Test Flowers model components
        let _ = std::fs::create_dir_all("../../verification/rust");

        for i in 1..=6 {
            let model_name = format!("FlowersTest{}", i);
            match capture_model_state(&model_name, 42, 0) {
                Ok(result) => {
                    let json = serde_json::to_string_pretty(&result).unwrap();
                    let _ = std::fs::write(
                        format!("../../verification/rust/{}_seed42.json", model_name),
                        &json,
                    );
                    println!("{}: {} cells", model_name, result.state.len());
                }
                Err(e) => {
                    println!("{}: ERROR - {}", model_name, e);
                }
            }
        }
    }

    #[test]
    fn test_capture_river_model() {
        // River model - more complex 2D model
        let result = capture_model_state("River", 42, 0);

        match result {
            Ok(state) => {
                println!("Model: {}", state.model);
                println!("Seed: {}", state.seed);
                println!(
                    "Dimensions: {}x{}x{}",
                    state.dimensions[0], state.dimensions[1], state.dimensions[2]
                );
                println!("Characters: {:?}", state.characters);
                println!("State length: {}", state.state.len());

                // Save to file for comparison
                let json = serde_json::to_string_pretty(&state).unwrap();
                let _ = std::fs::create_dir_all("../../verification/rust");
                let _ = std::fs::write("../../verification/rust/River_seed42.json", &json);

                // Print first few values
                println!("First 20 state values: {:?}", &state.state[..20]);
            }
            Err(e) => {
                println!("Error loading model: {}", e);
            }
        }
    }

    #[test]
    fn test_capture_flowers_model() {
        // Flowers - simple 3D growth model
        let result = capture_model_state("Flowers", 42, 0);

        match result {
            Ok(state) => {
                println!("Model: {}", state.model);
                println!("Seed: {}", state.seed);
                println!(
                    "Dimensions: {}x{}x{}",
                    state.dimensions[0], state.dimensions[1], state.dimensions[2]
                );
                println!("Characters: {:?}", state.characters);
                println!("State length: {}", state.state.len());

                // Save to file for comparison
                let json = serde_json::to_string_pretty(&state).unwrap();
                let _ = std::fs::create_dir_all("../../verification/rust");
                let _ = std::fs::write("../../verification/rust/Flowers_seed42.json", &json);
            }
            Err(e) => {
                println!("Error loading model: {}", e);
            }
        }
    }

    #[test]
    fn test_capture_growth_model() {
        // Growth - 3D model
        let result = capture_model_state("Growth", 42, 0);

        match result {
            Ok(state) => {
                println!("Model: {}", state.model);
                println!("Seed: {}", state.seed);
                println!(
                    "Dimensions: {}x{}x{}",
                    state.dimensions[0], state.dimensions[1], state.dimensions[2]
                );
                println!("Characters: {:?}", state.characters);
                println!("State length: {}", state.state.len());

                // Save to file for comparison
                let json = serde_json::to_string_pretty(&state).unwrap();
                let _ = std::fs::create_dir_all("../../verification/rust");
                let _ = std::fs::write("../../verification/rust/Growth_seed42.json", &json);
            }
            Err(e) => {
                println!("Error loading model: {}", e);
            }
        }
    }

    #[test]
    fn test_compare_states_identical() {
        let state = ModelState {
            model: "Test".to_string(),
            seed: 42,
            dimensions: [3, 3, 1],
            characters: vec!["B".to_string(), "W".to_string()],
            state: vec![0, 1, 0, 1, 0, 1, 0, 1, 0],
        };

        let result = compare_states(&state, &state);
        assert!(result.is_perfect());
        assert_eq!(result.accuracy(), 100.0);
    }

    #[test]
    fn test_compare_states_different() {
        let expected = ModelState {
            model: "Test".to_string(),
            seed: 42,
            dimensions: [3, 3, 1],
            characters: vec!["B".to_string(), "W".to_string()],
            state: vec![0, 1, 0, 1, 0, 1, 0, 1, 0],
        };

        let actual = ModelState {
            model: "Test".to_string(),
            seed: 42,
            dimensions: [3, 3, 1],
            characters: vec!["B".to_string(), "W".to_string()],
            state: vec![0, 1, 1, 1, 0, 1, 0, 0, 0], // 2 differences at positions 2, 7
        };

        let result = compare_states(&expected, &actual);
        assert!(!result.is_perfect());
        assert_eq!(result.differences.len(), 2);
        assert_eq!(result.matching_cells, 7);
        assert!((result.accuracy() - 77.78).abs() < 0.1);
    }

    /// Generate Rust verification output for a model.
    /// This test is meant to be run manually to generate outputs for comparison.
    ///
    /// Run with: cargo test -p studio_core verification::tests::generate_model_output -- --nocapture MODEL_NAME
    #[test]
    #[ignore] // Run manually
    fn generate_model_output() {
        // Get model name from env var or use default
        let model_name = std::env::var("MJ_MODEL").unwrap_or_else(|_| "Basic".to_string());
        let seed: i32 = std::env::var("MJ_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(42);

        let _ = std::fs::create_dir_all("../../verification/rust");

        match capture_model_state(&model_name, seed, 0) {
            Ok(state) => {
                let json = serde_json::to_string_pretty(&state).unwrap();
                let path = format!("../../verification/rust/{}_seed{}.json", model_name, seed);
                std::fs::write(&path, &json).expect("Failed to write output");
                println!("Generated: {}", path);
                println!("  Dimensions: {:?}", state.dimensions);
                println!("  Cells: {}", state.state.len());
            }
            Err(e) => {
                println!("ERROR: {}: {}", model_name, e);
            }
        }
    }

    /// Batch generate outputs for all models in a list.
    /// Models are passed via MJ_MODELS env var (comma-separated).
    #[test]
    #[ignore]
    fn batch_generate_outputs() {
        let models_str = std::env::var("MJ_MODELS")
            .unwrap_or_else(|_| "Basic,River,Growth,Flowers,MazeGrowth,Maze".to_string());
        let seed: i32 = std::env::var("MJ_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(42);

        let _ = std::fs::create_dir_all("../../verification/rust");

        let mut success = 0;
        let mut failed = 0;

        for model_name in models_str.split(',') {
            let model_name = model_name.trim();
            if model_name.is_empty() {
                continue;
            }

            match capture_model_state(model_name, seed, 0) {
                Ok(state) => {
                    let json = serde_json::to_string_pretty(&state).unwrap();
                    let path = format!("../../verification/rust/{}_seed{}.json", model_name, seed);
                    if let Err(e) = std::fs::write(&path, &json) {
                        println!("WRITE ERROR: {}: {}", model_name, e);
                        failed += 1;
                    } else {
                        println!("OK: {} ({} cells)", model_name, state.state.len());
                        success += 1;
                    }
                }
                Err(e) => {
                    println!("ERROR: {}: {}", model_name, e);
                    failed += 1;
                }
            }
        }

        println!("\nGenerated: {}, Failed: {}", success, failed);
    }

    /// Test to see how many steps Circuit takes without a limit
    #[test]
    #[ignore]
    fn test_circuit_step_count() {
        use crate::markov_junior::loader::LoadedModel;
        use crate::markov_junior::rng::DotNetRandom;
        use crate::markov_junior::Interpreter;

        let (loaded, config) = load_model_by_name("Circuit").unwrap();
        let LoadedModel { root, grid, origin } = loaded;

        let mut interpreter = if origin {
            Interpreter::with_origin(root, grid)
        } else {
            Interpreter::new(root, grid)
        };

        let rng = Box::new(DotNetRandom::from_seed(42));
        interpreter.reset_with_rng(rng);

        println!("Config steps: {}", config.steps);

        // Run without limit (but cap at 5000 to avoid infinite loops)
        let mut external_steps = 0;
        while interpreter.is_running() && external_steps < 5000 {
            interpreter.step();
            external_steps += 1;
        }

        println!("External steps: {}", external_steps);
        println!("Interpreter counter: {}", interpreter.counter());
        println!("Still running: {}", interpreter.is_running());
    }
}
