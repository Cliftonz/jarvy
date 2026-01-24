//! Drift detection configuration types

use serde::{Deserialize, Serialize};

/// Drift detection configuration section in jarvy.toml
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DriftConfig {
    /// Enable drift detection
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Check for drift on every jarvy command
    #[serde(default)]
    pub check_on_run: bool,

    /// Files to track for changes
    #[serde(default)]
    pub track_files: Vec<String>,

    /// Version matching policy
    #[serde(default)]
    pub version_policy: VersionPolicy,

    /// Tools to ignore during drift detection
    #[serde(default)]
    pub ignore_tools: Vec<String>,

    /// Allow upgrades (only flag downgrades as drift)
    #[serde(default)]
    pub allow_upgrades: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_on_run: false,
            track_files: Vec::new(),
            version_policy: VersionPolicy::Minor,
            ignore_tools: Vec::new(),
            allow_upgrades: false,
        }
    }
}

/// Version matching policy for drift detection
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VersionPolicy {
    /// Only major version must match (1.x.x)
    Major,
    /// Major and minor must match (1.2.x)
    #[default]
    Minor,
    /// Major, minor, and patch must match (1.2.3)
    Patch,
    /// Exact version required (including pre-release, build metadata)
    Exact,
}

impl VersionPolicy {
    /// Check if two versions match according to this policy
    pub fn versions_match(&self, expected: &str, actual: &str) -> bool {
        match self {
            VersionPolicy::Exact => expected == actual,
            VersionPolicy::Patch | VersionPolicy::Minor | VersionPolicy::Major => {
                let exp = semver::Version::parse(expected).ok();
                let act = semver::Version::parse(actual).ok();

                match (exp, act) {
                    (Some(e), Some(a)) => match self {
                        VersionPolicy::Patch => {
                            e.major == a.major && e.minor == a.minor && e.patch == a.patch
                        }
                        VersionPolicy::Minor => e.major == a.major && e.minor == a.minor,
                        VersionPolicy::Major => e.major == a.major,
                        VersionPolicy::Exact => unreachable!(),
                    },
                    // Fallback to string comparison if not valid semver
                    _ => expected == actual,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_policy_exact() {
        let policy = VersionPolicy::Exact;
        assert!(policy.versions_match("1.2.3", "1.2.3"));
        assert!(!policy.versions_match("1.2.3", "1.2.4"));
        assert!(!policy.versions_match("1.2.3", "1.2.3-beta"));
    }

    #[test]
    fn test_version_policy_patch() {
        let policy = VersionPolicy::Patch;
        assert!(policy.versions_match("1.2.3", "1.2.3"));
        assert!(!policy.versions_match("1.2.3", "1.2.4"));
        assert!(!policy.versions_match("1.2.3", "1.3.3"));
    }

    #[test]
    fn test_version_policy_minor() {
        let policy = VersionPolicy::Minor;
        assert!(policy.versions_match("1.2.3", "1.2.3"));
        assert!(policy.versions_match("1.2.3", "1.2.99"));
        assert!(!policy.versions_match("1.2.3", "1.3.0"));
        assert!(!policy.versions_match("1.2.3", "2.2.3"));
    }

    #[test]
    fn test_version_policy_major() {
        let policy = VersionPolicy::Major;
        assert!(policy.versions_match("1.2.3", "1.2.3"));
        assert!(policy.versions_match("1.2.3", "1.99.99"));
        assert!(!policy.versions_match("1.2.3", "2.0.0"));
    }

    #[test]
    fn test_version_policy_non_semver() {
        // Non-semver versions fall back to exact string comparison
        let policy = VersionPolicy::Minor;
        assert!(policy.versions_match("abc123", "abc123"));
        assert!(!policy.versions_match("abc123", "abc124"));
    }

    #[test]
    fn test_drift_config_defaults() {
        let config = DriftConfig::default();
        assert!(config.enabled);
        assert!(!config.check_on_run);
        assert!(config.track_files.is_empty());
        assert_eq!(config.version_policy, VersionPolicy::Minor);
        assert!(config.ignore_tools.is_empty());
        assert!(!config.allow_upgrades);
    }

    #[test]
    fn test_drift_config_parsing() {
        let toml_str = r#"
enabled = true
check_on_run = false
track_files = [".vscode/settings.json", "package.json"]
version_policy = "minor"
ignore_tools = ["vim", "neovim"]
allow_upgrades = true
"#;
        let config: DriftConfig = toml::from_str(toml_str).expect("Failed to parse config");

        assert!(config.enabled);
        assert!(!config.check_on_run);
        assert_eq!(config.track_files.len(), 2);
        assert_eq!(config.version_policy, VersionPolicy::Minor);
        assert_eq!(config.ignore_tools, vec!["vim", "neovim"]);
        assert!(config.allow_upgrades);
    }
}
