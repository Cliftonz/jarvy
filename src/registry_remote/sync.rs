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
    #[error("tool {name:?} body is not valid utf-8 or not a parseable PluginTool TOML")]
    ToolParseFailed { name: String },
}

/// Per-worker accumulator returned from a `thread::scope` spawn. Workers
/// build local Vecs (zero shared-state contention on the success path)
/// and the orchestrator merges them after the scope joins.
struct WorkerResult {
    filenames: Vec<String>,
    parsed_tools: Vec<crate::tools::plugins::PluginTool>,
}

/// First-error-wins: only the worker that successfully flips the
/// AtomicBool from false→true writes its error into the mutex slot.
/// Subsequent failures see the flag already set and return without
/// touching the slot.
fn set_first_error(
    flag: &std::sync::atomic::AtomicBool,
    slot: &std::sync::Mutex<Option<SyncError>>,
    err: SyncError,
) {
    if flag
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .is_ok()
    {
        *slot.lock().expect("first_error_slot poisoned") = Some(err);
    }
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

    // -- 1. Fetch manifest bytes. PARSE BEFORE WRITING — a malformed
    //       manifest (invalid UTF-8, bad JSON, bad schema) must not
    //       leave a `manifest.json` on disk that a subsequent
    //       `jarvy registry status` would happily print.
    let manifest_bytes = fetch_with_event(&cfg.manifest_url(), MAX_MANIFEST_BYTES, &redacted_url)?;
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

    let cache_root = cache::cache_root()?;
    let manifest_path = cache_root.join("manifest.json");
    let sig_path = cache_root.join("manifest.json.sig");
    let pem_path = cache_root.join("manifest.json.pem");
    let manifest_unverified = cache_root.join("manifest.json.unverified");
    // verify_sigstore_signature_with_identity derives the sig/pem
    // siblings via `file_path.with_extension(format!("{ext}.sig"))` on
    // the input path. Given `manifest.json.unverified` (extension =
    // "unverified"), it looks for `manifest.json.unverified.sig` and
    // `manifest.json.unverified.pem`. Stage at exactly those names.
    let sig_unverified = cache_root.join("manifest.json.unverified.sig");
    let pem_unverified = cache_root.join("manifest.json.unverified.pem");
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

    // -- 3. (Manifest already parsed in step 1 — pre-promote so a
    //       malformed body doesn't poison the canonical manifest.json.)

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
    // Hot-path coordination: AtomicBool for the flag workers check on
    // every iteration (lock-free read); the typed SyncError only goes
    // through the mutex on the once-per-error write path.
    let first_error_flag = std::sync::atomic::AtomicBool::new(false);
    let first_error_slot: std::sync::Mutex<Option<SyncError>> = std::sync::Mutex::new(None);
    let next_idx = std::sync::atomic::AtomicUsize::new(0);
    // ABSOLUTE upper bound prevents `JARVY_REGISTRY_SYNC_PARALLELISM=
    // usize::MAX` from spawning thousands of OS threads on a hostile
    // manifest. 64 matches what `crate::net::agent`'s connection pool
    // can usefully keep alive concurrently.
    let max_parallel = std::env::var("JARVY_REGISTRY_SYNC_PARALLELISM")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8);
    #[allow(clippy::manual_clamp)] // explicit two-step floor/ceiling reads cleaner here
    let max_parallel = max_parallel.max(1).min(total.max(1)).min(64);
    let cfg_ref = cfg;
    let manifest_ref = &manifest;
    let redacted_ref = redacted_url.as_str();
    let first_error_flag_ref = &first_error_flag;
    let first_error_slot_ref = &first_error_slot;
    let next_idx_ref = &next_idx;
    let staging_ref = &staging;

    // Spawn workers inside the scope AND join them inside, so the
    // returned `WorkerResult`s outlive the scope without holding any
    // ScopedJoinHandle references on the outer stack.
    let (parsed_tools, written_filenames) = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(max_parallel);
        for worker_id in 0..max_parallel {
            handles.push(scope.spawn(move || -> WorkerResult {
                let mut local_filenames: Vec<String> = Vec::with_capacity(total / max_parallel + 1);
                let mut local_tools: Vec<crate::tools::plugins::PluginTool> =
                    Vec::with_capacity(total / max_parallel + 1);
                let mut sha_buf = String::with_capacity(64);
                let mut filename_buf = String::with_capacity(64);
                loop {
                    // Lock-free fast-exit if another worker already errored.
                    if first_error_flag_ref.load(std::sync::atomic::Ordering::Acquire) {
                        return WorkerResult {
                            filenames: local_filenames,
                            parsed_tools: local_tools,
                        };
                    }
                    let idx = next_idx_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if idx >= total {
                        return WorkerResult {
                            filenames: local_filenames,
                            parsed_tools: local_tools,
                        };
                    }
                    let entry = &manifest_ref.tools[idx];
                    let url = cfg_ref.tool_url(&entry.path);
                    let url_for_log = crate::network::redact_credentials(&url).into_owned();
                    emit(|| {
                        tracing::debug!(
                            event = "registry.sync.tool.start",
                            tool = %entry.name,
                            worker_id = worker_id,
                            url = %url_for_log,
                        );
                    });
                    let body = match fetch_with_event(&url, MAX_TOOL_BYTES, redacted_ref) {
                        Ok(b) => b,
                        Err(e) => {
                            emit(|| {
                                tracing::warn!(
                                    event = "registry.sync.tool_fetch_failed",
                                    tool = %entry.name,
                                    worker_id = worker_id,
                                    url = %url_for_log,
                                    error = %e,
                                );
                            });
                            set_first_error(first_error_flag_ref, first_error_slot_ref, e);
                            return WorkerResult {
                                filenames: local_filenames,
                                parsed_tools: local_tools,
                            };
                        }
                    };
                    sha_buf.clear();
                    sha256_hex_into(&body, &mut sha_buf);
                    if sha_buf != entry.sha256 {
                        emit(|| {
                            tracing::error!(
                                event = "registry.sync.sha_mismatch",
                                tool = %entry.name,
                                worker_id = worker_id,
                                url = %url_for_log,
                                expected = %entry.sha256,
                                actual = %sha_buf,
                            );
                        });
                        set_first_error(
                            first_error_flag_ref,
                            first_error_slot_ref,
                            SyncError::ShaMismatch {
                                name: entry.name.clone(),
                                expected: entry.sha256.clone(),
                                actual: sha_buf.clone(),
                            },
                        );
                        return WorkerResult {
                            filenames: local_filenames,
                            parsed_tools: local_tools,
                        };
                    }

                    // Parse the TOML BEFORE writing — if it's malformed
                    // the sha matched but the body is unusable, and we'd
                    // rather fail the sync than silently ship a bad TOML
                    // into staging.
                    let parsed = match std::str::from_utf8(&body)
                        .ok()
                        .and_then(|s| toml::from_str::<crate::tools::plugins::PluginTool>(s).ok())
                    {
                        Some(p) => p,
                        None => {
                            emit(|| {
                                tracing::error!(
                                    event = "registry.sync.tool_parse_failed",
                                    tool = %entry.name,
                                    worker_id = worker_id,
                                );
                            });
                            set_first_error(
                                first_error_flag_ref,
                                first_error_slot_ref,
                                SyncError::ToolParseFailed {
                                    name: entry.name.clone(),
                                },
                            );
                            return WorkerResult {
                                filenames: local_filenames,
                                parsed_tools: local_tools,
                            };
                        }
                    };

                    filename_buf.clear();
                    write!(filename_buf, "{}.toml", entry.name)
                        .expect("write to String never fails");
                    let dest = staging_ref.join(&filename_buf);
                    if let Err(e) = cache::write_atomic(&dest, &body) {
                        emit(|| {
                            tracing::error!(
                                event = "registry.sync.tool_write_failed",
                                tool = %entry.name,
                                worker_id = worker_id,
                                error = %e,
                            );
                        });
                        set_first_error(
                            first_error_flag_ref,
                            first_error_slot_ref,
                            SyncError::Cache(e),
                        );
                        return WorkerResult {
                            filenames: local_filenames,
                            parsed_tools: local_tools,
                        };
                    }
                    emit(|| {
                        tracing::debug!(
                            event = "registry.sync.tool.synced",
                            tool = %entry.name,
                            worker_id = worker_id,
                            bytes = body.len() as u64,
                        );
                    });
                    // Per-worker accumulator — merged after scope joins.
                    // Single mem::take + reset gives us reuse without
                    // a per-iteration clone.
                    let owned = std::mem::take(&mut filename_buf);
                    filename_buf = String::with_capacity(64);
                    local_filenames.push(owned);
                    local_tools.push(parsed);
                }
            }));
        }
        // Join handles inside the scope so their data outlives it.
        let mut filenames: HashSet<String> = HashSet::with_capacity(total);
        let mut tools: Vec<crate::tools::plugins::PluginTool> = Vec::with_capacity(total);
        for h in handles {
            let WorkerResult {
                filenames: f,
                parsed_tools: t,
            } = h.join().expect("worker thread panicked");
            for name in f {
                filenames.insert(name);
            }
            tools.extend(t);
        }
        (tools, filenames)
    });

    if let Some(err) = first_error_slot
        .into_inner()
        .expect("first_error_slot poisoned")
    {
        return Err(err);
    }

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

    // Build the parsed-tools index alongside meta.json. We hand it the
    // already-parsed PluginTool set the workers accumulated — no extra
    // walk + read + parse. Failures here are non-fatal: the loader
    // falls back to walking tools/ on next startup.
    if let Err(e) = crate::tools::plugins::build_remote_index(now, parsed_tools) {
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
        if entry.file_type()?.is_file()
            && let Some(name) = entry.file_name().to_str()
            && name.ends_with(".toml")
        {
            out.push(name.to_string());
        }
    }
    Ok(out)
}

// Tracing-emit helper now lives in `observability::telemetry_gate` so
// every registry call site (sync, CLI handler, cache, plugin loader)
// can route through the same gate. Re-exported as a local alias to
// keep the existing call-site shape `emit(|| tracing::...)`.
use crate::observability::telemetry_gate::emit;

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
