//! On-disk cache for fetched library manifests.
//!
//! Layout:
//!
//! ```text
//! ~/.jarvy/library.d/
//!   <sha256-of-url>/
//!     manifest.json
//! ```
//!
//! `<sha256-of-url>` because URLs can contain characters
//! (`?`, `/`, `:`) that are awkward in filesystem paths. The hash is
//! collision-free and reversible only via lookup against the
//! in-process cache's URL list.
//!
//! All writes go through `write_manifest` which uses an atomic
//! rename (`.new` → final) so a partial write never leaves the cache
//! in a corrupt state.

use super::manifest::Manifest;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

/// `~/.jarvy/library.d/`. Created on first call.
pub fn cache_root() -> std::io::Result<PathBuf> {
    let home = dirs_home().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no home directory; set HOME or pass --library-cache",
        )
    })?;
    let root = home.join(".jarvy").join("library.d");
    if !root.exists() {
        std::fs::create_dir_all(&root)?;
    }
    Ok(root)
}

/// Path to `manifest.json` for a given source URL.
pub fn manifest_cache_path(url: &str) -> std::io::Result<PathBuf> {
    let hash = sha256_hex(url.as_bytes());
    let dir = cache_root()?.join(&hash);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir.join("manifest.json"))
}

/// Content-addressed companion cache slot:
/// `~/.jarvy/library.d/companions/<sha256>`.
///
/// Keyed by the manifest-pinned content hash (NOT the URL) — pinned
/// content is immutable, so two libraries referencing the same artifact
/// share one cache entry, and a hit is verifiable by re-hashing. The
/// caller (`companion::fetch_verified`) validates the pin is 64 hex
/// chars before it ever reaches this path.
pub fn companion_cache_path(sha256_lower: &str) -> std::io::Result<PathBuf> {
    debug_assert!(
        sha256_lower.len() == 64 && sha256_lower.bytes().all(|b| b.is_ascii_hexdigit()),
        "companion cache key must be a validated sha256 hex digest"
    );
    let dir = cache_root()?.join("companions");
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir.join(sha256_lower))
}

/// Atomic byte write: `<path>.new` → fsync → rename. Crash mid-write
/// leaves the previous cache entry (or nothing) intact. Same shape as
/// [`write_manifest`], minus the serialization.
pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("new");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Read a cached manifest. `Ok(None)` if the file doesn't exist;
/// `Err` for permission / parse failures.
pub fn read_manifest(path: &Path) -> std::io::Result<Option<Manifest>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cache manifest parse failed: {e}"),
        )
    })?;
    Ok(Some(manifest))
}

/// Atomic write: serialize to `<path>.new`, fsync, then rename to
/// `<path>`. Crash mid-write leaves the previous cached manifest
/// intact.
pub fn write_manifest(path: &Path, manifest: &Manifest) -> std::io::Result<()> {
    let tmp = path.with_extension("new");
    let serialized = serde_json::to_vec_pretty(manifest).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("serialize manifest: {e}"),
        )
    })?;
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&serialized)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Wipe every cached manifest. Used by `jarvy library clean` and tests.
#[allow(dead_code)] // Public API for `jarvy library clean` (CLI follow-up)
pub fn clean_all() -> std::io::Result<()> {
    let root = cache_root()?;
    if root.exists() {
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                std::fs::remove_dir_all(&path)?;
            } else {
                std::fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    hex::encode(digest)
}

/// Home-dir lookup. Honors `JARVY_HOME` for tests (matches the
/// existing config-loading convention).
fn dirs_home() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("JARVY_HOME") {
        return Some(PathBuf::from(v));
    }
    std::env::var_os("HOME").map(PathBuf::from).or_else(|| {
        // Windows
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sha256_hex_stable() {
        assert_eq!(
            sha256_hex(b"https://example.com/manifest.json"),
            sha256_hex(b"https://example.com/manifest.json"),
        );
    }

    #[test]
    fn write_then_read_round_trips() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("manifest.json");
        let m = Manifest {
            schema_version: 1,
            publisher: "test".into(),
            description: String::new(),
            homepage: String::new(),
            generated_at: String::new(),
            items: vec![],
        };
        write_manifest(&path, &m).unwrap();
        let read = read_manifest(&path).unwrap().unwrap();
        assert_eq!(read.publisher, "test");
    }

    #[test]
    fn read_missing_returns_none() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("does-not-exist.json");
        let read = read_manifest(&path).unwrap();
        assert!(read.is_none());
    }
}
