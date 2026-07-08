//! Companion-artifact fetch (PRD-054 follow-up phase).
//!
//! Library manifest items may reference bodies by URL instead of
//! inlining them: `ai_hook` items via `bash_url` / `powershell_url`,
//! `skill` items via `companion_files`. Every companion reference
//! MUST carry a manifest-pinned sha256 — there is no unverified
//! fetch path.
//!
//! # Trust model
//!
//! - Same HTTPS-only bounded fetch as manifests (`fetch::fetch_bounded`,
//!   capped at [`fetch::MAX_ITEM_BYTES`]).
//! - sha256 verified against the manifest pin before the body is
//!   handed to any consumer. Mismatch is a hard refusal.
//! - `file://` URLs are honored only inside the library cache root
//!   (git-fetched libraries synthesize them) — containment is
//!   enforced by [`git_fetch::read_file_url`].
//!
//! # Cache
//!
//! Content-addressed disk cache at
//! `~/.jarvy/library.d/companions/<sha256>`. Because the manifest pins
//! the content hash, a cached body is immutable by construction: a
//! cache hit skips the network entirely, and a corrupted cache entry
//! (hash no longer matches) falls through to a re-fetch.

use super::fetch::{self, FetchError};
use super::{cache, git_fetch, sha256_hex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompanionError {
    #[error(
        "companion sha256 pin for {url} is not a 64-char hex digest — \
         refusing to fetch unverifiable content"
    )]
    InvalidShaPin { url: String },

    #[error("fetch failed for companion {url}: {source}")]
    Fetch {
        url: String,
        #[source]
        source: FetchError,
    },

    #[error(
        "sha256 mismatch for companion {url}: manifest pins `{expected}`, \
         fetched body computes `{actual}` — either the publisher re-published \
         without updating the manifest, or the artifact was tampered with in transit"
    )]
    ShaMismatch {
        url: String,
        expected: String,
        actual: String,
    },

    #[error("companion body for {url} is not valid UTF-8 (script bodies must be UTF-8)")]
    NotUtf8 { url: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl CompanionError {
    /// Stable telemetry discriminant.
    pub fn kind(&self) -> &'static str {
        match self {
            CompanionError::InvalidShaPin { .. } => "invalid_sha_pin",
            CompanionError::Fetch { .. } => "fetch",
            CompanionError::ShaMismatch { .. } => "sha_mismatch",
            CompanionError::NotUtf8 { .. } => "not_utf8",
            CompanionError::Io(_) => "io",
        }
    }
}

/// Fetch a companion artifact and verify it against the manifest-pinned
/// sha256. Serves from the content-addressed disk cache when the pinned
/// content is already present; writes through on a successful fetch.
///
/// # Errors
///
/// - [`CompanionError::InvalidShaPin`] — pin is not 64 hex chars (also
///   guards the cache path against injection; the pin becomes a filename).
/// - [`CompanionError::Fetch`] — network / HTTP / non-HTTPS refusal.
/// - [`CompanionError::ShaMismatch`] — body doesn't match the pin.
pub fn fetch_verified(url: &str, expected_sha256: &str) -> Result<Vec<u8>, CompanionError> {
    let telemetry_on = crate::observability::telemetry_gate::is_enabled();
    let redacted = || crate::network::redact_credentials(url).into_owned();

    // Validate the pin BEFORE using it as a cache filename. A malformed
    // pin can never match a real digest, so refuse early with a clear
    // error instead of fetching bytes we could never accept.
    if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        let err = CompanionError::InvalidShaPin { url: redacted() };
        emit_fetch_failed(telemetry_on, &redacted(), &err);
        return Err(err);
    }
    let pin = expected_sha256.to_ascii_lowercase();

    // Content-addressed cache probe. A hit is self-validating: re-hash
    // the cached bytes so a corrupted / truncated cache entry falls
    // through to a fresh fetch instead of poisoning the consumer.
    let cache_path = cache::companion_cache_path(&pin)?;
    if let Ok(cached) = std::fs::read(&cache_path)
        && sha256_hex(&cached) == pin
    {
        if telemetry_on {
            tracing::debug!(
                event = "library.companion.fetched",
                url = %redacted(),
                bytes = cached.len(),
                from_cache = true,
            );
        }
        return Ok(cached);
    }

    // PRD-055: git-fetched libraries synthesize `file://` URLs pointing
    // into the local clone cache. `read_file_url` enforces containment
    // inside the cache root (emits `library.file_url_refused` otherwise).
    let body = if url.starts_with("file://") {
        git_fetch::read_file_url(url).map_err(|e| {
            let err = CompanionError::Fetch {
                url: redacted(),
                source: FetchError::Read {
                    url: redacted(),
                    source: match e {
                        super::LibraryError::Io(io) => io,
                        other => std::io::Error::other(format!("{other}")),
                    },
                },
            };
            emit_fetch_failed(telemetry_on, &redacted(), &err);
            err
        })?
    } else {
        fetch::fetch_bounded(url, fetch::MAX_ITEM_BYTES).map_err(|e| {
            let err = CompanionError::Fetch {
                url: redacted(),
                source: e,
            };
            emit_fetch_failed(telemetry_on, &redacted(), &err);
            err
        })?
    };

    let actual = sha256_hex(&body);
    if actual != pin {
        if telemetry_on {
            tracing::error!(
                event = "library.companion.sha_mismatch",
                url = %redacted(),
                expected = %pin,
                actual = %actual,
            );
        }
        return Err(CompanionError::ShaMismatch {
            url: redacted(),
            expected: pin,
            actual,
        });
    }

    // Write-through cache, best-effort — a full disk must not fail the
    // install. Mirrors the manifest cache's `library.cache.write_failed`.
    if let Err(e) = cache::write_bytes_atomic(&cache_path, &body) {
        tracing::warn!(
            event = "library.cache.write_failed",
            url = %redacted(),
            error = %e,
        );
    }

    if telemetry_on {
        tracing::info!(
            event = "library.companion.fetched",
            url = %redacted(),
            bytes = body.len(),
            from_cache = false,
        );
    }
    Ok(body)
}

/// [`fetch_verified`] for script bodies that must be UTF-8 (hook
/// `bash_url` / `powershell_url` targets).
///
/// # Errors
///
/// Everything [`fetch_verified`] returns, plus
/// [`CompanionError::NotUtf8`] when the verified body isn't UTF-8.
pub fn fetch_verified_utf8(url: &str, expected_sha256: &str) -> Result<String, CompanionError> {
    let body = fetch_verified(url, expected_sha256)?;
    String::from_utf8(body).map_err(|_| CompanionError::NotUtf8 {
        url: crate::network::redact_credentials(url).into_owned(),
    })
}

fn emit_fetch_failed(telemetry_on: bool, url: &str, err: &CompanionError) {
    if telemetry_on {
        tracing::warn!(
            event = "library.companion.fetch_failed",
            url = %url,
            error_kind = err.kind(),
            error = %err,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    /// Point `JARVY_HOME` at a fresh tempdir and return the guard.
    /// Callers are `#[serial(jarvy_home_env)]` so the process-global
    /// mutation can't race other env-touching tests.
    fn isolated_home() -> tempfile::TempDir {
        let home = tempdir().unwrap();
        // SAFETY: serial-test gate ensures no concurrent env access.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_HOME", home.path());
        }
        home
    }

    fn clear_home() {
        // SAFETY: serial-test gate ensures no concurrent env access.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_HOME");
        }
    }

    /// Drop a body into the library cache root and hand back a
    /// containment-safe `file://` URL for it.
    fn seed_cache_root_file(name: &str, body: &[u8]) -> String {
        let root = cache::cache_root().unwrap();
        let path = root.join(name);
        std::fs::write(&path, body).unwrap();
        format!("file://{}", path.canonicalize().unwrap().display())
    }

    #[test]
    #[serial(jarvy_home_env)]
    fn refuses_malformed_sha_pin_without_fetching() {
        let _home = isolated_home();
        let err = fetch_verified("https://cdn.example.com/x.sh", "deadbeef").unwrap_err();
        assert!(matches!(err, CompanionError::InvalidShaPin { .. }));
        // Traversal shapes must never reach the filesystem layer.
        let err = fetch_verified("https://cdn.example.com/x.sh", "../../etc/passwd").unwrap_err();
        assert!(matches!(err, CompanionError::InvalidShaPin { .. }));
        clear_home();
    }

    #[test]
    #[serial(jarvy_home_env)]
    fn refuses_sha_mismatch() {
        let _home = isolated_home();
        let url = seed_cache_root_file("companion-mismatch.sh", b"actual body");
        let wrong_pin = sha256_hex(b"some other body");
        let err = fetch_verified(&url, &wrong_pin).unwrap_err();
        match err {
            CompanionError::ShaMismatch {
                expected, actual, ..
            } => {
                assert_eq!(expected, wrong_pin);
                assert_eq!(actual, sha256_hex(b"actual body"));
            }
            other => panic!("expected ShaMismatch, got {other:?}"),
        }
        clear_home();
    }

    #[test]
    #[serial(jarvy_home_env)]
    fn accepts_matching_sha_case_insensitive_and_caches() {
        let _home = isolated_home();
        let body = b"#!/bin/sh\necho ok\n";
        let url = seed_cache_root_file("companion-ok.sh", body);
        let pin_upper = sha256_hex(body).to_uppercase();
        let fetched = fetch_verified(&url, &pin_upper).expect("matching pin accepts");
        assert_eq!(fetched, body);
        // Write-through cache landed, content-addressed by the pin.
        let cached = cache::companion_cache_path(&pin_upper.to_ascii_lowercase()).unwrap();
        assert_eq!(std::fs::read(&cached).unwrap(), body);
        clear_home();
    }

    #[test]
    #[serial(jarvy_home_env)]
    fn cache_hit_skips_the_source_entirely() {
        let _home = isolated_home();
        let body = b"cached companion body";
        let pin = sha256_hex(body);
        let cache_path = cache::companion_cache_path(&pin).unwrap();
        std::fs::write(&cache_path, body).unwrap();
        // An HTTPS URL that would refuse/fail if actually fetched — a
        // cache hit must return before the network layer is consulted.
        let fetched =
            fetch_verified("https://127.0.0.1:1/unreachable.sh", &pin).expect("cache-first");
        assert_eq!(fetched, body);
        clear_home();
    }

    #[test]
    #[serial(jarvy_home_env)]
    fn corrupted_cache_entry_falls_through_to_refetch() {
        let _home = isolated_home();
        let body = b"real companion body";
        let pin = sha256_hex(body);
        // Poison the content-addressed slot with non-matching bytes.
        let cache_path = cache::companion_cache_path(&pin).unwrap();
        std::fs::write(&cache_path, b"corrupted").unwrap();
        let url = seed_cache_root_file("companion-refetch.sh", body);
        let fetched = fetch_verified(&url, &pin).expect("refetch replaces corrupt cache");
        assert_eq!(fetched, body);
        // Cache slot healed by the write-through.
        assert_eq!(std::fs::read(&cache_path).unwrap(), body);
        clear_home();
    }

    #[test]
    #[serial(jarvy_home_env)]
    fn utf8_helper_refuses_non_utf8_body() {
        let _home = isolated_home();
        let body: &[u8] = &[0xff, 0xfe, 0x00, 0x41];
        let url = seed_cache_root_file("companion-binary.bin", body);
        let pin = sha256_hex(body);
        let err = fetch_verified_utf8(&url, &pin).unwrap_err();
        assert!(matches!(err, CompanionError::NotUtf8 { .. }));
        clear_home();
    }

    #[test]
    #[serial(jarvy_home_env)]
    fn refuses_non_https_url() {
        let _home = isolated_home();
        let pin = sha256_hex(b"whatever");
        let err = fetch_verified("http://example.com/x.sh", &pin).unwrap_err();
        match err {
            CompanionError::Fetch { source, .. } => {
                assert!(matches!(source, FetchError::NonHttps(_)));
            }
            other => panic!("expected Fetch(NonHttps), got {other:?}"),
        }
        clear_home();
    }
}
