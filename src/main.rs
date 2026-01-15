use crate::analytics::init_logging;
use crate::config::{Config, EnvValue, create_default_config};
use crate::env::{
    DotenvConfig, EnvContext, SecretsConfig, ShellConfig, ShellType, collect_secrets, detect_shell,
    expand_value, generate_dotenv, parse_shell, preview_dotenv, preview_shell_rc, update_shell_rc,
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
mod ci;
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
mod services;
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
        /// Force CI mode (non-interactive, auto-answer prompts)
        #[clap(long, conflicts_with = "no_ci")]
        ci: bool,
        /// Force interactive mode even in CI environments
        #[clap(long, conflicts_with = "ci")]
        no_ci: bool,
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
    /// Generate CI configuration files for various providers
    CiConfig {
        /// CI provider to generate config for (github, gitlab, circleci, azure, bitbucket)
        #[clap(value_parser = parse_ci_provider)]
        provider: ci::CiProvider,
        /// Output directory (defaults to current directory)
        #[clap(short, long, default_value = ".")]
        output: String,
        /// Show the config without writing to file
        #[clap(long)]
        dry_run: bool,
    },
    /// Show detected CI environment information
    CiInfo {},
    /// Manage project services (docker-compose, tilt)
    Services {
        #[clap(subcommand)]
        action: ServicesAction,
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
    },
    /// Catch-all for unknown subcommands and their args
    #[clap(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand)]
enum ServicesAction {
    /// Start project services
    Start {
        /// Run services in the foreground (attached)
        #[clap(long)]
        foreground: bool,
    },
    /// Stop project services
    Stop {},
    /// Show service status
    Status {},
    /// Restart project services
    Restart {
        /// Run services in the foreground (attached)
        #[clap(long)]
        foreground: bool,
    },
}

fn parse_ci_provider(s: &str) -> Result<ci::CiProvider, String> {
    match s.to_lowercase().as_str() {
        "github" | "github-actions" | "gha" => Ok(ci::CiProvider::GitHubActions),
        "gitlab" | "gitlab-ci" => Ok(ci::CiProvider::GitLabCi),
        "circleci" | "circle" => Ok(ci::CiProvider::CircleCi),
        "azure" | "azure-devops" | "ado" => Ok(ci::CiProvider::AzureDevOps),
        "bitbucket" | "bitbucket-pipelines" => Ok(ci::CiProvider::Bitbucket),
        "travis" | "travis-ci" => Ok(ci::CiProvider::TravisCi),
        "jenkins" => Ok(ci::CiProvider::Jenkins),
        "buildkite" => Ok(ci::CiProvider::Buildkite),
        "teamcity" => Ok(ci::CiProvider::TeamCity),
        "appveyor" => Ok(ci::CiProvider::AppVeyor),
        _ => Err(format!(
            "Unknown CI provider '{}'. Supported: github, gitlab, circleci, azure, bitbucket",
            s
        )),
    }
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
            Some(Commands::CiConfig { .. }) => "ci-config",
            Some(Commands::CiInfo { .. }) => "ci-info",
            Some(Commands::Services { .. }) => "services",
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
        Some(Commands::Setup {
            file,
            no_hooks,
            dry_run,
            ci,
            no_ci,
        }) => {
            // Handle CI mode detection with CLI overrides
            // SAFETY: We're setting env vars at startup before any threads are spawned
            let ci_env = if *ci {
                // Force CI mode
                unsafe { std::env::set_var("JARVY_CI", "1") };
                crate::ci::detect()
            } else if *no_ci {
                // Force non-CI mode
                unsafe { std::env::set_var("JARVY_NO_CI", "1") };
                None
            } else {
                crate::ci::detect()
            };

            // Log CI detection
            if let Some(ref env) = ci_env {
                let output = env.output();
                output.notice(&format!("Running in CI mode: {}", env.provider));
                if let Some(ref build_id) = env.build_id {
                    output.debug(&format!("Build ID: {}", build_id));
                }
            }

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

            // Phase 2: Parallel version checking - determine which tools need installation
            println!("Checking tool versions...");
            let version_check = tools::spec::check_tools_parallel(
                tools.iter().map(|(_, t)| (t.name.as_str(), t.version.as_str())),
            );

            // Report version check results
            println!("{}", version_check.summary_string());

            // Log already-satisfied tools (verbose mode)
            if !version_check.satisfied.is_empty() {
                println!(
                    "Already installed: {}",
                    version_check
                        .satisfied
                        .iter()
                        .map(|(n, _)| n.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            // Log unknown tools
            for (name, version) in &version_check.unknown {
                let msg = format!(
                    "We do not currently have support for {} package but we have logged it and will be adding it soon.",
                    name
                );
                if posthog::telemetry_enabled() {
                    let mut props = serde_json::Map::new();
                    props.insert(
                        "tool".to_string(),
                        serde_json::Value::String(name.clone()),
                    );
                    props.insert(
                        "version_hint".to_string(),
                        serde_json::Value::String(version.clone()),
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
            }

            // Create list of known tools for hook execution (needed later)
            let known_tools: Vec<_> = tools
                .iter()
                .filter(|(_, t)| tools::get_tool(&t.name).is_some())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            // Only install tools that actually need installation (from parallel check)
            // Group tools by package manager for batch installation
            let tool_groups = tools::spec::group_tools_for_installation(
                version_check
                    .needs_install
                    .iter()
                    .map(|(n, v)| (n.as_str(), v.as_str())),
            );

            // Track successfully installed tools for hook execution
            let mut successfully_installed: Vec<(String, String)> = Vec::new();

            if *dry_run {
                // Dry-run: show what would be installed
                for (pm, packages) in &tool_groups.by_package_manager {
                    let package_names: Vec<&str> =
                        packages.iter().map(|(_, pkg, _)| pkg.as_str()).collect();
                    println!(
                        "[DRY-RUN] Would batch install via {:?}: {}",
                        pm,
                        package_names.join(", ")
                    );
                }
                for (name, version) in &tool_groups.custom_install {
                    println!(
                        "[DRY-RUN] Would install {} version {} using custom installer",
                        name, version
                    );
                }
            } else {
                // Batch install by package manager
                for (pm, packages) in &tool_groups.by_package_manager {
                    if packages.is_empty() {
                        continue;
                    }

                    let package_names: Vec<&str> =
                        packages.iter().map(|(_, pkg, _)| pkg.as_str()).collect();
                    println!(
                        "Batch installing {} packages via {:?}: {}",
                        packages.len(),
                        pm,
                        package_names.join(", ")
                    );

                    match tools::common::PkgOps::batch_install(*pm, &package_names, None) {
                        Ok(result) => {
                            // Track successful installs
                            for pkg_name in &result.succeeded {
                                // Find the tool name for this package
                                if let Some((tool_name, _, version)) =
                                    packages.iter().find(|(_, pkg, _)| pkg == pkg_name)
                                {
                                    println!("Successfully installed {} ({})", tool_name, version);
                                    successfully_installed
                                        .push((tool_name.clone(), version.clone()));
                                }
                            }
                            // Log failures
                            for (pkg_name, error) in &result.failed {
                                if let Some((tool_name, _, version)) =
                                    packages.iter().find(|(_, pkg, _)| pkg == pkg_name)
                                {
                                    let msg = format!(
                                        "Failed to install {} ({}): {}",
                                        tool_name, version, error
                                    );
                                    eprintln!("{}", msg);
                                    if posthog::telemetry_enabled() {
                                        let mut props = serde_json::Map::new();
                                        props.insert(
                                            "tool".to_string(),
                                            serde_json::Value::String(tool_name.clone()),
                                        );
                                        props.insert(
                                            "version_hint".to_string(),
                                            serde_json::Value::String(version.clone()),
                                        );
                                        props.insert(
                                            "error".to_string(),
                                            serde_json::Value::String(error.clone()),
                                        );
                                        posthog::capture_error("tool_install_failed", &msg, props);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            // Batch install failed entirely - log all as failed
                            for (tool_name, _, version) in packages {
                                let msg = format!(
                                    "Failed to install {} ({}): {:?}",
                                    tool_name, version, e
                                );
                                eprintln!("{}", msg);
                                if posthog::telemetry_enabled() {
                                    let mut props = serde_json::Map::new();
                                    props.insert(
                                        "tool".to_string(),
                                        serde_json::Value::String(tool_name.clone()),
                                    );
                                    props.insert(
                                        "version_hint".to_string(),
                                        serde_json::Value::String(version.clone()),
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
                }

                // Install custom tools individually (these require special handling)
                for (name, version) in &tool_groups.custom_install {
                    println!(
                        "Installing {} version {} using custom installer",
                        name, version
                    );

                    match tools::add(name, version) {
                        Ok(()) => {
                            println!("Successfully installed {} ({})", name, version);
                            successfully_installed.push((name.clone(), version.clone()));
                        }
                        Err(e) => {
                            let msg = format!("Failed to install {} ({}): {:?}", name, version, e);
                            eprintln!("{}", msg);
                            if posthog::telemetry_enabled() {
                                let mut props = serde_json::Map::new();
                                props.insert(
                                    "tool".to_string(),
                                    serde_json::Value::String(name.clone()),
                                );
                                props.insert(
                                    "version_hint".to_string(),
                                    serde_json::Value::String(version.clone()),
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

                // Execute hooks for successfully installed tools
                if !no_hooks {
                    for (tool_name, version) in &successfully_installed {
                        let user_hook = config
                            .get_tool_hooks(tool_name)
                            .and_then(|h| h.post_install.as_ref());

                        if let Some(script) = user_hook {
                            // User-provided hook takes precedence
                            let env = HookEnv::for_tool(tool_name, version);
                            let hook = Hook::with_config(
                                script,
                                &format!("{} post_install", tool_name),
                                hook_settings.clone(),
                            )
                            .with_env(env);
                            match hook.execute() {
                                Ok(_) => {}
                                Err(e) => {
                                    if !hook_settings.continue_on_error {
                                        eprintln!(
                                            "Post-install hook for {} failed: {}",
                                            tool_name, e
                                        );
                                        std::process::exit(crate::error_codes::HOOK_FAILED);
                                    }
                                    eprintln!(
                                        "Warning: Post-install hook for {} failed: {}",
                                        tool_name, e
                                    );
                                }
                            }
                        } else if let Some(default_hook) =
                            tools::spec::get_tool_default_hook(tool_name)
                        {
                            // Fall back to tool's built-in default hook
                            println!(
                                "Running default hook for {}: {}",
                                tool_name, default_hook.description
                            );
                            let env = HookEnv::for_tool(tool_name, version);
                            let hook = Hook::with_config(
                                default_hook.script,
                                &format!("{} default_hook", tool_name),
                                hook_settings.clone(),
                            )
                            .with_env(env);
                            match hook.execute() {
                                Ok(_) => {}
                                Err(e) => {
                                    // Default hooks are advisory; always continue on error
                                    eprintln!(
                                        "Warning: Default hook for {} failed: {}",
                                        tool_name, e
                                    );
                                }
                            }
                        }
                    }
                }
            }

            // Show dry-run for per-tool hooks
            if *dry_run && !no_hooks {
                for (_, tool) in &known_tools {
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
                let mut all_vars: HashMap<String, String> = env_config
                    .vars
                    .iter()
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
                        println!(
                            "[DRY-RUN] Would generate .env file at {}",
                            dotenv_path.display()
                        );
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

            // Auto-start services if configured
            let services_config = &config.services;
            let is_ci = ci_env.is_some();
            if services_config.should_auto_start(is_ci) {
                let working_dir = std::path::Path::new(file)
                    .parent()
                    .unwrap_or(std::path::Path::new("."));

                // Detect service backend
                let backend_result = services::detect_backend_with_config(
                    working_dir,
                    services_config.compose_file.as_deref(),
                    services_config.tilt_file.as_deref(),
                );

                if let Some((backend, config_path)) = backend_result {
                    let backend_impl = services::get_backend(backend);

                    if backend_impl.is_installed() {
                        if *dry_run {
                            println!("\n[DRY-RUN] Would auto-start {} services", backend);
                        } else {
                            println!("\nAuto-starting {} services...", backend);
                            match backend_impl.start(&config_path, true) {
                                Ok(result) => {
                                    println!("{}", result.message);
                                }
                                Err(e) => {
                                    // Services auto-start is advisory - don't fail the setup
                                    eprintln!("Warning: Failed to auto-start services: {}", e);
                                }
                            }
                        }
                    } else {
                        eprintln!(
                            "Note: {} config found but {} is not installed. Skipping services auto-start.",
                            backend, backend
                        );
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
                    println!(
                        "=== .env file preview (would be written to {}) ===",
                        dotenv_path.display()
                    );
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
                                eprintln!(
                                    "Tip: Use --force to overwrite existing non-Jarvy .env files"
                                );
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
                            println!(
                                "Tip: Run 'source {}' or restart your shell to apply changes",
                                path.display()
                            );
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
        Some(Commands::CiConfig {
            provider,
            output,
            dry_run,
        }) => {
            let template = match ci::CiConfigTemplate::for_provider(*provider) {
                Some(t) => t,
                None => {
                    eprintln!(
                        "Error: CI config generation is not supported for {}",
                        provider
                    );
                    eprintln!("Supported providers: github, gitlab, circleci, azure, bitbucket");
                    std::process::exit(crate::error_codes::CONFIG_ERROR);
                }
            };

            if *dry_run {
                println!("=== {} ===", template.file_path);
                println!("{}", template.content);
            } else {
                let base_path = std::path::Path::new(output);
                match template.write(base_path) {
                    Ok(path) => {
                        println!("Generated CI config: {}", path.display());
                        println!("Provider: {}", template.provider);
                        println!("Description: {}", template.description);
                    }
                    Err(e) => {
                        eprintln!("Error generating CI config: {}", e);
                        std::process::exit(crate::error_codes::CONFIG_ERROR);
                    }
                }
            }
        }
        Some(Commands::CiInfo {}) => match ci::detect() {
            Some(env) => {
                println!("CI Environment Detected");
                println!("=======================");
                println!("Provider: {}", env.provider);
                println!("Forced: {}", env.forced);
                println!();
                println!("Features:");
                println!("  - Log groups: {}", env.provider.supports_groups());
                println!("  - Output vars: {}", env.provider.supports_output_vars());
                println!("  - Caching: {}", env.provider.supports_cache());
                if let Some(cache_dir) = env.provider.cache_dir() {
                    println!("  - Cache dir: {}", cache_dir);
                }
                println!();
                println!("Build Information:");
                if let Some(ref id) = env.build_id {
                    println!("  - Build ID: {}", id);
                }
                if let Some(ref repo) = env.repository {
                    println!("  - Repository: {}", repo);
                }
                if let Some(ref branch) = env.branch {
                    println!("  - Branch: {}", branch);
                }
                if let Some(ref sha) = env.commit_sha {
                    println!("  - Commit: {}", sha);
                }
            }
            None => {
                println!("Not running in a CI environment.");
                println!();
                println!("Supported CI providers:");
                println!("  - GitHub Actions (GITHUB_ACTIONS=true)");
                println!("  - GitLab CI (GITLAB_CI=true)");
                println!("  - CircleCI (CIRCLECI=true)");
                println!("  - Travis CI (TRAVIS=true)");
                println!("  - Azure DevOps (TF_BUILD=True)");
                println!("  - Jenkins (JENKINS_URL set)");
                println!("  - Bitbucket (BITBUCKET_BUILD_NUMBER set)");
                println!("  - Buildkite (BUILDKITE=true)");
                println!("  - TeamCity (TEAMCITY_VERSION set)");
                println!("  - AppVeyor (APPVEYOR=True)");
                println!("  - Generic (CI=true)");
                println!();
                println!("Use --ci flag to force CI mode, or set JARVY_CI=1");
            }
        },
        Some(Commands::Services { action, file }) => {
            let config = Config::new(file);
            let services_config = config.services.clone();

            // Check if services are enabled
            if !services_config.enabled {
                eprintln!("Services are not enabled in the configuration.");
                eprintln!("Add [services] enabled = true to your jarvy.toml");
                return;
            }

            // Detect CI environment (available for future auto-start integration)
            let _is_ci = ci::detect().is_some();

            // Get the working directory
            let working_dir = std::path::Path::new(file)
                .parent()
                .unwrap_or(std::path::Path::new("."));

            // Detect service backend (or use config overrides)
            let backend_result = services::detect_backend_with_config(
                working_dir,
                services_config.compose_file.as_deref(),
                services_config.tilt_file.as_deref(),
            );

            let (backend, config_path) = match backend_result {
                Some((b, p)) => (b, p),
                None => {
                    eprintln!("No service configuration found.");
                    eprintln!("Supported: docker-compose.yml, compose.yml, Tiltfile");
                    return;
                }
            };

            let backend_impl = services::get_backend(backend);

            // Check if backend is installed
            if !backend_impl.is_installed() {
                eprintln!("{} is not installed.", backend);
                eprintln!("Install it with: jarvy setup");
                return;
            }

            match action {
                ServicesAction::Start { foreground } => {
                    println!("Starting {} services...", backend);
                    let detach = !foreground;
                    match backend_impl.start(&config_path, detach) {
                        Ok(result) => {
                            println!("{}", result.message);
                        }
                        Err(e) => {
                            eprintln!("Failed to start services: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                ServicesAction::Stop {} => {
                    println!("Stopping {} services...", backend);
                    match backend_impl.stop(&config_path) {
                        Ok(result) => {
                            println!("{}", result.message);
                        }
                        Err(e) => {
                            eprintln!("Failed to stop services: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                ServicesAction::Status {} => match backend_impl.status(&config_path) {
                    Ok(status) => {
                        println!("Service Backend: {}", status.backend);
                        println!("Installed: {}", if status.installed { "Yes" } else { "No" });
                        println!("Running: {}", if status.running { "Yes" } else { "No" });
                        if !status.details.is_empty() {
                            println!("\nDetails:\n{}", status.details);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to get service status: {}", e);
                        std::process::exit(1);
                    }
                },
                ServicesAction::Restart { foreground } => {
                    println!("Restarting {} services...", backend);
                    let detach = !foreground;
                    match backend_impl.restart(&config_path, detach) {
                        Ok(result) => {
                            println!("{}", result.message);
                        }
                        Err(e) => {
                            eprintln!("Failed to restart services: {}", e);
                            std::process::exit(1);
                        }
                    }
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
