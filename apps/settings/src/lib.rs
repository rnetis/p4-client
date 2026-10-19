//! Settings app (PRD §18): categories Network / Display / Input / Storage /
//! VNC / System, persisting through `Settings` (TOML) and mutating the Wi-Fi
//! manager configuration live.

use p4_core::{App, P4Result};
use p4_network::WifiConfig;
use p4_settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepTimeout {
    S30,
    Min1,
    Min5,
    Min10,
    Min30,
    Never,
}

impl SleepTimeout {
    pub fn seconds(self) -> u32 {
        match self {
            SleepTimeout::S30 => 30,
            SleepTimeout::Min1 => 60,
            SleepTimeout::Min5 => 300,
            SleepTimeout::Min10 => 600,
            SleepTimeout::Min30 => 1800,
            SleepTimeout::Never => 0,
        }
    }

    pub fn from_seconds(s: u32) -> Self {
        match s {
            30 => SleepTimeout::S30,
            60 => SleepTimeout::Min1,
            600 => SleepTimeout::Min10,
            1800 => SleepTimeout::Min30,
            0 => SleepTimeout::Never,
            _ => SleepTimeout::Min5,
        }
    }
}

pub struct SettingsApp {
    pub settings: Settings,
    /// Wi-Fi config mirror kept in sync on network changes.
    pub wifi_config: WifiConfig,
}

impl SettingsApp {
    pub fn new(settings: Settings) -> Self {
        let wifi_config = WifiConfig {
            dhcp: settings.network.dhcp,
            max_retries: 3,
            auto_reconnect: settings.vnc.auto_reconnect,
            static_ip: settings.network.static_ip.as_ref().and_then(|s| parse_ip(s)),
            dns: settings.network.dns.as_ref().and_then(|s| parse_ip(s)),
        };
        Self {
            settings,
            wifi_config,
        }
    }

    pub fn set_brightness(&mut self, pct: u8) {
        self.settings.display.brightness = pct.clamp(10, 100);
    }

    pub fn set_theme(&mut self, dark: bool) {
        let t = if dark {
            p4_settings::Theme::Dark
        } else {
            p4_settings::Theme::Light
        };
        self.settings.display.theme = t;
        self.settings.reader.theme = t;
    }

    pub fn set_sleep_timeout(&mut self, t: SleepTimeout) {
        self.settings.display.sleep_timeout = t.seconds();
    }

    pub fn sleep_timeout(&self) -> SleepTimeout {
        SleepTimeout::from_seconds(self.settings.display.sleep_timeout)
    }

    pub fn set_dhcp(&mut self, dhcp: bool) {
        self.settings.network.dhcp = dhcp;
        self.wifi_config.dhcp = dhcp;
    }

    pub fn set_font_size(&mut self, size: u8) {
        self.settings.reader.font_size = size.clamp(10, 32);
    }

    /// Serialize for persistence (PRD §19). `Ok(None)` means unchanged.
    pub fn to_toml(&self) -> P4Result<String> {
        self.settings.to_toml()
    }

    pub fn apply_toml(&mut self, s: &str) -> P4Result<()> {
        self.settings = Settings::from_toml(s)?;
        Ok(())
    }

    /// PRD §25 diagnostics snapshot.
    pub fn diagnostics(&self, free_heap: usize, psram: usize, rssi: i8, ip: Option<String>) -> String {
        format!(
            "Firmware: p4-client v{}\nDevice: {}\nFree heap: {free_heap} B\nPSRAM: {psram} B\nWi-Fi RSSI: {rssi} dBm\nIP: {}",
            env!("CARGO_PKG_VERSION"),
            self.settings.device_name,
            ip.unwrap_or_else(|| "none".into()),
        )
    }
}

fn parse_ip(s: &str) -> Option<[u8; 4]> {
    s.split('.')
        .map(|p| p.parse::<u8>().ok())
        .collect::<Option<Vec<_>>>()
        .and_then(|v| <[u8; 4]>::try_from(v).ok())
}

impl App for SettingsApp {
    fn id(&self) -> &'static str {
        "settings"
    }
    fn display_name(&self) -> &'static str {
        "Settings"
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
        Ok(())
    }
    fn update(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_clamped() {
        let mut s = SettingsApp::new(Settings::default_device());
        s.set_brightness(200);
        assert_eq!(s.settings.display.brightness, 100);
        s.set_brightness(0);
        assert_eq!(s.settings.display.brightness, 10);
    }

    #[test]
    fn sleep_timeout_roundtrip() {
        let mut s = SettingsApp::new(Settings::default_device());
        s.set_sleep_timeout(SleepTimeout::Never);
        assert_eq!(s.settings.display.sleep_timeout, 0);
        assert_eq!(s.sleep_timeout(), SleepTimeout::Never);
        s.set_sleep_timeout(SleepTimeout::Min10);
        assert_eq!(SleepTimeout::from_seconds(600), SleepTimeout::Min10);
    }

    #[test]
    fn theme_applies_to_display_and_reader() {
        let mut s = SettingsApp::new(Settings::default_device());
        s.set_theme(false);
        assert_eq!(s.settings.display.theme, p4_settings::Theme::Light);
        assert_eq!(s.settings.reader.theme, p4_settings::Theme::Light);
    }

    #[test]
    fn dhcp_syncs_to_wifi_config() {
        let mut s = SettingsApp::new(Settings::default_device());
        assert!(s.wifi_config.dhcp);
        s.set_dhcp(false);
        assert!(!s.wifi_config.dhcp);
        assert!(!s.settings.network.dhcp);
    }

    #[test]
    fn toml_roundtrip_through_app() {
        let mut s = SettingsApp::new(Settings::default_device());
        s.set_font_size(22);
        let t = s.to_toml().unwrap();
        let mut s2 = SettingsApp::new(Settings::default_device());
        s2.apply_toml(&t).unwrap();
        assert_eq!(s2.settings.reader.font_size, 22);
    }

    #[test]
    fn diagnostics_contains_fields() {
        let s = SettingsApp::new(Settings::default_device());
        let d = s.diagnostics(1024, 8 << 20, -58, Some("192.168.1.44".into()));
        assert!(d.contains("Free heap: 1024 B"));
        assert!(d.contains("192.168.1.44"));
        assert!(d.contains("-58 dBm"));
    }
}
