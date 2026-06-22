//! On-disk cache layout for synced registry contents.
//!
//! ```text
//! ~/.jarvy/tools.d/
//! └── .remote/
//!     ├── manifest.json            ← cached manifest (verified at sync time)
//!     ├── manifest.json.sig        ← cosign signature
//!     ├── manifest.json.pem        ← cosign cert
//!     ├── meta.json                ← {"last_synced_at": "<iso8601>", "registry_url": "..."}
//!     └── tools/
//!         ├── foo.toml
//!         └── bar.toml
//! ```
//!
//! The cache mirrors the `tools.d/` plugin convention so the existing
//! plugin loader can walk it with no special-casing — except it lives
//! under the dotfile-prefixed `.remote/` directory, which the plugin
//! loader is taught to walk in a sibling commit.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cannot resolve cache dir: {0}")]
    NoHome(#[from] crate::paths::NoHomeDir),
    #[error("cache IO: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolve `~/.jarvy/tools.d/.remote/` and ensure it exists with 0700
/// perms on Unix. Cache writes leak intermediate state — keep them
/// user-only readable.
pub fn cache_root() -> Result<PathBuf, CacheError> {
    let dir = crate::paths::registry_remote_cache_dir()?;
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// `~/.jarvy/tools.d/.remote/tools/`.
pub fn tools_dir() -> Result<PathBuf, CacheError> {
    let dir = cache_root()?.join("tools");
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// Atomic write: writes to `<path>.tmp` first, fsyncs, then renames into
/// place. Avoids leaving a half-written manifest visible to a parallel
/// `jarvy setup`.
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
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Write the sync-completion marker. Stored as JSON so the future
/// `jarvy registry status` command can read it without re-parsing the
/// manifest.
pub fn write_meta(meta_json: &str) -> Result<(), CacheError> {
    let path = cache_root()?.join("meta.json");
    write_atomic(&path, meta_json.as_bytes())
}

/// Clear the tools directory before re-populating it. A subsequent sync
/// then writes only the manifest's current tool set, so a tool removed
/// upstream disappears locally on next sync.
pub fn wipe_tools_dir() -> Result<(), CacheError> {
    let dir = tools_dir()?;
    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(&path)?;
            }
        }
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
}
