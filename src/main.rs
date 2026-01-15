use crate::analytics::init_logging;
use crate::config::{Config, create_default_config, EnvValue};
use crate::env::{
    DotenvConfig, EnvContext, SecretsConfig, ShellConfig, ShellType,
    collect_secrets, detect_shell, expand_value, generate_dotenv, parse_shell, preview_dotenv,
    preview_shell_rc, update_shell_rc,
};
use crate::hooks::{Hook, HookConfig, HookEnv};
use crate::init::initialize;
use crate::report::{Status, ToolReport, collect_reports};
use crate::setup::setup;
use clap::{Parser, Subcommand, ValueEnum};
use inquire::{InquireError, Select};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;

mod analytics;
mod bootstrap;
mod config;
mod env;
mod error_codes;
mod hooks;
mod init;
mod os_setup;
mod outputs;
mod posthog;
mod provisioner;
mod report;
mod setup;
mod tools;

#[derive(Parser)]
#[clap(
    name = "jarvy",
    version = "0.2",
    author = "Zac Clifton",
    about = "Jarvy: a helper to configure and verify your computer",
    long_about = "Jarvy helps you set up and verify your computer based on a jarvy.toml configuration.\n\nUSAGE:\n    jarvy <COMMAND> [OPTIONS]\n\nEXAMPLES:\n    jarvy --help\n    jarvy configure\n    jarvy setup --file ./jarvy.toml\n    jarvy get --format json --output report.json\n\nRun without a subcommand to use the interactive menu."
)]
struct Cli {
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum OutputFormat {
    Json,
    Yaml,
    Toml,
    Pretty,
}

#[derive(Subcommand)]
enum Commands {
    /// Set up the environment based on the configuration file
    Setup {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Skip all hook execution
        #[clap(long)]
        no_hooks: bool,
        /// Show what would happen without executing (dry run mode)
        #[clap(long)]
        dry_run: bool,
    },
    /// Perform a minimal machine bootstrap (base requirements only, no dev tooling)
    Bootstrap {},
    /// Generate a default jarvy.toml configuration in the current directory
    Configure {},
    /// Display configured tools vs what is actually installed
    Get {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Output format: json, yaml, toml, pretty
        #[clap(short = 'F', long = "format", value_enum, default_value = "pretty")]
        output_format: OutputFormat,
        /// Optional file to write output to; prints to stdout if omitted
        #[clap(short, long)]
        output: Option<String>,
    },
    /// List all supported tools or output the tool index
    Tools {
        /// Output the full tool index as JSON
        #[clap(long)]
        index: bool,
        /// List tools with built-in default hooks
        #[clap(long)]
        default_hooks: bool,
        /// Output format: json, yaml, toml, pretty (for --index)
        #[clap(short = 'F', long = "format", value_enum, default_value = "pretty")]
        output_format: OutputFormat,
        /// Optional file to write output to; prints to stdout if omitted
        #[clap(short, long)]
        output: Option<String>,
    },
    /// Manage environment variables from jarvy.toml
    Env {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Generate .env file only
        #[clap(long)]
        dotenv: bool,
        /// Update shell rc file only
        #[clap(long)]
        shell: bool,
        /// Show what would happen without making changes
        #[clap(long)]
        dry_run: bool,
        /// Output for shell eval (export statements)
        #[clap(long)]
        export: bool,
        /// Shell type to use (bash, zsh, fish). Auto-detected if not specified.
        #[clap(long)]
        shell_type: Option<String>,
        /// Force overwrite of existing .env file (even if not created by Jarvy)
        #[clap(long)]
        force: bool,
    },
    /// Catch-all for unknown subcommands and their args
    #[clap(external_subcommand)]
    External(Vec<String>),
}

#[derive(Serialize)]
struct Reports {
    tools: Vec<ToolReport>,
}

fn color_for_status(status: &Status) -> &'static str {
    match status {
        Status::Match => "\x1b[32m",        // green
        Status::Mismatch => "\x1b[33m",     // yellow
        Status::NotInstalled => "\x1b[31m", // red
    }
}

fn pretty_output(reports: &[ToolReport]) -> String {
    let mut s = String::new();
    s.push_str("Tools status\n");
    for r in reports {
        let color = color_for_status(&r.status);
        let reset = "\x1b[0m";
        let status_label = match r.status {
            Status::Match => "match",
            Status::Mismatch => "mismatch",
            Status::NotInstalled => "not_installed",
        };
        let installed = r.installed.as_deref().unwrap_or("-");
        s.push_str(&format!(
            "{}{}{}: expected={}, installed={} [{}]\n",
            color, r.name, reset, r.expected, installed, status_label
        ));
    }
    s
}

fn main() {
    // Run the CLI Parser first so that -h/--help and -V/--version can exit without side effects
    let cli = Cli::parse();

    // If a user typed an unknown subcommand, handle it here (before any initialization)
    if let Some(Commands::External(args)) = &cli.command {
        if let Some(first) = args.first() {
            eprintln!("Unrecognized command: '{}'", first);
            eprintln!("Tip: run 'jarvy --help' to see available commands.");
        } else {
            eprintln!("Unrecognized command");
        }
        // Fall back to an interactive menu
        user_select();
        return;
    }

    // Initialize after parsing arguments
    let global_config = initialize();

    init_logging(global_config.settings.telemetry);

    // Initialize PostHog client (no-op if disabled or no API key)
    let fingerprint = global_config
        .settings
        .fingerprint
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    posthog::init(global_config.settings.telemetry, fingerprint.clone());

    // Send a cli_start event and set global analytics context
    {
        let cmd_name = match &cli.command {
            Some(Commands::Setup { .. }) => "setup",
            Some(Commands::Bootstrap { .. }) => "bootstrap",
            Some(Commands::Configure { .. }) => "configure",
            Some(Commands::Get { .. }) => "get",
            Some(Commands::Tools { .. }) => "tools",
            Some(Commands::Env { .. }) => "env",
            Some(Commands::External(..)) => "external",
            None => "interactive",
        };
        // Set global context for subsequent analytics/error events
        let mut ctx = serde_json::Map::new();
        ctx.insert(
            "command".to_string(),
            serde_json::Value::String(cmd_name.to_string()),
        );
        ctx.insert(
            "telemetry_enabled".to_string(),
            serde_json::Value::Bool(global_config.settings.telemetry),
        );
        let args_json = std::env::args()
            .skip(1)
            .map(serde_json::Value::String)
            .collect::<Vec<_>>();
        ctx.insert("args".to_string(), serde_json::Value::Array(args_json));
        posthog::set_context_map(ctx);

        // Emit cli_start
        let mut props = serde_json::Map::new();
        props.insert(
            "command".to_string(),
            serde_json::Value::String(cmd_name.to_string()),
        );
        posthog::capture("cli_start", props);
    }

    // Install panic hook to report CLI errors to PostHog and stderr
    {
        std::panic::set_hook(Box::new(|info| {
            // Print to stderr
            eprintln!("Jarvy panic: {}", info);

            // Send to PostHog using $exception format
            let bt = std::backtrace::Backtrace::capture();
            let stack_str = format!("{}", bt);
            let mut ctx = serde_json::Map::new();
            ctx.insert(
                "kind".to_string(),
                serde_json::Value::String("panic".to_string()),
            );
            crate::posthog::capture_exception(&format!("{}", info), "panic", Some(stack_str), ctx);
        }));
    }

    // Test-only telemetry smoke: if set, emit only logging events and then flush.
    if std::env::var("JARVY_TELEMETRY_SMOKE").as_deref() == Ok("1") {
        tracing::info!("telemetry smoke info");
        tracing::error!("telemetry smoke error");

        // Nudge the test collector: make a direct best-effort POST to /v1/logs.
        crate::analytics::send_otlp_smoke_probe();

        // Give exporters a brief moment to ship data.
        std::thread::sleep(std::time::Duration::from_millis(800));
    }

    // Register built-in tools so registry lookups are meaningful
    crate::tools::register_all();

    match &cli.command {
        Some(Commands::Setup { file, no_hooks, dry_run }) => {
            let config = Config::new(file);
            let hooks_config = config.get_hooks();
            let hook_settings = HookConfig::from(&hooks_config.config);

            // Set the global default for sudo usage based on config
            crate::tools::set_default_use_sudo(config.use_sudo());

            // Execute pre_setup hook if configured
            if !no_hooks {
                if let Some(ref script) = hooks_config.pre_setup {
                    let hook = Hook::with_config(script, "pre_setup", hook_settings.clone())
                        .with_env(HookEnv::global());
                    if *dry_run {
                        hook.dry_run();
                    } else {
                        match hook.execute() {
                            Ok(_) => {}
                            Err(e) => {
                                if !hook_settings.continue_on_error {
                                    eprintln!("Pre-setup hook failed: {}", e);
                                    std::process::exit(crate::error_codes::HOOK_FAILED);
                                }
                                eprintln!("Warning: Pre-setup hook failed: {}", e);
                            }
                        }
                    }
                }
            }

            if !*dry_run {
                setup();
            } else {
                println!("[DRY-RUN] Would run platform setup");
            }

            let tools = config.get_tool_configs();

            for (id, tool) in tools {
                // If the tool is not in the registry, log and guide the user
                if tools::get_tool(&tool.name).is_none() {
                    let msg = format!(
                        "We do not currently have support for {} package but we have logged it and will be adding it soon.",
                        tool.name
                    );
                    if posthog::telemetry_enabled() {
                        let mut props = serde_json::Map::new();
                        props.insert(
                            "tool".to_string(),
                            serde_json::Value::String(tool.name.clone()),
                        );
                        props.insert(
                            "version_hint".to_string(),
                            serde_json::Value::String(tool.version.clone()),
                        );
                        props.insert(
                            "source".to_string(),
                            serde_json::Value::String("config".to_string()),
                        );
                        posthog::capture_error("unknown_tool_in_config", &msg, props);
                        eprintln!("{}", msg);
                    } else {
                        eprintln!("{}", msg);
                        eprintln!(
                            "Telemetry is disabled. Please consider creating a feature request here: https://github.com/bearbinary/Jarvy/issues/new"
                        );
                    }
                    continue;
                }

                if *dry_run {
                    println!(
                        "[DRY-RUN] Would install {}: {} version {} using package manager: {}",
                        id, tool.name, tool.version, tool.version_manager
                    );
                } else {
                    println!(
                        "Installing {}: {} version {} using package manager: {}",
                        id, tool.name, tool.version, tool.version_manager
                    );

                    match tools::add(&tool.name, &tool.version) {
                        Ok(()) => {
                            println!("Successfully installed {} ({})", tool.name, tool.version);

                            // Execute per-tool post_install hook if configured
                            if !no_hooks {
                                let user_hook = config
                                    .get_tool_hooks(&tool.name)
                                    .and_then(|h| h.post_install.as_ref());

                                if let Some(script) = user_hook {
                                    // User-provided hook takes precedence
                                    let env = HookEnv::for_tool(&tool.name, &tool.version);
                                    let hook = Hook::with_config(
                                        script,
                                        &format!("{} post_install", tool.name),
                                        hook_settings.clone(),
                                    )
                                    .with_env(env);
                                    match hook.execute() {
                                        Ok(_) => {}
                                        Err(e) => {
                                            if !hook_settings.continue_on_error {
                                                eprintln!(
                                                    "Post-install hook for {} failed: {}",
                                                    tool.name, e
                                                );
                                                std::process::exit(crate::error_codes::HOOK_FAILED);
                                            }
                                            eprintln!(
                                                "Warning: Post-install hook for {} failed: {}",
                                                tool.name, e
                                            );
                                        }
                                    }
                                } else if let Some(default_hook) =
                                    tools::spec::get_tool_default_hook(&tool.name)
                                {
                                    // Fall back to tool's built-in default hook
                                    println!(
                                        "Running default hook for {}: {}",
                                        tool.name, default_hook.description
                                    );
                                    let env = HookEnv::for_tool(&tool.name, &tool.version);
                                    let hook = Hook::with_config(
                                        default_hook.script,
                                        &format!("{} default_hook", tool.name),
                                        hook_settings.clone(),
                                    )
                                    .with_env(env);
                                    match hook.execute() {
                                        Ok(_) => {}
                                        Err(e) => {
                                            // Default hooks are advisory; always continue on error
                                            eprintln!(
                                                "Warning: Default hook for {} failed: {}",
                                                tool.name, e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let msg = format!(
                                "Failed to install {} ({}): {:?}",
                                tool.name, tool.version, e
                            );
                            eprintln!("{}", msg);
                            if posthog::telemetry_enabled() {
                                let mut props = serde_json::Map::new();
                                props.insert(
                                    "tool".to_string(),
                                    serde_json::Value::String(tool.name.clone()),
                                );
                                props.insert(
                                    "version_hint".to_string(),
                                    serde_json::Value::String(tool.version.clone()),
                                );
                                props.insert(
                                    "error".to_string(),
                                    serde_json::Value::String(format!("{:?}", e)),
                                );
                                posthog::capture_error("tool_install_failed", &msg, props);
                            }
                        }
                    }
                }

                // Show dry-run for per-tool hooks (user-provided or default)
                if *dry_run && !no_hooks {
                    let user_hook = config
                        .get_tool_hooks(&tool.name)
                        .and_then(|h| h.post_install.as_ref());

                    if let Some(script) = user_hook {
                        let env = HookEnv::for_tool(&tool.name, &tool.version);
                        let hook = Hook::with_config(
                            script,
                            &format!("{} post_install", tool.name),
                            hook_settings.clone(),
                        )
                        .with_env(env);
                        hook.dry_run();
                    } else if let Some(default_hook) =
                        tools::spec::get_tool_default_hook(&tool.name)
                    {
                        // Show default hook in dry-run
                        println!(
                            "[DRY-RUN] Would run default hook for {}: {}",
                            tool.name, default_hook.description
                        );
                        let env = HookEnv::for_tool(&tool.name, &tool.version);
                        let hook = Hook::with_config(
                            default_hook.script,
                            &format!("{} default_hook", tool.name),
                            hook_settings.clone(),
                        )
                        .with_env(env);
                        hook.dry_run();
                    }
                }
            }

            // Environment variable setup
            let env_config = config.get_env();
            let env_settings = &env_config.config;

            if !env_config.vars.is_empty() || !env_config.secrets.is_empty() {
                // Build environment context
                let ctx = EnvContext::new();

                // Collect secrets if any (skip in CI mode or if dry run)
                let secrets_config = SecretsConfig {
                    ci_mode: std::env::var("CI").is_ok()
                        || std::env::var("JARVY_CI").is_ok()
                        || std::env::var("JARVY_TEST_MODE").is_ok()
                        || *dry_run,
                    fail_on_missing: false,
                };

                let secrets = if !*dry_run && !env_config.secrets.is_empty() {
                    match collect_secrets(&env_config.secrets, &ctx, &secrets_config) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("Warning: Could not collect secrets: {}", e);
                            HashMap::new()
                        }
                    }
                } else {
                    HashMap::new()
                };

                // Merge vars and secrets
                let mut all_vars: HashMap<String, String> = env_config.vars.iter()
                    .map(|(k, v)| (k.clone(), v.value().to_string()))
                    .collect();
                all_vars.extend(secrets);

                // Generate .env file if configured
                if env_settings.generate_dotenv {
                    let dotenv_path = std::path::Path::new(".env");
                    let dotenv_config = DotenvConfig {
                        backup: true,
                        force: false,
                        add_to_gitignore: env_settings.add_to_gitignore,
                    };

                    if *dry_run {
                        println!("\n=== Environment Setup (dry-run) ===");
                        println!("[DRY-RUN] Would generate .env file at {}", dotenv_path.display());
                        let preview = preview_dotenv(&all_vars, &ctx);
                        println!("{}", preview);
                    } else {
                        match generate_dotenv(dotenv_path, &all_vars, &ctx, &dotenv_config) {
                            Ok(_) => println!("\nGenerated .env file at {}", dotenv_path.display()),
                            Err(e) => eprintln!("Warning: Could not generate .env file: {}", e),
                        }
                    }
                }

                // Update shell rc file if configured
                if env_settings.update_rc {
                    let shell = detect_shell();
                    let shell_config = ShellConfig {
                        backup: true,
                        validate: false,
                    };

                    if *dry_run {
                        if !env_settings.generate_dotenv {
                            println!("\n=== Environment Setup (dry-run) ===");
                        }
                        println!("[DRY-RUN] Would update shell rc for {}", shell);
                        let preview = preview_shell_rc(shell, &all_vars, &ctx);
                        println!("{}", preview);
                    } else {
                        match update_shell_rc(shell, &all_vars, &ctx, &shell_config) {
                            Ok(path) => println!("Updated shell rc at {}", path.display()),
                            Err(e) => eprintln!("Warning: Could not update shell rc: {}", e),
                        }
                    }
                }
            }

            // Execute post_setup hook if configured
            if !no_hooks {
                if let Some(ref script) = hooks_config.post_setup {
                    let hook = Hook::with_config(script, "post_setup", hook_settings.clone())
                        .with_env(HookEnv::global());
                    if *dry_run {
                        hook.dry_run();
                    } else {
                        match hook.execute() {
                            Ok(_) => {}
                            Err(e) => {
                                if !hook_settings.continue_on_error {
                                    eprintln!("Post-setup hook failed: {}", e);
                                    std::process::exit(crate::error_codes::HOOK_FAILED);
                                }
                                eprintln!("Warning: Post-setup hook failed: {}", e);
                            }
                        }
                    }
                }
            }

            if config.has_hooks() && !no_hooks {
                println!("\nHooks execution summary:");
                if hooks_config.pre_setup.is_some() {
                    println!("  - pre_setup: executed");
                }
                let tool_hooks_count = hooks_config
                    .tool_hooks
                    .values()
                    .filter(|h| h.post_install.is_some())
                    .count();
                if tool_hooks_count > 0 {
                    println!("  - tool post_install hooks: {} executed", tool_hooks_count);
                }
                if hooks_config.post_setup.is_some() {
                    println!("  - post_setup: executed");
                }
            }
        }
        Some(Commands::Bootstrap {}) => {
            bootstrap::bootstrap();
        }
        Some(Commands::Configure {}) => create_default_config(),
        Some(Commands::Get {
            file,
            output_format,
            output,
        }) => {
            let config = Config::new(file);
            let reports = collect_reports(&config);

            let content = match output_format {
                OutputFormat::Json => {
                    let wrapper = Reports {
                        tools: reports.clone(),
                    };
                    serde_json::to_string_pretty(&wrapper)
                        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
                }
                OutputFormat::Yaml => {
                    let wrapper = Reports {
                        tools: reports.clone(),
                    };
                    serde_yaml::to_string(&wrapper).unwrap_or_else(|e| format!("error: {}", e))
                }
                OutputFormat::Toml => {
                    let wrapper = Reports {
                        tools: reports.clone(),
                    };
                    toml::to_string(&wrapper).unwrap_or_else(|e| format!("error = \"{}\"", e))
                }
                OutputFormat::Pretty => pretty_output(&reports),
            };

            if let Some(path) = output {
                if let Err(e) = fs::write(path, content) {
                    eprintln!("Failed to write output: {}", e);
                }
            } else {
                println!("{}", content);
            }
        }
        Some(Commands::Tools {
            index,
            default_hooks,
            output_format,
            output,
        }) => {
            let content = if *default_hooks {
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
                    OutputFormat::Yaml => serde_yaml::to_string(&hook_infos)
                        .unwrap_or_else(|e| format!("error: {}", e)),
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
            } else if *index {
                // Output the full tool index
                let tool_index = tools::spec::generate_tool_index();
                match output_format {
                    OutputFormat::Json => serde_json::to_string_pretty(&tool_index)
                        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e)),
                    OutputFormat::Yaml => serde_yaml::to_string(&tool_index)
                        .unwrap_or_else(|e| format!("error: {}", e)),
                    OutputFormat::Toml => toml::to_string(&tool_index)
                        .unwrap_or_else(|e| format!("error = \"{}\"", e)),
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
        Some(Commands::Env {
            file,
            dotenv,
            shell,
            dry_run,
            export,
            shell_type,
            force,
        }) => {
            let config = Config::new(file);
            let env_config = config.get_env();

            // Determine shell type
            let target_shell = if let Some(shell_str) = shell_type {
                match parse_shell(shell_str) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(crate::error_codes::CONFIG_ERROR);
                    }
                }
            } else if let Some(ref shell_str) = env_config.config.shell {
                parse_shell(shell_str).unwrap_or_else(|_| detect_shell())
            } else {
                detect_shell()
            };

            // Create context for variable expansion
            let ctx = EnvContext::new();

            // Collect all regular vars
            let vars: HashMap<String, String> = env_config
                .vars
                .iter()
                .map(|(k, v)| (k.clone(), v.value().to_string()))
                .collect();

            // Handle --export flag (output for shell eval)
            if *export {
                let preview = preview_shell_rc(target_shell, &vars, &ctx);
                println!("{}", preview);
                return;
            }

            // Collect secrets (in CI mode, won't prompt)
            let secrets_config = SecretsConfig::default();
            let secrets = match collect_secrets(&env_config.secrets, &ctx, &secrets_config) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error collecting secrets: {}", e);
                    std::process::exit(crate::error_codes::CONFIG_ERROR);
                }
            };

            // Merge vars and secrets
            let secrets_count = secrets.len();
            let mut all_vars = vars.clone();
            all_vars.extend(secrets);

            // Determine what to do
            let do_dotenv = *dotenv || (!*shell && env_config.config.generate_dotenv);
            let do_shell = *shell || (!*dotenv && env_config.config.update_rc);

            if !config.has_env() {
                println!("No environment variables configured in {}", file);
                return;
            }

            // Generate .env file
            if do_dotenv {
                let dotenv_path = &env_config.config.dotenv_path;

                if *dry_run {
                    println!("=== .env file preview (would be written to {}) ===", dotenv_path.display());
                    let content = preview_dotenv(&all_vars, &ctx);
                    println!("{}", content);
                } else {
                    let dotenv_config = DotenvConfig {
                        backup: true,
                        force: *force,
                        add_to_gitignore: env_config.config.add_to_gitignore,
                    };

                    match generate_dotenv(dotenv_path, &all_vars, &ctx, &dotenv_config) {
                        Ok(()) => {
                            println!("Generated .env file at: {}", dotenv_path.display());
                        }
                        Err(e) => {
                            eprintln!("Failed to generate .env file: {}", e);
                            if !*force {
                                eprintln!("Tip: Use --force to overwrite existing non-Jarvy .env files");
                            }
                            std::process::exit(crate::error_codes::CONFIG_ERROR);
                        }
                    }
                }
            }

            // Update shell rc file
            if do_shell {
                if *dry_run {
                    println!("\n=== Shell rc preview ({}) ===", target_shell);
                    let preview = preview_shell_rc(target_shell, &vars, &ctx);
                    println!("{}", preview);
                } else {
                    let shell_config = ShellConfig {
                        backup: env_config.config.backup_rc,
                        validate: false,
                    };

                    match update_shell_rc(target_shell, &vars, &ctx, &shell_config) {
                        Ok(path) => {
                            println!("Updated shell rc file: {}", path.display());
                            println!("Tip: Run 'source {}' or restart your shell to apply changes", path.display());
                        }
                        Err(e) => {
                            eprintln!("Failed to update shell rc file: {}", e);
                            std::process::exit(crate::error_codes::CONFIG_ERROR);
                        }
                    }
                }
            }

            // Summary
            if !*dry_run {
                println!("\nEnvironment configuration applied:");
                println!("  - Variables: {}", vars.len());
                if secrets_count > 0 {
                    println!("  - Secrets: {}", secrets_count);
                }
            }
        }
        None => {
            user_select();
        }
        Some(Commands::External(_)) => unreachable!("External subcommand handled before init"),
    }
}

fn user_select() {
    // Test mode: avoid interactive prompts and side-effects
    if std::env::var("JARVY_TEST_MODE").as_deref() == Ok("1") {
        println!("TEST: user_select invoked");
        return;
    }

    print_logo();

    println!("\t\tHi, I'm Jarvy! I'm here to help you get your development environment set up.");

    let options = vec![
        "Run the project",
        "Test the project",
        "Development environment setup",
    ];

    let selection: Result<&str, InquireError> =
        Select::new("What would you like to do today?", options).prompt();

    match selection {
        Ok(choice) => {
            println!("selection: {}", choice);
            match choice {
                "Run the project" => {
                    println!("R");
                    // TODO set the override command in settings
                    match std::process::Command::new("cargo").arg("run").output() {
                        Ok(output) => {
                            // Handle the output here
                            println!("Output: {}", String::from_utf8_lossy(&output.stdout));
                        }
                        Err(e) => println!("Failed to execute command: {}", e),
                    }
                }
                "Test the project" => {
                    println!("T");
                    // TODO set the override command in settings
                    match std::process::Command::new("cargo").arg("test").output() {
                        Ok(output) => {
                            // Handle the output here
                            println!("Output: {}", String::from_utf8_lossy(&output.stdout));
                        }
                        Err(e) => println!("Failed to execute command: {}", e),
                    }
                }
                "Development environment setup" => {
                    // TODO set the override command in settings
                    println!("D");
                    setup();
                }
                _ => {}
            }
        }
        Err(_) => {
            println!("No choice was made")
        }
    }
}

fn print_logo() {
    println!(
        "
 .----------------.
|   J A R V Y  ⚡   |
 '----------------'
    "
    );
}
