//! Tools command handler - list supported tools and tool index

use std::fs;

use crate::cli::OutputFormat;
use crate::error_codes;
use crate::telemetry;
use crate::tools;

/// Run the tools command.
///
/// Returns a process exit code so callers (main.rs) can propagate it.
/// `--request` always returns 0 — generating an issue URL isn't an error
/// even though the underlying situation is "unsupported tool". The exit
/// code 8 signal lives on the `setup` path, not the helper command.
pub fn run_tools(
    index: bool,
    default_hooks: bool,
    request: Option<&str>,
    open: bool,
    output_format: OutputFormat,
    output: Option<&str>,
) -> i32 {
    // --request short-circuits all other modes: it's a "how do I ask for
    // this tool?" helper, not a listing command.
    if let Some(name) = request {
        return run_request(name, open, output_format, output);
    }

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
    error_codes::EXIT_SUCCESS
}

/// Handle `jarvy tools --request <name> [--open]`.
///
/// Prints (or writes) the structured `UnsupportedToolReport` so the user
/// — or an AI agent — has everything needed to file a request and start
/// scaffolding: pre-filled issue URL, fuzzy suggestions for typos, the
/// `cargo-jarvy new-tool` command, and a `define_tool!` snippet.
fn run_request(name: &str, open: bool, output_format: OutputFormat, output: Option<&str>) -> i32 {
    // If the tool *is* already supported, say so rather than silently
    // generating a request URL for it. Saves the user from filing dupes.
    if tools::spec::get_tool_spec(name).is_some()
        || crate::tools::registry::get_tool(name).is_some()
    {
        eprintln!(
            "[jarvy] `{}` is already supported. Run `jarvy tools` to list, or `jarvy explain {}` for details.",
            name, name
        );
        return error_codes::EXIT_SUCCESS;
    }

    // Explicit user action — fire telemetry regardless of opt-in.
    // `--request` IS the request mechanism. Telemetry is the canonical
    // channel because it requires no GitHub account from user or agent
    // and zero triage work from the maintainer. The fallback URL stays
    // in the payload for users who explicitly want a public record.
    let channel = tools::unsupported::RequestChannel::Sent;
    let report = tools::unsupported::build_report(name, None, channel);
    telemetry::tool_request_explicit(name, &report.suggestions);

    let snippet = tools::unsupported::scaffold_snippet(name);

    let content = match output_format {
        OutputFormat::Json => {
            #[derive(serde::Serialize)]
            struct RequestPayload<'a> {
                #[serde(flatten)]
                report: &'a tools::unsupported::UnsupportedToolReport,
                snippet: String,
            }
            serde_json::to_string_pretty(&RequestPayload {
                report: &report,
                snippet,
            })
            .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
        }
        OutputFormat::Yaml => {
            serde_yaml::to_string(&report).unwrap_or_else(|e| format!("error: {}", e))
        }
        OutputFormat::Toml => {
            // Wrap because TOML can't represent a top-level array of
            // suggestions cleanly mixed with scalars in serde_json::Value.
            let wrapper = serde_json::json!({
                "report": &report,
                "snippet": snippet,
            });
            toml::to_string(&wrapper).unwrap_or_else(|e| format!("error = \"{}\"", e))
        }
        OutputFormat::Pretty => {
            let mut s = String::with_capacity(512);
            s.push_str(&tools::unsupported::to_human(&report, channel));
            if !report.suggestions.is_empty() {
                s.push_str("\nIf one of those is the tool you wanted, edit your jarvy.toml.\n");
            }
            s.push_str("\nScaffold snippet (drop into src/tools/");
            s.push_str(&name.to_ascii_lowercase());
            s.push_str("/):\n\n");
            s.push_str(&snippet);
            s
        }
    };

    if let Some(path) = output {
        if let Err(e) = fs::write(path, &content) {
            eprintln!("Failed to write output: {}", e);
        }
    } else {
        println!("{}", content);
    }

    if open {
        open_browser(&report.fallback_issue_url);
    }

    error_codes::EXIT_SUCCESS
}

/// Best-effort browser opener. Uses platform-native commands and never
/// fails the parent command — printing the URL is the contract; the
/// browser launch is a convenience.
fn open_browser(url: &str) {
    let result = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()
    } else {
        std::process::Command::new("xdg-open").arg(url).status()
    };
    if result.is_err() {
        eprintln!("[jarvy] could not open browser; copy this URL: {}", url);
    }
}
