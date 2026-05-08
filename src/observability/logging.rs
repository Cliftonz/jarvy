//! Structured Logging Configuration
//!
//! Provides unified logging with multiple outputs:
//! - Console output (for interactive use)
//! - File output to ~/.jarvy/logs/ (always enabled for debugging)
//! - Optional OTLP export (when telemetry is configured)
//!
//! ## Log Levels
//!
//! - `Quiet`: Errors only
//! - `Normal`: Info and above (default)
//! - `Verbose`: Includes warnings
//! - `Debug`: Full debug logs
//! - `Trace`: Trace-level detail
//!
//! ## Usage
//!
//! ```bash
//! jarvy setup --debug              # Debug logging
//! jarvy setup --trace              # Trace logging
//! jarvy setup --debug --log-format json   # JSON output
//! ```

#![allow(dead_code)] // Public API for logging configuration

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

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

impl LogLevel {
    /// Convert to tracing EnvFilter string
    pub fn as_filter_string(self) -> &'static str {
        match self {
            LogLevel::Quiet => "error",
            LogLevel::Normal => "info",
            LogLevel::Verbose => "warn,jarvy=info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
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

/// Logging configuration
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

/// Get the default log directory path (~/.jarvy/logs/)
fn default_log_directory() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".jarvy")
        .join("logs")
}

/// Ensure log directory exists
fn ensure_log_directory() -> std::io::Result<PathBuf> {
    let log_dir = default_log_directory();
    fs::create_dir_all(&log_dir)?;
    Ok(log_dir)
}

/// File writer that implements `Write`. Wraps the underlying file in a
/// `BufWriter` so per-event tracing emits are coalesced into 8 KB writes
/// instead of one `write(2)` syscall per log line. The `Mutex` serializes
/// concurrent writers (rayon parallel install path).
struct FileWriter {
    file: Mutex<std::io::BufWriter<std::fs::File>>,
}

impl FileWriter {
    fn new(path: &str) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: Mutex::new(std::io::BufWriter::with_capacity(8 * 1024, file)),
        })
    }
}

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut file = self.file.lock().unwrap();
        file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut file = self.file.lock().unwrap();
        file.flush()
    }
}

impl Write for &FileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut file = self.file.lock().unwrap();
        file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut file = self.file.lock().unwrap();
        file.flush()
    }
}

/// Initialize unified logging with console and file output
///
/// This sets up:
/// 1. Console output (stdout for non-errors, stderr for errors)
/// 2. File output to ~/.jarvy/logs/jarvy.YYYY-MM-DD.log (daily rotation)
///
/// Returns Ok(true) if debug logging was enabled, Ok(false) if using default logging.
pub fn init_debug_logging(config: &LogConfig) -> Result<bool, super::error::ObservabilityError> {
    // Build filter string
    let filter_str = if let Some(ref module_filter) = config.filter {
        format!("{},{}", config.level.as_filter_string(), module_filter)
    } else {
        config.level.as_filter_string().to_string()
    };

    let env_filter = EnvFilter::try_new(&filter_str)
        .unwrap_or_else(|_| EnvFilter::new(config.level.as_filter_string()));

    // Create file layer if not disabled
    let file_layer = if !config.disable_file_logging {
        match ensure_log_directory() {
            Ok(log_dir) => {
                // Use daily rotation
                let file_appender = RollingFileAppender::new(Rotation::DAILY, log_dir, "jarvy.log");

                // File layer always uses JSON for machine-readability
                let file_layer = fmt::layer()
                    .json()
                    .with_writer(file_appender)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_current_span(true)
                    .with_target(true)
                    .with_ansi(false)
                    .with_filter(env_filter.clone());

                Some(file_layer)
            }
            Err(e) => {
                eprintln!("Warning: Could not create log directory: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Create console layer based on format
    let console_layer = match config.format {
        LogFormat::Json => fmt::layer()
            .json()
            .with_span_events(FmtSpan::CLOSE)
            .with_current_span(true)
            .with_target(true)
            .with_filter(env_filter)
            .boxed(),
        LogFormat::Text => fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_file(config.level == LogLevel::Debug || config.level == LogLevel::Trace)
            .with_line_number(config.level == LogLevel::Debug || config.level == LogLevel::Trace)
            .with_filter(env_filter)
            .boxed(),
    };

    // Build and set the subscriber
    let subscriber = tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer);

    subscriber.init();

    // Return whether we enabled non-default logging
    let is_non_default = config.level != LogLevel::Normal
        || config.format != LogFormat::Text
        || config.filter.is_some()
        || config.file.is_some();

    Ok(is_non_default)
}

/// Initialize minimal logging (for when full logging isn't needed)
///
/// This is used when debug flags aren't present. Sets up basic info-level logging
/// with optional file output.
pub fn init_minimal_logging() -> Result<(), super::error::ObservabilityError> {
    let config = LogConfig {
        level: LogLevel::Normal,
        format: LogFormat::Text,
        filter: None,
        file: None,
        disable_file_logging: false,
    };

    init_debug_logging(&config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_to_filter() {
        assert_eq!(LogLevel::Quiet.as_filter_string(), "error");
        assert_eq!(LogLevel::Normal.as_filter_string(), "info");
        assert_eq!(LogLevel::Debug.as_filter_string(), "debug");
        assert_eq!(LogLevel::Trace.as_filter_string(), "trace");
    }

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert_eq!(config.level, LogLevel::Normal);
        assert_eq!(config.format, LogFormat::Text);
        assert!(config.filter.is_none());
        assert!(config.file.is_none());
        assert!(!config.disable_file_logging);
    }

    #[test]
    fn test_default_log_directory() {
        let dir = default_log_directory();
        assert!(dir.ends_with(".jarvy/logs"));
    }
}
