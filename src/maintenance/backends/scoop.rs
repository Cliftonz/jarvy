#![allow(dead_code)] // Windows-only backend; parsers reachable via tests on all OSes.

//! Scoop freshness backend via `scoop info`.
//!
//! `scoop info <pkg>` prints a key-value block. Modern scoop
//! (v0.4+) emits proper `Name`, `Version`, `Description`, ...
//! lines. Older scoop wraps output in a formatted table but the
//! `Version` key is still on its own line — safe to grep.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct ScoopBackend;

impl FreshnessBackend for ScoopBackend {
    fn name(&self) -> &'static str {
        "scoop"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe =
            crate::tools::common::probe_with_timeout("scoop", &["info", pkg_id], BACKEND_TIMEOUT);
        let out = probe_error(probe)?;
        if !out.status.success() {
            return Err(BackendError::Other);
        }
        parse_scoop_info(&out.stdout)
    }
}

fn parse_scoop_info(stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    if text.trim().is_empty() {
        return Err(BackendError::NotFound);
    }
    if text.contains("Could not find manifest") || text.contains("No matches found") {
        return Err(BackendError::NotFound);
    }
    for line in text.lines() {
        let line = line.trim();
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        if label.trim().eq_ignore_ascii_case("Version") {
            let v = value.trim();
            if v.is_empty() {
                return Err(BackendError::ParseFailed);
            }
            return Ok(v.to_string());
        }
    }
    Err(BackendError::ParseFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_line() {
        let fixture = b"Name: git\nVersion: 2.45.2\nBucket: main\n";
        assert_eq!(parse_scoop_info(fixture).unwrap(), "2.45.2");
    }

    #[test]
    fn not_found_error_text() {
        let fixture = b"Could not find manifest for 'nope'\n";
        assert_eq!(
            parse_scoop_info(fixture).unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn empty_stdout_is_not_found() {
        assert_eq!(parse_scoop_info(b"").unwrap_err(), BackendError::NotFound);
    }
}
