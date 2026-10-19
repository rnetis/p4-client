//! Wi-Fi network service (PRD §17).
//!
//! State machine:
//! DISCONNECTED -> SCANNING -> CONNECTING -> CONNECTED -> ONLINE
//! CONNECTING -> FAILED -> RETRY (back to CONNECTING)

use log::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiState {
    Disconnected,
    Scanning,
    Connecting,
    Connected,
    Online,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetEvent {
    ScanRequested,
    ScanComplete(Vec<AccessPoint>),
    ConnectRequested { ssid: String },
    AssociationOk,
    DhcpAcquired { ip: [u8; 4] },
    ConnectionFailed { reason: String },
    RetryRequested,
    DisconnectRequested,
    ForgotNetwork { ssid: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPoint {
    pub ssid: String,
    pub rssi: i8,
    pub requires_password: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiConfig {
    pub dhcp: bool,
    pub static_ip: Option<[u8; 4]>,
    pub dns: Option<[u8; 4]>,
    pub max_retries: u8,
    /// Auto reconnect on drop (PRD §2 secondary goal).
    pub auto_reconnect: bool,
}

impl Default for WifiConfig {
    fn default() -> Self {
        Self {
            dhcp: true,
            static_ip: None,
            dns: None,
            max_retries: 3,
            auto_reconnect: true,
        }
    }
}

/// Pure state machine; the driver layer (ESP-Hosted on the C6) performs the
/// actual radio work in response to [`WifiManager::step`] outputs.
#[derive(Debug)]
pub struct WifiManager {
    pub state: WifiState,
    pub config: WifiConfig,
    pub ssid: Option<String>,
    pub ip: Option<[u8; 4]>,
    pub known_networks: Vec<String>,
    retries: u8,
    reconnect_after_drop: bool,
}

impl WifiManager {
    pub fn new(config: WifiConfig) -> Self {
        Self {
            state: WifiState::Disconnected,
            config,
            ssid: None,
            ip: None,
            known_networks: Vec::new(),
            retries: 0,
            reconnect_after_drop: false,
        }
    }

    /// Returns true when the state changed.
    pub fn step(&mut self, ev: NetEvent) -> bool {
        use NetEvent::*;
        let prev = self.state;
        match ev {
            ScanRequested => self.state = WifiState::Scanning,
            ScanComplete(aps) => {
                if self.state == WifiState::Scanning {
                    self.state = WifiState::Disconnected;
                    let _ = aps;
                }
            }
            ConnectRequested { ssid } => {
                self.ssid = Some(ssid.clone());
                self.retries = 0;
                self.state = WifiState::Connecting;
            }
            AssociationOk => {
                if self.state == WifiState::Connecting {
                    self.state = WifiState::Connected;
                }
            }
            DhcpAcquired { ip } => {
                if self.state == WifiState::Connected {
                    self.ip = Some(ip);
                    self.state = WifiState::Online;
                    self.reconnect_after_drop = false;
                }
            }
            ConnectionFailed { reason } => {
                info!("wifi connect failed: {reason}");
                if self.state == WifiState::Connecting {
                    self.retries += 1;
                    if self.retries < self.config.max_retries {
                        self.state = WifiState::Failed;
                    } else {
                        self.state = WifiState::Disconnected;
                        self.retries = 0;
                    }
                }
            }
            RetryRequested => {
                if self.state == WifiState::Failed {
                    self.state = WifiState::Connecting;
                }
            }
            DisconnectRequested => {
                self.state = WifiState::Disconnected;
                self.ip = None;
            }
            ForgotNetwork { ssid } => {
                self.known_networks.retain(|n| *n != ssid);
            }
        }
        let changed = prev != self.state;
        if changed {
            info!("wifi: {:?} -> {:?}", prev, self.state);
        }
        changed
    }

    /// Called by the driver when the radio drops an established link.
    pub fn link_dropped(&mut self) {
        if self.state == WifiState::Online || self.state == WifiState::Connected {
            self.ip = None;
            if self.config.auto_reconnect {
                self.reconnect_after_drop = true;
                self.state = WifiState::Connecting;
            } else {
                self.state = WifiState::Disconnected;
            }
        }
    }

    /// True while a reconnect attempt triggered by `link_dropped` is pending.
    pub fn pending_reconnect(&self) -> bool {
        self.reconnect_after_drop
    }

    pub fn retries(&self) -> u8 {
        self.retries
    }
}

/// Signal strength to status-bar bars (PRD §8: Wi-Fi ████).
pub fn signal_bars(rssi: i8) -> u8 {
    match rssi {
        r if r >= -55 => 4,
        r if r >= -65 => 3,
        r if r >= -75 => 2,
        r if r >= -85 => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> WifiManager {
        WifiManager::new(WifiConfig::default())
    }

    #[test]
    fn happy_path_to_online() {
        let mut m = manager();
        assert!(m.step(NetEvent::ScanRequested));
        assert_eq!(m.state, WifiState::Scanning);
        m.step(NetEvent::ScanComplete(vec![AccessPoint {
            ssid: "home".into(),
            rssi: -55,
            requires_password: true,
        }]));
        assert_eq!(m.state, WifiState::Disconnected);
        m.step(NetEvent::ConnectRequested { ssid: "home".into() });
        assert_eq!(m.state, WifiState::Connecting);
        m.step(NetEvent::AssociationOk);
        assert_eq!(m.state, WifiState::Connected);
        m.step(NetEvent::DhcpAcquired { ip: [192, 168, 1, 44] });
        assert_eq!(m.state, WifiState::Online);
        assert_eq!(m.ip, Some([192, 168, 1, 44]));
    }

    #[test]
    fn failure_retries_then_gives_up() {
        let mut m = manager();
        m.step(NetEvent::ConnectRequested { ssid: "x".into() });
        let fail = || NetEvent::ConnectionFailed { reason: "auth".into() };
        m.step(fail());
        assert_eq!(m.state, WifiState::Failed);
        assert_eq!(m.retries(), 1);
        m.step(NetEvent::RetryRequested);
        m.step(fail());
        m.step(NetEvent::RetryRequested);
        m.step(fail()); // retries == max (3) -> disconnected
        assert_eq!(m.state, WifiState::Disconnected);
        assert_eq!(m.retries(), 0);
    }

    #[test]
    fn auto_reconnect_on_drop() {
        let mut m = manager();
        m.step(NetEvent::ConnectRequested { ssid: "home".into() });
        m.step(NetEvent::AssociationOk);
        m.step(NetEvent::DhcpAcquired { ip: [10, 0, 0, 2] });
        m.link_dropped();
        assert_eq!(m.state, WifiState::Connecting);
        assert!(m.pending_reconnect());
    }

    #[test]
    fn no_reconnect_when_disabled() {
        let mut m = WifiManager::new(WifiConfig { auto_reconnect: false, ..Default::default() });
        m.step(NetEvent::ConnectRequested { ssid: "home".into() });
        m.step(NetEvent::AssociationOk);
        m.step(NetEvent::DhcpAcquired { ip: [10, 0, 0, 2] });
        m.link_dropped();
        assert_eq!(m.state, WifiState::Disconnected);
        assert!(!m.pending_reconnect());
    }

    #[test]
    fn forget_removes_known_network() {
        let mut m = manager();
        m.known_networks.push("old".into());
        m.step(NetEvent::ForgotNetwork { ssid: "old".into() });
        assert!(m.known_networks.is_empty());
    }

    #[test]
    fn bars() {
        assert_eq!(signal_bars(-45), 4);
        assert_eq!(signal_bars(-75), 2);
        assert_eq!(signal_bars(-90), 0);
    }
}
