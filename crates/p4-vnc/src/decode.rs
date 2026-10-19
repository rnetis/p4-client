//! Framebuffer-update decoders. Raw (encoding 0) and CopyRect (encoding 1).
//! Decoded pixels are written straight into the framebuffer in its native
//! RGB565 layout — no intermediate copies (PRD §10).

use crate::framebuffer::Framebuffer;
use crate::Rect;
use p4_core::{P4Error, P4Result};

/// Raw encoding: 8/16/32 bpp per server's chosen pixel format.
/// `bpp` in bits, `depth` unused here since we fix the client format per mode.
pub fn raw(rect: &Rect, data: &[u8], fb: &mut Framebuffer) -> P4Result<()> {
    let bpp = fb.client_bytes_per_pixel();
    let expected = rect.w as usize * rect.h as usize * bpp;
    if data.len() < expected {
        return Err(P4Error::Protocol(format!(
            "raw rect short: got {} want {}",
            data.len(),
            expected
        )));
    }
    for row in 0..rect.h as usize {
        for col in 0..rect.w as usize {
            let src = &data[(row * rect.w as usize + col) * bpp..][..bpp];
            let px = match bpp {
                1 => rgb332_to_rgb565(src[0]),
                2 => u16::from_be_bytes([src[0], src[1]]),
                4 => rgb888_to_rgb565(src[0], src[1], src[2]), // little-endian RGBX
                _ => return Err(P4Error::Unsupported("bpp".into())),
            };
            fb.set_pixel(rect.x as usize + col, rect.y as usize + row, px);
        }
    }
    Ok(())
}

/// CopyRect: blit a region from elsewhere in the framebuffer.
pub fn copy_rect(rect: &Rect, src_x: u16, src_y: u16, fb: &mut Framebuffer) {
    fb.copy_region(
        src_x as usize,
        src_y as usize,
        rect.x as usize,
        rect.y as usize,
        rect.w as usize,
        rect.h as usize,
    );
}

fn rgb332_to_rgb565(v: u8) -> u16 {
    let r = ((v >> 5) & 7) as u16;
    let g = ((v >> 2) & 7) as u16;
    let b = (v & 3) as u16;
    ((r * 9) as u16 / 2) << 11 | ((g * 9) / 2) << 5 | (b * 41) / 3
}

fn rgb888_to_rgb565(r: u8, g: u8, b: u8) -> u16 {
    (r as u16 >> 3) << 11 | (g as u16 >> 2) << 5 | (b as u16 >> 3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framebuffer::{Framebuffer, PixelFormat};

    #[test]
    fn raw_rgb565_roundtrip() {
        let mut fb = Framebuffer::new(4, 2, PixelFormat::RGB565);
        let rect = Rect { x: 0, y: 0, w: 4, h: 2 };
        let data: Vec<u8> = [0xF8_1Fu16, 0x07_E0, 0x00_1F, 0xFF_FF, 0, 0, 0, 0]
            .iter()
            .flat_map(|p| p.to_be_bytes())
            .collect();
        raw(&rect, &data, &mut fb).unwrap();
        assert_eq!(fb.pixel(0, 0), 0xF81F); // red
        assert_eq!(fb.pixel(1, 0), 0x07E0); // green
        assert_eq!(fb.pixel(2, 0), 0x001F); // blue
        assert_eq!(fb.pixel(3, 0), 0xFFFF); // white
    }

    #[test]
    fn raw_rejects_short_data() {
        let mut fb = Framebuffer::new(4, 2, PixelFormat::RGB565);
        let rect = Rect { x: 0, y: 0, w: 4, h: 2 };
        assert!(matches!(raw(&rect, &[0, 0, 0], &mut fb), Err(P4Error::Protocol(_))));
    }

    #[test]
    fn copy_rect_moves_pixels() {
        let mut fb = Framebuffer::new(4, 1, PixelFormat::RGB565);
        fb.set_pixel(0, 0, 0x1234);
        let rect = Rect { x: 2, y: 0, w: 1, h: 1 };
        copy_rect(&rect, 0, 0, &mut fb);
        assert_eq!(fb.pixel(2, 0), 0x1234);
    }
}
