#![allow(dead_code)] // Linux-only backend; parsers reachable via tests on all OSes.

//! dnf freshness backend via `dnf info`.
//!
//! `dnf info <pkg>` output blocks look like:
//!
//! ```text
//! Available Packages
//! Name         : jq
//! Version      : 1.7.1
//! Release      : 3.fc40
//! ...
//! ```
//!
//! Packages that are installed emit two blocks (`Installed
//! Packages` then `Available Packages`). We take the LAST
//! `Version :` line so the newer of the two wins when both are
//! present.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct DnfBackend;

impl FreshnessBackend for DnfBackend {
    fn name(&self) -> &'static str {
        "dnf"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe = crate::tools::common::probe_with_timeout(
            "dnf",
            &["info", "--quiet", pkg_id],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("No matches found")
                || stderr.contains("Error: No matching")
                || stderr.contains("No package")
            {
                return Err(BackendError::NotFound);
            }
            return Err(BackendError::Other);
        }
        parse_dnf_info(&out.stdout)
    }
}

fn parse_dnf_info(stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    let mut latest: Option<String> = None;
    for line in text.lines() {
        let line = line.trim_start();
        // dnf uses "Version      : X.Y.Z" (localizable in older
        // dnf5, but "Version" stays English by default). Match on
        // the ':' split so extra whitespace / dnf5 field ordering
        // doesn't break parsing.
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        if label.trim() != "Version" {
            continue;
        }
        let value = value.trim();
        if !value.is_empty() {
            latest = Some(value.to_string());
        }
    }
    latest.ok_or(BackendError::ParseFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_line() {
        let fixture =
            b"Available Packages\nName         : jq\nVersion      : 1.7.1\nRelease      : 3.fc40\n";
        assert_eq!(parse_dnf_info(fixture).unwrap(), "1.7.1");
    }

    #[test]
    fn takes_last_version_when_installed_and_available_both_listed() {
        let fixture = b"Installed Packages\nName         : jq\nVersion      : 1.6\nAvailable Packages\nName         : jq\nVersion      : 1.7.1\n";
        assert_eq!(parse_dnf_info(fixture).unwrap(), "1.7.1");
    }

    #[test]
    fn missing_version_is_parse_failed() {
        let fixture = b"Available Packages\nName : jq\n";
        assert_eq!(
            parse_dnf_info(fixture).unwrap_err(),
            BackendError::ParseFailed
        );
    }
}
