//! Drift remediation functionality

use serde::{Deserialize, Serialize};

use super::{DriftReport, MissingTool, VersionChange};

/// Drift fixer for remediating detected issues
pub struct DriftFixer {
    /// Whether to run in dry-run mode (no actual changes)
    pub dry_run: bool,
}

/// Result of a fix operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixResult {
    /// Status of the fix
    pub status: FixStatus,
    /// Tool or file that was fixed
    pub target: String,
    /// Description of what was done
    pub message: String,
}

/// Status of a fix operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixStatus {
    /// Fix was successful
    Success,
    /// Fix was skipped (not auto-fixable)
    Skipped,
    /// Fix failed
    Failed,
    /// Dry run - would have fixed
    DryRun,
}

impl DriftFixer {
    /// Create a new drift fixer
    pub fn new(dry_run: bool) -> Self {
        Self { dry_run }
    }

    /// Attempt to fix all auto-fixable issues in a drift report
    pub fn fix_all(&self, report: &DriftReport) -> Vec<FixResult> {
        let mut results = Vec::new();

        // Fix missing tools
        for tool in &report.missing_tools {
            results.push(self.fix_missing_tool(tool));
        }

        // Fix version changes
        for change in &report.version_changes {
            results.push(self.fix_version_change(change));
        }

        // Note: Changed files are not auto-fixable
        for file in &report.changed_files {
            results.push(FixResult {
                status: FixStatus::Skipped,
                target: file.path.clone(),
                message: "file changes require manual review".to_string(),
            });
        }

        results
    }

    /// Fix a missing tool by reinstalling it
    fn fix_missing_tool(&self, tool: &MissingTool) -> FixResult {
        if !tool.auto_fixable {
            return FixResult {
                status: FixStatus::Skipped,
                target: tool.tool.clone(),
                message: "tool requires manual installation".to_string(),
            };
        }

        if self.dry_run {
            return FixResult {
                status: FixStatus::DryRun,
                target: tool.tool.clone(),
                message: format!("would install {} {}", tool.tool, tool.expected_version),
            };
        }

        // Attempt to reinstall the tool using the registry
        match self.install_tool(&tool.tool, &tool.expected_version) {
            Ok(()) => FixResult {
                status: FixStatus::Success,
                target: tool.tool.clone(),
                message: format!("installed {} {}", tool.tool, tool.expected_version),
            },
            Err(e) => FixResult {
                status: FixStatus::Failed,
                target: tool.tool.clone(),
                message: format!("failed to install: {}", e),
            },
        }
    }

    /// Fix a version change by reinstalling the expected version
    fn fix_version_change(&self, change: &VersionChange) -> FixResult {
        if !change.auto_fixable {
            return FixResult {
                status: FixStatus::Skipped,
                target: change.tool.clone(),
                message: "version change requires manual intervention".to_string(),
            };
        }

        if self.dry_run {
            return FixResult {
                status: FixStatus::DryRun,
                target: change.tool.clone(),
                message: format!(
                    "would reinstall {} {} (currently {})",
                    change.tool, change.expected, change.actual
                ),
            };
        }

        // Attempt to reinstall the expected version
        match self.install_tool(&change.tool, &change.expected) {
            Ok(()) => FixResult {
                status: FixStatus::Success,
                target: change.tool.clone(),
                message: format!(
                    "reinstalled {} {} (was {})",
                    change.tool, change.expected, change.actual
                ),
            },
            Err(e) => FixResult {
                status: FixStatus::Failed,
                target: change.tool.clone(),
                message: format!("failed to reinstall: {}", e),
            },
        }
    }

    /// Install a tool using the tool registry
    fn install_tool(&self, tool_name: &str, version: &str) -> Result<(), String> {
        // Try to get the tool handler from the registry and run installation
        use crate::tools::registry::get_tool;

        if let Some(handler) = get_tool(tool_name) {
            handler(version).map_err(|e| format!("{}", e))
        } else {
            Err(format!("no handler found for tool '{}'", tool_name))
        }
    }

    /// Print a summary of fix results
    pub fn print_summary(results: &[FixResult]) {
        let success = results
            .iter()
            .filter(|r| r.status == FixStatus::Success)
            .count();
        let failed = results
            .iter()
            .filter(|r| r.status == FixStatus::Failed)
            .count();
        let skipped = results
            .iter()
            .filter(|r| r.status == FixStatus::Skipped)
            .count();
        let dry_run = results
            .iter()
            .filter(|r| r.status == FixStatus::DryRun)
            .count();

        println!();
        println!("\x1b[1mFix Summary:\x1b[0m");

        for result in results {
            let symbol = match result.status {
                FixStatus::Success => "\x1b[32m✓\x1b[0m",
                FixStatus::Failed => "\x1b[31m✗\x1b[0m",
                FixStatus::Skipped => "\x1b[33m-\x1b[0m",
                FixStatus::DryRun => "\x1b[36m○\x1b[0m",
            };
            println!("  {} {}: {}", symbol, result.target, result.message);
        }

        println!();
        if dry_run > 0 {
            println!(
                "  Dry run: {} action{} would be taken",
                dry_run,
                if dry_run == 1 { "" } else { "s" }
            );
        } else {
            println!(
                "  {} succeeded, {} failed, {} skipped",
                success, failed, skipped
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::VersionDirection;

    #[test]
    fn test_fix_result_serialization() {
        let result = FixResult {
            status: FixStatus::Success,
            target: "node".to_string(),
            message: "installed successfully".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\""));
    }

    #[test]
    fn test_dry_run_missing_tool() {
        let fixer = DriftFixer::new(true);
        let tool = MissingTool {
            tool: "node".to_string(),
            expected_version: "20.0.0".to_string(),
            auto_fixable: true,
        };

        let result = fixer.fix_missing_tool(&tool);
        assert_eq!(result.status, FixStatus::DryRun);
        assert!(result.message.contains("would install"));
    }

    #[test]
    fn test_skip_non_fixable() {
        let fixer = DriftFixer::new(false);
        let tool = MissingTool {
            tool: "rustup".to_string(),
            expected_version: "1.0.0".to_string(),
            auto_fixable: false,
        };

        let result = fixer.fix_missing_tool(&tool);
        assert_eq!(result.status, FixStatus::Skipped);
    }

    #[test]
    fn test_dry_run_version_change() {
        let fixer = DriftFixer::new(true);
        let change = VersionChange {
            tool: "node".to_string(),
            expected: "20.0.0".to_string(),
            actual: "21.0.0".to_string(),
            direction: VersionDirection::Upgrade,
            auto_fixable: true,
            reason: None,
        };

        let result = fixer.fix_version_change(&change);
        assert_eq!(result.status, FixStatus::DryRun);
        assert!(result.message.contains("would reinstall"));
    }
}
