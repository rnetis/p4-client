//! p4-core: shared application foundation for P4 Client.
//!
//! Contains the `App` trait/lifecycle (PRD §20), the AppManager, error types,
//! and the HAL traits that keep application code free of ESP-IDF details
//! (PRD Rule 1).

pub mod app;
pub mod error;
pub mod hal;
pub mod logging;

pub use app::{App, AppManager, AppState};
pub use error::P4Error;
pub type P4Result<T> = Result<T, P4Error>;
