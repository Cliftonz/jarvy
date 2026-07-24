#![allow(dead_code)] // Windows-only backend; parsers reachable via tests on all OSes.

//! Chocolatey freshness backend via `choco search --exact
//! --limit-output`.
//!
//! `--limit-output` renders one `pkg|version` line per hit — no
//! decorations, stable since choco 0.10.
//!
//! We treat non-matching first-column ids as `NotFound` rather
//! than a parse error so a `choco search` that returns unrelated
//! partial matches (rare with `--exact` but seen historically) is
//! still classified cleanly.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct ChocoBackend;

impl FreshnessBackend for ChocoBackend {
    fn name(&self) -> &'static str {
        "choco"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe = crate::tools::common::probe_with_timeout(
            "choco",
            &["search", pkg_id, "--exact", "--limit-output"],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            return Err(BackendError::Other);
        }
        parse_choco_search(pkg_id, &out.stdout)
    }
}

fn parse_choco_search(pkg_id: &str, stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, version)) = line.split_once('|') else {
            continue;
        };
        if !name.eq_ignore_ascii_case(pkg_id) {
            continue;
        }
        let v = version.trim();
        if v.is_empty() {
            return Err(BackendError::ParseFailed);
        }
        return Ok(v.to_string());
    }
    Err(BackendError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_match() {
        let out = b"git|2.45.2\n";
        assert_eq!(parse_choco_search("git", out).unwrap(), "2.45.2");
    }

    #[test]
    fn wrong_name_is_not_found() {
        let out = b"git|2.45.2\n";
        assert_eq!(
            parse_choco_search("nodejs", out).unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn case_insensitive_match() {
        let out = b"Git|2.45.2\n";
        assert_eq!(parse_choco_search("git", out).unwrap(), "2.45.2");
    }

    #[test]
    fn empty_output_is_not_found() {
        assert_eq!(
            parse_choco_search("git", b"").unwrap_err(),
            BackendError::NotFound
        );
    }
}
