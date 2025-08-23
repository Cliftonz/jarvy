use serde::Serialize;
use std::process::Command;

use crate::config::Config;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    NotInstalled,
    Match,
    Mismatch,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolReport {
    pub name: String,
    pub expected: String,
    pub installed: Option<String>,
    pub status: Status,
}

pub fn collect_reports(config: &Config) -> Vec<ToolReport> {
    let mut out = Vec::new();

    for (_id, tool) in config.get_tool_configs() {
        let expected = tool.version;
        let name = tool.name;
        let installed = detect_version(&name);
        let status = match &installed {
            None => Status::NotInstalled,
            Some(inst) => {
                if expected == "latest" {
                    Status::Match
                } else if inst.contains(&expected) {
                    Status::Match
                } else {
                    Status::Mismatch
                }
            }
        };
        out.push(ToolReport {
            name,
            expected,
            installed,
            status,
        });
    }
    out
}

fn detect_version(cmd: &str) -> Option<String> {
    // Check if command exists by attempting `cmd --version`
    let output = Command::new(cmd).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}
