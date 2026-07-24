//! End-to-end cache behavior for the freshness advisory
//! (PRD-057).
//!
//! These tests exercise the on-disk cache format directly via
//! the `jarvy::maintenance::cache` public API. They live under
//! `tests/` (not as unit tests) so the atomic write-then-rename
//! path and the `JARVY_TEST_HOME` interception actually round-
//! trip through the filesystem — a mocked in-memory store would
//! hide the write-rename dance.
//!
//! The env-var mutation is serialized under a shared mutex so
//! parallel test threads don't race each other's `JARVY_TEST_HOME`
//! writes. Every test acquires `env_lock()` up front.

use jarvy::maintenance::cache::{self, CacheEntry, CacheStore, err_entry, ok_entry};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

// Single mutex protects the JARVY_TEST_HOME env var across all
// tests in this file. Cargo runs tests per-file in parallel but
// serializes within a file when tests share a mutex like this.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

#[allow(unsafe_code)] // SAFETY: env mutation serialized by `ENV_LOCK`.
fn scoped_home() -> (TempDir, MutexGuard<'static, ()>) {
    let guard = env_lock();
    let dir = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("JARVY_TEST_HOME", dir.path());
    }
    (dir, guard)
}

fn now() -> SystemTime {
    SystemTime::now()
}

fn cache_file() -> PathBuf {
    cache::cache_path().expect("cache path")
}

// ---------------------------------------------------------------
// Load / save round-trip
// ---------------------------------------------------------------

#[test]
fn save_then_load_returns_same_entries() {
    let (_dir, _g) = scoped_home();
    let mut store = CacheStore::default();
    store.put("brew", "jq", ok_entry("1.7.1", now()));
    store.put("cargo", "ripgrep", ok_entry("14.1.0", now()));
    store.put("npm", "typescript", err_entry("timeout", now()));

    cache::save(&store).expect("save");
    let loaded = cache::load().expect("load");

    assert_eq!(loaded.entries.len(), 3);
    assert_eq!(
        loaded.entries[&CacheStore::key("brew", "jq")]
            .latest
            .as_deref(),
        Some("1.7.1")
    );
    assert_eq!(
        loaded.entries[&CacheStore::key("cargo", "ripgrep")]
            .latest
            .as_deref(),
        Some("14.1.0")
    );
    assert_eq!(
        loaded.entries[&CacheStore::key("npm", "typescript")]
            .backend_error
            .as_deref(),
        Some("timeout")
    );
}

#[test]
fn load_missing_cache_returns_empty_store() {
    let (_dir, _g) = scoped_home();
    let loaded = cache::load().expect("load");
    assert!(
        loaded.entries.is_empty(),
        "fresh home should yield empty cache"
    );
}

#[test]
fn load_corrupt_cache_returns_json_error() {
    let (dir, _g) = scoped_home();
    let jarvy_dir = dir.path().join(".jarvy");
    std::fs::create_dir_all(&jarvy_dir).unwrap();
    std::fs::write(jarvy_dir.join("update-cache.json"), b"{ not json").unwrap();

    let err = cache::load().expect_err("corrupt should Err");
    assert!(
        matches!(err, cache::CacheError::Json(_)),
        "corrupt should return Json error, got: {err}"
    );
}

// ---------------------------------------------------------------
// TTL semantics
// ---------------------------------------------------------------

#[test]
fn success_entry_fresh_within_ttl() {
    let entry = ok_entry("1.0.0", now());
    let ttl = Duration::from_secs(3600);
    assert!(CacheStore::is_fresh(&entry, ttl, now()));
}

#[test]
fn success_entry_stale_past_ttl() {
    let t0 = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let entry = ok_entry("1.0.0", t0);
    assert!(!CacheStore::is_fresh(
        &entry,
        Duration::from_secs(3600),
        t0 + Duration::from_secs(3601)
    ));
}

#[test]
fn error_entry_uses_1h_cap_even_with_24h_ttl() {
    // Documented behavior: error entries cache for min(ttl, 1h)
    // so transient network hiccups don't wedge the check for a
    // full day.
    let t0 = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let entry = err_entry("timeout", t0);
    let ttl = Duration::from_secs(24 * 3600);
    // 59m past — still fresh under the 1h cap.
    assert!(CacheStore::is_fresh(
        &entry,
        ttl,
        t0 + Duration::from_secs(59 * 60)
    ));
    // 61m past — stale under the 1h cap despite the 24h TTL.
    assert!(!CacheStore::is_fresh(
        &entry,
        ttl,
        t0 + Duration::from_secs(61 * 60)
    ));
}

// ---------------------------------------------------------------
// get_fresh: cache-miss semantics
// ---------------------------------------------------------------

#[test]
fn get_fresh_treats_stale_as_miss() {
    let t0 = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let mut store = CacheStore::default();
    store.put("brew", "jq", ok_entry("1.7.1", t0));
    assert!(
        store
            .get_fresh(
                "brew",
                "jq",
                Duration::from_secs(3600),
                t0 + Duration::from_secs(3601)
            )
            .is_none(),
        "stale entry should be indistinguishable from missing"
    );
}

#[test]
fn get_fresh_returns_hit_within_ttl() {
    let t0 = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let mut store = CacheStore::default();
    store.put("brew", "jq", ok_entry("1.7.1", t0));
    let hit = store.get_fresh(
        "brew",
        "jq",
        Duration::from_secs(3600),
        t0 + Duration::from_secs(1800),
    );
    assert!(hit.is_some());
    assert_eq!(hit.unwrap().latest.as_deref(), Some("1.7.1"));
}

#[test]
fn get_fresh_returns_none_for_unknown_key() {
    let store = CacheStore::default();
    assert!(
        store
            .get_fresh("brew", "nonexistent", Duration::from_secs(3600), now())
            .is_none()
    );
}

// ---------------------------------------------------------------
// Atomic write
// ---------------------------------------------------------------

#[test]
fn save_writes_atomically_via_tempfile() {
    // `cache::save` should write to a `.tmp` sibling then rename
    // over the target. Verify no `.tmp` orphan remains after a
    // successful save.
    let (dir, _g) = scoped_home();
    let mut store = CacheStore::default();
    store.put("brew", "jq", ok_entry("1.7.1", now()));
    cache::save(&store).expect("save");

    let jarvy_dir = dir.path().join(".jarvy");
    let entries: Vec<_> = std::fs::read_dir(&jarvy_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();

    // Exactly one file: update-cache.json. Any stray `.tmp` is a
    // regression on the atomic-write contract.
    let has_final = entries.iter().any(|n| n == "update-cache.json");
    let has_tmp = entries
        .iter()
        .any(|n| n.to_string_lossy().ends_with(".tmp"));
    assert!(has_final, "final cache file missing");
    assert!(!has_tmp, "orphan .tmp file left behind: {entries:?}");
}

#[test]
fn save_creates_home_dir_when_missing() {
    let (dir, _g) = scoped_home();
    // Fresh scratch home — `.jarvy/` doesn't exist yet.
    assert!(!dir.path().join(".jarvy").exists());
    let store = CacheStore::default();
    cache::save(&store).expect("save creates parent");
    assert!(dir.path().join(".jarvy").exists());
    assert!(cache_file().exists());
}

// ---------------------------------------------------------------
// Cache-entry constructors
// ---------------------------------------------------------------

#[test]
fn ok_entry_carries_latest_and_no_error() {
    let e: CacheEntry = ok_entry("1.7.1", now());
    assert_eq!(e.latest.as_deref(), Some("1.7.1"));
    assert!(e.backend_error.is_none());
}

#[test]
fn err_entry_carries_kind_and_no_latest() {
    let e: CacheEntry = err_entry("timeout", now());
    assert!(e.latest.is_none());
    assert_eq!(e.backend_error.as_deref(), Some("timeout"));
}

// ---------------------------------------------------------------
// Unix perms
// ---------------------------------------------------------------

#[cfg(unix)]
#[test]
fn save_sets_0600_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, _g) = scoped_home();
    let mut store = CacheStore::default();
    store.put("brew", "jq", ok_entry("1.7.1", now()));
    cache::save(&store).expect("save");
    let mode = std::fs::metadata(cache_file())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "cache file should be chmod 0600");
}
