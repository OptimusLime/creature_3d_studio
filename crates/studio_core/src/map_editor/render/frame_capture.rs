//! Frame capture system for video export.
//!
//! Provides `FrameCapture` for recording generation progress as a sequence of frames
//! that can be exported to PNG sequence or video file.
//!
//! # Example
//!
//! ```ignore
//! let mut capture = FrameCapture::new(30);  // 30 fps
//! capture.start();
//!
//! // During generation loop:
//! capture.capture_frame(&manager.render_composite(&ctx));
//!
//! // Export
//! capture.export_pngs("/tmp/frames")?;
//! capture.export_video("/tmp/generation.mp4", "libx264")?;
//! ```

use super::pixel_buffer::PixelBuffer;
use bevy::prelude::Resource;
use std::io;
use std::path::Path;
use std::process::Command;

/// Frame capture state for video export.
#[derive(Resource)]
pub struct FrameCapture {
    /// Captured frames.
    frames: Vec<PixelBuffer>,
    /// Target frame rate for video export.
    frame_rate: u32,
    /// Whether recording is currently active.
    recording: bool,
    /// Maximum frames to store (0 = unlimited).
    max_frames: usize,
}

impl Default for FrameCapture {
    fn default() -> Self {
        Self::new(30)
    }
}

impl FrameCapture {
    /// Create a new frame capture with the given frame rate.
    pub fn new(frame_rate: u32) -> Self {
        Self {
            frames: Vec::new(),
            frame_rate,
            recording: false,
            max_frames: 0,
        }
    }

    /// Create with a maximum frame limit to prevent memory exhaustion.
    pub fn with_max_frames(frame_rate: u32, max_frames: usize) -> Self {
        Self {
            frames: Vec::new(),
            frame_rate,
            recording: false,
            max_frames,
        }
    }

    /// Start recording.
    pub fn start(&mut self) {
        self.recording = true;
    }

    /// Stop recording.
    pub fn stop(&mut self) {
        self.recording = false;
    }

    /// Check if recording is active.
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Clear all captured frames.
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    /// Get the number of captured frames.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Get the target frame rate.
    pub fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    /// Set the target frame rate.
    pub fn set_frame_rate(&mut self, fps: u32) {
        self.frame_rate = fps;
    }

    /// Capture a frame if recording is active.
    ///
    /// Returns true if the frame was captured, false if not recording or at limit.
    pub fn capture_frame(&mut self, pixels: &PixelBuffer) -> bool {
        if !self.recording {
            return false;
        }

        if self.max_frames > 0 && self.frames.len() >= self.max_frames {
            return false;
        }

        self.frames.push(pixels.clone());
        true
    }

    /// Force capture a frame even if not in recording mode.
    pub fn force_capture(&mut self, pixels: &PixelBuffer) -> bool {
        if self.max_frames > 0 && self.frames.len() >= self.max_frames {
            return false;
        }

        self.frames.push(pixels.clone());
        true
    }

    /// Export all frames as PNG sequence to a directory.
    ///
    /// Files are named `frame_00001.png`, `frame_00002.png`, etc.
    pub fn export_pngs(&self, dir: &Path) -> io::Result<usize> {
        // Create directory if it doesn't exist
        std::fs::create_dir_all(dir)?;

        for (i, frame) in self.frames.iter().enumerate() {
            let path = dir.join(format!("frame_{:05}.png", i + 1));
            let png_bytes = encode_png(frame)?;
            std::fs::write(&path, png_bytes)?;
        }

        Ok(self.frames.len())
    }

    /// Export frames to a video file using ffmpeg.
    ///
    /// Requires ffmpeg to be installed and in PATH.
    /// Common codecs: "libx264" (H.264), "libx265" (H.265), "libvpx-vp9" (VP9)
    pub fn export_video(&self, output_path: &Path, codec: &str) -> io::Result<()> {
        if self.frames.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "No frames to export",
            ));
        }

        // Create temp directory for frames
        let temp_dir = std::env::temp_dir().join(format!("frame_capture_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir)?;

        // Export frames to temp directory
        self.export_pngs(&temp_dir)?;

        // Build ffmpeg command
        let input_pattern = temp_dir.join("frame_%05d.png");
        let status = Command::new("ffmpeg")
            .args([
                "-y", // Overwrite output
                "-framerate",
                &self.frame_rate.to_string(),
                "-i",
                input_pattern.to_str().unwrap(),
                "-c:v",
                codec,
                "-pix_fmt",
                "yuv420p", // Widely compatible pixel format
                "-crf",
                "23", // Quality (lower = better, 18-28 typical)
                output_path.to_str().unwrap(),
            ])
            .status();

        // Clean up temp directory
        let _ = std::fs::remove_dir_all(&temp_dir);

        match status {
            Ok(exit_status) => {
                if exit_status.success() {
                    Ok(())
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("ffmpeg exited with status: {}", exit_status),
                    ))
                }
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "ffmpeg not found. Please install ffmpeg and ensure it's in PATH.",
                    ))
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Get a reference to all captured frames.
    pub fn frames(&self) -> &[PixelBuffer] {
        &self.frames
    }

    /// Get estimated memory usage in bytes.
    pub fn memory_usage(&self) -> usize {
        self.frames.iter().map(|f| f.data.len()).sum()
    }
}

/// Encode a pixel buffer to PNG bytes.
fn encode_png(pixels: &PixelBuffer) -> io::Result<Vec<u8>> {
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;

    let mut png_bytes = Vec::new();
    let encoder = PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(
            pixels.as_bytes(),
            pixels.width as u32,
            pixels.height as u32,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    Ok(png_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_capture_basic() {
        let mut capture = FrameCapture::new(30);
        assert!(!capture.is_recording());
        assert_eq!(capture.frame_count(), 0);

        capture.start();
        assert!(capture.is_recording());

        let frame = PixelBuffer::with_color(4, 4, [255, 0, 0, 255]);
        assert!(capture.capture_frame(&frame));
        assert_eq!(capture.frame_count(), 1);

        capture.stop();
        assert!(!capture.is_recording());
        assert!(!capture.capture_frame(&frame)); // Should not capture when stopped
        assert_eq!(capture.frame_count(), 1);
    }

    #[test]
    fn test_max_frames() {
        let mut capture = FrameCapture::with_max_frames(30, 3);
        capture.start();

        let frame = PixelBuffer::new(4, 4);
        assert!(capture.capture_frame(&frame));
        assert!(capture.capture_frame(&frame));
        assert!(capture.capture_frame(&frame));
        assert!(!capture.capture_frame(&frame)); // Should fail, at limit

        assert_eq!(capture.frame_count(), 3);
    }

    #[test]
    fn test_force_capture() {
        let mut capture = FrameCapture::new(30);
        // Not recording
        assert!(!capture.is_recording());

        let frame = PixelBuffer::new(4, 4);
        assert!(capture.force_capture(&frame)); // Should work even when not recording
        assert_eq!(capture.frame_count(), 1);
    }

    #[test]
    fn test_memory_usage() {
        let mut capture = FrameCapture::new(30);
        capture.start();

        let frame = PixelBuffer::new(10, 10); // 10*10*4 = 400 bytes per frame
        capture.capture_frame(&frame);
        capture.capture_frame(&frame);

        assert_eq!(capture.memory_usage(), 800);
    }

    #[test]
    fn test_clear() {
        let mut capture = FrameCapture::new(30);
        capture.start();

        let frame = PixelBuffer::new(4, 4);
        capture.capture_frame(&frame);
        capture.capture_frame(&frame);

        assert_eq!(capture.frame_count(), 2);
        capture.clear();
        assert_eq!(capture.frame_count(), 0);
    }
}
