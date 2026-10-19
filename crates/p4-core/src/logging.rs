//! Logging facade setup (PRD §24). Levels: ERROR/WARN/INFO/DEBUG/TRACE,
//! default INFO on device; host tests may lower it.

pub fn init(level: log::LevelFilter) {
    // On ESP-IDF the logger target is the serial console; on host, stdout.
    // env_logger / esp-println wiring happens in the binary crates; this
    // documents the default policy.
    let _ = level;
}

pub const DEFAULT_LEVEL: log::LevelFilter = log::LevelFilter::Info;
