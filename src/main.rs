use crate::analytics::init_logging;
use crate::config::{Config, EnvValue, create_default_config};
use crate::env::{
    DotenvConfig, EnvContext, SecretsConfig, ShellConfig, ShellType, collect_secrets, detect_shell,
    expand_value, generate_dotenv, parse_shell, preview_dotenv, preview_shell_rc, update_shell_rc,
};
use crate::hooks::{Hook, HookConfig, HookEnv};
use crate::init::initialize;
use crate::onboarding::{is_first_run, mark_initialized, show_welcome_banner, WelcomeBannerConfig};
use crate::report::{Status, ToolReport, collect_reports};
use crate::setup::setup;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use inquire::{InquireError, Select};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};

mod analytics;
mod bootstrap;
mod ci;
mod commands;
mod config;
mod env;
mod error_codes;
mod hooks;
mod init;
mod lock;
mod mcp;
mod network;
mod observability;
mod onboarding;
mod os_setup;
mod output;
mod outputs;
// PostHog removed in PRD-022 - now using unified telemetry module
mod provisioner;
mod report;
mod roles;
mod services;
mod setup;
mod team;
mod telemetry;
mod templates;
mod tools;
mod update;

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
        /// Fetch configuration from a URL (e.g., GitHub raw URL, gist, HTTP endpoint)
        #[clap(long, value_name = "URL")]
        from: Option<String>,
        /// Override role assignment for this run (temporary, doesn't modify config)
        #[clap(long, value_name = "ROLE")]
        role: Option<String>,
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
        /// Number of parallel jobs for user-space package installations (npm, pip, cargo, go, custom installers).
        /// Default: 4. Set to 1 for sequential installation.
        #[clap(short, long, default_value = "4")]
        jobs: usize,
        /// Force sequential installation (equivalent to --jobs 1). Useful for deterministic output.
        #[clap(long)]
        sequential: bool,
        /// Ignore missing dependency warnings (advanced use).
        /// Normally, jarvy warns when installing tools whose dependencies are missing.
        /// Use this flag to suppress those warnings (e.g., if dependencies are pre-installed elsewhere).
        #[clap(long)]
        ignore_missing_deps: bool,
        /// Skip SSL certificate verification for --from URL (not recommended)
        #[clap(long)]
        insecure: bool,
        /// Add custom HTTP header for authenticated config fetching (can be repeated)
        /// Example: --header "Authorization: token ghp_xxxx" --header "X-Custom: value"
        #[clap(long, value_name = "HEADER", action = clap::ArgAction::Append)]
        header: Vec<String>,
        // Observability flags
        /// Suppress all output except errors
        #[clap(long, short = 'q')]
        quiet: bool,
        /// Verbose output (use -v for warnings, -vv for debug, -vvv for trace)
        #[clap(long, short = 'v', action = clap::ArgAction::Count)]
        verbose: u8,
        /// Enable performance profiling
        #[clap(long)]
        profile: bool,
        /// Write profile results to file (JSON)
        #[clap(long, value_name = "FILE")]
        profile_output: Option<String>,
        /// Log output format: text (default), json
        #[clap(long, value_name = "FORMAT")]
        log_format: Option<String>,
        /// Write logs to file instead of stderr
        #[clap(long, value_name = "FILE")]
        log_file: Option<String>,
        /// Filter debug logs to specific modules (e.g., jarvy::tools::docker)
        #[clap(long, value_name = "MODULE")]
        debug_filter: Option<String>,
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
    /// Diagnose environment issues, check tool health, and verify PATH
    Doctor {
        /// Path to the configuration file (optional)
        #[clap(short, long)]
        file: Option<String>,
        /// Only check specific tools (comma-separated)
        #[clap(long)]
        tools: Option<String>,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
        /// Show extended health dashboard with system metrics
        #[clap(long)]
        extended: bool,
        /// Export diagnostic report as markdown
        #[clap(long)]
        report: Option<String>,
    },
    /// Preview changes before running setup (dry-run)
    Diff {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Only show changes (hide satisfied tools)
        #[clap(long)]
        changes_only: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Generate jarvy.toml from currently installed tools
    Export {
        /// Only include specific tools (comma-separated)
        #[clap(long)]
        tools: Option<String>,
        /// Include all detected tools
        #[clap(long)]
        all: bool,
        /// Show verbose output (include paths)
        #[clap(short, long)]
        verbose: bool,
        /// Output format: toml, json
        #[clap(short = 'F', long = "format", default_value = "toml")]
        output_format: String,
        /// Output file (stdout if not specified)
        #[clap(short, long)]
        output: Option<String>,
    },
    /// Upgrade tools to their latest versions
    Upgrade {
        /// Path to the configuration file (optional)
        #[clap(short, long)]
        file: Option<String>,
        /// Only upgrade specific tools (comma-separated or tool@version)
        #[clap(long)]
        tools: Option<String>,
        /// Show what would be upgraded without making changes
        #[clap(long)]
        dry_run: bool,
        /// Force upgrade even if already at required version
        #[clap(long)]
        force: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Create a new jarvy.toml configuration file interactively
    Init {
        /// Use a predefined template (react, vue, go-api, rust-cli, etc.)
        #[clap(short, long)]
        template: Option<String>,
        /// Run without interactive prompts (requires --template)
        #[clap(long)]
        non_interactive: bool,
        /// Output to stdout instead of file
        #[clap(long)]
        stdout: bool,
        /// Output file path (default: jarvy.toml)
        #[clap(short, long)]
        output: Option<String>,
    },
    /// Search available tools that Jarvy can install
    Search {
        /// Search query (tool name or partial match)
        query: Option<String>,
        /// Show all available tools
        #[clap(long)]
        all: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Validate a jarvy.toml configuration file
    Validate {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Fetch configuration from a URL and validate it (e.g., GitHub raw URL, gist)
        #[clap(long, value_name = "URL")]
        from: Option<String>,
        /// Treat warnings as errors
        #[clap(long)]
        strict: bool,
        /// Add custom HTTP header for authenticated config fetching (can be repeated)
        /// Example: --header "Authorization: token ghp_xxxx"
        #[clap(long, value_name = "HEADER", action = clap::ArgAction::Append)]
        header: Vec<String>,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, powershell, elvish)
        shell: String,
        /// Show installation instructions
        #[clap(long)]
        instructions: bool,
    },
    /// Browse and use pre-built configuration templates
    Templates {
        #[clap(subcommand)]
        action: TemplatesSubcommand,
    },
    /// Manage telemetry settings (OTEL endpoint, signals)
    Telemetry {
        #[clap(subcommand)]
        action: TelemetryAction,
    },
    /// Start the MCP (Model Context Protocol) server for LLM integration
    Mcp {
        /// Path to MCP configuration file (defaults to ~/.jarvy/mcp-config.toml)
        #[clap(short, long)]
        config: Option<std::path::PathBuf>,
    },
    /// Deep diagnosis for a specific tool - check installation, dependencies, and health
    Diagnose {
        /// Tool to diagnose (e.g., 'docker', 'node', 'git')
        tool: String,
        /// Attempt to automatically fix detected issues
        #[clap(long)]
        fix: bool,
        /// Export diagnostic bundle to a file
        #[clap(long)]
        export: bool,
        /// Scope for export: tools, network, all (comma-separated)
        #[clap(long, default_value = "all")]
        scope: String,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Manage team configuration sources for shared configs
    Team {
        #[clap(subcommand)]
        action: TeamAction,
    },
    /// Manage role-based configurations (list, show, diff)
    Roles {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        #[clap(subcommand)]
        action: roles::RolesAction,
    },
    /// Manage version lock files for reproducible environments
    Lock {
        #[clap(subcommand)]
        action: LockAction,
    },
    /// Manage configuration inheritance and remote configs
    Config {
        #[clap(subcommand)]
        action: ConfigAction,
    },
    /// Guided quickstart experience for new users
    Quickstart {
        /// Run without interactive prompts
        #[clap(long)]
        non_interactive: bool,
        /// Skip system check step
        #[clap(long)]
        skip_check: bool,
    },
    /// Check for and install Jarvy updates
    Update {
        #[clap(subcommand)]
        action: Option<UpdateSubcommand>,
        /// Install specific version
        #[clap(long)]
        version: Option<String>,
        /// Use specific release channel (stable, beta, nightly)
        #[clap(long)]
        channel: Option<String>,
        /// Override installation method (homebrew, cargo, apt, dnf, winget, chocolatey, scoop, binary)
        #[clap(long)]
        method: Option<String>,
        /// Rollback to previous version
        #[clap(long)]
        rollback: bool,
    },
    /// Catch-all for unknown subcommands and their args
    #[clap(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand)]
enum TemplatesSubcommand {
    /// List all available templates
    List {},
    /// Show details of a specific template
    Show {
        /// Template name to show
        name: String,
    },
    /// Use a template to create jarvy.toml
    Use {
        /// Template name to use
        name: String,
        /// Output file path (default: jarvy.toml)
        #[clap(short, long)]
        output: Option<String>,
        /// Run setup immediately after creating config
        #[clap(long)]
        setup: bool,
    },
}

#[derive(Subcommand)]
enum TelemetryAction {
    /// Show current telemetry configuration
    Status {},
    /// Enable telemetry
    Enable {},
    /// Disable telemetry
    Disable {},
    /// Set OTLP endpoint URL
    SetEndpoint {
        /// OTLP endpoint URL (e.g., http://localhost:4318)
        url: String,
    },
    /// Test telemetry connectivity
    Test {},
    /// Preview what telemetry would be sent
    Preview {},
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

#[derive(Subcommand)]
enum TeamAction {
    /// Add a team configuration source
    Add {
        /// Name for this source (e.g., 'company', 'team-frontend')
        name: String,
        /// Base URL for the config repository
        url: String,
        /// Description of this source
        #[clap(short, long)]
        description: Option<String>,
    },
    /// List registered team sources
    List {},
    /// Browse available configs from a source
    Browse {
        /// Source name to browse
        source: String,
    },
    /// Sync config index from a source
    Sync {
        /// Source name to sync (syncs all if not specified)
        source: Option<String>,
    },
    /// Remove a team source
    Remove {
        /// Source name to remove
        name: String,
    },
    /// Initialize project with a team config
    Init {
        /// Config to use (format: source/config-name)
        #[clap(long)]
        from: String,
        /// Output file path
        #[clap(short, long, default_value = "./jarvy.toml")]
        output: String,
    },
}

#[derive(Subcommand)]
enum LockAction {
    /// Generate a lock file from current environment
    Generate {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Output lock file path
        #[clap(short, long, default_value = "./jarvy.lock")]
        output: String,
    },
    /// Show lock file status (compare with installed versions)
    Status {
        /// Path to the lock file
        #[clap(short, long, default_value = "./jarvy.lock")]
        lock_file: String,
        /// Show detailed output
        #[clap(short, long)]
        verbose: bool,
    },
    /// Verify installed tools match lock file
    Verify {
        /// Path to the lock file
        #[clap(short, long, default_value = "./jarvy.lock")]
        lock_file: String,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show resolved configuration (with inheritance applied)
    Show {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Show resolved config (after inheritance)
        #[clap(long)]
        resolved: bool,
        /// Show the extends chain
        #[clap(long)]
        extends_chain: bool,
        /// Output format: toml, json, yaml
        #[clap(short = 'F', long = "format", default_value = "toml")]
        output_format: String,
    },
    /// Refresh cached remote configs
    Refresh {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Force refresh even if cache is valid
        #[clap(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum UpdateSubcommand {
    /// Check for available updates
    Check {
        /// Check specific channel instead of configured
        #[clap(long)]
        channel: Option<String>,
    },
    /// Show update history
    History {},
    /// Show update configuration
    Config {},
    /// Enable auto-updates
    Enable {},
    /// Disable auto-updates
    Disable {},
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

fn parse_update_channel(s: &str) -> Option<update::Channel> {
    match s.to_lowercase().as_str() {
        "stable" => Some(update::Channel::Stable),
        "beta" => Some(update::Channel::Beta),
        "nightly" => Some(update::Channel::Nightly),
        _ => {
            eprintln!("Unknown update channel '{}'. Using stable.", s);
            Some(update::Channel::Stable)
        }
    }
}

fn parse_install_method(s: &str) -> Option<update::InstallMethod> {
    match s.to_lowercase().as_str() {
        "homebrew" | "brew" => Some(update::InstallMethod::Homebrew),
        "cargo" => Some(update::InstallMethod::Cargo),
        "apt" | "apt-get" => Some(update::InstallMethod::Apt),
        "dnf" => Some(update::InstallMethod::Dnf),
        "pacman" => Some(update::InstallMethod::Pacman),
        "winget" => Some(update::InstallMethod::Winget),
        "chocolatey" | "choco" => Some(update::InstallMethod::Chocolatey),
        "scoop" => Some(update::InstallMethod::Scoop),
        "binary" | "direct" => Some(update::InstallMethod::Binary),
        _ => {
            eprintln!("Unknown install method '{}'. Auto-detecting.", s);
            None
        }
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

    // Machine fingerprint for telemetry (used if needed for user identification)
    let _fingerprint = global_config
        .settings
        .fingerprint
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    // Initialize unified telemetry (OTEL-based)
    // Merge config file settings with environment variable overrides
    let mut telemetry_config = global_config.telemetry.clone();
    // Legacy compatibility: if settings.telemetry is false, disable new telemetry too
    if !global_config.settings.telemetry {
        telemetry_config.enabled = false;
    }
    // Environment variables take precedence
    let env_config = telemetry::TelemetryConfig::from_env();
    if std::env::var("JARVY_TELEMETRY").is_ok() {
        telemetry_config.enabled = env_config.enabled;
    }
    if std::env::var("JARVY_OTLP_ENDPOINT").is_ok() {
        telemetry_config.endpoint = env_config.endpoint;
    }
    telemetry::init(telemetry_config);

    // Install panic hook to log errors
    {
        std::panic::set_hook(Box::new(|info| {
            eprintln!("Jarvy panic: {}", info);
            // Note: OTEL telemetry is flush-at-exit, panic info captured by tracing
            tracing::error!(event = "panic", message = %info);
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
            from,
            role,
            no_hooks,
            dry_run,
            ci,
            no_ci,
            jobs,
            sequential,
            ignore_missing_deps,
            insecure,
            header,
            ..  // Ignore observability fields (quiet, verbose, profile, etc.)
        }) => {
            // Determine effective parallelism level
            let parallel_jobs = if *sequential { 1 } else { *jobs.max(&1) };

            // Set env var for dependency warning suppression
            if *ignore_missing_deps {
                // SAFETY: Setting env var at startup before spawning threads
                unsafe { std::env::set_var("JARVY_IGNORE_MISSING_DEPS", "1") };
            }
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

            // Determine config file path: fetch from URL or use local file
            let config_path = if let Some(url) = from {
                match fetch_remote_config(url, *insecure, header) {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Error fetching remote config: {}", e);
                        std::process::exit(crate::error_codes::CONFIG_ERROR);
                    }
                }
            } else {
                file.clone()
            };

            let config = Config::new(&config_path);
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

            // Get tool configs with role override if --role flag was used
            if let Some(role_name) = role {
                println!("Using role override: {}", role_name);
            }
            let tools = config.get_tool_configs_with_role_override(role.as_deref());

            // Phase 2: Parallel version checking - determine which tools need installation
            println!("Checking tool versions...");
            let version_check = tools::spec::check_tools_parallel(
                tools
                    .iter()
                    .map(|(_, t)| (t.name.as_str(), t.version.as_str())),
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

            // Log unknown tools - critical for MCP feedback loop
            for (name, version) in &version_check.unknown {
                let msg = format!(
                    "We do not currently have support for {} package but we have logged it and will be adding it soon.",
                    name
                );
                eprintln!("{}", msg);
                // Emit telemetry for unknown tool (used by MCP feedback)
                telemetry::tool_not_supported(name, Some(version), telemetry::Source::Config);
                if !telemetry::is_enabled() {
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
            // First, order tools by dependencies to ensure version managers are installed first
            let ordered_tools = tools::spec::order_tools_by_dependencies(
                version_check
                    .needs_install
                    .iter()
                    .map(|(n, v)| (n.as_str(), v.as_str())),
            );

            // Group tools by package manager for batch installation
            let tool_groups = tools::spec::group_tools_for_installation(
                ordered_tools.iter().map(|(n, v)| (n.as_str(), v.as_str())),
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
                // Emit setup_started event
                telemetry::setup_started(version_check.needs_install.len());
                let setup_start = telemetry::now();

                // Batch install by package manager
                for (pm, packages) in &tool_groups.by_package_manager {
                    if packages.is_empty() {
                        continue;
                    }

                    // Emit tool_requested for each tool in the batch
                    for (tool_name, _, version) in packages {
                        telemetry::tool_requested(tool_name, version, telemetry::Source::Config);
                    }

                    let package_names: Vec<&str> =
                        packages.iter().map(|(_, pkg, _)| pkg.as_str()).collect();
                    println!(
                        "Batch installing {} packages via {:?}: {}",
                        packages.len(),
                        pm,
                        package_names.join(", ")
                    );

                    let install_start = telemetry::now();
                    match tools::common::PkgOps::batch_install(*pm, &package_names, None) {
                        Ok(result) => {
                            let batch_duration = install_start.elapsed();
                            // Track successful installs
                            for pkg_name in &result.succeeded {
                                // Find the tool name for this package
                                if let Some((tool_name, _, version)) =
                                    packages.iter().find(|(_, pkg, _)| pkg == pkg_name)
                                {
                                    println!("Successfully installed {} ({})", tool_name, version);
                                    successfully_installed
                                        .push((tool_name.clone(), version.clone()));
                                    telemetry::tool_installed(
                                        tool_name,
                                        version,
                                        &format!("{:?}", pm),
                                        batch_duration,
                                    );
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
                                    telemetry::tool_failed(tool_name, version, error);
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
                                telemetry::tool_failed(tool_name, version, &format!("{:?}", e));
                            }
                        }
                    }
                }

                // Install custom tools with configurable parallelism
                // User-space installers (nvm, rustup, etc.) don't require system locks
                // and can safely run in parallel
                if !tool_groups.custom_install.is_empty() {
                    // Emit tool_requested for each custom tool
                    for (name, version) in &tool_groups.custom_install {
                        telemetry::tool_requested(name, version, telemetry::Source::Config);
                    }

                    let custom_count = tool_groups.custom_install.len();
                    let effective_jobs = parallel_jobs.min(custom_count);

                    if effective_jobs > 1 {
                        println!(
                            "Installing {} custom tools with {} parallel jobs",
                            custom_count, effective_jobs
                        );

                        // Configure thread pool for this installation phase
                        let pool = rayon::ThreadPoolBuilder::new()
                            .num_threads(effective_jobs)
                            .build()
                            .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap());

                        // Thread-safe collectors for results
                        let success_collector: Arc<Mutex<Vec<(String, String)>>> =
                            Arc::new(Mutex::new(Vec::new()));
                        let error_collector: Arc<Mutex<Vec<(String, String, String)>>> =
                            Arc::new(Mutex::new(Vec::new()));

                        pool.install(|| {
                            tool_groups
                                .custom_install
                                .par_iter()
                                .for_each(|(name, version)| {
                                    // Note: println! is thread-safe in Rust
                                    println!(
                                        "Installing {} version {} using custom installer",
                                        name, version
                                    );

                                    match tools::add(name, version) {
                                        Ok(()) => {
                                            println!(
                                                "Successfully installed {} ({})",
                                                name, version
                                            );
                                            if let Ok(mut guard) = success_collector.lock() {
                                                guard.push((name.clone(), version.clone()));
                                            }
                                        }
                                        Err(e) => {
                                            let msg = format!(
                                                "Failed to install {} ({}): {:?}",
                                                name, version, e
                                            );
                                            eprintln!("{}", msg);
                                            if let Ok(mut guard) = error_collector.lock() {
                                                guard.push((
                                                    name.clone(),
                                                    version.clone(),
                                                    format!("{:?}", e),
                                                ));
                                            }
                                        }
                                    }
                                });
                        });

                        // Merge successful installs
                        if let Ok(guard) = success_collector.lock() {
                            successfully_installed.extend(guard.iter().cloned());
                        }

                        // Report errors to telemetry
                        if let Ok(guard) = error_collector.lock() {
                            for (name, version, error) in guard.iter() {
                                telemetry::tool_failed(name, version, error);
                            }
                        }
                    } else {
                        // Sequential installation (--sequential or --jobs 1)
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
                                    let msg = format!(
                                        "Failed to install {} ({}): {:?}",
                                        name, version, e
                                    );
                                    eprintln!("{}", msg);
                                    telemetry::tool_failed(name, version, &format!("{:?}", e));
                                }
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

            // Emit setup_completed telemetry (note: setup_start may not exist in dry-run)
            // We only emit this if there was an actual setup (not dry-run)

            // Mark as initialized after successful setup (first-run complete)
            if !dry_run {
                let _ = mark_initialized();
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
                    println!("  - Secrets: configured");
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
        Some(Commands::Doctor {
            file,
            tools,
            output_format,
            extended,
            report,
        }) => {
            let config = file.as_ref().map(|f| Config::new(f));
            let specific_tools = tools.as_ref().map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<_>>()
            });

            if *extended {
                let result = commands::doctor::run_doctor_extended(config.as_ref(), specific_tools);

                // Handle report export for extended mode
                if let Some(report_path) = report {
                    match commands::doctor::export_report(&result, &report_path) {
                        Ok(_) => println!("Report exported to: {}", report_path),
                        Err(e) => eprintln!("Failed to export report: {}", e),
                    }
                }

                let output = if output_format == "json" {
                    serde_json::to_string_pretty(&result)
                        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
                } else {
                    use crate::output::Outputable;
                    result.to_human()
                };
                println!("{}", output);

                use crate::output::Outputable;
                std::process::exit(result.exit_code().code());
            } else {
                let result = commands::doctor::run_doctor(config.as_ref(), specific_tools);

                let output = if output_format == "json" {
                    serde_json::to_string_pretty(&result)
                        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
                } else {
                    use crate::output::Outputable;
                    result.to_human()
                };
                println!("{}", output);

                use crate::output::Outputable;
                std::process::exit(result.exit_code().code());
            }
        }
        Some(Commands::Diff {
            file,
            changes_only,
            output_format,
        }) => {
            let config = Config::new(file);
            let result = commands::diff::run_diff(&config, *changes_only);

            let output = if output_format == "json" {
                serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
            } else {
                use crate::output::Outputable;
                result.to_human()
            };
            println!("{}", output);

            use crate::output::Outputable;
            std::process::exit(result.exit_code().code());
        }
        Some(Commands::Export {
            tools,
            all,
            verbose,
            output_format,
            output,
        }) => {
            let filter_tools = tools.as_ref().map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<_>>()
            });

            let result = commands::export::export_tools(filter_tools, *all, *verbose);

            use crate::output::Outputable;
            let content = if output_format == "json" {
                result.to_json()
            } else {
                result.to_human()
            };

            if let Some(path) = output {
                if let Err(e) = fs::write(path, &content) {
                    eprintln!("Failed to write output: {}", e);
                    std::process::exit(1);
                }
                println!("Exported to: {}", path);
            } else {
                println!("{}", content);
            }

            std::process::exit(result.exit_code().code());
        }
        Some(Commands::Upgrade {
            file,
            tools,
            dry_run,
            force,
            output_format,
        }) => {
            let config = file.as_ref().map(|f| Config::new(f));
            let specific_tools = tools.as_ref().map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<_>>()
            });

            let result =
                commands::upgrade::run_upgrade(config.as_ref(), specific_tools, *dry_run, *force);

            let output = if output_format == "json" {
                serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
            } else {
                use crate::output::Outputable;
                result.to_human()
            };
            println!("{}", output);

            use crate::output::Outputable;
            std::process::exit(result.exit_code().code());
        }
        Some(Commands::Init {
            template,
            non_interactive,
            stdout,
            output,
        }) => {
            let options = commands::init::InitOptions {
                template: template.clone(),
                non_interactive: *non_interactive,
                stdout: *stdout,
                output: output.as_ref().map(|s| std::path::PathBuf::from(s)),
            };

            let result = commands::init::run_init(options);

            use crate::output::Outputable;
            let content = result.to_human();
            if !content.is_empty() {
                print!("{}", content);
            }

            std::process::exit(result.exit_code().code());
        }
        Some(Commands::Search {
            query,
            all,
            output_format,
        }) => {
            let query_str = query.as_deref().unwrap_or("");
            let result = commands::search::search_tools(query_str, *all);

            let output = if output_format == "json" {
                serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
            } else {
                use crate::output::Outputable;
                result.to_human()
            };
            println!("{}", output);

            use crate::output::Outputable;
            std::process::exit(result.exit_code().code());
        }
        Some(Commands::Validate {
            file,
            from,
            strict,
            header,
            output_format,
        }) => {
            // Determine config file path: fetch from URL or use local file
            let config_path = if let Some(url) = from {
                match fetch_remote_config(url, false, header) {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Error fetching remote config: {}", e);
                        std::process::exit(crate::error_codes::CONFIG_ERROR);
                    }
                }
            } else {
                file.clone()
            };

            let result = commands::validate::validate_config(&config_path, *strict);

            let output = if output_format == "json" {
                serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
            } else {
                use crate::output::Outputable;
                result.to_human()
            };
            println!("{}", output);

            use crate::output::Outputable;
            std::process::exit(result.exit_code().code());
        }
        Some(Commands::Completions {
            shell,
            instructions,
        }) => {
            if *instructions {
                println!("{}", commands::completions::get_install_instructions());
                return;
            }

            let shell_type: commands::completions::CompletionShell = match shell.parse() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            // Build the CLI command for completion generation
            let mut cmd = Cli::command();
            let completions =
                commands::completions::generate_completions_string(&mut cmd, shell_type);
            println!("{}", completions);
        }
        Some(Commands::Templates { action }) => {
            use crate::output::Outputable;
            match action {
                TemplatesSubcommand::List {} => {
                    let result = commands::templates::list_templates();
                    println!("{}", result.to_human());
                    std::process::exit(result.exit_code().code());
                }
                TemplatesSubcommand::Show { name } => {
                    let result = commands::templates::show_template(name);
                    println!("{}", result.to_human());
                    std::process::exit(result.exit_code().code());
                }
                TemplatesSubcommand::Use { name, output, setup } => {
                    let output_path = output.as_ref().map(|s| std::path::PathBuf::from(s));
                    let result = commands::templates::use_template(name, output_path);
                    println!("{}", result.to_human());

                    // If setup requested and file was created, run setup
                    if *setup && result.created {
                        println!("\nRunning setup...\n");
                        // TODO: Call setup command
                    }

                    std::process::exit(result.exit_code().code());
                }
            }
        }
        Some(Commands::Quickstart {
            non_interactive,
            skip_check,
        }) => {
            let options = commands::quickstart::QuickstartOptions {
                non_interactive: *non_interactive,
                skip_check: *skip_check,
            };

            let result = commands::quickstart::run_quickstart(options);

            use crate::output::Outputable;
            println!("{}", result.to_human());

            // Mark as initialized after quickstart (first-run complete)
            if !result.aborted {
                let _ = mark_initialized();
            }

            std::process::exit(result.exit_code().code());
        }
        Some(Commands::Telemetry { action }) => {
            handle_telemetry_command(action, &global_config);
        }
        Some(Commands::Mcp { config }) => {
            if let Err(e) = mcp::run(config.clone()) {
                eprintln!("MCP server error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Diagnose { tool, fix, export, scope, output_format }) => {
            commands::diagnose::run_diagnose(
                tool,
                *fix,
                *export,
                scope,
                output_format,
            );
        }
        Some(Commands::Team { action }) => {
            handle_team_command(action);
        }
        Some(Commands::Roles { file, action }) => {
            handle_roles_command(file, action);
        }
        Some(Commands::Lock { action }) => {
            handle_lock_command(action);
        }
        Some(Commands::Config { action }) => {
            handle_config_command(action);
        }
        Some(Commands::Update { action, version, channel, method, rollback }) => {
            let update_action = match action {
                Some(UpdateSubcommand::Check { channel: ch }) => {
                    let ch = ch.as_ref().or(channel.as_ref());
                    update::UpdateAction::Check {
                        channel: ch.and_then(|c| parse_update_channel(c)),
                    }
                }
                Some(UpdateSubcommand::History {}) => update::UpdateAction::History,
                Some(UpdateSubcommand::Config {}) => update::UpdateAction::Config,
                Some(UpdateSubcommand::Enable {}) => update::UpdateAction::Enable,
                Some(UpdateSubcommand::Disable {}) => update::UpdateAction::Disable,
                None => {
                    // Default: install update
                    update::UpdateAction::Install {
                        version: version.clone(),
                        channel: channel.as_ref().and_then(|c| parse_update_channel(c)),
                        method: method.as_ref().and_then(|m| parse_install_method(m)),
                        rollback: *rollback,
                    }
                }
            };
            std::process::exit(update::run_update_command(update_action));
        }
        None => {
            user_select();
        }
        Some(Commands::External(_)) => unreachable!("External subcommand handled before init"),
    }
}

/// Handle telemetry subcommands
fn handle_telemetry_command(action: &TelemetryAction, global_config: &init::CliConfig) {
    match action {
        TelemetryAction::Status {} => {
            let config = telemetry::config();
            println!("Telemetry Configuration");
            println!("=======================");
            if let Some(cfg) = config {
                println!(
                    "Status:    {}",
                    if cfg.is_enabled() {
                        "\x1b[32menabled\x1b[0m"
                    } else {
                        "\x1b[33mdisabled\x1b[0m"
                    }
                );
                println!(
                    "Endpoint:  {} ({})",
                    cfg.endpoint,
                    cfg.protocol.to_uppercase()
                );
                println!(
                    "Signals:   logs={}, metrics={}, traces={}",
                    if cfg.logs { "on" } else { "off" },
                    if cfg.metrics { "on" } else { "off" },
                    if cfg.traces { "on" } else { "off" }
                );
                println!("Sample:    {}%", (cfg.sample_rate * 100.0) as u32);
            } else {
                println!("Status:    \x1b[33mnot initialized\x1b[0m");
            }
            println!();
            println!("Configuration sources:");
            println!("  - Config file: ~/.jarvy/config.toml [telemetry] section");
            println!("  - Environment: JARVY_TELEMETRY, JARVY_OTLP_ENDPOINT");
        }
        TelemetryAction::Enable {} => {
            update_telemetry_config(true, None);
            println!("Telemetry enabled.");
            println!("Configure endpoint with: jarvy telemetry set-endpoint <url>");
        }
        TelemetryAction::Disable {} => {
            update_telemetry_config(false, None);
            println!("Telemetry disabled.");
        }
        TelemetryAction::SetEndpoint { url } => {
            update_telemetry_config(true, Some(url.clone()));
            println!("Endpoint set to: {}", url);
        }
        TelemetryAction::Test {} => {
            let config = telemetry::config();
            if let Some(cfg) = config {
                if !cfg.is_enabled() {
                    println!("Telemetry is disabled. Enable with: jarvy telemetry enable");
                    return;
                }
                println!("Sending test event to {}...", cfg.endpoint);
                telemetry::command_executed(
                    "telemetry_test",
                    std::time::Duration::from_millis(1),
                    true,
                );
                // Give exporters a moment to ship
                std::thread::sleep(std::time::Duration::from_millis(500));
                println!("Test event sent. Check your OTEL backend for:");
                println!("  - Event: command.executed");
                println!("  - Command: telemetry_test");
            } else {
                println!("Telemetry not initialized.");
            }
        }
        TelemetryAction::Preview {} => {
            println!("Telemetry Events Preview");
            println!("========================");
            println!();
            println!("On next setup, the following events would be sent:");
            println!();
            println!("Tool Events:");
            println!("  - tool.requested   (per tool in config)");
            println!("  - tool.installed   (for each successful install)");
            println!("  - tool.failed      (for each failed install)");
            println!("  - tool.not_supported (for unknown tools)");
            println!();
            println!("Setup Events:");
            println!("  - setup.started    (when setup begins)");
            println!("  - setup.completed  (summary with counts/duration)");
            println!();
            println!("Hook Events:");
            println!("  - hook.started     (when hook begins)");
            println!("  - hook.completed   (on success)");
            println!("  - hook.failed      (on error)");
            println!("  - hook.timeout     (if hook exceeds timeout)");
            println!();
            println!("Metrics:");
            println!("  - jarvy.tool.requests      (counter)");
            println!("  - jarvy.tool.installs      (counter by status)");
            println!("  - jarvy.install.duration   (histogram in seconds)");
            println!("  - jarvy.setup.duration     (histogram in seconds)");
            println!();
            println!("Privacy: File paths and secrets are redacted before sending.");
        }
    }
}

/// Update telemetry configuration in ~/.jarvy/config.toml
fn update_telemetry_config(enabled: bool, endpoint: Option<String>) {
    let home_dir = match dirs::home_dir() {
        Some(h) => h,
        None => {
            eprintln!("Could not determine home directory");
            return;
        }
    };
    let config_path = home_dir.join(".jarvy").join("config.toml");

    // Read existing config
    let mut config: init::CliConfig = if config_path.exists() {
        let content = fs::read_to_string(&config_path).unwrap_or_default();
        toml::from_str(&content).unwrap_or_default()
    } else {
        init::CliConfig::default()
    };

    // Update telemetry settings
    config.telemetry.enabled = enabled;
    if let Some(ep) = endpoint {
        config.telemetry.endpoint = ep;
    }

    // Write back
    match toml::to_string_pretty(&config) {
        Ok(content) => {
            if let Err(e) = fs::write(&config_path, content) {
                eprintln!("Failed to write config: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to serialize config: {}", e);
        }
    }
}

/// Maximum size for remote config files (1MB)
const MAX_REMOTE_CONFIG_SIZE: u64 = 1024 * 1024;

/// Fetch a jarvy.toml configuration from a remote URL with caching
///
/// PRD-015/016: Remote config loading support
///
/// Supports:
/// - GitHub raw URLs
/// - Gist URLs
/// - Any HTTP/HTTPS URL returning TOML content
/// - Custom headers for authenticated requests
///
/// Caching:
/// - Configs are cached in ~/.jarvy/cache/configs/
/// - Cache expires after 1 hour
/// - Use --insecure to skip SSL verification (not recommended)
///
/// Security:
/// - Enforces 1MB size limit to prevent memory exhaustion
fn fetch_remote_config(url: &str, _insecure: bool, headers: &[String]) -> Result<String, String> {
    use std::io::{Read, Write};
    use std::time::Duration;

    // Get cache directory
    let cache_dir = dirs::home_dir()
        .ok_or("Could not determine home directory")?
        .join(".jarvy")
        .join("cache")
        .join("configs");

    // Create cache directory if it doesn't exist
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create cache directory: {}", e))?;
    }

    // Generate cache key from URL (simple hash)
    let cache_key = url
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    let cache_file = cache_dir.join(format!("{:x}.toml", cache_key));
    let cache_meta = cache_dir.join(format!("{:x}.meta", cache_key));

    // Check if cached file exists and is fresh (< 1 hour old)
    let cache_valid = if cache_file.exists() && cache_meta.exists() {
        if let Ok(metadata) = fs::metadata(&cache_meta) {
            if let Ok(modified) = metadata.modified() {
                modified
                    .elapsed()
                    .map(|d| d < Duration::from_secs(3600))
                    .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    if cache_valid {
        println!("Using cached config from {}", url);
        return Ok(cache_file.to_string_lossy().to_string());
    }

    println!("Fetching config from {}...", url);

    // Transform GitHub URLs to raw URLs if needed
    let fetch_url = transform_github_url(url);

    // Create HTTP agent
    let agent = ureq::Agent::new_with_defaults();

    // Build the request with default headers
    let mut request = agent
        .get(&fetch_url)
        .header(
            "User-Agent",
            "Jarvy/0.1 (https://github.com/bearbinary/jarvy)",
        )
        .header("Accept", "text/plain, application/toml, */*");

    // Add custom headers (for authentication, etc.)
    for header in headers {
        if let Some((key, value)) = header.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if !key.is_empty() && !value.is_empty() {
                request = request.header(key, value);
            } else {
                eprintln!(
                    "Warning: Invalid header format '{}', expected 'Name: Value'",
                    header
                );
            }
        } else {
            eprintln!(
                "Warning: Invalid header format '{}', expected 'Name: Value'",
                header
            );
        }
    }

    // Fetch the config
    let response = request
        .call()
        .map_err(|e| format!("Failed to fetch config: {}", e))?;

    if response.status() != 200 {
        return Err(format!("HTTP error {}", response.status()));
    }

    // Check content-length header if available
    if let Some(content_length) = response.headers().get("content-length") {
        if let Some(length) = content_length
            .to_str()
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
        {
            if length > MAX_REMOTE_CONFIG_SIZE {
                return Err(format!(
                    "Remote config too large: {} bytes (max {} bytes)",
                    length, MAX_REMOTE_CONFIG_SIZE
                ));
            }
        }
    }

    // Read with size limit (even if Content-Length was not present or was incorrect)
    let mut content = String::new();
    let mut body = response.into_body();
    let mut reader = body.as_reader();
    let mut limited_reader = reader.take(MAX_REMOTE_CONFIG_SIZE + 1);

    limited_reader
        .read_to_string(&mut content)
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    // Check if we hit the limit
    if content.len() as u64 > MAX_REMOTE_CONFIG_SIZE {
        return Err(format!(
            "Remote config too large: exceeds {} bytes limit",
            MAX_REMOTE_CONFIG_SIZE
        ));
    }

    // Validate that content is valid TOML
    let _: toml::Value =
        toml::from_str(&content).map_err(|e| format!("Invalid TOML in remote config: {}", e))?;

    // Write to cache
    let mut file =
        fs::File::create(&cache_file).map_err(|e| format!("Failed to create cache file: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write cache file: {}", e))?;

    // Write metadata (URL for reference)
    let mut meta_file = fs::File::create(&cache_meta)
        .map_err(|e| format!("Failed to create cache metadata: {}", e))?;
    meta_file
        .write_all(url.as_bytes())
        .map_err(|e| format!("Failed to write cache metadata: {}", e))?;

    println!("Config cached at {}", cache_file.display());

    Ok(cache_file.to_string_lossy().to_string())
}

/// Transform GitHub URLs to raw content URLs
fn transform_github_url(url: &str) -> String {
    // Transform github.com blob URLs to raw.githubusercontent.com
    // e.g., https://github.com/user/repo/blob/main/jarvy.toml
    // -> https://raw.githubusercontent.com/user/repo/main/jarvy.toml
    if url.contains("github.com") && url.contains("/blob/") {
        return url
            .replace("github.com", "raw.githubusercontent.com")
            .replace("/blob/", "/");
    }

    // Transform gist URLs to raw
    // e.g., https://gist.github.com/user/hash
    // -> https://gist.githubusercontent.com/user/hash/raw
    if url.contains("gist.github.com") && !url.contains("/raw") {
        return format!("{}/raw", url.trim_end_matches('/'));
    }

    url.to_string()
}

fn user_select() {
    // Test mode: avoid interactive prompts and side-effects
    if std::env::var("JARVY_TEST_MODE").as_deref() == Ok("1") {
        println!("TEST: user_select invoked");
        return;
    }

    // Check if this is the first run
    if is_first_run() {
        // Show welcome banner for first-time users
        let use_colors = std::io::IsTerminal::is_terminal(&std::io::stdout());
        show_welcome_banner(&WelcomeBannerConfig {
            enabled: true,
            use_colors,
        });

        // Offer first-run options
        let options = vec![
            "Run quickstart (guided setup)",
            "Create a config (jarvy init)",
            "Browse templates",
            "Skip for now",
        ];

        let selection: Result<&str, InquireError> =
            Select::new("How would you like to get started?", options).prompt();

        match selection {
            Ok(choice) => match choice {
                "Run quickstart (guided setup)" => {
                    let options = commands::quickstart::QuickstartOptions::default();
                    let result = commands::quickstart::run_quickstart(options);
                    use crate::output::Outputable;
                    println!("{}", result.to_human());
                    // Mark as initialized after quickstart
                    let _ = mark_initialized();
                }
                "Create a config (jarvy init)" => {
                    let options = commands::init::InitOptions::default();
                    let result = commands::init::run_init(options);
                    use crate::output::Outputable;
                    print!("{}", result.to_human());
                    // Mark as initialized after init
                    let _ = mark_initialized();
                }
                "Browse templates" => {
                    let result = commands::templates::list_templates();
                    use crate::output::Outputable;
                    println!("{}", result.to_human());
                }
                _ => {
                    println!("\nYou can always run these later:");
                    println!("  \x1b[36mjarvy quickstart\x1b[0m  - Guided setup");
                    println!("  \x1b[36mjarvy init\x1b[0m        - Create a config");
                    println!("  \x1b[36mjarvy templates\x1b[0m   - Browse templates\n");
                }
            },
            Err(_) => {
                println!("No choice was made");
            }
        }
        return;
    }

    // Normal flow for returning users
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

/// Handle team subcommands
fn handle_team_command(action: &TeamAction) {
    use team::registry::Registry;

    match action {
        TeamAction::Add {
            name,
            url,
            description,
        } => {
            let mut registry = Registry::load();
            match registry.add_source(name, url, description.as_deref()) {
                Ok(()) => {
                    if let Err(e) = registry.save() {
                        eprintln!("Warning: Failed to save registry: {}", e);
                    }
                    println!("Added team source '{}' -> {}", name, url);
                    println!("Run 'jarvy team sync {}' to fetch available configs.", name);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        TeamAction::List {} => {
            let registry = Registry::load();
            let sources = registry.list_sources();

            if sources.is_empty() {
                println!("No team sources registered.");
                println!("Add one with: jarvy team add <name> <url>");
                return;
            }

            println!("Team Configuration Sources");
            println!("==========================");
            for source in sources {
                println!();
                println!("  {} ({})", source.name, source.url);
                if let Some(ref desc) = source.description {
                    println!("    {}", desc);
                }
                if let Some(last_sync) = source.last_sync {
                    let ago = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs().saturating_sub(last_sync))
                        .unwrap_or(0);
                    println!(
                        "    Last sync: {}s ago ({} configs)",
                        ago,
                        source.configs.len()
                    );
                } else {
                    println!("    Not synced yet");
                }
            }
        }
        TeamAction::Browse { source } => {
            let registry = Registry::load();
            match registry.get_source(source) {
                Some(src) => {
                    if src.configs.is_empty() {
                        println!(
                            "No configs found for '{}'. Run 'jarvy team sync {}' first.",
                            source, source
                        );
                        return;
                    }
                    println!("Available configs from '{}':", source);
                    println!();
                    for config in &src.configs {
                        println!("  {}/{}", source, config.name);
                        if let Some(ref desc) = config.description {
                            println!("    {}", desc);
                        }
                        if !config.tags.is_empty() {
                            println!("    Tags: {}", config.tags.join(", "));
                        }
                    }
                }
                None => {
                    eprintln!("Source '{}' not found.", source);
                    std::process::exit(1);
                }
            }
        }
        TeamAction::Sync { source } => {
            let mut registry = Registry::load();

            let sources_to_sync: Vec<String> = match source {
                Some(s) => vec![s.clone()],
                None => registry.sources.keys().cloned().collect(),
            };

            if sources_to_sync.is_empty() {
                println!("No sources to sync.");
                return;
            }

            for source_name in sources_to_sync {
                print!("Syncing '{}'... ", source_name);
                match registry.sync_source(&source_name) {
                    Ok(count) => {
                        println!("found {} configs", count);
                    }
                    Err(e) => {
                        println!("failed: {}", e);
                    }
                }
            }

            if let Err(e) = registry.save() {
                eprintln!("Warning: Failed to save registry: {}", e);
            }
        }
        TeamAction::Remove { name } => {
            let mut registry = Registry::load();
            match registry.remove_source(name) {
                Ok(_) => {
                    if let Err(e) = registry.save() {
                        eprintln!("Warning: Failed to save registry: {}", e);
                    }
                    println!("Removed team source '{}'", name);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        TeamAction::Init { from, output } => {
            let registry = Registry::load();
            match registry.get_config_url(from) {
                Some(url) => {
                    println!("Fetching config from {}...", url);
                    match fetch_remote_config(&url, false, &[]) {
                        Ok(cached_path) => {
                            // Copy to output location
                            if let Err(e) = fs::copy(&cached_path, output) {
                                eprintln!("Failed to write config: {}", e);
                                std::process::exit(1);
                            }
                            println!("Created {} from {}", output, from);
                        }
                        Err(e) => {
                            eprintln!("Error fetching config: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    eprintln!(
                        "Config '{}' not found. Use 'jarvy team browse <source>' to see available configs.",
                        from
                    );
                    std::process::exit(1);
                }
            }
        }
    }
}

/// Handle roles subcommands
fn handle_roles_command(file: &str, action: &roles::RolesAction) {
    let config = config::Config::new(file);

    if let Err(e) = roles::handle_roles_command(
        action.clone(),
        Some(config.get_roles_config()),
        config
            .get_assigned_roles()
            .map(|v| v.first().copied())
            .flatten(),
    ) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Handle lock subcommands
fn handle_lock_command(action: &LockAction) {
    use std::path::Path;

    match action {
        LockAction::Generate { file, output } => {
            let config = config::Config::new(file);
            let tools = config.get_tool_configs();

            println!("Generating lock file from {}...", file);

            match lock::generate_lock(&tools) {
                Ok(lock_file) => {
                    let path = Path::new(output);
                    match lock_file.save(path) {
                        Ok(()) => {
                            println!("Lock file generated: {}", output);
                            println!("  Tools locked: {}", lock_file.tools.len());
                        }
                        Err(e) => {
                            eprintln!("Failed to save lock file: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to generate lock file: {}", e);
                    std::process::exit(1);
                }
            }
        }
        LockAction::Status { lock_file, verbose } => {
            let path = Path::new(lock_file);
            if !path.exists() {
                eprintln!("Lock file not found: {}", lock_file);
                eprintln!("Generate one with: jarvy lock generate");
                std::process::exit(1);
            }

            match lock::LockFile::load(path) {
                Ok(lock) => {
                    let platform = std::env::consts::OS;
                    let result = lock::verify_lock(&lock, platform);

                    println!("Lock File Status");
                    println!("================");
                    println!("File: {}", lock_file);
                    println!("Version: {}", lock.version);
                    println!("Tools: {}", lock.tools.len());
                    println!();

                    if *verbose {
                        for tool in &result.tools {
                            let status_icon = match tool.status {
                                lock::VerificationStatus::Match => "✓",
                                lock::VerificationStatus::VersionMismatch => "✗",
                                lock::VerificationStatus::NotInstalled => "○",
                                lock::VerificationStatus::NotLocked => "?",
                                lock::VerificationStatus::Unknown => "?",
                            };
                            let installed = tool.installed_version.as_deref().unwrap_or("-");
                            println!(
                                "  {} {} (locked: {}, installed: {})",
                                status_icon, tool.name, tool.locked_version, installed
                            );
                        }
                        println!();
                    }

                    println!(
                        "Summary: {} matched, {} mismatched, {} missing",
                        result.matched, result.mismatched, result.missing
                    );

                    if result.all_match {
                        println!("Status: All tools match lock file ✓");
                    } else {
                        println!("Status: Some tools differ from lock file");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load lock file: {}", e);
                    std::process::exit(1);
                }
            }
        }
        LockAction::Verify {
            lock_file,
            output_format,
        } => {
            let path = Path::new(lock_file);
            if !path.exists() {
                eprintln!("Lock file not found: {}", lock_file);
                std::process::exit(1);
            }

            match lock::LockFile::load(path) {
                Ok(lock) => {
                    let platform = std::env::consts::OS;
                    let result = lock::verify_lock(&lock, platform);

                    if output_format == "json" {
                        // JSON output
                        let output: Vec<serde_json::Value> = result
                            .tools
                            .iter()
                            .map(|t| {
                                serde_json::json!({
                                    "name": t.name,
                                    "status": t.status.to_string(),
                                    "locked_version": t.locked_version,
                                    "installed_version": t.installed_version,
                                })
                            })
                            .collect();
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&output).unwrap_or_default()
                        );
                    } else {
                        // Pretty output
                        for tool in &result.tools {
                            let color = match tool.status {
                                lock::VerificationStatus::Match => "\x1b[32m",
                                lock::VerificationStatus::VersionMismatch => "\x1b[33m",
                                lock::VerificationStatus::NotInstalled => "\x1b[31m",
                                _ => "\x1b[90m",
                            };
                            let reset = "\x1b[0m";
                            let installed = tool.installed_version.as_deref().unwrap_or("-");
                            println!(
                                "{}{}{}: locked={}, installed={} [{}]",
                                color,
                                tool.name,
                                reset,
                                tool.locked_version,
                                installed,
                                tool.status
                            );
                        }
                    }

                    if !result.all_match {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load lock file: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

/// Handle config subcommands
fn handle_config_command(action: &ConfigAction) {
    match action {
        ConfigAction::Show {
            file,
            resolved,
            extends_chain,
            output_format,
        } => {
            if *extends_chain {
                // Show the inheritance chain
                let base_path = std::path::Path::new(file)
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

                let mut resolver = team::InheritanceResolver::new().with_base_dir(base_path);
                match resolver.resolve(file) {
                    Ok(_) => {
                        let trace = resolver.trace();
                        println!("Extends Chain for {}", file);
                        println!("========================");
                        for (i, entry) in trace.entries.iter().enumerate() {
                            let indent = "  ".repeat(entry.depth);
                            println!("{}↳ {}", indent, entry.source);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error resolving config: {:?}", e);
                        std::process::exit(1);
                    }
                }
                return;
            }

            if *resolved {
                // Show resolved config with inheritance applied
                let base_path = std::path::Path::new(file)
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

                let mut resolver = team::InheritanceResolver::new().with_base_dir(base_path);
                match resolver.resolve(file) {
                    Ok(extended) => {
                        match output_format.as_str() {
                            "json" => {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&extended).unwrap_or_default()
                                );
                            }
                            "yaml" => {
                                println!(
                                    "{}",
                                    serde_yaml::to_string(&extended).unwrap_or_default()
                                );
                            }
                            _ => {
                                // TOML
                                println!(
                                    "{}",
                                    toml::to_string_pretty(&extended).unwrap_or_default()
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error resolving config: {:?}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Show raw config without inheritance - just read the file
                match fs::read_to_string(file) {
                    Ok(content) => {
                        match output_format.as_str() {
                            "json" => {
                                // Parse TOML then convert to JSON
                                if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                                    println!(
                                        "{}",
                                        serde_json::to_string_pretty(&value).unwrap_or_default()
                                    );
                                } else {
                                    eprintln!("Failed to parse config file");
                                    std::process::exit(1);
                                }
                            }
                            "yaml" => {
                                // Parse TOML then convert to YAML
                                if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                                    println!(
                                        "{}",
                                        serde_yaml::to_string(&value).unwrap_or_default()
                                    );
                                } else {
                                    eprintln!("Failed to parse config file");
                                    std::process::exit(1);
                                }
                            }
                            _ => {
                                // Just output the raw TOML
                                println!("{}", content);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read config file: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        ConfigAction::Refresh { file, force } => {
            let base_path = std::path::Path::new(file)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

            let mut resolver = team::InheritanceResolver::new().with_base_dir(base_path);

            if *force {
                // Clear cache for this config's dependencies
                let _cache = team::ConfigCache::new();
                println!("Clearing config cache...");
                // Note: We'd need to implement cache clearing in ConfigCache
                // For now, just re-resolve which will refresh stale entries
            }

            println!("Resolving config from {}...", file);
            match resolver.resolve(file) {
                Ok(_extended) => {
                    let trace = resolver.trace();
                    println!("Config resolved successfully.");
                    println!("  Sources: {}", trace.entries.len());
                    for entry in &trace.entries {
                        println!("    - {}", entry.source);
                    }
                }
                Err(e) => {
                    eprintln!("Error resolving config: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
