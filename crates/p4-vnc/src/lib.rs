//! RFB/VNC protocol client (PRD §9, §10).
//!
//! Protocol logic is split into pure, testable pieces:
//! - `handshake`: RFB 3.8 handshake + VNC DES authentication
//! - `messages`: client->server message builders
//! - `decode`: framebuffer-update rect decoders (Raw, CopyRect)
//! - `framebuffer`: RGB565 framebuffer with dirty-rectangle updates
//!
//! `VncClient` drives a generic duplex stream (TCP on host/device).

pub mod decode;
pub mod framebuffer;
pub mod handshake;
pub mod messages;

use p4_core::{P4Error, P4Result};
use std::io::{Read, Write};

pub use framebuffer::{PixelFormat, Rect};

/// Connection quality mode (PRD §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityMode {
    LowBandwidth,
    Balanced,
    Quality,
}

impl QualityMode {
    /// SetPixelFormat pixel format per mode.
    pub fn pixel_format(self) -> PixelFormat {
        match self {
            QualityMode::LowBandwidth => PixelFormat::RGB332,
            QualityMode::Balanced => PixelFormat::RGB565,
            QualityMode::Quality => PixelFormat::RGB888,
        }
    }
}

/// RFB 3.8 client over any Read+Write stream.
pub struct VncClient<S: Read + Write> {
    stream: S,
    pub width: u16,
    pub height: u16,
    pub quality: QualityMode,
}

impl<S: Read + Write> VncClient<S> {
    pub fn new(stream: S, quality: QualityMode) -> Self {
        Self {
            stream,
            width: 0,
            height: 0,
            quality,
        }
    }

    /// Full connect: version exchange, security, auth, init. Returns the
    /// remote framebuffer dimensions.
    pub fn connect(&mut self, password: Option<&str>) -> P4Result<(u16, u16)> {
        handshake::do_handshake(&mut self.stream, password)?;
        let (w, h) = self.send_init()?;
        self.width = w;
        self.height = h;
        self.configure_encodings()?;
        Ok((w, h))
    }

    fn send_init(&mut self) -> P4Result<(u16, u16)> {
        // SetPixelFormat (shared=1) — 20 bytes
        let pf = self.quality.pixel_format();
        let mut m = vec![0u8; 20];
        m[0] = 0;
        m[3..19].copy_from_slice(&pf.to_wire());
        self.stream.write_all(&m)?;
        // ClientInit (shared=true)
        self.stream.write_all(&[1])?;
        // ServerInit: width u16, height u16, pixelformat 16, name-length u32
        let mut hdr = [0u8; 24];
        self.stream.read_exact(&mut hdr)?;
        let w = u16::from_be_bytes([hdr[0], hdr[1]]);
        let h = u16::from_be_bytes([hdr[2], hdr[3]]);
        let name_len = u32::from_be_bytes([hdr[20], hdr[21], hdr[22], hdr[23]]);
        let mut name = vec![0u8; name_len as usize];
        if name_len > 0 {
            self.stream.read_exact(&mut name)?;
        }
        log::info!(
            "VNC connected: {}x{} name={}",
            w,
            h,
            String::from_utf8_lossy(&name)
        );
        Ok((w, h))
    }

    /// Prefer Hextile, fall back to Raw (MVP decoders; PRD prioritizes
    /// responsiveness over image quality).
    fn configure_encodings(&mut self) -> P4Result<()> {
        let encodings: Vec<i32> = vec![
            -239, /*pseudo cursor*/
            5,    /*hextile*/
            0,    /*raw*/
        ];
        self.stream
            .write_all(&messages::set_encodings(&encodings))?;
        Ok(())
    }

    /// Request an incremental framebuffer update for a region (PRD §10:
    /// only changed regions).
    pub fn request_update(&mut self, incremental: bool) -> P4Result<()> {
        self.stream
            .write_all(&messages::framebuffer_update_request(
                0,
                0,
                self.width,
                self.height,
                incremental,
            ))?;
        Ok(())
    }

    /// Block until the next framebuffer update; apply decoded rects to `fb`.
    /// Returns the dirty region(s) applied.
    pub fn read_update(&mut self, fb: &mut framebuffer::Framebuffer) -> P4Result<Vec<Rect>> {
        let mut msg_type = [0u8; 1];
        self.stream.read_exact(&mut msg_type)?;
        match msg_type[0] {
            0 => {
                // FramebufferUpdate: 1 pad, 2 nrects
                let mut hdr = [0u8; 3];
                self.stream.read_exact(&mut hdr)?;
                let nrects = u16::from_be_bytes([hdr[1], hdr[2]]);
                let mut dirty = Vec::new();
                for _ in 0..nrects {
                    let mut rh = [0u8; 12];
                    self.stream.read_exact(&mut rh)?;
                    let rect = Rect {
                        x: u16::from_be_bytes([rh[0], rh[1]]),
                        y: u16::from_be_bytes([rh[2], rh[3]]),
                        w: u16::from_be_bytes([rh[4], rh[5]]),
                        h: u16::from_be_bytes([rh[6], rh[7]]),
                    };
                    let enc = i32::from_be_bytes([rh[8], rh[9], rh[10], rh[11]]);
                    match enc {
                        0 => {
                            let bytes =
                                (rect.w as usize) * (rect.h as usize) * self.bytes_per_pixel();
                            let mut data = vec![0u8; bytes];
                            self.stream.read_exact(&mut data)?;
                            decode::raw(&rect, &data, fb)?;
                            dirty.push(rect);
                        }
                        1 => {
                            // CopyRect: src x,y
                            let mut src = [0u8; 4];
                            self.stream.read_exact(&mut src)?;
                            let sx = u16::from_be_bytes([src[0], src[1]]);
                            let sy = u16::from_be_bytes([src[2], src[3]]);
                            decode::copy_rect(&rect, sx, sy, fb);
                            dirty.push(rect);
                        }
                        other => {
                            return Err(P4Error::Protocol(format!("unsupported encoding {other}")))
                        }
                    }
                }
                Ok(dirty)
            }
            1 => {
                // SetColourMapEntries — skip
                let mut hdr = [0u8; 5];
                self.stream.read_exact(&mut hdr)?;
                let n = u16::from_be_bytes([hdr[3], hdr[4]]);
                let mut skip = vec![0u8; n as usize * 6];
                self.stream.read_exact(&mut skip)?;
                Ok(Vec::new())
            }
            2 => Ok(Vec::new()), // Bell
            3 => {
                // ServerCutText
                let mut hdr = [0u8; 7];
                self.stream.read_exact(&mut hdr)?;
                let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as usize;
                if len > 1_048_576 {
                    return Err(P4Error::Protocol("cut text too large".into()));
                }
                let mut skip = vec![0u8; len];
                self.stream.read_exact(&mut skip)?;
                Ok(Vec::new())
            }
            other => Err(P4Error::Protocol(format!("unknown server message {other}"))),
        }
    }

    pub fn send_pointer(&mut self, x: u16, y: u16, mask: u8) -> P4Result<()> {
        Ok(self
            .stream
            .write_all(&messages::pointer_event(x, y, mask))?)
    }

    pub fn send_key(&mut self, keysym: u32, down: bool) -> P4Result<()> {
        Ok(self.stream.write_all(&messages::key_event(keysym, down))?)
    }

    pub fn bytes_per_pixel(&self) -> usize {
        match self.quality.pixel_format() {
            PixelFormat::RGB332 => 1,
            PixelFormat::RGB565 => 2,
            PixelFormat::RGB888 => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_formats_per_quality() {
        assert_eq!(
            QualityMode::LowBandwidth.pixel_format(),
            PixelFormat::RGB332
        );
        assert_eq!(QualityMode::Balanced.pixel_format(), PixelFormat::RGB565);
        assert_eq!(QualityMode::Quality.pixel_format(), PixelFormat::RGB888);
    }
}
