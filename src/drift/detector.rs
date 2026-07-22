//! Drift detection logic

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

use super::DriftError;
use super::config::DriftConfig;
use super::state::{EnvironmentState, hash_file};

/// Drift detection engine
pub struct DriftDetector<'a> {
    config: &'a DriftConfig,
    expected_state: &'a EnvironmentState,
    project_dir: &'a Path,
}

/// Complete drift report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    /// Timestamp of the report
    pub timestamp: String,

    /// Overall drift status
    pub status: DriftStatus,

    /// Summary counts
    pub summary: DriftSummary,

    /// Tool version changes detected
    pub version_changes: Vec<VersionChange>,

    /// Tools that are missing
    pub missing_tools: Vec<MissingTool>,

    /// Tools installed but not in config
    pub extra_tools: Vec<ExtraTool>,

    /// Files that have changed
    pub changed_files: Vec<ChangedFile>,
}

/// Summary of drift issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftSummary {
    pub total_issues: usize,
    pub version_changes: usize,
    pub missing_tools: usize,
    pub extra_tools: usize,
    pub changed_files: usize,
}

/// Drift detection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    /// No drift detected
    NoDrift,
    /// Drift was detected
    DriftDetected,
    /// No baseline state to compare against
    NoBaseline,
}

/// A version change in a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionChange {
    pub tool: String,
    pub expected: String,
    pub actual: String,
    pub direction: VersionDirection,
    pub auto_fixable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Direction of version change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionDirection {
    Upgrade,
    Downgrade,
}

/// A tool that is missing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingTool {
    pub tool: String,
    pub expected_version: String,
    pub auto_fixable: bool,
}

/// A tool that is installed but not in config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraTool {
    pub tool: String,
    pub version: String,
}

/// A file that has changed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub expected_hash: String,
    pub actual_hash: String,
    pub auto_fixable: bool,
}

impl<'a> DriftDetector<'a> {
    /// Create a new drift detector
    pub fn new(
        config: &'a DriftConfig,
        expected_state: &'a EnvironmentState,
        project_dir: &'a Path,
    ) -> Self {
        Self {
            config,
            expected_state,
            project_dir,
        }
    }

    /// Detect drift between expected and actual state
    pub fn detect(&self) -> Result<DriftReport, DriftError> {
        let mut report = DriftReport {
            timestamp: current_timestamp(),
            status: DriftStatus::NoDrift,
            summary: DriftSummary {
                total_issues: 0,
                version_changes: 0,
                missing_tools: 0,
                extra_tools: 0,
                changed_files: 0,
            },
            version_changes: Vec::new(),
            missing_tools: Vec::new(),
            extra_tools: Vec::new(),
            changed_files: Vec::new(),
        };

        // Check each expected tool
        for (name, expected) in &self.expected_state.tools {
            if self.config.ignore_tools.contains(name) {
                continue;
            }

            match get_tool_version(name) {
                Some(actual_version) => {
                    if !self
                        .config
                        .version_policy
                        .versions_match(&expected.version, &actual_version)
                    {
                        let direction = if is_upgrade(&expected.version, &actual_version) {
                            VersionDirection::Upgrade
                        } else {
                            VersionDirection::Downgrade
                        };

                        // Skip if allow_upgrades and this is an upgrade
                        if self.config.allow_upgrades && direction == VersionDirection::Upgrade {
                            continue;
                        }

                        report.version_changes.push(VersionChange {
                            tool: name.clone(),
                            expected: expected.version.clone(),
                            actual: actual_version,
                            direction,
                            auto_fixable: is_auto_fixable(name, &expected.install_method),
                            reason: None,
                        });
                    }
                }
                None => {
                    report.missing_tools.push(MissingTool {
                        tool: name.clone(),
                        expected_version: expected.version.clone(),
                        auto_fixable: true,
                    });
                }
            }
        }

        // Check tracked files
        for (path, expected_hash) in &self.expected_state.files {
            let file_path = self.project_dir.join(path);

            if !file_path.exists() {
                report.changed_files.push(ChangedFile {
                    path: path.clone(),
                    expected_hash: expected_hash.clone(),
                    actual_hash: "missing".to_string(),
                    auto_fixable: false,
                });
            } else if let Ok(actual_hash) = hash_file(&file_path)
                && actual_hash != *expected_hash
            {
                report.changed_files.push(ChangedFile {
                    path: path.clone(),
                    expected_hash: expected_hash.clone(),
                    actual_hash,
                    auto_fixable: false,
                });
            }
        }

        // Update summary
        report.summary.version_changes = report.version_changes.len();
        report.summary.missing_tools = report.missing_tools.len();
        report.summary.extra_tools = report.extra_tools.len();
        report.summary.changed_files = report.changed_files.len();
        report.summary.total_issues = report.summary.version_changes
            + report.summary.missing_tools
            + report.summary.changed_files;

        // Update status
        if report.summary.total_issues > 0 {
            report.status = DriftStatus::DriftDetected;
        }

        Ok(report)
    }
}

/// Get the installed version of a tool
fn get_tool_version(tool: &str) -> Option<String> {
    // Try common version flags
    let output = Command::new(tool)
        .arg("--version")
        .output()
        .or_else(|_| Command::new(tool).arg("-V").output())
        .or_else(|_| Command::new(tool).arg("version").output())
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    extract_version(&output_str)
}

/// Extract version from command output. Delegates to the canonical
/// extractor in `tools::version` so drift reports and lock-file
/// output normalize identical `--version` output to identical strings
/// (round-2 maint F14).
fn extract_version(output: &str) -> Option<String> {
    crate::tools::version::extract_version(output).map(|v| v.to_string())
}

/// Check if new version is an upgrade from old version
fn is_upgrade(old: &str, new: &str) -> bool {
    match (semver::Version::parse(old), semver::Version::parse(new)) {
        (Ok(old_v), Ok(new_v)) => new_v > old_v,
        _ => new > old, // Fallback to string comparison
    }
}

/// Check if a tool can be automatically fixed
fn is_auto_fixable(tool: &str, install_method: &str) -> bool {
    // Version managers typically require manual intervention
    let version_managers = ["rustup", "nvm", "pyenv", "rbenv", "sdkman"];
    !version_managers.contains(&install_method)
        && !version_managers
            .iter()
            .any(|vm| tool.contains(vm) || install_method.contains(vm))
}

/// Get current timestamp
fn current_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}Z", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version() {
        assert_eq!(
            extract_version("git version 2.39.0"),
            Some("2.39.0".to_string())
        );
        assert_eq!(
            extract_version("node v20.10.0"),
            Some("20.10.0".to_string())
        );
        assert_eq!(extract_version("rustc 1.75.0"), Some("1.75.0".to_string()));
        assert_eq!(
            extract_version("Docker version 24.0.7, build afdd53b"),
            Some("24.0.7".to_string())
        );
    }

    #[test]
    fn test_is_upgrade() {
        assert!(is_upgrade("1.0.0", "1.0.1"));
        assert!(is_upgrade("1.0.0", "1.1.0"));
        assert!(is_upgrade("1.0.0", "2.0.0"));
        assert!(!is_upgrade("1.0.1", "1.0.0"));
        assert!(!is_upgrade("2.0.0", "1.0.0"));
        assert!(!is_upgrade("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_is_auto_fixable() {
        // Regular tools are fixable
        assert!(is_auto_fixable("node", "brew"));
        assert!(is_auto_fixable("docker", "apt"));

        // Version managers are not auto-fixable
        assert!(!is_auto_fixable("rust", "rustup"));
        assert!(!is_auto_fixable("node", "nvm"));
        assert!(!is_auto_fixable("python", "pyenv"));
    }

    #[test]
    fn test_drift_status_serialization() {
        let status = DriftStatus::DriftDetected;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"drift_detected\"");
    }

    #[test]
    fn test_version_direction_serialization() {
        let upgrade = VersionDirection::Upgrade;
        let json = serde_json::to_string(&upgrade).unwrap();
        assert_eq!(json, "\"upgrade\"");
    }
}
