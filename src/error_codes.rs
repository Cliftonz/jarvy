//! Standardized process exit codes for Jarvy CLI.
//!
//! These codes are intended to be stable and shared across the codebase.
//! Keep meanings aligned with README/docs and CI expectations.
#![allow(dead_code)]

/// 0 — Success
///
/// Meaning: Command completed successfully.
/// Typical remediation: N/A
pub const EXIT_SUCCESS: i32 = 0;

/// 2 — Config/manifest error
///
/// Meaning: jarvy.toml or jarvy-extension.toml is missing or malformed.
/// Typical remediation: Fix the configuration file(s) and retry.
pub const CONFIG_ERROR: i32 = 2;

/// 3 — Prerequisite missing
///
/// Meaning: Required package manager or dependency (e.g., Homebrew) is not installed.
/// Typical remediation: Install the missing prerequisite and rerun.
pub const PREREQ_MISSING: i32 = 3;

/// 4 — Network/timeout
///
/// Meaning: Network issue, timeout, or proxy-related failure occurred.
/// Typical remediation: Check connectivity and proxy settings; retry.
pub const NETWORK_TIMEOUT: i32 = 4;

/// 5 — Permission/elevation required
///
/// Meaning: Operation requires elevated privileges (admin/sudo).
/// Typical remediation: Re-run with admin/sudo or adjust permissions.
pub const PERMISSION_REQUIRED: i32 = 5;

/// 6 — Incompatible OS/arch
///
/// Meaning: The current OS/architecture is unsupported for the requested action.
/// Typical remediation: Adjust the target or use an alternative installer.
pub const INCOMPATIBLE_OS_ARCH: i32 = 6;

pub struct ErrorCodeInfo {
    pub code: i32,
    pub key: &'static str,
    pub meaning: &'static str,
    pub remediation: &'static str,
    pub slug: &'static str,
}

static ERROR_CODES: &[ErrorCodeInfo] = &[
    ErrorCodeInfo {
        code: EXIT_SUCCESS,
        key: "EXIT_SUCCESS",
        meaning: "Command completed successfully.",
        remediation: "N/A",
        slug: "exit_success",
    },
    ErrorCodeInfo {
        code: CONFIG_ERROR,
        key: "CONFIG_ERROR",
        meaning: "jarvy.toml or jarvy-extension.toml is missing or malformed.",
        remediation: "Fix the configuration file(s) and retry.",
        slug: "config_error",
    },
    ErrorCodeInfo {
        code: PREREQ_MISSING,
        key: "PREREQ_MISSING",
        meaning: "Required package manager or dependency (e.g., Homebrew) is not installed.",
        remediation: "Install the missing prerequisite and rerun.",
        slug: "prereq_missing",
    },
    ErrorCodeInfo {
        code: NETWORK_TIMEOUT,
        key: "NETWORK_TIMEOUT",
        meaning: "Network issue, timeout, or proxy-related failure occurred.",
        remediation: "Check connectivity and proxy settings; retry.",
        slug: "network_timeout",
    },
    ErrorCodeInfo {
        code: PERMISSION_REQUIRED,
        key: "PERMISSION_REQUIRED",
        meaning: "Operation requires elevated privileges (admin/sudo).",
        remediation: "Re-run with admin/sudo or adjust permissions.",
        slug: "permission_required",
    },
    ErrorCodeInfo {
        code: INCOMPATIBLE_OS_ARCH,
        key: "INCOMPATIBLE_OS_ARCH",
        meaning: "The current OS/architecture is unsupported for the requested action.",
        remediation: "Adjust the target or use an alternative installer.",
        slug: "incompatible_os_arch",
    },
];

pub fn list_error_codes() -> &'static [ErrorCodeInfo] {
    ERROR_CODES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_have_expected_values() {
        assert_eq!(EXIT_SUCCESS, 0);
        assert_eq!(CONFIG_ERROR, 2);
        assert_eq!(PREREQ_MISSING, 3);
        assert_eq!(NETWORK_TIMEOUT, 4);
        assert_eq!(PERMISSION_REQUIRED, 5);
        assert_eq!(INCOMPATIBLE_OS_ARCH, 6);
    }

    #[test]
    fn list_contains_all_codes() {
        let list = list_error_codes();
        let codes: Vec<i32> = list.iter().map(|e| e.code).collect();
        assert!(codes.contains(&EXIT_SUCCESS));
        assert!(codes.contains(&CONFIG_ERROR));
        assert!(codes.contains(&PREREQ_MISSING));
        assert!(codes.contains(&NETWORK_TIMEOUT));
        assert!(codes.contains(&PERMISSION_REQUIRED));
        assert!(codes.contains(&INCOMPATIBLE_OS_ARCH));
    }
}
