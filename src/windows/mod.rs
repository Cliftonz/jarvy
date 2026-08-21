//! Windows-specific setup phase.
//!
//! Fixes the "why does my `.sh` script open in Notepad" paper cut via
//! two independent knobs teams can pick:
//!
//! - **`sh_pathext`** — appends `.SH` to the user's PATHEXT so
//!   `myscript.sh` runs from cmd / PowerShell.
//! - **`sh_association`** — routes the `.sh` file `open` verb to
//!   `bash.exe` so Explorer double-click / `start script.sh` runs it.
//!
//! Both writes are HKCU-scoped (no admin needed) and idempotent.
//! Non-Windows targets short-circuit with a `windows.phase_skipped`
//! event so a cross-platform team can commit one jarvy.toml.

pub mod config;
pub mod pathext;
pub mod sh_association;

#[allow(unused_imports)]
// WSL helpers are consumed by cross-platform integration tests + Windows exec
pub use config::{
    PathTranslateError, ShAssociationMode, WindowsConfig, WslConfig, translate_win_path,
    validate_bash_path, validate_channel, validate_distro_name, validate_distro_slug,
    validate_install_location,
};

/// Reason the Windows phase was skipped without applying anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Config block absent or `enabled = false`.
    Disabled,
    /// Running on a non-Windows target — the block parses but is a
    /// no-op.
    NotWindows,
    /// Both knobs are set to their no-op values (`sh_pathext = false`
    /// AND `sh_association = "off"`). Emits a distinct event so
    /// operators can distinguish "user opted into the block but not
    /// any knob" from "user hasn't discovered the block yet".
    NothingToDo,
    /// Remote config attempted without `allow_remote = true`.
    RemoteRefused,
}

impl SkipReason {
    pub fn as_label(self) -> &'static str {
        match self {
            SkipReason::Disabled => "disabled",
            SkipReason::NotWindows => "not_windows",
            SkipReason::NothingToDo => "nothing_to_do",
            SkipReason::RemoteRefused => "remote_refused",
        }
    }
}

/// Errors that can bubble up from the Windows phase. Advisory only —
/// [`run_windows_phase`] never fails `jarvy setup` on Windows errors,
/// mirroring the `[dotfiles]` phase convention.
#[derive(Debug, thiserror::Error)]
pub enum WindowsPhaseError {
    #[error(transparent)]
    Association(#[from] sh_association::AssociationError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid bash_path_override: {0}")]
    InvalidBashPath(String),
}

/// Decide whether the Windows phase should run and, if so, apply it.
///
/// Return value distinguishes "applied" from "skipped-with-reason" so
/// the setup summary can print the right line. Never panics; errors
/// route through the returned `Result` for the caller to log without
/// failing setup.
pub fn run_windows_phase(
    config: Option<&WindowsConfig>,
) -> Result<WindowsPhaseOutcome, WindowsPhaseError> {
    let Some(cfg) = config else {
        emit_skipped(SkipReason::Disabled);
        return Ok(WindowsPhaseOutcome::Skipped(SkipReason::Disabled));
    };
    if !cfg.enabled {
        emit_skipped(SkipReason::Disabled);
        return Ok(WindowsPhaseOutcome::Skipped(SkipReason::Disabled));
    }
    if cfg.origin == crate::ai_hooks::ConfigOrigin::Remote && !cfg.allow_remote {
        emit_remote_refused();
        return Ok(WindowsPhaseOutcome::Skipped(SkipReason::RemoteRefused));
    }
    let both_off = !cfg.sh_pathext && matches!(cfg.sh_association, ShAssociationMode::Off);
    if both_off {
        emit_skipped(SkipReason::NothingToDo);
        return Ok(WindowsPhaseOutcome::Skipped(SkipReason::NothingToDo));
    }
    // Validate bash_path_override at phase-run time — config-load
    // may not run this check for every code path, and it's cheap.
    if let Some(ref override_path) = cfg.bash_path_override
        && let Err(msg) = validate_bash_path(override_path)
    {
        return Err(WindowsPhaseError::InvalidBashPath(msg.to_string()));
    }

    #[cfg(target_os = "windows")]
    {
        use std::path::Path;
        emit_phase_started(cfg);
        let started_at = std::time::Instant::now();
        let pathext_outcome = if cfg.sh_pathext {
            match pathext::apply(".sh") {
                Ok(pathext::PathExtOutcome::Applied) => {
                    emit_pathext_applied();
                    Some("applied")
                }
                Ok(pathext::PathExtOutcome::Unchanged) => {
                    emit_pathext_unchanged();
                    Some("unchanged")
                }
                Err(err) => {
                    emit_pathext_failed(&err);
                    Some("failed")
                }
            }
        } else {
            None
        };
        let assoc_outcome = if !matches!(cfg.sh_association, ShAssociationMode::Off) {
            let plan = sh_association::plan_association(
                cfg.sh_association,
                cfg.bash_path_override.as_deref(),
                &|p| Path::new(p).exists(),
            )?;
            match plan {
                Some(launcher) => match sh_association::apply(&launcher) {
                    Ok(sh_association::AssociationOutcome::Applied) => {
                        emit_association_applied(&launcher);
                        Some("applied")
                    }
                    Ok(sh_association::AssociationOutcome::Unchanged) => {
                        emit_association_unchanged(&launcher);
                        Some("unchanged")
                    }
                    Err(err) => {
                        emit_association_failed(&err);
                        Some("failed")
                    }
                },
                None => None,
            }
        } else {
            None
        };
        emit_phase_completed(pathext_outcome, assoc_outcome, started_at.elapsed());
        return Ok(WindowsPhaseOutcome::Applied {
            pathext: pathext_outcome.map(str::to_string),
            association: assoc_outcome.map(str::to_string),
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        // The block was declared and enabled but we're not on Windows.
        // Advisory-only — cross-platform teams commit one jarvy.toml.
        emit_skipped(SkipReason::NotWindows);
        Ok(WindowsPhaseOutcome::Skipped(SkipReason::NotWindows))
    }
}

/// Outcome of [`run_windows_phase`]. `Applied` carries the per-knob
/// outcome labels so the setup summary can render them accurately.
///
/// `Applied` is only constructible from the `cfg(target_os = "windows")`
/// arm; the outer `#[allow(dead_code)]` on the variant fields prevents
/// the non-Windows dead-code warning without hiding the type from the
/// consumers that do exist (Windows exec + tests).
#[derive(Debug)]
#[allow(dead_code)]
pub enum WindowsPhaseOutcome {
    Applied {
        pathext: Option<String>,
        association: Option<String>,
    },
    Skipped(SkipReason),
}

fn emit_skipped(reason: SkipReason) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(event = "windows.phase_skipped", reason = reason.as_label(),);
}

fn emit_remote_refused() {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::warn!(
        event = "windows.remote_refused",
        reason = "allow_remote_not_set",
    );
}

#[cfg(target_os = "windows")]
fn emit_phase_started(cfg: &WindowsConfig) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(
        event = "windows.phase_started",
        sh_pathext = cfg.sh_pathext,
        sh_association = %match cfg.sh_association {
            ShAssociationMode::Off => "off",
            ShAssociationMode::Open => "open",
        },
        has_override = cfg.bash_path_override.is_some(),
    );
}

#[cfg(target_os = "windows")]
fn emit_phase_completed(
    pathext: Option<&'static str>,
    association: Option<&'static str>,
    duration: std::time::Duration,
) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(
        event = "windows.phase_completed",
        pathext = pathext.unwrap_or("skipped"),
        association = association.unwrap_or("skipped"),
        duration_ms = duration.as_millis() as u64,
    );
}

#[cfg(target_os = "windows")]
fn emit_pathext_applied() {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(event = "windows.pathext_applied", extension = ".SH");
}

#[cfg(target_os = "windows")]
fn emit_pathext_unchanged() {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(event = "windows.pathext_unchanged", extension = ".SH");
}

#[cfg(target_os = "windows")]
fn emit_pathext_failed(err: &std::io::Error) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::warn!(
        event = "windows.pathext_failed",
        error = %err,
    );
}

#[cfg(target_os = "windows")]
fn emit_association_applied(launcher: &sh_association::BashLauncher) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(
        event = "windows.sh_association_applied",
        program = %launcher.program,
    );
}

#[cfg(target_os = "windows")]
fn emit_association_unchanged(launcher: &sh_association::BashLauncher) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(
        event = "windows.sh_association_unchanged",
        program = %launcher.program,
    );
}

#[cfg(target_os = "windows")]
fn emit_association_failed(err: &sh_association::AssociationError) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::warn!(
        event = "windows.sh_association_failed",
        error = %err,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_hooks::ConfigOrigin;

    #[test]
    fn phase_skips_when_config_absent() {
        let outcome = run_windows_phase(None).unwrap();
        assert!(matches!(
            outcome,
            WindowsPhaseOutcome::Skipped(SkipReason::Disabled)
        ));
    }

    #[test]
    fn phase_skips_when_disabled() {
        let cfg = WindowsConfig {
            enabled: false,
            ..Default::default()
        };
        let outcome = run_windows_phase(Some(&cfg)).unwrap();
        assert!(matches!(
            outcome,
            WindowsPhaseOutcome::Skipped(SkipReason::Disabled)
        ));
    }

    #[test]
    fn phase_skips_when_both_knobs_off() {
        // enabled = true but no actual knob turned on — surfaces
        // as NothingToDo so operators see "user has the block but
        // no active knob" vs "user hasn't discovered the block".
        let cfg = WindowsConfig::default();
        let outcome = run_windows_phase(Some(&cfg)).unwrap();
        assert!(matches!(
            outcome,
            WindowsPhaseOutcome::Skipped(SkipReason::NothingToDo)
        ));
    }

    #[test]
    fn phase_refuses_remote_without_opt_in() {
        let cfg = WindowsConfig {
            sh_pathext: true,
            origin: ConfigOrigin::Remote,
            allow_remote: false,
            ..Default::default()
        };
        let outcome = run_windows_phase(Some(&cfg)).unwrap();
        assert!(matches!(
            outcome,
            WindowsPhaseOutcome::Skipped(SkipReason::RemoteRefused)
        ));
    }

    #[test]
    fn phase_allows_remote_when_explicitly_opted_in() {
        let cfg = WindowsConfig {
            sh_pathext: true,
            origin: ConfigOrigin::Remote,
            allow_remote: true,
            ..Default::default()
        };
        // On non-Windows the phase runs to NotWindows regardless.
        // On Windows it would attempt to apply — outcome depends on
        // the runner. Either way it must NOT be RemoteRefused.
        let outcome = run_windows_phase(Some(&cfg)).unwrap();
        assert!(!matches!(
            outcome,
            WindowsPhaseOutcome::Skipped(SkipReason::RemoteRefused)
        ));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn phase_skips_on_non_windows_target() {
        let cfg = WindowsConfig {
            sh_pathext: true,
            sh_association: ShAssociationMode::Open,
            ..Default::default()
        };
        let outcome = run_windows_phase(Some(&cfg)).unwrap();
        assert!(matches!(
            outcome,
            WindowsPhaseOutcome::Skipped(SkipReason::NotWindows)
        ));
    }

    #[test]
    fn phase_rejects_invalid_bash_path_override() {
        let cfg = WindowsConfig {
            sh_association: ShAssociationMode::Open,
            bash_path_override: Some("bash\".exe".to_string()),
            ..Default::default()
        };
        let err = run_windows_phase(Some(&cfg)).unwrap_err();
        assert!(matches!(err, WindowsPhaseError::InvalidBashPath(_)));
    }
}
