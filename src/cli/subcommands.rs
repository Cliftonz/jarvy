//! Subcommand enums for nested CLI commands
//!
//! Contains all subcommand enums used by the main Commands enum.

use clap::Subcommand;

#[derive(Subcommand)]
pub enum SkillsAction {
    /// Install every skill from `[skills.install]`, or a single named skill
    Install {
        /// Skill name to install. Configured entries use their pinned
        /// version; a name absent from `[skills.install]` is resolved
        /// ad-hoc from library_sources at `latest`.
        name: Option<String>,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Re-fetch from library_sources and reinstall skills whose version/sha changed
    Update {
        /// Skill name to update (defaults to every configured skill)
        name: Option<String>,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Uninstall a skill (SKILL.md + sidecar) from every targeted agent
    Remove {
        /// Skill name to remove
        name: String,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// List skills declared in jarvy.toml + their installation status across agents
    List {},
    /// Drift check: which configured skills are missing / out-of-version per agent
    Status {},
    /// Show which AI agents are detected on disk
    Agents {},
}

#[derive(Subcommand)]
pub enum HooksAction {
    /// Install the configured git hook framework into `.git/hooks/`
    Install {},
    /// Run `pre-commit autoupdate` then reinstall hooks
    Update {},
    /// Show framework + installation status
    Status {},
    /// List configured hooks from `.pre-commit-config.yaml`
    List {},
    /// Run hooks once (defaults to changed files; `--all-files` for whole tree)
    Run {
        /// Run against every tracked file, not just staged changes
        #[clap(long)]
        all_files: bool,
        /// Run a single hook by id (e.g. `--hook black`)
        #[clap(long)]
        hook: Option<String>,
    },
    /// Remove jarvy-installed hooks (calls `pre-commit uninstall`)
    Uninstall {},
}

#[derive(Subcommand)]
pub enum McpRegisterAction {
    /// Show what's in jarvy.toml + agent → path mapping
    List {},
    /// Register the Jarvy MCP server with every targeted agent
    Apply {
        /// Override scope: `user` or `project`
        #[clap(long)]
        scope: Option<String>,
    },
    /// Diff desired vs. on-disk state (exit 1 if drift)
    Check {
        #[clap(long)]
        scope: Option<String>,
    },
    /// Strip jarvy-managed entries from every targeted agent
    Remove {
        #[clap(long)]
        scope: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum AiHooksAction {
    /// List provisioned hooks or the built-in library
    List {
        /// Show the built-in hook library instead of project config
        #[clap(long)]
        library: bool,
    },
    /// Write hook configs to every targeted AI agent
    Apply {
        /// Override scope: `user` or `project`
        #[clap(long)]
        scope: Option<String>,
    },
    /// Diff desired vs. on-disk state (exit 1 if drift)
    Check {
        #[clap(long)]
        scope: Option<String>,
    },
    /// Strip jarvy-managed entries from every targeted agent
    Remove {
        #[clap(long)]
        scope: Option<String>,
    },
    /// Inspect a single library hook (event, matcher, script bodies)
    Test {
        /// Library hook name (e.g. block-rm-rf)
        name: String,
    },
}

#[derive(Subcommand)]
pub enum TemplatesSubcommand {
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
pub enum RegistryAction {
    /// Fetch the remote registry: verify signature, sha-verify each tool
    /// TOML, and cache under ~/.jarvy/tools.d/.remote/. The next
    /// `jarvy setup` / `jarvy validate` run picks up the synced tools
    /// via the plugin loader.
    Sync {},
    /// Show the last sync's metadata (URL, count, timestamp,
    /// signature-verified flag).
    Status {},
    /// Clear the local registry cache. Synced tools disappear on next
    /// startup until you run `jarvy registry sync` again.
    Clear {},
}

#[derive(Subcommand)]
pub enum TelemetryAction {
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
pub enum ServicesAction {
    /// Start project services
    Start {
        /// Run services in the foreground (attached)
        #[clap(long)]
        foreground: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Stop project services
    Stop {
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Show service status
    Status {
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Restart project services
    Restart {
        /// Run services in the foreground (attached)
        #[clap(long)]
        foreground: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
}

#[derive(Subcommand)]
pub enum TeamAction {
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
pub enum LockAction {
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
pub enum ConfigAction {
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
pub enum UpdateSubcommand {
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

#[derive(Subcommand)]
pub enum DriftAction {
    /// Check for configuration drift
    Check {
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Show current state baseline
    Status {
        /// Show detailed tool information
        #[clap(short, long)]
        verbose: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Accept current state as new baseline
    Accept {
        /// Accept specific tools only (comma-separated)
        #[clap(long)]
        tools: Option<String>,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Fix detected drift issues
    Fix {
        /// Show what would be fixed without making changes
        #[clap(long)]
        dry_run: bool,
        /// Force fix non-auto-fixable issues (may require confirmation)
        #[clap(long)]
        force: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
}

#[derive(Clone, Subcommand)]
pub enum LogsAction {
    /// View recent log entries
    View {
        /// Number of lines to show (default: 100)
        #[clap(short = 'n', long, default_value = "100")]
        lines: usize,
        /// Filter by log level (error, warn, info, debug, trace)
        #[clap(short, long)]
        level: Option<String>,
        /// Filter logs containing this text
        #[clap(short, long)]
        grep: Option<String>,
        /// Output format: text, json
        #[clap(short = 'F', long = "format", default_value = "text")]
        output_format: String,
    },
    /// Show log statistics
    Stats {
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Clean old log files
    Clean {
        /// Remove all log files (not just old ones)
        #[clap(long)]
        all: bool,
        /// Show what would be removed without removing
        #[clap(long)]
        dry_run: bool,
        /// Strip matching lines from rotated log files instead of deleting files.
        /// Accepts `event=NAME` (JSON-field match) or a bare substring.
        /// Active jarvy.log is always skipped. Without `--all`, only
        /// files past the retention age are touched.
        #[clap(long, value_name = "PATTERN")]
        filter: Option<String>,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Show logging configuration
    Config {
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
}

#[derive(Subcommand)]
pub enum LibraryAction {
    /// List every cached library (URL, publisher, item counts)
    List {
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Show the items inside one cached library
    Show {
        /// Library URL (or shorthand `github:owner/repo@<ref>`) — as declared in [<subsystem>.library_sources]
        url: String,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Wipe the on-disk library cache (`~/.jarvy/library.d/`)
    Clean {
        /// Show what would be removed without removing
        #[clap(long)]
        dry_run: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Force-refresh every cached library_sources entry declared in jarvy.toml
    Sync {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
}

#[derive(Subcommand)]
pub enum WorkspaceAction {
    /// List all workspace members and their tools
    List {
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Show the resolved tool set for one member (with inheritance applied)
    Show {
        /// Member name as declared in `[workspace] members`
        name: String,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Validate the workspace (members exist, each jarvy.toml parses)
    Validate {
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
}

#[derive(Clone, Subcommand)]
pub enum TicketAction {
    /// Create a new debug ticket
    Create {
        /// Focus on a specific tool
        #[clap(short, long)]
        tool: Option<String>,
        /// Number of log lines to include (default: 500)
        #[clap(short = 'n', long, default_value = "500")]
        logs: usize,
        /// Output path (default: ~/.jarvy/tickets/)
        #[clap(short, long)]
        output: Option<String>,
        /// Show what would be collected without creating ticket
        #[clap(long)]
        dry_run: bool,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Show contents of a ticket
    Show {
        /// Ticket ID or path to ticket ZIP
        ticket: String,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// List existing tickets
    List {
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
    /// Clean old tickets
    Clean {
        /// Remove tickets older than this many days (default: 30)
        #[clap(long, default_value = "30")]
        older_than: u32,
        /// Output format: json, pretty
        #[clap(short = 'F', long = "format", default_value = "pretty")]
        output_format: String,
    },
}
