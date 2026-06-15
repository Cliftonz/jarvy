//! Observability & Debugging Module
//!
//! Provides comprehensive observability features for Jarvy including:
//! - Structured logging with multiple verbosity levels
//! - Performance profiling with phase tracking
//! - Network request tracing
//! - Sensitive data sanitization
//! - Diagnostic bundle export

pub mod bundle;
pub mod error;
pub mod logging;
pub mod network_trace;
pub mod profiler;
pub mod sanitizer;
pub mod telemetry_gate;

// Public API exports - some may not be used internally but are part of the module's interface
#[allow(unused_imports)]
// Bundle / Profiler / NetworkTracer have NO callers outside this module
// today (round-2 maint F15: ~1700 LOC of unused public surface). Demote
// to `pub(crate)` so we don't lock the API in at v0.1.0; promote back
// to `pub` when a caller actually wires them up (e.g. `jarvy diagnose
// --bundle` → DiagnosticBundle).
pub(crate) use bundle::{BundleScope, DiagnosticBundle, SystemInfo as BundleSystemInfo};
#[allow(unused_imports)]
pub use error::ObservabilityError;
#[allow(unused_imports)]
pub use logging::{LogConfig, LogFormat, LogLevel};
#[allow(unused_imports)]
pub(crate) use network_trace::{DomainStats, NetworkSummary, NetworkTiming, NetworkTracer};
#[allow(unused_imports)]
pub(crate) use profiler::{PhaseTiming, ProfileReport, Profiler};
pub use sanitizer::{Sanitizer, redact_for_display};

/// Global observability configuration
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Public API for observability configuration
pub struct ObservabilityConfig {
    /// Logging configuration
    pub log: LogConfig,
    /// Whether profiling is enabled
    pub profile: bool,
    /// Path to write profile output
    pub profile_output: Option<String>,
    /// Whether network tracing is enabled
    pub trace_network: bool,
    /// Path to write network trace
    pub network_log: Option<String>,
}

#[allow(dead_code)] // Public API for observability configuration
impl ObservabilityConfig {
    /// Create from CLI flags
    #[allow(clippy::too_many_arguments)]
    pub fn from_flags(
        quiet: bool,
        verbose: u8,
        log_format: Option<&str>,
        debug_filter: Option<&str>,
        log_file: Option<&str>,
        profile: bool,
        profile_output: Option<&str>,
        trace_network: bool,
        network_log: Option<&str>,
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
            profile,
            profile_output: profile_output.map(|s| s.to_string()),
            trace_network,
            network_log: network_log.map(|s| s.to_string()),
        }
    }
}
