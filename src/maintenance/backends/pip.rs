//! PyPI freshness backend via `pip index versions`.
//!
//! `pip index versions <pkg>` (pip ≥ 21.2) output shape:
//!
//! ```text
//! <pkg> (X.Y.Z)
//! Available versions: X.Y.Z, X.Y.Z-1, ...
//!   INSTALLED: X.Y.Z
//!   LATEST:    X.Y.Z
//! ```
//!
//! The first-line `(<ver>)` is the newest release. On failure we
//! fall back to parsing the `Available versions:` line to be
//! resilient to pip layout drift between minor versions.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct PipBackend;

impl FreshnessBackend for PipBackend {
    fn name(&self) -> &'static str {
        "pip"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe = crate::tools::common::probe_with_timeout(
            "pip",
            &["index", "versions", pkg_id, "--disable-pip-version-check"],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("No matching distribution") || stderr.contains("ERROR: No matching")
            {
                return Err(BackendError::NotFound);
            }
            return Err(BackendError::Other);
        }
        parse_pip_index(pkg_id, &out.stdout)
    }
}

fn parse_pip_index(pkg_id: &str, stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    if text.trim().is_empty() {
        return Err(BackendError::NotFound);
    }
    // Preferred path: first line `<pkg> (X.Y.Z)`.
    if let Some(first) = text.lines().next()
        && let Some((name, rest)) = first.split_once(' ')
        && name.eq_ignore_ascii_case(pkg_id)
        && let Some(v) = rest
            .trim()
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
        && !v.is_empty()
    {
        return Ok(v.to_string());
    }
    // Fallback: `Available versions: X, Y, Z` — first entry is
    // the newest release.
    for line in text.lines() {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix("Available versions:") else {
            continue;
        };
        let head = rest.split(',').next().map(str::trim).unwrap_or("");
        if !head.is_empty() {
            return Ok(head.to_string());
        }
    }
    Err(BackendError::ParseFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_first_line_shape() {
        let out =
            b"black (24.4.2)\nAvailable versions: 24.4.2, 24.4.1\n  INSTALLED: 24.4.2\n  LATEST: 24.4.2\n";
        assert_eq!(parse_pip_index("black", out).unwrap(), "24.4.2");
    }

    #[test]
    fn parses_available_versions_fallback() {
        let out = b"Available versions: 3.0.0, 2.9.0\n";
        assert_eq!(parse_pip_index("black", out).unwrap(), "3.0.0");
    }

    #[test]
    fn empty_output_is_not_found() {
        assert_eq!(
            parse_pip_index("black", b"").unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn missing_shape_is_parse_failed() {
        let out = b"weird pip output that we don't understand\n";
        assert_eq!(
            parse_pip_index("black", out).unwrap_err(),
            BackendError::ParseFailed
        );
    }
}
