//! npm registry freshness backend via `npm view`.
//!
//! `npm view <pkg> version --json` returns a bare JSON string
//! (e.g. `"1.2.3"`). Also handles the multi-hit shape `["1.2.3"]`
//! that some scopes return.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct NpmBackend;

impl FreshnessBackend for NpmBackend {
    fn name(&self) -> &'static str {
        "npm"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe = crate::tools::common::probe_with_timeout(
            "npm",
            &["view", pkg_id, "version", "--json"],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("404") || stderr.contains("Not found") {
                return Err(BackendError::NotFound);
            }
            return Err(BackendError::Other);
        }
        parse_npm_view(&out.stdout)
    }
}

fn parse_npm_view(stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(BackendError::NotFound);
    }
    let value: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|_| BackendError::ParseFailed)?;
    if let Some(s) = value.as_str() {
        return Ok(s.to_string());
    }
    if let Some(arr) = value.as_array() {
        // Multi-hit returns every version in publish order; last
        // entry is the newest release (mirrors `npm dist-tag ls`).
        if let Some(last) = arr.last().and_then(|v| v.as_str()) {
            return Ok(last.to_string());
        }
    }
    Err(BackendError::ParseFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_string_version() {
        assert_eq!(parse_npm_view(b"\"18.2.0\"\n").unwrap(), "18.2.0");
    }

    #[test]
    fn parses_array_and_picks_last() {
        let out = br#"["17.0.0","18.1.0","18.2.0"]"#;
        assert_eq!(parse_npm_view(out).unwrap(), "18.2.0");
    }

    #[test]
    fn empty_output_is_not_found() {
        assert_eq!(parse_npm_view(b"\n").unwrap_err(), BackendError::NotFound);
    }

    #[test]
    fn malformed_json_is_parse_failed() {
        assert_eq!(parse_npm_view(b"{").unwrap_err(), BackendError::ParseFailed);
    }
}
