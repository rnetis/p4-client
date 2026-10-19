//! Persistent configuration (PRD §19, §9.3) and the secret store (PRD §26:
//! VNC passwords must never be stored as plaintext).

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};
use p4_core::hal::KeyProvider;
use p4_core::{P4Error, P4Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VncQuality {
    #[serde(rename = "low")]
    LowBandwidth,
    #[serde(rename = "balanced")]
    Balanced,
    #[serde(rename = "quality")]
    Quality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub brightness: u8,
    pub theme: Theme,
    pub sleep_timeout: u32,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            brightness: 80,
            theme: Theme::Dark,
            sleep_timeout: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkSettings {
    pub dhcp: bool,
    pub static_ip: Option<String>,
    pub dns: Option<String>,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            dhcp: true,
            static_ip: None,
            dns: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VncSettings {
    pub default_profile: Option<String>,
    pub quality: VncQuality,
    pub auto_reconnect: bool,
}

impl Default for VncSettings {
    fn default() -> Self {
        Self {
            default_profile: None,
            quality: VncQuality::Balanced,
            auto_reconnect: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReaderSettings {
    pub font_size: u8,
    pub line_spacing: f32,
    pub theme: Theme,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        Self {
            font_size: 18,
            line_spacing: 1.4,
            theme: Theme::Dark,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub display: DisplaySettings,
    pub network: NetworkSettings,
    pub vnc: VncSettings,
    pub reader: ReaderSettings,
    pub device_name: String,
}

impl Settings {
    pub fn default_device() -> Self {
        Self {
            device_name: "p4-client".into(),
            ..Default::default()
        }
    }

    pub fn to_toml(&self) -> P4Result<String> {
        toml::to_string_pretty(self).map_err(|e| P4Error::Config(e.to_string()))
    }

    pub fn from_toml(s: &str) -> P4Result<Self> {
        toml::from_str(s).map_err(|e| P4Error::Config(e.to_string()))
    }
}

/// A saved VNC connection profile (PRD §9.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VncProfile {
    pub name: String,
    pub hostname: String,
    pub port: u16,
    pub username: Option<String>,
    /// Encrypted password blob (nonce || ciphertext) — never plaintext.
    pub password_blob: Option<String>,
    pub scaling_mode: String,
    pub color_depth: u8,
}

/// Store for VNC profiles; passwords are encrypted with AES-256-GCM using a
/// key supplied by the platform (`KeyProvider`, efuse-backed on device).
pub struct ProfileStore {
    profiles: Vec<VncProfile>,
}

impl ProfileStore {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    pub fn profiles(&self) -> &[VncProfile] {
        &self.profiles
    }

    pub fn upsert(&mut self, p: VncProfile) {
        match self.profiles.iter().position(|x| x.name == p.name) {
            Some(i) => self.profiles[i] = p,
            None => self.profiles.push(p),
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.profiles.retain(|p| p.name != name);
    }

    /// Encrypt and attach a password to a profile.
    pub fn set_password(
        &mut self,
        profile: &str,
        password: &str,
        keys: &impl KeyProvider,
    ) -> P4Result<()> {
        let blob = encrypt_secret(password.as_bytes(), keys.key())?;
        let p = self
            .profiles
            .iter_mut()
            .find(|p| p.name == profile)
            .ok_or_else(|| P4Error::Config("unknown profile".into()))?;
        p.password_blob = Some(blob);
        Ok(())
    }

    /// Decrypt a profile password on connect. Zeroization of the returned
    /// buffer is the caller's responsibility.
    pub fn password(&self, profile: &str, keys: &impl KeyProvider) -> P4Result<Vec<u8>> {
        let p = self
            .profiles
            .iter()
            .find(|p| p.name == profile)
            .ok_or_else(|| P4Error::Config("unknown profile".into()))?;
        let blob = p
            .password_blob
            .as_deref()
            .ok_or_else(|| P4Error::Config("no password set".into()))?;
        decrypt_secret(blob, keys.key())
    }

    pub fn to_json(&self) -> P4Result<String> {
        serde_json::to_string(&self.profiles).map_err(|e| P4Error::Config(e.to_string()))
    }

    pub fn from_json(s: &str) -> P4Result<Self> {
        Ok(Self {
            profiles: serde_json::from_str(s).map_err(|e| P4Error::Config(e.to_string()))?,
        })
    }
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self::new()
    }
}

fn encrypt_secret(plain: &[u8], key: [u8; 32]) -> P4Result<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, plain)
        .map_err(|_| P4Error::Security("encrypt failed".into()))?;
    let mut out = nonce.to_vec();
    out.extend(ct);
    Ok(base64_encode(&out))
}

fn decrypt_secret(blob: &str, key: [u8; 32]) -> P4Result<Vec<u8>> {
    let raw = base64_decode(blob)?;
    if raw.len() < 12 {
        return Err(P4Error::Security("blob too short".into()));
    }
    let (nonce, ct) = raw.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|_| P4Error::Security("decrypt failed (wrong key or tampered)".into()))
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn base64_decode(s: &str) -> P4Result<Vec<u8>> {
    let mut vals = Vec::new();
    for c in s.chars() {
        let v = match c {
            'A'..='Z' => c as u8 - b'A',
            'a'..='z' => c as u8 - b'a' + 26,
            '0'..='9' => c as u8 - b'0' + 52,
            '+' => 62,
            '/' => 63,
            '=' => continue,
            _ => return Err(P4Error::Security("bad base64".into())),
        };
        vals.push(v);
    }
    let mut out = Vec::new();
    for chunk in vals.chunks(4) {
        let mut n = 0u32;
        for (i, &v) in chunk.iter().enumerate() {
            n |= (v as u32) << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

/// Trivial file-backed KeyProvider for host tests / simulator. On device the
/// implementation reads the eFuse key block.
pub struct StaticKeyProvider(pub [u8; 32]);

impl KeyProvider for StaticKeyProvider {
    fn key(&self) -> [u8; 32] {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_toml() {
        let s = Settings::default_device();
        let t = s.to_toml().unwrap();
        assert!(t.contains("brightness = 80"));
        assert!(t.contains("theme = \"Dark\"") || t.contains("theme = 'Dark'"));
        let back = Settings::from_toml(&t).unwrap();
        assert_eq!(back.display.brightness, 80);
        assert_eq!(back.reader.line_spacing, 1.4);
    }

    #[test]
    fn defaults_match_prd() {
        let s = Settings::default_device();
        assert_eq!(s.display.sleep_timeout, 300);
        assert_eq!(s.vnc.quality, VncQuality::Balanced);
        assert_eq!(s.reader.font_size, 18);
        assert_eq!(s.device_name, "p4-client");
    }

    #[test]
    fn password_is_never_plaintext() {
        let keys = StaticKeyProvider([7u8; 32]);
        let mut store = ProfileStore::new();
        store.upsert(VncProfile {
            name: "work".into(),
            hostname: "192.168.1.20".into(),
            port: 5900,
            username: Some("dev".into()),
            password_blob: None,
            scaling_mode: "fit".into(),
            color_depth: 16,
        });
        store.set_password("work", "hunter2-secret", &keys).unwrap();
        let json = store.to_json().unwrap();
        assert!(!json.contains("hunter2-secret"), "plaintext leaked: {json}");
        assert_eq!(store.password("work", &keys).unwrap(), b"hunter2-secret");
    }

    #[test]
    fn wrong_key_fails_closed() {
        let mut store = ProfileStore::new();
        store.upsert(VncProfile {
            name: "work".into(),
            hostname: "h".into(),
            port: 5900,
            username: None,
            password_blob: None,
            scaling_mode: "fit".into(),
            color_depth: 16,
        });
        store
            .set_password("work", "pw", &StaticKeyProvider([1; 32]))
            .unwrap();
        assert!(store.password("work", &StaticKeyProvider([2; 32])).is_err());
    }

    #[test]
    fn profile_roundtrip_and_upsert() {
        let mut store = ProfileStore::new();
        store.upsert(VncProfile {
            name: "home".into(),
            hostname: "192.168.1.50".into(),
            port: 5900,
            username: None,
            password_blob: None,
            scaling_mode: "fit".into(),
            color_depth: 24,
        });
        store.upsert(VncProfile {
            name: "home".into(),
            hostname: "192.168.1.51".into(),
            port: 5901,
            username: None,
            password_blob: None,
            scaling_mode: "fit".into(),
            color_depth: 24,
        });
        assert_eq!(store.profiles().len(), 1);
        assert_eq!(store.profiles()[0].port, 5901);
    }

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_decode("Zm9v").unwrap(), b"foo");
    }
}
