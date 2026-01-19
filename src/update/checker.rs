//! Version checking with throttling and state persistence
//!
//! Handles background update checking with configurable intervals.

use crate::update::config::{Channel, UpdateConfig};
use crate::update::release::{GitHubRelease, ReleaseClient, ReleaseError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Current version of Jarvy
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Update state persisted between runs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateState {
    /// Unix timestamp of last check
    #[serde(default)]
    pub last_checked: Option<u64>,

    /// Latest available version found
    #[serde(default)]
    pub available_version: Option<String>,

    /// Previous version before last update (for rollback)
    #[serde(default)]
    pub previous_version: Option<String>,

    /// Channel that was checked
    #[serde(default)]
    pub channel: Option<String>,

    /// Whether user was notified about available update
    #[serde(default)]
    pub notified: bool,

    /// Changelog/what's new for available version
    #[serde(default)]
    pub changelog: Option<String>,

    /// Release URL for more info
    #[serde(default)]
    pub release_url: Option<String>,
}

impl UpdateState {
    /// Load state from disk
    pub fn load() -> Self {
        Self::state_path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    /// Save state to disk
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::state_path()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No state path"))?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)
    }

    /// Get state file path
    fn state_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".jarvy").join("update-state.json"))
    }

    /// Get current Unix timestamp
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Update last checked timestamp
    pub fn mark_checked(&mut self) {
        self.last_checked = Some(Self::now());
    }

    /// Record available update
    pub fn record_available(&mut self, release: &GitHubRelease, channel: Channel) {
        self.available_version = Some(release.version().to_string());
        self.channel = Some(channel.as_str().to_string());
        self.changelog = release.changelog().map(|s| s.to_string());
        self.release_url = Some(release.html_url.clone());
        self.notified = false;
    }

    /// Clear available update (after successful update)
    pub fn clear_available(&mut self, previous: &str) {
        self.previous_version = Some(previous.to_string());
        self.available_version = None;
        self.changelog = None;
        self.release_url = None;
        self.notified = false;
    }

    /// Check if there's an update available
    pub fn has_update(&self) -> bool {
        if let Some(available) = &self.available_version {
            let current = semver::Version::parse(CURRENT_VERSION).ok();
            let avail = semver::Version::parse(available).ok();

            match (current, avail) {
                (Some(c), Some(a)) => a > c,
                _ => false,
            }
        } else {
            false
        }
    }
}

/// Update checker with throttling
pub struct UpdateChecker {
    config: UpdateConfig,
    state: UpdateState,
    client: ReleaseClient,
}

impl UpdateChecker {
    /// Create a new update checker
    pub fn new() -> Self {
        Self {
            config: UpdateConfig::load(),
            state: UpdateState::load(),
            client: ReleaseClient::new(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: UpdateConfig) -> Self {
        Self {
            config,
            state: UpdateState::load(),
            client: ReleaseClient::new(),
        }
    }

    /// Check if an update check should be performed
    pub fn should_check(&self) -> bool {
        // Disabled?
        if self.config.is_disabled() {
            return false;
        }

        // Force check requested via env var?
        if UpdateConfig::force_check_requested() {
            return true;
        }

        // Check interval elapsed?
        if let Some(last) = self.state.last_checked {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let elapsed = Duration::from_secs(now.saturating_sub(last));
            elapsed >= self.config.check_interval
        } else {
            // Never checked before
            true
        }
    }

    /// Perform update check
    pub fn check(&mut self) -> Result<CheckResult, CheckError> {
        if !self.should_check() {
            // Return cached result if available
            if self.state.has_update() {
                return Ok(CheckResult::UpdateAvailable {
                    current: CURRENT_VERSION.to_string(),
                    latest: self.state.available_version.clone().unwrap_or_default(),
                    changelog: self.state.changelog.clone(),
                    release_url: self.state.release_url.clone(),
                });
            }
            return Ok(CheckResult::UpToDate);
        }

        // Fetch latest release
        let latest = self
            .client
            .fetch_latest(self.config.channel)
            .map_err(CheckError::Release)?;

        // Mark as checked
        self.state.mark_checked();

        match latest {
            Some(release) => {
                let current = semver::Version::parse(CURRENT_VERSION)
                    .map_err(|e| CheckError::Version(e.to_string()))?;
                let latest_ver = release
                    .semver()
                    .ok_or_else(|| CheckError::Version("Invalid release version".to_string()))?;

                // Check if patch_only constraint applies
                let is_valid_update = if self.config.patch_only {
                    latest_ver.major == current.major && latest_ver.minor == current.minor
                } else {
                    true
                };

                if latest_ver > current && is_valid_update {
                    // Record the available update
                    self.state.record_available(&release, self.config.channel);
                    let _ = self.state.save();

                    Ok(CheckResult::UpdateAvailable {
                        current: CURRENT_VERSION.to_string(),
                        latest: release.version().to_string(),
                        changelog: release.changelog().map(|s| s.to_string()),
                        release_url: Some(release.html_url),
                    })
                } else {
                    let _ = self.state.save();
                    Ok(CheckResult::UpToDate)
                }
            }
            None => {
                let _ = self.state.save();
                Ok(CheckResult::UpToDate)
            }
        }
    }

    /// Get current update state
    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    /// Get mutable update state
    pub fn state_mut(&mut self) -> &mut UpdateState {
        &mut self.state
    }

    /// Get current config
    pub fn config(&self) -> &UpdateConfig {
        &self.config
    }

    /// Check if should auto-install based on version difference
    pub fn should_auto_install(&self, latest: &str) -> bool {
        let current = match semver::Version::parse(CURRENT_VERSION) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let new = match semver::Version::parse(latest) {
            Ok(v) => v,
            Err(_) => return false,
        };

        self.config.auto_install.should_auto_install(&current, &new)
    }

    /// Mark notification as shown
    pub fn mark_notified(&mut self) {
        self.state.notified = true;
        let _ = self.state.save();
    }

    /// Check if notification should be shown
    pub fn should_notify(&self) -> bool {
        self.config.show_notifications && self.state.has_update() && !self.state.notified
    }

    /// Get notification message if update is available
    pub fn notification_message(&self) -> Option<String> {
        if !self.should_notify() {
            return None;
        }

        let available = self.state.available_version.as_ref()?;
        Some(format!(
            "A new version of Jarvy is available: {} -> {}\nRun 'jarvy update' to install or 'jarvy update check' for details.",
            CURRENT_VERSION, available
        ))
    }
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of an update check
#[derive(Debug, Clone)]
pub enum CheckResult {
    /// Already on the latest version
    UpToDate,
    /// Update is available
    UpdateAvailable {
        current: String,
        latest: String,
        changelog: Option<String>,
        release_url: Option<String>,
    },
}

impl CheckResult {
    /// Check if an update is available
    pub fn has_update(&self) -> bool {
        matches!(self, CheckResult::UpdateAvailable { .. })
    }
}

/// Errors during update checking
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("Release error: {0}")]
    Release(#[from] ReleaseError),

    #[error("Version error: {0}")]
    Version(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_state_default() {
        let state = UpdateState::default();
        assert!(state.last_checked.is_none());
        assert!(state.available_version.is_none());
        assert!(!state.has_update());
    }

    #[test]
    fn test_update_state_has_update() {
        let mut state = UpdateState::default();

        // Set available version higher than current
        state.available_version = Some("999.0.0".to_string());
        assert!(state.has_update());

        // Set available version lower
        state.available_version = Some("0.0.1".to_string());
        assert!(!state.has_update());
    }

    #[test]
    fn test_check_result() {
        let up_to_date = CheckResult::UpToDate;
        assert!(!up_to_date.has_update());

        let available = CheckResult::UpdateAvailable {
            current: "1.0.0".to_string(),
            latest: "1.1.0".to_string(),
            changelog: None,
            release_url: None,
        };
        assert!(available.has_update());
    }

    #[test]
    fn test_checker_should_check_disabled() {
        let mut config = UpdateConfig::default();
        config.enabled = false;

        let checker = UpdateChecker::with_config(config);
        assert!(!checker.should_check());
    }

    #[test]
    fn test_checker_should_check_pinned() {
        let mut config = UpdateConfig::default();
        config.pinned_version = Some("1.0.0".to_string());

        let checker = UpdateChecker::with_config(config);
        assert!(!checker.should_check());
    }
}
