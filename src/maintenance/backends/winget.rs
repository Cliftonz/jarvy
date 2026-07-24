#![allow(dead_code)] // Windows-only backend; parsers reachable via tests on all OSes.

//! winget freshness backend via `winget show --id X --exact`.
//!
//! `winget show` output structure:
//!
//! ```text
//! Found <Name> [Publisher.Name]
//! Version: 24.0.7
//! Publisher: ...
//! ```
//!
//! On modern winget the header is `Found <Name> [<Id>]` followed
//! by a `Version:` line at column 0. Parsing that single field
//! keeps us robust to winget's output layout shifting per Windows
//! release (which happens more than we'd like).

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct WingetBackend;

impl FreshnessBackend for WingetBackend {
    fn name(&self) -> &'static str {
        "winget"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe = crate::tools::common::probe_with_timeout(
            "winget",
            &[
                "show",
                "--id",
                pkg_id,
                "--exact",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        // winget returns non-zero exit for "No package found" but
        // its stderr / stdout wording rotates. Fall through to
        // parsing so the caller sees `NotFound` when possible.
        parse_winget_show(&out.stdout)
    }
}

fn parse_winget_show(stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    if text.trim().is_empty() {
        return Err(BackendError::NotFound);
    }
    if text.contains("No package found matching")
        || text.contains("No installed package found matching")
    {
        return Err(BackendError::NotFound);
    }
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("Version:") else {
            continue;
        };
        let v = rest.trim();
        if v.is_empty() {
            return Err(BackendError::ParseFailed);
        }
        return Ok(v.to_string());
    }
    Err(BackendError::ParseFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_line() {
        let fixture =
            b"Found Git [Git.Git]\nVersion: 2.45.2\nPublisher: The Git Development Community\n";
        assert_eq!(parse_winget_show(fixture).unwrap(), "2.45.2");
    }

    #[test]
    fn no_package_found_is_not_found() {
        let fixture = b"No package found matching input criteria.\n";
        assert_eq!(
            parse_winget_show(fixture).unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn missing_version_is_parse_failed() {
        let fixture = b"Found Something [Some.Id]\nPublisher: nope\n";
        assert_eq!(
            parse_winget_show(fixture).unwrap_err(),
            BackendError::ParseFailed
        );
    }
}
