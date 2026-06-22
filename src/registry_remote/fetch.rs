//! HTTPS fetch helpers for registry sync.
//!
//! Routes through the shared `crate::net::agent` so we inherit the
//! process-wide timeout policy + zero-redirect default + sane User-Agent.
//! Adds:
//!
//! - **HTTPS-only refusal**: refuses non-`https://` URLs at the entry
//!   point so a typo in `[registry] url` can't downgrade to plaintext.
//! - **Bounded response read**: each kind of artifact (manifest / tool /
//!   sig) has its own size cap. Defaults are generous but guard against
//!   accidental DoS from a misbehaving registry.

use std::io::Read;
use thiserror::Error;

/// Manifest response cap. Registries with more than a few thousand tools
/// can lift this but the default protects against accidental DoS.
pub const MAX_MANIFEST_BYTES: u64 = 5 * 1024 * 1024;

/// Tool-TOML response cap. Per-tool definitions are tiny in practice
/// (~1 KB) so 1 MiB is generous.
pub const MAX_TOOL_BYTES: u64 = 1024 * 1024;

/// Cosign sig/cert companions are tiny.
pub const MAX_SIG_BYTES: u64 = 64 * 1024;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("fetch failed for {url}: {message}")]
    Network { url: String, message: String },
    #[error("fetch returned HTTP {status} for {url}")]
    HttpStatus { url: String, status: u16 },
    #[error("response body too large for {url}: capped at {cap} bytes")]
    TooLarge { url: String, cap: u64 },
    #[error("read error for {url}: {source}")]
    Read {
        url: String,
        #[source]
        source: std::io::Error,
    },
    #[error("non-https url refused: {0}")]
    NonHttps(String),
}

/// Fetch a URL into a bounded byte buffer. Refuses non-HTTPS URLs
/// unless the loopback-test bypass is active (see
/// [`insecure_loopback_allowed`]).
pub fn fetch_bounded(url: &str, max_bytes: u64) -> Result<Vec<u8>, FetchError> {
    if !url.starts_with("https://") && !insecure_loopback_allowed(url) {
        return Err(FetchError::NonHttps(url.to_string()));
    }

    let agent = crate::net::agent::agent();
    let response = agent
        .get(url)
        .header("User-Agent", crate::net::agent::USER_AGENT)
        .call()
        .map_err(|e| FetchError::Network {
            url: url.to_string(),
            message: e.to_string(),
        })?;

    if response.status() != 200 {
        return Err(FetchError::HttpStatus {
            url: url.to_string(),
            status: response.status().as_u16(),
        });
    }

    let mut body = response.into_body();
    let reader = body.as_reader();
    // Take(max_bytes + 1) so we can distinguish exact-fit from overflow.
    let mut limited = reader.take(max_bytes + 1);
    let mut buf = Vec::with_capacity(8 * 1024);
    limited
        .read_to_end(&mut buf)
        .map_err(|e| FetchError::Read {
            url: url.to_string(),
            source: e,
        })?;

    if buf.len() as u64 > max_bytes {
        return Err(FetchError::TooLarge {
            url: url.to_string(),
            cap: max_bytes,
        });
    }

    Ok(buf)
}

/// Loopback-only escape hatch for integration tests. The CLI ships an
/// HTTPS-only fetch policy because a typo in `[registry] url` would
/// otherwise silently downgrade to plaintext. Spinning up a real TLS
/// listener per test would be heavyweight, so tests opt in via env var
/// AND restrict to 127.0.0.1 / localhost URLs. Production users have no
/// way to set this — the env var has no other consumer and the URL
/// guard means even with the env var set, only loopback fetches are
/// allowed. Combined this is far weaker than a config flag (which would
/// be parseable from `~/.jarvy/config.toml`) yet sufficient for the
/// integration-test harness.
fn insecure_loopback_allowed(url: &str) -> bool {
    if std::env::var_os("JARVY_REGISTRY_ALLOW_INSECURE_FETCH").is_none() {
        return false;
    }
    url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")
}

#[cfg(test)]
#[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial(registry_env)]
    fn refuses_http_url() {
        // SAFETY: serial-test gate (`registry_env` group) ensures no other
        // env-mutating test in this group runs concurrently.
        unsafe {
            std::env::remove_var("JARVY_REGISTRY_ALLOW_INSECURE_FETCH");
        }
        let err = fetch_bounded("http://example.com/x", 1024).unwrap_err();
        assert!(matches!(err, FetchError::NonHttps(_)));
    }

    #[test]
    #[serial(registry_env)]
    fn refuses_ftp_url() {
        // SAFETY: serialized via #[serial(registry_env)].
        unsafe {
            std::env::remove_var("JARVY_REGISTRY_ALLOW_INSECURE_FETCH");
        }
        let err = fetch_bounded("ftp://example.com/x", 1024).unwrap_err();
        assert!(matches!(err, FetchError::NonHttps(_)));
    }

    #[test]
    #[serial(registry_env)]
    fn refuses_non_loopback_even_with_env() {
        // Bypass requires BOTH the env var AND a loopback URL.
        // SAFETY: serialized via #[serial(registry_env)].
        unsafe {
            std::env::set_var("JARVY_REGISTRY_ALLOW_INSECURE_FETCH", "1");
        }
        let err = fetch_bounded("http://attacker.example/x", 1024).unwrap_err();
        assert!(matches!(err, FetchError::NonHttps(_)));
        // SAFETY: same.
        unsafe {
            std::env::remove_var("JARVY_REGISTRY_ALLOW_INSECURE_FETCH");
        }
    }
}
