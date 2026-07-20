//! Observability & Debugging Module
//!
//! Provides observability features for Jarvy including:
//! - Structured logging with multiple verbosity levels (wired into the
//!   canonical subscriber init in `crate::analytics` via the
//!   `jarvy setup -q/-v/--log-format/--log-file/--debug-filter` flags)
//! - Performance profiling with phase tracking (`jarvy setup --profile`)
//! - Sensitive data sanitization
//!
//! The former `network_trace` and `bundle` modules were removed after
//! shipping unwired for two release cycles (round-2 maint F15 →
//! observability cleanup): `DiagnosticBundle` duplicated the wired
//! `src/ticket/` ZIP bundler, and `NetworkTracer` had no CLI surface or
//! callers. Resurrect from git history if a support case ever demands
//! domain-level fetch stats.

pub mod error;
pub mod logging;
pub mod profiler;
pub mod sanitizer;
pub mod telemetry_gate;

#[allow(unused_imports)]
pub use error::ObservabilityError;
#[allow(unused_imports)]
pub use logging::{LogConfig, LogFormat, LogLevel};
#[allow(unused_imports)]
pub(crate) use profiler::{PhaseTiming, ProfileReport, Profiler};
pub use sanitizer::{Sanitizer, redact_for_display};

/// Log-shaping configuration, built from the `jarvy setup`
/// observability flags in `main.rs` and consumed by
/// `analytics::init_logging` (log level / format / file / filter).
/// Profiling (`--profile` / `--profile-output`) flows through
/// `dispatch.rs` → `run_setup` directly and is not carried here.
#[derive(Debug, Clone, Default)]
pub struct ObservabilityConfig {
    /// Logging configuration
    pub log: LogConfig,
}

impl ObservabilityConfig {
    /// Create from CLI flags
    pub fn from_flags(
        quiet: bool,
        verbose: u8,
        log_format: Option<&str>,
        debug_filter: Option<&str>,
        log_file: Option<&str>,
    ) -> Self {
        let level = if quiet {
            LogLevel::Quiet
        } else {
            match verbose {
                0 => LogLevel::Normal,
                1 => LogLevel::Verbose,
                2 => LogLevel::Debug,
                _ => LogLevel::Trace,
            }
        };

        let format = match log_format {
            Some("json") => LogFormat::Json,
            _ => LogFormat::Text,
        };

        Self {
            log: LogConfig {
                level,
                format,
                filter: debug_filter.map(|s| s.to_string()),
                file: log_file.map(|s| s.to_string()),
                disable_file_logging: false,
            },
        }
    }

    /// Console default for startup one-shots — see [`LogLevel::WarnOnly`]
    /// for the full rationale. `-v` on the same command restores INFO.
    pub fn startup_quiet() -> Self {
        Self {
            log: LogConfig {
                level: LogLevel::WarnOnly,
                ..LogConfig::default()
            },
        }
    }

    /// True when the CLI expresses *filtering* intent that should widen
    /// the registry `EnvFilter` and take precedence over `RUST_LOG`.
    ///
    /// Only `-v/-vv/-vvv` (level more verbose than `Normal`) and
    /// `--debug-filter <module>` qualify. `--quiet` is a console-only
    /// cap (it must not lower the registry floor — see
    /// `analytics::cli_log_directives`), and `--log-format` / `--log-file`
    /// shape the sink, not the filter. Including those sink flags here
    /// previously let `--log-file out.log` alone silently discard a
    /// user's `RUST_LOG` (perf/QA/observability/maintainability review).
    pub fn has_filter_overrides(&self) -> bool {
        matches!(
            self.log.level,
            LogLevel::Verbose | LogLevel::Debug | LogLevel::Trace
        ) || self.log.filter.is_some()
    }
}
