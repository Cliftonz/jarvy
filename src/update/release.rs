//! GitHub Releases API client
//!
//! Fetches release information from GitHub for update checking.

#![allow(dead_code)] // Public API for GitHub releases

use crate::update::config::Channel;
use serde::Deserialize;

/// GitHub repository information
pub const GITHUB_OWNER: &str = "Cliftonz";
pub const GITHUB_REPO: &str = "jarvy";

/// GitHub release information
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    /// Release tag name (e.g., "v1.2.3")
    pub tag_name: String,
    /// Release name/title
    pub name: Option<String>,
    /// Release body/description (changelog)
    pub body: Option<String>,
    /// Whether this is a prerelease
    pub prerelease: bool,
    /// Whether this is a draft
    pub draft: bool,
    /// Release assets (binaries, checksums)
    pub assets: Vec<ReleaseAsset>,
    /// Published timestamp
    pub published_at: Option<String>,
    /// HTML URL to the release page
    pub html_url: String,
}

impl GitHubRelease {
    /// Get version string without 'v' prefix
    pub fn version(&self) -> &str {
        self.tag_name.strip_prefix('v').unwrap_or(&self.tag_name)
    }

    /// Parse version as semver
    pub fn semver(&self) -> Option<semver::Version> {
        semver::Version::parse(self.version()).ok()
    }

    /// Check if this release matches the given channel
    pub fn matches_channel(&self, channel: Channel) -> bool {
        if self.draft {
            return false;
        }

        match channel {
            Channel::Stable => !self.prerelease && channel.matches_version(self.version()),
            Channel::Beta => channel.matches_version(self.version()),
            Channel::Nightly => true,
        }
    }

    /// Get changelog/what's new text
    pub fn changelog(&self) -> Option<&str> {
        self.body.as_deref()
    }

    /// Get asset for current platform
    pub fn asset_for_platform(&self) -> Option<&ReleaseAsset> {
        let target = get_current_target();
        self.assets.iter().find(|a| {
            let name = a.name.to_lowercase();
            name.contains(&target) && (name.ends_with(".tar.gz") || name.ends_with(".zip"))
        })
    }

    /// Get checksum asset for current platform
    pub fn checksum_asset(&self) -> Option<&ReleaseAsset> {
        self.assets.iter().find(|a| {
            let name = a.name.to_lowercase();
            name.contains("sha256") || name.ends_with(".sha256")
        })
    }

    /// Cosign companion asset matching `<archive>.<ext>`. Used to fetch the
    /// `.sig` and `.pem` files Sigstore verification expects on disk.
    /// Without these the signature step always returns `SignatureFilesMissing`
    /// and `--allow-unsigned` rubber-stamps the install (security review F-4).
    pub fn cosign_companion(&self, archive_name: &str, ext: &str) -> Option<&ReleaseAsset> {
        let want = format!("{}.{}", archive_name, ext);
        self.assets.iter().find(|a| a.name == want)
    }
}

/// Release asset (binary, checksum file, etc.)
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    /// Asset name
    pub name: String,
    /// Download URL
    pub browser_download_url: String,
    /// File size in bytes
    pub size: u64,
    /// Content type
    pub content_type: String,
}

/// Rust target triple for the current platform, matching the asset
/// names `release.yml` ships since #30 (`jarvy-v<ver>-<triple>.{tar.gz,zip}`).
///
/// [`GitHubRelease::asset_for_platform`] substring-matches this string
/// against the release asset names, so it MUST equal the shipped Rust
/// triple. The previous implementation returned an ad-hoc `<os>-<arch>`
/// string (`linux-x86_64`, `darwin-aarch64`, `windows-x86_64`) that
/// matched NONE of the triple-named assets, silently breaking
/// `jarvy update --method binary` on every platform. x86_64 Linux maps
/// to the static `-musl` build; aarch64 / armv7 Linux map to their
/// `-gnu` / `-gnueabihf` builds — mirroring the release build matrix.
pub fn get_current_target() -> String {
    let triple = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        // No Intel macOS prebuilt is shipped (Apple Silicon only since
        // v0.1.0). Returned for completeness; asset_for_platform finds
        // nothing today, surfacing an honest "No binary for this platform".
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-musl"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "arm")) {
        "armv7-unknown-linux-gnueabihf"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else {
        "unknown"
    };

    triple.to_string()
}

/// GitHub Releases API client
pub struct ReleaseClient {
    owner: String,
    repo: String,
}

impl ReleaseClient {
    /// Create a new client for the Jarvy repository
    pub fn new() -> Self {
        Self {
            owner: GITHUB_OWNER.to_string(),
            repo: GITHUB_REPO.to_string(),
        }
    }

    /// Create a client for a custom repository
    pub fn custom(owner: &str, repo: &str) -> Self {
        Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
        }
    }

    /// Fetch all releases from GitHub
    pub fn fetch_releases(&self) -> Result<Vec<GitHubRelease>, ReleaseError> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases",
            self.owner, self.repo
        );

        // `github_api_agent` follows up to 3 redirects so a repo rename
        // (e.g. `bearbinary/jarvy` → `Cliftonz/jarvy`, 2026-06-26) doesn't
        // brick auto-update on previously-installed binaries — GitHub
        // responds 301 to the canonical `/repositories/<id>/releases`
        // endpoint on the same host. The default `agent()` pins
        // `max_redirects(0)` and would surface the 301 body as a JSON
        // parse error, which the install path swallows as "up to date".
        let response = crate::net::github_api_agent()
            .get(&url)
            .header("User-Agent", crate::net::USER_AGENT)
            .header("Accept", "application/vnd.github.v3+json")
            .call()
            .map_err(|e| ReleaseError::NetworkError(e.to_string()))?;

        let releases: Vec<GitHubRelease> = response
            .into_body()
            .read_json()
            .map_err(|e| ReleaseError::ParseError(e.to_string()))?;

        Ok(releases)
    }

    /// Fetch the latest release for a given channel
    pub fn fetch_latest(&self, channel: Channel) -> Result<Option<GitHubRelease>, ReleaseError> {
        let releases = self.fetch_releases()?;

        Ok(releases
            .into_iter()
            .filter(|r| r.matches_channel(channel))
            .max_by(|a, b| {
                let a_ver = a.semver();
                let b_ver = b.semver();
                match (a_ver, b_ver) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }))
    }

    /// Fetch a specific release by tag
    pub fn fetch_by_tag(&self, tag: &str) -> Result<GitHubRelease, ReleaseError> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/tags/{}",
            self.owner, self.repo, tag
        );

        // See `fetch_releases` for the rationale on `github_api_agent`.
        let response = crate::net::github_api_agent()
            .get(&url)
            .header("User-Agent", crate::net::USER_AGENT)
            .header("Accept", "application/vnd.github.v3+json")
            .call()
            .map_err(|e| ReleaseError::NetworkError(e.to_string()))?;

        let release: GitHubRelease = response
            .into_body()
            .read_json()
            .map_err(|e| ReleaseError::ParseError(e.to_string()))?;

        Ok(release)
    }
}

impl Default for ReleaseClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors from release operations
#[derive(Debug, thiserror::Error)]
pub enum ReleaseError {
    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Failed to parse release data: {0}")]
    ParseError(String),

    #[error("Release not found: {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_string(),
            browser_download_url: format!("https://example.test/{name}"),
            size: 0,
            content_type: "application/octet-stream".to_string(),
        }
    }

    fn release_with(assets: Vec<ReleaseAsset>) -> GitHubRelease {
        GitHubRelease {
            tag_name: "v1.0.0".to_string(),
            name: None,
            body: None,
            prerelease: false,
            draft: false,
            assets,
            published_at: None,
            html_url: "https://example.test".to_string(),
        }
    }

    #[test]
    fn cosign_companion_finds_sig_and_pem() {
        // Round-2 P0 regression guard for the F-4 fix: the installer
        // depends on this lookup hitting `<archive>.sig` / `<archive>.pem`
        // exactly. A drift to substring matching would break the whole
        // companion-download contract.
        let r = release_with(vec![
            asset("jarvy-x86_64.tar.gz"),
            asset("jarvy-x86_64.tar.gz.sig"),
            asset("jarvy-x86_64.tar.gz.pem"),
            asset("checksums.txt"),
        ]);
        let sig = r.cosign_companion("jarvy-x86_64.tar.gz", "sig");
        assert!(sig.is_some(), "missing .sig companion");
        assert_eq!(sig.unwrap().name, "jarvy-x86_64.tar.gz.sig");

        let pem = r.cosign_companion("jarvy-x86_64.tar.gz", "pem");
        assert!(pem.is_some(), "missing .pem companion");
        assert_eq!(pem.unwrap().name, "jarvy-x86_64.tar.gz.pem");
    }

    #[test]
    fn cosign_companion_missing_returns_none() {
        let r = release_with(vec![asset("jarvy-x86_64.tar.gz")]);
        assert!(r.cosign_companion("jarvy-x86_64.tar.gz", "sig").is_none());
        assert!(r.cosign_companion("jarvy-x86_64.tar.gz", "pem").is_none());
    }

    #[test]
    fn cosign_companion_uses_exact_match_not_substring() {
        // A `.sig` for a *different* archive must not be returned for
        // the requested archive. Substring drift would let an attacker
        // who supplied `evil.tar.gz.sig` satisfy the lookup for
        // `jarvy-x86_64.tar.gz`.
        let r = release_with(vec![asset("jarvy-x86_64.tar.gz"), asset("evil.tar.gz.sig")]);
        assert!(r.cosign_companion("jarvy-x86_64.tar.gz", "sig").is_none());
    }

    #[test]
    fn test_release_version() {
        let release = GitHubRelease {
            tag_name: "v1.2.3".to_string(),
            name: None,
            body: None,
            prerelease: false,
            draft: false,
            assets: vec![],
            published_at: None,
            html_url: "https://github.com/test".to_string(),
        };

        assert_eq!(release.version(), "1.2.3");
        assert_eq!(release.semver(), Some(semver::Version::new(1, 2, 3)));
    }

    #[test]
    fn test_matches_channel() {
        let stable_release = GitHubRelease {
            tag_name: "v1.0.0".to_string(),
            name: None,
            body: None,
            prerelease: false,
            draft: false,
            assets: vec![],
            published_at: None,
            html_url: "".to_string(),
        };

        let beta_release = GitHubRelease {
            tag_name: "v1.1.0-beta.1".to_string(),
            prerelease: true,
            ..stable_release.clone()
        };

        let draft_release = GitHubRelease {
            tag_name: "v2.0.0".to_string(),
            draft: true,
            ..stable_release.clone()
        };

        assert!(stable_release.matches_channel(Channel::Stable));
        assert!(stable_release.matches_channel(Channel::Beta));
        assert!(stable_release.matches_channel(Channel::Nightly));

        assert!(!beta_release.matches_channel(Channel::Stable));
        assert!(beta_release.matches_channel(Channel::Beta));
        assert!(beta_release.matches_channel(Channel::Nightly));

        // Drafts never match
        assert!(!draft_release.matches_channel(Channel::Stable));
        assert!(!draft_release.matches_channel(Channel::Nightly));
    }

    #[test]
    fn test_get_current_target() {
        let target = get_current_target();
        // Must equal the exact Rust triple release.yml ships, so the
        // substring match in asset_for_platform() finds the asset.
        assert!(target.contains('-'));
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        assert_eq!(target, "aarch64-apple-darwin");
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert_eq!(target, "x86_64-unknown-linux-musl");
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        assert_eq!(target, "aarch64-unknown-linux-gnu");
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        assert_eq!(target, "x86_64-pc-windows-msvc");
    }
}
