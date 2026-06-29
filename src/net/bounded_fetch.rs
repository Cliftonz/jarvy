//! Shared HTTPS-only bounded fetch (maint P1, review item 18).
//!
//! Previously `library_registry::fetch` and `registry_remote::fetch`
//! each carried a copy of the same HTTPS-refusal, loopback-bypass, and
//! bounded-read loop. The two error types stay distinct because
//! downstream callers match on them and each ecosystem wraps with its
//! own redaction policy, but the body of the fetch is now one place.

use std::io::Read;

/// Reason a `bounded_fetch` call failed. Each consumer adds the (already
/// redacted) URL into its own enum so error messages match what callers
/// previously printed verbatim.
#[derive(Debug)]
pub enum BoundedFetchErrorKind {
    Network(String),
    HttpStatus(u16),
    TooLarge,
    Read(std::io::Error),
    NonHttps,
}

/// Per-call config knob — which env var enables the loopback bypass.
/// Each ecosystem keeps its own env var so test isolation stays per
/// feature (otherwise setting one would silently allow plaintext fetch
/// for both subsystems).
#[derive(Debug, Clone, Copy)]
pub struct BoundedFetchConfig {
    pub insecure_loopback_env: &'static str,
}

/// Fetch a URL into a bounded byte buffer. Refuses non-HTTPS URLs
/// unless the configured loopback bypass is active (env var set AND URL
/// is plain loopback).
pub fn bounded_fetch(
    url: &str,
    max_bytes: u64,
    cfg: BoundedFetchConfig,
) -> Result<Vec<u8>, BoundedFetchErrorKind> {
    if !url.starts_with("https://") && !insecure_loopback_allowed(url, cfg.insecure_loopback_env) {
        return Err(BoundedFetchErrorKind::NonHttps);
    }

    let agent = crate::net::agent::agent();
    let response = agent
        .get(url)
        .header("User-Agent", crate::net::agent::USER_AGENT)
        .call()
        .map_err(|e| BoundedFetchErrorKind::Network(e.to_string()))?;

    if response.status() != 200 {
        return Err(BoundedFetchErrorKind::HttpStatus(
            response.status().as_u16(),
        ));
    }

    let mut body = response.into_body();
    let reader = body.as_reader();
    // Take(max_bytes + 1) so we can distinguish exact-fit from overflow.
    let mut limited = reader.take(max_bytes + 1);
    let mut buf = Vec::with_capacity(8 * 1024);
    limited
        .read_to_end(&mut buf)
        .map_err(BoundedFetchErrorKind::Read)?;

    if buf.len() as u64 > max_bytes {
        return Err(BoundedFetchErrorKind::TooLarge);
    }

    Ok(buf)
}

fn insecure_loopback_allowed(url: &str, env_var: &'static str) -> bool {
    // Hard refusal in release builds — the `test-bypass` Cargo feature
    // gates the entire escape hatch out of shipped binaries (review
    // item 15). Without the feature compiled in, the env var is inert.
    #[cfg(not(feature = "test-bypass"))]
    {
        let _ = (url, env_var);
        false
    }
    #[cfg(feature = "test-bypass")]
    {
        if std::env::var_os(env_var).is_none() {
            return false;
        }
        is_plain_loopback_http(url)
    }
}

/// True iff `url` is `http://<loopback-host>[:port]/...` with NO
/// `userinfo@` segment. Byte-prefix matching the serialized form is
/// not enough: `http://127.0.0.1:80@attacker.example/` parses with
/// `127.0.0.1:80` as USERINFO and `attacker.example` as host (RFC 3986).
///
/// `pub(crate)` so consumers cannot reach in and call the parser
/// directly — they MUST go through `bounded_fetch` which also requires
/// the env-var consent gate. Exposed module-wide for the local tests
/// below. Gated behind `test-bypass` because the only legitimate
/// caller is `insecure_loopback_allowed`, which is itself gated.
#[cfg(any(feature = "test-bypass", test))]
pub(crate) fn is_plain_loopback_http(url: &str) -> bool {
    let Some(after_scheme) = url.strip_prefix("http://") else {
        return false;
    };
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let authority = &after_scheme[..authority_end];
    if authority.contains('@') {
        return false;
    }
    let host_end = authority.find(':').unwrap_or(authority.len());
    let host = &authority[..host_end];
    matches!(host, "127.0.0.1" | "localhost")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_accepts_clean_loopback() {
        assert!(is_plain_loopback_http("http://127.0.0.1:8080/x"));
        assert!(is_plain_loopback_http("http://localhost:8080/x"));
        assert!(is_plain_loopback_http("http://127.0.0.1/"));
    }

    #[test]
    fn parser_refuses_userinfo_bypass() {
        assert!(!is_plain_loopback_http(
            "http://127.0.0.1:80@attacker.example/x"
        ));
        assert!(!is_plain_loopback_http("http://user@127.0.0.1:8080/x"));
        assert!(!is_plain_loopback_http(
            "http://localhost@attacker.example/x"
        ));
    }
}
