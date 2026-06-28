//! Shared library-registry pattern (PRD-054).
//!
//! Establishes the manifest + HTTPS fetch + cache mechanism reused by
//! three consumers: `[ai_hooks] library_sources`, `[mcp_register]
//! library_sources`, `[skills] library_sources`. One format, one fetch
//! pipeline, one cache layout — adding a fourth library kind in the
//! future is a new variant on `LibraryItem`, not a new module.
//!
//! # Trust model
//!
//! - **HTTPS-only**. Non-HTTPS URLs refused at the fetch boundary.
//! - **Bounded reads** (`MAX_MANIFEST_BYTES`, `MAX_ITEM_BYTES`) to
//!   protect against accidental DoS from a misbehaving publisher.
//! - **Remote-config refusal**: `library_sources` from a remote-fetched
//!   `jarvy.toml` (`jarvy setup --from <url>`) are refused — emits
//!   `library.remote_refused` event. Mirrors `[packages] allow_remote`
//!   semantics.
//! - **Signature verification**: scaffolded but not enforced in v1.
//!   `require_signature = true` (default) is honored once cosign wiring
//!   lands. Until then, libraries are fetched on trust of the URL plus
//!   sha256 verification of off-manifest artifacts. Documented as
//!   "signing recommended, will be enforced in a future Jarvy release."
//!
//! # Cache
//!
//! In-process cache per `LibrarySource` URL, populated lazily on first
//! `sync()` call. Persisted to disk at
//! `~/.jarvy/library.d/<sha256-of-url>/manifest.json` so a network
//! outage doesn't break `jarvy setup`. TTL is per-source via
//! `refresh_interval_secs` (default 86400 = 24h).
//!
//! # Public API
//!
//! ```ignore
//! // Fetch + cache a manifest:
//! let report = library_registry::sync(&source)?;
//!
//! // Resolve a hook / mcp server / skill by name across every cached
//! // library, in declaration order. First match wins; built-in
//! // libraries (`crate::ai_hooks::LIBRARY`) are checked BEFORE
//! // library_sources by the consumer, so name collisions favor the
//! // canonical Jarvy-shipped entry.
//! let item: Option<LibraryHookItem> = library_registry::resolve_hook("no-prod-deploys");
//! ```

pub mod cache;
pub mod config;
pub mod fetch;
pub mod git_fetch;
pub mod manifest;
pub mod url_parser;

pub use config::LibrarySource;
pub use manifest::{
    LibraryHookItem, LibraryItem, LibraryMcpItem, LibrarySkillItem, MANIFEST_SCHEMA_VERSION,
    Manifest,
};

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("fetch failed: {0}")]
    Fetch(#[from] fetch::FetchError),

    #[error("manifest parse error for {url}: {source}")]
    Parse {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("unsupported manifest schema_version {actual} (this binary understands {expected})")]
    UnsupportedSchema { expected: u32, actual: u32 },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "remote-fetched config attempted to declare library_sources for {consumer}; refusing (PRD-054 trust gate)"
    )]
    RemoteRefused { consumer: &'static str },
}

impl LibraryError {
    /// Stable telemetry discriminant.
    #[allow(dead_code)] // Public lib API for downstream consumers
    pub fn kind(&self) -> &'static str {
        match self {
            LibraryError::Fetch(_) => "fetch",
            LibraryError::Parse { .. } => "parse",
            LibraryError::UnsupportedSchema { .. } => "unsupported_schema",
            LibraryError::Io(_) => "io",
            LibraryError::RemoteRefused { .. } => "remote_refused",
        }
    }
}

/// Result of a successful `sync` call.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields surface via Debug + structured logging; consumers may read individually
pub struct SyncReport {
    pub url: String,
    pub items_total: usize,
    pub ai_hook_count: usize,
    pub mcp_server_count: usize,
    pub skill_count: usize,
    pub from_cache: bool,
    pub signature_verified: bool,
}

/// Process-wide cache of fetched manifests, keyed by URL. Populated
/// lazily on first `sync()` for each URL. Survives the process lifetime
/// — no TTL — because the disk-cache layer handles staleness.
static MANIFEST_CACHE: Mutex<Option<Vec<(String, Manifest)>>> = Mutex::new(None);

/// Fetch + cache the manifest at `source.url`. Returns a `SyncReport`
/// describing what was synced. On network failure, falls back to the
/// disk cache if present; if neither network nor disk has a copy,
/// returns the original `LibraryError::Fetch`.
pub fn sync(source: &LibrarySource) -> Result<SyncReport, LibraryError> {
    let telemetry_on = crate::observability::telemetry_gate::is_enabled();
    if telemetry_on {
        tracing::info!(
            event = "library.sync.started",
            url = %crate::network::redact_credentials(&source.url),
            require_signature = source.require_signature,
        );
    }

    // Dispatch on URL scheme (PRD-055): manifest URLs go through the
    // HTTPS fetcher; git+https:// / github: URLs go through the git
    // fetcher which clones + walks for SKILL.md and synthesizes a
    // manifest in-memory.
    let scheme = url_parser::parse_source(&source.url)?;
    let cache_path = cache::manifest_cache_path(&source.url)?;
    let (manifest, from_cache) = match scheme {
        url_parser::SourceScheme::Manifest { .. } => match fetch_and_parse(source) {
            Ok(m) => {
                // Write-through cache. Best-effort — if disk is full, we
                // still return the freshly-fetched manifest.
                if let Err(e) = cache::write_manifest(&cache_path, &m) {
                    tracing::warn!(
                        event = "library.cache.write_failed",
                        url = %crate::network::redact_credentials(&source.url),
                        error = %e,
                    );
                }
                (m, false)
            }
            Err(fetch_err) => match cache::read_manifest(&cache_path) {
                Ok(Some(m)) => {
                    if telemetry_on {
                        tracing::info!(
                            event = "library.fetch.cached_hit",
                            url = %crate::network::redact_credentials(&source.url),
                            reason = "fetch_failed",
                        );
                    }
                    (m, true)
                }
                Ok(None) | Err(_) => {
                    // Review item 8 (P0). Previously every sync failure
                    // returned silently to the caller; on-call could not
                    // compute "what fraction of library syncs failed in
                    // the last hour." Emit a structured event so
                    // dashboards can graph the failure rate.
                    if telemetry_on {
                        tracing::error!(
                            event = "library.sync.failed",
                            url = %crate::network::redact_credentials(&source.url),
                            scheme = "manifest",
                            error_kind = fetch_err.kind(),
                            error = %fetch_err,
                        );
                    }
                    return Err(fetch_err);
                }
            },
        },
        url_parser::SourceScheme::Git {
            repo,
            git_ref,
            subpath,
        } => {
            // Git fetch always populates from origin (or the disk cache
            // of the prior clone if `git fetch` succeeds offline-stale).
            // The synthesized manifest is written next to the clone.
            let clone_root = cache_path
                .parent()
                .ok_or_else(|| LibraryError::Io(std::io::Error::other("cache path has no parent")))?
                .to_path_buf();
            match git_fetch::sync_git(&repo, &git_ref, subpath.as_deref(), &clone_root) {
                Ok((m, _)) => {
                    if let Err(e) = cache::write_manifest(&cache_path, &m) {
                        tracing::warn!(
                            event = "library.cache.write_failed",
                            url = %crate::network::redact_credentials(&source.url),
                            error = %e,
                        );
                    }
                    (m, false)
                }
                Err(git_err) => match cache::read_manifest(&cache_path) {
                    Ok(Some(m)) => {
                        if telemetry_on {
                            tracing::info!(
                                event = "library.git.cache_hit",
                                url = %crate::network::redact_credentials(&source.url),
                                reason = "git_failed",
                            );
                        }
                        (m, true)
                    }
                    Ok(None) | Err(_) => {
                        // Review item 8 (P0) — see manifest-branch
                        // counterpart above.
                        if telemetry_on {
                            tracing::error!(
                                event = "library.sync.failed",
                                url = %crate::network::redact_credentials(&source.url),
                                scheme = "git",
                                error_kind = git_err.kind(),
                                error = %git_err,
                            );
                        }
                        return Err(git_err);
                    }
                },
            }
        }
    };

    // Populate the process cache.
    let mut cache_guard = MANIFEST_CACHE.lock().unwrap_or_else(|p| p.into_inner());
    let entries = cache_guard.get_or_insert_with(Vec::new);
    if let Some(slot) = entries.iter_mut().find(|(u, _)| *u == source.url) {
        slot.1 = manifest.clone();
    } else {
        entries.push((source.url.clone(), manifest.clone()));
    }
    drop(cache_guard);

    let mut ai_hook_count = 0;
    let mut mcp_server_count = 0;
    let mut skill_count = 0;
    for item in &manifest.items {
        match item {
            LibraryItem::AiHook(_) => ai_hook_count += 1,
            LibraryItem::McpServer(_) => mcp_server_count += 1,
            LibraryItem::Skill(_) => skill_count += 1,
        }
    }

    let report = SyncReport {
        url: source.url.clone(),
        items_total: manifest.items.len(),
        ai_hook_count,
        mcp_server_count,
        skill_count,
        from_cache,
        signature_verified: false, // v1 — see module-level doc
    };

    if telemetry_on {
        tracing::info!(
            event = "library.sync.completed",
            url = %crate::network::redact_credentials(&source.url),
            items_synced = report.items_total,
            ai_hook_count,
            mcp_server_count,
            skill_count,
            from_cache,
            signature_verified = report.signature_verified,
        );
    }

    if !source.require_signature && telemetry_on {
        tracing::warn!(
            event = "library.signature_disabled",
            url = %crate::network::redact_credentials(&source.url),
        );
        eprintln!(
            "  Warning: library {} fetched without signature verification \
             (`require_signature = false`); recommended only for development.",
            crate::network::redact_credentials(&source.url),
        );
    }

    Ok(report)
}

fn fetch_and_parse(source: &LibrarySource) -> Result<Manifest, LibraryError> {
    let url = canonicalize_manifest_url(&source.url);
    let body = fetch::fetch_bounded(&url, fetch::MAX_MANIFEST_BYTES)?;
    let manifest: Manifest = serde_json::from_slice(&body).map_err(|e| LibraryError::Parse {
        url: crate::network::redact_credentials(&url).into_owned(),
        source: e,
    })?;
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(LibraryError::UnsupportedSchema {
            expected: MANIFEST_SCHEMA_VERSION,
            actual: manifest.schema_version,
        });
    }
    Ok(manifest)
}

/// If the URL ends with `/`, append `manifest.json` so users can point
/// at the parent directory instead of typing the filename. Idempotent.
fn canonicalize_manifest_url(url: &str) -> String {
    if url.ends_with('/') {
        format!("{url}manifest.json")
    } else {
        url.to_string()
    }
}

/// Look up an AI hook by name across every cached library, in
/// insertion order. Returns the first match. Consumers SHOULD check
/// the built-in `crate::ai_hooks::LIBRARY` first so name collisions
/// favor the canonical Jarvy-shipped entry.
pub fn resolve_hook(name: &str) -> Option<LibraryHookItem> {
    with_cache(|entries| {
        for (_, manifest) in entries {
            for item in &manifest.items {
                if let LibraryItem::AiHook(h) = item {
                    if h.name == name {
                        return Some(h.clone());
                    }
                }
            }
        }
        None
    })
}

/// Look up an MCP server by name across every cached library.
pub fn resolve_mcp_server(name: &str) -> Option<LibraryMcpItem> {
    with_cache(|entries| {
        for (_, manifest) in entries {
            for item in &manifest.items {
                if let LibraryItem::McpServer(s) = item {
                    if s.name == name {
                        return Some(s.clone());
                    }
                }
            }
        }
        None
    })
}

/// Look up a skill by name across every cached library.
pub fn resolve_skill(name: &str) -> Option<LibrarySkillItem> {
    with_cache(|entries| {
        for (_, manifest) in entries {
            for item in &manifest.items {
                if let LibraryItem::Skill(s) = item {
                    if s.name == name {
                        return Some(s.clone());
                    }
                }
            }
        }
        None
    })
}

/// Summary view of every cached library — used by `jarvy library list`.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Reserved for `jarvy library list` (PRD-054 phase 6)
pub struct CachedLibrary {
    pub url: String,
    pub publisher: String,
    pub description: String,
    pub item_counts: (usize, usize, usize), // (ai_hooks, mcp_servers, skills)
}

#[allow(dead_code)] // Reserved for `jarvy library list` (PRD-054 phase 6)
pub fn list_cached() -> Vec<CachedLibrary> {
    with_cache(|entries| {
        entries
            .iter()
            .map(|(url, m)| {
                let mut ai = 0;
                let mut mcp = 0;
                let mut sk = 0;
                for item in &m.items {
                    match item {
                        LibraryItem::AiHook(_) => ai += 1,
                        LibraryItem::McpServer(_) => mcp += 1,
                        LibraryItem::Skill(_) => sk += 1,
                    }
                }
                CachedLibrary {
                    url: url.clone(),
                    publisher: m.publisher.clone(),
                    description: m.description.clone(),
                    item_counts: (ai, mcp, sk),
                }
            })
            .collect()
    })
}

/// Wipe the process cache. Used by tests and by `jarvy library clean`.
#[allow(dead_code)] // Reserved for `jarvy library clean` (PRD-054 phase 6) + tests
pub fn clear_cache() {
    let mut guard = MANIFEST_CACHE.lock().unwrap_or_else(|p| p.into_inner());
    *guard = None;
}

fn with_cache<T>(f: impl FnOnce(&[(String, Manifest)]) -> T) -> T
where
    T: Default,
{
    let guard = MANIFEST_CACHE.lock().unwrap_or_else(|p| p.into_inner());
    match guard.as_deref() {
        Some(entries) => f(entries),
        None => T::default(),
    }
}

/// Refuse a `library_sources` declaration that came from a remote
/// `jarvy.toml`. Used by every consumer at apply time.
pub fn check_origin(
    origin: crate::ai_hooks::ConfigOrigin,
    consumer: &'static str,
) -> Result<(), LibraryError> {
    if origin == crate::ai_hooks::ConfigOrigin::Remote {
        let telemetry_on = crate::observability::telemetry_gate::is_enabled();
        if telemetry_on {
            tracing::warn!(
                event = "library.remote_refused",
                consumer,
                reason = "remote_config_cannot_declare_library_sources",
            );
        }
        return Err(LibraryError::RemoteRefused { consumer });
    }
    Ok(())
}

/// Touch the disk cache mtime for a fetched manifest so `staleness`
/// checks based on `SystemTime::now() - mtime` are accurate even when
/// the cache was hit (not refetched).
#[allow(dead_code)] // Reserved for the TTL-aware cache refresh path
pub fn cache_age(source: &LibrarySource) -> Option<std::time::Duration> {
    let path = cache::manifest_cache_path(&source.url).ok()?;
    let mtime = std::fs::metadata(&path).ok()?.modified().ok()?;
    SystemTime::now().duration_since(mtime).ok()
}

#[allow(dead_code)]
fn _ensure_cache_dir() -> std::io::Result<PathBuf> {
    cache::cache_root()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_appends_manifest_json_to_trailing_slash() {
        assert_eq!(
            canonicalize_manifest_url("https://cdn.example.com/jarvy/"),
            "https://cdn.example.com/jarvy/manifest.json"
        );
    }

    #[test]
    fn canonicalize_passes_explicit_filename_through() {
        assert_eq!(
            canonicalize_manifest_url("https://cdn.example.com/jarvy/manifest.json"),
            "https://cdn.example.com/jarvy/manifest.json"
        );
        assert_eq!(
            canonicalize_manifest_url("https://cdn.example.com/library-v2.json"),
            "https://cdn.example.com/library-v2.json"
        );
    }

    #[test]
    fn check_origin_refuses_remote() {
        let err = check_origin(crate::ai_hooks::ConfigOrigin::Remote, "ai_hooks")
            .expect_err("remote must refuse");
        match err {
            LibraryError::RemoteRefused { consumer } => assert_eq!(consumer, "ai_hooks"),
            other => panic!("expected RemoteRefused, got {other:?}"),
        }
    }

    #[test]
    fn check_origin_allows_local() {
        check_origin(crate::ai_hooks::ConfigOrigin::Local, "ai_hooks").expect("local must pass");
    }

    #[test]
    fn resolve_returns_none_when_cache_empty() {
        clear_cache();
        assert!(resolve_hook("anything").is_none());
        assert!(resolve_mcp_server("anything").is_none());
        assert!(resolve_skill("anything").is_none());
    }
}
