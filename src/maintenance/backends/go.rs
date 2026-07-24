//! Go module freshness backend via `go list -m -versions -json`.
//!
//! `go list -m -versions -json <module>@latest` prints JSON with:
//!
//! ```json
//! { "Path": "...", "Version": "v1.2.3", "Versions": ["v1.0.0", "v1.1.0", "v1.2.3"] }
//! ```
//!
//! We prefer the `Version` field (Go's own "latest" resolution)
//! and fall back to the last entry of `Versions` when it's
//! absent. `v`-prefixes are stripped so the value compares as
//! plain semver.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct GoBackend;

impl FreshnessBackend for GoBackend {
    fn name(&self) -> &'static str {
        "go"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let arg = format!("{pkg_id}@latest");
        let probe = crate::tools::common::probe_with_timeout(
            "go",
            &["list", "-m", "-versions", "-json", &arg],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("no matching versions")
                || stderr.contains("module not found")
                || stderr.contains("no required module provides package")
            {
                return Err(BackendError::NotFound);
            }
            return Err(BackendError::Other);
        }
        parse_go_list(&out.stdout)
    }
}

fn parse_go_list(stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    if text.trim().is_empty() {
        return Err(BackendError::NotFound);
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| BackendError::ParseFailed)?;
    if let Some(v) = value.get("Version").and_then(|v| v.as_str()) {
        return Ok(strip_v_prefix(v));
    }
    if let Some(arr) = value.get("Versions").and_then(|v| v.as_array())
        && let Some(last) = arr.last().and_then(|v| v.as_str())
    {
        return Ok(strip_v_prefix(last));
    }
    Err(BackendError::ParseFailed)
}

fn strip_v_prefix(v: &str) -> String {
    v.strip_prefix('v').unwrap_or(v).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_field() {
        let out = br#"{"Path":"golang.org/x/tools","Version":"v0.20.0","Versions":["v0.18.0","v0.19.0","v0.20.0"]}"#;
        assert_eq!(parse_go_list(out).unwrap(), "0.20.0");
    }

    #[test]
    fn falls_back_to_versions_array() {
        let out = br#"{"Path":"foo","Versions":["v1.0.0","v1.2.0"]}"#;
        assert_eq!(parse_go_list(out).unwrap(), "1.2.0");
    }

    #[test]
    fn empty_output_is_not_found() {
        assert_eq!(parse_go_list(b"").unwrap_err(), BackendError::NotFound);
    }

    #[test]
    fn missing_both_fields_is_parse_failed() {
        let out = br#"{"Path":"foo"}"#;
        assert_eq!(parse_go_list(out).unwrap_err(), BackendError::ParseFailed);
    }
}
