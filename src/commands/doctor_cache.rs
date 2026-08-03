//! Disk-backed version-probe cache for `jarvy doctor`.
//!
//! Stored at `~/.jarvy/doctor-cache.json`, mirroring the
//! `maintenance/cache.rs` pattern (schema version, atomic 0600
//! tmp-write + rename, `JARVY_TEST_HOME` hook, corrupt file →
//! empty store).
//!
//! Validity is stat-based rather than TTL-based for successful
//! probes: an entry is served only while the resolved binary path,
//! mtime, and size all still match — so upgrading a tool
//! invalidates its entry automatically with no staleness window.
//! Negative probes (binary present but version undetectable, or
//! binary missing) additionally expire after 1 hour so a transient
//! timeout doesn't hide a tool forever.
//!
//! `--fresh` bypasses reads but still writes, refreshing the cache.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const CACHE_SCHEMA_VERSION: u32 = 1;
pub const CACHE_FILENAME: &str = "doctor-cache.json";
/// Negative probes re-run after this long even when the binary's
/// stat signature is unchanged.
const NEGATIVE_TTL_SECS: u64 = 3600;

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

/// Binary identity at probe time: (mtime unix seconds, byte size).
pub type StatSig = (i64, u64);

/// One cached probe result for a command.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProbeEntry {
    /// Resolved binary path at probe time; `None` = binary was missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtime_unix: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Raw probe output; `None` = probe failed / timed out / no output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    pub cached_at_unix: u64,
}

/// On-disk cache document.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProbeStore {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub entries: BTreeMap<String, ProbeEntry>,
}

fn default_schema_version() -> u32 {
    CACHE_SCHEMA_VERSION
}

impl Default for ProbeStore {
    fn default() -> Self {
        Self {
            schema_version: CACHE_SCHEMA_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

/// Pure validity check — takes the current stat signature so tests
/// don't need a real filesystem.
///
/// Valid when the recorded path matches the current path AND:
/// - path present: current stat matches the recorded mtime+size, and
///   negative entries (`output: None`) are within [`NEGATIVE_TTL_SECS`];
/// - path absent (binary missing then and now): within the negative TTL.
pub fn entry_valid(
    entry: &ProbeEntry,
    current_path: Option<&str>,
    current_stat: Option<StatSig>,
    now_secs: u64,
) -> bool {
    if entry.path.as_deref() != current_path {
        return false;
    }
    let within_negative_ttl = now_secs.saturating_sub(entry.cached_at_unix) < NEGATIVE_TTL_SECS;
    match current_path {
        Some(_) => {
            let Some((mtime, size)) = current_stat else {
                return false;
            };
            if entry.mtime_unix != Some(mtime) || entry.size != Some(size) {
                return false;
            }
            entry.output.is_some() || within_negative_ttl
        }
        None => within_negative_ttl,
    }
}

/// Stat the binary; `None` when unreadable (treated as invalid).
fn stat_sig(path: &str) -> Option<StatSig> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Some((mtime, meta.len()))
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn cache_path() -> Result<PathBuf, CacheError> {
    Ok(jarvy_dir()?.join(CACHE_FILENAME))
}

fn jarvy_dir() -> Result<PathBuf, CacheError> {
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

/// Load from disk. Missing file → empty store. Corrupt file → empty
/// store + `Err(Json)` so the caller can emit telemetry; the empty
/// store is still usable.
pub fn load() -> Result<ProbeStore, CacheError> {
    let path = cache_path()?;
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(CacheError::Json),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ProbeStore::default()),
        Err(e) => Err(CacheError::Io(e)),
    }
}

/// Persist atomically: 0600 tmp-write → rename → re-assert mode.
pub fn save(store: &ProbeStore) -> Result<(), CacheError> {
    let path = cache_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(CacheError::Io)?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(store).map_err(CacheError::Json)?;
    crate::paths::write_0600(&tmp, &body).map_err(CacheError::Io)?;
    fs::rename(&tmp, &path).map_err(CacheError::Io)?;
    crate::paths::apply_secure_mode_0600(&path);
    Ok(())
}

struct CacheState {
    store: ProbeStore,
    dirty: bool,
    fresh_mode: bool,
    hits: u64,
    misses: u64,
}

static STATE: OnceLock<Mutex<CacheState>> = OnceLock::new();

fn state() -> &'static Mutex<CacheState> {
    STATE.get_or_init(|| {
        let store = match load() {
            Ok(s) if s.schema_version == CACHE_SCHEMA_VERSION => s,
            Ok(_) => ProbeStore::default(),
            Err(e) => {
                if crate::observability::telemetry_gate::is_enabled() {
                    tracing::warn!(
                        event = "doctor.probe_cache_read_failed",
                        error_kind = error_kind(&e),
                    );
                }
                ProbeStore::default()
            }
        };
        Mutex::new(CacheState {
            store,
            dirty: false,
            fresh_mode: false,
            hits: 0,
            misses: 0,
        })
    })
}

fn error_kind(e: &CacheError) -> &'static str {
    match e {
        CacheError::NoHome => "no_home",
        CacheError::Io(_) => "io",
        CacheError::Json(_) => "json",
    }
}

/// `--fresh`: bypass cache reads for this process (writes still happen).
pub fn set_fresh(fresh: bool) {
    if let Ok(mut st) = state().lock() {
        st.fresh_mode = fresh;
    }
}

/// Cache-aware version probe. `path` is the already-resolved binary
/// path (doctor computes it anyway for display); it doubles as the
/// stat key. Falls back to a direct probe on any lock failure.
pub fn version_output(command: &str, path: Option<&str>) -> Option<String> {
    let now = unix_now_secs();
    if let Ok(mut st) = state().lock() {
        let cached = (!st.fresh_mode)
            .then(|| st.store.entries.get(command))
            .flatten()
            .filter(|entry| entry_valid(entry, path, path.and_then(stat_sig), now))
            .map(|entry| entry.output.clone());
        if let Some(output) = cached {
            st.hits += 1;
            return output;
        }
    }
    // Probe WITHOUT the lock held — a server-dialing CLI can burn
    // 3 × 5s in timeouts and must not block other cache users.
    let probed = crate::tools::common::cmd_version_output(command);
    if let Ok(mut st) = state().lock() {
        st.misses += 1;
        let stat = path.and_then(stat_sig);
        st.store.entries.insert(
            command.to_string(),
            ProbeEntry {
                path: path.map(str::to_string),
                mtime_unix: stat.map(|(m, _)| m),
                size: stat.map(|(_, s)| s),
                output: probed.clone(),
                cached_at_unix: now,
            },
        );
        st.dirty = true;
    }
    probed
}

/// Save if dirty and emit the per-run summary event. Called once at
/// the end of the doctor tool pass; a no-probe run stays silent.
pub fn persist() {
    let Ok(mut st) = state().lock() else { return };
    if st.hits == 0 && st.misses == 0 {
        return;
    }
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "doctor.probe_cache",
            hits = st.hits,
            misses = st.misses,
            fresh = st.fresh_mode,
        );
    }
    if st.dirty {
        if let Err(e) = save(&st.store) {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "doctor.probe_cache_write_failed",
                    error_kind = error_kind(&e),
                );
            }
        } else {
            st.dirty = false;
        }
    }
    st.hits = 0;
    st.misses = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        path: Option<&str>,
        stat: Option<StatSig>,
        output: Option<&str>,
        cached_at: u64,
    ) -> ProbeEntry {
        ProbeEntry {
            path: path.map(str::to_string),
            mtime_unix: stat.map(|(m, _)| m),
            size: stat.map(|(_, s)| s),
            output: output.map(str::to_string),
            cached_at_unix: cached_at,
        }
    }

    const T0: u64 = 1_700_000_000;
    const SIG: StatSig = (1_650_000_000, 4096);

    #[test]
    fn valid_when_stat_matches() {
        let e = entry(Some("/usr/bin/jq"), Some(SIG), Some("jq-1.7.1"), T0);
        // No TTL for positive entries — a year later is still valid.
        assert!(entry_valid(
            &e,
            Some("/usr/bin/jq"),
            Some(SIG),
            T0 + 365 * 86400
        ));
    }

    #[test]
    fn invalid_when_binary_upgraded() {
        let e = entry(Some("/usr/bin/jq"), Some(SIG), Some("jq-1.7.1"), T0);
        // mtime changed
        assert!(!entry_valid(
            &e,
            Some("/usr/bin/jq"),
            Some((SIG.0 + 60, SIG.1)),
            T0 + 1
        ));
        // size changed
        assert!(!entry_valid(
            &e,
            Some("/usr/bin/jq"),
            Some((SIG.0, SIG.1 + 1)),
            T0 + 1
        ));
    }

    #[test]
    fn invalid_when_path_moved_or_gone() {
        let e = entry(Some("/usr/bin/jq"), Some(SIG), Some("jq-1.7.1"), T0);
        assert!(!entry_valid(
            &e,
            Some("/opt/homebrew/bin/jq"),
            Some(SIG),
            T0
        ));
        assert!(!entry_valid(&e, None, None, T0));
        // Path present but no longer statable (race with uninstall).
        assert!(!entry_valid(&e, Some("/usr/bin/jq"), None, T0));
    }

    #[test]
    fn negative_entry_expires_after_hour() {
        // Binary present, probe failed (argocd-style timeout).
        let e = entry(Some("/usr/bin/argocd"), Some(SIG), None, T0);
        assert!(entry_valid(
            &e,
            Some("/usr/bin/argocd"),
            Some(SIG),
            T0 + 3599
        ));
        assert!(!entry_valid(
            &e,
            Some("/usr/bin/argocd"),
            Some(SIG),
            T0 + 3600
        ));
    }

    #[test]
    fn missing_binary_entry_expires_after_hour() {
        let e = entry(None, None, None, T0);
        assert!(entry_valid(&e, None, None, T0 + 3599));
        assert!(!entry_valid(&e, None, None, T0 + 3600));
        // Binary got installed since — must re-probe immediately.
        assert!(!entry_valid(&e, Some("/usr/bin/new"), Some(SIG), T0 + 1));
    }

    #[test]
    fn round_trip_serde() {
        let mut store = ProbeStore::default();
        store.entries.insert(
            "jq".into(),
            entry(Some("/usr/bin/jq"), Some(SIG), Some("jq-1.7.1"), T0),
        );
        store
            .entries
            .insert("ghost".into(), entry(None, None, None, T0));
        let text = serde_json::to_string(&store).unwrap();
        let back: ProbeStore = serde_json::from_str(&text).unwrap();
        assert_eq!(back.schema_version, CACHE_SCHEMA_VERSION);
        assert_eq!(back.entries.len(), 2);
        assert_eq!(back.entries["jq"].output.as_deref(), Some("jq-1.7.1"));
        assert!(back.entries["ghost"].path.is_none());
    }
}
