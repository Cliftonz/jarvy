//! uv freshness backend (PRD-060 Phase 2).
//!
//! Serves receipt-routed tools installed via `uv tool install`. The
//! latest-version probe is the PyPI JSON API
//! (`https://pypi.org/pypi/<pkg>/json` → `info.version`) over
//! `net::bounded_fetch` — `uv tool upgrade --dry-run` output is not a
//! stable contract (DECIDED, PRD-060), so this backend is the one
//! deliberate exception to this module's "no new HTTP surface" rule.
//! HTTPS-only, bounded read; no subprocess, no uv CLI requirement for
//! the probe itself.

use crate::net::bounded_fetch::{BoundedFetchConfig, BoundedFetchErrorKind, bounded_fetch};
use crate::net::url_encode::encode_unreserved;

use super::{BackendError, FreshnessBackend};

/// PyPI project docs list every release; large projects run multiple
/// MB of JSON. 20 MiB is comfortably above anything real without
/// letting a hostile mirror stream unbounded.
const PYPI_MAX_BYTES: u64 = 20 * 1024 * 1024;

/// Test-isolation loopback bypass (compiled out of release builds via
/// the shared `test-bypass` gating inside `bounded_fetch`).
const INSECURE_FETCH_ENV: &str = "JARVY_PYPI_ALLOW_INSECURE_FETCH";

pub struct UvBackend;

impl FreshnessBackend for UvBackend {
    fn name(&self) -> &'static str {
        "uv"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let url = pypi_url(pkg_id);
        let body = bounded_fetch(
            &url,
            PYPI_MAX_BYTES,
            BoundedFetchConfig {
                insecure_loopback_env: INSECURE_FETCH_ENV,
            },
        )
        .map_err(map_fetch_err)?;
        parse_pypi_latest(&body).ok_or(BackendError::ParseFailed)
    }
}

fn pypi_url(pkg_id: &str) -> String {
    format!("https://pypi.org/pypi/{}/json", encode_unreserved(pkg_id))
}

fn map_fetch_err(e: BoundedFetchErrorKind) -> BackendError {
    match e {
        BoundedFetchErrorKind::HttpStatus(404) => BackendError::NotFound,
        BoundedFetchErrorKind::Network(_)
        | BoundedFetchErrorKind::HttpStatus(_)
        | BoundedFetchErrorKind::TooLarge
        | BoundedFetchErrorKind::Read(_)
        | BoundedFetchErrorKind::NonHttps => BackendError::Other,
    }
}

/// Pull `info.version` out of a PyPI project JSON document.
fn parse_pypi_latest(body: &[u8]) -> Option<String> {
    let doc: serde_json::Value = serde_json::from_slice(body).ok()?;
    let version = doc.get("info")?.get("version")?.as_str()?;
    if version.is_empty() {
        return None;
    }
    Some(version.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_info_version() {
        let body = br#"{"info":{"name":"cfn-lint","version":"1.18.4"},"releases":{}}"#;
        assert_eq!(parse_pypi_latest(body).as_deref(), Some("1.18.4"));
    }

    #[test]
    fn rejects_missing_or_empty_version() {
        assert_eq!(parse_pypi_latest(br#"{"info":{}}"#), None);
        assert_eq!(parse_pypi_latest(br#"{"info":{"version":""}}"#), None);
        assert_eq!(parse_pypi_latest(br#"{}"#), None);
        assert_eq!(parse_pypi_latest(b"not json"), None);
    }

    #[test]
    fn url_encodes_package_id() {
        assert_eq!(pypi_url("cfn-lint"), "https://pypi.org/pypi/cfn-lint/json");
        assert_eq!(pypi_url("a/b?c"), "https://pypi.org/pypi/a%2Fb%3Fc/json");
    }

    #[test]
    fn http_404_maps_to_not_found() {
        assert_eq!(
            map_fetch_err(BoundedFetchErrorKind::HttpStatus(404)),
            BackendError::NotFound
        );
        assert_eq!(
            map_fetch_err(BoundedFetchErrorKind::HttpStatus(500)),
            BackendError::Other
        );
        assert_eq!(
            map_fetch_err(BoundedFetchErrorKind::NonHttps),
            BackendError::Other
        );
    }
}
