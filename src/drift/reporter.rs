//! Drift report formatting and output

use super::{DriftReport, DriftStatus, VersionDirection};

/// Report formatter for drift detection results
pub struct DriftReporter;

impl DriftReporter {
    /// Print a human-readable report to stdout
    pub fn print_report(report: &DriftReport) {
        match report.status {
            DriftStatus::NoDrift => {
                println!("\x1b[32m✓\x1b[0m No configuration drift detected");
                println!("  Last checked: {}", format_timestamp(&report.timestamp));
                return;
            }
            DriftStatus::NoBaseline => {
                println!("\x1b[33m⚠\x1b[0m No baseline state found");
                println!("  Run 'jarvy setup' to capture the initial state");
                return;
            }
            DriftStatus::DriftDetected => {
                println!(
                    "\x1b[31m✗\x1b[0m Configuration drift detected ({} issue{})",
                    report.summary.total_issues,
                    if report.summary.total_issues == 1 {
                        ""
                    } else {
                        "s"
                    }
                );
            }
        }

        // Version changes
        if !report.version_changes.is_empty() {
            println!("\n\x1b[1mVersion Changes:\x1b[0m");
            for change in &report.version_changes {
                let direction_symbol = match change.direction {
                    VersionDirection::Upgrade => "\x1b[33m↑\x1b[0m", // Yellow for upgrade
                    VersionDirection::Downgrade => "\x1b[31m↓\x1b[0m", // Red for downgrade
                };
                let fixable = if change.auto_fixable {
                    " \x1b[36m[auto-fixable]\x1b[0m"
                } else {
                    ""
                };
                println!(
                    "  {} {} {} → {}{}",
                    direction_symbol, change.tool, change.expected, change.actual, fixable
                );
                if let Some(ref reason) = change.reason {
                    println!("    Reason: {}", reason);
                }
            }
        }

        // Missing tools
        if !report.missing_tools.is_empty() {
            println!("\n\x1b[1mMissing Tools:\x1b[0m");
            for tool in &report.missing_tools {
                let fixable = if tool.auto_fixable {
                    " \x1b[36m[auto-fixable]\x1b[0m"
                } else {
                    ""
                };
                println!(
                    "  \x1b[31m✗\x1b[0m {} (expected {}){}",
                    tool.tool, tool.expected_version, fixable
                );
            }
        }

        // Extra tools
        if !report.extra_tools.is_empty() {
            println!("\n\x1b[1mExtra Tools (not in config):\x1b[0m");
            for tool in &report.extra_tools {
                println!("  \x1b[33m?\x1b[0m {} {}", tool.tool, tool.version);
            }
        }

        // Changed files
        if !report.changed_files.is_empty() {
            println!("\n\x1b[1mChanged Files:\x1b[0m");
            for file in &report.changed_files {
                let status = if file.actual_hash == "missing" {
                    "\x1b[31mmissing\x1b[0m"
                } else {
                    "\x1b[33mmodified\x1b[0m"
                };
                println!("  {} {}", status, file.path);
            }
        }

        // Summary
        println!();
        let auto_fixable = report
            .version_changes
            .iter()
            .filter(|c| c.auto_fixable)
            .count()
            + report
                .missing_tools
                .iter()
                .filter(|t| t.auto_fixable)
                .count();

        if auto_fixable > 0 {
            println!(
                "\x1b[36mℹ\x1b[0m {} issue{} can be auto-fixed with 'jarvy drift fix'",
                auto_fixable,
                if auto_fixable == 1 { "" } else { "s" }
            );
        }
    }

    /// Convert report to JSON string
    pub fn to_json(report: &DriftReport) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(report)
    }

    /// Convert report to compact JSON string
    #[allow(dead_code)]
    pub fn to_json_compact(report: &DriftReport) -> Result<String, serde_json::Error> {
        serde_json::to_string(report)
    }
}

/// Format a Unix timestamp for display
fn format_timestamp(timestamp: &str) -> String {
    // The timestamp is in format "1234567890Z" (Unix seconds)
    if let Some(secs_str) = timestamp.strip_suffix('Z') {
        if let Ok(secs) = secs_str.parse::<u64>() {
            // Simple relative time formatting
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let diff = now.saturating_sub(secs);
            if diff < 60 {
                return "just now".to_string();
            } else if diff < 3600 {
                let mins = diff / 60;
                return format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" });
            } else if diff < 86400 {
                let hours = diff / 3600;
                return format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" });
            } else {
                let days = diff / 86400;
                return format!("{} day{} ago", days, if days == 1 { "" } else { "s" });
            }
        }
    }
    timestamp.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::{ChangedFile, DriftSummary, MissingTool, VersionChange};

    #[test]
    fn test_to_json() {
        let report = DriftReport {
            timestamp: "1234567890Z".to_string(),
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

        let json = DriftReporter::to_json(&report).unwrap();
        assert!(json.contains("\"status\": \"no_drift\""));
    }

    #[test]
    fn test_format_timestamp() {
        // Test relative time formatting
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let just_now = format!("{}Z", now);
        assert_eq!(format_timestamp(&just_now), "just now");

        let five_mins_ago = format!("{}Z", now - 300);
        assert!(format_timestamp(&five_mins_ago).contains("minute"));
    }

    #[test]
    fn test_json_with_drift() {
        let report = DriftReport {
            timestamp: "1234567890Z".to_string(),
            status: DriftStatus::DriftDetected,
            summary: DriftSummary {
                total_issues: 2,
                version_changes: 1,
                missing_tools: 1,
                extra_tools: 0,
                changed_files: 0,
            },
            version_changes: vec![VersionChange {
                tool: "node".to_string(),
                expected: "20.0.0".to_string(),
                actual: "21.0.0".to_string(),
                direction: VersionDirection::Upgrade,
                auto_fixable: true,
                reason: None,
            }],
            missing_tools: vec![MissingTool {
                tool: "docker".to_string(),
                expected_version: "24.0.0".to_string(),
                auto_fixable: true,
            }],
            extra_tools: Vec::new(),
            changed_files: Vec::new(),
        };

        let json = DriftReporter::to_json(&report).unwrap();
        assert!(json.contains("\"drift_detected\""));
        assert!(json.contains("\"node\""));
        assert!(json.contains("\"docker\""));
    }
}
