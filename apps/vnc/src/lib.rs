//! VNC app (PRD §9, §10): profiles, connect, touch->pointer, keyboard,
//! auto-reconnect, quality modes.

use p4_core::{App, P4Error, P4Result};
use p4_settings::{ProfileStore, VncQuality};
use p4_vnc::{QualityMode, VncClient};
use std::io::{Read, Write};
use std::net::TcpStream;

pub struct VncApp {
    client: Option<VncClient<TcpStream>>,
    pub connected_profile: Option<String>,
    pub quality: VncQuality,
    auto_reconnect: bool,
}

impl VncApp {
    pub fn new(settings: &p4_settings::VncSettings) -> Self {
        Self {
            client: None,
            connected_profile: None,
            quality: settings.quality.clone(),
            auto_reconnect: settings.auto_reconnect,
        }
    }

    pub fn quality_mode(&self) -> QualityMode {
        match self.quality {
            VncQuality::LowBandwidth => QualityMode::LowBandwidth,
            VncQuality::Balanced => QualityMode::Balanced,
            VncQuality::Quality => QualityMode::Quality,
        }
    }

    /// Connect to a profile, decrypting its password via the key provider.
    pub fn connect(
        &mut self,
        store: &ProfileStore,
        profile: &str,
        keys: &impl p4_core::hal::KeyProvider,
    ) -> P4Result<()> {
        let p = store
            .profiles()
            .iter()
            .find(|p| p.name == profile)
            .ok_or_else(|| P4Error::Config(format!("no profile '{profile}'")))?;
        let password = p
            .password_blob
            .as_ref()
            .map(|_| store.password(profile, keys))
            .transpose()?;
        let password_str = password
            .as_deref()
            .map(|b| String::from_utf8_lossy(b).into_owned());
        let addr = format!("{}:{}", p.hostname, p.port);
        let stream =
            TcpStream::connect(&addr).map_err(|e| P4Error::Network(format!("{addr}: {e}")))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).ok();
        let mut client = VncClient::new(stream, self.quality_mode());
        let (w, h) = client.connect(password_str.as_deref())?;
        log::info!("VNC profile '{profile}': {w}x{h}");
        self.client = Some(client);
        self.connected_profile = Some(profile.to_string());
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.client = None;
        self.connected_profile = None;
    }

    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    /// VNC prevents display sleep while connected (PRD §21).
    pub fn prevents_sleep(&self) -> bool {
        self.is_connected()
    }

    /// Pump one framebuffer update into `fb`; on I/O failure with auto
    /// reconnect configured, drop the link (caller may re-`connect`).
    pub fn pump(
        &mut self,
        fb: &mut p4_vnc::framebuffer::Framebuffer,
    ) -> P4Result<Vec<p4_vnc::Rect>> {
        match self.client.as_mut() {
            Some(c) => match c.request_update(true).and_then(|_| c.read_update(fb)) {
                Ok(dirty) => Ok(dirty),
                Err(e) => {
                    log::warn!("VNC link lost: {e}");
                    self.disconnect();
                    Err(e)
                }
            },
            None => Err(P4Error::Network("not connected".into())),
        }
    }

    /// Touch event (from p4-input mapping) -> PointerEvent on the wire.
    pub fn send_pointer(&mut self, x: u16, y: u16, mask: u8) -> P4Result<()> {
        self.client
            .as_mut()
            .ok_or_else(|| P4Error::Network("not connected".into()))?
            .send_pointer(x, y, mask)
    }

    pub fn send_key(&mut self, keysym: u32, down: bool) -> P4Result<()> {
        self.client
            .as_mut()
            .ok_or_else(|| P4Error::Network("not connected".into()))?
            .send_key(keysym, down)
    }
}

impl App for VncApp {
    fn id(&self) -> &'static str {
        "vnc"
    }
    fn display_name(&self) -> &'static str {
        "VNC"
    }
    fn launch(&mut self) -> P4Result<()> {
        Ok(())
    }
    fn suspend(&mut self) -> P4Result<()> {
        Ok(())
    }
    fn resume(&mut self) -> P4Result<()> {
        Ok(())
    }
    fn close(&mut self) -> P4Result<()> {
        self.disconnect();
        Ok(())
    }
    fn update(&mut self) {}

    fn stays_active_in_background(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p4_settings::{ProfileStore, StaticKeyProvider, VncProfile, VncSettings};
    use std::time::Duration;

    struct NoopKey([u8; 32]);
    impl p4_core::hal::KeyProvider for NoopKey {
        fn key(&self) -> [u8; 32] {
            self.0
        }
    }

    /// Minimal RFB 3.8 mock server: handshake + one raw framebuffer update.
    fn mock_server(port: u16, password: impl Into<String> + Send + 'static) -> std::thread::JoinHandle<()> {
        let password = password.into();
        std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            sock.write_all(b"RFB 003.008\n").unwrap();
            let mut ver = [0u8; 12];
            sock.read_exact(&mut ver).unwrap();
            if !password.is_empty() {
                sock.write_all(&[1, 2]).unwrap();
                let mut sel = [0u8; 1];
                sock.read_exact(&mut sel).unwrap();
                assert_eq!(sel[0], 2);
                sock.write_all(&[0x11; 16]).unwrap();
                let mut resp = [0u8; 16];
                sock.read_exact(&mut resp).unwrap();
                sock.write_all(&[0, 0, 0, 0]).unwrap();
            } else {
                sock.write_all(&[1, 1]).unwrap();
                let mut sel = [0u8; 1];
                sock.read_exact(&mut sel).unwrap();
                // RFB 3.8: security result OK even for None auth.
                sock.write_all(&[0, 0, 0, 0]).unwrap();
            }
            // ServerInit after SetPixelFormat+ClientInit
            let mut init = [0u8; 20];
            sock.read_exact(&mut init).unwrap();
            let mut shared = [0u8; 1];
            sock.read_exact(&mut shared).unwrap();
            let name = b"mock";
            let mut si = Vec::new();
            si.extend_from_slice(&64u16.to_be_bytes());
            si.extend_from_slice(&48u16.to_be_bytes());
            si.extend_from_slice(&[16u8, 8, 0, 1]); // depth, big-endian, true-colour
            si.extend_from_slice(&[0u8; 12]); // max/shift RGB + padding (16-byte PF)
            si.extend_from_slice(&(name.len() as u32).to_be_bytes());
            si.extend_from_slice(name);
            sock.write_all(&si).unwrap();
            // SetEncodings
            let mut hdr = [0u8; 4];
            sock.read_exact(&mut hdr).unwrap();
            let n = u16::from_be_bytes([hdr[2], hdr[3]]);
            let mut skip = vec![0u8; n as usize * 4];
            sock.read_exact(&mut skip).unwrap();
            // First FB update request
            let mut req = [0u8; 10];
            sock.read_exact(&mut req).unwrap();
            // Send one raw rect 1x1 red at 0,0
            let mut up = Vec::new();
            up.push(0u8);
            up.push(0);
            up.extend_from_slice(&1u16.to_be_bytes()); // nrects
            up.extend_from_slice(&0u16.to_be_bytes()); // x
            up.extend_from_slice(&0u16.to_be_bytes()); // y
            up.extend_from_slice(&1u16.to_be_bytes()); // w
            up.extend_from_slice(&1u16.to_be_bytes()); // h
            up.extend_from_slice(&0i32.to_be_bytes()); // encoding: raw
            up.extend_from_slice(&0xF81Fu16.to_be_bytes()); // one RGB565 pixel
            sock.write_all(&up).unwrap();
            sock.flush().unwrap();
            // Keep the link open so the client can send input events.
            std::thread::sleep(Duration::from_millis(300));
        })
    }

    #[test]
    fn connect_and_pump_against_mock() {
        let server = mock_server(15901, "");
        std::thread::sleep(Duration::from_millis(100));

        let mut store = ProfileStore::new();
        store.upsert(VncProfile {
            name: "mock".into(),
            hostname: "127.0.0.1".into(),
            port: 15901,
            username: None,
            password_blob: None,
            scaling_mode: "fit".into(),
            color_depth: 16,
        });
        let mut app = VncApp::new(&VncSettings::default());
        app.connect(&store, "mock", &NoopKey([0; 32])).unwrap();
        assert_eq!(app.connected_profile.as_deref(), Some("mock"));
        assert!(app.prevents_sleep());

        let mut fb = p4_vnc::framebuffer::Framebuffer::new(64, 48, p4_vnc::PixelFormat::RGB565);
        let dirty = app.pump(&mut fb).unwrap();
        assert_eq!(dirty.len(), 1);
        app.send_pointer(10, 10, 1).unwrap();
        app.send_key(b'a' as u32, true).unwrap();
        app.disconnect();
        assert!(!app.is_connected());
        server.join().unwrap();
    }

    #[test]
    fn connect_unknown_profile_fails() {
        let mut app = VncApp::new(&VncSettings::default());
        let store = ProfileStore::new();
        assert!(app.connect(&store, "nope", &NoopKey([0; 32])).is_err());
    }

    #[test]
    fn quality_maps() {
        let mut s = VncSettings::default();
        s.quality = VncQuality::LowBandwidth;
        assert_eq!(VncApp::new(&s).quality_mode(), QualityMode::LowBandwidth);
    }
}
