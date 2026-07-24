//! Advisory lockfile for the freshness refresher (PRD-057).
//!
//! Prevents two concurrent `jarvy check-updates --refresh` /
//! `--background` runs from stomping on each other's cache
//! writes. Lock lives at `~/.jarvy/update-cache.lock`; contents
//! record the owning PID and start time so a fresh acquirer can
//! distinguish "another refresher is genuinely running" from
//! "previous refresher crashed without cleanup" purely by wall-
//! clock age.
//!
//! The stale-reclaim threshold is 1 hour, chosen to comfortably
//! exceed the [`super::backends::BACKEND_TIMEOUT`] × N-tools
//! ceiling: no legitimate refresh should take that long, and if
//! one does, the process wedged and its cache slot deserves to
//! be reclaimed rather than deadlock every future setup.
//!
//! Not a real cross-process lock — a well-behaved refresher
//! releases via RAII on `Drop`. A crashed refresher leaves the
//! file behind and the next run's age check handles it. This is
//! coarse but predictable and needs zero platform-specific
//! syscall wiring.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::cache::{CacheError, lock_path};

/// Stale-reclaim age. Any lock file older than this is treated
/// as belonging to a dead process and gets reclaimed.
pub const STALE_AGE: Duration = Duration::from_secs(3600);

/// Outcome of trying to acquire the refresher lock.
#[derive(Debug)]
pub enum LockOutcome {
    /// We hold the lock; drop the `RefreshLock` to release.
    Acquired(RefreshLock),
    /// A live refresher already holds the lock. Caller should
    /// bail with `maintenance.refresh_already_running` telemetry.
    AlreadyRunning { held_by_pid: u32, age_seconds: u64 },
    /// We couldn't figure out where the lock file lives (no HOME
    /// resolution). Refresher should skip and log
    /// `maintenance.cache_read_failed { error_kind = "no_home" }`.
    NoHome,
    /// I/O failure reading or writing the lock file. Non-fatal —
    /// the refresher can still run, it just won't guard against
    /// concurrency. Caller decides whether to proceed.
    IoError(std::io::Error),
}

/// RAII handle that removes the lock file on drop. `forget()` on
/// this struct is a bug — the file will stay behind and force
/// every next run to wait for the stale threshold.
#[derive(Debug)]
pub struct RefreshLock {
    path: PathBuf,
    /// Set when the file we wrote may not be the file we hold:
    /// if a racing writer overwrote our file after we won the
    /// race, we skip cleanup so we don't delete their lock.
    fingerprint: LockFile,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LockFile {
    pid: u32,
    started_at_unix: u64,
    /// Random discriminator so `Drop` can verify we still own the
    /// file it points to. Cheap paranoia against a concurrent
    /// reclaim path.
    nonce: u64,
}

impl Drop for RefreshLock {
    fn drop(&mut self) {
        // Only remove the lock if the on-disk file still matches
        // our fingerprint — protects against the racy case where
        // a stale-reclaimer has already taken over.
        let Ok(text) = fs::read_to_string(&self.path) else {
            return;
        };
        let Ok(current) = serde_json::from_str::<LockFile>(&text) else {
            return;
        };
        if current.nonce == self.fingerprint.nonce {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Try to take the refresher lock. See [`LockOutcome`] for the
/// full decision tree.
pub fn acquire() -> LockOutcome {
    let path = match lock_path() {
        Ok(p) => p,
        Err(CacheError::NoHome) => return LockOutcome::NoHome,
        Err(CacheError::Io(e)) => return LockOutcome::IoError(e),
        Err(_) => return LockOutcome::NoHome,
    };
    if let Some(parent) = path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        return LockOutcome::IoError(e);
    }

    if let Ok(text) = fs::read_to_string(&path)
        && let Ok(existing) = serde_json::from_str::<LockFile>(&text)
    {
        let age = age_seconds(existing.started_at_unix);
        if age < STALE_AGE.as_secs() {
            return LockOutcome::AlreadyRunning {
                held_by_pid: existing.pid,
                age_seconds: age,
            };
        }
        // Stale — fall through and overwrite.
    }

    let file = LockFile {
        pid: std::process::id(),
        started_at_unix: unix_secs_now(),
        nonce: fresh_nonce(),
    };
    let body = match serde_json::to_string(&file) {
        Ok(b) => b,
        Err(_) => {
            return LockOutcome::IoError(std::io::Error::other("serialize LockFile"));
        }
    };
    if let Err(e) = fs::write(&path, body) {
        return LockOutcome::IoError(e);
    }
    LockOutcome::Acquired(RefreshLock {
        path,
        fingerprint: file,
    })
}

fn unix_secs_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn age_seconds(started_at_unix: u64) -> u64 {
    unix_secs_now().saturating_sub(started_at_unix)
}

/// Cheap non-cryptographic nonce — uses the current time in
/// nanoseconds XORed with the process id. Collisions between
/// concurrent acquirers on the same host are the only failure
/// mode; both would then drop early rather than delete each
/// other's file. Not a security-critical value.
fn fresh_nonce() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (std::process::id() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;

    /// Global env-mutation guard. `JARVY_TEST_HOME` is process-
    /// wide and `cargo test` runs tests in parallel by default;
    /// without serialization the acquire tests race each other's
    /// lock files. One mutex → tests still fast, no flaking.
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    /// Sets `JARVY_TEST_HOME` under the mutex, returning both the
    /// temp dir handle (dropped at end of scope) and the mutex
    /// guard (holds the env-var stable across `acquire()` calls).
    #[allow(unsafe_code)] // SAFETY: env mutation is serialized by
    // `ENV_GUARD`, so no other thread reads/writes the same var
    // while this function holds the lock (Rust 2024's
    // `set_var` guarantee).
    fn scoped_home() -> (tempfile::TempDir, std::sync::MutexGuard<'static, ()>) {
        let guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let dir = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("JARVY_TEST_HOME", dir.path());
        }
        (dir, guard)
    }

    #[test]
    fn acquire_returns_lock_when_absent() {
        let (_dir, _g) = scoped_home();
        let outcome = acquire();
        assert!(matches!(outcome, LockOutcome::Acquired(_)));
    }

    #[test]
    fn drop_releases_lock() {
        let (_dir, _g) = scoped_home();
        let path = lock_path().unwrap();
        {
            let outcome = acquire();
            assert!(matches!(outcome, LockOutcome::Acquired(_)));
            assert!(path.exists());
        }
        assert!(!path.exists(), "Drop should remove lock file");
    }

    #[test]
    fn already_running_when_fresh_lock_present() {
        let (_dir, _g) = scoped_home();
        let file = LockFile {
            pid: 99999,
            started_at_unix: unix_secs_now(),
            nonce: 1,
        };
        let path = lock_path().unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();
        match acquire() {
            LockOutcome::AlreadyRunning { held_by_pid, .. } => {
                assert_eq!(held_by_pid, 99999);
            }
            other => panic!("expected AlreadyRunning, got {other:?}"),
        }
    }

    #[test]
    fn stale_lock_is_reclaimed() {
        let (_dir, _g) = scoped_home();
        let file = LockFile {
            pid: 99999,
            started_at_unix: unix_secs_now().saturating_sub(STALE_AGE.as_secs() + 60),
            nonce: 1,
        };
        let path = lock_path().unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();
        assert!(matches!(acquire(), LockOutcome::Acquired(_)));
    }

    #[test]
    fn age_calc_saturates() {
        assert_eq!(age_seconds(u64::MAX), 0);
        let _ = Duration::from_secs(0);
    }
}
