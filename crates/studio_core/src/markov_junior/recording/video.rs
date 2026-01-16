//! Video export for simulation archives.
//!
//! Uses ffmpeg to encode PNG frames into MP4 video.

use super::archive::SimulationArchive;
use super::grid_type::GridType;
use super::traits::default_colors_for_palette;
use image::{ImageBuffer, Rgba, RgbaImage};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Errors that can occur during video export.
#[derive(Debug)]
pub enum VideoError {
    /// I/O error.
    Io(std::io::Error),
    /// Image encoding error.
    Image(image::ImageError),
    /// ffmpeg not found or failed to start.
    FfmpegNotFound,
    /// ffmpeg returned an error.
    FfmpegError(String),
    /// Unsupported grid type for video export.
    UnsupportedGridType(GridType),
    /// No frames to export.
    NoFrames,
}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Image(e) => write!(f, "Image error: {}", e),
            Self::FfmpegNotFound => write!(f, "ffmpeg not found - please install ffmpeg"),
            Self::FfmpegError(s) => write!(f, "ffmpeg error: {}", s),
            Self::UnsupportedGridType(gt) => write!(f, "Unsupported grid type for video: {:?}", gt),
            Self::NoFrames => write!(f, "No frames to export"),
        }
    }
}

impl std::error::Error for VideoError {}

impl From<std::io::Error> for VideoError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<image::ImageError> for VideoError {
    fn from(e: image::ImageError) -> Self {
        Self::Image(e)
    }
}

/// Video exporter for simulation archives.
///
/// Renders frames from a SimulationArchive and encodes them to MP4 using ffmpeg.
pub struct VideoExporter {
    archive: SimulationArchive,
    colors: Vec<[u8; 4]>,
    background: [u8; 4],
    image_size: u32,
}

impl VideoExporter {
    /// Create a new video exporter.
    ///
    /// # Arguments
    /// * `archive` - The simulation archive to export
    /// * `colors` - Color palette for rendering (use `default_colors_for_palette` for defaults)
    /// * `image_size` - Width and height of output frames in pixels
    pub fn new(archive: SimulationArchive, colors: Vec<[u8; 4]>, image_size: u32) -> Self {
        Self {
            archive,
            colors,
            background: [50, 50, 50, 255], // Dark gray background
            image_size,
        }
    }

    /// Create a new video exporter with default colors from the archive's palette.
    pub fn with_default_colors(archive: SimulationArchive, image_size: u32) -> Self {
        let colors = default_colors_for_palette(&archive.palette);
        Self::new(archive, colors, image_size)
    }

    /// Set the background color.
    pub fn set_background(&mut self, color: [u8; 4]) {
        self.background = color;
    }

    /// Export the simulation to an MP4 video.
    ///
    /// # Arguments
    /// * `path` - Output file path (should end in .mp4)
    /// * `target_duration_secs` - Desired video duration in seconds
    /// * `fps` - Frames per second (e.g., 30)
    ///
    /// The exporter will sample frames from the archive to achieve the target duration.
    /// If the archive has fewer frames than needed, frames will be duplicated.
    /// If it has more, frames will be sampled evenly.
    pub fn export_mp4<P: AsRef<Path>>(
        &self,
        path: P,
        target_duration_secs: f32,
        fps: u32,
    ) -> Result<(), VideoError> {
        if self.archive.frame_count() == 0 {
            return Err(VideoError::NoFrames);
        }

        // Only 2D grids supported for now
        if !self.archive.grid_type.is_2d() {
            return Err(VideoError::UnsupportedGridType(self.archive.grid_type));
        }

        let target_frames = (target_duration_secs * fps as f32) as usize;
        let source_frames = self.archive.frame_count();
        let last_frame = source_frames - 1;

        // Calculate frame indices to use
        // IMPORTANT: Always include the last frame
        let mut frame_indices: Vec<usize> = if source_frames >= target_frames {
            // Sample evenly from source frames
            (0..target_frames)
                .map(|i| (i * source_frames) / target_frames)
                .collect()
        } else {
            // Duplicate frames to fill target duration
            (0..target_frames)
                .map(|i| (i * source_frames) / target_frames)
                .collect()
        };

        // Ensure the last frame is included (replace the final frame index)
        if let Some(last) = frame_indices.last_mut() {
            *last = last_frame;
        }

        // Start ffmpeg process
        let path_str = path.as_ref().to_string_lossy();
        let mut ffmpeg = Command::new("ffmpeg")
            .args([
                "-y", // Overwrite output
                "-f",
                "image2pipe", // Input format: pipe of images
                "-vcodec",
                "png", // Input codec: PNG
                "-r",
                &fps.to_string(), // Input frame rate
                "-i",
                "-", // Input from stdin
                "-c:v",
                "libx264", // Output codec: H.264
                "-pix_fmt",
                "yuv420p", // Pixel format for compatibility
                "-preset",
                "medium", // Encoding speed/quality tradeoff
                "-crf",
                "23",      // Quality (lower = better, 18-28 typical)
                &path_str, // Output file
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| VideoError::FfmpegNotFound)?;

        let stdin = ffmpeg.stdin.as_mut().ok_or(VideoError::FfmpegNotFound)?;

        // Render and pipe each frame
        for (i, &frame_idx) in frame_indices.iter().enumerate() {
            let frame_data = self.archive.frame(frame_idx).unwrap();
            let image = self.render_frame(frame_data)?;

            // Encode to PNG and write to stdin
            let mut png_data = Vec::new();
            image.write_to(
                &mut std::io::Cursor::new(&mut png_data),
                image::ImageFormat::Png,
            )?;
            stdin.write_all(&png_data)?;

            // Progress indication (every 10%)
            if i % (target_frames / 10).max(1) == 0 {
                let pct = (i * 100) / target_frames;
                eprintln!("Encoding: {}%", pct);
            }
        }

        // Close stdin and wait for ffmpeg
        drop(ffmpeg.stdin.take());
        let output = ffmpeg.wait_with_output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VideoError::FfmpegError(stderr.to_string()));
        }

        eprintln!("Encoding: 100% - Done!");
        Ok(())
    }

    /// Render a single frame from raw bytes to an image.
    fn render_frame(&self, frame_data: &[u8]) -> Result<RgbaImage, VideoError> {
        match self.archive.grid_type {
            GridType::Cartesian2D { width, height } => {
                self.render_cartesian_2d(frame_data, width, height)
            }
            GridType::Polar2D {
                r_min,
                r_depth,
                theta_divisions,
            } => self.render_polar_2d(frame_data, r_min, r_depth, theta_divisions),
            _ => Err(VideoError::UnsupportedGridType(self.archive.grid_type)),
        }
    }

    /// Render a Cartesian 2D frame.
    fn render_cartesian_2d(
        &self,
        frame_data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<RgbaImage, VideoError> {
        let img_size = self.image_size;
        let mut img: RgbaImage = ImageBuffer::from_pixel(img_size, img_size, Rgba(self.background));

        // Scale to fit image
        let scale_x = img_size as f32 / width as f32;
        let scale_y = img_size as f32 / height as f32;
        let scale = scale_x.min(scale_y) * 0.95; // 5% margin

        let offset_x = (img_size as f32 - width as f32 * scale) / 2.0;
        let offset_y = (img_size as f32 - height as f32 * scale) / 2.0;

        // Draw each cell
        for y in 0..height {
            for x in 0..width {
                let idx = (x + y * width) as usize;
                let value = frame_data[idx] as usize;

                if value >= self.colors.len() || self.colors[value][3] == 0 {
                    continue;
                }

                let color = Rgba(self.colors[value]);

                // Fill the cell rectangle
                let px_start = (offset_x + x as f32 * scale) as u32;
                let py_start = (offset_y + y as f32 * scale) as u32;
                let px_end = (offset_x + (x + 1) as f32 * scale) as u32;
                let py_end = (offset_y + (y + 1) as f32 * scale) as u32;

                for py in py_start..py_end.min(img_size) {
                    for px in px_start..px_end.min(img_size) {
                        img.put_pixel(px, py, color);
                    }
                }
            }
        }

        Ok(img)
    }

    /// Render a Polar 2D frame.
    fn render_polar_2d(
        &self,
        frame_data: &[u8],
        r_min: u32,
        r_depth: u16,
        theta_divisions: u16,
    ) -> Result<RgbaImage, VideoError> {
        use std::f32::consts::PI;

        let img_size = self.image_size;
        let mut img: RgbaImage = ImageBuffer::from_pixel(img_size, img_size, Rgba(self.background));

        let center = img_size as f32 / 2.0;
        let r_min_f = r_min as f32;
        let r_max_f = (r_min + r_depth as u32) as f32;

        // Scale factor: map r_max to image edge with margin
        let scale = (img_size as f32 * 0.48) / r_max_f;

        // Pixel-based rendering
        for py in 0..img_size {
            for px in 0..img_size {
                let x = px as f32 - center;
                let y = py as f32 - center;

                let pixel_r = (x * x + y * y).sqrt() / scale;

                if pixel_r < r_min_f || pixel_r >= r_max_f {
                    continue;
                }

                let r_index = (pixel_r - r_min_f) as usize;
                if r_index >= r_depth as usize {
                    continue;
                }

                let mut angle = y.atan2(x);
                if angle < 0.0 {
                    angle += 2.0 * PI;
                }

                let theta_index = ((angle / (2.0 * PI)) * theta_divisions as f32) as usize
                    % theta_divisions as usize;

                let frame_idx = r_index * theta_divisions as usize + theta_index;
                if frame_idx >= frame_data.len() {
                    continue;
                }

                let value = frame_data[frame_idx] as usize;

                if value >= self.colors.len() || self.colors[value][3] == 0 {
                    continue;
                }

                img.put_pixel(px, py, Rgba(self.colors[value]));
            }
        }

        Ok(img)
    }

    /// Export individual frames as PNG files (for debugging or custom processing).
    pub fn export_frames<P: AsRef<Path>>(
        &self,
        output_dir: P,
        prefix: &str,
    ) -> Result<Vec<std::path::PathBuf>, VideoError> {
        std::fs::create_dir_all(&output_dir)?;

        let mut paths = Vec::new();
        let frame_count = self.archive.frame_count();
        let digits = ((frame_count as f32).log10() as usize) + 1;

        for i in 0..frame_count {
            let frame_data = self.archive.frame(i).unwrap();
            let image = self.render_frame(frame_data)?;

            let filename = format!("{}_{:0width$}.png", prefix, i, width = digits);
            let path = output_dir.as_ref().join(&filename);
            image.save(&path)?;
            paths.push(path);
        }

        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_sampling_more_source_than_target() {
        // 100 source frames, 30 fps, 2 second video = 60 target frames
        // Should sample every ~1.67 frames
        let source = 100;
        let target = 60;

        let indices: Vec<usize> = (0..target).map(|i| (i * source) / target).collect();

        assert_eq!(indices.len(), 60);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[59], 98); // Last should be near end
    }

    #[test]
    fn test_frame_sampling_fewer_source_than_target() {
        // 30 source frames, 30 fps, 2 second video = 60 target frames
        // Should duplicate frames
        let source = 30;
        let target = 60;

        let indices: Vec<usize> = (0..target).map(|i| (i * source) / target).collect();

        assert_eq!(indices.len(), 60);
        // Frame 0 should appear twice
        assert_eq!(indices[0], 0);
        assert_eq!(indices[1], 0);
    }
}
