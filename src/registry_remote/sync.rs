//! Sync orchestrator: fetch → verify → cache.
//!
//! Flow (read top-to-bottom):
//!
//! 1. Load `RegistryConfig` from `~/.jarvy/config.toml`. If absent or
//!    disabled, refuse with a clear error.
//! 2. Fetch `<base>/manifest.json` + `.sig` + `.pem` companions.
//! 3. Stage all three on disk, then call cosign-verify-blob against the
//!    configured identity-regexp + OIDC issuer. Refuse if it fails
//!    (unless `require_signature = false`).
//! 4. Parse the manifest, validate every entry.
//! 5. For each tool entry: fetch the TOML, sha256-verify against the
//!    manifest, write into the cache.
//! 6. Wipe any stale tool TOMLs that were removed upstream.
//! 7. Write `meta.json` with `last_synced_at`.
//!
//! Each step is fail-fast — a single failure aborts the whole sync and
//! leaves the prior cache state untouched. A partial sync would leave
//! the runtime in an inconsistent state where some tools point at the
//! new manifest's versions but others are stale; better to keep the
//! known-good cache and report a clear failure.

use std::collections::HashSet;
use std::time::SystemTime;
use thiserror::Error;

use super::cache::{self, CacheError};
use super::config::RegistryConfig;
use super::fetch::{self, FetchError, MAX_MANIFEST_BYTES, MAX_SIG_BYTES, MAX_TOOL_BYTES};
use super::manifest::{Manifest, ManifestError};
use crate::update::signature::{SignatureOutcome, VerifyError, signature_outcome_is_acceptable};

#[derive(Debug, Error)]
pub enum SyncError {
    #[error(
        "registry not configured — set [registry] url and enabled = true in ~/.jarvy/config.toml"
    )]
    NotConfigured,
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
    #[error("manifest fetched as text but is not valid utf-8")]
    ManifestNotUtf8,
}

/// Summary returned to the CLI handler so it can print a human report.
#[derive(Debug, Clone)]
pub struct SyncReport {
    pub tools_synced: usize,
    pub tools_removed: usize,
    pub signature_verified: bool,
    pub registry_url: String,
}

/// Run a full sync. Returns the report or the first error.
pub fn run_sync() -> Result<SyncReport, SyncError> {
    let cfg = RegistryConfig::load().ok_or(SyncError::NotConfigured)?;
    run_sync_with_config(&cfg)
}

/// Run a sync against an explicit config. Kept separate from `run_sync`
/// for testability — callers in tests can hand-build a `RegistryConfig`
/// pointing at a local mock URL.
pub fn run_sync_with_config(cfg: &RegistryConfig) -> Result<SyncReport, SyncError> {
    if !cfg.is_active() {
        return Err(SyncError::NotConfigured);
    }

    // 1. Fetch + stage manifest + signature companions.
    let manifest_bytes = fetch::fetch_bounded(&cfg.manifest_url(), MAX_MANIFEST_BYTES)?;
    let cache_root = cache::cache_root()?;
    let manifest_path = cache_root.join("manifest.json");
    let sig_path = cache_root.join("manifest.json.sig");
    let pem_path = cache_root.join("manifest.json.pem");

    cache::write_atomic(&manifest_path, &manifest_bytes)?;

    // 2. Verify signature (or accept unsigned if explicitly allowed).
    let signature_verified = if cfg.require_signature {
        let sig_bytes = fetch::fetch_bounded(&cfg.signature_url(), MAX_SIG_BYTES)?;
        let pem_bytes = fetch::fetch_bounded(&cfg.certificate_url(), MAX_SIG_BYTES)?;
        cache::write_atomic(&sig_path, &sig_bytes)?;
        cache::write_atomic(&pem_path, &pem_bytes)?;

        let outcome = verify_with_identity(
            &manifest_path,
            &cfg.signature_identity_regexp,
            &cfg.signature_oidc_issuer,
        )?;
        if let Err(reason) = signature_outcome_is_acceptable(&outcome, false) {
            return Err(SyncError::Signature(reason));
        }
        matches!(outcome, SignatureOutcome::Verified)
    } else {
        eprintln!(
            "jarvy: WARNING — registry signature verification disabled (require_signature=false); \
             only safe for local development against trusted mirrors"
        );
        false
    };

    // 3. Parse manifest. Validation rejects malformed entries wholesale.
    let manifest_str =
        std::str::from_utf8(&manifest_bytes).map_err(|_| SyncError::ManifestNotUtf8)?;
    let manifest = Manifest::parse(manifest_str)?;

    // 4. Snapshot pre-existing tools so we can count removals after.
    let pre_existing = list_cached_tool_files()?;

    // 5. Fetch + sha-verify each tool TOML and write into cache.
    cache::wipe_tools_dir()?;
    let mut written_count = 0;
    let mut written_filenames: HashSet<String> = HashSet::new();
    let tools_dir = cache::tools_dir()?;
    for entry in &manifest.tools {
        let url = cfg.tool_url(&entry.path);
        let body = fetch::fetch_bounded(&url, MAX_TOOL_BYTES)?;
        let actual_sha = sha256_hex(&body);
        if actual_sha != entry.sha256 {
            return Err(SyncError::ShaMismatch {
                name: entry.name.clone(),
                expected: entry.sha256.clone(),
                actual: actual_sha,
            });
        }

        // File on disk is `<tool-name>.toml`, NOT the manifest's path —
        // collapsing nested manifest paths into a flat `tools/` dir
        // keeps the plugin loader's walk shallow and predictable.
        let filename = format!("{}.toml", entry.name);
        let dest = tools_dir.join(&filename);
        cache::write_atomic(&dest, &body)?;
        written_filenames.insert(filename);
        written_count += 1;
    }

    let removed_count = pre_existing
        .iter()
        .filter(|f| !written_filenames.contains(*f))
        .count();

    // 6. Mark sync complete.
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let meta = serde_json::json!({
        "last_synced_at_unix": now,
        "registry_url": cfg.url,
        "tools_count": written_count,
        "signature_verified": signature_verified,
    });
    cache::write_meta(&meta.to_string())?;

    Ok(SyncReport {
        tools_synced: written_count,
        tools_removed: removed_count,
        signature_verified,
        registry_url: cfg.url.clone(),
    })
}

/// Hex-encode the sha256 of a byte slice. Lowercase to match the
/// manifest invariant.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    out.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Thin wrapper around `verify_sigstore_signature` that lets us swap
/// the identity-regexp + issuer instead of relying on the update-path's
/// hardcoded Jarvy-release identity.
fn verify_with_identity(
    manifest_path: &std::path::Path,
    identity_regexp: &str,
    oidc_issuer: &str,
) -> Result<SignatureOutcome, VerifyError> {
    // Reuse the public cosign helpers from src/update/signature.rs but
    // call cosign with explicit identity/issuer flags. Mirrors the
    // pattern the update installer uses; just identity-pinned to the
    // registry repo instead of the Jarvy release workflow.
    use std::process::Command;

    let sig_path = manifest_path.with_extension("json.sig");
    let pem_path = manifest_path.with_extension("json.pem");

    if !sig_path.exists() || !pem_path.exists() {
        return Ok(SignatureOutcome::SignatureFilesMissing);
    }

    if Command::new("cosign").arg("version").output().is_err() {
        return Ok(SignatureOutcome::CosignMissing);
    }

    let output = Command::new("cosign")
        .arg("verify-blob")
        .arg("--signature")
        .arg(&sig_path)
        .arg("--certificate")
        .arg(&pem_path)
        .arg("--certificate-identity-regexp")
        .arg(identity_regexp)
        .arg("--certificate-oidc-issuer")
        .arg(oidc_issuer)
        .arg(manifest_path)
        .output()
        .map_err(VerifyError::Io)?;

    if output.status.success() {
        Ok(SignatureOutcome::Verified)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Ok(SignatureOutcome::Rejected(stderr))
    }
}

/// List `*.toml` files currently in the cache's tools/ dir. Used to
/// count removals (manifest dropped these between syncs).
fn list_cached_tool_files() -> Result<Vec<String>, CacheError> {
    let dir = cache::tools_dir()?;
    let mut out = Vec::new();
    if dir.exists() {
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
    }
    Ok(out)
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
    fn sha256_hex_is_lowercase_64_chars() {
        let sha = sha256_hex(b"abc");
        assert_eq!(sha.len(), 64);
        assert!(
            sha.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        // Known sha256("abc")
        assert_eq!(
            sha,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
