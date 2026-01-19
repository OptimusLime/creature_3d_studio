//! RGBA pixel buffer for rendering.

/// RGBA pixel buffer for render output.
///
/// Stores pixels in row-major order with 4 bytes per pixel (RGBA).
#[derive(Clone)]
pub struct PixelBuffer {
    /// Raw RGBA pixel data.
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
}

impl PixelBuffer {
    /// Create a new pixel buffer filled with transparent black.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            data: vec![0; width * height * 4],
            width,
            height,
        }
    }

    /// Create a pixel buffer filled with a solid color.
    pub fn with_color(width: usize, height: usize, color: [u8; 4]) -> Self {
        let mut buffer = Self::new(width, height);
        for y in 0..height {
            for x in 0..width {
                buffer.set_pixel(x, y, color);
            }
        }
        buffer
    }

    /// Set a pixel at (x, y) to the given RGBA color.
    ///
    /// Does nothing if coordinates are out of bounds.
    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: [u8; 4]) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) * 4;
            self.data[idx..idx + 4].copy_from_slice(&color);
        }
    }

    /// Get the RGBA color at (x, y).
    ///
    /// Returns transparent black if out of bounds.
    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> [u8; 4] {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) * 4;
            [
                self.data[idx],
                self.data[idx + 1],
                self.data[idx + 2],
                self.data[idx + 3],
            ]
        } else {
            [0, 0, 0, 0]
        }
    }

    /// Blend a pixel with alpha compositing (source over destination).
    ///
    /// Uses standard Porter-Duff "source over" compositing.
    #[inline]
    pub fn blend_pixel(&mut self, x: usize, y: usize, color: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }

        let src_a = color[3] as f32 / 255.0;
        if src_a == 0.0 {
            return; // Fully transparent, nothing to blend
        }
        if src_a == 1.0 {
            self.set_pixel(x, y, color); // Fully opaque, just overwrite
            return;
        }

        let dst = self.get_pixel(x, y);
        let dst_a = dst[3] as f32 / 255.0;

        let out_a = src_a + dst_a * (1.0 - src_a);
        if out_a == 0.0 {
            self.set_pixel(x, y, [0, 0, 0, 0]);
            return;
        }

        let blend = |s: u8, d: u8| -> u8 {
            let s = s as f32 / 255.0;
            let d = d as f32 / 255.0;
            let out = (s * src_a + d * dst_a * (1.0 - src_a)) / out_a;
            (out * 255.0).clamp(0.0, 255.0) as u8
        };

        self.set_pixel(
            x,
            y,
            [
                blend(color[0], dst[0]),
                blend(color[1], dst[1]),
                blend(color[2], dst[2]),
                (out_a * 255.0) as u8,
            ],
        );
    }

    /// Fill entire buffer with a color.
    pub fn fill(&mut self, color: [u8; 4]) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.set_pixel(x, y, color);
            }
        }
    }

    /// Clear to transparent black.
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    /// Get raw RGBA data as slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buf = PixelBuffer::new(4, 4);
        assert_eq!(buf.width, 4);
        assert_eq!(buf.height, 4);
        assert_eq!(buf.data.len(), 64); // 4 * 4 * 4
        assert_eq!(buf.get_pixel(0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn test_set_get_pixel() {
        let mut buf = PixelBuffer::new(4, 4);
        buf.set_pixel(1, 2, [255, 128, 64, 255]);
        assert_eq!(buf.get_pixel(1, 2), [255, 128, 64, 255]);
        assert_eq!(buf.get_pixel(0, 0), [0, 0, 0, 0]); // unchanged
    }

    #[test]
    fn test_out_of_bounds() {
        let mut buf = PixelBuffer::new(4, 4);
        buf.set_pixel(10, 10, [255, 0, 0, 255]); // Should not crash
        assert_eq!(buf.get_pixel(10, 10), [0, 0, 0, 0]); // Returns transparent
    }

    #[test]
    fn test_blend_opaque() {
        let mut buf = PixelBuffer::new(2, 2);
        buf.set_pixel(0, 0, [100, 100, 100, 255]);
        buf.blend_pixel(0, 0, [200, 200, 200, 255]); // Opaque overwrites
        assert_eq!(buf.get_pixel(0, 0), [200, 200, 200, 255]);
    }

    #[test]
    fn test_blend_transparent() {
        let mut buf = PixelBuffer::new(2, 2);
        buf.set_pixel(0, 0, [100, 100, 100, 255]);
        buf.blend_pixel(0, 0, [200, 200, 200, 0]); // Transparent does nothing
        assert_eq!(buf.get_pixel(0, 0), [100, 100, 100, 255]);
    }

    #[test]
    fn test_blend_semi_transparent() {
        let mut buf = PixelBuffer::new(2, 2);
        buf.set_pixel(0, 0, [0, 0, 0, 255]); // Black
        buf.blend_pixel(0, 0, [255, 255, 255, 128]); // 50% white
        let result = buf.get_pixel(0, 0);
        // Should be grayish (around 128)
        assert!(result[0] > 100 && result[0] < 150);
    }

    #[test]
    fn test_fill() {
        let mut buf = PixelBuffer::new(2, 2);
        buf.fill([255, 0, 0, 255]);
        assert_eq!(buf.get_pixel(0, 0), [255, 0, 0, 255]);
        assert_eq!(buf.get_pixel(1, 1), [255, 0, 0, 255]);
    }
}
