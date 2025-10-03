use crate::analytics::init_logging;
use crate::config::{Config, create_default_config};
use crate::init::initialize;
use crate::report::{Status, ToolReport, collect_reports};
use crate::setup::setup;
use clap::{Parser, Subcommand, ValueEnum};
use inquire::{InquireError, Select};
use serde::Serialize;
use std::fs;

mod analytics;
mod bootstrap;
mod config;
mod error_codes;
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

        // Give exporters a brief moment to ship data.
        std::thread::sleep(std::time::Duration::from_millis(800));
    }

    // Register built-in tools so registry lookups are meaningful
    crate::tools::register_all();

    match &cli.command {
        Some(Commands::Setup { file }) => {
            let config = Config::new(file);

            setup();

            let tools = config.get_tool_configs();
            for (id, tool) in tools {
                // If the tool is not in the registry, log and guide the user
                if crate::tools::get_tool(&tool.name).is_none() {
                    let msg = format!(
                        "We do not currently have support for {} package but we have logged it and will be adding it soon.",
                        tool.name
                    );
                    if crate::posthog::telemetry_enabled() {
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
                        crate::posthog::capture_error("unknown_tool_in_config", &msg, props);
                        eprintln!("{}", msg);
                    } else {
                        eprintln!("{}", msg);
                        eprintln!(
                            "Telemetry is disabled. Please consider creating a feature request here: https://github.com/bearbinary/Jarvy/issues/new"
                        );
                    }
                    continue;
                }

                println!(
                    "Installing {}: {} version {} using package manager: {}",
                    id, tool.name, tool.version, tool.version_manager
                );
                // Call the appropriate installer function here
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
