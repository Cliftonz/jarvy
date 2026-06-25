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
    // Per-invocation correlation ID so the `tool.unsupported` event
    // emitted by this path can be stitched against any other Jarvy
    // log records the operator is debugging alongside. Mirrors the
    // `setup` span at `setup_cmd.rs:99` — without it, --request
    // events appear orphaned in the trace view.
    let run_id = uuid::Uuid::now_v7();
    let req_span = tracing::info_span!("tools.request", run_id = %run_id);
    let _req_span_guard = req_span.enter();

    // Normalize before lookup so `--request "GIT "` (trailing space,
    // case mismatch) hits the already-supported branch instead of
    // routing to the request path for a tool that exists.
    let normalized = name.trim();
    if let Err(reason) = tools::unsupported::validate_tool_name(normalized) {
        eprintln!(
            "[jarvy] refusing to process tool name: {}. \
             Tool names must match [A-Za-z0-9._-] and be 1–{} bytes.",
            reason,
            tools::unsupported::MAX_TOOL_NAME_LEN
        );
        return error_codes::CONFIG_ERROR;
    }
    let name = normalized;

    // If the tool *is* already supported (inventory OR registry —
    // covers nvm/rustup/brew custom installs and plugin tools), say
    // so rather than silently generating a request URL for it.
    if tools::spec::get_tool_spec(name).is_some()
        || crate::tools::registry::get_tool(name).is_some()
    {
        eprintln!(
            "[jarvy] `{}` is already supported. Run `jarvy tools` to list, or `jarvy explain {}` for details.",
            name, name
        );
        return error_codes::EXIT_SUCCESS;
    }

    // Explicit user action — fire telemetry regardless of consent gate.
    // `--request` IS the request mechanism. Telemetry is the canonical
    // channel because it requires no GitHub account from user or agent
    // and zero triage work from the maintainer.
    //
    // Branching on `counter_fired` is load-bearing: when telemetry was
    // never initialized (`JARVY_TELEMETRY=0` or never opted in), the
    // metric drops silently. Hardcoding `RequestChannel::Sent` in that
    // case would make the renderer print "Reported via telemetry" and
    // hide the GitHub URL — the same lie pattern the setup-path bug
    // fixed in the previous round. The fallback URL must be visible
    // when nothing actually went out.
    //
    // We build the report once with a placeholder channel, fire
    // telemetry, then rebuild with the truthful channel. The double-
    // build is acceptable on this one-shot user-typed path.
    let initial_report =
        tools::unsupported::build_report(name, None, tools::unsupported::RequestChannel::Sent);
    let counter_fired = telemetry::tool_request_explicit(name, &initial_report.suggestions);
    let channel = if counter_fired {
        tools::unsupported::RequestChannel::Sent
    } else {
        tools::unsupported::RequestChannel::Manual
    };
    let report = tools::unsupported::build_report(name, None, channel);

    // Emit the canonical `tool.unsupported` event with the same field
    // shape as the setup-path warn — single query covers both call sites.
    // `fallback_issue_url` is included only when the channel is manual
    // (the URL is the only remaining signal in that case); when
    // telemetry covered the request, the URL bloats the log line for
    // no operator benefit.
    if matches!(channel, tools::unsupported::RequestChannel::Manual) {
        tracing::warn!(
            event = "tool.unsupported",
            tool = %report.tool,
            source = %telemetry::Source::Request,
            platform = %std::env::consts::OS,
            suggestions = %report.suggestions.join(","),
            channel = %report.channel,
            fallback_issue_url = %report.fallback_issue_url,
            scaffold_cmd = %report.scaffold_cmd,
            exit_code = report.exit_code,
            opt_in_bypassed = true,
            counter_fired = counter_fired,
            "explicit tool request"
        );
    } else {
        tracing::warn!(
            event = "tool.unsupported",
            tool = %report.tool,
            source = %telemetry::Source::Request,
            platform = %std::env::consts::OS,
            suggestions = %report.suggestions.join(","),
            channel = %report.channel,
            scaffold_cmd = %report.scaffold_cmd,
            exit_code = report.exit_code,
            opt_in_bypassed = true,
            counter_fired = counter_fired,
            "explicit tool request"
        );
    }

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
            // run_request is the explicit-consent path, never seamless-
            // dependent — pass false unconditionally.
            s.push_str(&tools::unsupported::to_human(&report, channel, false));
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

/// Build the platform-native command that opens `url` in the default
/// browser. Extracted from `open_browser` for testability — the
/// constructor is pure (no spawn) and unit-testable per-platform.
///
/// Windows uses `rundll32 url.dll,FileProtocolHandler` rather than
/// `cmd /C start "" <url>`. The cmd.exe path is broken for URLs
/// containing `&` (every URL we build): `std::process::Command` on
/// Windows does not quote args containing only `&`, so cmd.exe
/// interprets `&` as a statement separator. `rundll32` invokes the
/// shell URL handler directly without cmd-style re-parsing.
fn browser_command(url: &str) -> std::process::Command {
    if cfg!(target_os = "macos") {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(url);
        cmd
    } else if cfg!(target_os = "windows") {
        // `url.dll,FileProtocolHandler` is the Win32 shell entry point
        // for "open this URL in the default handler". Bypasses cmd.exe
        // entirely so URL metacharacters (& ? = #) survive intact.
        let mut cmd = std::process::Command::new("rundll32");
        cmd.args(["url.dll,FileProtocolHandler", url]);
        cmd
    } else {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(url);
        cmd
    }
}

/// Best-effort browser opener. Never fails the parent command —
/// printing the URL is the contract; the browser launch is a
/// convenience.
fn open_browser(url: &str) {
    if browser_command(url).status().is_err() {
        eprintln!("[jarvy] could not open browser; copy this URL: {}", url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `browser_command` is a pure constructor — assert each platform
    /// branch picks the expected program. The synthesis plan flagged
    /// the Windows branch specifically because the cmd.exe path
    /// silently broke for URLs containing `&`.
    #[test]
    fn browser_command_picks_platform_program() {
        let cmd = browser_command("https://example.com");
        let program = cmd.get_program().to_string_lossy().to_string();
        if cfg!(target_os = "macos") {
            assert_eq!(program, "open");
        } else if cfg!(target_os = "windows") {
            assert_eq!(program, "rundll32");
            // Must include the shell URL handler entry point.
            let args: Vec<String> = cmd
                .get_args()
                .map(|a| a.to_string_lossy().to_string())
                .collect();
            assert_eq!(
                args.first().map(String::as_str),
                Some("url.dll,FileProtocolHandler")
            );
        } else {
            assert_eq!(program, "xdg-open");
        }
    }

    /// The URL is passed as a separate argv element on every platform
    /// — guards against any future refactor that interpolates it into
    /// a shell-string and reintroduces the command-injection vector.
    #[test]
    fn browser_command_passes_url_as_separate_arg() {
        let url = "https://example.com/?a=1&b=2";
        let cmd = browser_command(url);
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(
            args.iter().any(|a| a == url),
            "URL must appear verbatim as a separate argv element: {:?}",
            args
        );
    }

    /// The URL must be the LAST argv element — guards against a future
    /// refactor that adds an arg between program and URL (where
    /// attacker-controlled bytes in `--new-window=<x>` could land in
    /// the in-between slot). The order matters because tools like
    /// `xdg-open` and `open` accept flags before the URL.
    #[test]
    fn browser_command_url_is_last_argv_element() {
        let url = "https://example.com/?a=1&b=2";
        let cmd = browser_command(url);
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            args.last().map(String::as_str),
            Some(url),
            "URL must be the last argv element: {:?}",
            args
        );
    }

    /// Negative assertion — guards against a future change reintroducing
    /// the cmd.exe path on Windows that broke for URLs containing `&`.
    /// If `rundll32` ever fails-over to `cmd /C start` the test fails.
    #[cfg(target_os = "windows")]
    #[test]
    fn browser_command_never_references_cmd_exe() {
        let cmd = browser_command("https://a?b=1&c=2");
        let program = cmd.get_program().to_string_lossy().to_string();
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_ne!(program, "cmd", "must not invoke cmd.exe");
        assert!(
            !args.iter().any(|a| a == "/C" || a == "start"),
            "must not pass cmd.exe-style argv: {:?}",
            args
        );
    }
}
