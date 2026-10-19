//! Headless UI model (PRD §6, §8, §12).
//!
//! The layout/widget *model* is platform-independent and fully tested here;
//! on device it is rendered through LVGL (bindings live in `firmware/`).
//! Touch targets respect the 44 px minimum / 48–56 px preferred (PRD §6).

use p4_input::{Gesture, GestureRecognizer, InputEvent, TouchPhase};

pub const SCREEN_W: u16 = 800;
pub const SCREEN_H: u16 = 480;
pub const MIN_TOUCH_TARGET: u16 = 44;
pub const PREFERRED_TOUCH_TARGET: u16 = 56;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: ThemeName,
    pub bg: u16,
    pub fg: u16,
    pub accent: u16,
}

impl Theme {
    pub const DARK: Theme = Theme {
        name: ThemeName::Dark,
        bg: 0x1082,
        fg: 0xFFFF,
        accent: 0x051F,
    };
    pub const LIGHT: Theme = Theme {
        name: ThemeName::Light,
        bg: 0xFFFF,
        fg: 0x0000,
        accent: 0x21E0,
    };
    pub fn by_name(n: ThemeName) -> Theme {
        match n {
            ThemeName::Dark => Theme::DARK,
            ThemeName::Light => Theme::LIGHT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitBox {
    pub id: &'static str,
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl HitBox {
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Status bar state (PRD §8): time, Wi-Fi bars, battery, connection, offline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBar {
    pub time: String,
    pub wifi_bars: u8,
    pub battery_percent: Option<u8>,
    pub connected: bool,
}

impl StatusBar {
    pub fn offline_mode(&self) -> bool {
        self.wifi_bars == 0
    }

    /// "Wi-Fi ████" style text.
    pub fn wifi_text(&self) -> String {
        let bars: String = "█".repeat(self.wifi_bars as usize);
        format!("Wi-Fi {bars}")
    }
}

/// Launcher grid (PRD §8): 2x2 app tiles on a 7-inch 800x480 screen.
pub struct Launcher {
    pub apps: Vec<(&'static str, &'static str)>, // (id, label)
    hitboxes: Vec<HitBox>,
}

impl Launcher {
    pub fn new(apps: Vec<(&'static str, &'static str)>) -> Self {
        let mut hitboxes = Vec::new();
        let cols = 2usize;
        let tile_w = 200u16;
        let tile_h = 120u16;
        for (i, (id, _)) in apps.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let x =
                (SCREEN_W - tile_w * 2) / 3 + col as u16 * (tile_w + (SCREEN_W - tile_w * 2) / 3);
            let y = 80 + row as u16 * (tile_h + 40);
            hitboxes.push(HitBox {
                id,
                x,
                y,
                w: tile_w,
                h: tile_h,
            });
        }
        Self { apps, hitboxes }
    }

    pub fn hitboxes(&self) -> &[HitBox] {
        &self.hitboxes
    }

    /// Dispatch a tap to the launcher; returns the app id hit, if any.
    pub fn tap_at(&self, x: u16, y: u16) -> Option<&'static str> {
        self.hitboxes
            .iter()
            .find(|h| h.contains(x, y))
            .map(|h| h.id)
    }

    /// All tiles meet the minimum touch target.
    pub fn touch_targets_valid(&self) -> bool {
        self.hitboxes
            .iter()
            .all(|h| h.w >= MIN_TOUCH_TARGET && h.h >= MIN_TOUCH_TARGET)
    }
}

/// QWERTY virtual keyboard (PRD §12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Char(char),
    Shift,
    Backspace,
    Enter,
    Space,
    SwitchLayout(Layout),
    Dismiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Lower,
    Upper,
    Numeric,
    Symbols,
}

pub struct VirtualKeyboard {
    pub layout: Layout,
    pub shifted: bool,
}

impl Default for VirtualKeyboard {
    fn default() -> Self {
        Self {
            layout: Layout::Lower,
            shifted: false,
        }
    }
}

impl VirtualKeyboard {
    pub fn key(&mut self, row: usize, col: usize) -> KeyAction {
        match self.layout {
            Layout::Lower | Layout::Upper => {
                const ROWS: [&[u8]; 3] = [b"qwertyuiop", b"asdfghjkl", b"zxcvbnm"];
                let upper = self.layout == Layout::Upper || self.shifted;
                match (row, col) {
                    (3, 0) => {
                        self.shifted = !self.shifted;
                        return KeyAction::Shift;
                    }
                    (3, 1) => return KeyAction::Backspace,
                    (3, 2) => return KeyAction::Space,
                    (3, 3) => return KeyAction::Enter,
                    (3, 4) => return KeyAction::SwitchLayout(Layout::Numeric),
                    _ => {}
                }
                if let Some(&c) = ROWS.get(row).and_then(|r| r.get(col)) {
                    let c = if upper { c.to_ascii_uppercase() } else { c };
                    if self.shifted {
                        self.shifted = false; // shift is one-shot
                    }
                    KeyAction::Char(c as char)
                } else {
                    KeyAction::SwitchLayout(Layout::Symbols)
                }
            }
            Layout::Numeric => {
                const ROWS: [&str; 3] = ["1234567890", "-/:;()€&@", ".,?!'"];
                match (row, col) {
                    (3, 2) => return KeyAction::Space,
                    (3, 3) => return KeyAction::Enter,
                    (3, 4) => {
                        self.layout = Layout::Lower;
                        return KeyAction::SwitchLayout(Layout::Lower);
                    }
                    _ => {}
                }
                ROWS[row]
                    .chars()
                    .nth(col)
                    .map(KeyAction::Char)
                    .unwrap_or(KeyAction::SwitchLayout(Layout::Symbols))
            }
            Layout::Symbols => {
                const ROWS: [&str; 3] = [r#"[]{}#%^*+=_"#, r#"\|~<>$€£¥•"#, ".,?!'"];
                match (row, col) {
                    (3, 2) => return KeyAction::Space,
                    (3, 3) => return KeyAction::Enter,
                    (3, 4) => {
                        self.layout = Layout::Lower;
                        return KeyAction::SwitchLayout(Layout::Lower);
                    }
                    _ => {}
                }
                ROWS[row]
                    .chars()
                    .nth(col)
                    .map(KeyAction::Char)
                    .unwrap_or(KeyAction::Backspace)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p4_input::TouchEvent;

    #[test]
    fn launcher_tiles_meet_touch_target() {
        let l = Launcher::new(vec![
            ("vnc", "VNC"),
            ("reader", "Books"),
            ("files", "Files"),
            ("settings", "Settings"),
        ]);
        assert_eq!(l.hitboxes().len(), 4);
        assert!(l.touch_targets_valid());
    }

    #[test]
    fn launcher_dispatch_by_coordinate() {
        let l = Launcher::new(vec![("vnc", "VNC"), ("reader", "Books")]);
        let hb = l.hitboxes()[0];
        assert_eq!(l.tap_at(hb.x + 5, hb.y + 5), Some("vnc"));
        assert_eq!(l.tap_at(hb.x + hb.w + 10, hb.y), None);
        let hb1 = l.hitboxes()[1];
        assert_eq!(l.tap_at(hb1.x + 5, hb1.y + 5), Some("reader"));
    }

    #[test]
    fn status_bar_offline_indicator() {
        let sb = StatusBar {
            time: "14:32".into(),
            wifi_bars: 0,
            battery_percent: Some(82),
            connected: false,
        };
        assert!(sb.offline_mode());
        assert_eq!(sb.wifi_text(), "Wi-Fi ");
    }

    #[test]
    fn status_bar_bars() {
        let sb = StatusBar {
            time: "14:32".into(),
            wifi_bars: 4,
            battery_percent: Some(82),
            connected: true,
        };
        assert_eq!(sb.wifi_text(), "Wi-Fi ████");
        assert!(!sb.offline_mode());
    }

    #[test]
    fn keyboard_types_lowercase_and_uppercase() {
        let mut kb = VirtualKeyboard::default();
        assert_eq!(kb.key(0, 0), KeyAction::Char('q'));
        assert_eq!(kb.key(2, 1), KeyAction::Char('x'));
        kb.shifted = true;
        assert_eq!(kb.key(0, 0), KeyAction::Char('Q'));
        // one-shot shift consumed
        assert_eq!(kb.key(0, 1), KeyAction::Char('w'));
    }

    #[test]
    fn keyboard_special_keys() {
        let mut kb = VirtualKeyboard::default();
        assert_eq!(kb.key(3, 0), KeyAction::Shift);
        assert_eq!(kb.key(3, 1), KeyAction::Backspace);
        assert_eq!(kb.key(3, 2), KeyAction::Space);
        assert_eq!(kb.key(3, 3), KeyAction::Enter);
    }

    #[test]
    fn keyboard_layout_switching() {
        let mut kb = VirtualKeyboard::default();
        assert_eq!(kb.key(3, 4), KeyAction::SwitchLayout(Layout::Numeric));
        kb.layout = Layout::Numeric;
        assert_eq!(kb.key(0, 0), KeyAction::Char('1'));
        assert_eq!(kb.key(3, 4), KeyAction::SwitchLayout(Layout::Lower));
    }

    #[test]
    fn themes_differ() {
        assert_ne!(Theme::DARK.bg, Theme::LIGHT.bg);
    }

    // Smoke: a full tap gesture through GestureRecognizer hits a tile.
    #[test]
    fn gesture_pipeline_reaches_launcher() {
        let l = Launcher::new(vec![("vnc", "VNC")]);
        let hb = l.hitboxes()[0];
        let mut rec = GestureRecognizer::default();
        let down = TouchEvent {
            x: hb.x,
            y: hb.y,
            phase: TouchPhase::Start,
            fingers: 1,
        };
        let up = TouchEvent {
            x: hb.x,
            y: hb.y,
            phase: TouchPhase::End,
            fingers: 1,
        };
        rec.feed(&down, 0);
        let g = rec.feed(&up, 40);
        assert_eq!(g, Gesture::Tap);
        assert_eq!(l.tap_at(hb.x, hb.y), Some("vnc"));
    }
}
