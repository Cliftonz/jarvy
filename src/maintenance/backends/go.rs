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
//!
//! `go list -m` takes MODULE paths, but receipt-routed fallback
//! tools carry PACKAGE paths (`<module>/cmd/<bin>`, e.g.
//! `github.com/doitintl/kube-no-trouble/cmd/kubent`). Probing a
//! package path errors, which used to dump those tools in the
//! error bucket. `module_candidate` lexically truncates at the
//! `/cmd/` layout-convention boundary and probes the module
//! first; a `NotFound` there retries the full path in case a
//! module genuinely contains `/cmd/` in its own path.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct GoBackend;

impl FreshnessBackend for GoBackend {
    fn name(&self) -> &'static str {
        "go"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        match module_candidate(pkg_id) {
            Some(module) => match probe_module(module) {
                Err(BackendError::NotFound) => probe_module(pkg_id),
                other => other,
            },
            None => probe_module(pkg_id),
        }
    }
}

fn probe_module(module: &str) -> Result<String, BackendError> {
    let arg = format!("{module}@latest");
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

/// Lexical module-path guess for a `<module>/cmd/<bin>` package
/// path: everything before the first `/cmd/` segment, kept only
/// when it still has the `host/org/repo` minimum of 3 segments.
fn module_candidate(pkg_id: &str) -> Option<&str> {
    let idx = pkg_id.find("/cmd/")?;
    let prefix = &pkg_id[..idx];
    (prefix.split('/').count() >= 3).then_some(prefix)
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

    #[test]
    fn module_candidate_truncates_at_cmd_boundary() {
        assert_eq!(
            module_candidate("github.com/doitintl/kube-no-trouble/cmd/kubent"),
            Some("github.com/doitintl/kube-no-trouble")
        );
        assert_eq!(
            module_candidate("github.com/argoproj/argo-workflows/v4/cmd/argo"),
            Some("github.com/argoproj/argo-workflows/v4")
        );
    }

    #[test]
    fn module_candidate_passes_module_roots_through() {
        assert_eq!(
            module_candidate("github.com/deviceinsight/kafkactl/v5"),
            None
        );
        assert_eq!(module_candidate("github.com/nats-io/nats-server/v2"), None);
    }

    #[test]
    fn module_candidate_refuses_short_prefixes() {
        // Prefix below host/org/repo can't be a module path.
        assert_eq!(module_candidate("github.com/cmd/foo"), None);
        assert_eq!(module_candidate("cmd/foo"), None);
    }

    #[test]
    fn module_candidate_cuts_at_first_cmd_segment() {
        assert_eq!(
            module_candidate("github.com/x/y/cmd/a/cmd/b"),
            Some("github.com/x/y")
        );
    }
}
