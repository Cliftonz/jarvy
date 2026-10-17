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

use crate::net::bounded_fetch::{BoundedFetchConfig, BoundedFetchErrorKind, bounded_fetch};
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

/// Fetch a URL into a bounded byte buffer. Refuses non-HTTPS URLs unless
/// the loopback-test bypass (env `JARVY_REGISTRY_ALLOW_INSECURE_FETCH`
/// plus a loopback host) is active. Delegates to `net::bounded_fetch` so
/// the HTTPS-only refusal, bounded-read, and loopback-parser logic stays
/// in exactly one place (maint P1, review item 18).
pub fn fetch_bounded(url: &str, max_bytes: u64) -> Result<Vec<u8>, FetchError> {
    let cfg = BoundedFetchConfig {
        insecure_loopback_env: "JARVY_REGISTRY_ALLOW_INSECURE_FETCH",
    };
    bounded_fetch(url, max_bytes, cfg).map_err(|kind| {
        let redacted = crate::network::redact_userinfo(url).into_owned();
        match kind {
            BoundedFetchErrorKind::NonHttps => FetchError::NonHttps(redacted),
            BoundedFetchErrorKind::Network(message) => FetchError::Network {
                url: redacted,
                message,
            },
            BoundedFetchErrorKind::HttpStatus(status) => FetchError::HttpStatus {
                url: redacted,
                status,
            },
            BoundedFetchErrorKind::TooLarge => FetchError::TooLarge {
                url: redacted,
                cap: max_bytes,
            },
            BoundedFetchErrorKind::Read(source) => FetchError::Read {
                url: redacted,
                source,
            },
        }
    })
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

    /// Userinfo bypass: `http://127.0.0.1:80@attacker.example/x` parses
    /// (per RFC 3986) with `127.0.0.1:80` as USERINFO and `attacker.example`
    /// as host. Pre-fix byte-prefix matching accepted this; the
    /// post-fix authority parser refuses anything with `@`.
    #[test]
    #[serial(registry_env)]
    fn refuses_userinfo_authority_bypass() {
        // SAFETY: serialized via #[serial(registry_env)].
        unsafe {
            std::env::set_var("JARVY_REGISTRY_ALLOW_INSECURE_FETCH", "1");
        }
        for url in [
            "http://127.0.0.1:80@attacker.example/x",
            "http://localhost:80@attacker.example/x",
            "http://127.0.0.1@attacker.example/x",
            "http://user:pass@127.0.0.1:8080/x",
        ] {
            let err = fetch_bounded(url, 1024).unwrap_err();
            assert!(
                matches!(err, FetchError::NonHttps(_)),
                "must refuse userinfo-bearing URL {url:?}"
            );
        }
        // SAFETY: same.
        unsafe {
            std::env::remove_var("JARVY_REGISTRY_ALLOW_INSECURE_FETCH");
        }
    }

    // is_plain_loopback_http parser tests now live in
    // `crate::net::bounded_fetch::tests` — the function moved there when
    // the fetch loop was consolidated.
}
