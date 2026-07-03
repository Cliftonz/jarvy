//! Wizard session token — RAII marker file that gates the MCP
//! mutation bypass in `mcp::extended_tools::gate_mutation`.
//!
//! # Model
//!
//! `JARVY_WIZARD_SESSION=1` on its own is an ambient env-var flag. It
//! propagates into every descendant process the spawned agent forks
//! (language servers, `tmux new-session`, `nohup`-detached
//! background daemons, docker daemons, …) and survives well past the
//! wizard's own exit. A prompt-injection scenario where the agent
//! reads a hostile file and issues extra MCP mutating calls — or a
//! future refactor that leaves a stray env carrier alive — silently
//! carries the bypass token into contexts the operator never
//! consented to.
//!
//! This module tightens the check by tying the bypass to a
//! per-invocation **marker file** at
//! `~/.jarvy/state/wizard-session-<uuid>.active`. The wizard creates
//! it on start (RAII: `WizardSessionGuard::activate`) and removes it
//! on scope exit (Drop). `gate_mutation` reads the UUID from the
//! `JARVY_WIZARD_SESSION_ID` env var and requires the paired marker
//! file to exist AND be recent (mtime within a wall-clock window) —
//! stale tokens from crashed wizards can't be replayed after the
//! guard's Drop failed to run.
//!
//! # Threat model
//!
//! The check is narrowly a defense-in-depth boundary against
//! ambient-env leakage. Same-user code exec remains game-over — an
//! attacker who can write to `~/.jarvy/state/` or fake the env can
//! forge a token. But the ambient descendant case (language server
//! forked from the wizard-spawned agent, still alive hours after the
//! wizard exited) no longer bypasses: no active marker file, no
//! bypass.

use std::path::PathBuf;

/// Env-var name carrying the per-invocation UUID. Kept in one place
/// so `wizard/headless.rs`, `wizard/session.rs`, and
/// `mcp/extended_tools.rs` don't drift into three different string
/// literals.
pub const SESSION_ID_ENV: &str = "JARVY_WIZARD_SESSION_ID";

/// Max wall-clock age of a valid session marker. A token file with
/// mtime older than this is treated as stale and refused — a lingering
/// descendant that inherited the env var but is running an hour after
/// the wizard exited can't bypass the gate.
///
/// Wizard sessions in practice last seconds-to-minutes (the spawned
/// agent completes its playbook then exits). Ten minutes is generous
/// enough to cover a slow LLM call over a bad network while still
/// bounding the window.
const MAX_TOKEN_AGE_SECS: u64 = 10 * 60;

/// RAII marker that a wizard invocation is live. Constructed by
/// `activate()` in `commands::wizard_cmd::run_headless`, dropped when
/// the wizard scope exits (including panic unwinds — RAII guarantees).
///
/// Cleanup is best-effort: a hard kill (SIGKILL) or crash mid-run
/// leaves a stale marker. The age check in `is_active()` catches
/// those on the next check.
pub struct WizardSessionGuard {
    token_path: Option<PathBuf>,
}

impl WizardSessionGuard {
    /// Create the marker file at `~/.jarvy/state/wizard-session-
    /// <session_id>.active`. Returns a guard whose Drop removes the
    /// file.
    ///
    /// Errors are downgraded to a warning and the guard becomes a
    /// no-op — the wizard still runs, and `gate_mutation` will fall
    /// back to the normal confirmation prompt when the marker check
    /// fails. Better to lose the ergonomic bypass than to refuse the
    /// wizard entirely because `~/.jarvy/state/` couldn't be created.
    pub fn activate(session_id: &str) -> Self {
        match token_path_for(session_id) {
            Ok(path) => {
                if let Some(parent) = path.parent() {
                    // Sec F2: `create_dir_all` uses the default umask
                    // (usually 0755) — that lets any local user
                    // `ls ~victim/.jarvy/state/` and enumerate session
                    // UUIDs, undermining the file-level 0600 chmod
                    // below (attacker doesn't need to READ the file
                    // to know the UUID + wizard-running status).
                    // Mirror the pattern used for staging_dir / backup_dir.
                    let _ = crate::paths::ensure_dir_0700(parent);
                }
                match std::fs::write(&path, session_id) {
                    Ok(()) => {
                        // Best-effort chmod to 0600 — even though the file
                        // contains no secrets, it's a session-scoped
                        // capability marker. Restrict to owner-only so
                        // another user on the box can't forge it.
                        // Mirrors atomic_write's chmod telemetry
                        // pattern — if chmod silently fails on
                        // NFS/drvfs/exFAT, on-call sees the event
                        // rather than debugging "wizard bypass worked
                        // when it shouldn't have."
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Err(e) = std::fs::set_permissions(
                                &path,
                                std::fs::Permissions::from_mode(0o600),
                            ) {
                                if crate::observability::telemetry_gate::is_enabled() {
                                    tracing::warn!(
                                        event = "wizard.session_token_perms_unsafe",
                                        path = %path.display(),
                                        error = %e,
                                        fs_hint = "chmod_failed",
                                    );
                                }
                            } else if let Ok(meta) = std::fs::metadata(&path) {
                                let mode = meta.permissions().mode() & 0o777;
                                if mode != 0o600
                                    && crate::observability::telemetry_gate::is_enabled()
                                {
                                    tracing::warn!(
                                        event = "wizard.session_token_perms_unsafe",
                                        path = %path.display(),
                                        mode = format!("{mode:o}"),
                                        fs_hint = "chmod_ignored",
                                    );
                                }
                            }
                        }
                        Self {
                            token_path: Some(path),
                        }
                    }
                    Err(e) => {
                        if crate::observability::telemetry_gate::is_enabled() {
                            tracing::warn!(
                                event = "wizard.session_token_activate_failed",
                                error = %e,
                                path = %path.display(),
                            );
                        }
                        Self { token_path: None }
                    }
                }
            }
            Err(_) => Self { token_path: None },
        }
    }
}

impl Drop for WizardSessionGuard {
    fn drop(&mut self) {
        if let Some(path) = self.token_path.take() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Compute the marker-file path for a given session UUID. Kept
/// deterministic so `gate_mutation` can independently derive the same
/// path from the env-var UUID without any shared state.
pub fn token_path_for(session_id: &str) -> Result<PathBuf, crate::paths::NoHomeDir> {
    Ok(crate::paths::state_dir()?.join(format!("wizard-session-{session_id}.active")))
}

/// Check whether the current process is a legitimate descendant of a
/// LIVE wizard. Called by `mcp::extended_tools::gate_mutation`.
///
/// Returns `true` only when all three conditions hold:
/// 1. `JARVY_WIZARD_SESSION_ID` is set to a non-empty value.
/// 2. The paired marker file at `~/.jarvy/state/wizard-session-
///    <uuid>.active` exists.
/// 3. The marker file's mtime is within `MAX_TOKEN_AGE_SECS`.
///
/// Any failure short-circuits to `false` — the caller then falls back
/// to the normal confirmation gate (or refuses if stdin isn't a TTY).
pub fn is_active() -> bool {
    // Cheap tracing span so on-call can spot filesystem-slow homes
    // (network mounts, macOS Time Machine snapshots, ~/.jarvy on
    // synced drives) that make the marker-file stat expensive.
    // Debug level — one span per mutating MCP call, which is rare.
    let _guard = tracing::debug_span!("wizard.session.check").entered();

    let Ok(session_id) = std::env::var(SESSION_ID_ENV) else {
        emit_refused("env_missing", "");
        return false;
    };
    if session_id.is_empty() {
        emit_refused("env_empty", "");
        return false;
    }
    let Ok(path) = token_path_for(&session_id) else {
        emit_refused("no_home", &session_id);
        return false;
    };
    let Ok(meta) = std::fs::metadata(&path) else {
        emit_refused("marker_missing", &session_id);
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        emit_refused("mtime_unavailable", &session_id);
        return false;
    };
    let session_id_owned = std::env::var(SESSION_ID_ENV).unwrap_or_default();
    match mtime.elapsed() {
        Ok(age) => {
            if age.as_secs() <= MAX_TOKEN_AGE_SECS {
                true
            } else {
                emit_refused("marker_stale", &session_id_owned);
                false
            }
        }
        // Future mtime: `elapsed()` returns Err(SystemTimeError). This
        // can happen legitimately (fs with second-granularity mtime;
        // a wizard that just started; NTP-adjusted clock jump). But
        // an attacker with `utimensat` can also plant a future mtime
        // to keep the bypass alive indefinitely — pre-fix the code
        // returned `true` for any future mtime, converting the
        // 10-minute window into an unbounded window with one syscall.
        //
        // Cap forward drift at MAX_FORWARD_SKEW_SECS. Legitimate skew
        // is measured in seconds; hours-ahead mtimes are forged.
        Err(_) => match std::time::SystemTime::now().duration_since(mtime) {
            // mtime > now (still in the Err arm because now < mtime)
            // — measure how far ahead.
            Err(future_delta) => {
                let allowed = future_delta.duration().as_secs() <= MAX_FORWARD_SKEW_SECS;
                if !allowed {
                    emit_refused("marker_future_mtime", &session_id_owned);
                }
                allowed
            }
            // duration_since returned Ok — means mtime <= now — hit
            // during a race between the two syscalls. Accept.
            Ok(_) => true,
        },
    }
}

/// Max forward mtime skew we tolerate on the marker file. Legitimate
/// clock jitter is measured in seconds — anything beyond is forged.
/// Same-user code exec is game-over per the threat model; this bound
/// is defense-in-depth against a single-syscall persistence trick.
const MAX_FORWARD_SKEW_SECS: u64 = 5;

/// Debug-level event emitted when `is_active()` refuses the bypass.
/// On-call answering "why did MCP mutation X get gated?" can
/// distinguish "user typed no" from "orphaned descendant of a
/// killed wizard tried to bypass" from "clock skew". Telemetry-gated.
fn emit_refused(reason: &'static str, session_id: &str) {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::debug!(
            event = "wizard.session.bypass_refused",
            reason = reason,
            session_id = %session_id,
        );
    }
}

#[cfg(test)]
#[allow(unsafe_code)] // Env-var manipulation is the entire point.
mod tests {
    use super::*;

    /// RAII contract: guard's Drop removes the marker file. Without
    /// this, a stale marker from a panicked wizard could be replayed
    /// by any process that inherits the SESSION_ID env var.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn guard_drop_removes_the_marker_file() {
        // Isolate JARVY_HOME so we don't touch the developer's real
        // ~/.jarvy/state/ directory.
        let tmp = tempfile::TempDir::new().unwrap();
        // SAFETY: serial_test::serial serialises access to the env.
        unsafe { std::env::set_var("JARVY_HOME", tmp.path()) };
        let session_id = uuid::Uuid::now_v7().to_string();

        let path = token_path_for(&session_id).unwrap();
        assert!(!path.exists(), "precondition: token doesn't yet exist");

        {
            let _guard = WizardSessionGuard::activate(&session_id);
            assert!(path.exists(), "activate must create the marker file");
        }
        // Drop ran — file gone.
        assert!(
            !path.exists(),
            "guard Drop must remove the marker file (RAII contract) — \
             found stale token at {path:?}"
        );
        unsafe { std::env::remove_var("JARVY_HOME") };
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn is_active_returns_true_when_marker_exists_and_env_set() {
        let tmp = tempfile::TempDir::new().unwrap();
        unsafe { std::env::set_var("JARVY_HOME", tmp.path()) };
        let session_id = uuid::Uuid::now_v7().to_string();
        unsafe { std::env::set_var(SESSION_ID_ENV, &session_id) };
        let _guard = WizardSessionGuard::activate(&session_id);
        assert!(
            is_active(),
            "is_active must return true when env var + marker file both present"
        );
        unsafe { std::env::remove_var(SESSION_ID_ENV) };
        unsafe { std::env::remove_var("JARVY_HOME") };
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn is_active_returns_false_when_env_set_but_marker_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        unsafe { std::env::set_var("JARVY_HOME", tmp.path()) };
        // Env carries a session ID but NO paired marker file (stale
        // env from a killed/crashed wizard). Must refuse the bypass.
        unsafe { std::env::set_var(SESSION_ID_ENV, "orphaned-session") };
        assert!(
            !is_active(),
            "orphaned env var without a marker file must NOT authorise \
             the bypass — that's the whole point of the marker check"
        );
        unsafe { std::env::remove_var(SESSION_ID_ENV) };
        unsafe { std::env::remove_var("JARVY_HOME") };
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn is_active_returns_false_when_env_missing() {
        unsafe { std::env::remove_var(SESSION_ID_ENV) };
        assert!(
            !is_active(),
            "no SESSION_ID_ENV → no bypass, regardless of any marker \
             files on disk"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn is_active_returns_false_when_env_empty_string() {
        unsafe { std::env::set_var(SESSION_ID_ENV, "") };
        assert!(
            !is_active(),
            "empty session id is not a valid identifier — must refuse"
        );
        unsafe { std::env::remove_var(SESSION_ID_ENV) };
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn is_active_returns_false_for_marker_older_than_max_age() {
        // Regression guard for the 10-minute staleness window. A
        // future refactor that inverts the elapsed-time comparison
        // (`>` → `<`), drops the age check, or moves cleanup earlier
        // MUST fail this test. Uses `filetime` (dev-dep) to force an
        // arbitrary mtime — `std::fs` doesn't expose an mtime setter.
        let tmp = tempfile::TempDir::new().unwrap();
        unsafe { std::env::set_var("JARVY_HOME", tmp.path()) };
        let session_id = uuid::Uuid::now_v7().to_string();
        let path = token_path_for(&session_id).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &session_id).unwrap();
        // Backdate the marker's mtime past the staleness window.
        let old =
            std::time::SystemTime::now() - std::time::Duration::from_secs(MAX_TOKEN_AGE_SECS + 60);
        filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(old))
            .expect("set_file_mtime must succeed on the temp file");
        unsafe { std::env::set_var(SESSION_ID_ENV, &session_id) };
        assert!(
            !is_active(),
            "marker older than MAX_TOKEN_AGE_SECS ({}s) MUST refuse — \
             the whole point of the age check is to catch stale tokens \
             from crashed / killed wizards whose Drop didn't run",
            MAX_TOKEN_AGE_SECS
        );
        let _ = std::fs::remove_file(&path);
        unsafe { std::env::remove_var(SESSION_ID_ENV) };
        unsafe { std::env::remove_var("JARVY_HOME") };
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn is_active_refuses_future_mtime_beyond_clock_skew() {
        // A same-user attacker with `utimensat` can set a marker's
        // mtime hours ahead to keep the bypass alive indefinitely.
        // The "clock skew" allowance must cap forward drift at a small
        // window — hours-ahead mtimes are forged, not clock jitter.
        // Pin the invariant so a future edit that widens the tolerance
        // for `Err(SystemTimeError)` (future mtime) doesn't quietly
        // reopen the persistence hole.
        let tmp = tempfile::TempDir::new().unwrap();
        unsafe { std::env::set_var("JARVY_HOME", tmp.path()) };
        let session_id = uuid::Uuid::now_v7().to_string();
        let path = token_path_for(&session_id).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &session_id).unwrap();
        // Set mtime 24 hours in the future — no legitimate wizard
        // start produces this.
        let far_future =
            std::time::SystemTime::now() + std::time::Duration::from_secs(24 * 60 * 60);
        filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(far_future)).unwrap();
        unsafe { std::env::set_var(SESSION_ID_ENV, &session_id) };
        assert!(
            !is_active(),
            "24-hour-future mtime must be treated as forged, not as \
             clock skew — a wizard that just started never has an mtime \
             hours ahead of now"
        );
        let _ = std::fs::remove_file(&path);
        unsafe { std::env::remove_var(SESSION_ID_ENV) };
        unsafe { std::env::remove_var("JARVY_HOME") };
    }

    /// Graceful-degradation: if `~/.jarvy/state/` can't be created (perms
    /// error, disk full, read-only FS), `activate` returns a no-op guard
    /// and the wizard still runs — falling back to the normal confirmation
    /// gate. Pin so a refactor that panics instead of degrading breaks
    /// wizard on any read-only home.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn activate_degrades_gracefully_when_state_dir_write_fails() {
        let tmp = tempfile::TempDir::new().unwrap();
        // JARVY_HOME points at a regular FILE, not a dir — so
        // `create_dir_all(state_dir)` will fail.
        let bogus = tmp.path().join("home-is-a-file");
        std::fs::write(&bogus, "").unwrap();
        unsafe { std::env::set_var("JARVY_HOME", &bogus) };
        // Neither activate() nor drop() may panic.
        let guard = WizardSessionGuard::activate("test-uuid");
        drop(guard);
        unsafe { std::env::remove_var("JARVY_HOME") };
    }
}
