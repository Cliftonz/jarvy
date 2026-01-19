//! Tools command handler - list supported tools and tool index

use std::fs;

use crate::cli::OutputFormat;
use crate::tools;

/// Run the tools command
pub fn run_tools(
    index: bool,
    default_hooks: bool,
    output_format: OutputFormat,
    output: Option<&str>,
) {
    let content = if default_hooks {
        // List tools with default hooks
        let hooks_list = tools::spec::list_tools_with_default_hooks();

        #[derive(serde::Serialize)]
        struct HookInfo {
            tool: String,
            description: String,
            script: String,
            platform: Option<String>,
        }

        let hook_infos: Vec<HookInfo> = hooks_list
            .iter()
            .map(|(name, hook)| HookInfo {
                tool: name.to_string(),
                description: hook.description.to_string(),
                script: hook.script.to_string(),
                platform: hook.platform.map(|p| p.to_string()),
            })
            .collect();

        match output_format {
            OutputFormat::Json => serde_json::to_string_pretty(&hook_infos)
                .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e)),
            OutputFormat::Yaml => {
                serde_yaml::to_string(&hook_infos).unwrap_or_else(|e| format!("error: {}", e))
            }
            OutputFormat::Toml => {
                let wrapper = serde_json::json!({ "hooks": hook_infos });
                toml::to_string(&wrapper).unwrap_or_else(|e| format!("error = \"{}\"", e))
            }
            OutputFormat::Pretty => {
                let mut s = String::new();
                s.push_str(&format!(
                    "Tools with default hooks ({} tools):\n",
                    hooks_list.len()
                ));
                s.push_str("─".repeat(60).as_str());
                s.push('\n');
                for (name, hook) in &hooks_list {
                    s.push_str(&format!("\n{}\n", name));
                    s.push_str(&format!("  Description: {}\n", hook.description));
                    if let Some(platform) = hook.platform {
                        s.push_str(&format!("  Platform: {}\n", platform));
                    }
                    s.push_str("  Script:\n");
                    for line in hook.script.lines().take(5) {
                        if !line.trim().is_empty() {
                            s.push_str(&format!("    {}\n", line));
                        }
                    }
                    if hook.script.lines().count() > 5 {
                        s.push_str("    ...\n");
                    }
                }
                s
            }
        }
    } else if index {
        // Output the full tool index
        let tool_index = tools::spec::generate_tool_index();
        match output_format {
            OutputFormat::Json => serde_json::to_string_pretty(&tool_index)
                .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e)),
            OutputFormat::Yaml => {
                serde_yaml::to_string(&tool_index).unwrap_or_else(|e| format!("error: {}", e))
            }
            OutputFormat::Toml => {
                toml::to_string(&tool_index).unwrap_or_else(|e| format!("error = \"{}\"", e))
            }
            OutputFormat::Pretty => {
                let mut s = String::new();
                s.push_str(&format!(
                    "Tool Index v{} ({} tools)\n",
                    tool_index.version, tool_index.count
                ));
                s.push_str("─".repeat(50).as_str());
                s.push('\n');
                for tool in &tool_index.tools {
                    let platforms = [
                        tool.macos.as_ref().map(|_| "macOS"),
                        tool.linux.as_ref().map(|_| "Linux"),
                        tool.windows.as_ref().map(|_| "Windows"),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(", ");
                    let custom = if tool.custom_install.has_custom_installer {
                        " [custom]"
                    } else {
                        ""
                    };
                    s.push_str(&format!("{:<15} ({}){}  \n", tool.name, platforms, custom));
                }
                s
            }
        }
    } else {
        // Just list tool names
        let names = tools::spec::list_tool_names();
        match output_format {
            OutputFormat::Json => serde_json::to_string_pretty(&names)
                .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e)),
            OutputFormat::Yaml => {
                serde_yaml::to_string(&names).unwrap_or_else(|e| format!("error: {}", e))
            }
            OutputFormat::Toml => {
                // TOML doesn't support bare arrays at root, wrap it
                let wrapper = serde_json::json!({ "tools": names });
                toml::to_string(&wrapper).unwrap_or_else(|e| format!("error = \"{}\"", e))
            }
            OutputFormat::Pretty => {
                let mut s = String::new();
                s.push_str(&format!("Supported tools ({}):\n", names.len()));
                for name in &names {
                    s.push_str(&format!("  - {}\n", name));
                }
                s
            }
        }
    };

    if let Some(path) = output {
        if let Err(e) = fs::write(path, &content) {
            eprintln!("Failed to write output: {}", e);
        }
    } else {
        println!("{}", content);
    }
}
