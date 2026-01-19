//! Subcommand enums for nested CLI commands
//!
//! Contains all subcommand enums used by the main Commands enum.

use clap::Subcommand;

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
