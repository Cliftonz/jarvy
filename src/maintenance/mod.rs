//! Tool freshness advisory (PRD-057).
//!
//! Compares installed provisioner-tool versions against the latest
//! version each tool's package manager reports. Read-only and
//! advisory — never blocks setup, never auto-upgrades. Results land
//! in `~/.jarvy/update-cache.json`; `jarvy setup` reads the cache
//! and spawns a detached background refresher so the *next*
//! invocation sees fresh data. No inline network work on the hot
//! path.
//!
//! Trust boundaries (mirror the `[packages]` / `[git_hooks]`
//! pattern):
//!
//! - `[maintenance] check_updates = false` opts a project out.
//! - `JARVY_CHECK_UPDATES=0` env kill-switch.
//! - Sandbox / CI / non-TTY skip the *spawn* (cache reads still
//!   happen — they cost nothing).
//! - Remote-fetched configs (`ConfigOrigin::Remote`) cannot flip
//!   `check_updates` on without `allow_remote = true` — a hostile
//!   remote can't force network probes on the operator's box.
//!
//! No new HTTP surface — every backend shells out to an existing
//! package-manager binary. If the manager isn't installed, the tool
//! lands in the `unchecked` bucket. Same trust surface as `setup`.

pub mod backends;
pub mod cache;
pub mod checker;
pub mod config;
pub mod lock;
pub mod reporter;
pub mod resolver;

pub use config::MaintenanceConfig;

/// Kill-switch env var (documented in `docs/environment.md`). `0` /
/// `false` / `no` disables the phase globally. Overrides
/// `[maintenance] check_updates = true`. See CLAUDE.md's Trust
/// Boundaries section for the full precedence.
pub const KILL_SWITCH_ENV: &str = "JARVY_CHECK_UPDATES";

/// Return `true` when the kill-switch env var explicitly disables
/// the phase. Missing env is `false` (do not disable), so the
/// default remains "on".
pub fn kill_switch_engaged() -> bool {
    match std::env::var(KILL_SWITCH_ENV) {
        Ok(v) => v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("no"),
        Err(_) => false,
    }
}

/// Reasons the phase may be skipped without emitting a report.
/// Maps to the `maintenance.phase_skipped { reason }` telemetry
/// event; keep this enum in lock-step with the taxonomy table in
/// CLAUDE.md so on-call dashboards remain stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Reserved for TTY / cache-fresh / remote gates
// wired up in follow-up tasks; enum defines the
// full skip taxonomy up-front so telemetry
// reason labels never drift.
pub enum SkipReason {
    DisabledByConfig,
    DisabledByEnv,
    Sandbox,
    Ci,
    NonTty,
    NoProvisionerTools,
    CacheFresh,
    RemoteRefused,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DisabledByConfig => "disabled_by_config",
            Self::DisabledByEnv => "disabled_by_env",
            Self::Sandbox => "sandbox",
            Self::Ci => "ci",
            Self::NonTty => "non_tty",
            Self::NoProvisionerTools => "no_provisioner_tools",
            Self::CacheFresh => "cache_fresh",
            Self::RemoteRefused => "remote_refused",
        }
    }
}

/// Decide whether the phase should run in the current environment.
/// Returns `Ok(())` when everything is clear to proceed, `Err(reason)`
/// with a bounded label suitable for `phase_skipped` telemetry.
///
/// Precedence:
/// 1. Config-level opt-out (`check_updates = false`).
/// 2. Env kill-switch (`JARVY_CHECK_UPDATES=0`).
/// 3. Sandbox auto-skip.
/// 4. CI auto-skip.
///
/// TTY and cache-fresh are caller-specific — they're only enforced
/// in the spawn path from `setup`, not in the explicit
/// `jarvy check-updates` CLI. Handle those there.
pub fn should_run(cfg: &MaintenanceConfig) -> Result<(), SkipReason> {
    if !cfg.check_updates {
        return Err(SkipReason::DisabledByConfig);
    }
    if kill_switch_engaged() {
        return Err(SkipReason::DisabledByEnv);
    }
    if crate::sandbox::is_sandbox() {
        return Err(SkipReason::Sandbox);
    }
    if crate::ci::is_ci() {
        return Err(SkipReason::Ci);
    }
    Ok(())
}
