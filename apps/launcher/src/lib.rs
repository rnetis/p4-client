//! Launcher app (PRD §8): home screen with app tiles, status bar, offline
//! indicator. Routes taps to the AppManager.

use p4_core::{App, AppManager, P4Result};
use p4_ui::Launcher;

pub struct LauncherApp {
    manager: *mut AppManager,
    launcher: Launcher,
    last_tap: Option<&'static str>,
}

// The launcher owns the AppManager for the lifetime of the firmware; the app
// registry is fixed after boot, so raw access is confined to this module.
unsafe impl Send for LauncherApp {}

impl LauncherApp {
    /// `apps`: (id, label) in display order.
    pub fn new(manager: &mut AppManager, apps: Vec<(&'static str, &'static str)>) -> Self {
        Self {
            manager,
            launcher: Launcher::new(apps),
            last_tap: None,
        }
    }

    pub fn launcher(&self) -> &Launcher {
        &self.launcher
    }

    /// Handle a tap at screen coordinates; opens the tapped app.
    pub fn tap(&mut self, x: u16, y: u16) -> bool {
        if let Some(id) = self.launcher.tap_at(x, y) {
            unsafe {
                let _ = (*self.manager).launch(id);
            }
            self.last_tap = Some(id);
            true
        } else {
            false
        }
    }
}

impl App for LauncherApp {
    fn id(&self) -> &'static str {
        "launcher"
    }
    fn display_name(&self) -> &'static str {
        "Home"
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
    use p4_core::AppManager;
    use p4_ui::MIN_TOUCH_TARGET;

    struct NoopApp(&'static str);
    impl App for NoopApp {
        fn id(&self) -> &'static str {
            self.0
        }
        fn display_name(&self) -> &'static str {
            self.0
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

    #[test]
    fn tap_opens_app() {
        let mut m = AppManager::new();
        m.register(Box::new(NoopApp("vnc")));
        m.register(Box::new(NoopApp("reader")));
        m.register(Box::new(NoopApp("files")));
        m.register(Box::new(NoopApp("settings")));
        let mut l = LauncherApp::new(
            &mut m,
            vec![("vnc", "VNC"), ("reader", "Books"), ("files", "Files"), ("settings", "Settings")],
        );
        let hb = l.launcher().hitboxes()[0];
        assert!(l.tap(hb.x + 2, hb.y + 2));
        assert_eq!(l.last_tap, Some("vnc"));
        assert!(!l.tap(1, 1));
    }

    #[test]
    fn tiles_valid_for_touch() {
        let mut m = AppManager::new();
        let l = LauncherApp::new(&mut m, vec![("vnc", "VNC")]);
        assert!(l.launcher().touch_targets_valid());
        assert!(l.launcher().hitboxes()[0].w >= MIN_TOUCH_TARGET);
    }
}
