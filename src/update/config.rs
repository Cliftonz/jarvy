//! Update configuration types and loading
//!
//! Supports configuration via:
//! - ~/.jarvy/config.toml [update] section
//! - Environment variables (JARVY_UPDATE, JARVY_UPDATE_CHANNEL, etc.)
//! - CLI flags (--method, --channel, etc.)

#![allow(dead_code)] // Public API for update configuration

use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

/// Update release channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Stable releases only (default)
    #[default]
    Stable,
    /// Beta/pre-release versions
    Beta,
    /// Nightly/development builds
    Nightly,
}

impl Channel {
    /// Parse channel from string (case-insensitive). Returns `Option`
    /// for parity with `InstallMethod::from_str`; intentionally not a
    /// `std::str::FromStr` impl.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "stable" => Some(Channel::Stable),
            "beta" => Some(Channel::Beta),
            "nightly" => Some(Channel::Nightly),
            _ => None,
        }
    }

    /// Get channel name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
            Channel::Nightly => "nightly",
        }
    }

    /// Check if a version string matches this channel
    pub fn matches_version(&self, version: &str) -> bool {
        match self {
            Channel::Stable => {
                !version.contains("-alpha")
                    && !version.contains("-beta")
                    && !version.contains("-rc")
                    && !version.contains("-nightly")
            }
            Channel::Beta => {
                version.contains("-beta")
                    || version.contains("-rc")
                    || self.matches_version_stable(version)
            }
            Channel::Nightly => true, // Nightly accepts all versions
        }
    }

    fn matches_version_stable(&self, version: &str) -> bool {
        !version.contains("-alpha")
            && !version.contains("-beta")
            && !version.contains("-rc")
            && !version.contains("-nightly")
    }
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Auto-install policy for updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoInstallPolicy {
    /// Never auto-install updates (prompt only)
    Never,
    /// Auto-install patch versions only (0.0.X)
    PatchOnly,
    /// Auto-install patch and minor versions (0.X.0)
    #[default]
    PatchAndMinor,
    /// Auto-install all updates including major versions
    All,
}

impl AutoInstallPolicy {
    /// Check if auto-install should proceed for given version change
    pub fn should_auto_install(&self, current: &semver::Version, new: &semver::Version) -> bool {
        match self {
            AutoInstallPolicy::Never => false,
            AutoInstallPolicy::PatchOnly => {
                current.major == new.major && current.minor == new.minor
            }
            AutoInstallPolicy::PatchAndMinor => current.major == new.major,
            AutoInstallPolicy::All => true,
        }
    }
}

/// Update configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Whether updates are enabled (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Release channel to follow (default: stable)
    #[serde(default)]
    pub channel: Channel,

    /// How often to check for updates (default: 24 hours)
    #[serde(default = "default_check_interval", with = "humantime_serde")]
    pub check_interval: Duration,

    /// Auto-install policy (default: patch_and_minor)
    #[serde(default)]
    pub auto_install: AutoInstallPolicy,

    /// Only allow updates within same major.minor version
    #[serde(default)]
    pub patch_only: bool,

    /// Override detected installation method
    #[serde(default)]
    pub install_method: Option<String>,

    /// Pinned version (prevents auto-update)
    #[serde(default)]
    pub pinned_version: Option<String>,

    /// Whether to show update notifications after commands
    #[serde(default = "default_enabled")]
    pub show_notifications: bool,
}

fn default_enabled() -> bool {
    true
}

fn default_check_interval() -> Duration {
    Duration::from_secs(24 * 60 * 60) // 24 hours
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            channel: Channel::Stable,
            check_interval: default_check_interval(),
            auto_install: AutoInstallPolicy::PatchAndMinor,
            patch_only: false,
            install_method: None,
            pinned_version: None,
            show_notifications: true,
        }
    }
}

impl UpdateConfig {
    /// Load configuration with environment variable overrides
    pub fn load() -> Self {
        let mut config = Self::load_from_file().unwrap_or_default();
        config.apply_env_overrides();
        config
    }

    /// Load configuration from ~/.jarvy/config.toml
    fn load_from_file() -> Option<Self> {
        let config_path = crate::paths::config_toml().ok()?;

        if !config_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&config_path).ok()?;

        // Parse the full config and extract [update] section
        #[derive(Deserialize)]
        struct GlobalConfig {
            #[serde(default)]
            update: UpdateConfig,
        }

        let global: GlobalConfig = toml::from_str(&content).ok()?;
        Some(global.update)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        // JARVY_UPDATE=0 or JARVY_UPDATE=false disables updates
        if let Ok(val) = env::var("JARVY_UPDATE") {
            self.enabled = !matches!(val.as_str(), "0" | "false" | "no" | "off");
        }

        // JARVY_UPDATE_CHANNEL=beta|nightly
        if let Ok(val) = env::var("JARVY_UPDATE_CHANNEL")
            && let Some(channel) = Channel::from_str(&val)
        {
            self.channel = channel;
        }

        // JARVY_UPDATE_CHECK=1 forces immediate check (handled in checker)
        // This env var is checked dynamically in should_check()

        // JARVY_PINNED_VERSION=1.2.3 pins to specific version
        if let Ok(val) = env::var("JARVY_PINNED_VERSION")
            && !val.is_empty()
        {
            self.pinned_version = Some(val);
        }

        // Unattended-mode auto-disable: covers CI runners AND sandbox
        // environments (Codespaces, Claude Code, e2b, etc.). Uses the
        // `_auto` variant so a hostile env that forces `JARVY_SANDBOX=1`
        // cannot silence security-patch self-updates on a victim's
        // machine — forced sandbox requires explicit `JARVY_UPDATE=0`.
        // See `crate::sandbox::is_seamless_auto` and PRD-053.
        if crate::sandbox::is_seamless_auto() && env::var("JARVY_UPDATE").is_err() {
            self.enabled = false;
        }
    }

    /// Check if updates are effectively disabled
    pub fn is_disabled(&self) -> bool {
        !self.enabled || self.pinned_version.is_some()
    }

    /// Get the effective version to use (pinned or latest)
    pub fn effective_pinned_version(&self) -> Option<&str> {
        self.pinned_version.as_deref()
    }

    /// Check if a forced check is requested via environment
    pub fn force_check_requested() -> bool {
        matches!(
            env::var("JARVY_UPDATE_CHECK").as_deref(),
            Ok("1" | "true" | "yes")
        )
    }

    /// Save configuration to ~/.jarvy/config.toml
    pub fn save(&self) -> std::io::Result<()> {
        let config_dir = crate::paths::jarvy_home().map_err(std::io::Error::other)?;
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("config.toml");

        // Read existing config to preserve other sections
        let mut existing: toml::Table = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content).unwrap_or_default()
        } else {
            toml::Table::new()
        };

        // Update the [update] section
        let update_value = toml::Value::try_from(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        existing.insert("update".to_string(), update_value);

        // Write back
        let content = toml::to_string_pretty(&existing)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&config_path, content)
    }
}

/// Check if running in interactive mode (TTY available)
pub fn is_interactive() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

// Implement humantime_serde for Duration serialization
mod humantime_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let secs = duration.as_secs();
        if secs.is_multiple_of(24 * 60 * 60) {
            serializer.serialize_str(&format!("{}d", secs / (24 * 60 * 60)))
        } else if secs.is_multiple_of(60 * 60) {
            serializer.serialize_str(&format!("{}h", secs / (60 * 60)))
        } else if secs.is_multiple_of(60) {
            serializer.serialize_str(&format!("{}m", secs / 60))
        } else {
            serializer.serialize_str(&format!("{}s", secs))
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_duration(&s).map_err(serde::de::Error::custom)
    }

    fn parse_duration(s: &str) -> Result<Duration, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("empty duration string".to_string());
        }

        // Try to parse as plain number (seconds)
        if let Ok(secs) = s.parse::<u64>() {
            return Ok(Duration::from_secs(secs));
        }

        // Parse with suffix
        let (num_str, suffix) = s.split_at(s.len() - 1);
        let num: u64 = num_str
            .trim()
            .parse()
            .map_err(|_| format!("invalid duration: {}", s))?;

        match suffix {
            "s" => Ok(Duration::from_secs(num)),
            "m" => Ok(Duration::from_secs(num * 60)),
            "h" => Ok(Duration::from_secs(num * 60 * 60)),
            "d" => Ok(Duration::from_secs(num * 24 * 60 * 60)),
            _ => Err(format!("unknown duration suffix: {}", suffix)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_from_str() {
        assert_eq!(Channel::from_str("stable"), Some(Channel::Stable));
        assert_eq!(Channel::from_str("BETA"), Some(Channel::Beta));
        assert_eq!(Channel::from_str("Nightly"), Some(Channel::Nightly));
        assert_eq!(Channel::from_str("invalid"), None);
    }

    #[test]
    fn test_channel_matches_version() {
        assert!(Channel::Stable.matches_version("1.2.3"));
        assert!(!Channel::Stable.matches_version("1.2.3-beta.1"));
        assert!(!Channel::Stable.matches_version("1.2.3-rc.1"));

        assert!(Channel::Beta.matches_version("1.2.3"));
        assert!(Channel::Beta.matches_version("1.2.3-beta.1"));
        assert!(Channel::Beta.matches_version("1.2.3-rc.1"));
        assert!(!Channel::Beta.matches_version("1.2.3-alpha.1"));

        assert!(Channel::Nightly.matches_version("1.2.3"));
        assert!(Channel::Nightly.matches_version("1.2.3-nightly.123"));
    }

    #[test]
    fn test_auto_install_policy() {
        let v1_0_0 = semver::Version::new(1, 0, 0);
        let v1_0_1 = semver::Version::new(1, 0, 1);
        let v1_1_0 = semver::Version::new(1, 1, 0);
        let v2_0_0 = semver::Version::new(2, 0, 0);

        assert!(!AutoInstallPolicy::Never.should_auto_install(&v1_0_0, &v1_0_1));

        assert!(AutoInstallPolicy::PatchOnly.should_auto_install(&v1_0_0, &v1_0_1));
        assert!(!AutoInstallPolicy::PatchOnly.should_auto_install(&v1_0_0, &v1_1_0));

        assert!(AutoInstallPolicy::PatchAndMinor.should_auto_install(&v1_0_0, &v1_0_1));
        assert!(AutoInstallPolicy::PatchAndMinor.should_auto_install(&v1_0_0, &v1_1_0));
        assert!(!AutoInstallPolicy::PatchAndMinor.should_auto_install(&v1_0_0, &v2_0_0));

        assert!(AutoInstallPolicy::All.should_auto_install(&v1_0_0, &v2_0_0));
    }

    #[test]
    fn test_default_config() {
        let config = UpdateConfig::default();
        assert!(config.enabled);
        assert_eq!(config.channel, Channel::Stable);
        assert_eq!(config.check_interval, Duration::from_secs(24 * 60 * 60));
        assert_eq!(config.auto_install, AutoInstallPolicy::PatchAndMinor);
        assert!(!config.patch_only);
        assert!(config.pinned_version.is_none());
    }

    #[test]
    fn test_is_disabled() {
        let mut config = UpdateConfig::default();
        assert!(!config.is_disabled());

        config.enabled = false;
        assert!(config.is_disabled());

        config.enabled = true;
        config.pinned_version = Some("1.0.0".to_string());
        assert!(config.is_disabled());
    }
}
