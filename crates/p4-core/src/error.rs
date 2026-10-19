use thiserror::Error;

/// Errors shared across the P4 Client runtime.
///
/// Rule 5 (PRD §33): a failed application must not crash the firmware, so all
/// recoverable failures are modeled as values, never panics.
#[derive(Debug, Error)]
pub enum P4Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config error: {0}")]
    Config(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("app '{0}' not found")]
    AppNotFound(String),
    #[error("security violation: {0}")]
    Security(String),
}
