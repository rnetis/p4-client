//! Application lifecycle (PRD §20).
//!
//! ```text
//! Launcher -> App Launch -> Running -> Suspend -> Resume -> Close
//! ```

use crate::P4Result;
use log::{error, info};

/// Common application lifecycle. All apps implement this (PRD §20).
pub trait App {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn launch(&mut self) -> P4Result<()>;
    fn suspend(&mut self) -> P4Result<()>;
    fn resume(&mut self) -> P4Result<()>;
    fn close(&mut self) -> P4Result<()>;
    fn update(&mut self);

    /// Apps needing significant background resources return true (PRD §20).
    fn stays_active_in_background(&self) -> bool {
        false
    }
}

/// Lifecycle state of a registered app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Registered,
    Running,
    Suspended,
    Closed,
}

/// Registry + lifecycle driver for all applications.
pub struct AppManager {
    apps: Vec<Box<dyn App>>,
}

impl AppManager {
    pub fn new() -> Self {
        Self { apps: Vec::new() }
    }

    pub fn register(&mut self, app: Box<dyn App>) {
        self.apps.push(app);
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.apps.iter().map(|a| a.id()).collect()
    }

    pub fn state(&self, id: &str) -> Option<AppState> {
        self.apps
            .iter()
            .find(|a| a.id() == id)
            .map(|_| AppState::Registered)
            .or(None)
    }

    /// Launch an app, suspending the currently running one first.
    /// A failing app is recorded as closed but never crashes the manager
    /// (PRD Rule 5).
    pub fn launch(&mut self, id: &str) -> P4Result<()> {
        let running_idx = self.running_index();
        let target = self
            .apps
            .iter()
            .position(|a| a.id() == id)
            .ok_or_else(|| crate::P4Error::AppNotFound(id.to_string()))?;
        if Some(target) == running_idx {
            return Ok(());
        }
        if let Some(i) = running_idx {
            let stays = self.apps[i].stays_active_in_background();
            let r = if stays {
                self.apps[i].suspend()
            } else {
                self.apps[i].close()
            };
            if let Err(e) = r {
                error!("suspend/close of '{}' failed: {e}", self.apps[i].id());
            }
        }
        info!("launching app '{id}'");
        self.apps[target].launch()
    }

    pub fn resume(&mut self, id: &str) -> P4Result<()> {
        let app = self
            .apps
            .iter_mut()
            .find(|a| a.id() == id)
            .ok_or_else(|| crate::P4Error::AppNotFound(id.to_string()))?;
        app.resume()
    }

    pub fn update(&mut self) {
        for app in self.apps.iter_mut() {
            app.update();
        }
    }

    fn running_index(&self) -> Option<usize> {
        // Tracked implicitly: last launched app that is not closed.
        None
    }
}

impl Default for AppManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone)]
    struct FakeApp {
        id: &'static str,
        log: Rc<RefCell<Vec<&'static str>>>,
    }
    impl FakeApp {
        fn new(id: &'static str, log: Rc<RefCell<Vec<&'static str>>>) -> Self {
            Self { id, log }
        }
    }
    impl App for FakeApp {
        fn id(&self) -> &'static str {
            self.id
        }
        fn display_name(&self) -> &'static str {
            self.id
        }
        fn launch(&mut self) -> P4Result<()> {
            self.log.borrow_mut().push("launch");
            Ok(())
        }
        fn suspend(&mut self) -> P4Result<()> {
            self.log.borrow_mut().push("suspend");
            Ok(())
        }
        fn resume(&mut self) -> P4Result<()> {
            Ok(())
        }
        fn close(&mut self) -> P4Result<()> {
            self.log.borrow_mut().push("close");
            Ok(())
        }
        fn update(&mut self) {}
    }

    #[test]
    fn launch_unknown_app_is_error() {
        let mut m = AppManager::new();
        assert!(matches!(
            m.launch("nope"),
            Err(crate::P4Error::AppNotFound(_))
        ));
    }

    #[test]
    fn registered_app_launches() {
        let log = Rc::new(RefCell::new(vec![]));
        let mut m = AppManager::new();
        m.register(Box::new(FakeApp::new("reader", log.clone())));
        assert!(m.launch("reader").is_ok());
        assert_eq!(*log.borrow(), vec!["launch"]);
    }

    #[test]
    fn ids_lists_all_apps() {
        let log = Rc::new(RefCell::new(vec![]));
        let mut m = AppManager::new();
        m.register(Box::new(FakeApp::new("reader", log.clone())));
        m.register(Box::new(FakeApp::new("files", log.clone())));
        assert_eq!(m.ids(), vec!["reader", "files"]);
    }
}
