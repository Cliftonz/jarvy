//! Homebrew freshness backend.
//!
//! Uses `brew info --json=v2 <formula>` which:
//! - Returns machine-parseable JSON (stable schema Homebrew ships).
//! - Consults the local formula cache first; only hits the network
//!   when the formula isn't in the tap cache. Cheap for the common
//!   case of tools already tapped.
//! - Reports the *upstream stable* version via
//!   `formulae[0].versions.stable`, ignoring any local revision
//!   pinning we don't want influencing "is a newer version out?"

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct BrewBackend;

impl FreshnessBackend for BrewBackend {
    fn name(&self) -> &'static str {
        "brew"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe = crate::tools::common::probe_with_timeout(
            "brew",
            &["info", "--json=v2", pkg_id],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // brew emits "No available formula with the name" for
            // typos / unknown packages; distinguish from other
            // failure modes so telemetry is actionable.
            if stderr.contains("No available formula") || stderr.contains("No formulae found") {
                return Err(BackendError::NotFound);
            }
            return Err(BackendError::Other);
        }
        parse_brew_info(&out.stdout)
    }
}

fn parse_brew_info(stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| BackendError::ParseFailed)?;
    // brew info --json=v2 shape:
    //   { "formulae": [ { "versions": { "stable": "X.Y.Z", ... }, ... } ], "casks": [...] }
    // Cask lookups populate `casks[0].version` instead.
    let stable = value
        .get("formulae")
        .and_then(|v| v.get(0))
        .and_then(|f| f.get("versions"))
        .and_then(|v| v.get("stable"))
        .and_then(|s| s.as_str());
    if let Some(v) = stable {
        return Ok(v.to_string());
    }
    let cask = value
        .get("casks")
        .and_then(|v| v.get(0))
        .and_then(|c| c.get("version"))
        .and_then(|s| s.as_str());
    match cask {
        Some(v) => Ok(v.to_string()),
        None => Err(BackendError::ParseFailed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMULA_FIXTURE: &str = r#"{
        "formulae": [
            {
                "name": "jq",
                "versions": {
                    "stable": "1.7.1",
                    "head": "HEAD",
                    "bottle": true
                }
            }
        ],
        "casks": []
    }"#;

    const CASK_FIXTURE: &str = r#"{
        "formulae": [],
        "casks": [
            {
                "token": "iterm2",
                "version": "3.5.4"
            }
        ]
    }"#;

    #[test]
    fn parses_formula_stable_version() {
        assert_eq!(
            parse_brew_info(FORMULA_FIXTURE.as_bytes()).unwrap(),
            "1.7.1"
        );
    }

    #[test]
    fn parses_cask_version() {
        assert_eq!(parse_brew_info(CASK_FIXTURE.as_bytes()).unwrap(), "3.5.4");
    }

    #[test]
    fn invalid_json_is_parse_failed() {
        assert_eq!(
            parse_brew_info(b"not-json").unwrap_err(),
            BackendError::ParseFailed
        );
    }

    #[test]
    fn empty_results_is_parse_failed() {
        let empty = br#"{"formulae":[],"casks":[]}"#;
        assert_eq!(
            parse_brew_info(empty).unwrap_err(),
            BackendError::ParseFailed
        );
    }
}
