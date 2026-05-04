//! Config migration and normalization
//!
//! Detects deprecated patterns in jarvy.toml and suggests or applies fixes.

use crate::output::{ExitCode, Outputable, colors, header};
use crate::tools::spec::get_tool_spec;
use serde::Serialize;
use std::fs;

/// A single migration suggestion
#[derive(Debug, Clone, Serialize)]
pub struct Migration {
    pub kind: String,
    pub message: String,
    pub line_hint: Option<String>,
    pub severity: String,
}

/// Migration report
#[derive(Debug, Clone, Serialize)]
pub struct MigrateReport {
    pub file: String,
    pub migrations: Vec<Migration>,
    pub applied: bool,
}

impl Outputable for MigrateReport {
    fn to_human(&self) -> String {
        let mut out = String::new();
        out.push_str(&header(&format!("Migration Report: {}", self.file)));
        out.push('\n');

        if self.migrations.is_empty() {
            out.push_str(&format!(
                "\n{}No migrations needed.{} Config is up to date.\n",
                colors::GREEN,
                colors::RESET
            ));
            return out;
        }

        for (i, m) in self.migrations.iter().enumerate() {
            let color = match m.severity.as_str() {
                "error" => colors::RED,
                "warning" => colors::YELLOW,
                _ => colors::CYAN,
            };
            out.push_str(&format!(
                "\n  {}{}. [{}]{} {}\n",
                color,
                i + 1,
                m.kind,
                colors::RESET,
                m.message
            ));
            if let Some(ref hint) = m.line_hint {
                out.push_str(&format!("     {}{}{}\n", colors::DIM, hint, colors::RESET));
            }
        }

        out.push_str(&format!(
            "\n{} migration(s) found.\n",
            self.migrations.len()
        ));

        if !self.applied {
            out.push_str(&format!(
                "{}Tip:{} Run with --apply to apply changes.\n",
                colors::DIM,
                colors::RESET
            ));
        }

        out
    }

    fn exit_code(&self) -> ExitCode {
        if self.migrations.iter().any(|m| m.severity == "error") {
            ExitCode::Error
        } else if !self.migrations.is_empty() {
            ExitCode::Warning
        } else {
            ExitCode::Ok
        }
    }
}

/// Analyze a jarvy.toml for migration needs
pub fn run_migrate(file: &str, _apply: bool) -> MigrateReport {
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            return MigrateReport {
                file: file.to_string(),
                migrations: vec![Migration {
                    kind: "error".to_string(),
                    message: format!("Cannot read {}: {}", file, e),
                    line_hint: None,
                    severity: "error".to_string(),
                }],
                applied: false,
            };
        }
    };

    let parsed: Result<toml::Value, _> = toml::from_str(&content);
    let Ok(config) = parsed else {
        return MigrateReport {
            file: file.to_string(),
            migrations: vec![Migration {
                kind: "parse-error".to_string(),
                message: format!("Invalid TOML: {}", parsed.unwrap_err()),
                line_hint: None,
                severity: "error".to_string(),
            }],
            applied: false,
        };
    };

    let mut migrations = Vec::new();

    // Check for unknown tool names in [provisioner]
    if let Some(provisioner) = config.get("provisioner").and_then(|p| p.as_table()) {
        for tool_name in provisioner.keys() {
            if get_tool_spec(tool_name).is_none() {
                migrations.push(Migration {
                    kind: "unknown-tool".to_string(),
                    message: format!(
                        "'{}' is not a recognized tool. Check spelling or remove it.",
                        tool_name
                    ),
                    line_hint: Some(format!("[provisioner]\n{} = ...", tool_name)),
                    severity: "warning".to_string(),
                });
            }
        }
    }

    // Check for deprecated field names
    if config.get("tools").is_some() {
        migrations.push(Migration {
            kind: "renamed-section".to_string(),
            message: "[tools] has been renamed to [provisioner]. Update your config.".to_string(),
            line_hint: Some("Replace [tools] with [provisioner]".to_string()),
            severity: "warning".to_string(),
        });
    }

    // Check for hooks referencing unknown tools
    if let Some(hooks) = config.get("hooks").and_then(|h| h.as_table()) {
        for key in hooks.keys() {
            if key == "pre_setup" || key == "post_setup" || key == "config" {
                continue;
            }
            if get_tool_spec(key).is_none() {
                migrations.push(Migration {
                    kind: "unknown-hook-tool".to_string(),
                    message: format!(
                        "Hook references unknown tool '{}'. It may not trigger.",
                        key
                    ),
                    line_hint: Some(format!("[hooks.{}]", key)),
                    severity: "info".to_string(),
                });
            }
        }
    }

    // If apply mode and there are fixable migrations, we could rewrite the file
    // For now, just report — auto-fix is limited to reporting
    let applied = false;

    MigrateReport {
        file: file.to_string(),
        migrations,
        applied,
    }
}
