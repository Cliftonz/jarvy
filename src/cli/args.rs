//! CLI argument definitions for Jarvy
//!
//! Contains the main `Cli` struct and `Commands` enum with all subcommand definitions.

use crate::ci;
use crate::roles;
use crate::update;
use clap::{Parser, Subcommand, ValueEnum};

use super::subcommands::*;

#[derive(Parser)]
#[clap(
    name = "jarvy",
    version = env!("CARGO_PKG_VERSION"),
    author = "Zac Clifton",
    about = "Jarvy: a helper to configure and verify your computer",
    long_about = "Jarvy helps you set up and verify your computer based on a jarvy.toml configuration.\n\nUSAGE:\n    jarvy <COMMAND> [OPTIONS]\n\nEXAMPLES:\n    jarvy --help\n    jarvy configure\n    jarvy setup --file ./jarvy.toml\n    jarvy get --format json --output report.json\n\nRun without a subcommand to use the interactive menu."
)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Option<Commands>,
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
pub enum Commands {
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
        #[clap(long, alias = "plan")]
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
        /// Generate a pre-filled GitHub issue URL and scaffold snippet
        /// for requesting support for an unsupported tool.
        #[clap(long, value_name = "TOOL")]
        request: Option<String>,
        /// With --request, open the pre-filled GitHub issue in the
        /// default browser instead of just printing the URL.
        #[clap(long, requires = "request")]
        open: bool,
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
    CiInfo {
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Scan the project for tooling and suggest a jarvy.toml (PRD-044)
    Discover {
        /// Path to the configuration file to read / update
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Write suggestions into jarvy.toml (creates the file if missing)
        #[clap(long)]
        apply: bool,
        /// Show only tools that aren't already pinned (one `name = "version"` per line)
        #[clap(long)]
        missing: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
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
    /// Sync + inspect the remote tool registry configured in ~/.jarvy/config.toml [registry]
    Registry {
        #[clap(subcommand)]
        action: RegistryAction,
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
        /// Skip Sigstore signature verification (DANGEROUS — only when cosign
        /// is unavailable and you accept supply-chain risk).
        #[clap(long)]
        allow_unsigned: bool,
    },
    /// Detect configuration drift in the environment
    Drift {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        #[clap(subcommand)]
        action: DriftAction,
    },
    /// View and manage log files
    Logs {
        #[clap(subcommand)]
        action: LogsAction,
    },
    /// Generate debug tickets for support
    Ticket {
        #[clap(subcommand)]
        action: TicketAction,
    },
    /// Output shell initialization snippet for RC files.
    /// Add `eval "$(jarvy shell-init)"` to your .bashrc/.zshrc.
    #[clap(name = "shell-init")]
    ShellInit {
        /// Shell type (bash, zsh, fish, sh, powershell). Auto-detected if not specified.
        #[clap(long)]
        shell: Option<String>,
    },
    /// Ensure base tools are installed (lightweight check for shell startup).
    /// Reads tool list from [shell_init] in ~/.jarvy/config.toml.
    Ensure {
        /// Force re-check, ignore stamp file
        #[clap(long)]
        force: bool,
        /// Suppress all output
        #[clap(short, long)]
        quiet: bool,
        /// Run in foreground (override background default)
        #[clap(long)]
        foreground: bool,
    },
    /// Get detailed information about a specific tool
    Explain {
        /// Tool to explain (e.g., 'docker', 'node', 'git')
        tool: String,
        /// Path to the configuration file (optional, for role/version context)
        #[clap(short, long)]
        file: Option<String>,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Run security scanners and produce a unified audit report
    Audit {
        /// Run only a specific scanner (betterleaks, gitleaks, trivy, etc.)
        #[clap(long)]
        tool: Option<String>,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Check jarvy.toml for deprecated patterns and suggest migrations
    Migrate {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Apply migrations (default is dry-run report only)
        #[clap(long)]
        apply: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Output the JSON Schema for jarvy.toml (for editor autocomplete)
    Schema {
        /// Write to file instead of stdout
        #[clap(short, long)]
        output: Option<String>,
    },
    /// Manage AI agent hooks (Claude Code / Cursor / Codex / Windsurf / Cline / Continue)
    AiHooks {
        #[clap(subcommand)]
        action: AiHooksAction,
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
    },
    /// Register the Jarvy MCP server with terminal AI agents
    McpRegister {
        #[clap(subcommand)]
        action: McpRegisterAction,
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
    },
    /// Manage git hook frameworks (pre-commit, husky, lefthook)
    Hooks {
        #[clap(subcommand)]
        action: HooksAction,
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
    },
    /// Install and manage AI agent skills from library_sources (PRD-049 + PRD-054)
    Skills {
        #[clap(subcommand)]
        action: SkillsAction,
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
    },
    /// Catch-all for unknown subcommands and their args
    #[clap(external_subcommand)]
    External(Vec<String>),
}

/// Parse CI provider from string
pub fn parse_ci_provider(s: &str) -> Result<ci::CiProvider, String> {
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

/// Parse update channel from string
pub fn parse_update_channel(s: &str) -> Option<update::Channel> {
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

/// Parse install method from string
pub fn parse_install_method(s: &str) -> Option<update::InstallMethod> {
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
