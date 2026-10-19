//! Hardware Abstraction Layer traits (PRD §3.1, Rule 1).
//!
//! Application code depends only on these traits. Board-specific
//! implementations live in `firmware/esp/*` (ESP-IDF) and host mocks in tests.

/// Monotonic + wall clock.
pub trait Clock {
    fn unix_time(&self) -> u64;
    fn uptime_ms(&self) -> u64;
}

/// Display back-end: push dirty rectangles to the framebuffer.
pub trait Display {
    fn size(&self) -> (u16, u16);
    fn flush(&mut self, x: u16, y: u16, w: u16, h: u16, pixels: &[u8]);
    fn set_brightness(&mut self, percent: u8);
}

/// Touch controller.
pub trait TouchDevice {
    /// Returns pending touch points (x, y, pressure, contact).
    fn read(&mut self) -> Vec<(u16, u16, u8, bool)>;
}

/// Battery gauge, where the board provides one (PRD §8: "if available").
pub trait Battery {
    fn percent(&self) -> Option<u8>;
    fn charging(&self) -> Option<bool>;
}

/// Persistent key/value storage back-end (NVS on device).
pub trait KeyValueStore {
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    fn set(&mut self, key: &str, value: &[u8]);
    fn remove(&mut self, key: &str);
}

/// Non-unique secret material used to protect stored credentials (PRD §26).
pub trait KeyProvider {
    fn key(&self) -> [u8; 32];
}
