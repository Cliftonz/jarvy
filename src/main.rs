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
mod config;
mod error_codes;
mod init;
mod os_setup;
mod outputs;
mod provisioner;
mod report;
mod setup;
mod tests;
mod tools;

#[derive(Parser)]
#[clap(name = "jarvy", version = "0.2", author = "Zac Clifton")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
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
    Configure {},
    /// Display configured tools vs what is actually installed
    Get {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
        /// Output format: json, yaml, toml, pretty
        #[clap(short = 'F', long = "format", value_enum, default_value = "pretty")]
        format: OutputFormat,
        /// Optional file to write output to; prints to stdout if omitted
        #[clap(short, long)]
        output: Option<String>,
    },
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
    let global_config = initialize();

    init_logging(global_config.telemetry);

    // Run the CLI Parser and commands
    let cli = Cli::parse();

    match &cli.command {
        Commands::Setup { file } => {
            let config = Config::new(file);

            setup();

            let tools = config.get_tool_configs();
            for (id, tool) in tools {
                println!(
                    "Installing {}: {} version {} using package manager: {}",
                    id, tool.name, tool.version, tool.version_manager
                );
                // Call the appropriate installer function here
            }
        }
        Commands::Configure {} => create_default_config(),
        Commands::Get {
            file,
            format,
            output,
        } => {
            let config = Config::new(file);
            let reports = collect_reports(&config);

            let content = match format {
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
        _ => {
            user_select();
        }
    }
}

fn user_select() {
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
