//! Screenshot verification tests for the rendering pipeline.
//!
//! NOTE: Due to macOS requirements (winit must run on main thread),
//! these tests are run as binaries rather than `cargo test`.
//!
//! Run with: `cargo run --example p0_screenshot_test`
//!
//! Screenshots are saved to the `screenshots/` folder (gitignored).

use std::path::Path;

/// Test configuration
const SCREENSHOT_DIR: &str = "screenshots";

/// Ensure screenshots directory exists
pub fn ensure_screenshot_dir() {
    std::fs::create_dir_all(SCREENSHOT_DIR).expect("Failed to create screenshots directory");
}

/// Verify that a screenshot file exists
pub fn verify_screenshot_exists(path: &str) -> bool {
    Path::new(path).exists()
}

/// These tests cannot run via `cargo test` on macOS because winit requires main thread.
/// Instead, run the examples:
/// - `cargo run --example p0_screenshot_test`
/// - `cargo run --example p1_black_void_test`
#[test]
fn screenshot_tests_are_examples() {
    // This test just documents that screenshot tests are run as examples
    println!("Screenshot tests must be run as examples:");
    println!("  cargo run --example p0_screenshot_test");
    println!("  cargo run --example p1_black_void_test");
}
