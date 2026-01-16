//! Integration tests for simulation recording and video export.
//!
//! These tests load actual MarkovJunior XML models and record their execution.

#[cfg(test)]
mod integration_tests {
    use crate::markov_junior::recording::*;
    use crate::markov_junior::{load_model_str, Interpreter, LoadedModel};
    use std::path::PathBuf;

    fn output_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("screenshots/recordings")
    }

    #[allow(dead_code)]
    fn mj_models_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MarkovJunior/models")
    }

    /// Create an interpreter from a loaded model, handling origin flag.
    fn create_interpreter(loaded: LoadedModel) -> Interpreter {
        let mut interp = if loaded.origin {
            Interpreter::with_origin(loaded.root, loaded.grid)
        } else {
            Interpreter::new(loaded.root, loaded.grid)
        };
        // Always enable animated mode for frame-by-frame recording
        interp.set_animated(true);
        interp
    }

    /// Test: Load and run BasicDungeonGrowth.xml, record to video.
    #[test]
    fn test_xml_dungeon_growth_to_video() {
        let out_dir = output_dir();
        std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

        // Load the BasicDungeonGrowth model
        let xml = r#"
            <sequence values="BRACDG" origin="True">
              <union symbol="?" values="BR"/>
              <one in="**?**/*BBB*/*BBB?/*BBB*/**R**" out="AARAA/ADDDA/ADDDR/ADDDA/AACAA"/>
              <one in="ACA/BBB" out="ARA/BBB"/>
              <all>
                <rule in="C" out="D"/>
                <rule in="R" out="D"/>
              </all>
              <all in="BD" out="*A"/>
              <all in="DDD/ADA/DDD" out="***/D*D/***"/>
              <all in="DDD/DAD/DDD" out="***/*D*/***"/>
            </sequence>
        "#;

        let loaded = load_model_str(xml, 64, 64, 1).expect("Failed to load model");
        let mut interp = create_interpreter(loaded);

        // Record simulation
        let mut recorder = SimulationRecorder::new(interp.grid());

        interp.reset(42);
        recorder.record_frame(interp.grid()); // Initial state

        let max_steps = 5000;
        let mut steps = 0;
        while interp.step() && steps < max_steps {
            recorder.record_frame(interp.grid());
            steps += 1;
        }

        println!(
            "BasicDungeonGrowth: {} steps recorded",
            recorder.frame_count()
        );

        // Save archive
        let archive = recorder.into_archive();
        let archive_path = out_dir.join("dungeon_growth.mjsim");
        archive.save(&archive_path).expect("Failed to save archive");
        println!("Saved: {}", archive_path.display());

        // Export to MP4
        // Palette: B=0, R=1, A=2, C=3, D=4, G=5
        // B=background, R=room seed, A=room floor, C=corridor marker, D=door, G=unused
        let colors = vec![
            [30, 30, 40, 255],    // B - background/wall (dark)
            [255, 80, 80, 255],   // R - room seed (red)
            [200, 180, 150, 255], // A - room floor (tan)
            [100, 200, 100, 255], // C - corridor (green)
            [150, 100, 50, 255],  // D - door (brown)
            [80, 80, 100, 255],   // G - unused (gray)
        ];
        let exporter = VideoExporter::new(archive, colors, 512);

        let video_path = out_dir.join("dungeon_growth.mp4");
        match exporter.export_mp4(&video_path, 15.0, 30) {
            Ok(()) => println!("Exported: {}", video_path.display()),
            Err(VideoError::FfmpegNotFound) => {
                println!("Skipping MP4 export (ffmpeg not installed)");
            }
            Err(e) => panic!("Video export failed: {}", e),
        }
    }

    /// Test: Load and run Cave.xml, record to video.
    #[test]
    fn test_xml_cave_to_video() {
        let out_dir = output_dir();
        std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

        // Cave model - cellular automata style
        let xml = r#"
            <sequence values="DA">
              <prl in="***/*D*/***" out="***/*A*/***"/>
              <prl in="A" out="D" p="0.435" steps="1"/>
              <convolution neighborhood="Moore">
                <rule in="A" out="D" sum="5..8" values="D"/>
                <rule in="D" out="A" sum="6..8" values="A"/>
              </convolution>
              <all in="AD/DA" out="AA/DA"/>
            </sequence>
        "#;

        let loaded = load_model_str(xml, 64, 64, 1).expect("Failed to load model");
        let mut interp = create_interpreter(loaded);

        // Record simulation
        let mut recorder = SimulationRecorder::new(interp.grid());

        interp.reset(12345);
        recorder.record_frame(interp.grid());

        let max_steps = 1000;
        let mut steps = 0;
        while interp.step() && steps < max_steps {
            recorder.record_frame(interp.grid());
            steps += 1;
        }

        println!("Cave: {} steps recorded", recorder.frame_count());

        // Save archive
        let archive = recorder.into_archive();
        let archive_path = out_dir.join("cave.mjsim");
        archive.save(&archive_path).expect("Failed to save archive");
        println!("Saved: {}", archive_path.display());

        // Export to MP4
        // Custom colors: D=dark (cave wall), A=light (air/passage)
        let colors = vec![
            [60, 50, 40, 255],    // D - dark brown cave wall
            [200, 180, 150, 255], // A - light tan passage
        ];
        let exporter = VideoExporter::new(archive, colors, 512);

        let video_path = out_dir.join("cave.mp4");
        match exporter.export_mp4(&video_path, 8.0, 30) {
            Ok(()) => println!("Exported: {}", video_path.display()),
            Err(VideoError::FfmpegNotFound) => {
                println!("Skipping MP4 export (ffmpeg not installed)");
            }
            Err(e) => panic!("Video export failed: {}", e),
        }
    }

    /// Test: Simple flood fill model via XML.
    #[test]
    fn test_xml_flood_fill_to_video() {
        let out_dir = output_dir();
        std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

        // Simple flood fill from center
        let xml = r#"
            <one values="BW" origin="True" in="BW" out="WW"/>
        "#;

        let loaded = load_model_str(xml, 48, 48, 1).expect("Failed to load model");
        let mut interp = create_interpreter(loaded);

        // Record simulation
        let mut recorder = SimulationRecorder::new(interp.grid());

        interp.reset(999);
        recorder.record_frame(interp.grid());

        let max_steps = 3000;
        let mut steps = 0;
        while interp.step() && steps < max_steps {
            recorder.record_frame(interp.grid());
            steps += 1;
        }

        println!("FloodFill: {} steps recorded", recorder.frame_count());

        // Save archive
        let archive = recorder.into_archive();
        let archive_path = out_dir.join("flood_fill.mjsim");
        archive.save(&archive_path).expect("Failed to save archive");
        println!("Saved: {}", archive_path.display());

        // Export to MP4
        let colors = vec![
            [20, 20, 30, 255],    // B - dark background
            [240, 240, 230, 255], // W - white fill
        ];
        let exporter = VideoExporter::new(archive, colors, 512);

        let video_path = out_dir.join("flood_fill.mp4");
        match exporter.export_mp4(&video_path, 6.0, 30) {
            Ok(()) => println!("Exported: {}", video_path.display()),
            Err(VideoError::FfmpegNotFound) => {
                println!("Skipping MP4 export (ffmpeg not installed)");
            }
            Err(e) => panic!("Video export failed: {}", e),
        }
    }

    /// Test: Maze generation via XML - using reference MazeBacktracker model.
    #[test]
    fn test_xml_maze_to_video() {
        let out_dir = output_dir();
        std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

        // Reference MazeBacktracker from MarkovJunior
        // values="BRGW" means B=0, R=1, G=2, W=3
        // origin=True sets center to R (value 1)
        // R carves through B, leaving G trail, then G->W cleanup
        let xml = r#"
            <markov values="BRGW" origin="True">
              <one in="RBB" out="GGR"/>
              <one in="RGG" out="WWR"/>
            </markov>
        "#;

        let loaded = load_model_str(xml, 31, 31, 1).expect("Failed to load model");
        let mut interp = create_interpreter(loaded);

        // Record simulation
        let mut recorder = SimulationRecorder::new(interp.grid());

        interp.reset(42424);
        recorder.record_frame(interp.grid());

        let max_steps = 10000;
        let mut steps = 0;
        while interp.step() && steps < max_steps {
            recorder.record_frame(interp.grid());
            steps += 1;
        }

        println!("Maze: {} steps recorded", recorder.frame_count());

        // Save archive
        let archive = recorder.into_archive();
        let archive_path = out_dir.join("maze.mjsim");
        archive.save(&archive_path).expect("Failed to save archive");
        println!("Saved: {}", archive_path.display());

        // Export to MP4
        // B=black wall, R=red head, G=green trail, W=white passage
        let colors = vec![
            [20, 20, 30, 255],    // B - wall (dark)
            [255, 60, 60, 255],   // R - current head (red)
            [60, 180, 60, 255],   // G - trail being carved (green)
            [240, 240, 230, 255], // W - finished passage (white)
        ];
        let exporter = VideoExporter::new(archive, colors, 512);

        let video_path = out_dir.join("maze.mp4");
        match exporter.export_mp4(&video_path, 15.0, 30) {
            Ok(()) => println!("Exported: {}", video_path.display()),
            Err(VideoError::FfmpegNotFound) => {
                println!("Skipping MP4 export (ffmpeg not installed)");
            }
            Err(e) => panic!("Video export failed: {}", e),
        }
    }

    /// Test that archives round-trip correctly.
    #[test]
    fn test_archive_roundtrip() {
        use crate::markov_junior::MjGrid;

        // Create a simple grid
        let mut grid = MjGrid::with_values(10, 10, 1, "BW");
        grid.set(5, 5, 0, 1);

        let mut recorder = SimulationRecorder::new(&grid);
        recorder.record_frame(&grid);

        grid.set(4, 5, 0, 1);
        grid.set(6, 5, 0, 1);
        recorder.record_frame(&grid);

        let archive = recorder.into_archive();

        // Write to buffer
        let mut buffer = Vec::new();
        archive.write_to(&mut buffer).unwrap();

        // Read back
        let loaded = SimulationArchive::read_from(&mut std::io::Cursor::new(buffer)).unwrap();

        assert_eq!(loaded.grid_type, archive.grid_type);
        assert_eq!(loaded.palette, archive.palette);
        assert_eq!(loaded.frame_count(), archive.frame_count());

        // Check frame content
        assert_eq!(loaded.frame(0), archive.frame(0));
        assert_eq!(loaded.frame(1), archive.frame(1));
    }
}
