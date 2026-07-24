#![allow(dead_code)] // Linux-only backend; parsers reachable via tests on all OSes.

//! Arch Linux `pacman` freshness backend via `pacman -Si <pkg>`.
//!
//! `-S` = sync repo, `-i` = info. Output is a key-value block:
//!
//! ```text
//! Repository      : extra
//! Name            : jq
//! Version         : 1.7.1-3
//! Description     : ...
//! ```
//!
//! The `Version` field carries a `-<pkgrel>` suffix (Arch's
//! packaging revision); we strip it so the semver compare
//! against upstream doesn't misfire.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct PacmanBackend;

impl FreshnessBackend for PacmanBackend {
    fn name(&self) -> &'static str {
        "pacman"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe =
            crate::tools::common::probe_with_timeout("pacman", &["-Si", pkg_id], BACKEND_TIMEOUT);
        let out = probe_error(probe)?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("was not found") || stderr.contains("target not found") {
                return Err(BackendError::NotFound);
            }
            return Err(BackendError::Other);
        }
        parse_pacman_info(&out.stdout)
    }
}

fn parse_pacman_info(stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    for line in text.lines() {
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        if label.trim() != "Version" {
            continue;
        }
        let v = value.trim();
        if v.is_empty() {
            return Err(BackendError::ParseFailed);
        }
        return Ok(strip_pkgrel(v));
    }
    Err(BackendError::ParseFailed)
}

/// `1.7.1-3` → `1.7.1`. Arch's pkgrel is always numeric after the
/// last `-`, so we can strip it deterministically.
fn strip_pkgrel(v: &str) -> String {
    match v.rsplit_once('-') {
        Some((upstream, rev)) if rev.chars().all(|c| c.is_ascii_digit() || c == '.') => {
            upstream.to_string()
        }
        _ => v.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_and_strips_pkgrel() {
        let fixture =
            b"Repository      : extra\nName            : jq\nVersion         : 1.7.1-3\nDescription     : Command-line JSON processor\n";
        assert_eq!(parse_pacman_info(fixture).unwrap(), "1.7.1");
    }

    #[test]
    fn keeps_version_without_pkgrel() {
        let fixture = b"Name : x\nVersion : 2.0.0\n";
        assert_eq!(parse_pacman_info(fixture).unwrap(), "2.0.0");
    }

    #[test]
    fn missing_version_is_parse_failed() {
        let fixture = b"Name : x\n";
        assert_eq!(
            parse_pacman_info(fixture).unwrap_err(),
            BackendError::ParseFailed
        );
    }
}
