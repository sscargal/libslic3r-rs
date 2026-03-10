//! RGBA framebuffer with z-buffer for rasterization.

/// An RGBA pixel framebuffer with a depth buffer for z-testing.
pub(crate) struct Framebuffer {
    width: u32,
    height: u32,
    pixels: Vec<[u8; 4]>,
    depth: Vec<f32>,
}

impl Framebuffer {
    /// Creates a new framebuffer initialized to the given background color.
    /// Depth buffer is initialized to f32::INFINITY (everything is behind).
    pub fn new(width: u32, height: u32, background: [u8; 4]) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            width,
            height,
            pixels: vec![background; size],
            depth: vec![f32::INFINITY; size],
        }
    }

    /// Sets a pixel at (x, y) with z-testing.
    /// Only writes if z < current depth at that pixel (closer to camera).
    /// Out-of-bounds writes are silently ignored.
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, z: f32, color: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        if z < self.depth[idx] {
            self.depth[idx] = z;
            self.pixels[idx] = color;
        }
    }

    /// Returns a reference to the pixel data.
    pub fn pixels(&self) -> &[[u8; 4]] {
        &self.pixels
    }

    /// Returns the framebuffer width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the framebuffer height.
    pub fn height(&self) -> u32 {
        self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_framebuffer_correct_size() {
        let fb = Framebuffer::new(100, 100, [0, 0, 0, 0]);
        assert_eq!(fb.pixels().len(), 10000);
        // All pixels should be transparent
        for px in fb.pixels() {
            assert_eq!(*px, [0, 0, 0, 0]);
        }
    }

    #[test]
    fn z_test_closer_overwrites() {
        let mut fb = Framebuffer::new(100, 100, [0, 0, 0, 0]);
        // Write red at z=0.5
        fb.set_pixel(5, 5, 0.5, [255, 0, 0, 255]);
        let idx = 5 * 100 + 5;
        assert_eq!(fb.pixels()[idx], [255, 0, 0, 255]);

        // Write green at z=0.3 (closer) -- should overwrite
        fb.set_pixel(5, 5, 0.3, [0, 255, 0, 255]);
        assert_eq!(fb.pixels()[idx], [0, 255, 0, 255]);

        // Write blue at z=0.8 (farther) -- should NOT overwrite
        fb.set_pixel(5, 5, 0.8, [0, 0, 255, 255]);
        assert_eq!(fb.pixels()[idx], [0, 255, 0, 255]);
    }

    #[test]
    fn out_of_bounds_silent() {
        let mut fb = Framebuffer::new(10, 10, [0, 0, 0, 0]);
        // These should not panic
        fb.set_pixel(10, 5, 0.5, [255, 0, 0, 255]);
        fb.set_pixel(5, 10, 0.5, [255, 0, 0, 255]);
        fb.set_pixel(100, 100, 0.5, [255, 0, 0, 255]);
    }
}
