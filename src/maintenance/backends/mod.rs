//! Package-manager backends for freshness lookups (PRD-057).
//!
//! Every backend is a thin shell-out wrapper around an existing
//! package-manager CLI. No new HTTP surface — if the manager isn't
//! on `PATH`, the backend returns [`BackendError::ManagerMissing`]
//! and the tool lands in the report's `unchecked` bucket.
//! One deliberate exception: [`uv`] probes the PyPI JSON API over
//! `net::bounded_fetch` because `uv tool upgrade --dry-run` output is
//! not a stable contract (PRD-060 decision).

pub mod apk;
pub mod apt;
pub mod brew;
pub mod cargo;
pub mod choco;
pub mod dnf;
pub mod gem;
pub mod go;
pub mod npm;
pub mod nuget;
pub mod pacman;
pub mod pip;
pub mod scoop;
pub mod uv;
pub mod winget;

use std::time::Duration;

/// Hard wall-clock cap per backend probe. Anything hung beyond this
/// is treated as a timeout and the tool is marked unchecked —
/// preserves setup-latency SLOs even when a network is
/// misbehaving. Matches the pattern from `commands::diagnose`.
pub const BACKEND_TIMEOUT: Duration = Duration::from_secs(5);

/// Trait every freshness backend implements. Kept object-safe so
/// the router can hold `Box<dyn FreshnessBackend>` per configured
/// tool.
pub trait FreshnessBackend {
    /// Stable telemetry label. Must match the `backend` field on
    /// `maintenance.*` events documented in CLAUDE.md.
    fn name(&self) -> &'static str;

    /// Look up the latest available version for `pkg_id` (the
    /// package-manager-native identifier, not the jarvy tool name).
    fn latest(&self, pkg_id: &str) -> Result<String, BackendError>;
}

/// Bounded backend errors. Every variant maps to an `error_kind`
/// label on the `maintenance.check_failed` telemetry event.
/// Adding a variant requires updating the taxonomy in CLAUDE.md
/// AND the [`BackendError::kind`] mapping below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// The package manager doesn't know about this package. Usually
    /// means the jarvy tool's `pkg_id` is wrong for this backend —
    /// worth surfacing but not a runtime failure.
    NotFound,
    /// Package manager binary isn't on `PATH`. Expected on machines
    /// that don't use that manager; the tool is bucketed as
    /// `unchecked`.
    ManagerMissing,
    /// Hit [`BACKEND_TIMEOUT`]. Child was killed.
    Timeout,
    /// Backend returned output that didn't match the expected shape.
    /// Shape drift on the package-manager side; needs investigation.
    ParseFailed,
    /// Filesystem or spawn permission was denied.
    PermissionDenied,
    /// Anything that doesn't fit above. Includes IO errors on the
    /// spawn path.
    Other,
}

impl BackendError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::ManagerMissing => "manager_missing",
            Self::Timeout => "timeout",
            Self::ParseFailed => "parse_failed",
            Self::PermissionDenied => "permission_denied",
            Self::Other => "other",
        }
    }
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotFound => "package not found",
            Self::ManagerMissing => "package manager not installed",
            Self::Timeout => "backend timed out",
            Self::ParseFailed => "backend output could not be parsed",
            Self::PermissionDenied => "permission denied",
            Self::Other => "backend error",
        })
    }
}

impl std::error::Error for BackendError {}

/// Map a [`ProbeResult`] onto a [`BackendError`] for the "no output"
/// error paths. Callers still parse `Completed` themselves —
/// nothing here presumes the shape of stdout.
pub(crate) fn probe_error(
    probe: crate::tools::common::ProbeResult,
) -> Result<std::process::Output, BackendError> {
    use crate::tools::common::ProbeResult;
    match probe {
        ProbeResult::Completed(out) => Ok(out),
        ProbeResult::Timeout => Err(BackendError::Timeout),
        ProbeResult::Missing => Err(BackendError::ManagerMissing),
        ProbeResult::PermissionDenied => Err(BackendError::PermissionDenied),
        ProbeResult::IoError(_) => Err(BackendError::Other),
    }
}
