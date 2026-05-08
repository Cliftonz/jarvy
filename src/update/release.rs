//! GitHub Releases API client
//!
//! Fetches release information from GitHub for update checking.

#![allow(dead_code)] // Public API for GitHub releases

use crate::update::config::Channel;
use serde::Deserialize;

/// GitHub repository information
pub const GITHUB_OWNER: &str = "bearbinary";
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

/// Get target triple for current platform
pub fn get_current_target() -> String {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else {
        "unknown"
    };

    format!("{}-{}", os, arch)
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

        let response = crate::net::agent()
            .get(&url)
            .header("User-Agent", &crate::net::user_agent())
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

        let response = crate::net::agent()
            .get(&url)
            .header("User-Agent", &crate::net::user_agent())
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
        // Should contain OS and arch
        assert!(target.contains('-'));
        #[cfg(target_os = "macos")]
        assert!(target.starts_with("darwin"));
        #[cfg(target_os = "linux")]
        assert!(target.starts_with("linux"));
        #[cfg(target_os = "windows")]
        assert!(target.starts_with("windows"));
    }
}
