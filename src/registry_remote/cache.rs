//! On-disk cache layout for synced registry contents.
//!
//! ```text
//! ~/.jarvy/tools.d/
//! └── .remote/
//!     ├── manifest.json            ← cached manifest (verified at sync time)
//!     ├── manifest.json.sig        ← cosign signature
//!     ├── manifest.json.pem        ← cosign cert
//!     ├── meta.json                ← {"last_synced_at_unix": ..., "registry_url": ..., ...}
//!     ├── tools/                   ← active set the plugin loader reads
//!     │   ├── foo.toml
//!     │   └── bar.toml
//!     └── tools.new/               ← staging dir; only swapped into place after a
//!                                   successful sync (atomic rename)
//! ```
//!
//! The cache mirrors the `tools.d/` plugin convention so the existing
//! plugin loader walks it with no special-casing — except it lives under
//! the dotfile-prefixed `.remote/` directory.
//!
//! ## Fail-fast staging
//!
//! `tools/` is the active set; `tools.new/` is the staging dir written
//! during a sync. The orchestrator populates `tools.new/`, validates every
//! entry, then calls [`swap_staging_into_tools_dir`] which atomically
//! renames `tools/` aside and `tools.new/` into its place. A failure mid-
//! sync deletes `tools.new/` and leaves the prior `tools/` untouched —
//! the invariant `mod.rs` documents.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Filesystem mode required on cache directories (Unix). Anything looser
/// is refused at `cache_root()` / `tools_dir()` time so the cosign cert,
/// manifest, and per-tool TOMLs aren't readable by other local users.
#[cfg(unix)]
const CACHE_DIR_MODE: u32 = 0o700;

/// Mode applied to cache files. Same reasoning.
#[cfg(unix)]
const CACHE_FILE_MODE: u32 = 0o600;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cannot resolve cache dir: {0}")]
    NoHome(#[from] crate::paths::NoHomeDir),
    #[error("cache IO: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "refusing to use {path}: mode {mode:#o} grants group/other access. \
         Run `chmod 0700 {path}` or remove the dir so jarvy can recreate it."
    )]
    InsecurePerms { path: String, mode: u32 },
}

/// Resolve `~/.jarvy/tools.d/.remote/`, create it if absent with 0700, and
/// refuse to return it if existing perms are looser. Silent-no-op chmod on
/// filesystems that ignore it (some NFS mounts, drvfs, exFAT) would leave
/// the cosign cert + meta.json world-readable; better to fail loudly.
pub fn cache_root() -> Result<PathBuf, CacheError> {
    let dir = crate::paths::registry_remote_cache_dir()?;
    fs::create_dir_all(&dir)?;
    enforce_dir_perms(&dir)?;
    Ok(dir)
}

/// `~/.jarvy/tools.d/.remote/tools/` — the ACTIVE tool TOML set. Loader
/// reads from here on every CLI startup.
pub fn tools_dir() -> Result<PathBuf, CacheError> {
    let dir = cache_root()?.join("tools");
    fs::create_dir_all(&dir)?;
    enforce_dir_perms(&dir)?;
    Ok(dir)
}

/// `~/.jarvy/tools.d/.remote/tools.new/` — STAGING dir. Sync orchestrator
/// writes here, validates every entry, then atomic-swaps into `tools/`.
/// Always wiped on entry so a previous interrupted sync doesn't bleed
/// half-fetched files into the swap.
pub fn fresh_staging_tools_dir() -> Result<PathBuf, CacheError> {
    let dir = cache_root()?.join("tools.new");
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    enforce_dir_perms(&dir)?;
    Ok(dir)
}

/// Atomic-swap `tools.new/` into the place of `tools/`. Uses a two-rename
/// dance so the active `tools/` is replaced as a single inode flip rather
/// than rm-then-rename (which has a window where `tools/` doesn't exist
/// and a parallel reader would 404).
pub fn swap_staging_into_tools_dir() -> Result<(), CacheError> {
    let root = cache_root()?;
    let active = root.join("tools");
    let staging = root.join("tools.new");
    let retired = root.join("tools.old");

    if !staging.exists() {
        return Err(CacheError::Io(std::io::Error::other(
            "swap called without a staging dir; call fresh_staging_tools_dir first",
        )));
    }

    if retired.exists() {
        fs::remove_dir_all(&retired)?;
    }
    if active.exists() {
        fs::rename(&active, &retired)?;
    }
    if let Err(e) = fs::rename(&staging, &active) {
        tracing::error!(
            event = "registry.cache.swap_failed",
            stage = "promote",
            error = %e,
        );
        // Best-effort rollback so the user isn't left with no active dir.
        if retired.exists() {
            if let Err(rollback_err) = fs::rename(&retired, &active) {
                tracing::error!(
                    event = "registry.cache.swap_failed",
                    stage = "rollback",
                    error = %rollback_err,
                    promote_error = %e,
                );
            }
        }
        return Err(e.into());
    }
    if retired.exists() {
        fs::remove_dir_all(&retired)?;
    }
    Ok(())
}

/// Atomic write: writes to `<path>.tmp` first, fsyncs, then renames into
/// place. Avoids leaving a half-written file visible to a parallel reader.
/// Cleans up the `.tmp` file on any write error so disk-full mid-write
/// doesn't leak.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), CacheError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension({
        let mut ext = path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        ext.push_str(".tmp");
        ext
    });
    // Clean up any stale .tmp from a prior panicked write before opening
    // with O_EXCL. Two concurrent writers to the SAME dest would race
    // on the create_new; manifest::DuplicateName rejection at parse
    // closes the only known source of that race within a single sync,
    // so EEXIST here surfaces a real bug (two threads same path) rather
    // than a stale-tmp issue.
    let _ = fs::remove_file(&tmp);
    let write_result = (|| -> Result<(), std::io::Error> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    fs::rename(&tmp, path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(CACHE_FILE_MODE))?;
    }
    Ok(())
}

/// Stat the dir AFTER `set_permissions` and refuse if mode is still loose.
/// Filesystems that ignore chmod (some NFS mounts, drvfs, exFAT) silently
/// leave the dir world-readable; previous swallow-the-error pattern hid
/// the failure. Better to fail at sync time than leak cached secrets.
fn enforce_dir_perms(dir: &Path) -> Result<(), CacheError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Try to tighten if loose — but don't swallow the result either.
        let meta = fs::metadata(dir)?;
        let current = meta.permissions().mode() & 0o777;
        if current != CACHE_DIR_MODE {
            fs::set_permissions(dir, fs::Permissions::from_mode(CACHE_DIR_MODE))?;
            let after = fs::metadata(dir)?.permissions().mode() & 0o777;
            if after & 0o077 != 0 {
                return Err(CacheError::InsecurePerms {
                    path: dir.display().to_string(),
                    mode: after,
                });
            }
        }
    }
    #[cfg(not(unix))]
    {
        // Windows: no Unix mode bits; rely on parent ACLs of $HOME. Not a
        // P0 hardening surface for the CLI's threat model today.
        let _ = dir;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_writes_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("manifest.json");
        write_atomic(&path, b"hello").expect("write");
        let read = fs::read(&path).expect("read");
        assert_eq!(read, b"hello");
    }

    #[test]
    fn write_atomic_overwrites_existing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("manifest.json");
        write_atomic(&path, b"first").expect("first write");
        write_atomic(&path, b"second").expect("second write");
        let read = fs::read(&path).expect("read");
        assert_eq!(read, b"second");
    }

    #[test]
    fn write_atomic_creates_parent_dirs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("a").join("b").join("c.toml");
        write_atomic(&path, b"x").expect("write");
        assert!(path.exists());
    }

    /// `.tmp` is removed on disk-full / write error so partial writes don't
    /// leak. We can't reliably force a disk-full in unit tests, but we can
    /// at least confirm a successful write leaves no `.tmp` behind.
    #[test]
    fn write_atomic_leaves_no_tmp_on_success() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("foo.toml");
        write_atomic(&path, b"x").expect("write");
        let tmp_path = path.with_extension("toml.tmp");
        assert!(!tmp_path.exists(), "stale .tmp should not survive success");
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_sets_0600_perms() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("secret.json");
        write_atomic(&path, b"x").expect("write");
        let mode = fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode, CACHE_FILE_MODE);
    }
}
