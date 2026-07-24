//! RubyGems freshness backend via `gem search -re`.
//!
//! `-r` searches remote gems, `-e` restricts to an exact-name
//! match. Output shape:
//!
//! ```text
//! *** REMOTE GEMS ***
//!
//! rails (7.1.3.2)
//! ```
//!
//! The first `<pkg> (<ver>)` line under the REMOTE header is the
//! newest published release.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct GemBackend;

impl FreshnessBackend for GemBackend {
    fn name(&self) -> &'static str {
        "gem"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe = crate::tools::common::probe_with_timeout(
            "gem",
            &["search", "-re", pkg_id],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            return Err(BackendError::Other);
        }
        parse_gem_search(pkg_id, &out.stdout)
    }
}

fn parse_gem_search(pkg_id: &str, stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("***") {
            continue;
        }
        let Some((name, rest)) = line.split_once(' ') else {
            continue;
        };
        if !name.eq_ignore_ascii_case(pkg_id) {
            continue;
        }
        let rest = rest.trim();
        // `(1.2.3, 1.2.2, ...)` — take the first entry.
        let inner = rest
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or(BackendError::ParseFailed)?;
        let first = inner.split(',').next().map(str::trim).unwrap_or("");
        if first.is_empty() {
            return Err(BackendError::ParseFailed);
        }
        return Ok(first.to_string());
    }
    Err(BackendError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_version() {
        let out = b"\n*** REMOTE GEMS ***\n\nrails (7.1.3.2)\n";
        assert_eq!(parse_gem_search("rails", out).unwrap(), "7.1.3.2");
    }

    #[test]
    fn parses_multi_version_and_picks_first() {
        let out = b"*** REMOTE GEMS ***\n\nrails (7.1.3.2, 7.1.3.1, 7.1.3)\n";
        assert_eq!(parse_gem_search("rails", out).unwrap(), "7.1.3.2");
    }

    #[test]
    fn wrong_name_is_not_found() {
        let out = b"*** REMOTE GEMS ***\n\nfoo (1.0.0)\n";
        assert_eq!(
            parse_gem_search("rails", out).unwrap_err(),
            BackendError::NotFound
        );
    }
}
