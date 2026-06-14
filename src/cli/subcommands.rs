//! Subcommand enums for nested CLI commands
//!
//! Contains all subcommand enums used by the main Commands enum.

use clap::Subcommand;

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
    },
    /// Accept current state as new baseline
    Accept {
        /// Accept specific tools only (comma-separated)
        #[clap(long)]
        tools: Option<String>,
    },
    /// Fix detected drift issues
    Fix {
        /// Show what would be fixed without making changes
        #[clap(long)]
        dry_run: bool,
        /// Force fix non-auto-fixable issues (may require confirmation)
        #[clap(long)]
        force: bool,
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
    Stats {},
    /// Clean old log files
    Clean {
        /// Remove all log files (not just old ones)
        #[clap(long)]
        all: bool,
        /// Show what would be removed without removing
        #[clap(long)]
        dry_run: bool,
    },
    /// Show logging configuration
    Config {},
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
    },
    /// Show contents of a ticket
    Show {
        /// Ticket ID or path to ticket ZIP
        ticket: String,
    },
    /// List existing tickets
    List {},
    /// Clean old tickets
    Clean {
        /// Remove tickets older than this many days (default: 30)
        #[clap(long, default_value = "30")]
        older_than: u32,
    },
}
