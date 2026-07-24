// Backend is routed only on Linux; parsers keep their tests
// cross-platform, which means the struct + helpers look
// "unused" from any macOS/Windows compile pass. Suppress the
// dead-code lint at module scope rather than fan out per-item
// `#[allow]`s that drift the moment a new helper lands.
#![allow(dead_code)]

//! apt freshness backend via `apt-cache policy`.
//!
//! `apt-cache policy <pkg>` output shape (Debian/Ubuntu, stable
//! since apt 0.7):
//!
//! ```text
//! <pkg>:
//!   Installed: 2.44.0-1
//!   Candidate: 2.45.0-1
//!   Version table:
//!     ...
//! ```
//!
//! We parse the `Candidate:` line — that's what `apt-get install`
//! would resolve to. Debian version strings frequently carry
//! epoch (`1:`) and revision (`-1ubuntu1`) suffixes; we strip
//! them so downstream semver compare against a plain upstream
//! version doesn't misclassify as `Direction::Downgrade`.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct AptBackend;

impl FreshnessBackend for AptBackend {
    fn name(&self) -> &'static str {
        "apt"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe = crate::tools::common::probe_with_timeout(
            "apt-cache",
            &["policy", pkg_id],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            return Err(BackendError::Other);
        }
        parse_apt_policy(&out.stdout)
    }
}

fn parse_apt_policy(stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    // Empty stdout is apt's canonical "no such package" response —
    // apt-cache exits 0 with nothing on stdout when the package is
    // unknown.
    if text.trim().is_empty() {
        return Err(BackendError::NotFound);
    }
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("Candidate:") else {
            continue;
        };
        let rest = rest.trim();
        if rest.is_empty() || rest == "(none)" {
            return Err(BackendError::NotFound);
        }
        return Ok(normalize_debian_version(rest));
    }
    Err(BackendError::ParseFailed)
}

/// Strip Debian epoch (`1:`) and revision (`-1ubuntu1`) so the
/// version string compares cleanly against upstream semver. The
/// full Debian version stays in the raw report if a caller wants
/// it; we're optimizing the shape for the semver compare in
/// [`crate::maintenance::checker::compare_versions`].
fn normalize_debian_version(v: &str) -> String {
    let without_epoch = match v.split_once(':') {
        Some((prefix, rest)) if prefix.chars().all(|c| c.is_ascii_digit()) => rest,
        _ => v,
    };
    // Take everything before the first '-' (Debian revision).
    match without_epoch.split_once('-') {
        Some((upstream, _rev)) => upstream.to_string(),
        None => without_epoch.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY_FIXTURE: &[u8] = b"jq:\n  Installed: 1.6-2.1ubuntu3\n  Candidate: 1.7.1-1\n  Version table:\n     1.7.1-1 500\n        500 http://archive.ubuntu.com/ubuntu jammy/main amd64 Packages\n";

    #[test]
    fn parses_candidate_line() {
        assert_eq!(parse_apt_policy(POLICY_FIXTURE).unwrap(), "1.7.1");
    }

    #[test]
    fn strips_epoch_prefix() {
        let fixture = b"pkg:\n  Candidate: 2:24.0.7-0ubuntu1\n";
        assert_eq!(parse_apt_policy(fixture).unwrap(), "24.0.7");
    }

    #[test]
    fn candidate_none_is_not_found() {
        let fixture = b"pkg:\n  Installed: (none)\n  Candidate: (none)\n";
        assert_eq!(
            parse_apt_policy(fixture).unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn empty_output_is_not_found() {
        assert_eq!(parse_apt_policy(b"").unwrap_err(), BackendError::NotFound);
    }

    #[test]
    fn missing_candidate_line_is_parse_failed() {
        let fixture = b"pkg:\n  Installed: 1.0.0\n";
        assert_eq!(
            parse_apt_policy(fixture).unwrap_err(),
            BackendError::ParseFailed
        );
    }
}
