//! Upgrade tools to their latest versions
//!
//! Checks for newer versions and upgrades tools using appropriate methods.

use crate::config::Config;
use crate::output::{ExitCode, Outputable, colors, header, icons};
use crate::telemetry;
use crate::tools::common::PackageManager;
use crate::tools::common::{has, run};
use crate::tools::spec::{get_tool_install_info, get_tool_spec};
use serde::Serialize;

/// Result of upgrading a single tool
#[derive(Debug, Clone, Serialize)]
pub struct ToolUpgrade {
    pub name: String,
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    pub status: UpgradeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpgradeStatus {
    Upgraded,
    AlreadyLatest,
    Failed,
    Skipped,
    DryRun,
}

/// Complete upgrade result
#[derive(Debug, Clone, Serialize)]
pub struct UpgradeResult {
    pub tools: Vec<ToolUpgrade>,
    pub upgraded_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
}

impl Outputable for UpgradeResult {
    fn to_human(&self) -> String {
        let mut output = String::new();

        output.push_str(&header("Jarvy Upgrade"));
        output.push('\n');

        if self.tools.is_empty() {
            output.push_str("\nNo tools to upgrade.\n");
            return output;
        }

        for (i, tool) in self.tools.iter().enumerate() {
            let (icon, color) = match tool.status {
                UpgradeStatus::Upgraded => (icons::OK, colors::GREEN),
                UpgradeStatus::AlreadyLatest => (icons::SATISFIED, colors::DIM),
                UpgradeStatus::Failed => (icons::ERROR, colors::RED),
                UpgradeStatus::Skipped => (icons::WARN, colors::YELLOW),
                UpgradeStatus::DryRun => (icons::INFO, colors::CYAN),
            };

            let version_info = match (&tool.from_version, &tool.to_version) {
                (Some(from), Some(to)) if from != to => format!("{} -> {}", from, to),
                (Some(v), _) | (_, Some(v)) => v.clone(),
                _ => "latest".to_string(),
            };

            output.push_str(&format!(
                "\n[{}/{}] {}{}{} {}: {}\n",
                i + 1,
                self.tools.len(),
                color,
                icon,
                colors::RESET,
                tool.name,
                version_info
            ));

            if let Some(ref msg) = tool.message {
                output.push_str(&format!("      {}\n", msg));
            }
        }

        output.push_str(&format!(
            "\nUpgrade complete. {} upgraded, {} failed, {} skipped.\n",
            self.upgraded_count, self.failed_count, self.skipped_count
        ));

        output
    }

    fn exit_code(&self) -> ExitCode {
        if self.failed_count > 0 {
            ExitCode::Error
        } else if self.skipped_count > 0 {
            ExitCode::Warning
        } else {
            ExitCode::Ok
        }
    }
}

/// Run the upgrade command
pub fn run_upgrade(
    config: Option<&Config>,
    specific_tools: Option<Vec<String>>,
    dry_run: bool,
    force: bool,
) -> UpgradeResult {
    let mut upgrades = Vec::new();

    // Determine which tools to upgrade
    let tools_to_upgrade: Vec<(String, String)> = if let Some(tools) = specific_tools {
        // Parse tool@version syntax
        tools
            .iter()
            .map(|t| {
                if let Some((name, version)) = t.split_once('@') {
                    (name.to_string(), version.to_string())
                } else {
                    (t.clone(), "latest".to_string())
                }
            })
            .collect()
    } else if let Some(cfg) = config {
        // From config file
        cfg.get_tool_configs()
            .values()
            .map(|t| (t.name.clone(), t.version.clone()))
            .collect()
    } else {
        // No config, nothing to upgrade
        return UpgradeResult {
            tools: vec![],
            upgraded_count: 0,
            failed_count: 0,
            skipped_count: 0,
        };
    };

    for (name, target_version) in tools_to_upgrade {
        let result = upgrade_tool(&name, &target_version, dry_run, force);
        upgrades.push(result);
    }

    let upgraded_count = upgrades
        .iter()
        .filter(|u| u.status == UpgradeStatus::Upgraded)
        .count();
    let failed_count = upgrades
        .iter()
        .filter(|u| u.status == UpgradeStatus::Failed)
        .count();
    let skipped_count = upgrades
        .iter()
        .filter(|u| u.status == UpgradeStatus::Skipped || u.status == UpgradeStatus::AlreadyLatest)
        .count();

    // Emit telemetry
    telemetry::upgrade_result(upgraded_count, failed_count, skipped_count);

    UpgradeResult {
        tools: upgrades,
        upgraded_count,
        failed_count,
        skipped_count,
    }
}

fn upgrade_tool(name: &str, target_version: &str, dry_run: bool, force: bool) -> ToolUpgrade {
    let spec = get_tool_spec(name);

    // Check if tool is installed
    let command = spec.map(|s| s.command).unwrap_or(name);
    if !has(command) {
        return ToolUpgrade {
            name: name.to_string(),
            from_version: None,
            to_version: None,
            status: UpgradeStatus::Skipped,
            message: Some("Tool not installed - use 'jarvy setup' to install".to_string()),
        };
    }

    let current_version = get_installed_version(command);

    // Check if already at target version (unless force)
    if !force && target_version != "latest" {
        if let Some(ref current) = current_version {
            if version_satisfies(current, target_version) {
                return ToolUpgrade {
                    name: name.to_string(),
                    from_version: current_version,
                    to_version: Some(target_version.to_string()),
                    status: UpgradeStatus::AlreadyLatest,
                    message: Some("Already at required version".to_string()),
                };
            }
        }
    }

    if dry_run {
        return ToolUpgrade {
            name: name.to_string(),
            from_version: current_version,
            to_version: Some(target_version.to_string()),
            status: UpgradeStatus::DryRun,
            message: Some("Would upgrade (dry-run)".to_string()),
        };
    }

    // Perform the upgrade
    let upgrade_result = perform_upgrade(name, target_version);

    match upgrade_result {
        Ok(msg) => {
            let new_version = get_installed_version(command);
            ToolUpgrade {
                name: name.to_string(),
                from_version: current_version,
                to_version: new_version,
                status: UpgradeStatus::Upgraded,
                message: Some(msg),
            }
        }
        Err(e) => ToolUpgrade {
            name: name.to_string(),
            from_version: current_version,
            to_version: None,
            status: UpgradeStatus::Failed,
            message: Some(e),
        },
    }
}

fn perform_upgrade(name: &str, _target_version: &str) -> Result<String, String> {
    // Get install info to determine upgrade method
    let install_info = get_tool_install_info(name, "latest");

    // Handle special cases first
    match name.to_lowercase().as_str() {
        "rust" => {
            // Use rustup
            if has("rustup") {
                run("rustup", &["update", "stable"])
                    .map_err(|e| format!("rustup update failed: {:?}", e))?;
                return Ok("Updated via rustup".to_string());
            }
            return Err("rustup not found".to_string());
        }
        // nvm needs to be sourced, this is tricky; for now, suggest manual
        // upgrade. Match guard collapses the inner `if has("nvm")` per
        // clippy::collapsible_match (added as a deny-level lint in Rust 1.95).
        "node" if has("nvm") => {
            return Err("Use 'nvm install <version>' to upgrade node".to_string());
        }
        "node" => {
            // fall through to package manager
        }
        _ => {}
    }

    // Use package manager
    if let Some(info) = install_info {
        let result = match info.package_manager {
            PackageManager::Brew => run("brew", &["upgrade", &info.package_name]),
            PackageManager::BrewCask => run("brew", &["upgrade", "--cask", &info.package_name]),
            PackageManager::Apt => run(
                "apt",
                &["install", "--only-upgrade", "-y", &info.package_name],
            ),
            PackageManager::Dnf => run("dnf", &["upgrade", "-y", &info.package_name]),
            PackageManager::Pacman => run("pacman", &["-Syu", "--noconfirm", &info.package_name]),
            PackageManager::Winget => run("winget", &["upgrade", "-e", "--id", &info.package_name]),
            PackageManager::Choco => run("choco", &["upgrade", &info.package_name, "-y"]),
            _ => {
                return Err(format!(
                    "Upgrade not supported for package manager {:?}",
                    info.package_manager
                ));
            }
        };

        result
            .map(|_| format!("Upgraded via {:?}", info.package_manager))
            .map_err(|e| format!("Upgrade failed: {:?}", e))
    } else {
        Err(format!("No upgrade method available for {}", name))
    }
}

fn get_installed_version(command: &str) -> Option<String> {
    for flag in ["--version", "-v", "-V", "version"] {
        if let Ok(output) = std::process::Command::new(command).arg(flag).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{}{}", stdout, stderr);

                if let Some(version) = extract_version(&combined) {
                    return Some(version);
                }
            }
        }
    }
    None
}

fn extract_version(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"v?(\d+\.\d+(?:\.\d+)?)").ok()?;
    re.captures(text).map(|c| c[1].to_string())
}

fn version_satisfies(current: &str, required: &str) -> bool {
    if required == "latest" {
        return false; // Always try to upgrade for "latest"
    }

    // Simple version comparison
    // Parse versions and compare
    let current_parts: Vec<u32> = current.split('.').filter_map(|p| p.parse().ok()).collect();
    let required_parts: Vec<u32> = required
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .split('.')
        .filter_map(|p| p.parse().ok())
        .collect();

    if current_parts.is_empty() || required_parts.is_empty() {
        return false;
    }

    // Compare each component
    for (c, r) in current_parts.iter().zip(required_parts.iter()) {
        if c > r {
            return true;
        }
        if c < r {
            return false;
        }
    }

    // If we get here, all compared parts are equal
    // Current satisfies if it has same or more parts
    current_parts.len() >= required_parts.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_satisfies() {
        assert!(version_satisfies("2.43.0", "2.40"));
        assert!(version_satisfies("2.43.0", "2.43.0"));
        assert!(!version_satisfies("2.40.0", "2.43"));
        assert!(!version_satisfies("1.0.0", "latest"));
    }

    #[test]
    fn test_upgrade_status_serialization() {
        let upgrade = ToolUpgrade {
            name: "test".to_string(),
            from_version: Some("1.0.0".to_string()),
            to_version: Some("2.0.0".to_string()),
            status: UpgradeStatus::Upgraded,
            message: None,
        };
        let json = serde_json::to_string(&upgrade).unwrap();
        assert!(json.contains("\"status\":\"upgraded\""));
    }

    #[test]
    fn test_upgrade_result_exit_codes() {
        let ok_result = UpgradeResult {
            tools: vec![],
            upgraded_count: 1,
            failed_count: 0,
            skipped_count: 0,
        };
        assert_eq!(ok_result.exit_code(), ExitCode::Ok);

        let warn_result = UpgradeResult {
            tools: vec![],
            upgraded_count: 0,
            failed_count: 0,
            skipped_count: 1,
        };
        assert_eq!(warn_result.exit_code(), ExitCode::Warning);

        let err_result = UpgradeResult {
            tools: vec![],
            upgraded_count: 0,
            failed_count: 1,
            skipped_count: 0,
        };
        assert_eq!(err_result.exit_code(), ExitCode::Error);
    }

    #[test]
    fn test_extract_version() {
        assert_eq!(
            extract_version("git version 2.43.0"),
            Some("2.43.0".to_string())
        );
        assert_eq!(extract_version("v20.11.0"), Some("20.11.0".to_string()));
    }
}
