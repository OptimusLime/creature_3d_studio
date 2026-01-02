//! Shared benchmark infrastructure for measuring performance across examples.
//!
//! Provides a simple plugin that reports FPS and frame timing statistics
//! at configurable intervals.
//!
//! ## Usage
//!
//! ```ignore
//! use studio_core::benchmark::{BenchmarkPlugin, BenchmarkConfig};
//!
//! App::new()
//!     .add_plugins(BenchmarkPlugin)
//!     .insert_resource(BenchmarkConfig {
//!         report_interval_secs: 2.0,
//!         enabled: true,
//!     })
//!     .run();
//! ```
//!
//! Output (every N seconds):
//! ```text
//! FPS: 60.0 | Frame: 16.6ms | Min: 58.2 | Max: 17.2ms
//! ```

use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};

/// Configuration for benchmark reporting.
#[derive(Resource)]
pub struct BenchmarkConfig {
    /// How often to report stats (in seconds)
    pub report_interval_secs: f32,
    /// Whether benchmarking is enabled
    pub enabled: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            report_interval_secs: 2.0,
            enabled: true,
        }
    }
}

/// Internal state for tracking benchmark statistics.
#[derive(Resource, Default)]
pub struct BenchmarkStats {
    /// Time since last report
    time_since_report: f32,
    /// Frame times collected since last report
    frame_times: Vec<f64>,
    /// FPS values collected since last report
    fps_values: Vec<f64>,
}

impl BenchmarkStats {
    /// Calculate statistics from collected samples.
    pub fn calculate(&self) -> BenchmarkResult {
        if self.frame_times.is_empty() || self.fps_values.is_empty() {
            return BenchmarkResult::default();
        }

        let avg_fps = self.fps_values.iter().sum::<f64>() / self.fps_values.len() as f64;
        let avg_frame_ms = self.frame_times.iter().sum::<f64>() / self.frame_times.len() as f64;
        
        let min_fps = self.fps_values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_frame_ms = self.frame_times.iter().cloned().fold(0.0, f64::max);

        BenchmarkResult {
            avg_fps,
            avg_frame_ms,
            min_fps,
            max_frame_ms,
            sample_count: self.frame_times.len(),
        }
    }

    /// Clear collected samples.
    pub fn clear(&mut self) {
        self.frame_times.clear();
        self.fps_values.clear();
        self.time_since_report = 0.0;
    }
}

/// Result of benchmark calculation.
#[derive(Debug, Clone, Default)]
pub struct BenchmarkResult {
    pub avg_fps: f64,
    pub avg_frame_ms: f64,
    pub min_fps: f64,
    pub max_frame_ms: f64,
    pub sample_count: usize,
}

impl std::fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FPS: {:.1} | Frame: {:.2}ms | Min: {:.1} | Max: {:.2}ms",
            self.avg_fps, self.avg_frame_ms, self.min_fps, self.max_frame_ms
        )
    }
}

/// System that collects frame timing data.
fn collect_benchmark_data(
    config: Res<BenchmarkConfig>,
    diagnostics: Res<DiagnosticsStore>,
    mut stats: ResMut<BenchmarkStats>,
) {
    if !config.enabled {
        return;
    }

    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    {
        stats.fps_values.push(fps);
    }

    if let Some(frame_time) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
    {
        stats.frame_times.push(frame_time);
    }
}

/// System that reports benchmark results at configured intervals.
fn report_benchmark(
    config: Res<BenchmarkConfig>,
    mut stats: ResMut<BenchmarkStats>,
    time: Res<Time>,
) {
    if !config.enabled {
        return;
    }

    stats.time_since_report += time.delta_secs();

    if stats.time_since_report >= config.report_interval_secs {
        let result = stats.calculate();
        if result.sample_count > 0 {
            info!("{}", result);
        }
        stats.clear();
    }
}

/// Plugin that provides benchmark reporting.
///
/// Automatically adds `FrameTimeDiagnosticsPlugin` if not already present.
pub struct BenchmarkPlugin;

impl Plugin for BenchmarkPlugin {
    fn build(&self, app: &mut App) {
        // Add frame time diagnostics if not present
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }

        app.init_resource::<BenchmarkConfig>()
            .init_resource::<BenchmarkStats>()
            .add_systems(Update, (collect_benchmark_data, report_benchmark).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_config_default() {
        let config = BenchmarkConfig::default();
        assert!((config.report_interval_secs - 2.0).abs() < 0.001);
        assert!(config.enabled);
    }

    #[test]
    fn test_benchmark_stats_empty() {
        let stats = BenchmarkStats::default();
        let result = stats.calculate();
        assert_eq!(result.sample_count, 0);
        assert_eq!(result.avg_fps, 0.0);
    }

    #[test]
    fn test_benchmark_stats_calculation() {
        let mut stats = BenchmarkStats::default();
        
        // Add some sample data
        stats.fps_values = vec![60.0, 58.0, 62.0, 59.0];
        stats.frame_times = vec![16.6, 17.2, 16.1, 16.9];

        let result = stats.calculate();
        
        assert_eq!(result.sample_count, 4);
        
        // Average FPS: (60 + 58 + 62 + 59) / 4 = 59.75
        assert!((result.avg_fps - 59.75).abs() < 0.01);
        
        // Average frame time: (16.6 + 17.2 + 16.1 + 16.9) / 4 = 16.7
        assert!((result.avg_frame_ms - 16.7).abs() < 0.01);
        
        // Min FPS: 58
        assert!((result.min_fps - 58.0).abs() < 0.01);
        
        // Max frame time: 17.2
        assert!((result.max_frame_ms - 17.2).abs() < 0.01);
    }

    #[test]
    fn test_benchmark_stats_clear() {
        let mut stats = BenchmarkStats::default();
        stats.fps_values = vec![60.0, 58.0];
        stats.frame_times = vec![16.6, 17.2];
        stats.time_since_report = 1.5;

        stats.clear();

        assert!(stats.fps_values.is_empty());
        assert!(stats.frame_times.is_empty());
        assert_eq!(stats.time_since_report, 0.0);
    }

    #[test]
    fn test_benchmark_result_display() {
        let result = BenchmarkResult {
            avg_fps: 59.75,
            avg_frame_ms: 16.7,
            min_fps: 58.0,
            max_frame_ms: 17.2,
            sample_count: 4,
        };

        let display = format!("{}", result);
        assert!(display.contains("FPS: 59.8"));
        assert!(display.contains("Frame: 16.70ms"));
        assert!(display.contains("Min: 58.0"));
        assert!(display.contains("Max: 17.20ms"));
    }
}
