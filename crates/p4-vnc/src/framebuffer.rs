//! RGB565 framebuffer with dirty-rectangle support (PRD §10, §28: reuse
//! buffers, limit framebuffer memory).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    RGB332,
    RGB565,
    RGB888,
}

impl PixelFormat {
    /// RFB SetPixelFormat wire layout (16 bytes): bpp, depth, big-endian,
    /// true-color, max-red/green/blue, shift-red/green/blue, padding.
    pub fn to_wire(self) -> [u8; 16] {
        match self {
            PixelFormat::RGB332 => [8, 8, 0, 1, 7, 3, 3, 0, 7, 2, 5, 0, 5, 0, 0, 0],
            PixelFormat::RGB565 => [16, 16, 0, 1, 31, 3, 3, 0, 63, 2, 5, 0, 5, 0, 0, 0],
            PixelFormat::RGB888 => [32, 24, 0, 1, 255, 3, 3, 0, 255, 2, 8, 0, 255, 0, 16, 0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

pub struct Framebuffer {
    width: usize,
    height: usize,
    format: PixelFormat,
    /// Native RGB565 pixels.
    pixels: Vec<u16>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize, format: PixelFormat) -> Self {
        Self {
            width,
            height,
            format,
            pixels: vec![0u16; width * height],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn format(&self) -> PixelFormat {
        self.format
    }

    /// Bytes per pixel of the wire format negotiated with the server.
    pub fn client_bytes_per_pixel(&self) -> usize {
        match self.format {
            PixelFormat::RGB332 => 1,
            PixelFormat::RGB565 => 2,
            PixelFormat::RGB888 => 4,
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> u16 {
        self.pixels[y * self.width + x]
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, v: u16) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = v;
        }
    }

    pub fn pixels(&self) -> &[u16] {
        &self.pixels
    }

    /// Blit from (sx, sy) to (dx, dy), w x h. Overlap-safe via copy buffer.
    pub fn copy_region(&mut self, sx: usize, sy: usize, dx: usize, dy: usize, w: usize, h: usize) {
        let mut buf = Vec::with_capacity(w * h);
        for row in 0..h {
            for col in 0..w {
                let (px, py) = (sx + col, sy + row);
                let v = if px < self.width && py < self.height {
                    self.pixel(px, py)
                } else {
                    0
                };
                buf.push(v);
            }
        }
        for row in 0..h {
            for col in 0..w {
                self.set_pixel(dx + col, dy + row, buf[row * w + col]);
            }
        }
    }

    /// Fill a rect with a solid RGB565 color.
    pub fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, color: u16) {
        for row in y..(y + h).min(self.height) {
            for col in x..(x + w).min(self.width) {
                self.set_pixel(col, row, color);
            }
        }
    }

    /// Memory footprint (PRD §28: limit VNC framebuffer memory).
    pub fn bytes(&self) -> usize {
        self.pixels.len() * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_and_bounds() {
        let mut fb = Framebuffer::new(10, 10, PixelFormat::RGB565);
        fb.set_pixel(3, 4, 0xABCD);
        assert_eq!(fb.pixel(3, 4), 0xABCD);
        fb.set_pixel(100, 100, 0x1234); // ignored, no panic
        assert_eq!(fb.pixel(9, 9), 0);
    }

    #[test]
    fn fill_and_memory() {
        let mut fb = Framebuffer::new(8, 8, PixelFormat::RGB565);
        fb.fill(2, 2, 4, 4, 0xFFFF);
        assert_eq!(fb.pixel(2, 2), 0xFFFF);
        assert_eq!(fb.pixel(0, 0), 0);
        assert_eq!(fb.bytes(), 8 * 8 * 2);
    }

    #[test]
    fn copy_region_overlapping() {
        let mut fb = Framebuffer::new(4, 1, PixelFormat::RGB565);
        fb.set_pixel(0, 0, 1);
        fb.set_pixel(1, 0, 2);
        fb.set_pixel(2, 0, 3);
        // copy [0..2] to [1..3] (overlapping)
        fb.copy_region(0, 0, 1, 0, 2, 1);
        assert_eq!(fb.pixel(1, 0), 1);
        assert_eq!(fb.pixel(2, 0), 2);
    }
}
