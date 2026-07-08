//! Performance Profiler
//!
//! Records phase-level timing for `jarvy setup --profile`.
//!
//! Scope note: this profiler is **phase-only**. Per-tool and per-network
//! timing were removed with the `network_trace` deletion (observability
//! cleanup) — the default install path is parallel (PRD-001), so the
//! sequential `start_tool`/`end_tool` model was racy, and the per-tool
//! `duration_ms` telemetry events already answer "which tool was slow?".
//! Resurrect a parallel-safe per-tool model from git history if a real
//! need appears.
//!
//! ## Usage (crate-internal)
//!
//! ```ignore
//! let mut profiler = Profiler::new();
//! profiler.start_phase("config_parsing");
//! // ... do work ...
//! profiler.start_phase("install");  // auto-ends the previous phase
//! // ... do work ...
//! let report = profiler.report();
//! eprint!("{}", report.to_summary());
//! ```

use serde::Serialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Serialize a `Duration` as integer milliseconds under a `_ms`-suffixed
/// key, matching the repo-wide `duration_ms` telemetry contract (every
/// `*.phase_completed` / `*.completed` event carries integer ms). serde's
/// default `Duration` encoding is `{secs, nanos}`, which would force any
/// consumer joining `--profile-output` JSON against log-derived
/// dashboards to special-case the units (observability review F7).
fn ser_duration_ms<S: serde::Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_u64(d.as_millis() as u64)
}

/// Phase timing information.
#[derive(Debug, Clone, Serialize)]
pub struct PhaseTiming {
    /// Duration of the phase, in milliseconds.
    #[serde(rename = "duration_ms", serialize_with = "ser_duration_ms")]
    pub duration: Duration,
    /// Start time (not serialized).
    #[serde(skip)]
    pub start: Option<Instant>,
}

impl Default for PhaseTiming {
    fn default() -> Self {
        Self {
            duration: Duration::ZERO,
            start: None,
        }
    }
}

/// Phase-level performance profiler.
#[derive(Debug)]
pub struct Profiler {
    /// Overall start time.
    start: Instant,
    /// Phase timings by name.
    phases: HashMap<String, PhaseTiming>,
    /// Current active phase.
    current_phase: Option<String>,
    /// Whether profiling is enabled.
    enabled: bool,
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler {
    /// Create a new profiler.
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            phases: HashMap::new(),
            current_phase: None,
            enabled: true,
        }
    }

    /// Create a disabled profiler (all methods no-op).
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::new()
        }
    }

    /// Check if profiling is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Start timing a phase. Auto-ends the current phase, so callers can
    /// sprinkle `start_phase` at each boundary without pairing an
    /// `end_phase`.
    pub fn start_phase(&mut self, name: &str) {
        if !self.enabled {
            return;
        }
        self.end_phase();
        self.current_phase = Some(name.to_string());
        self.phases.insert(
            name.to_string(),
            PhaseTiming {
                duration: Duration::ZERO,
                start: Some(Instant::now()),
            },
        );
    }

    /// End the current phase.
    pub fn end_phase(&mut self) {
        if !self.enabled {
            return;
        }
        if let Some(name) = self.current_phase.take() {
            if let Some(phase) = self.phases.get_mut(&name) {
                if let Some(start) = phase.start.take() {
                    phase.duration = start.elapsed();
                }
            }
        }
    }

    /// Generate a profile report.
    pub fn report(&self) -> ProfileReport {
        ProfileReport {
            total_duration: self.start.elapsed(),
            phases: self.phases.clone(),
        }
    }
}

/// Profile report with phase-level timing.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileReport {
    /// Total duration of profiled operations, in milliseconds.
    #[serde(rename = "total_duration_ms", serialize_with = "ser_duration_ms")]
    pub total_duration: Duration,
    /// Phase breakdown.
    pub phases: HashMap<String, PhaseTiming>,
}

impl ProfileReport {
    /// Generate a human-readable summary.
    pub fn to_summary(&self) -> String {
        let mut output = String::new();

        output.push_str("══════════════════════════════════════════════════════════\n");
        output.push_str("Performance Profile\n");
        output.push_str("══════════════════════════════════════════════════════════\n\n");

        output.push_str(&format!(
            "Total duration: {:.2}s\n\n",
            self.total_duration.as_secs_f64()
        ));

        if !self.phases.is_empty() {
            output.push_str("Phase breakdown:\n");
            let mut phases: Vec<_> = self.phases.iter().collect();
            phases.sort_by_key(|p| std::cmp::Reverse(p.1.duration));

            for (name, timing) in phases {
                let percentage = if self.total_duration.as_secs_f64() > 0.0 {
                    (timing.duration.as_secs_f64() / self.total_duration.as_secs_f64()) * 100.0
                } else {
                    0.0
                };
                output.push_str(&format!(
                    "  {:20} {:>6.2}s  ({:>5.1}%)\n",
                    name,
                    timing.duration.as_secs_f64(),
                    percentage
                ));
            }
            output.push('\n');
        }

        output
    }

    /// Export as pretty JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Export as JSON to a file.
    pub fn to_json_file(&self, path: &str) -> Result<(), super::error::ObservabilityError> {
        let json = self.to_json()?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_profiler_phases() {
        let mut profiler = Profiler::new();
        profiler.start_phase("test_phase");
        sleep(Duration::from_millis(10));
        profiler.end_phase();

        let report = profiler.report();
        assert!(report.phases.contains_key("test_phase"));
        assert!(report.phases["test_phase"].duration.as_millis() >= 10);
    }

    #[test]
    fn test_start_phase_auto_ends_previous() {
        let mut profiler = Profiler::new();
        profiler.start_phase("a");
        sleep(Duration::from_millis(5));
        profiler.start_phase("b"); // should close "a"
        let report = profiler.report();
        assert!(report.phases["a"].duration.as_millis() >= 5);
        assert!(report.phases.contains_key("b"));
    }

    #[test]
    fn test_profiler_disabled() {
        let mut profiler = Profiler::disabled();
        assert!(!profiler.is_enabled());
        profiler.start_phase("test");
        profiler.end_phase();
        let report = profiler.report();
        assert!(report.phases.is_empty());
    }

    #[test]
    fn test_profile_report_summary() {
        let mut profiler = Profiler::new();
        profiler.start_phase("config");
        profiler.end_phase();

        let summary = profiler.report().to_summary();
        assert!(summary.contains("Performance Profile"));
        assert!(summary.contains("Total duration"));
        assert!(summary.contains("config"));
    }

    #[test]
    fn test_profile_report_json_uses_ms() {
        let report = Profiler::new().report();
        let json = report.to_json().unwrap();
        assert!(json.contains("total_duration_ms"));
        assert!(json.contains("phases"));
        // No {secs,nanos} Duration encoding leaks into the output.
        assert!(!json.contains("nanos"));
    }
}
