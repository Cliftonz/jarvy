//! On-disk freshness cache (PRD-057).
//!
//! Stored at `~/.jarvy/update-cache.json`. Setup reads it (never
//! writes); `jarvy check-updates --refresh` writes it. TTL check
//! lives here so callers can trust `entries()` to return only
//! fresh data.
//!
//! Corrupt cache is treated as empty — every write is best-effort.
//! Missing home dir returns `Err(CacheError::NoHome)`; callers
//! typically log and continue with an empty in-memory cache.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Cache-file schema version. Bump when the JSON shape changes so
/// older versions of jarvy don't misparse newer files.
pub const CACHE_SCHEMA_VERSION: u32 = 1;

/// Filename inside `~/.jarvy/`.
pub const CACHE_FILENAME: &str = "update-cache.json";
/// Advisory-lock filename co-located with the cache.
#[allow(dead_code)] // Reserved for the concurrent-refresher lock
// (PRD-057 § Setup phase wiring). Wired in a
// follow-up task once the lockfile guard lands.
pub const LOCK_FILENAME: &str = "update-cache.lock";

#[derive(Debug)]
pub enum CacheError {
    NoHome,
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoHome => f.write_str("cannot resolve home directory"),
            Self::Io(e) => write!(f, "cache I/O: {}", e),
            Self::Json(e) => write!(f, "cache JSON: {}", e),
        }
    }
}

impl std::error::Error for CacheError {}

/// One cached lookup. Errors ARE cached (with a shorter TTL) so a
/// transient network blip doesn't force a re-fetch on every setup.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CacheEntry {
    /// Latest version reported by the backend. `None` when
    /// `backend_error` is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    /// Unix epoch seconds when this entry was written.
    pub checked_at_unix: u64,
    /// Bounded `BackendError::kind()` label when the lookup failed.
    /// Cached with a shorter TTL — see [`CacheStore::is_fresh`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_error: Option<String>,
}

/// On-disk cache document.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CacheStore {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub entries: BTreeMap<String, CacheEntry>,
}

fn default_schema_version() -> u32 {
    CACHE_SCHEMA_VERSION
}

impl Default for CacheStore {
    fn default() -> Self {
        Self {
            schema_version: CACHE_SCHEMA_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

impl CacheStore {
    /// Cache key format: `<backend>:<pkg_id>`. Same package installed
    /// via two managers gets two entries — that's intentional; the
    /// version reported by brew for `jq` isn't necessarily the same
    /// as the one reported by apt.
    pub fn key(backend: &str, pkg_id: &str) -> String {
        format!("{backend}:{pkg_id}")
    }

    /// Return `true` when the entry is still fresh under `ttl`.
    /// Successful entries use the full TTL; error entries use
    /// `min(ttl, 1h)` so transient failures don't wedge the cache
    /// for a full day.
    pub fn is_fresh(entry: &CacheEntry, ttl: Duration, now: SystemTime) -> bool {
        let now_secs = now
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let effective = if entry.backend_error.is_some() {
            ttl.min(Duration::from_secs(3600))
        } else {
            ttl
        };
        now_secs
            .saturating_sub(entry.checked_at_unix)
            .lt(&effective.as_secs())
    }

    /// Fetch a fresh entry from the store, respecting the TTL.
    /// Returns `None` when absent OR stale — callers treat both as
    /// a cache miss.
    pub fn get_fresh(
        &self,
        backend: &str,
        pkg_id: &str,
        ttl: Duration,
        now: SystemTime,
    ) -> Option<&CacheEntry> {
        let entry = self.entries.get(&Self::key(backend, pkg_id))?;
        if Self::is_fresh(entry, ttl, now) {
            Some(entry)
        } else {
            None
        }
    }

    /// Insert or overwrite an entry.
    pub fn put(&mut self, backend: &str, pkg_id: &str, entry: CacheEntry) {
        self.entries.insert(Self::key(backend, pkg_id), entry);
    }
}

/// Return the canonical cache-file path (`~/.jarvy/update-cache.json`).
pub fn cache_path() -> Result<PathBuf, CacheError> {
    Ok(jarvy_dir()?.join(CACHE_FILENAME))
}

/// Return the canonical lockfile path.
#[allow(dead_code)]
pub fn lock_path() -> Result<PathBuf, CacheError> {
    Ok(jarvy_dir()?.join(LOCK_FILENAME))
}

fn jarvy_dir() -> Result<PathBuf, CacheError> {
    // Test hook — mirrors the pattern used by other subsystems so
    // integration tests don't stomp on the user's real cache.
    if let Ok(v) = std::env::var("JARVY_TEST_HOME") {
        return Ok(PathBuf::from(v).join(".jarvy"));
    }
    let home = home_dir().ok_or(CacheError::NoHome)?;
    Ok(home.join(".jarvy"))
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
}

/// Load the cache from disk. Missing file → empty store. Corrupt
/// file → empty store + `Err(CacheError::Json)` so the caller can
/// emit `maintenance.cache_read_failed` telemetry; the empty store
/// is still usable.
pub fn load() -> Result<CacheStore, CacheError> {
    let path = cache_path()?;
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(CacheError::Json),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(CacheStore::default()),
        Err(e) => Err(CacheError::Io(e)),
    }
}

/// Persist `store` to disk atomically via write-to-temp + rename.
/// Best-effort — permission errors etc. bubble up; callers log
/// `maintenance.cache_write_failed` and continue.
pub fn save(store: &CacheStore) -> Result<(), CacheError> {
    let path = cache_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(CacheError::Io)?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(store).map_err(CacheError::Json)?;
    // Create the tmp with 0600 from the start (avoid umask 0644 window).
    crate::paths::write_0600(&tmp, &body).map_err(CacheError::Io)?;
    fs::rename(&tmp, &path).map_err(CacheError::Io)?;
    crate::paths::apply_secure_mode_0600(&path);
    Ok(())
}

/// Convenience: build a `CacheEntry` for a successful lookup.
pub fn ok_entry(latest: impl Into<String>, now: SystemTime) -> CacheEntry {
    CacheEntry {
        latest: Some(latest.into()),
        checked_at_unix: unix_secs(now),
        backend_error: None,
    }
}

/// Convenience: build a `CacheEntry` for a backend failure.
pub fn err_entry(kind: &str, now: SystemTime) -> CacheEntry {
    CacheEntry {
        latest: None,
        checked_at_unix: unix_secs(now),
        backend_error: Some(kind.to_string()),
    }
}

fn unix_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_700_000_000)
    }

    #[test]
    fn fresh_when_within_ttl() {
        let entry = ok_entry("1.0.0", t0());
        assert!(CacheStore::is_fresh(
            &entry,
            Duration::from_secs(3600),
            t0() + Duration::from_secs(1800)
        ));
    }

    #[test]
    fn stale_when_past_ttl() {
        let entry = ok_entry("1.0.0", t0());
        assert!(!CacheStore::is_fresh(
            &entry,
            Duration::from_secs(3600),
            t0() + Duration::from_secs(3601)
        ));
    }

    #[test]
    fn error_entry_uses_shorter_ttl() {
        let entry = err_entry("timeout", t0());
        // 30m past — fresh under both the full TTL and the 1h error cap.
        assert!(CacheStore::is_fresh(
            &entry,
            Duration::from_secs(24 * 3600),
            t0() + Duration::from_secs(1800)
        ));
        // 2h past — error cap says stale even though the full TTL is 24h.
        assert!(!CacheStore::is_fresh(
            &entry,
            Duration::from_secs(24 * 3600),
            t0() + Duration::from_secs(2 * 3600 + 1)
        ));
    }

    #[test]
    fn get_fresh_returns_none_for_stale() {
        let mut store = CacheStore::default();
        store.put("brew", "jq", ok_entry("1.0.0", t0()));
        assert!(
            store
                .get_fresh(
                    "brew",
                    "jq",
                    Duration::from_secs(3600),
                    t0() + Duration::from_secs(3601)
                )
                .is_none()
        );
    }

    #[test]
    fn round_trip_serde() {
        let mut store = CacheStore::default();
        store.put("brew", "jq", ok_entry("1.7.1", t0()));
        store.put("cargo", "ripgrep", err_entry("timeout", t0()));
        let text = serde_json::to_string(&store).unwrap();
        let back: CacheStore = serde_json::from_str(&text).unwrap();
        assert_eq!(back.entries.len(), 2);
        assert_eq!(
            back.entries[&CacheStore::key("brew", "jq")]
                .latest
                .as_deref(),
            Some("1.7.1")
        );
    }
}
