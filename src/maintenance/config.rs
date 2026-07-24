//! `[maintenance]` config block (PRD-057).

use serde::{Deserialize, Serialize};

use crate::ai_hooks::{ConfigOrigin, HasOrigin};

/// Default cache TTL when the user doesn't override it. Chosen to
/// give a "once a day" cadence on machines that run jarvy at least
/// daily; setups that never re-run stay stale forever, which is the
/// intended behavior (no daemon).
pub const DEFAULT_CACHE_TTL_HOURS: u64 = 24;

/// `[maintenance]` block. Parsed from `jarvy.toml`; every field is
/// optional so an empty block still yields the default (on) behavior.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MaintenanceConfig {
    /// Master switch. Default `true` — freshness advisories are the
    /// intended out-of-the-box UX. Set `false` to opt a project out.
    #[serde(default = "default_check_updates")]
    pub check_updates: bool,

    /// Cache TTL in hours. `0` forces every read to trigger a
    /// refresh (development / debugging setting; not recommended
    /// for normal use because it defeats the whole "cheap setup
    /// summary" model).
    #[serde(default = "default_cache_ttl_hours")]
    pub cache_ttl_hours: u64,

    /// Tools to skip entirely. Names match `[provisioner]` keys.
    /// Useful for tools with noisy upstream release cadence (e.g.
    /// nightly rebuilds) or where the maintainer has intentionally
    /// pinned an older version.
    #[serde(default)]
    pub ignore: Vec<String>,

    /// When to surface the summary during `jarvy setup`. `"setup"`
    /// (default) prints a one-line advisory after the completion
    /// banner. `"manual"` only surfaces via `jarvy check-updates`.
    /// `"never"` silences everything but keeps the background
    /// refresh spawn (so the cache stays warm for other integrations).
    #[serde(default = "default_notify_on")]
    pub notify_on: NotifyOn,

    /// Remote-config trust gate. When this `MaintenanceConfig`
    /// arrives via `jarvy setup --from <url>` (i.e.
    /// `ConfigOrigin::Remote`), the phase refuses to enable
    /// `check_updates` unless this is `true`. A remote config
    /// setting `check_updates = false` is always honored (narrowing
    /// trust is fine); flipping it on requires explicit local
    /// consent.
    #[serde(default)]
    pub allow_remote: bool,

    /// Origin tag. Not serialized; populated by
    /// `Config::mark_remote()`.
    #[serde(skip)]
    pub origin: ConfigOrigin,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            check_updates: default_check_updates(),
            cache_ttl_hours: default_cache_ttl_hours(),
            ignore: Vec::new(),
            notify_on: default_notify_on(),
            allow_remote: false,
            origin: ConfigOrigin::default(),
        }
    }
}

impl HasOrigin for MaintenanceConfig {
    fn set_origin(&mut self, origin: ConfigOrigin) {
        self.origin = origin;
    }
}

/// When to print the advisory summary line during setup.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NotifyOn {
    #[default]
    Setup,
    Manual,
    Never,
}

fn default_check_updates() -> bool {
    true
}

fn default_cache_ttl_hours() -> u64 {
    DEFAULT_CACHE_TTL_HOURS
}

fn default_notify_on() -> NotifyOn {
    NotifyOn::Setup
}

impl MaintenanceConfig {
    /// Return `true` when a remote-origin config is trying to enable
    /// `check_updates` without explicit `allow_remote = true`. The
    /// caller should refuse the phase and emit
    /// `maintenance.remote_refused`.
    pub fn remote_refused(&self) -> bool {
        matches!(self.origin, ConfigOrigin::Remote) && self.check_updates && !self.allow_remote
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_on() {
        let c = MaintenanceConfig::default();
        assert!(c.check_updates);
        assert_eq!(c.cache_ttl_hours, DEFAULT_CACHE_TTL_HOURS);
        assert_eq!(c.notify_on, NotifyOn::Setup);
    }

    #[test]
    fn remote_config_on_without_allow_is_refused() {
        let mut c = MaintenanceConfig {
            check_updates: true,
            allow_remote: false,
            ..Default::default()
        };
        c.set_origin(ConfigOrigin::Remote);
        assert!(c.remote_refused());
    }

    #[test]
    fn remote_config_off_is_never_refused() {
        let mut c = MaintenanceConfig {
            check_updates: false,
            allow_remote: false,
            ..Default::default()
        };
        c.set_origin(ConfigOrigin::Remote);
        assert!(
            !c.remote_refused(),
            "narrowing trust must always be allowed"
        );
    }

    #[test]
    fn remote_config_with_allow_is_permitted() {
        let mut c = MaintenanceConfig {
            check_updates: true,
            allow_remote: true,
            ..Default::default()
        };
        c.set_origin(ConfigOrigin::Remote);
        assert!(!c.remote_refused());
    }

    #[test]
    fn local_config_ignores_gate() {
        let c = MaintenanceConfig {
            check_updates: true,
            allow_remote: false,
            ..Default::default()
        };
        assert!(!c.remote_refused());
    }
}
