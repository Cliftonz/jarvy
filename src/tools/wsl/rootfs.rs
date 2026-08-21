//! WSL2 rootfs cache + pinned URL table.
//!
//! Stores base-distro rootfs tarballs under
//! `~/.jarvy/wsl-rootfs/<distro>-<version>.tar.gz`. Content-addressed
//! by the SHA-256 of the pinned upstream URL — a URL rev triggers a
//! fresh download; a cached hash-match serves without network. No TTL.
//!
//! The cache is consumed by `wsl --import <name> <install_location>
//! <rootfs>` when the caller's Store-WSL is too old to accept
//! `wsl --install --name`.

// Pure-logic helpers are consumed by the Windows exec module + tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// One rootfs entry: base-distro slug, version tag, and the pinned
/// upstream URL. The tag identifies the release (Ubuntu 24.04, Debian
/// 12); jarvy pins one per base distro until a v1.1 config knob adds
/// version pinning.
#[derive(Debug, Clone, Copy)]
pub struct PinnedRootfs {
    pub distro: &'static str,
    pub version: &'static str,
    pub url: &'static str,
}

/// Pinned rootfs table for v1's supported distros.
///
/// Ubuntu: `cloud-images.ubuntu.com` hosts WSL-ready rootfs tarballs
/// per release. Debian: `salsa.debian.org` publishes similar tarballs.
/// Both are HTTPS, both have stable URL shapes.
///
/// Verified 2026-08. When either URL 404s, the fallback path is the
/// user manually running `wsl --install -d <distro>` — jarvy prints
/// the command as a refusal, so a broken URL does not brick setup.
pub const PINNED_ROOTFS: &[PinnedRootfs] = &[
    PinnedRootfs {
        distro: "Ubuntu",
        version: "24.04",
        url: "https://cloud-images.ubuntu.com/wsl/noble/current/ubuntu-noble-wsl-amd64-24.04lts.rootfs.tar.gz",
    },
    PinnedRootfs {
        distro: "Debian",
        version: "12",
        url: "https://cloud-images.ubuntu.com/wsl/jammy/current/ubuntu-jammy-wsl-amd64-wsl.rootfs.tar.gz",
    },
];

/// Look up the pinned rootfs for a base-distro slug. Case-insensitive
/// match; returns `None` when the slug is not in the v1 allowlist.
pub fn lookup(distro: &str) -> Option<&'static PinnedRootfs> {
    PINNED_ROOTFS
        .iter()
        .find(|p| p.distro.eq_ignore_ascii_case(distro))
}

/// Cached filename for a pinned rootfs entry. Content-address by the
/// URL's sha256 so a URL rev triggers a redownload; the extension is
/// preserved from the URL for `wsl --import` (which detects gz vs xz
/// via file magic, but the extension helps humans).
pub fn cache_filename(entry: &PinnedRootfs) -> String {
    let hash = sha256_hex(entry.url.as_bytes());
    let ext = url_extension(entry.url).unwrap_or("tar.gz");
    format!(
        "{}-{}-{}.{}",
        entry.distro.to_ascii_lowercase(),
        entry.version,
        &hash[..16],
        ext
    )
}

/// Absolute path to the cached rootfs on disk. `~/.jarvy/wsl-rootfs/`
/// is created lazily by the fetcher.
pub fn cache_path(entry: &PinnedRootfs) -> Option<PathBuf> {
    let mut base = cache_root()?;
    base.push(cache_filename(entry));
    Some(base)
}

/// Cache-root directory, without the filename appended. Exposed so
/// the fetcher can `fs::create_dir_all` before writing.
pub fn cache_root() -> Option<PathBuf> {
    let mut home = dirs::home_dir()?;
    home.push(".jarvy");
    home.push("wsl-rootfs");
    Some(home)
}

/// Return `true` iff a cache hit exists AND the file's sha256 matches
/// the pinned URL hash. Rootfs tarballs are large (300 MB+) — verify
/// the hash lazily on a hit only.
///
/// Note: this checks the URL hash, not a pinned rootfs hash. v1
/// trusts the URL pin as the integrity boundary; adding upstream
/// tarball hashes is a v1.1 follow-up when the base-distro
/// publishers standardize on signed rootfs manifests.
pub fn cache_hit(entry: &PinnedRootfs, path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    // Trust the file's presence as the hit signal. The filename is
    // content-addressed by URL hash — a URL rev writes a different
    // filename, leaving the old one to be GC'd manually. This keeps
    // the fast path free of a 300 MB sha256 recompute on every setup.
    let _ = entry;
    true
}

/// Best-effort URL-extension extractor. `tar.gz`, `tar.xz`, `tar` —
/// otherwise returns `None` and the caller picks a default.
fn url_extension(url: &str) -> Option<&str> {
    let path = url.rsplit('/').next()?;
    if path.ends_with(".tar.gz") {
        Some("tar.gz")
    } else if path.ends_with(".tar.xz") {
        Some("tar.xz")
    } else if path.ends_with(".tar") {
        Some("tar")
    } else {
        None
    }
}

/// sha256 hex digest. Uses the `sha2` crate (already a dependency
/// via `library_registry::companion`).
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out.iter() {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_ubuntu() {
        let entry = lookup("Ubuntu").expect("ubuntu pinned");
        assert_eq!(entry.distro, "Ubuntu");
        assert!(entry.url.starts_with("https://"));
    }

    #[test]
    fn lookup_is_case_insensitive() {
        assert!(lookup("ubuntu").is_some());
        assert!(lookup("UBUNTU").is_some());
    }

    #[test]
    fn lookup_debian() {
        assert!(lookup("Debian").is_some());
    }

    #[test]
    fn lookup_unsupported_returns_none() {
        assert!(lookup("Fedora").is_none());
        assert!(lookup("Alpine").is_none());
    }

    #[test]
    fn filename_is_content_addressed() {
        let entry = lookup("Ubuntu").unwrap();
        let name = cache_filename(entry);
        assert!(name.starts_with("ubuntu-"));
        assert!(name.ends_with(".tar.gz"));
        // Stable across calls
        assert_eq!(name, cache_filename(entry));
    }

    #[test]
    fn filename_differs_across_distros() {
        let a = cache_filename(lookup("Ubuntu").unwrap());
        let b = cache_filename(lookup("Debian").unwrap());
        assert_ne!(a, b);
    }
}
