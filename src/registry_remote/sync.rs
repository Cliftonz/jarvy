//! Sync orchestrator: fetch → verify → stage → atomic-swap.
//!
//! Flow (read top-to-bottom):
//!
//! 1. Load `RegistryConfig` from `~/.jarvy/config.toml`. Refuse if
//!    absent / disabled / unsafe-shaped (non-HTTPS, unanchored regex).
//! 2. Fetch `<base>/manifest.json` + `.sig` + `.pem` companions.
//! 3. Stage all three as `*.unverified` files; cosign-verify the
//!    `.unverified` manifest against the configured identity-regexp +
//!    OIDC issuer. Refuse if verification fails (unless
//!    `require_signature = false`). Promote `.unverified` → canonical
//!    name only after verification passes — the prior known-good
//!    manifest stays in place until that point.
//! 4. Parse the manifest; validate every entry wholesale.
//! 5. Fresh staging dir `tools.new/`. Per tool: fetch, sha256-verify,
//!    write into staging.
//! 6. After all tools land successfully, atomic-swap `tools.new/` into
//!    place of `tools/`. The prior `tools/` is removed only after the
//!    new one is in place.
//! 7. Write `meta.json` with `last_synced_at_unix`, `duration_ms`,
//!    `tools_count`, `signature_verified`.
//!
//! Each step is fail-fast — a single failure aborts the sync and
//! leaves the prior cache state untouched. The previous wipe-then-write
//! shape violated this invariant (the wipe ran before tool fetches, so
//! a failed first-tool-fetch left the cache empty). The staging-dir
//! pattern preserves the invariant for real.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::time::{Instant, SystemTime};
use thiserror::Error;

use super::cache::{self, CacheError};
use super::config::RegistryConfig;
use super::fetch::{self, FetchError, MAX_MANIFEST_BYTES, MAX_SIG_BYTES, MAX_TOOL_BYTES};
use super::manifest::{Manifest, ManifestError};
use crate::update::signature::{
    SignatureOutcome, VerifyError, signature_outcome_is_acceptable,
    verify_sigstore_signature_with_identity,
};

#[derive(Debug, Error)]
pub enum SyncError {
    #[error(
        "registry not configured — set [registry] url and enabled = true in ~/.jarvy/config.toml"
    )]
    NotConfigured,
    #[error("registry config is unsafe: {0}")]
    UnsafeConfig(String),
    #[error("fetch error: {0}")]
    Fetch(#[from] FetchError),
    #[error("manifest parse error: {0}")]
    Manifest(#[from] ManifestError),
    #[error("cache error: {0}")]
    Cache(#[from] CacheError),
    #[error("signature verification failed: {0}")]
    Signature(String),
    #[error("cosign error: {0}")]
    CosignBackend(#[from] VerifyError),
    #[error(
        "tool {name:?} sha256 mismatch: manifest says {expected}, fetched body hashes to {actual}"
    )]
    ShaMismatch {
        name: String,
        expected: String,
        actual: String,
    },
}

/// Summary returned to the CLI handler so it can print a human report.
#[derive(Debug, Clone)]
pub struct SyncReport {
    pub tools_synced: usize,
    pub tools_removed: usize,
    pub signature_verified: bool,
    /// Registry URL with any embedded credentials stripped — safe to
    /// surface in CLI output, meta.json, and tracing events.
    pub registry_url: String,
    pub duration_ms: u64,
}

/// Run a full sync. Returns the report or the first error.
pub fn run_sync() -> Result<SyncReport, SyncError> {
    let cfg = RegistryConfig::load().ok_or(SyncError::NotConfigured)?;
    run_sync_with_config(&cfg)
}

/// Run a sync against an explicit config. Kept separate from `run_sync`
/// so tests can hand-build a `RegistryConfig` pointing at a local mock
/// server without round-tripping through the real `~/.jarvy/config.toml`.
pub fn run_sync_with_config(cfg: &RegistryConfig) -> Result<SyncReport, SyncError> {
    let started_at = Instant::now();

    if !cfg.is_active() {
        emit(|| {
            tracing::warn!(
                event = "registry.sync.failed",
                stage = "preflight",
                reason = "not_configured"
            );
        });
        return Err(SyncError::NotConfigured);
    }
    if let Err(reason) = cfg.validate_safety() {
        emit(|| {
            tracing::error!(
                event = "registry.sync.failed",
                stage = "preflight",
                reason = "unsafe_config",
                detail = %reason,
            );
        });
        return Err(SyncError::UnsafeConfig(reason));
    }

    let redacted_url = crate::network::redact_credentials(&cfg.url).into_owned();
    emit(|| {
        tracing::info!(
            event = "registry.sync.started",
            registry_url = %redacted_url,
            require_signature = cfg.require_signature,
        );
    });

    // -- 1. Fetch manifest + cosign companions into *.unverified files.
    //
    // We do NOT touch the canonical `manifest.json` / `manifest.json.sig`
    // / `manifest.json.pem` paths until cosign verification passes. A
    // failed verify therefore leaves the prior known-good triplet in
    // place — preserving the doc-comment invariant.
    let manifest_bytes = fetch_with_event(&cfg.manifest_url(), MAX_MANIFEST_BYTES, &redacted_url)?;
    let cache_root = cache::cache_root()?;
    let manifest_path = cache_root.join("manifest.json");
    let sig_path = cache_root.join("manifest.json.sig");
    let pem_path = cache_root.join("manifest.json.pem");
    let manifest_unverified = cache_root.join("manifest.json.unverified");
    let sig_unverified = cache_root.join("manifest.json.sig.unverified");
    let pem_unverified = cache_root.join("manifest.json.pem.unverified");
    cache::write_atomic(&manifest_unverified, &manifest_bytes)?;

    // -- 2. Verify signature (or accept unsigned if explicitly allowed).
    let signature_verified = if cfg.require_signature {
        let sig_bytes = fetch_with_event(&cfg.signature_url(), MAX_SIG_BYTES, &redacted_url)?;
        let pem_bytes = fetch_with_event(&cfg.certificate_url(), MAX_SIG_BYTES, &redacted_url)?;
        cache::write_atomic(&sig_unverified, &sig_bytes)?;
        cache::write_atomic(&pem_unverified, &pem_bytes)?;

        // verify_sigstore_signature_with_identity looks for sig + pem
        // siblings of the file path it's given. Stage them under the
        // same .unverified.* prefix so it finds them without
        // overwriting the canonical pair.
        let outcome = verify_sigstore_signature_with_identity(
            &manifest_unverified,
            &cfg.signature_identity_regexp,
            &cfg.signature_oidc_issuer,
        )?;
        if let Err(reason) = signature_outcome_is_acceptable(&outcome, false) {
            // Clean up the unverified staging files so a re-run starts
            // fresh; don't leave attacker bytes on disk.
            let _ = std::fs::remove_file(&manifest_unverified);
            let _ = std::fs::remove_file(&sig_unverified);
            let _ = std::fs::remove_file(&pem_unverified);
            emit(|| {
                tracing::error!(
                    event = "registry.sync.signature_refused",
                    registry_url = %redacted_url,
                    identity_regexp = %cfg.signature_identity_regexp,
                    oidc_issuer = %cfg.signature_oidc_issuer,
                    reason = %reason,
                );
            });
            return Err(SyncError::Signature(reason));
        }
        // Promote .unverified → canonical AFTER the verify succeeds.
        std::fs::rename(&manifest_unverified, &manifest_path).map_err(CacheError::from)?;
        std::fs::rename(&sig_unverified, &sig_path).map_err(CacheError::from)?;
        std::fs::rename(&pem_unverified, &pem_path).map_err(CacheError::from)?;
        matches!(outcome, SignatureOutcome::Verified)
    } else {
        // require_signature=false is the documented escape hatch. Stderr
        // warning AND structured tracing event so a fleet operator can
        // detect machines running with signature checks disabled.
        eprintln!(
            "jarvy: WARNING — registry signature verification disabled \
             (require_signature=false); only safe for local development against \
             trusted mirrors"
        );
        emit(|| {
            tracing::warn!(
                event = "registry.signature_disabled",
                registry_url = %redacted_url,
            );
        });
        // Promote the manifest to the canonical path even without verify
        // so subsequent runs / status can read it.
        std::fs::rename(&manifest_unverified, &manifest_path).map_err(CacheError::from)?;
        false
    };

    // -- 3. Parse manifest. Validation rejects malformed entries wholesale.
    let manifest_str =
        std::str::from_utf8(&manifest_bytes).map_err(|_| ManifestError::InvalidEncoding)?;
    let manifest = Manifest::parse(manifest_str).inspect_err(|e| {
        emit(|| {
            tracing::error!(
                event = "registry.sync.failed",
                stage = "manifest_parse",
                error = %e,
            );
        });
    })?;

    // -- 4. Snapshot pre-existing tool filenames so we can count removals.
    let pre_existing = list_cached_tool_files()?;

    // -- 5. Per-tool fetch + sha-verify into a fresh STAGING dir,
    //       parallelized across a bounded worker pool.
    //
    // The staging dir is wiped fresh at entry; the active tools/ dir is
    // left alone. A failure in any worker stops the others fast via
    // `first_error` — same fail-fast semantics as the serial loop,
    // just with N parallel HTTPS round-trips per worker pool tick.
    //
    // Parallelism caps at JARVY_REGISTRY_SYNC_PARALLELISM (default 8)
    // OR the tool count, whichever is smaller — we never spawn more
    // threads than there is work. The shared `crate::net::agent`
    // ureq::Agent supports concurrent connections via its internal
    // pool, so workers reuse keep-alive connections rather than each
    // doing a fresh TLS handshake.
    let staging = cache::fresh_staging_tools_dir()?;
    let total = manifest.tools.len();
    let written_filenames: std::sync::Mutex<HashSet<String>> =
        std::sync::Mutex::new(HashSet::with_capacity(total));
    let first_error: std::sync::Mutex<Option<SyncError>> = std::sync::Mutex::new(None);
    let next_idx = std::sync::atomic::AtomicUsize::new(0);
    let max_parallel = std::env::var("JARVY_REGISTRY_SYNC_PARALLELISM")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8)
        .clamp(1, total.max(1));
    let staging_ref = &staging;
    let cfg_ref = cfg;
    let manifest_ref = &manifest;
    let redacted_ref = redacted_url.as_str();
    let first_error_ref = &first_error;
    let written_ref = &written_filenames;
    let next_idx_ref = &next_idx;

    std::thread::scope(|scope| {
        for _ in 0..max_parallel {
            scope.spawn(move || {
                let mut sha_buf = String::with_capacity(64);
                let mut filename_buf = String::with_capacity(64);
                loop {
                    // Fast-exit if another worker already errored.
                    if first_error_ref.lock().unwrap().is_some() {
                        return;
                    }
                    let idx = next_idx_ref.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if idx >= total {
                        return;
                    }
                    let entry = &manifest_ref.tools[idx];
                    let url = cfg_ref.tool_url(&entry.path);
                    let body = match fetch_with_event(&url, MAX_TOOL_BYTES, redacted_ref) {
                        Ok(b) => b,
                        Err(e) => {
                            let mut slot = first_error_ref.lock().unwrap();
                            if slot.is_none() {
                                *slot = Some(e);
                            }
                            return;
                        }
                    };
                    sha_buf.clear();
                    sha256_hex_into(&body, &mut sha_buf);
                    if sha_buf != entry.sha256 {
                        emit(|| {
                            tracing::error!(
                                event = "registry.sync.sha_mismatch",
                                tool = %entry.name,
                                url = %url,
                                expected = %entry.sha256,
                                actual = %sha_buf,
                            );
                        });
                        let mut slot = first_error_ref.lock().unwrap();
                        if slot.is_none() {
                            *slot = Some(SyncError::ShaMismatch {
                                name: entry.name.clone(),
                                expected: entry.sha256.clone(),
                                actual: sha_buf.clone(),
                            });
                        }
                        return;
                    }

                    filename_buf.clear();
                    write!(filename_buf, "{}.toml", entry.name)
                        .expect("write to String never fails");
                    let dest = staging_ref.join(&filename_buf);
                    if let Err(e) = cache::write_atomic(&dest, &body) {
                        let mut slot = first_error_ref.lock().unwrap();
                        if slot.is_none() {
                            *slot = Some(SyncError::Cache(e));
                        }
                        return;
                    }
                    emit(|| {
                        tracing::debug!(
                            event = "registry.sync.tool.synced",
                            tool = %entry.name,
                            bytes = body.len() as u64,
                        );
                    });
                    written_ref.lock().unwrap().insert(filename_buf.clone());
                }
            });
        }
    });

    if let Some(err) = first_error.into_inner().expect("first_error poisoned") {
        return Err(err);
    }
    let written_filenames = written_filenames
        .into_inner()
        .expect("written_filenames poisoned");

    // -- 6. Atomic-swap staging → active. From this point on the new set
    //       is live; pre_existing − written = removed.
    cache::swap_staging_into_tools_dir()?;
    let removed_count = pre_existing
        .iter()
        .filter(|f| !written_filenames.contains(*f))
        .count();

    // -- 7. Mark sync complete.
    let duration_ms = started_at.elapsed().as_millis() as u64;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let meta_payload = serde_json::json!({
        "last_synced_at_unix": now,
        "registry_url": redacted_url,
        "tools_count": manifest.tools.len(),
        "tools_removed": removed_count,
        "signature_verified": signature_verified,
        "duration_ms": duration_ms,
    });
    let meta_path = cache::cache_root()?.join("meta.json");
    cache::write_atomic(&meta_path, meta_payload.to_string().as_bytes())?;

    // Build the parsed-tools index alongside meta.json. The plugin
    // loader will prefer this single-file read over the per-tool walk
    // on subsequent CLI startups. Failures here are non-fatal — they
    // just mean the loader falls back to the walk.
    if let Err(e) = crate::tools::plugins::build_remote_index(now) {
        emit(|| {
            tracing::warn!(
                event = "registry.cache.index_build_failed",
                error = %e,
            );
        });
    }

    emit(|| {
        tracing::info!(
            event = "registry.sync.completed",
            registry_url = %redacted_url,
            tools_synced = manifest.tools.len(),
            tools_removed = removed_count,
            signature_verified = signature_verified,
            duration_ms = duration_ms,
        );
    });

    Ok(SyncReport {
        tools_synced: manifest.tools.len(),
        tools_removed: removed_count,
        signature_verified,
        registry_url: redacted_url,
        duration_ms,
    })
}

/// Bounded HTTPS fetch + structured tracing event around each call.
fn fetch_with_event(
    url: &str,
    max_bytes: u64,
    redacted_registry: &str,
) -> Result<Vec<u8>, SyncError> {
    emit(|| {
        tracing::debug!(
            event = "registry.fetch.start",
            url = %url,
            max_bytes = max_bytes,
        );
    });
    match fetch::fetch_bounded(url, max_bytes) {
        Ok(bytes) => {
            emit(|| {
                tracing::debug!(
                    event = "registry.fetch.completed",
                    url = %url,
                    bytes = bytes.len() as u64,
                );
            });
            Ok(bytes)
        }
        Err(e) => {
            emit(|| {
                tracing::warn!(
                    event = "registry.fetch.failed",
                    url = %url,
                    registry_url = %redacted_registry,
                    error = %e,
                );
            });
            Err(SyncError::Fetch(e))
        }
    }
}

/// Hex-encode the sha256 of a byte slice into a pre-allocated `String`.
/// Caller is responsible for clearing the buffer between iterations.
/// Single allocation per call (the buffer's existing capacity) — replaces
/// the per-byte `format!`-into-`collect` shape which allocated 32 short
/// strings + a final concat per tool.
fn sha256_hex_into(bytes: &[u8], out: &mut String) {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    for b in digest.iter() {
        write!(out, "{b:02x}").expect("write to String never fails");
    }
}

/// List `*.toml` files currently in the cache's tools/ dir. Used to
/// count removals (manifest dropped these between syncs).
fn list_cached_tool_files() -> Result<Vec<String>, CacheError> {
    let dir = cache::tools_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(64);
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".toml") {
                    out.push(name.to_string());
                }
            }
        }
    }
    Ok(out)
}

/// Tracing-emit helper. Gates every event on the global telemetry gate
/// per CLAUDE.md (the same contract `packages.*` events follow). When
/// telemetry is disabled at the user-config layer, registry events
/// don't leak to OTLP.
fn emit<F: FnOnce()>(f: F) {
    if crate::observability::telemetry_gate::is_enabled() {
        f();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_sync_refuses_disabled_config() {
        let cfg = RegistryConfig {
            url: "https://example.com/r/".into(),
            enabled: false,
            ..Default::default()
        };
        let err = run_sync_with_config(&cfg).unwrap_err();
        assert!(matches!(err, SyncError::NotConfigured));
    }

    #[test]
    fn run_sync_refuses_empty_url() {
        let cfg = RegistryConfig {
            url: "".into(),
            enabled: true,
            ..Default::default()
        };
        let err = run_sync_with_config(&cfg).unwrap_err();
        assert!(matches!(err, SyncError::NotConfigured));
    }

    #[test]
    fn run_sync_refuses_http_url() {
        let cfg = RegistryConfig {
            url: "http://example.com/r/".into(),
            enabled: true,
            ..Default::default()
        };
        let err = run_sync_with_config(&cfg).unwrap_err();
        assert!(matches!(err, SyncError::UnsafeConfig(_)));
    }

    #[test]
    fn run_sync_refuses_unanchored_identity_regex() {
        let cfg = RegistryConfig {
            url: "https://example.com/r/".into(),
            enabled: true,
            signature_identity_regexp: "github.com/x/.*".into(),
            ..Default::default()
        };
        let err = run_sync_with_config(&cfg).unwrap_err();
        assert!(matches!(err, SyncError::UnsafeConfig(_)));
    }

    #[test]
    fn sha256_hex_into_known_value() {
        let mut buf = String::with_capacity(64);
        sha256_hex_into(b"abc", &mut buf);
        assert_eq!(
            buf,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(buf.len(), 64);
        assert!(
            buf.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }
}
