//! Interactive project initialization wizard
//!
//! Creates a jarvy.toml configuration file through an interactive process
//! or from a template.

use crate::onboarding::detection::{ProjectType, detect_project_type};
use crate::output::{ExitCode, Outputable};
use crate::tools::spec::iter_tools;
use inquire::{MultiSelect, Select};
use serde::Serialize;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

/// Development stack categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackCategory {
    WebFrontend,
    BackendApi,
    FullStack,
    Mobile,
    DataScience,
    DevOps,
    Custom,
}

impl StackCategory {
    /// Get display name for the category
    pub fn display_name(&self) -> &'static str {
        match self {
            StackCategory::WebFrontend => "Web Frontend (React, Vue, Angular)",
            StackCategory::BackendApi => "Backend API (Node, Go, Rust, Python)",
            StackCategory::FullStack => "Full Stack (Frontend + Backend)",
            StackCategory::Mobile => "Mobile Development",
            StackCategory::DataScience => "Data Science / ML",
            StackCategory::DevOps => "DevOps / Infrastructure",
            StackCategory::Custom => "Custom (start from scratch)",
        }
    }

    /// Get default tools for this category
    pub fn default_tools(&self) -> Vec<&'static str> {
        match self {
            StackCategory::WebFrontend => vec!["git", "node", "docker", "jq", "ripgrep", "fzf"],
            StackCategory::BackendApi => vec!["git", "docker", "jq", "ripgrep", "httpie", "curl"],
            StackCategory::FullStack => {
                vec!["git", "node", "docker", "jq", "ripgrep", "httpie", "redis"]
            }
            StackCategory::Mobile => vec!["git", "node", "docker", "jq", "ripgrep"],
            StackCategory::DataScience => vec!["git", "python", "docker", "jq", "duckdb"],
            StackCategory::DevOps => {
                vec!["git", "docker", "kubectl", "terraform", "aws", "jq", "yq"]
            }
            StackCategory::Custom => vec!["git"],
        }
    }

    /// Get all stack categories
    pub fn all() -> Vec<StackCategory> {
        vec![
            StackCategory::WebFrontend,
            StackCategory::BackendApi,
            StackCategory::FullStack,
            StackCategory::Mobile,
            StackCategory::DataScience,
            StackCategory::DevOps,
            StackCategory::Custom,
        ]
    }
}

impl std::fmt::Display for StackCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Options for the init command
#[derive(Debug, Clone, Default)]
pub struct InitOptions {
    /// Template name to use (skips interactive mode)
    pub template: Option<String>,
    /// Non-interactive mode (requires template)
    pub non_interactive: bool,
    /// Output to stdout instead of file
    pub stdout: bool,
    /// Output file path
    pub output: Option<PathBuf>,
}

/// Result of the init command
#[derive(Debug, Clone, Serialize)]
pub struct InitResult {
    /// Path to created file (if written to file)
    pub output_path: Option<String>,
    /// Number of tools in the config
    pub tool_count: usize,
    /// Whether the file was created or already existed
    pub created: bool,
    /// The generated TOML content (for display)
    #[serde(skip)]
    pub content: String,
    /// Whether to output to stdout
    #[serde(skip)]
    pub stdout: bool,
}

impl Outputable for InitResult {
    fn to_human(&self) -> String {
        if self.stdout {
            return self.content.clone();
        }

        let mut output = String::new();
        if self.created {
            if let Some(ref path) = self.output_path {
                output.push_str(&format!(
                    "\n\x1b[32m✓\x1b[0m Created {} with {} tools\n\n",
                    path, self.tool_count
                ));
            }
            output.push_str("Next steps:\n");
            output.push_str("  1. Review your config: \x1b[36mcat jarvy.toml\x1b[0m\n");
            output.push_str("  2. Install tools: \x1b[36mjarvy setup\x1b[0m\n");
        } else {
            output.push_str("\n\x1b[33m!\x1b[0m jarvy.toml already exists. Use --output to specify a different path.\n");
        }
        output
    }

    fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    }

    fn exit_code(&self) -> ExitCode {
        if self.created || self.stdout {
            ExitCode::Ok
        } else {
            ExitCode::Warning
        }
    }
}

/// Run the init command
pub fn run_init(options: InitOptions) -> InitResult {
    // If non-interactive and no template, error
    if options.non_interactive && options.template.is_none() {
        return InitResult {
            output_path: None,
            tool_count: 0,
            created: false,
            content: String::new(),
            stdout: false,
        };
    }

    // Check if running in a TTY
    let is_tty = io::stdin().is_terminal();

    // If template provided, use it directly
    if let Some(ref template_name) = options.template {
        return run_init_with_template(template_name, &options);
    }

    // Non-interactive without template
    if !is_tty || options.non_interactive {
        eprintln!("Error: Interactive mode requires a TTY. Use --template or --non-interactive.");
        return InitResult {
            output_path: None,
            tool_count: 0,
            created: false,
            content: String::new(),
            stdout: false,
        };
    }

    // Run interactive wizard
    run_init_interactive(options)
}

/// Run init with a template
fn run_init_with_template(template_name: &str, options: &InitOptions) -> InitResult {
    use crate::templates::builtin::get_builtin_template;

    // Load template from builtin templates
    let template = match get_builtin_template(template_name) {
        Some(t) => t,
        None => {
            eprintln!("Error: Template '{}' not found.", template_name);
            eprintln!("Run 'jarvy templates' to see available templates.");
            return InitResult {
                output_path: None,
                tool_count: 0,
                created: false,
                content: String::new(),
                stdout: false,
            };
        }
    };

    // Convert template to jarvy.toml content
    let full_template = template.to_template();
    let content = full_template.to_jarvy_toml();

    let result = write_config(&content, options);

    // Emit the adoption event under `template.materialized` (same event
    // as `templates use`) with `source = "init"` so the two entry points
    // for the same underlying template are queryable together but the
    // init-adoption cohort is time-sliceable — matters for the mobile
    // template overhaul, whose whole product bet is the `jarvy init
    // --template react-native` flow.
    if result.created && crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "template.materialized",
            template = %full_template.template.name,
            category = %full_template.template.category,
            tool_count = result.tool_count,
            source = "init",
            output_path = %result.output_path.as_deref().unwrap_or("<stdout>"),
        );
    }

    result
}

/// Run interactive wizard
fn run_init_interactive(options: InitOptions) -> InitResult {
    println!();
    println!("\x1b[1mWelcome to Jarvy!\x1b[0m Let's set up your development environment.");
    println!();

    // Detect existing project
    let detected = detect_project_type(".");
    if detected.primary != ProjectType::Unknown {
        println!(
            "  \x1b[36mDetected:\x1b[0m {} project (found {})",
            detected.primary.display_name(),
            detected.detection_files.join(", ")
        );
        println!();
    }

    // Stack selection
    let stacks: Vec<StackCategory> = StackCategory::all();
    let stack = match Select::new("What type of project is this?", stacks).prompt() {
        Ok(s) => s,
        Err(_) => {
            return InitResult {
                output_path: None,
                tool_count: 0,
                created: false,
                content: String::new(),
                stdout: false,
            };
        }
    };

    // Get available tools
    let available_tools: Vec<String> = iter_tools().map(|t| t.spec.name.to_string()).collect();

    // Get default tools for selected stack
    let default_tools = stack.default_tools();
    let defaults: Vec<usize> = default_tools
        .iter()
        .filter_map(|t| available_tools.iter().position(|a| a == *t))
        .collect();

    // Tool selection
    let tool_options: Vec<&str> = available_tools.iter().map(|s| s.as_str()).collect();
    let selected = match MultiSelect::new("Select tools to install:", tool_options)
        .with_default(&defaults)
        .with_page_size(15)
        .prompt()
    {
        Ok(s) => s,
        Err(_) => {
            return InitResult {
                output_path: None,
                tool_count: 0,
                created: false,
                content: String::new(),
                stdout: false,
            };
        }
    };

    let content = generate_config(&selected, None);
    write_config(&content, &options)
}

/// Generate jarvy.toml content from selected tools
fn generate_config(tools: &[&str], template_name: Option<&str>) -> String {
    let mut content = String::new();

    content.push_str("# Generated by jarvy init\n");
    if let Some(template) = template_name {
        content.push_str(&format!("# Template: {}\n", template));
    }
    content.push('\n');
    content.push_str("[provisioner]\n");

    for tool in tools {
        content.push_str(&format!("{} = \"latest\"\n", tool));
    }

    content
}

/// Write config to file or stdout
fn write_config(content: &str, options: &InitOptions) -> InitResult {
    if options.stdout {
        return InitResult {
            output_path: None,
            tool_count: count_provisioner_tools(content),
            created: true,
            content: content.to_string(),
            stdout: true,
        };
    }

    let output_path = options
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from("jarvy.toml"));

    // Check if file exists
    if output_path.exists() && options.output.is_none() {
        return InitResult {
            output_path: Some(output_path.display().to_string()),
            tool_count: 0,
            created: false,
            content: content.to_string(),
            stdout: false,
        };
    }

    // Write file
    match fs::write(&output_path, content) {
        Ok(()) => InitResult {
            output_path: Some(output_path.display().to_string()),
            tool_count: count_provisioner_tools(content),
            created: true,
            content: content.to_string(),
            stdout: false,
        },
        Err(e) => {
            eprintln!("Error writing file: {}", e);
            InitResult {
                output_path: None,
                tool_count: 0,
                created: false,
                content: String::new(),
                stdout: false,
            }
        }
    }
}

/// Count LEAF tool entries under `[provisioner]`. Subtables like
/// `[provisioner.overrides]` are metadata about tool groups, not
/// themselves tools — filtering them out keeps the "N tools installed"
/// output honest as templates grow. Malformed TOML returns 0 rather
/// than panicking (init's other errors surface before this counter is
/// consulted).
fn count_provisioner_tools(content: &str) -> usize {
    toml::from_str::<toml::Value>(content)
        .ok()
        .and_then(|value| {
            value
                .get("provisioner")
                .and_then(toml::Value::as_table)
                .map(|table| table.values().filter(|v| !v.is_table()).count())
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_count_excludes_generated_commands() {
        let content = r#"
            [provisioner]
            java = "17"
            node = "20"
            pnpm = "latest"

            [commands]
            android = "pnpm --filter mobile android"
            "pre:android" = "jarvy doctor --check tools"
        "#;

        assert_eq!(count_provisioner_tools(content), 3);
    }

    #[test]
    fn tool_count_ignores_provisioner_subtables() {
        // `[provisioner.overrides]` is metadata about tool groups, not
        // itself a tool. Counting it would silently inflate the
        // user-facing "N tools installed" claim as templates grow.
        let content = r#"
            [provisioner]
            java = "17"

            [provisioner.overrides]
            foo = "bar"
        "#;
        assert_eq!(count_provisioner_tools(content), 1);
    }

    #[test]
    fn tool_count_returns_zero_on_malformed_toml() {
        // Init's other error paths surface before this counter is
        // consulted, so silent 0 is the right fallback. Regression
        // guard against a future refactor swapping in `.unwrap()`.
        assert_eq!(count_provisioner_tools("[provisioner\ngarbage"), 0);
    }

    #[test]
    fn tool_count_zero_for_empty_provisioner() {
        assert_eq!(count_provisioner_tools("[provisioner]\n"), 0);
    }

    #[test]
    fn test_stack_category_display() {
        assert_eq!(
            StackCategory::WebFrontend.display_name(),
            "Web Frontend (React, Vue, Angular)"
        );
        assert_eq!(
            StackCategory::Custom.display_name(),
            "Custom (start from scratch)"
        );
    }

    #[test]
    fn test_stack_category_default_tools() {
        let tools = StackCategory::WebFrontend.default_tools();
        assert!(tools.contains(&"git"));
        assert!(tools.contains(&"node"));
    }

    #[test]
    fn test_generate_config() {
        let tools = vec!["git", "node", "docker"];
        let content = generate_config(&tools, None);
        assert!(content.contains("[provisioner]"));
        assert!(content.contains("git = \"latest\""));
        assert!(content.contains("node = \"latest\""));
        assert!(content.contains("docker = \"latest\""));
    }

    #[test]
    fn test_generate_config_with_template() {
        let tools = vec!["git"];
        let content = generate_config(&tools, Some("react"));
        assert!(content.contains("# Template: react"));
    }
}
