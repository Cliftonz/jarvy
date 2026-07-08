//! Public log-config types consumed by `jarvy logs config` and the
//! `LogConfig` re-export in `crate::logging`. The previous `init_*`
//! functions were dead code (analytics.rs is the canonical subscriber
//! init) — they shipped a competing `set_global_default` that would
//! have panicked at runtime if anyone flipped them on. Removed
//! (round-2 obs / maint).

#![allow(dead_code)] // Public API consumed via re-export

use std::path::PathBuf;

/// Log verbosity level
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogLevel {
    /// Errors only (--quiet)
    Quiet,
    /// Info and above (default)
    #[default]
    Normal,
    /// Warnings included (--verbose / -v)
    Verbose,
    /// Full debug logs (--debug / -vv)
    Debug,
    /// Trace-level detail (--trace / -vvv)
    Trace,
}

/// Log output format
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable text (default)
    #[default]
    Text,
    /// Machine-parseable JSON
    Json,
}

/// Logging configuration. Currently consumed by `jarvy logs config`
/// for display; subscriber wiring lives in `crate::analytics`.
#[derive(Debug, Clone, Default)]
pub struct LogConfig {
    /// Verbosity level
    pub level: LogLevel,
    /// Output format
    pub format: LogFormat,
    /// Module filter (e.g., "jarvy::tools::docker")
    pub filter: Option<String>,
    /// File to write logs to (in addition to default ~/.jarvy/logs/)
    pub file: Option<String>,
    /// Disable file logging (for tests)
    pub disable_file_logging: bool,
}

/// Get the default log directory path (~/.jarvy/logs/) via the canonical
/// resolver so a `JARVY_HOME` override is honored.
pub fn default_log_directory() -> PathBuf {
    crate::paths::logs_dir().unwrap_or_else(|_| PathBuf::from(".jarvy/logs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert_eq!(config.level, LogLevel::Normal);
        assert_eq!(config.format, LogFormat::Text);
        assert!(config.filter.is_none());
        assert!(config.file.is_none());
        assert!(!config.disable_file_logging);
    }

    /// Serialized with the `jarvy_home_env` group — same rationale as
    /// the `src/logging/mod.rs` mirror test. Concurrent tests pinning
    /// JARVY_HOME to a tempdir otherwise race with the `.jarvy/logs`
    /// suffix assertion.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn test_default_log_directory() {
        let dir = default_log_directory();
        assert!(dir.ends_with(".jarvy/logs"));
    }
}
