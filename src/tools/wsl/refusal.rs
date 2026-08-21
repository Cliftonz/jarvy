//! Bounded refusal reasons for the WSL tool. Every refusal is either
//! emitted as a `wsl.bootstrap_refused` telemetry event with a bounded
//! `reason` label, or printed to the user with a copy-pasteable
//! remediation command. Keeping this enum in one place keeps the
//! `wsl.*` event taxonomy honest.

#![allow(dead_code)] // ReservedName variant reserved for future defensive checks

/// Reason the WSL tool refused to complete a bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    /// User did not opt in (`[windows.wsl] enabled = false`).
    NotOptedIn,
    /// `wsl --version` returned nothing — machine is either pre-Store
    /// WSL (Windows 10 pre-21H2) or WSL was never installed. Jarvy
    /// refuses to touch DISM.
    NotStoreWsl,
    /// The `wsl.exe` command is not on PATH at all.
    NoWslCommand,
    /// Distro is missing and `auto_bootstrap = false`.
    AutoBootstrapOff,
    /// Distro is missing, `auto_bootstrap = true`, but the outer
    /// process is not elevated — jarvy will not trigger UAC.
    NotElevated,
    /// Config carried a reserved distro name (`docker-desktop`,
    /// `docker-desktop-data`). Should be refused at config load; this
    /// variant exists so bootstrap can double-check defensively.
    ReservedName,
}

impl RefusalReason {
    /// Bounded label for the `wsl.bootstrap_refused { reason = ... }`
    /// telemetry field.
    pub fn as_label(self) -> &'static str {
        match self {
            RefusalReason::NotOptedIn => "not_opted_in",
            RefusalReason::NotStoreWsl => "not_store_wsl",
            RefusalReason::NoWslCommand => "no_wsl_command",
            RefusalReason::AutoBootstrapOff => "auto_bootstrap_off",
            RefusalReason::NotElevated => "not_elevated",
            RefusalReason::ReservedName => "reserved_name",
        }
    }
}
